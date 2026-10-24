use crate::Http;
use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use futures_util::{SinkExt, StreamExt};
use omt_core::{
    AssetClass, AssetId, Candle, ChartDataStatus, DataStatus, InstrumentInfo, Quote, Timeframe,
    TradingStatus, Venue,
};
use omt_data::{MarketState, Subscription};
use rust_decimal::Decimal;
use serde_json::{Value, json};
use std::{collections::BTreeMap, str::FromStr, time::Duration};
use tokio::{
    sync::{mpsc, watch},
    task::JoinSet,
};
use tokio_tungstenite::{connect_async, tungstenite::Message};

pub fn decimal(v: &Value) -> Result<Decimal> {
    Decimal::from_str(
        v.as_str()
            .map(str::to_owned)
            .unwrap_or_else(|| v.to_string())
            .as_str(),
    )
    .context("Invalid decimal in provider response")
}
fn time(ms: i64) -> Result<DateTime<Utc>> {
    DateTime::from_timestamp_millis(ms).context("Invalid provider timestamp")
}
fn step_precision(step: &str) -> Option<u32> {
    let (_, frac) = step.split_once('.')?;
    let pos = frac.find('1')?;
    Some((pos + 1) as u32)
}
pub fn parse_exchange_info(v: &Value) -> Result<Vec<InstrumentInfo>> {
    let symbols = v["symbols"]
        .as_array()
        .context("Missing symbols in exchangeInfo")?;
    let mut out = Vec::with_capacity(symbols.len());
    for r in symbols {
        if r.get("isSpotTradingAllowed")
            .and_then(Value::as_bool)
            .is_some_and(|allowed| !allowed)
        {
            continue;
        }
        let Some(sym) = r["symbol"].as_str() else {
            continue;
        };
        let base_asset = r["baseAsset"].as_str().unwrap_or("").to_ascii_uppercase();
        let quote_asset = r["quoteAsset"].as_str().unwrap_or("").to_ascii_uppercase();
        if sym.is_empty() || base_asset.is_empty() || quote_asset.is_empty() {
            continue;
        }
        let status = TradingStatus::from_binance(r["status"].as_str().unwrap_or("UNKNOWN"));
        let mut price_precision = r["quotePrecision"].as_u64().unwrap_or(8) as u32;
        let mut quantity_precision = r["baseAssetPrecision"].as_u64().unwrap_or(8) as u32;
        if let Some(filters) = r["filters"].as_array() {
            for f in filters {
                match f["filterType"].as_str() {
                    Some("PRICE_FILTER") => {
                        if let Some(p) = f["tickSize"].as_str().and_then(step_precision) {
                            price_precision = p;
                        }
                    }
                    Some("LOT_SIZE") => {
                        if let Some(p) = f["stepSize"].as_str().and_then(step_precision) {
                            quantity_precision = p;
                        }
                    }
                    _ => {}
                }
            }
        }
        out.push(InstrumentInfo {
            asset: AssetId {
                symbol: sym.to_ascii_uppercase(),
                asset_class: AssetClass::Crypto,
                venue: Venue::Binance,
            },
            base_asset,
            quote_asset,
            status,
            price_precision,
            quantity_precision,
            provider: "Binance Spot".into(),
            coverage: "Binance Spot exchange".into(),
        });
    }
    Ok(out)
}
pub async fn exchange_info(http: &Http) -> Result<Vec<InstrumentInfo>> {
    let v = http
        .json("https://data-api.binance.vision/api/v3/exchangeInfo")
        .await?;
    parse_exchange_info(&v)
}
pub fn parse_tickers_24hr(
    v: &Value,
    instruments: &BTreeMap<String, InstrumentInfo>,
) -> Result<Vec<Quote>> {
    let rows = v.as_array().context("Expected 24hr ticker array")?;
    let received_at = Utc::now();
    let mut out = Vec::with_capacity(rows.len());
    for r in rows {
        let Some(sym) = r["symbol"].as_str() else {
            continue;
        };
        let key = format!("BINANCE:{}", sym.to_ascii_uppercase());
        if let Some(info) = instruments.get(&key)
            && info.status != TradingStatus::Trading
        {
            continue;
        }
        let Ok(price) = decimal(&r["lastPrice"]) else {
            continue;
        };
        if price <= Decimal::ZERO {
            continue;
        }
        let (asset, currency, base_asset) = if let Some(info) = instruments.get(&key) {
            (
                info.asset.clone(),
                info.quote_asset.clone(),
                Some(info.base_asset.clone()),
            )
        } else {
            let Ok(asset) = key.parse::<AssetId>() else {
                continue;
            };
            let ccy = asset.currency().to_string();
            let base = Some(asset.base_asset().to_string());
            (asset, ccy, base)
        };
        let close_ms = r["closeTime"]
            .as_i64()
            .filter(|ms| *ms > 0)
            .unwrap_or_else(|| received_at.timestamp_millis());
        let event_at = time(close_ms)
            .unwrap_or(received_at)
            .min(received_at + chrono::Duration::seconds(5));
        let quote = Quote {
            asset,
            price,
            currency,
            change_pct: decimal(&r["priceChangePercent"]).ok(),
            change_abs: decimal(&r["priceChange"]).ok(),
            open_24h: decimal(&r["openPrice"]).ok().filter(|d| *d > Decimal::ZERO),
            high: decimal(&r["highPrice"]).ok().filter(|d| *d > Decimal::ZERO),
            low: decimal(&r["lowPrice"]).ok().filter(|d| *d > Decimal::ZERO),
            volume: decimal(&r["volume"]).ok(),
            quote_volume: decimal(&r["quoteVolume"]).ok(),
            bid: decimal(&r["bidPrice"]).ok().filter(|d| *d > Decimal::ZERO),
            ask: decimal(&r["askPrice"]).ok().filter(|d| *d > Decimal::ZERO),
            vwap: decimal(&r["weightedAvgPrice"])
                .ok()
                .filter(|d| *d > Decimal::ZERO),
            base_asset,
            provider: "Binance Spot".into(),
            coverage: "Binance exchange only".into(),
            event_at,
            received_at,
            status: DataStatus::Live,
            delay_seconds: Some(0),
        };
        if quote.validate().is_ok() {
            out.push(quote);
        }
    }
    Ok(out)
}
pub async fn tickers_24hr(
    http: &Http,
    instruments: &BTreeMap<String, InstrumentInfo>,
) -> Result<Vec<Quote>> {
    let v = http
        .json("https://data-api.binance.vision/api/v3/ticker/24hr")
        .await?;
    parse_tickers_24hr(&v, instruments)
}
pub fn binance_mini_ticker(v: &Value) -> Result<Quote> {
    let sym = v["s"].as_str().context("Missing symbol")?;
    let asset = AssetId {
        symbol: sym.to_ascii_uppercase(),
        asset_class: AssetClass::Crypto,
        venue: Venue::Binance,
    };
    let price = decimal(&v["c"])?;
    let open_24h = decimal(&v["o"]).ok().filter(|d| *d > Decimal::ZERO);
    let change_abs = open_24h.map(|o| price - o);
    let change_pct = open_24h.map(|o| (price - o) / o * Decimal::from(100));
    let currency = asset.currency().to_string();
    let base_asset = (currency != "UNKNOWN").then(|| asset.base_asset().to_string());
    let quote = Quote {
        currency,
        asset,
        price,
        change_pct,
        change_abs,
        open_24h,
        high: decimal(&v["h"]).ok().filter(|d| *d > Decimal::ZERO),
        low: decimal(&v["l"]).ok().filter(|d| *d > Decimal::ZERO),
        volume: decimal(&v["v"]).ok(),
        quote_volume: decimal(&v["q"]).ok(),
        bid: None,
        ask: None,
        vwap: None,
        base_asset,
        provider: "Binance Spot".into(),
        coverage: "Binance exchange only".into(),
        event_at: time(v["E"].as_i64().context("Missing event time")?)?,
        received_at: Utc::now(),
        status: DataStatus::Live,
        delay_seconds: Some(0),
    };
    if quote.price <= Decimal::ZERO {
        bail!("Nonpositive miniTicker price");
    }
    Ok(quote)
}
pub fn binance_mini_ticker_arr(v: &Value) -> Result<Vec<Quote>> {
    let data = v.get("data").unwrap_or(v);
    let arr = data.as_array().context("Expected miniTicker array")?;
    let mut out = Vec::with_capacity(arr.len());
    for item in arr {
        if item["e"].as_str() == Some("24hrMiniTicker")
            && let Ok(q) = binance_mini_ticker(item)
        {
            out.push(q);
        }
    }
    Ok(out)
}
pub fn binance_book_ticker(v: &Value) -> Result<(String, Decimal, Decimal)> {
    let sym = v["s"].as_str().context("Missing bookTicker symbol")?;
    let bid = decimal(&v["b"])?;
    let ask = decimal(&v["a"])?;
    if bid <= Decimal::ZERO || ask <= Decimal::ZERO || ask < bid {
        bail!("Invalid bookTicker bounds");
    }
    Ok((format!("BINANCE:{}", sym.to_ascii_uppercase()), bid, ask))
}
pub fn binance_ticker(v: &Value) -> Result<Quote> {
    let asset: AssetId = format!("BINANCE:{}", v["s"].as_str().context("Missing symbol")?)
        .parse()
        .map_err(anyhow::Error::msg)?;
    let price = decimal(&v["c"])?;
    let open_24h = decimal(&v["o"]).ok().filter(|d| *d > Decimal::ZERO);
    let change_abs = decimal(&v["p"])
        .ok()
        .or_else(|| open_24h.map(|o| price - o));
    let change_pct = decimal(&v["P"])
        .ok()
        .or_else(|| open_24h.map(|o| (price - o) / o * Decimal::from(100)));
    let base_asset = Some(asset.base_asset().to_string());
    let quote = Quote {
        currency: asset.currency().into(),
        asset,
        price,
        change_pct,
        change_abs,
        open_24h,
        high: decimal(&v["h"]).ok(),
        low: decimal(&v["l"]).ok(),
        volume: decimal(&v["v"]).ok(),
        quote_volume: decimal(&v["q"]).ok(),
        bid: decimal(&v["b"]).ok().filter(|d| *d > Decimal::ZERO),
        ask: decimal(&v["a"]).ok().filter(|d| *d > Decimal::ZERO),
        vwap: decimal(&v["w"]).ok().filter(|d| *d > Decimal::ZERO),
        base_asset,
        provider: "Binance Spot".into(),
        coverage: "Binance exchange only".into(),
        event_at: time(v["E"].as_i64().context("Missing event time")?)?,
        received_at: Utc::now(),
        status: DataStatus::Live,
        delay_seconds: Some(0),
    };
    quote.validate().map_err(anyhow::Error::msg)?;
    Ok(quote)
}
pub fn binance_kline(v: &Value) -> Result<Candle> {
    let k = &v["k"];
    let bar = Candle {
        open_at: time(k["t"].as_i64().context("Missing kline time")?)?,
        open: decimal(&k["o"])?,
        high: decimal(&k["h"])?,
        low: decimal(&k["l"])?,
        close: decimal(&k["c"])?,
        volume: decimal(&k["v"])?,
        closed: k["x"].as_bool().unwrap_or(false),
    };
    if !bar.validate() {
        bail!("Invalid OHLCV");
    }
    Ok(bar)
}
pub fn binance_bars(v: &Value) -> Result<Vec<Candle>> {
    v.as_array()
        .context("Expected bars array")?
        .iter()
        .map(|r| {
            let start = r[0].as_i64().context("Missing candle timestamp")?;
            let bar = Candle {
                open_at: time(start)?,
                open: decimal(&r[1])?,
                high: decimal(&r[2])?,
                low: decimal(&r[3])?,
                close: decimal(&r[4])?,
                volume: decimal(&r[5])?,
                closed: r[6]
                    .as_i64()
                    .is_some_and(|t| t < Utc::now().timestamp_millis()),
            };
            if !bar.validate() {
                bail!("Invalid OHLCV");
            }
            Ok(bar)
        })
        .collect()
}
pub async fn history(http: &Http, asset: &AssetId, tf: Timeframe) -> Result<Vec<Candle>> {
    history_before(http, asset, tf, None).await
}
pub async fn history_before(
    http: &Http,
    asset: &AssetId,
    tf: Timeframe,
    before_ms: Option<i64>,
) -> Result<Vec<Candle>> {
    match asset.venue {
        Venue::Binance => {
            let url = match before_ms {
                Some(end_ms) => format!(
                    "https://data-api.binance.vision/api/v3/klines?symbol={}&interval={tf}&limit=1000&endTime={end_ms}",
                    asset.symbol
                ),
                None => format!(
                    "https://data-api.binance.vision/api/v3/klines?symbol={}&interval={tf}&limit=1000",
                    asset.symbol
                ),
            };
            binance_bars(&http.json(&url).await?)
        }
        Venue::Coinbase => {
            if ![
                Timeframe::M1,
                Timeframe::M5,
                Timeframe::M15,
                Timeframe::H1,
                Timeframe::D1,
            ]
            .contains(&tf)
            {
                bail!("Coinbase does not support 4h candles");
            }
            let v = http
                .json(&format!(
                    "https://api.exchange.coinbase.com/products/{}/candles?granularity={}",
                    asset.symbol,
                    tf.seconds()
                ))
                .await?;
            let mut bars: Vec<_> = v
                .as_array()
                .context("Expected Coinbase candles")?
                .iter()
                .map(|r| {
                    let at =
                        DateTime::from_timestamp(r[0].as_i64().context("Missing timestamp")?, 0)
                            .context("Invalid timestamp")?;
                    let b = Candle {
                        open_at: at,
                        low: decimal(&r[1])?,
                        high: decimal(&r[2])?,
                        open: decimal(&r[3])?,
                        close: decimal(&r[4])?,
                        volume: decimal(&r[5])?,
                        closed: at.timestamp() + tf.seconds() <= Utc::now().timestamp(),
                    };
                    if !b.validate() {
                        bail!("Invalid candle");
                    }
                    Ok(b)
                })
                .collect::<Result<_>>()?;
            bars.sort_by_key(|b| b.open_at);
            Ok(bars)
        }
        Venue::Iex => {
            let (key, secret) = alpaca_credentials()?;
            let end = before_ms
                .and_then(DateTime::from_timestamp_millis)
                .unwrap_or_else(Utc::now);
            // Alpaca otherwise defaults start to today, even with a large limit.
            let start = end - chrono::Duration::seconds(tf.seconds() * 1500);
            let tf = match tf {
                Timeframe::M1 => "1Min",
                Timeframe::M5 => "5Min",
                Timeframe::M15 => "15Min",
                Timeframe::H1 => "1Hour",
                Timeframe::H4 => "4Hour",
                Timeframe::D1 => "1Day",
            };
            let v = http
                .request(
                    http.client
                        .get(format!(
                            "https://data.alpaca.markets/v2/stocks/{}/bars",
                            asset.symbol
                        ))
                        .query(&[
                            ("timeframe", tf),
                            ("feed", "iex"),
                            ("limit", "500"),
                            ("sort", "desc"),
                            ("start", start.to_rfc3339().as_str()),
                            ("end", end.to_rfc3339().as_str()),
                            ("adjustment", "raw"),
                        ])
                        .header("APCA-API-KEY-ID", key)
                        .header("APCA-API-SECRET-KEY", secret),
                )
                .await?;
            let mut bars = v["bars"]
                .as_array()
                .context("No IEX bars")?
                .iter()
                .map(|r| {
                    Ok(Candle {
                        open_at: DateTime::parse_from_rfc3339(
                            r["t"].as_str().context("Missing time")?,
                        )?
                        .with_timezone(&Utc),
                        open: decimal(&r["o"])?,
                        high: decimal(&r["h"])?,
                        low: decimal(&r["l"])?,
                        close: decimal(&r["c"])?,
                        volume: decimal(&r["v"])?,
                        closed: true,
                    })
                })
                .collect::<Result<Vec<_>>>()?;
            bars.reverse();
            if bars.iter().any(|b| !b.validate()) {
                bail!("Invalid IEX bars");
            }
            Ok(bars)
        }
    }
}
pub async fn catalog(http: &Http) -> Result<Vec<String>> {
    Ok(exchange_info(http)
        .await?
        .into_iter()
        .filter(|i| i.status == TradingStatus::Trading)
        .map(|i| i.asset.key())
        .collect())
}
pub fn alpaca_credentials() -> Result<(String, String)> {
    fn credential(name: &str) -> Result<String> {
        let value = std::env::var(name)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .with_context(|| format!("Set {name} for IEX equities"))?;
        Ok(value.trim().to_owned())
    }
    Ok((
        credential("APCA_API_KEY_ID")?,
        credential("APCA_API_SECRET_KEY")?,
    ))
}
enum FeedEvent {
    Universe(Vec<InstrumentInfo>),
    Snapshot(Vec<Quote>),
    MiniTickers(Vec<Quote>, u128),
    Quote(Box<Quote>),
    BookTicker(String, Decimal, Decimal, DateTime<Utc>),
    Candle(Candle),
    History(String, Vec<Candle>),
    ChartStatus(String, ChartDataStatus, Option<String>),
    Status(String, String),
    Reconnect,
    Coalesced(u64),
}
fn send_feed(tx: &mpsc::Sender<FeedEvent>, event: FeedEvent) {
    if tx.try_send(event).is_err() {
        let _ = tx.try_send(FeedEvent::Coalesced(1));
    }
}
pub async fn service(
    http: Http,
    mut subscriptions: watch::Receiver<Subscription>,
    snapshots: watch::Sender<MarketState>,
) {
    let mut state = snapshots.borrow().clone();
    let (tx, mut rx) = mpsc::channel(1024);

    // Persistent Task 1: Binance Spot Universe Discovery & 24hr Snapshot Reconciliation
    let discovery_http = http.clone();
    let discovery_tx = tx.clone();
    let (reconcile_tx, mut reconcile_rx) = mpsc::channel::<()>(4);
    let _discovery_task = tokio::spawn(async move {
        let mut known_instruments = BTreeMap::new();
        let mut interval = tokio::time::interval(Duration::from_secs(300));
        loop {
            if let Ok(info) = exchange_info(&discovery_http).await {
                for item in &info {
                    known_instruments.insert(item.asset.key(), item.clone());
                }
                let _ = discovery_tx.send(FeedEvent::Universe(info)).await;
            }
            if let Ok(quotes) = tickers_24hr(&discovery_http, &known_instruments).await {
                let _ = discovery_tx.send(FeedEvent::Snapshot(quotes)).await;
                let _ = discovery_tx
                    .send(FeedEvent::Status("BINANCE".into(), "Live".into()))
                    .await;
            }
            tokio::select! {
                _ = interval.tick() => {}
                Some(()) = reconcile_rx.recv() => {}
            }
        }
    });

    // Persistent Task 2: Exchange-wide Binance `!miniTicker@arr` stream
    let agg_tx = tx.clone();
    let _agg_task = tokio::spawn(async move {
        let mut attempt = 0u32;
        loop {
            let started = std::time::Instant::now();
            let res = stream_binance_aggregate(&agg_tx).await;
            if started.elapsed() > Duration::from_secs(60) {
                attempt = 0;
            }
            let err = res
                .err()
                .map(|e| e.to_string())
                .unwrap_or_else(|| "Reconnecting".into());
            let _ = agg_tx.send(FeedEvent::Status("BINANCE".into(), err)).await;
            let _ = agg_tx.send(FeedEvent::Reconnect).await;
            attempt = (attempt + 1).min(6);
            let jitter = (Utc::now().timestamp_subsec_millis() % 750) as u64;
            tokio::time::sleep(Duration::from_millis((1u64 << attempt) * 1000 + jitter)).await;
            let _ = reconcile_tx.try_send(());
        }
    });

    // Dynamic TaskSet 3: Active chart klines & selective detailed subscriptions
    let mut detail_tasks = JoinSet::new();
    let mut current_sub = subscriptions.borrow_and_update().clone().normalized();
    spawn_detail_tasks(
        &mut detail_tasks,
        http.clone(),
        current_sub.clone(),
        tx.clone(),
        &mut state,
    );

    let mut tick = tokio::time::interval(Duration::from_millis(33));
    let mut dirty = true;
    loop {
        tokio::select! {
            changed = subscriptions.changed() => {
                if changed.is_err() {
                    return;
                }
                let next_sub = subscriptions.borrow_and_update().clone().normalized();
                if next_sub != current_sub {
                    current_sub = next_sub;
                    detail_tasks.abort_all();
                    spawn_detail_tasks(&mut detail_tasks, http.clone(), current_sub.clone(), tx.clone(), &mut state);
                    dirty = true;
                }
            }
            event = rx.recv() => {
                match event {
                    Some(FeedEvent::Universe(info)) => {
                        state.register_instruments(info);
                    }
                    Some(FeedEvent::Snapshot(quotes)) => {
                        for q in quotes {
                            let key = q.asset.key();
                            if let Some(existing) = state.quotes.get(&key)
                                && existing.event_at > q.event_at
                            {
                                let mut merged = existing.clone();
                                merged.received_at = q.received_at;
                                merged.status = DataStatus::Live;
                                if merged.vwap.is_none() {
                                    merged.vwap = q.vwap;
                                }
                                if merged.bid.is_none() {
                                    merged.bid = q.bid;
                                    merged.ask = q.ask;
                                }
                                state.quotes.insert(key, merged);
                            } else {
                                state.ingest(q);
                            }
                        }
                        state.last_snapshot_at = Some(Utc::now());
                    }
                    Some(FeedEvent::MiniTickers(quotes, norm_us)) => {
                        state.last_normalize_micros = norm_us;
                        for q in quotes {
                            state.ingest(q);
                        }
                        state.venue_status.insert("BINANCE".into(), "Live".into());
                    }
                    Some(FeedEvent::Quote(q)) => {
                        let venue = q.asset.venue.to_string();
                        state.ingest(*q);
                        state.venue_status.insert(venue, "Live".into());
                    }
                    Some(FeedEvent::BookTicker(key, bid, ask, at)) => {
                        state.apply_book_ticker(&key, bid, ask, at);
                    }
                    Some(FeedEvent::Candle(b)) => state.candle(b),
                    Some(FeedEvent::History(key, bars)) => {
                        if state.chart_key == key {
                            let empty = bars.is_empty();
                            state.merge_candles(bars);
                            state.chart_status = if empty {
                                ChartDataStatus::Empty
                            } else {
                                ChartDataStatus::Ready
                            };
                            state.chart_error = None;
                            state.venue_status.remove("History");
                        }
                    }
                    Some(FeedEvent::ChartStatus(key, status, err)) => {
                        if state.chart_key == key {
                            state.chart_status = status;
                            state.chart_error = err;
                        }
                    }
                    Some(FeedEvent::Status(provider, status)) => {
                        if status != "Live" && !status.starts_with("Connected") && !status.starts_with("Optional") {
                            for quote in state
                                .quotes
                                .values_mut()
                                .filter(|q| q.asset.venue.to_string() == provider)
                            {
                                quote.status = DataStatus::Disconnected;
                            }
                        }
                        state.venue_status.insert(provider, status);
                    }
                    Some(FeedEvent::Reconnect) => {
                        state.reconnects = state.reconnects.saturating_add(1);
                    }
                    Some(FeedEvent::Coalesced(n)) => {
                        state.coalesced = state.coalesced.saturating_add(n);
                    }
                    None => return,
                }
                state.status = state
                    .venue_status
                    .iter()
                    .filter(|(k, v)| *k != "IEX" || !v.starts_with("Optional"))
                    .map(|(k, v)| format!("{k}: {v}"))
                    .collect::<Vec<_>>()
                    .join(" | ");
                if state.status.is_empty() {
                    state.status = "Connecting".into();
                }
                dirty = true;
            }
            _ = tick.tick() => {
                if dirty {
                    snapshots.send_replace(state.clone());
                    dirty = false;
                }
            }
        }
    }
}

