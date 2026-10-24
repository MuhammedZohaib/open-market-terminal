#![forbid(unsafe_code)]
use crate::types::*;
use anyhow::Result;
use chrono::Utc;
use omt_core::{
    AssetId, Candle, Company, InstrumentInfo, MacroObservation, NewsCategory, NewsItem, PeriodView,
    Quote, Timeframe,
    command::{self, Command, Workspace},
    facts::{self, StatementTab},
    portfolio::{Transaction, positions},
};
use omt_data::{MarketState, Subscription};
use omt_providers::{
    Http, market,
    research::{self, Sec},
};
use omt_storage::Store;
use rust_decimal::Decimal;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};
use tokio::sync::{broadcast, mpsc, watch};

#[derive(Clone, Debug)]
pub enum ResearchRequest {
    Impact(AssetId, AssetId),
    Company(String),
    Treasury,
    WorldBank(String, String),
    Fred(String),
    News(NewsCategory, Option<String>),
    Article(NewsItem),
    OlderHistory(AssetId, Timeframe, i64),
}

pub struct MarketEngine {
    pub store: Arc<Mutex<Store>>,
    pub http: Http,
    pub subscription_tx: watch::Sender<Subscription>,
    pub market_rx: watch::Receiver<MarketState>,
    pub research_tx: mpsc::Sender<ResearchRequest>,
    pub quote_broadcast_tx: broadcast::Sender<Vec<MarketQuoteDto>>,
    pub active_asset: Arc<Mutex<AssetId>>,
    pub active_timeframe: Arc<Mutex<Timeframe>>,
    pub active_account: Arc<Mutex<String>>,
    pub news_cache: Arc<Mutex<Vec<NewsItem>>>,
    pub offline: bool,
    pub _runtime: Option<Arc<tokio::runtime::Runtime>>,
}

