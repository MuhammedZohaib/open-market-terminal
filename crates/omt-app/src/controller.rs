use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use omt_core::{
    AssetClass, AssetId, Candle, Company, MacroObservation, NewsCategory, Timeframe, Venue,
    command::{self, Command, Workspace},
    facts::StatementTab,
    portfolio::{Side, Transaction},
};
use omt_data::Subscription;
use omt_storage::Store;
use omt_tui::{App, LayoutMode, MarketMode, OverlayMode, ScanMode, views};
use std::path::Path;

#[derive(Clone, Debug)]
pub enum ResearchRequest {
    Impact(AssetId, AssetId),
    Company(String),
    Treasury,
    WorldBank(String, String),
    Fred(String),
    News(NewsCategory, Option<String>),
    Article(omt_core::NewsItem),
    OlderHistory(AssetId, Timeframe, i64),
}
pub struct Controller {
    pub app: App,
    pub store: Store,
    pub offline: bool,
    pub macro_key: String,
}
pub fn subscription(app: &App) -> Subscription {
    let mut assets: Vec<_> = app
        .alerts
        .iter()
        .filter(|a| a.triggered_at.is_none())
        .map(|a| a.asset.clone())
        .collect();
    assets.extend(app.watchlist.clone());
    if let Ok(positions) = omt_core::portfolio::positions(&app.transactions) {
        assets.extend(
            positions
                .into_iter()
                .filter(|p| p.quantity > rust_decimal::Decimal::ZERO)
                .map(|p| p.asset),
        );
    }
    if let Some(ref key) = app.selected_market_key
        && let Ok(focused) = key.parse::<AssetId>()
    {
        assets.push(focused);
    }
    Subscription {
        assets,
        chart: app.asset.clone(),
        timeframe: app.timeframe,
    }
    .normalized()
}

fn open_terminal_url_safely(url: &str) -> Result<()> {
    if (!url.starts_with("https://") && !url.starts_with("http://"))
        || !url
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":/.-_?=&%#~+".contains(c))
    {
        bail!("Refusing to open untrusted or malformed URL");
    }
    #[cfg(target_os = "windows")]
    {
        let _ = std::process::Command::new("rundll32.exe")
            .args(["url.dll,FileProtocolHandler", url])
            .spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = std::process::Command::new("open").arg(url).spawn();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let _ = std::process::Command::new("xdg-open").arg(url).spawn();
    }
    Ok(())
}