fn spawn_detail_tasks(
    tasks: &mut JoinSet<()>,
    http: Http,
    sub: Subscription,
    tx: mpsc::Sender<FeedEvent>,
    state: &mut MarketState,
) {
    let key = format!("{}:{}", sub.chart.key(), sub.timeframe);
    if state.chart_key != key {
        state.candles.clear();
        state.candles_received_at = None;
        state.chart_key = key.clone();
    }
    if sub.chart.venue == Venue::Iex && alpaca_credentials().is_err() {
        state.chart_status = ChartDataStatus::AuthenticationRequired;
        state.chart_error = Some(
            "Set APCA_API_KEY_ID and APCA_API_SECRET_KEY in :SETTINGS or env for IEX equities"
                .into(),
        );
        state.venue_status.insert(
            "IEX".into(),
            "Optional (configure APCA_API_KEY_ID in Settings for live IEX)".into(),
        );
    } else {
        if state.candles.is_empty() {
            state.chart_status = ChartDataStatus::Loading;
            state.chart_error = None;
        }
        let history_tx = tx.clone();
        let h = http.clone();
        let chart = sub.chart.clone();
        let tf = sub.timeframe;
        let chart_key = key;
        tasks.spawn(async move {
            loop {
                match history(&h, &chart, tf).await {
                    Ok(b) => {
                        let is_empty = b.is_empty();
                        let _ = history_tx
                            .send(FeedEvent::History(chart_key.clone(), b))
                            .await;
                        let status = if is_empty {
                            ChartDataStatus::Empty
                        } else {
                            ChartDataStatus::Ready
                        };
                        let _ = history_tx
                            .send(FeedEvent::ChartStatus(chart_key.clone(), status, None))
                            .await;
                    }
                    Err(e) => {
                        let err_msg = e.to_string();
                        let status = if err_msg.contains("APCA_API_KEY_ID")
                            || err_msg.contains("401")
                            || err_msg.contains("403")
                        {
                            ChartDataStatus::AuthenticationRequired
                        } else if err_msg.contains("429")
                            || err_msg.contains("418")
                            || err_msg.contains("rate limit")
                        {
                            ChartDataStatus::RateLimited
                        } else if err_msg.contains("does not support")
                            || err_msg.contains("unsupported")
                        {
                            ChartDataStatus::Unsupported
                        } else {
                            ChartDataStatus::Error
                        };
                        let _ = history_tx
                            .send(FeedEvent::ChartStatus(
                                chart_key.clone(),
                                status,
                                Some(err_msg.clone()),
                            ))
                            .await;
                        let _ = history_tx
                            .send(FeedEvent::Status("History".into(), err_msg))
                            .await;
                    }
                }
                tokio::time::sleep(Duration::from_secs(60)).await;
            }
        });
    }

    for venue in [Venue::Binance, Venue::Coinbase, Venue::Iex] {
        let assets: Vec<_> = sub
            .assets
            .iter()
            .filter(|a| a.venue == venue)
            .cloned()
            .collect();
        if !assets.is_empty() {
            if venue == Venue::Iex && alpaca_credentials().is_err() {
                state.venue_status.insert(
                    "IEX".into(),
                    "Optional (configure APCA_API_KEY_ID in Settings for live IEX)".into(),
                );
                continue;
            }
            let tx = tx.clone();
            let sub = sub.clone();
            tasks.spawn(async move {
                stream_loop(venue, assets, sub, tx).await;
            });
        }
    }
}