impl MarketEngine {
    pub fn new(
        data_dir: PathBuf,
        offline: bool,
        sec_user_agent: Option<String>,
    ) -> Result<Arc<Self>> {
        let (runtime_opt, handle) = match tokio::runtime::Handle::try_current() {
            Ok(h) => (None, h),
            Err(_) => {
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .enable_all()
                    .build()?;
                let h = rt.handle().clone();
                (Some(Arc::new(rt)), h)
            }
        };
        let _guard = runtime_opt.as_ref().map(|rt| rt.enter());

        std::fs::create_dir_all(&data_dir)?;
        let store = Arc::new(Mutex::new(Store::open(&data_dir.join("omt.sqlite3"))?));
        let http = Http::new()?;

        // Default watchlist seeds if empty
        {
            let s = store.lock().unwrap();
            if s.watchlists()?.is_empty() {
                for sym in ["BTCUSDT", "ETHUSDT", "SOLUSDT"] {
                    if let Ok(asset) = sym.parse::<AssetId>() {
                        let _ = s.watch("default", &asset, true);
                    }
                }
            }
        }

        let default_asset = {
            let s = store.lock().unwrap();
            s.setting("asset")?
                .and_then(|a| a.parse::<AssetId>().ok())
                .unwrap_or_else(|| "BINANCE:BTCUSDT".parse().unwrap())
        };

        let active_asset = Arc::new(Mutex::new(default_asset.clone()));
        let active_timeframe = Arc::new(Mutex::new(Timeframe::H1));
        let active_account = Arc::new(Mutex::new("default".into()));
        let news_cache = Arc::new(Mutex::new(Vec::new()));

        // Restore cached news
        {
            let s = store.lock().unwrap();
            if let Ok(Some((cached_news, _))) = s.get::<Vec<NewsItem>>("news", "recent") {
                let mut n = news_cache.lock().unwrap();
                *n = cached_news;
            }
        }

        let initial_sub = Subscription {
            assets: vec![default_asset.clone()],
            chart: default_asset.clone(),
            timeframe: Timeframe::H1,
        }
        .normalized();

        let (subscription_tx, subscription_rx) = watch::channel(initial_sub);
        let mut initial_market = MarketState::default();

        // Restore universe and cached quotes
        {
            let s = store.lock().unwrap();
            if let Ok(Some((insts, _))) =
                s.get::<Vec<InstrumentInfo>>("universe", "binance_instruments")
            {
                initial_market.register_instruments(insts);
            }
            if let Ok(Some((quotes, _))) = s.get::<Vec<Quote>>("universe", "binance_top_quotes") {
                for mut q in quotes {
                    q.status = omt_core::DataStatus::Historical;
                    initial_market.quotes.insert(q.asset.key(), q);
                }
            }
        }

        let (market_tx, market_rx) = watch::channel(initial_market);
        let (research_tx, mut research_rx) = mpsc::channel::<ResearchRequest>(64);
        let (quote_broadcast_tx, _) = broadcast::channel::<Vec<MarketQuoteDto>>(256);

        let engine = Arc::new(Self {
            store: store.clone(),
            http: http.clone(),
            subscription_tx,
            market_rx,
            research_tx: research_tx.clone(),
            quote_broadcast_tx: quote_broadcast_tx.clone(),
            active_asset,
            active_timeframe,
            active_account,
            news_cache: news_cache.clone(),
            offline,
            _runtime: runtime_opt,
        });

        // Spawn background market ingestion service if online
        if !offline {
            handle.spawn(market::service(
                http.clone(),
                subscription_rx,
                market_tx.clone(),
            ));

            let ua = std::env::var("OMT_SEC_USER_AGENT").ok().or(sec_user_agent);
            let sec = ua.as_deref().and_then(|s| Sec::new(http.clone(), s).ok());
            let h = http.clone();
            let store_clone = store.clone();
            let news_clone = news_cache.clone();

            // Background researcher task
            handle.spawn(async move {
                let mut mapping = None;
                while let Some(req) = research_rx.recv().await {
                    match req {
                        ResearchRequest::Company(ticker) => {
                            if let Some(ref sec_client) = sec {
                                if mapping.is_none() {
                                    mapping = sec_client.tickers().await.ok();
                                }
                                if let Some(ref map) = mapping
                                    && let Ok(comp) = sec_client.company(&ticker, map).await
                                {
                                    let s = store_clone.lock().unwrap();
                                    let _ = s.put("company", &ticker, &comp);
                                    let filings_news = omt_core::company_filings_to_news(&comp);
                                    let mut nc = news_clone.lock().unwrap();
                                    nc.extend(filings_news);
                                    *nc = research::dedup_news(std::mem::take(&mut *nc));
                                    let _ = s.put("news", "recent", &*nc);
                                }
                            }
                        }
                        ResearchRequest::Treasury => {
                            if let Ok(rows) = research::treasury(&h).await {
                                let s = store_clone.lock().unwrap();
                                let _ = s.put("macro", "TREASURY", &rows);
                            }
                        }
                        ResearchRequest::WorldBank(country, series) => {
                            if let Ok(rows) = research::world_bank(&h, &country, &series).await {
                                let key = format!("WB {country} {series}");
                                let s = store_clone.lock().unwrap();
                                let _ = s.put("macro", &key, &rows);
                            }
                        }
                        ResearchRequest::Fred(series) => {
                            if let Ok(rows) = research::fred(&h, &series).await {
                                let key = format!("FRED {series}");
                                let s = store_clone.lock().unwrap();
                                let _ = s.put("macro", &key, &rows);
                            }
                        }
                        ResearchRequest::News(cat, ticker) => {
                            if let Ok(items) =
                                research::fetch_news(&h, ticker.as_deref(), cat).await
                            {
                                let mut nc = news_clone.lock().unwrap();
                                nc.extend(items);
                                *nc = research::dedup_news(std::mem::take(&mut *nc));
                                let s = store_clone.lock().unwrap();
                                let _ = s.put("news", "recent", &*nc);
                            }
                        }
                        ResearchRequest::Article(item) => {
                            let id = item.id.clone();
                            if let Ok(body) = research::fetch_article_or_filing_body(
                                &h,
                                ua.as_deref(),
                                &item.url,
                                item.is_sec_filing,
                                item.form.as_deref(),
                                Some(&item.headline),
                                &item.summary,
                            )
                            .await
                            {
                                let s = store_clone.lock().unwrap();
                                let _ = s.put("articles", &id, &body);
                                let mut nc = news_clone.lock().unwrap();
                                if let Some(n) = nc.iter_mut().find(|n| n.id == id) {
                                    n.body = Some(body);
                                }
                            }
                        }
                        ResearchRequest::OlderHistory(asset, tf, before) => {
                            if let Ok(older) =
                                market::history_before(&h, &asset, tf, Some(before)).await
                            {
                                let s = store_clone.lock().unwrap();
                                let key = format!("{}:{}", asset.key(), tf);
                                if let Ok(Some((mut current, at))) =
                                    s.get::<Vec<Candle>>("candles", &key)
                                {
                                    current.extend(older);
                                    let mut seen = std::collections::HashSet::new();
                                    current.retain(|c| seen.insert(c.open_at));
                                    current.sort_by_key(|c| c.open_at);
                                    let _ = s.put_at("candles", &key, &current, at);
                                }
                            }
                        }
                        ResearchRequest::Impact(asset, bench) => {
                            let (p1, p2, n) = tokio::join!(
                                market::history(&h, &asset, Timeframe::D1),
                                market::history(&h, &bench, Timeframe::D1),
                                research::fetch_news(
                                    &h,
                                    Some(asset.symbol.as_str()),
                                    NewsCategory::All
                                )
                            );
                            if let (Ok(p1), Ok(p2)) = (p1, p2) {
                                let impact = omt_core::impact::ImpactData {
                                    prices: p1,
                                    benchmark_prices: p2,
                                    asset: asset.clone(),
                                    benchmark: bench.clone(),
                                    news: n.unwrap_or_default(),
                                    fetched_at: Utc::now(),
                                    warnings: Vec::new(),
                                };
                                let s = store_clone.lock().unwrap();
                                let _ = s.put(
                                    "impact",
                                    &format!("{}|{}", asset.key(), bench.key()),
                                    &impact,
                                );
                            }
                        }
                    }
                }
            });

            // Initial catalog and news requests
            let r_tx = research_tx.clone();
            handle.spawn(async move {
                let _ = r_tx
                    .send(ResearchRequest::News(NewsCategory::All, None))
                    .await;
                let _ = r_tx.send(ResearchRequest::Treasury).await;
            });
        }

        // Periodic batch broadcaster to Tauri webview channels (30 FPS coalesced)
        let mut market_rx_broadcaster = market_tx.subscribe();
        let broadcast_tx = quote_broadcast_tx.clone();
        handle.spawn(async move {
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(33));
            loop {
                interval.tick().await;
                if market_rx_broadcaster.has_changed().unwrap_or(false) {
                    let state = market_rx_broadcaster.borrow_and_update().clone();
                    let quotes: Vec<MarketQuoteDto> = state
                        .quotes
                        .values()
                        .take(500)
                        .map(MarketQuoteDto::from_quote)
                        .collect();
                    if !quotes.is_empty() {
                        let _ = broadcast_tx.send(quotes);
                    }
                }
            }
        });