impl Controller {
    fn resolve_known_asset(&self, mut asset: AssetId) -> AssetId {
        if asset.venue == Venue::Iex {
            let binance_key = format!("BINANCE:{}", asset.symbol);
            if self.app.market.instruments.contains_key(&binance_key)
                || self.app.market.quotes.contains_key(&binance_key)
            {
                asset.venue = Venue::Binance;
                asset.asset_class = AssetClass::Crypto;
            }
        }
        asset
    }
    pub fn command(&mut self, text: &str) -> Result<Option<ResearchRequest>> {
        self.app.message = format!("Command: {text}");
        let mut research = None;
        match command::parse(text).map_err(anyhow::Error::msg)? {
            Command::Open(workspace, asset, timeframe) => {
                if workspace != self.app.workspace {
                    self.app
                        .workspace_history
                        .push((self.app.workspace, self.app.asset.clone()));
                    if self.app.workspace_history.len() > 32 {
                        self.app.workspace_history.remove(0);
                    }
                    self.app.cursor = 0;
                    self.app.candle_cursor = None;
                }
                self.app.workspace = workspace;
                self.app.des_mode = false;
                if workspace == Workspace::Market && self.app.market_mode == MarketMode::Wei {
                    self.app.market_mode = MarketMode::Overview;
                }
                if let Some(raw_asset) = asset.clone() {
                    let resolved = if matches!(workspace, Workspace::Fin | Workspace::News) {
                        raw_asset
                    } else {
                        self.resolve_known_asset(raw_asset)
                    };
                    if resolved != self.app.asset {
                        self.app.company = None;
                        self.app.cursor = 0;
                        self.app.candle_cursor = None;
                    }
                    self.app.set_active_asset(resolved);
                }
                if let Some(timeframe) = timeframe {
                    self.app.timeframe = timeframe;
                }
                self.app.offset = 0;
                let key = format!("{}:{}", self.app.asset.key(), self.app.timeframe);
                if self.app.market.chart_key != key {
                    self.app.market.candles.clear();
                    self.app.market.candles_received_at = None;
                    if let Some((bars, at)) = self.store.get::<Vec<Candle>>("candles", &key)? {
                        let is_stale = chrono::Utc::now().signed_duration_since(at).num_seconds()
                            > self.app.timeframe.seconds() * 2;
                        self.app.market.chart_status = if is_stale {
                            omt_core::ChartDataStatus::Stale
                        } else {
                            omt_core::ChartDataStatus::Ready
                        };
                        self.app.market.chart_error = None;
                        self.app.market.candles = bars;
                        self.app.market.candles_received_at = Some(at);
                        self.app.message = format!("Historical cache fetched {at}");
                    } else if self.app.asset.venue == Venue::Iex
                        && std::env::var("APCA_API_KEY_ID").is_err()
                    {
                        self.app.market.chart_status =
                            omt_core::ChartDataStatus::AuthenticationRequired;
                        self.app.market.chart_error = Some(
                            "Set APCA_API_KEY_ID and APCA_API_SECRET_KEY for IEX equities".into(),
                        );
                    } else {
                        self.app.market.chart_status = omt_core::ChartDataStatus::Loading;
                        self.app.market.chart_error = None;
                    }
                    self.app.market.chart_key = key;
                }
                if workspace == Workspace::Fin {
                    if self.app.asset.venue != Venue::Iex {
                        self.app.research_status =
                            "SEC financials require a US company symbol, e.g. FIN AAPL".into();
                    } else {
                        research = self.company(&self.app.asset.symbol.clone())?;
                    }
                } else if workspace == Workspace::News {
                    if let Some(ref raw) = asset {
                        self.app.news_ticker_filter = Some(raw.symbol.clone());
                        if raw.venue == Venue::Iex {
                            research = self.company(&raw.symbol)?;
                        } else {
                            research = Some(ResearchRequest::News(
                                NewsCategory::All,
                                Some(raw.symbol.clone()),
                            ));
                        }
                    } else if self.app.news_items.is_empty() {
                        research = Some(ResearchRequest::News(NewsCategory::All, None));
                    }
                }
                if workspace == Workspace::Econ && self.app.macro_data.is_empty() {
                    research = self.macro_request("TREASURY".into(), ResearchRequest::Treasury)?;
                }
            }
            Command::Help => self.app.help = true,
            Command::Settings => self.app.settings = true,
            Command::Watch(add, raw_asset) => {
                let asset = self.resolve_known_asset(raw_asset);
                if add && self.app.watchlist.len() >= 40 {
                    bail!("Watchlist limit is 40 assets to bound detailed subscriptions");
                }
                self.store.watch(&self.app.list_name, &asset, add)?;
                self.app.watchlist = self.store.watchlist(&self.app.list_name)?;
                self.app.message = format!(
                    "{} {} in watchlist '{}'",
                    if add { "Added" } else { "Removed" },
                    asset.key(),
                    self.app.list_name
                );
            }
            Command::Watchlist(name) => {
                if name.len() > 64 {
                    bail!("Watchlist name is too long");
                }
                self.store.ensure_watchlist(&name)?;
                self.app.list_name = name;
                self.app.watchlist = self.store.watchlist(&self.app.list_name)?;
                self.app.watch_cursor = 0;
            }
            Command::Raw(parts) => {
                let head = parts[0].to_ascii_uppercase();
                match head.as_str() {
                    "ALERT" => match parts.get(1).map(|s| s.to_ascii_uppercase()).as_deref() {
                        None | Some("LIST") => {
                            self.app.alerts_open = true;
                            self.app.cursor = 0;
                        }
                        Some("ADD") if parts.len() == 5 => {
                            if self.app.alerts.len() >= 40 {
                                bail!("Maximum 40 saved alerts; remove old alerts first");
                            }
                            let asset = self
                                .resolve_known_asset(parts[2].parse().map_err(anyhow::Error::msg)?);
                            let above = match parts[3].to_ascii_uppercase().as_str() {
                                "ABOVE" => true,
                                "BELOW" => false,
                                _ => bail!("Use ABOVE or BELOW"),
                            };
                            let threshold: rust_decimal::Decimal = parts[4].parse()?;
                            if threshold <= rust_decimal::Decimal::ZERO {
                                bail!("Threshold must be positive");
                            }
                            let id = self.app.alerts.iter().map(|a| a.id).max().unwrap_or(0) + 1;
                            self.app.alerts.push(omt_core::alerts::PriceAlert {
                                id,
                                asset,
                                above,
                                threshold,
                                created_at: Utc::now(),
                                triggered_at: None,
                                triggered_price: None,
                            });
                            self.store.put("alerts", "rules", &self.app.alerts)?;
                            self.app.message = format!(
                                "Alert #{id} armed; first fresh matching quote triggers once"
                            );
                        }
                        Some("REMOVE") if parts.len() == 3 => {
                            let id: u64 = parts[2].parse()?;
                            if !self.app.alerts.iter().any(|a| a.id == id) {
                                bail!("Alert ID not found");
                            }
                            self.app.alerts.retain(|a| a.id != id);
                            self.store.put("alerts", "rules", &self.app.alerts)?;
                            self.app.message = format!("Removed alert #{id}");
                        }
                        _ => bail!(
                            "ALERT ADD SYMBOL ABOVE|BELOW PRICE | ALERT LIST | ALERT REMOVE ID"
                        ),
                    },
                    "STUDY" => {
                        let item = views::selected_news_item(&self.app)
                            .context("Select a NEWS headline first")?;
                        let symbol = match (item.category, item.ticker.as_deref()) {
                            (NewsCategory::Macro, _) | (_, None) => self.app.asset.key(),
                            (NewsCategory::Crypto, Some(t))
                                if !t.contains('-')
                                    && !t.ends_with("USDT")
                                    && !t.ends_with("USDC") =>
                            {
                                format!("BINANCE:{t}USDT")
                            }
                            (_, Some(t)) => t.to_owned(),
                        };
                        let date = item.published_at.date_naive();
                        research = self.command(&format!("IMPACT {symbol}"))?;
                        self.app.impact_event = Some(date);
                        self.app.impact_origin = Some(item);
                        self.app.message = format!(
                            "Studying selected headline date {date}; daily association only"
                        );
                    }
                    "IMPACT" | "COMPARE" => {
                        if parts.len() > 3 {
                            bail!("IMPACT [SYMBOL] [BENCHMARK]");
                        }
                        if parts.len() == 1 && self.app.impact.is_some() {
                            self.app.workspace = Workspace::Impact;
                            return Ok(None);
                        }
                        let asset = parts
                            .get(1)
                            .map(|s| s.parse::<AssetId>().map_err(anyhow::Error::msg))
                            .transpose()?
                            .unwrap_or_else(|| self.app.asset.clone());
                        let asset = self.resolve_known_asset(asset);
                        let benchmark = parts
                            .get(2)
                            .map(|s| s.parse::<AssetId>().map_err(anyhow::Error::msg))
                            .transpose()?
                            .unwrap_or_else(|| {
                                if asset.venue == Venue::Iex {
                                    "IEX:SPY".parse().unwrap()
                                } else if asset.symbol == "BTCUSDT" {
                                    "BINANCE:ETHUSDT".parse().unwrap()
                                } else {
                                    "BINANCE:BTCUSDT".parse().unwrap()
                                }
                            });
                        let benchmark = self.resolve_known_asset(benchmark);
                        if asset == benchmark {
                            bail!("Choose two different instruments for IMPACT");
                        }
                        self.app
                            .workspace_history
                            .push((self.app.workspace, self.app.asset.clone()));
                        self.app.workspace = Workspace::Impact;
                        self.app.set_active_asset(asset.clone());
                        self.app.cursor = 0;
                        self.app.impact_event = None;
                        self.app.impact_origin = None;
                        self.app.impact_pair = Some((asset.clone(), benchmark.clone()));
                        let key = format!("{}|{}", asset.key(), benchmark.key());
                        self.app.impact = self
                            .store
                            .get::<omt_core::impact::ImpactData>("impact", &key)?
                            .map(|v| v.0);
                        self.app.impact_status = if self.offline {
                            "Offline: saved study only"
                        } else {
                            "Loading daily prices and related news..."
                        }
                        .into();
                        research = Some(ResearchRequest::Impact(asset, benchmark));
                    }
                    "EVENT" => {
                        if parts.len() != 2 {
                            bail!("EVENT YYYY-MM-DD or EVENT CLEAR");
                        }
                        let date = if parts[1].eq_ignore_ascii_case("CLEAR") {
                            None
                        } else {
                            Some(
                                chrono::NaiveDate::parse_from_str(&parts[1], "%Y-%m-%d")
                                    .context("Use EVENT YYYY-MM-DD")?,
                            )
                        };
                        self.app.impact_event = date;
                        self.app.impact_origin = None;
                        self.app.workspace = Workspace::Impact;
                    }
                    "PLOT" => {
                        if self.app.workspace == Workspace::Impact {
                            if let Some(arg) = parts.get(1) {
                                let arg_u = arg.to_ascii_uppercase();
                                match arg_u.as_str() {
                                    "REL" | "1" => self.app.impact_plot_mode = 0,
                                    "EVENT" | "2" => self.app.impact_plot_mode = 1,
                                    "CAR" | "3" => self.app.impact_plot_mode = 2,
                                    "SPREAD" | "4" => self.app.impact_plot_mode = 3,
                                    _ => bail!("PLOT [REL | EVENT | CAR | SPREAD]"),
                                }
                            } else {
                                self.app.impact_plot_mode = (self.app.impact_plot_mode + 1) % 4;
                            }
                            let mode_desc = match self.app.impact_plot_mode {
                                0 => {
                                    "Relative Performance (Normalized base=100 from first common close)"
                                }
                                1 => {
                                    "Event Window Trajectory (t-2 to t+5 re-indexed to baseline=100)"
                                }
                                2 => "Cumulative Abnormal Return / CAR (excess percentage points)",
                                _ => {
                                    "Daily Return Spread (Asset return % minus Benchmark return %)"
                                }
                            };
                            self.app.message = format!("IMPACT Visualization Mode: {mode_desc}");
                        } else if self.app.workspace == Workspace::Fin {
                            self.app.fin_chart_expanded = !self.app.fin_chart_expanded;
                            self.app.message = if self.app.fin_chart_expanded {
                                "FIN Trends: Visual Multi-Period Income & Revenue Chart active (press 'p' for tables)".into()
                            } else {
                                "FIN Statements: Multi-period comparative tables active (press 'p' for charts)".into()
                            };
                        } else {
                            self.app.workspace = Workspace::Chart;
                            self.app.message = "CHART: Candlestick price chart active".into();
                        }
                    }

                    "CRYPTO" => {
                        self.app.workspace = Workspace::Market;
                        self.app.market_mode = MarketMode::Crypto;
                        self.app.quote_filter = parts
                            .get(1)
                            .map(|s| s.to_ascii_uppercase())
                            .unwrap_or_else(|| "ALL".into());
                        self.app.cursor = 0;
                        self.app.sync_market_selection();
                        self.app.message = format!(
                            "Binance Spot Universe Explorer — Quote filter: {}",
                            self.app.quote_filter
                        );
                    }
                    "WEI" => {
                        self.app.workspace = Workspace::Market;
                        self.app.market_mode = MarketMode::Wei;
                        self.app.cursor = 0;
                        self.app.message =
                            "World Equity Indices (WEI) — Regional benchmarks & source status"
                                .into();
                    }
                    "MOST" => {
                        self.app.workspace = Workspace::Market;
                        self.app.market_mode = MarketMode::Overview;
                        self.app.sort = "qvol".into();
                        self.app.sort_ascending = false;
                        self.app.cursor = 0;
                        self.app.sync_market_selection();
                        self.app.message = "Most Active Instruments (24h Quote Volume)".into();
                    }
                    "MARKET" | "MKT" => {
                        self.app.workspace = Workspace::Market;
                        self.app.market_mode = MarketMode::Overview;
                        if let Some(sub) = parts.get(1).map(|s| s.to_ascii_uppercase()) {
                            match sub.as_str() {
                                "GAINERS" => {
                                    self.app.sort = "change".into();
                                    self.app.sort_ascending = false;
                                }
                                "LOSERS" => {
                                    self.app.sort = "change".into();
                                    self.app.sort_ascending = true;
                                }
                                "ACTIVE" => {
                                    self.app.sort = "qvol".into();
                                    self.app.sort_ascending = false;
                                }
                                "OVERVIEW" => {}
                                q => {
                                    self.app.quote_filter = q.into();
                                }
                            }
                        }
                        self.app.cursor = 0;
                        self.app.sync_market_selection();
                    }
                    "SCAN" | "SRCH" => {
                        self.app.workspace = Workspace::Scan;
                        self.app.cursor = 0;
                        if let Some(sub) = parts.get(1).map(|s| s.to_ascii_uppercase()) {
                            match sub.as_str() {
                                "CRYPTO" => {
                                    self.app.scan_mode = ScanMode::Crypto;
                                    if parts.len() > 2 {
                                        self.app.filter = parts[2..].join(" ");
                                    }
                                    self.app.sync_market_selection();
                                }
                                "EQUITIES" | "EQUITY" | "STOCKS" | "SEC" => {
                                    self.app.scan_mode = ScanMode::Equities;
                                    if parts.len() > 2 {
                                        self.app.filter = parts[2..].join(" ");
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                    "FIN_TAB" => {
                        if self.app.workspace != Workspace::Fin {
                            self.app
                                .workspace_history
                                .push((self.app.workspace, self.app.asset.clone()));
                        }
                        self.app.workspace = Workspace::Fin;
                        self.app.cursor = 0;
                        if let Some(sub) = parts.get(1).map(|s| s.to_ascii_uppercase()) {
                            self.app.stmt_tab = match sub.as_str() {
                                "INCOME" => StatementTab::Income,
                                "BALANCE" | "BS" => StatementTab::BalanceSheet,
                                "CASH" | "CF" => StatementTab::CashFlow,
                                "RATIOS" => StatementTab::Ratios,
                                _ => StatementTab::Overview,
                            };
                        }
                        if let Some(ticker) = parts.get(2) {
                            let asset: AssetId = ticker.parse().map_err(anyhow::Error::msg)?;
                            self.app.asset = asset;
                            research = self.company(&ticker.to_ascii_uppercase())?;
                        }
                    }
                    "QUOTE" => {
                        let q = parts
                            .get(1)
                            .context("QUOTE <USDT|USDC|BTC|FDUSD|EUR|ALL>")?
                            .to_ascii_uppercase();
                        self.app.quote_filter = q.clone();
                        self.app.cursor = 0;
                        self.app.sync_market_selection();
                        self.app.message = format!("Quote currency filter: {q}");
                    }
                    "MINVOL" => {
                        let v: rust_decimal::Decimal = parts
                            .get(1)
                            .context("MINVOL <amount>")?
                            .parse()
                            .context("Invalid minimum volume")?;
                        self.app.min_quote_volume = v;
                        self.app.cursor = 0;
                        self.app.sync_market_selection();
                        self.app.message = format!("Minimum 24h quote volume: {v}");
                    }
                    "STATUS" => {
                        let st = parts
                            .get(1)
                            .context("STATUS TRADING|ALL")?
                            .to_ascii_uppercase();
                        self.app.status_filter_trading_only = st != "ALL";
                        self.app.cursor = 0;
                        self.app.sync_market_selection();
                    }
                    "OPEN" => {
                        if let Some(url) = views::selected_filing_url(&self.app) {
                            if !self.offline {
                                open_terminal_url_safely(&url)?;
                            }
                            self.app.message = format!("Opened URL: {url}");
                        } else {
                            bail!("Select a news item, filing, or metric to open");
                        }
                    }
                    "BACK" => {
                        if let Some((prev_w, prev_a)) = self.app.workspace_history.pop() {
                            self.app.workspace = prev_w;
                            self.app.asset = prev_a;
                            self.app.cursor = 0;
                            self.app.message = format!("Returned to {}", prev_w.name());
                        } else {
                            self.app.message = "No prior workspace in history".into();
                        }
                    }
                    "ACCOUNT" => {
                        let name = parts.get(1).context("ACCOUNT name")?;
                        if name.len() > 64 {
                            bail!("Account name too long");
                        }
                        self.app.account = name.clone();
                        self.store.set("account", name)?;
                        self.app.workspace = Workspace::Port;
                        self.app.cursor = 0;
                    }
                    "PRESET" => {
                        if parts.len() != 3 {
                            bail!("PRESET SAVE|LOAD name");
                        }
                        let key = format!("screener:{}", parts[2]);
                        match parts[1].to_ascii_uppercase().as_str() {
                            "SAVE" => self.store.put(
                                "screener_v2",
                                &key,
                                &(
                                    self.app.filter.clone(),
                                    self.app.sort.clone(),
                                    self.app.sort_ascending,
                                    self.app.quote_filter.clone(),
                                    self.app.min_quote_volume,
                                    self.app.status_filter_trading_only,
                                    self.app.scan_mode == ScanMode::Equities,
                                ),
                            )?,
                            "LOAD" => {
                                type Preset = (
                                    String,
                                    String,
                                    bool,
                                    String,
                                    rust_decimal::Decimal,
                                    bool,
                                    bool,
                                );
                                if let Some((
                                    (filter, sort, ascending, quote, minvol, trading, equities),
                                    _,
                                )) = self.store.get::<Preset>("screener_v2", &key)?
                                {
                                    self.app.filter = filter;
                                    self.app.sort = sort;
                                    self.app.sort_ascending = ascending;
                                    self.app.quote_filter = quote;
                                    self.app.min_quote_volume = minvol;
                                    self.app.status_filter_trading_only = trading;
                                    self.app.scan_mode = if equities {
                                        ScanMode::Equities
                                    } else {
                                        ScanMode::Crypto
                                    };
                                } else {
                                    let value =
                                        self.store.setting(&key)?.context("Preset not found")?;
                                    let (filter, sort) =
                                        value.split_once('|').context("Invalid preset")?;
                                    self.app.filter = filter.into();
                                    self.app.sort = sort.into();
                                }
                                self.app.cursor = 0;
                                self.app.sync_market_selection();
                            }
                            _ => bail!("PRESET SAVE|LOAD name"),
                        }
                        self.app.workspace = Workspace::Scan;
                    }
                    "BUY" | "SELL" => {
                        if !(4..=6).contains(&parts.len()) {
                            bail!("BUY|SELL SYMBOL QUANTITY PRICE [FEE] [RFC3339 TIMESTAMP]");
                        }
                        let asset = self.resolve_known_asset(
                            parts[1]
                                .parse::<omt_core::AssetId>()
                                .map_err(anyhow::Error::msg)?,
                        );
                        let now = Utc::now();
                        let t = Transaction {
                            id: format!(
                                "{}-{}",
                                now.timestamp_nanos_opt().unwrap_or(0),
                                self.app.transactions.len()
                            ),
                            account: self.app.account.clone(),
                            currency: asset.currency().into(),
                            asset,
                            side: if head == "BUY" { Side::Buy } else { Side::Sell },
                            quantity: parts[2].parse()?,
                            price: parts[3].parse()?,
                            fee: parts
                                .get(4)
                                .map(|s| s.parse())
                                .transpose()?
                                .unwrap_or_default(),
                            at: parts
                                .get(5)
                                .map(|s| {
                                    DateTime::parse_from_rfc3339(s).map(|d| d.with_timezone(&Utc))
                                })
                                .transpose()?
                                .unwrap_or(now),
                        };
                        self.store.add_transactions(&[t])?;
                        self.app.transactions = self.store.transactions()?;
                        self.app.workspace = Workspace::Port;
                        self.app.message = "Transaction saved locally".into();
                    }
                    "DELTX" => {
                        let id = parts.get(1).context("DELTX <transaction_id>")?;
                        self.store.delete_transaction(id)?;
                        self.app.transactions = self.store.transactions()?;
                        self.app.workspace = Workspace::Port;
                        self.app.message = format!("Deleted transaction {id}");
                    }
                    "IND" => {
                        self.app.overlay =
                            match parts.get(1).map(|s| s.to_ascii_uppercase()).as_deref() {
                                Some("SMA") => OverlayMode::Sma,
                                Some("EMA") => OverlayMode::Ema,
                                Some("BB" | "BOLL" | "BBANDS") => OverlayMode::Bollinger,
                                Some("NONE" | "OFF") => OverlayMode::None,
                                None => self.app.overlay.next(),
                                _ => bail!("IND SMA|EMA|BB|NONE"),
                            };
                        self.app.workspace = Workspace::Chart;
                        self.app.message = format!("Chart overlay: {}", self.app.overlay.label());
                    }
                    "IMPORT" | "EXPORT" => {
                        let path = text
                            .split_once(char::is_whitespace)
                            .map(|(_, p)| p.trim().trim_matches('"'))
                            .context("Provide a CSV path")?;
                        let count = if head == "IMPORT" {
                            self.store.import_csv(Path::new(path))?
                        } else {
                            self.store.export_csv(Path::new(path))?
                        };
                        self.app.transactions = self.store.transactions()?;
                        self.app.message = format!("{head}: {count} transactions");
                    }
                    "FILTER" => {
                        let value = if parts.len() > 1 {
                            parts[1..].join(" ")
                        } else {
                            String::new()
                        };
                        if value.eq_ignore_ascii_case("CLEAR") || value.eq_ignore_ascii_case("NONE")
                        {
                            self.app.filter.clear();
                            self.app.cursor = 0;
                            self.app.sync_market_selection();
                            if !matches!(self.app.workspace, Workspace::Market | Workspace::Scan) {
                                self.app.workspace = Workspace::Scan;
                            }
                        } else {
                            let mut has_equity_field = false;
                            for token in value.split_whitespace() {
                                if token.contains(['>', '<']) {
                                    let (field, number) =
                                        token.split_once(['>', '<']).context("Invalid filter")?;
                                    let f_low = field.to_ascii_lowercase();
                                    if ![
                                        "price",
                                        "change",
                                        "chg",
                                        "volume",
                                        "vol",
                                        "qvol",
                                        "quote_volume",
                                        "revenue",
                                        "income",
                                        "assets",
                                    ]
                                    .contains(&f_low.as_str())
                                    {
                                        bail!(
                                            "Filter fields: price, change, volume, qvol, revenue, income, assets"
                                        );
                                    }
                                    if ["revenue", "income", "assets"].contains(&f_low.as_str()) {
                                        has_equity_field = true;
                                    }
                                    number.parse::<rust_decimal::Decimal>()?;
                                }
                            }
                            self.app.filter = value;
                            if has_equity_field {
                                self.app.scan_mode = ScanMode::Equities;
                            }
                            if !matches!(self.app.workspace, Workspace::Market | Workspace::Scan) {
                                self.app.workspace = Workspace::Scan;
                            }
                            self.app.cursor = 0;
                            self.app.sync_market_selection();
                        }
                    }
                    "SORT" => {
                        let field = parts
                            .get(1)
                            .context("SORT price|change|volume|qvol|symbol|revenue|income|assets [ASC|DESC]")?
                            .to_ascii_lowercase();
                        if ![
                            "price",
                            "change",
                            "volume",
                            "vol",
                            "qvol",
                            "quote_volume",
                            "symbol",
                            "alpha",
                            "revenue",
                            "income",
                            "assets",
                        ]
                        .contains(&field.as_str())
                        {
                            bail!("SORT price|change|volume|qvol|symbol|revenue|income|assets");
                        }
                        if let Some(dir) = parts.get(2).map(|s| s.to_ascii_uppercase()) {
                            match dir.as_str() {
                                "ASC" => self.app.sort_ascending = true,
                                "DESC" => self.app.sort_ascending = false,
                                _ => bail!("SORT <field> [ASC|DESC]"),
                            }
                        } else {
                            self.app.sort_ascending = matches!(field.as_str(), "symbol" | "alpha");
                        }
                        self.app.sort = field;
                        if ["revenue", "income", "assets"].contains(&self.app.sort.as_str()) {
                            self.app.scan_mode = ScanMode::Equities;
                        }
                        if let Some(ref key) = self.app.selected_market_key.clone() {
                            let keys = views::filtered_market_keys(&self.app);
                            if let Some(idx) = keys.iter().position(|k| k == key) {
                                self.app.cursor = idx;
                            }
                        }
                    }
                    "INDEX" => {
                        let ticker = parts
                            .get(1)
                            .context(
                                "INDEX TICKER explicitly fetches one company into the local index",
                            )?
                            .to_ascii_uppercase();
                        research = self.company(&ticker)?;
                        self.app.workspace = Workspace::Scan;
                        self.app.scan_mode = ScanMode::Equities;
                    }
                    "TREASURY" => {
                        research =
                            self.macro_request("TREASURY".into(), ResearchRequest::Treasury)?
                    }
                    "WB" => {
                        if parts.len() != 3 {
                            bail!("WB COUNTRY SERIES, e.g. WB US NY.GDP.MKTP.CD");
                        }
                        research = self.macro_request(
                            format!("WB {} {}", parts[1], parts[2]),
                            ResearchRequest::WorldBank(parts[1].clone(), parts[2].clone()),
                        )?;
                    }
                    "FRED" => {
                        let series = parts.get(1).context("FRED SERIES")?.to_ascii_uppercase();
                        research = self.macro_request(
                            format!("FRED {series}"),
                            ResearchRequest::Fred(series),
                        )?;
                    }
                    "TOP" => {
                        if self.app.workspace != Workspace::News {
                            self.app
                                .workspace_history
                                .push((self.app.workspace, self.app.asset.clone()));
                        }
                        self.app.workspace = Workspace::News;
                        self.app.news_category = NewsCategory::All;
                        self.app.news_ticker_filter = None;
                        self.app.cursor = 0;
                        self.app.news_scroll = 0;
                        self.app.message = "TOP — Top Financial & Market Headlines".into();
                        research = Some(ResearchRequest::News(NewsCategory::All, None));
                    }
                    "CN" => {
                        if self.app.workspace != Workspace::News {
                            self.app
                                .workspace_history
                                .push((self.app.workspace, self.app.asset.clone()));
                        }
                        self.app.workspace = Workspace::News;
                        let sym = self.app.asset.symbol.clone();
                        self.app.news_ticker_filter = Some(sym.clone());
                        self.app.cursor = 0;
                        self.app.news_scroll = 0;
                        self.app.message = format!("CN — Company News & Disclosures for {sym}");
                        if self.app.asset.venue == Venue::Iex {
                            research = self.company(&sym)?;
                        } else {
                            research = Some(ResearchRequest::News(NewsCategory::All, Some(sym)));
                        }
                    }
                    "NEWS" => {
                        if self.app.workspace != Workspace::News {
                            self.app
                                .workspace_history
                                .push((self.app.workspace, self.app.asset.clone()));
                        }
                        self.app.workspace = Workspace::News;
                        self.app.cursor = 0;
                        self.app.news_scroll = 0;
                        if parts.len() == 1 {
                            self.app.news_category = NewsCategory::All;
                            self.app.news_ticker_filter = None;
                            self.app.message =
                                "NEWS — All Real-Time Financial Feeds & SEC Disclosures".into();
                            research = Some(ResearchRequest::News(NewsCategory::All, None));
                        } else if parts.len() == 2 {
                            if let Some(cat) = NewsCategory::from_token(&parts[1]) {
                                self.app.news_category = cat;
                                self.app.news_ticker_filter = None;
                                self.app.message = format!("NEWS — Category: {}", cat.label());
                                research = Some(ResearchRequest::News(cat, None));
                            } else {
                                let sym = parts[1].to_ascii_uppercase();
                                self.app.news_ticker_filter = Some(sym.clone());
                                self.app.message = format!("NEWS — Filtered to {sym}");
                                if let Ok(asset) = sym.parse::<AssetId>() {
                                    let resolved = self.resolve_known_asset(asset);
                                    self.app.set_active_asset(resolved);
                                    if self.app.asset.venue == Venue::Iex {
                                        research = self.company(&sym)?;
                                    } else {
                                        research = Some(ResearchRequest::News(
                                            NewsCategory::All,
                                            Some(sym),
                                        ));
                                    }
                                }
                            }
                        } else {
                            let sym = parts[1].to_ascii_uppercase();
                            self.app.news_ticker_filter = Some(sym.clone());
                            if let Some(cat) = NewsCategory::from_token(&parts[2]) {
                                self.app.news_category = cat;
                                self.app.message = format!("NEWS — {} for {sym}", cat.label());
                                research = Some(ResearchRequest::News(cat, Some(sym.clone())));
                            }
                            if parts[2].eq_ignore_ascii_case("SEC")
                                || parts[2].eq_ignore_ascii_case("FILINGS")
                            {
                                research = self.company(&sym)?;
                            }
                        }
                    }
                    "FILINGS" | "CF" => {
                        if self.app.workspace != Workspace::News {
                            self.app
                                .workspace_history
                                .push((self.app.workspace, self.app.asset.clone()));
                        }
                        self.app.workspace = Workspace::News;
                        self.app.news_category = NewsCategory::Filings;
                        self.app.cursor = 0;
                        self.app.news_scroll = 0;
                        let sym = parts
                            .get(1)
                            .map(|s| s.to_ascii_uppercase())
                            .unwrap_or_else(|| self.app.asset.symbol.clone());
                        self.app.news_ticker_filter = Some(sym.clone());
                        self.app.message = format!("SEC EDGAR Filings (10-K, 10-Q, 8-K) — {sym}");
                        research = self.company(&sym)?;
                    }
                    "READ" => {
                        if self.app.workspace == Workspace::News {
                            self.app.news_reader_expanded = !self.app.news_reader_expanded;
                            if let Some(item) = views::selected_news_item(&self.app) {
                                if item.body.is_none() && !item.url.is_empty() {
                                    if let Ok(Some((cached_body, _))) =
                                        self.store.get::<String>("articles", &item.id)
                                    {
                                        if let Some(real_item) =
                                            self.app.news_items.iter_mut().find(|n| n.id == item.id)
                                        {
                                            real_item.body = Some(cached_body);
                                        }
                                    } else {
                                        research = Some(ResearchRequest::Article(item.clone()));
                                    }
                                }
                                self.app.message = format!(
                                    "News Reader {}: {}",
                                    if self.app.news_reader_expanded {
                                        "Expanded"
                                    } else {
                                        "Split"
                                    },
                                    item.headline
                                );
                            }
                        }
                    }
                    "REFRESH" => match self.app.workspace {
                        Workspace::Impact => {
                            if let Some((asset, benchmark)) = self.app.impact_pair.clone() {
                                self.app.impact_status = if self.offline {
                                    "Offline: saved study only"
                                } else {
                                    "Refreshing study..."
                                }
                                .into();
                                research = Some(ResearchRequest::Impact(asset, benchmark));
                            }
                        }
                        Workspace::News => {
                            self.app.message =
                                "Refreshing news feeds and regulatory filings...".into();
                            if let Some(ref sym) = self.app.news_ticker_filter.clone() {
                                if self.app.asset.venue == Venue::Iex {
                                    research = self.company(sym)?;
                                } else {
                                    research = Some(ResearchRequest::News(
                                        self.app.news_category,
                                        Some(sym.clone()),
                                    ));
                                }
                            } else {
                                research =
                                    Some(ResearchRequest::News(self.app.news_category, None));
                            }
                        }
                        Workspace::Chart => {
                            self.app.market.chart_key.clear();
                            self.app.market.chart_status = omt_core::ChartDataStatus::Loading;
                            self.app.market.chart_error = None;
                            self.app.message = format!(
                                "Refreshing chart candles for {} ({})...",
                                self.app.asset.key(),
                                self.app.timeframe
                            );
                        }
                        Workspace::Fin => {
                            research = self.company(&self.app.asset.symbol.clone())?;
                            self.app.message = format!(
                                "Refreshing SEC financials for {}...",
                                self.app.asset.symbol
                            );
                        }
                        _ => {
                            self.app.message = "Refreshed workspace state".into();
                        }
                    },
                    "LAUNCHPAD" => {
                        self.app.layout_mode = LayoutMode::Quad;
                        self.app.full_width = false;
                        self.app.message = "Launchpad 2×2 Multi-Monitor Layout active".into();
                    }
                    "LAYOUT" => {
                        let sub = parts
                            .get(1)
                            .context("LAYOUT STANDARD|FULL|SPLIT|QUAD|WATCH|CONTEXT")?
                            .to_ascii_uppercase();
                        match sub.as_str() {
                            "STANDARD" | "STD" | "DEFAULT" => {
                                self.app.layout_mode = LayoutMode::Standard;
                                self.app.full_width = false;
                            }
                            "FULL" | "ZOOM" | "MAX" => {
                                self.app.layout_mode = LayoutMode::FullWidth;
                                self.app.full_width = true;
                            }
                            "SPLIT" | "DUAL" | "2PANE" => {
                                self.app.layout_mode = LayoutMode::Split;
                                self.app.full_width = false;
                            }
                            "QUAD" | "LAUNCHPAD" | "4PANE" => {
                                self.app.layout_mode = LayoutMode::Quad;
                                self.app.full_width = false;
                            }
                            "WATCH" => {
                                self.app.show_watchlist = !self.app.show_watchlist;
                            }
                            "CONTEXT" | "INSPECTOR" | "INFO" => {
                                self.app.show_context = !self.app.show_context;
                            }
                            _ => bail!("LAYOUT STANDARD|FULL|SPLIT|QUAD|WATCH|CONTEXT"),
                        }
                        self.app.message =
                            format!("Workstation layout: {}", self.app.layout_mode.label());
                    }
                    "DES" => {
                        if parts.len() > 1 {
                            let sym = parts[1].to_ascii_uppercase();
                            if let Ok(asset) = sym.parse::<AssetId>() {
                                let resolved = self.resolve_known_asset(asset);
                                self.app.set_active_asset(resolved);
                            }
                        }
                        self.app.des_mode = true;
                        if self.app.asset.venue == Venue::Iex {
                            self.app.workspace = Workspace::Fin;
                            research = self.company(&self.app.asset.symbol.clone())?;
                        } else {
                            self.app.workspace = Workspace::Chart;
                        }
                        self.app.message =
                            format!("Security Description <DES> — {}", self.app.asset.key());
                    }
                    "OLDER_HISTORY" => {
                        if let Some(first_bar) = self.app.market.candles.first() {
                            let oldest_time = first_bar.open_at.timestamp_millis();
                            research = Some(ResearchRequest::OlderHistory(
                                self.app.asset.clone(),
                                self.app.timeframe,
                                oldest_time,
                            ));
                            self.app.message =
                                format!("Paginating history before {oldest_time}...");
                        }
                    }
                    _ => bail!("Unsupported command"),
                }
            }
        }
        Ok(if self.offline { None } else { research })
    }
    fn company(&mut self, ticker: &str) -> Result<Option<ResearchRequest>> {
        if let Some((company, at)) = self.store.get::<Company>("company", ticker)? {
            let has_modern_filings = company
                .filings
                .first()
                .is_none_or(|f| !f.description.is_empty());
            if self.app.asset.symbol.eq_ignore_ascii_case(ticker) {
                self.app.company = Some(company);
                self.app.research_status = format!("Cached SEC data fetched {at}");
            }
            if has_modern_filings && Utc::now().signed_duration_since(at).num_hours() < 6 {
                return Ok(None);
            }
        } else {
            self.app.research_status = if self.offline {
                "Offline: no cached SEC company"
            } else {
                "Loading SEC company facts and filings"
            }
            .into();
        }
        Ok(Some(ResearchRequest::Company(ticker.into())))
    }
    fn macro_request(
        &mut self,
        key: String,
        request: ResearchRequest,
    ) -> Result<Option<ResearchRequest>> {
        self.app.workspace = Workspace::Econ;
        self.app.cursor = 0;
        if self.macro_key != key {
            self.app.macro_data.clear();
        }
        self.macro_key = key.clone();
        if let Some((rows, at)) = self.store.get::<Vec<MacroObservation>>("macro", &key)? {
            self.app.macro_data = rows;
            self.app.research_status = format!("Cached observations fetched {at}");
            if Utc::now().signed_duration_since(at).num_hours() < 24 {
                return Ok(None);
            }
        } else {
            self.app.research_status = if self.offline {
                "Offline: no cached observations"
            } else {
                "Loading official dated observations"
            }
            .into();
        }
        Ok(Some(request))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omt_core::{Decimal, Filing, FinancialFact};

    #[test]
    fn impact_alerts_and_complete_presets() {
        let mut c = Controller {
            app: App::default(),
            store: Store::memory().unwrap(),
            offline: true,
            macro_key: String::new(),
        };
        assert!(c.command("IMPACT AAPL SPY").unwrap().is_none());
        assert_eq!(c.app.workspace, Workspace::Impact);
        assert_eq!(c.app.impact_pair.as_ref().unwrap().1.symbol, "SPY");
        assert!(c.command("IMPACT AAPL AAPL").is_err());
        c.command("EVENT 2026-01-15").unwrap();
        assert!(c.app.impact_event.is_some());
        assert!(c.command("EVENT 2026-02-30").is_err());
        c.command("EVENT CLEAR").unwrap();
        assert!(c.app.impact_event.is_none());
        c.command("PLOT EVENT").unwrap();
        assert_eq!(c.app.impact_plot_mode, 1);
        c.command("PLOT CAR").unwrap();
        assert_eq!(c.app.impact_plot_mode, 2);
        c.command("PLOT SPREAD").unwrap();
        assert_eq!(c.app.impact_plot_mode, 3);
        c.command("PLOT REL").unwrap();
        assert_eq!(c.app.impact_plot_mode, 0);
        c.command("PLOT").unwrap();
        assert_eq!(c.app.impact_plot_mode, 1);
        c.command("ALERT ADD AAPL ABOVE 250").unwrap();
        assert_eq!(
            c.store
                .get::<Vec<omt_core::alerts::PriceAlert>>("alerts", "rules")
                .unwrap()
                .unwrap()
                .0
                .len(),
            1
        );
        assert!(
            subscription(&c.app)
                .assets
                .iter()
                .any(|a| a.symbol == "AAPL")
        );
        assert!(c.command("ALERT ADD AAPL BELOW -1").is_err());
        assert!(c.command("ALERT REMOVE 99").is_err());
        c.command("ALERT REMOVE 1").unwrap();
        assert!(c.app.alerts.is_empty());
        c.command("SCAN EQUITIES").unwrap();
        c.app.filter = "assets>1000".into();
        c.app.sort = "assets".into();
        c.app.sort_ascending = true;
        c.app.quote_filter = "USDC".into();
        c.app.min_quote_volume = 123.into();
        c.app.status_filter_trading_only = false;
        c.command("PRESET SAVE complete").unwrap();
        c.app.filter.clear();
        c.app.sort.clear();
        c.app.sort_ascending = false;
        c.app.quote_filter = "BTC".into();
        c.app.min_quote_volume = 0.into();
        c.app.status_filter_trading_only = true;
        c.app.scan_mode = ScanMode::Crypto;
        c.command("PRESET LOAD complete").unwrap();
        assert_eq!(c.app.scan_mode, ScanMode::Equities);
        assert_eq!(c.app.filter, "assets>1000");
        assert_eq!(c.app.sort, "assets");
        assert!(c.app.sort_ascending);
        assert_eq!(c.app.quote_filter, "USDC");
        assert_eq!(c.app.min_quote_volume, Decimal::from(123));
        assert!(!c.app.status_filter_trading_only);
        c.app.workspace = Workspace::News;
        let action = c
            .app
            .key(crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('r'),
                crossterm::event::KeyModifiers::NONE,
            ))
            .unwrap();
        c.command(&action).unwrap();
        assert!(c.app.news_reader_expanded);
    }

    #[test]
    fn controller_commands_modes_and_cursor_reset() {
        let store = Store::memory().unwrap();
        let mut c = Controller {
            app: App::default(),
            store,
            offline: true,
            macro_key: String::new(),
        };

        // Test CRYPTO ALL command
        c.command("CRYPTO ALL").unwrap();
        assert_eq!(c.app.workspace, Workspace::Market);
        assert_eq!(c.app.market_mode, MarketMode::Crypto);
        assert_eq!(c.app.quote_filter, "ALL");

        // Test WEI command
        c.command("WEI").unwrap();
        assert_eq!(c.app.workspace, Workspace::Market);
        assert_eq!(c.app.market_mode, MarketMode::Wei);

        // Test SCAN EQUITIES and SCAN CRYPTO
        c.command("EQS").unwrap();
        assert_eq!(c.app.workspace, Workspace::Scan);
        assert_eq!(c.app.scan_mode, ScanMode::Equities);
        c.command("SCAN CRYPTO").unwrap();
        assert_eq!(c.app.scan_mode, ScanMode::Crypto);

        // Test QUOTE, MINVOL, STATUS, FILTER, SORT
        c.command("QUOTE USDC").unwrap();
        assert_eq!(c.app.quote_filter, "USDC");
        c.command("MINVOL 1000000").unwrap();
        assert_eq!(c.app.min_quote_volume, Decimal::from(1_000_000));
        c.command("STATUS ALL").unwrap();
        assert!(!c.app.status_filter_trading_only);
        c.command("FILTER change>2 price>10").unwrap();
        assert_eq!(c.app.filter, "change>2 price>10");
        c.command("SORT change ASC").unwrap();
        assert_eq!(c.app.sort, "change");
        assert!(c.app.sort_ascending);

        // Simulate non-zero cursor before switching to NEWS AAPL
        c.app.cursor = 9;
        let cached_company = Company {
            ticker: "AAPL".into(),
            cik: 320193,
            name: "Apple Inc.".into(),
            facts: vec![FinancialFact {
                metric: "Revenue".into(),
                concept: "Revenues".into(),
                value: Decimal::from(124_000_000_000i64),
                unit: "USD".into(),
                start: Some(chrono::NaiveDate::from_ymd_opt(2025, 9, 28).unwrap()),
                end: chrono::NaiveDate::from_ymd_opt(2025, 12, 27).unwrap(),
                filed: chrono::NaiveDate::from_ymd_opt(2026, 1, 30).unwrap(),
                accession: "0000320193-26-000006".into(),
                form: "10-Q".into(),
                source: "SEC".into(),
            }],
            filings: vec![Filing {
                form: "10-Q".into(),
                filed: chrono::NaiveDate::from_ymd_opt(2026, 1, 30).unwrap(),
                report_date: Some(chrono::NaiveDate::from_ymd_opt(2025, 12, 27).unwrap()),
                description: "10-Q".into(),
                accession: "0000320193-26-000006".into(),
                document: "aapl-20251227.htm".into(),
                url: "https://www.sec.gov/Archives/edgar/data/320193/000032019326000006/aapl-20251227.htm".into(),
            }],
            fetched_at: Utc::now(),
        };
        c.store.put("company", "AAPL", &cached_company).unwrap();
        c.command("NEWS AAPL").unwrap();
        assert_eq!(c.app.workspace, Workspace::News);
        // Cursor must be reset to 0 so the newest 2026 filings are visible at the top
        assert_eq!(c.app.cursor, 0);
        assert_eq!(
            c.app.company.as_ref().unwrap().filings[0].filed.to_string(),
            "2026-01-30"
        );

        // Test FIN AAPL BALANCE
        c.command("FIN AAPL BALANCE").unwrap();
        assert_eq!(c.app.workspace, Workspace::Fin);
        assert_eq!(c.app.stmt_tab, omt_core::facts::StatementTab::BalanceSheet);

        // Test BACK command
        c.command("BACK").unwrap();
        assert_eq!(c.app.workspace, Workspace::News);

        // Test TOP command
        c.command("TOP").unwrap();
        assert_eq!(c.app.workspace, Workspace::News);
        assert_eq!(c.app.news_category, NewsCategory::All);

        // Test NEWS CRYPTO command
        c.command("NEWS CRYPTO").unwrap();
        assert_eq!(c.app.workspace, Workspace::News);
        assert_eq!(c.app.news_category, NewsCategory::Crypto);

        // Test CN command
        c.command("CN").unwrap();
        assert_eq!(c.app.workspace, Workspace::News);
        assert_eq!(c.app.news_ticker_filter.as_deref(), Some("AAPL"));

        // Test FILINGS NVDA command
        c.command("FILINGS NVDA").unwrap();
        assert_eq!(c.app.workspace, Workspace::News);
        assert_eq!(c.app.news_category, NewsCategory::Filings);
        assert_eq!(c.app.news_ticker_filter.as_deref(), Some("NVDA"));

        // Test READ command
        let prev_expanded = c.app.news_reader_expanded;
        c.command("READ").unwrap();
        assert_eq!(c.app.news_reader_expanded, !prev_expanded);

        // Test LAUNCHPAD and LAYOUT commands
        c.command("LAUNCHPAD").unwrap();
        assert_eq!(c.app.layout_mode, LayoutMode::Quad);
        c.command("LAYOUT FULL").unwrap();
        assert_eq!(c.app.layout_mode, LayoutMode::FullWidth);
        assert!(c.app.full_width);
        c.command("LAYOUT STANDARD").unwrap();
        assert_eq!(c.app.layout_mode, LayoutMode::Standard);
        assert!(!c.app.full_width);

        // Test DES command
        c.command("DES AAPL").unwrap();
        assert!(c.app.des_mode);
        assert_eq!(c.app.workspace, Workspace::Fin);
    }
}