async fn stream_binance_aggregate(tx: &mpsc::Sender<FeedEvent>) -> Result<()> {
    let url = "wss://data-stream.binance.vision/ws/!miniTicker@arr";
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(12), connect_async(url))
        .await
        .context("Aggregate WebSocket timeout")?
        .map_err(|_| anyhow::anyhow!("Aggregate WebSocket unavailable"))?;
    let _ = tx.try_send(FeedEvent::Status("BINANCE".into(), "Live".into()));
    let rotate = tokio::time::sleep(Duration::from_secs(23 * 3600));
    tokio::pin!(rotate);
    loop {
        let message = tokio::select! {
            _ = &mut rotate => return Ok(()),
            message = tokio::time::timeout(Duration::from_secs(90), socket.next()) => {
                message.context("Aggregate feed timed out")?.context("Aggregate feed closed")??
            }
        };
        match message {
            Message::Ping(p) => socket.send(Message::Pong(p)).await?,
            Message::Close(_) => bail!("Aggregate feed disconnected"),
            Message::Text(text) => {
                let t0 = std::time::Instant::now();
                let value: Value =
                    serde_json::from_str(&text).context("Malformed aggregate JSON")?;
                let quotes = binance_mini_ticker_arr(&value)?;
                let norm_us = t0.elapsed().as_micros();
                if !quotes.is_empty() {
                    send_feed(tx, FeedEvent::MiniTickers(quotes, norm_us));
                }
            }
            _ => {}
        }
    }
}