        Ok(engine)
    }

    pub fn set_active_asset_and_timeframe(&self, asset: AssetId, tf: Timeframe) {
        {
            let mut a = self.active_asset.lock().unwrap();
            *a = asset.clone();
            let mut t = self.active_timeframe.lock().unwrap();
            *t = tf;
        }
        self.update_subscription();
    }

    pub fn update_subscription(&self) {
        let asset = self.active_asset.lock().unwrap().clone();
        let tf = *self.active_timeframe.lock().unwrap();
        let s = self.store.lock().unwrap();
        let mut assets = vec![asset.clone()];

        if let Ok(watch) = s.watchlist("default") {
            assets.extend(watch);
        }

        let sub = Subscription {
            assets,
            chart: asset,
            timeframe: tf,
        }
        .normalized();

        let _ = self.subscription_tx.send(sub);
    }

    pub fn market_snapshot(&self) -> MarketSnapshotDto {
        let state = self.market_rx.borrow().clone();
        let all_quotes: Vec<MarketQuoteDto> = state
            .quotes
            .values()
            .map(MarketQuoteDto::from_quote)
            .collect();

        // Sort by quote volume descending for most active
        let mut most_active = all_quotes.clone();
        most_active.sort_by(|a, b| {
            let av = a
                .quote_volume_24h
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            let bv = b
                .quote_volume_24h
                .as_deref()
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(0.0);
            bv.partial_cmp(&av).unwrap_or(std::cmp::Ordering::Equal)
        });
        most_active.truncate(20);

        // Gainers (sorted by change_percent descending)
        let mut top_gainers = all_quotes.clone();
        top_gainers.sort_by(|a, b| {
            let ac = a.change_percent_24h.unwrap_or(-999.0);
            let bc = b.change_percent_24h.unwrap_or(-999.0);
            bc.partial_cmp(&ac).unwrap_or(std::cmp::Ordering::Equal)
        });
        top_gainers.truncate(20);

        // Losers (sorted by change_percent ascending)
        let mut top_losers = all_quotes.clone();
        top_losers.sort_by(|a, b| {
            let ac = a.change_percent_24h.unwrap_or(999.0);
            let bc = b.change_percent_24h.unwrap_or(999.0);
            ac.partial_cmp(&bc).unwrap_or(std::cmp::Ordering::Equal)
        });
        top_losers.truncate(20);

        MarketSnapshotDto {
            quotes: all_quotes,
            total_instruments: state.instruments.len(),
            active_instruments: state.trading_instrument_count(),
            top_gainers,
            top_losers,
            most_active,
            connection_status: state.status.clone(),
            last_snapshot_at: state.last_snapshot_at.map(|t| t.to_rfc3339()),
        }
    }

    pub fn search_instruments(&self, query: &str) -> Vec<InstrumentSearchItemDto> {
        let q = query.trim().to_ascii_uppercase();
        let state = self.market_rx.borrow().clone();
        let mut results = Vec::new();

        for info in state.instruments.values() {
            if q.is_empty()
                || info.asset.symbol.contains(&q)
                || info.base_asset.contains(&q)
                || info.quote_asset.contains(&q)
            {
                results.push(InstrumentSearchItemDto {
                    key: info.asset.key(),
                    symbol: info.asset.symbol.clone(),
                    venue: info.asset.venue.to_string(),
                    asset_class: format!("{:?}", info.asset.asset_class),
                    quote_currency: info.quote_asset.clone(),
                    base_asset: info.base_asset.clone(),
                    status: format!("{:?}", info.status),
                });
                if results.len() >= 60 {
                    break;
                }
            }
        }
        results
    }

    pub fn historical_candles(
        &self,
        asset: &AssetId,
        timeframe: Timeframe,
    ) -> Result<ChartDataDto> {
        let key = format!("{}:{}", asset.key(), timeframe);
        let s = self.store.lock().unwrap();

        let (candles, at, status) =
            if let Some((bars, at)) = s.get::<Vec<Candle>>("candles", &key)? {
                let is_stale =
                    Utc::now().signed_duration_since(at).num_seconds() > timeframe.seconds() * 2;
                let st = if is_stale { "Stale" } else { "Ready" };
                (bars, Some(at), st.to_string())
            } else {
                (Vec::new(), None, "Loading".to_string())
            };

        // Compute technical indicators using omt_core
        let mut indicators = Vec::with_capacity(candles.len());

        let sma20 = omt_core::sma(&candles, 20);
        let sma50 = omt_core::sma(&candles, 50);
        let ema12 = omt_core::ema(&candles, 12);
        let ema26 = omt_core::ema(&candles, 26);
        let rsi14 = omt_core::rsi(&candles, 14);
        let macd_list = omt_core::macd(&candles, 12, 26, 9);
        let atr14 = omt_core::atr(&candles, 14);
        let bb_list = omt_core::bollinger_bands(&candles, 20, 2);

        for i in 0..candles.len() {
            let (m, ms, mh) = macd_list
                .get(i)
                .copied()
                .flatten()
                .map(|(a, b, c)| (Some(a), Some(b), Some(c)))
                .unwrap_or((None, None, None));
            let (b_low, b_mid, b_up) = bb_list
                .get(i)
                .copied()
                .flatten()
                .map(|(l, m, u)| (Some(l), Some(m), Some(u)))
                .unwrap_or((None, None, None));

            indicators.push(IndicatorsDto {
                sma20: sma20
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|d| d.to_string().parse().ok()),
                sma50: sma50
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|d| d.to_string().parse().ok()),
                ema12: ema12
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|d| d.to_string().parse().ok()),
                ema26: ema26
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|d| d.to_string().parse().ok()),
                rsi14: rsi14
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|d| d.to_string().parse().ok()),
                macd: m.and_then(|d| d.to_string().parse().ok()),
                macd_signal: ms.and_then(|d| d.to_string().parse().ok()),
                macd_hist: mh.and_then(|d| d.to_string().parse().ok()),
                atr14: atr14
                    .get(i)
                    .copied()
                    .flatten()
                    .and_then(|d| d.to_string().parse().ok()),
                bb_upper: b_up.and_then(|d| d.to_string().parse().ok()),
                bb_middle: b_mid.and_then(|d| d.to_string().parse().ok()),
                bb_lower: b_low.and_then(|d| d.to_string().parse().ok()),
            });
        }

        let candle_dtos = candles.iter().map(CandleDto::from_candle).collect();

        Ok(ChartDataDto {
            asset_key: asset.key(),
            symbol: asset.symbol.clone(),
            venue: asset.venue.to_string(),
            timeframe: timeframe.to_string(),
            candles: candle_dtos,
            indicators,
            status,
            error_message: None,
            received_at: at.map(|t| t.to_rfc3339()),
        })
    }

    pub fn company_facts(&self, ticker: &str) -> Result<Option<CompanyFactsDto>> {
        let s = self.store.lock().unwrap();
        let comp: Option<Company> = s.get::<Company>("company", ticker)?.map(|(c, _)| c);

        let Some(comp) = comp else {
            return Ok(None);
        };

        let build_matrix = |tab: StatementTab, view: PeriodView| -> StatementMatrixDto {
            let table = facts::comparative_table(&comp.facts, view, tab);
            let periods = table.periods.iter().map(|p| p.to_string()).collect();
            let rows = table
                .rows
                .into_iter()
                .map(|r| StatementRowDto {
                    concept: r.metric.to_string(),
                    label: r.metric.to_string(),
                    values: r
                        .values
                        .into_iter()
                        .map(|v| v.map(|d| d.to_string()))
                        .collect(),
                    is_header: false,
                })
                .collect();
            StatementMatrixDto { periods, rows }
        };

        let recent_filings = comp
            .filings
            .iter()
            .take(50)
            .map(|f| SecFilingDto {
                form: f.form.clone(),
                filed: f.filed.to_string(),
                report_date: f.report_date.map(|d| d.to_string()),
                accession: f.accession.clone(),
                description: f.description.clone(),
                document: f.document.clone(),
                url: f.url.clone(),
            })
            .collect();

        Ok(Some(CompanyFactsDto {
            ticker: comp.ticker.clone(),
            cik: comp.cik.to_string(),
            name: comp.name.clone(),
            facts_count: comp.facts.len(),
            income_statement_annual: build_matrix(StatementTab::Income, PeriodView::Annual),
            income_statement_quarterly: build_matrix(StatementTab::Income, PeriodView::Quarterly),
            balance_sheet_annual: build_matrix(StatementTab::BalanceSheet, PeriodView::Annual),
            balance_sheet_quarterly: build_matrix(
                StatementTab::BalanceSheet,
                PeriodView::Quarterly,
            ),
            cash_flow_annual: build_matrix(StatementTab::CashFlow, PeriodView::Annual),
            cash_flow_quarterly: build_matrix(StatementTab::CashFlow, PeriodView::Quarterly),
            ratios_annual: build_matrix(StatementTab::Ratios, PeriodView::Annual),
            ratios_quarterly: build_matrix(StatementTab::Ratios, PeriodView::Quarterly),
            recent_filings,
        }))
    }

    pub fn news(&self, category: Option<NewsCategory>, ticker: Option<&str>) -> Vec<NewsItemDto> {
        let items = self.news_cache.lock().unwrap().clone();
        items
            .into_iter()
            .filter(|n| {
                if let Some(cat) = category
                    && cat != NewsCategory::All
                    && n.category != cat
                {
                    return false;
                }
                if let Some(t) = ticker
                    && !n
                        .ticker
                        .as_deref()
                        .is_some_and(|tk| tk.eq_ignore_ascii_case(t))
                {
                    return false;
                }
                true
            })
            .map(|n| NewsItemDto::from_core(&n))
            .collect()
    }

    pub fn article(&self, id: &str) -> Result<Option<String>> {
        let s = self.store.lock().unwrap();
        Ok(s.get::<String>("articles", id)?.map(|(b, _)| b))
    }

    pub fn treasury_curve(&self) -> Result<Option<TreasuryCurveDto>> {
        let s = self.store.lock().unwrap();
        let rows: Option<Vec<MacroObservation>> = s
            .get::<Vec<MacroObservation>>("macro", "TREASURY")?
            .map(|(r, _)| r);

        let Some(rows) = rows else {
            return Ok(None);
        };

        if rows.is_empty() {
            return Ok(None);
        }

        let obs_date = rows[0].date.clone();
        let mut tenors = Vec::new();
        let mut y2: Option<f64> = None;
        let mut y10: Option<f64> = None;
        let mut y3m: Option<f64> = None;

        for r in &rows {
            if let Ok(val) = r.value.to_string().parse::<f64>() {
                tenors.push(TreasuryTenorDto {
                    tenor: r.series.clone(),
                    yield_value: val,
                });
                if r.series.contains("2Y") || r.series.contains("2 Year") {
                    y2 = Some(val);
                } else if r.series.contains("10Y") || r.series.contains("10 Year") {
                    y10 = Some(val);
                } else if r.series.contains("3M") || r.series.contains("3 Month") {
                    y3m = Some(val);
                }
            }
        }

        let spread_2y_10y = y10.zip(y2).map(|(ten, two)| ten - two);
        let spread_3m_10y = y10.zip(y3m).map(|(ten, three)| ten - three);

        Ok(Some(TreasuryCurveDto {
            observation_date: obs_date,
            tenors,
            spread_2y_10y,
            spread_3m_10y,
            source: "US Department of the Treasury (Daily Par Yield Curve)".into(),
        }))
    }

    pub fn portfolio(&self, account: &str) -> Result<PortfolioSummaryDto> {
        let s = self.store.lock().unwrap();
        let all_tx = s.transactions()?;
        let acct_tx: Vec<Transaction> = all_tx
            .into_iter()
            .filter(|t| t.account == account)
            .collect();

        let pos_list = positions(&acct_tx)?;
        let market_state = self.market_rx.borrow().clone();

        let mut total_cost = Decimal::ZERO;
        let mut total_market = Decimal::ZERO;
        let mut holdings = Vec::new();

        for pos in pos_list {
            if pos.quantity <= Decimal::ZERO {
                continue;
            }
            let key = pos.asset.key();
            let cur_quote = market_state.quotes.get(&key);
            let cur_price = cur_quote.map(|q| q.price);
            let cur_val = cur_price.and_then(|p| pos.market_value(p, &pos.currency));
            let pnl_val = cur_price.and_then(|p| pos.unrealized(p, &pos.currency));

            let pnl_pct = if let (Some(p), true) = (pnl_val, pos.cost > Decimal::ZERO) {
                ((p / pos.cost) * Decimal::from(100))
                    .to_string()
                    .parse::<f64>()
                    .ok()
            } else {
                None
            };

            total_cost += pos.cost;
            if let Some(cv) = cur_val {
                total_market += cv;
            }

            holdings.push(HoldingDto {
                asset_key: pos.asset.key(),
                symbol: pos.asset.symbol.clone(),
                venue: pos.asset.venue.to_string(),
                quantity: pos.quantity.to_string(),
                average_cost: pos
                    .average_cost()
                    .map(|c| c.to_string())
                    .unwrap_or_else(|| "0".into()),
                cost_basis: pos.cost.to_string(),
                current_price: cur_price.map(|p| p.to_string()),
                market_value: cur_val.map(|v| v.to_string()),
                unrealized_pnl: pnl_val.map(|p| p.to_string()),
                unrealized_pnl_percent: pnl_pct,
            });
        }

        let total_pnl = total_market - total_cost;
        let total_pnl_pct = if total_cost > Decimal::ZERO {
            ((total_pnl / total_cost) * Decimal::from(100))
                .to_string()
                .parse::<f64>()
                .ok()
        } else {
            None
        };

        let tx_dtos = acct_tx
            .iter()
            .map(TransactionDto::from_transaction)
            .collect();

        Ok(PortfolioSummaryDto {
            account: account.to_string(),
            total_cost_basis: total_cost.to_string(),
            total_market_value: total_market.to_string(),
            total_unrealized_pnl: total_pnl.to_string(),
            total_unrealized_pnl_percent: total_pnl_pct,
            holdings,
            transactions: tx_dtos,
        })
    }

    pub fn add_transaction(&self, tx: Transaction) -> Result<()> {
        let mut s = self.store.lock().unwrap();
        s.add_transactions(&[tx])?;
        Ok(())
    }

    pub fn delete_transaction(&self, id: &str) -> Result<()> {
        let mut s = self.store.lock().unwrap();
        s.delete_transaction(id)?;
        Ok(())
    }

    pub fn execute_command(&self, text: &str) -> Result<CommandOutcomeDto> {
        let parsed = command::parse(text).map_err(anyhow::Error::msg)?;

        match parsed {
            Command::Open(workspace, asset_opt, tf_opt) => {
                let mut outcome = CommandOutcomeDto {
                    success: true,
                    message: format!("Opened workspace {:?}", workspace),
                    workspace_target: Some(format!("{:?}", workspace).to_ascii_uppercase()),
                    asset_target: None,
                    timeframe_target: tf_opt.map(|t| t.to_string()),
                    statement_tab: None,
                    quote_filter: None,
                    alert_message: None,
                };

                if let Some(asset) = asset_opt {
                    outcome.asset_target = Some(asset.key());
                    if let Some(tf) = tf_opt {
                        self.set_active_asset_and_timeframe(asset.clone(), tf);
                    } else {
                        let tf = *self.active_timeframe.lock().unwrap();
                        self.set_active_asset_and_timeframe(asset.clone(), tf);
                    }
                    if workspace == Workspace::Fin && !self.offline {
                        let _ = self
                            .research_tx
                            .try_send(ResearchRequest::Company(asset.symbol.clone()));
                    }
                }

                Ok(outcome)
            }
            Command::Help => Ok(CommandOutcomeDto {
                success: true,
                message: "Showing help dialog".into(),
                workspace_target: None,
                asset_target: None,
                timeframe_target: None,
                statement_tab: None,
                quote_filter: None,
                alert_message: None,
            }),
            Command::Settings => Ok(CommandOutcomeDto {
                success: true,
                message: "Showing settings dialog".into(),
                workspace_target: Some("SETTINGS".into()),
                asset_target: None,
                timeframe_target: None,
                statement_tab: None,
                quote_filter: None,
                alert_message: None,
            }),
            Command::Watch(add, asset) => {
                let s = self.store.lock().unwrap();
                s.watch("default", &asset, add)?;
                Ok(CommandOutcomeDto {
                    success: true,
                    message: format!(
                        "{} {} in default watchlist",
                        if add { "Added" } else { "Removed" },
                        asset.key()
                    ),
                    workspace_target: None,
                    asset_target: Some(asset.key()),
                    timeframe_target: None,
                    statement_tab: None,
                    quote_filter: None,
                    alert_message: None,
                })
            }
            Command::Watchlist(name) => {
                let s = self.store.lock().unwrap();
                s.ensure_watchlist(&name)?;
                Ok(CommandOutcomeDto {
                    success: true,
                    message: format!("Active watchlist switched to '{}'", name),
                    workspace_target: None,
                    asset_target: None,
                    timeframe_target: None,
                    statement_tab: None,
                    quote_filter: None,
                    alert_message: None,
                })
            }
            Command::Raw(parts) => {
                let head = parts[0].to_ascii_uppercase();
                match head.as_str() {
                    "MARKET" | "MKT" => {
                        let quote = parts.get(1).map(|s| s.to_ascii_uppercase());
                        Ok(CommandOutcomeDto {
                            success: true,
                            message: "Switched to Market Monitor".into(),
                            workspace_target: Some("MARKET".into()),
                            asset_target: None,
                            timeframe_target: None,
                            statement_tab: None,
                            quote_filter: quote,
                            alert_message: None,
                        })
                    }
                    "SCAN" | "SRCH" => Ok(CommandOutcomeDto {
                        success: true,
                        message: "Switched to Screener".into(),
                        workspace_target: Some("SCAN".into()),
                        asset_target: None,
                        timeframe_target: None,
                        statement_tab: None,
                        quote_filter: None,
                        alert_message: None,
                    }),
                    "NEWS" | "TOP" | "CN" => Ok(CommandOutcomeDto {
                        success: true,
                        message: "Switched to Live News Workstation".into(),
                        workspace_target: Some("NEWS".into()),
                        asset_target: parts.get(1).cloned(),
                        timeframe_target: None,
                        statement_tab: None,
                        quote_filter: None,
                        alert_message: None,
                    }),
                    "ECON" | "YCRV" => Ok(CommandOutcomeDto {
                        success: true,
                        message: "Switched to US Treasury & Macro Economics".into(),
                        workspace_target: Some("ECON".into()),
                        asset_target: None,
                        timeframe_target: None,
                        statement_tab: None,
                        quote_filter: None,
                        alert_message: None,
                    }),
                    "PORT" | "PRTU" => Ok(CommandOutcomeDto {
                        success: true,
                        message: "Switched to Portfolio Accounting".into(),
                        workspace_target: Some("PORT".into()),
                        asset_target: None,
                        timeframe_target: None,
                        statement_tab: None,
                        quote_filter: None,
                        alert_message: None,
                    }),
                    "LAUNCHPAD" => Ok(CommandOutcomeDto {
                        success: true,
                        message: "Switched to Multi-Pane Launchpad Workstation".into(),
                        workspace_target: Some("LAUNCHPAD".into()),
                        asset_target: None,
                        timeframe_target: None,
                        statement_tab: None,
                        quote_filter: None,
                        alert_message: None,
                    }),
                    _ => Ok(CommandOutcomeDto {
                        success: true,
                        message: format!("Executed raw command: {}", text),
                        workspace_target: None,
                        asset_target: None,
                        timeframe_target: None,
                        statement_tab: None,
                        quote_filter: None,
                        alert_message: None,
                    }),
                }
            }
            Command::Worksheet(action) => Ok(CommandOutcomeDto {
                success: true,
                message: format!("Worksheet action: {:?}", action),
                workspace_target: Some("MARKET".into()),
                asset_target: None,
                timeframe_target: None,
                statement_tab: None,
                quote_filter: None,
                alert_message: None,
            }),
        }
    }
}