async fn stream_loop(
    venue: Venue,
    assets: Vec<AssetId>,
    sub: Subscription,
    tx: mpsc::Sender<FeedEvent>,
) {
    let mut attempt = 0u32;
    loop {
        if venue != Venue::Binance {
            let _ = tx
                .send(FeedEvent::Status(venue.to_string(), "Connecting".into()))
                .await;
        }
        let started = std::time::Instant::now();
        let result = stream_once(&venue, &assets, &sub, &tx).await;
        if started.elapsed() > Duration::from_secs(60) {
            attempt = 0;
        }
        let error = result
            .err()
            .map(|e| e.to_string())
            .unwrap_or_else(|| "Planned reconnect".into());
        let permanent = error.contains("credentials")
            || error.contains("authentication")
            || error.contains("permission")
            || error.contains("APCA_");
        if venue != Venue::Binance {
            let _ = tx.send(FeedEvent::Status(venue.to_string(), error)).await;
        }
        if permanent {
            return;
        }
        let _ = tx.send(FeedEvent::Reconnect).await;
        attempt = (attempt + 1).min(6);
        let jitter = (Utc::now().timestamp_subsec_millis() % 750) as u64;
        tokio::time::sleep(Duration::from_millis((1u64 << attempt) * 1000 + jitter)).await;
    }
}
async fn stream_once(
    venue: &Venue,
    assets: &[AssetId],
    sub: &Subscription,
    tx: &mpsc::Sender<FeedEvent>,
) -> Result<()> {
    let url = match venue {
        Venue::Binance => {
            let mut streams = Vec::new();
            for a in assets.iter().take(12) {
                let s = a.symbol.to_ascii_lowercase();
                streams.push(format!("{s}@bookTicker"));
            }
            if sub.chart.venue == Venue::Binance {
                let s = sub.chart.symbol.to_ascii_lowercase();
                streams.push(format!("{s}@ticker"));
                streams.push(format!("{s}@kline_{}", sub.timeframe));
            }
            streams.sort();
            streams.dedup();
            format!(
                "wss://data-stream.binance.vision/stream?streams={}",
                streams.join("/")
            )
        }
        Venue::Coinbase => "wss://ws-feed.exchange.coinbase.com".into(),
        Venue::Iex => {
            alpaca_credentials()?;
            "wss://stream.data.alpaca.markets/v2/iex".into()
        }
    };
    stream_socket(&url, venue, assets, tx).await
}
async fn stream_socket(
    url: &str,
    venue: &Venue,
    assets: &[AssetId],
    tx: &mpsc::Sender<FeedEvent>,
) -> Result<()> {
    let credentials = if *venue == Venue::Iex {
        Some(alpaca_credentials()?)
    } else {
        None
    };
    stream_socket_authenticated(url, venue, assets, tx, credentials).await
}
async fn stream_socket_authenticated(
    url: &str,
    venue: &Venue,
    assets: &[AssetId],
    tx: &mpsc::Sender<FeedEvent>,
    credentials: Option<(String, String)>,
) -> Result<()> {
    let (mut socket, _) = tokio::time::timeout(Duration::from_secs(12), connect_async(url))
        .await
        .context("WebSocket connection timeout")?
        .map_err(|_| anyhow::anyhow!("WebSocket connection unavailable"))?;
    if *venue == Venue::Coinbase {
        socket
            .send(Message::Text(
                json!({
                    "type": "subscribe",
                    "product_ids": assets.iter().map(|a| &a.symbol).collect::<Vec<_>>(),
                    "channels": ["ticker", "heartbeat"]
                })
                .to_string()
                .into(),
            ))
            .await?;
    }
    if *venue == Venue::Iex {
        let (key, secret) = credentials.context("Missing Alpaca credentials")?;
        socket
            .send(Message::Text(
                json!({"action": "auth", "key": key, "secret": secret})
                    .to_string()
                    .into(),
            ))
            .await?;
    }
    let rotate = tokio::time::sleep(Duration::from_secs(23 * 3600));
    tokio::pin!(rotate);
    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.tick().await;
    let mut last_message = tokio::time::Instant::now();
    loop {
        let message = tokio::select! {
            _ = &mut rotate => return Ok(()),
            _ = heartbeat.tick() => {
                if last_message.elapsed() >= Duration::from_secs(90) { bail!("Feed timed out"); }
                socket.send(Message::Ping(Vec::new().into())).await?;
                continue;
            }
            message = socket.next() => message.context("Feed closed")??,
        };
        last_message = tokio::time::Instant::now();
        match message {
            Message::Ping(p) => socket.send(Message::Pong(p)).await?,
            Message::Close(_) => bail!("Feed disconnected"),
            Message::Text(text) => {
                let t0 = std::time::Instant::now();
                let value: Value = serde_json::from_str(&text).context("Malformed stream JSON")?;
                match venue {
                    Venue::Binance => {
                        let v = value.get("data").unwrap_or(&value);
                        if v.is_array() {
                            let quotes = binance_mini_ticker_arr(v)?;
                            if !quotes.is_empty() {
                                send_feed(
                                    tx,
                                    FeedEvent::MiniTickers(quotes, t0.elapsed().as_micros()),
                                );
                                let _ = tx
                                    .try_send(FeedEvent::Status(venue.to_string(), "Live".into()));
                            }
                        } else {
                            let event = match v["e"].as_str() {
                                Some("24hrTicker") => {
                                    Some(FeedEvent::Quote(Box::new(binance_ticker(v)?)))
                                }
                                Some("24hrMiniTicker") => {
                                    Some(FeedEvent::Quote(Box::new(binance_mini_ticker(v)?)))
                                }
                                Some("kline") => Some(FeedEvent::Candle(binance_kline(v)?)),
                                _ if v.get("b").is_some()
                                    && v.get("a").is_some()
                                    && v.get("s").is_some() =>
                                {
                                    let (k, b, a) = binance_book_ticker(v)?;
                                    Some(FeedEvent::BookTicker(k, b, a, Utc::now()))
                                }
                                _ => None,
                            };
                            if let Some(e) = event {
                                send_feed(tx, e);
                                let _ = tx
                                    .try_send(FeedEvent::Status(venue.to_string(), "Live".into()));
                            }
                        }
                    }
                    Venue::Coinbase => {
                        if value["type"] == "error" {
                            bail!("Coinbase subscription rejected");
                        }
                        if value["type"] == "ticker" {
                            let asset: AssetId = format!(
                                "COINBASE:{}",
                                value["product_id"].as_str().context("Missing product")?
                            )
                            .parse()
                            .map_err(anyhow::Error::msg)?;
                            let price = decimal(&value["price"])?;
                            let open = decimal(&value["open_24h"])
                                .ok()
                                .filter(|v| *v > Decimal::ZERO);
                            let base_asset = Some(asset.base_asset().to_string());
                            let q = Quote {
                                currency: asset.currency().into(),
                                asset,
                                price,
                                change_pct: open
                                    .map(|v| (price / v - Decimal::ONE) * Decimal::from(100)),
                                change_abs: open.map(|v| price - v),
                                open_24h: open,
                                high: decimal(&value["high_24h"]).ok(),
                                low: decimal(&value["low_24h"]).ok(),
                                volume: decimal(&value["volume_24h"]).ok(),
                                quote_volume: None,
                                bid: decimal(&value["best_bid"])
                                    .ok()
                                    .filter(|d| *d > Decimal::ZERO),
                                ask: decimal(&value["best_ask"])
                                    .ok()
                                    .filter(|d| *d > Decimal::ZERO),
                                vwap: None,
                                base_asset,
                                provider: "Coinbase Exchange".into(),
                                coverage: "Coinbase exchange only".into(),
                                event_at: DateTime::parse_from_rfc3339(
                                    value["time"].as_str().context("Missing ticker time")?,
                                )?
                                .with_timezone(&Utc),
                                received_at: Utc::now(),
                                status: DataStatus::Live,
                                delay_seconds: Some(0),
                            };
                            q.validate().map_err(anyhow::Error::msg)?;
                            send_feed(tx, FeedEvent::Quote(Box::new(q)));
                            let _ =
                                tx.try_send(FeedEvent::Status(venue.to_string(), "Live".into()));
                        }
                    }
                    Venue::Iex => {
                        for v in value.as_array().context("Invalid Alpaca message")? {
                            if v["T"] == "error" {
                                let code = v["code"].as_u64().unwrap_or(0);
                                if [401, 402, 403, 405, 406, 409].contains(&code) {
                                    bail!(
                                        "Alpaca authentication, permission or subscription limit failure"
                                    );
                                }
                                bail!("Alpaca feed error {code}");
                            }
                            if v["T"] == "subscription" {
                                let _ = tx.try_send(FeedEvent::Status(
                                    venue.to_string(),
                                    "Connected / subscribed; awaiting trades".into(),
                                ));
                            }
                            if v["msg"] == "authenticated" {
                                let _ = tx.try_send(FeedEvent::Status(
                                    venue.to_string(),
                                    "Connected / authenticated; subscribing".into(),
                                ));
                                socket
                                    .send(Message::Text(
                                        json!({
                                            "action": "subscribe",
                                            "trades": assets.iter().map(|a| &a.symbol).collect::<Vec<_>>(),
                                            "bars": assets.iter().map(|a| &a.symbol).collect::<Vec<_>>()
                                        })
                                        .to_string()
                                        .into(),
                                    ))
                                    .await?;
                            }
                            if v["T"] == "t" {
                                let asset: AssetId =
                                    format!("IEX:{}", v["S"].as_str().context("Missing symbol")?)
                                        .parse()
                                        .map_err(anyhow::Error::msg)?;
                                let base_asset = Some(asset.symbol.clone());
                                let q = Quote {
                                    asset,
                                    price: decimal(&v["p"])?,
                                    currency: "USD".into(),
                                    change_pct: None,
                                    change_abs: None,
                                    open_24h: None,
                                    high: None,
                                    low: None,
                                    volume: None,
                                    quote_volume: None,
                                    bid: None,
                                    ask: None,
                                    vwap: None,
                                    base_asset,
                                    provider: "Alpaca".into(),
                                    coverage: "IEX only (not consolidated)".into(),
                                    event_at: DateTime::parse_from_rfc3339(
                                        v["t"].as_str().context("Missing trade time")?,
                                    )?
                                    .with_timezone(&Utc),
                                    received_at: Utc::now(),
                                    status: DataStatus::Live,
                                    delay_seconds: Some(0),
                                };
                                send_feed(tx, FeedEvent::Quote(Box::new(q)));
                                let _ = tx
                                    .try_send(FeedEvent::Status(venue.to_string(), "Live".into()));
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
pub async fn probe(http: &Http) -> Result<String> {
    let info = exchange_info(http).await?;
    let trading = info
        .iter()
        .filter(|i| i.status == TradingStatus::Trading)
        .count();
    let asset = "BTCUSDT".parse().map_err(anyhow::Error::msg)?;
    let bars = history(http, &asset, Timeframe::H1).await?;
    let sub = Subscription {
        assets: vec![asset.clone()],
        chart: asset,
        timeframe: Timeframe::H1,
    };
    let (tx, mut rx) = mpsc::channel(32);
    let task =
        tokio::spawn(async move { stream_once(&Venue::Binance, &sub.assets, &sub, &tx).await });
    let result = tokio::time::timeout(Duration::from_secs(20), async {
        while let Some(event) = rx.recv().await {
            if let FeedEvent::Quote(q) = event {
                return Ok(format!(
                    "Binance verified: {} symbols ({} trading), {} historical candles; {} {} at {} (received {}).",
                    info.len(),
                    trading,
                    bars.len(),
                    q.price,
                    q.currency,
                    q.event_at,
                    q.received_at
                ));
            }
        }
        bail!("Stream closed before a quote")
    })
    .await;
    task.abort();
    result.context("No real quote received within 20 seconds")?
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn alpaca_subscription_is_connected_without_inventing_a_quote() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let auth = ws.next().await.unwrap().unwrap();
            let value: Value = serde_json::from_str(auth.to_text().unwrap()).unwrap();
            assert_eq!(value["action"], "auth");
            ws.send(Message::Text(
                r#"[{"T":"success","msg":"authenticated"}]"#.into(),
            ))
            .await
            .unwrap();
            let subscription = ws.next().await.unwrap().unwrap();
            let value: Value = serde_json::from_str(subscription.to_text().unwrap()).unwrap();
            assert_eq!(value["trades"][0], "AAPL");
            ws.send(Message::Text(
                r#"[{"T":"subscription","trades":["AAPL"],"bars":["AAPL"]}]"#.into(),
            ))
            .await
            .unwrap();
            ws.close(None).await.unwrap();
        });
        let (tx, mut rx) = mpsc::channel(8);
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            stream_socket_authenticated(
                &format!("ws://{address}"),
                &Venue::Iex,
                &["AAPL".parse().unwrap()],
                &tx,
                Some(("fixture-id".into(), "fixture-secret".into())),
            ),
        )
        .await
        .unwrap();
        drop(tx);
        let mut subscribed = false;
        while let Some(event) = rx.recv().await {
            match event {
                FeedEvent::Status(_, status) => {
                    assert_ne!(status, "Live");
                    subscribed |= status.contains("subscribed");
                }
                FeedEvent::Quote(_) => panic!("No trade was sent"),
                _ => {}
            }
        }
        assert!(subscribed);
        server.await.unwrap();
    }
    #[tokio::test]
    async fn websocket_ping_quote_and_close() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            ws.send(Message::Ping(vec![1, 2, 3].into())).await.unwrap();
            assert!(matches!(
                ws.next().await.unwrap().unwrap(),
                Message::Pong(_)
            ));
            ws.send(Message::Text(
                json!({"e":"24hrTicker","s":"BTCUSDT","c":"100","E":Utc::now().timestamp_millis()})
                    .to_string()
                    .into(),
            ))
            .await
            .unwrap();
            ws.close(None).await.unwrap();
        });
        let (tx, mut rx) = mpsc::channel(8);
        let result = tokio::time::timeout(
            Duration::from_secs(3),
            stream_socket(&format!("ws://{address}"), &Venue::Binance, &[], &tx),
        )
        .await
        .unwrap();
        assert!(result.unwrap_err().to_string().contains("disconnected"));
        assert!(matches!(rx.recv().await, Some(FeedEvent::Quote(_))));
        server.await.unwrap();
    }
    #[test]
    fn parses_exchange_info_and_mini_tickers() {
        let ex = json!({
            "symbols": [
                {
                    "symbol": "BTCUSDT",
                    "status": "TRADING",
                    "baseAsset": "BTC",
                    "quoteAsset": "USDT",
                    "isSpotTradingAllowed": true,
                    "filters": [
                        {"filterType": "PRICE_FILTER", "tickSize": "0.01000000"},
                        {"filterType": "LOT_SIZE", "stepSize": "0.00001000"}
                    ]
                },
                {
                    "symbol": "ETHBTC",
                    "status": "TRADING",
                    "baseAsset": "ETH",
                    "quoteAsset": "BTC",
                    "isSpotTradingAllowed": true,
                    "filters": [
                        {"filterType": "PRICE_FILTER", "tickSize": "0.00001000"},
                        {"filterType": "LOT_SIZE", "stepSize": "0.00010000"}
                    ]
                },
                {
                    "symbol": "DEADUSDT",
                    "status": "BREAK",
                    "baseAsset": "DEAD",
                    "quoteAsset": "USDT",
                    "isSpotTradingAllowed": true,
                    "filters": []
                }
            ]
        });
        let info = parse_exchange_info(&ex).unwrap();
        assert_eq!(info.len(), 3);
        assert_eq!(info[0].price_precision, 2);
        assert_eq!(info[0].quantity_precision, 5);
        assert_eq!(info[1].base_asset, "ETH");
        assert_eq!(info[1].quote_asset, "BTC");
        assert_eq!(info[2].status, TradingStatus::Break);

        let mut map = BTreeMap::new();
        for i in info {
            map.insert(i.asset.key(), i);
        }
        let now_ms = Utc::now().timestamp_millis();
        let snap = json!([
            {
                "symbol": "BTCUSDT",
                "lastPrice": "68000.50",
                "openPrice": "66000.50",
                "priceChange": "2000.00",
                "priceChangePercent": "3.03",
                "highPrice": "69000.00",
                "lowPrice": "65500.00",
                "volume": "12000.5",
                "quoteVolume": "816000000.0",
                "bidPrice": "68000.40",
                "askPrice": "68000.60",
                "weightedAvgPrice": "67500.00",
                "closeTime": now_ms - 1000
            },
            {
                "symbol": "DEADUSDT",
                "lastPrice": "1.00",
                "closeTime": now_ms - 1000
            }
        ]);
        let quotes = parse_tickers_24hr(&snap, &map).unwrap();
        assert_eq!(quotes.len(), 1);
        assert_eq!(quotes[0].asset.symbol, "BTCUSDT");
        assert_eq!(quotes[0].quote_volume, Some(Decimal::from(816000000)));

        let arr = json!([
            {
                "e": "24hrMiniTicker",
                "E": now_ms,
                "s": "ETHBTC",
                "c": "0.03300",
                "o": "0.03000",
                "h": "0.03400",
                "l": "0.02950",
                "v": "5000.0",
                "q": "165.0"
            }
        ]);
        let mini = binance_mini_ticker_arr(&arr).unwrap();
        assert_eq!(mini.len(), 1);
        assert_eq!(mini[0].asset.symbol, "ETHBTC");
        assert_eq!(mini[0].currency, "BTC");
        assert_eq!(mini[0].change_pct, Some(Decimal::from(10)));
        assert_eq!(
            mini[0].change_abs,
            Some(Decimal::from_str("0.00300").unwrap())
        );
    }
    #[test]
    fn parses_partial_kline() {
        let v: Value =
            serde_json::from_str(include_str!("../../../tests/fixtures/binance-kline.json"))
                .unwrap();
        let c = binance_kline(&v).unwrap();
        assert!(!c.closed);
        assert_eq!(c.close, Decimal::from(101));
    }
    #[test]
    fn malformed_prices_fail() {
        assert!(binance_ticker(&json!({"s":"BTCUSDT","c":"NaN","E":1})).is_err());
        assert!(binance_bars(&json!([[1, "2", "1", "3", "2", "-1", 2]])).is_err());
    }
    #[tokio::test]
    async fn mini_ticker_array_stream_and_snapshot_reconciliation() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let now_ms = Utc::now().timestamp_millis();
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            ws.send(Message::Text(
                json!([
                    {
                        "e": "24hrMiniTicker",
                        "E": now_ms,
                        "s": "DOGEUSDT",
                        "c": "0.16500",
                        "o": "0.15000",
                        "h": "0.17000",
                        "l": "0.14800",
                        "v": "500000000",
                        "q": "82500000"
                    },
                    {
                        "e": "24hrMiniTicker",
                        "E": now_ms,
                        "s": "SOLUSDC",
                        "c": "188.50",
                        "o": "180.00",
                        "h": "190.00",
                        "l": "178.00",
                        "v": "120000",
                        "q": "22620000"
                    }
                ])
                .to_string()
                .into(),
            ))
            .await
            .unwrap();
            ws.close(None).await.unwrap();
        });
        let (tx, mut rx) = mpsc::channel(8);
        let _ = tokio::time::timeout(
            Duration::from_secs(3),
            stream_socket(&format!("ws://{address}"), &Venue::Binance, &[], &tx),
        )
        .await;
        let event = rx.recv().await.unwrap();
        let mut state = MarketState::default();
        match event {
            FeedEvent::MiniTickers(batch, norm_us) => {
                assert_eq!(batch.len(), 2);
                state.last_normalize_micros = norm_us;
                for q in batch {
                    assert!(state.ingest(q));
                }
            }
            _ => panic!("Expected MiniTickers batch"),
        }
        // Verify non-watchlist symbols are stored and retained
        assert!(state.quotes.contains_key("BINANCE:DOGEUSDT"));
        assert!(state.quotes.contains_key("BINANCE:SOLUSDC"));
        assert_eq!(
            state.quotes["BINANCE:DOGEUSDT"].change_pct,
            Some(Decimal::from(10))
        );

        // Simulate bookTicker bid/ask enrichment followed by snapshot reconciliation
        assert!(state.apply_book_ticker(
            "BINANCE:DOGEUSDT",
            Decimal::from_str("0.16490").unwrap(),
            Decimal::from_str("0.16510").unwrap(),
            Utc::now()
        ));
        let mut reconciled = state.quotes["BINANCE:DOGEUSDT"].clone();
        reconciled.price = Decimal::from_str("0.16505").unwrap();
        reconciled.bid = None;
        reconciled.ask = None;
        reconciled.event_at = Utc::now() + chrono::Duration::milliseconds(500);
        reconciled.received_at = Utc::now() + chrono::Duration::milliseconds(500);
        assert!(state.ingest(reconciled));
        assert_eq!(
            state.quotes["BINANCE:DOGEUSDT"].bid,
            Some(Decimal::from_str("0.16490").unwrap())
        );
        assert_eq!(
            state.quotes["BINANCE:DOGEUSDT"].price,
            Decimal::from_str("0.16505").unwrap()
        );
        server.await.unwrap();
    }
}
