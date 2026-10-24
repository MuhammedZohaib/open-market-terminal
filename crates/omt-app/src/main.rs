#![forbid(unsafe_code)]
mod controller;
use anyhow::{Context, Result};
use clap::Parser;
use controller::{Controller, ResearchRequest, subscription};
use crossterm::{
    cursor::Show,
    event::{self, Event, EventStream, KeyEventKind, MouseEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures_util::StreamExt;
use omt_core::{
    AssetId, Candle, Company, DataStatus, InstrumentInfo, MacroObservation, NewsCategory, NewsItem,
    Quote, Timeframe, command::Workspace,
};
use omt_providers::{
    Http, market,
    research::{self, Sec},
};
use omt_storage::Store;
use omt_tui::App;
use serde::Deserialize;
use std::{
    path::PathBuf,
    time::{Duration, Instant},
};
use tokio::sync::{mpsc, watch};
#[derive(Parser)]
#[command(version, about = "Local-first financial research terminal")]
struct Args {
    /// Asset symbol or quoted command, e.g. 'CHART BTCUSDT 1H'
    symbol: Option<String>,
    #[arg(long)]
    offline: bool,
    #[arg(long)]
    config: Option<PathBuf>,
    #[arg(long)]
    data_dir: Option<PathBuf>,
    /// Render a real headless 140x40 terminal frame and exit
    #[arg(long)]
    snapshot: bool,
    /// Seconds to wait for actual providers before a snapshot
    #[arg(long, default_value_t = 0)]
    wait_seconds: u64,
    /// Verify public Binance history and an actual WebSocket ticker
    #[arg(long)]
    probe: bool,
}
#[derive(Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Config {
    name: String,
    ascii: bool,
    sec_user_agent: Option<String>,
}
impl Default for Config {
    fn default() -> Self {
        Self {
            name: "Open Market Terminal".into(),
            ascii: false,
            sec_user_agent: None,
        }
    }
}
fn load_config(path: &std::path::Path, explicit: bool) -> Result<Config> {
    if path.exists() {
        Ok(toml::from_str(&std::fs::read_to_string(path)?)?)
    } else if explicit {
        anyhow::bail!("Configuration path does not exist")
    } else {
        Ok(Config::default())
    }
}
struct TerminalGuard;
fn restore() {
    let _ = disable_raw_mode();
    let _ = execute!(
        std::io::stdout(),
        LeaveAlternateScreen,
        Show,
        event::DisableMouseCapture
    );
}
impl Drop for TerminalGuard {
    fn drop(&mut self) {
        restore();
    }
}
enum ResearchResult {
    Company(Result<Company>, String),
    Macro(Result<Vec<MacroObservation>>, String),
    Catalog(Vec<String>),
    News(Result<Vec<NewsItem>>, NewsCategory, Option<String>),
    Article(String, Result<String>),
    OlderCandles(Result<Vec<Candle>>, AssetId, Timeframe),
}
async fn researcher(
    http: Http,
    sec: Option<Sec>,
    sec_user_agent: Option<String>,
    mut requests: mpsc::Receiver<ResearchRequest>,
    results: mpsc::Sender<ResearchResult>,
) {
    let mut mapping = None;
    while let Some(request) = requests.recv().await {
        let result = match request {
            ResearchRequest::Company(ticker) => {
                let result = async {
                    let sec = sec.as_ref().context(
                        "Set OMT_SEC_USER_AGENT to an app name and maintained email before SEC requests",
                    )?;
                    if mapping.is_none() {
                        mapping = Some(sec.tickers().await?);
                    }
                    sec.company(&ticker, mapping.as_ref().context("SEC mapping unavailable")?)
                        .await
                }
                .await;
                ResearchResult::Company(result, ticker)
            }
            ResearchRequest::Treasury => {
                ResearchResult::Macro(research::treasury(&http).await, "TREASURY".into())
            }
            ResearchRequest::WorldBank(country, series) => {
                let key = format!("WB {country} {series}");
                ResearchResult::Macro(research::world_bank(&http, &country, &series).await, key)
            }
            ResearchRequest::Fred(series) => {
                let key = format!("FRED {series}");
                ResearchResult::Macro(research::fred(&http, &series).await, key)
            }
            ResearchRequest::News(category, ticker) => {
                let res = research::fetch_news(&http, ticker.as_deref(), category).await;
                ResearchResult::News(res, category, ticker)
            }
            ResearchRequest::Article(item) => {
                let id = item.id.clone();
                let res = research::fetch_article_or_filing_body(
                    &http,
                    sec_user_agent.as_deref(),
                    &item.url,
                    item.is_sec_filing,
                    item.form.as_deref(),
                    &item.summary,
                )
                .await;
                ResearchResult::Article(id, res)
            }
            ResearchRequest::OlderHistory(asset, timeframe, before_open_time) => {
                let res =
                    market::history_before(&http, &asset, timeframe, Some(before_open_time)).await;
                ResearchResult::OlderCandles(res, asset, timeframe)
            }
        };
        if results.send(result).await.is_err() {
            return;
        }
    }
}
fn apply_research(
    c: &mut Controller,
    event: ResearchResult,
    request_tx: &mpsc::Sender<ResearchRequest>,
) -> Result<()> {
    match event {
        ResearchResult::Company(result, ticker) => match result {
            Ok(company) => {
                let filing_news = omt_core::company_filings_to_news(&company);
                let mut current = std::mem::take(&mut c.app.news_items);
                current.extend(filing_news);
                c.app.news_items = research::dedup_news(current);
                for item in &mut c.app.news_items {
                    if item.body.is_none()
                        && let Ok(Some((body, _))) = c.store.get::<String>("articles", &item.id)
                    {
                        item.body = Some(body);
                    }
                }
                let _ = c.store.put("news", "recent", &c.app.news_items);
                c.store.put("company", &ticker, &company)?;
                c.app.companies = c.store.companies()?;
                if c.app.asset.symbol == ticker {
                    c.app.company = Some(company);
                    c.app.research_status = "SEC company facts and filings loaded".into();
                }
            }
            Err(e) => {
                c.app.research_status = format!("{ticker}: {e}");
            }
        },
        ResearchResult::Macro(result, key) => match result {
            Ok(rows) => {
                c.store.put("macro", &key, &rows)?;
                if c.macro_key == key {
                    c.app.macro_data = rows;
                    c.app.research_status = format!(
                        "{key} / fetched {} UTC",
                        chrono::Utc::now().format("%Y-%m-%d %H:%M")
                    );
                }
            }
            Err(e) => {
                if c.macro_key == key {
                    c.app.research_status = e.to_string();
                }
            }
        },
        ResearchResult::Catalog(symbols) => c.app.catalog.extend(symbols),
        ResearchResult::News(result, category, _ticker) => match result {
            Ok(incoming) => {
                let mut current = std::mem::take(&mut c.app.news_items);
                current.extend(incoming);
                c.app.news_items = research::dedup_news(current);
                c.app.news_updated_at = Some(chrono::Utc::now());
                for item in &mut c.app.news_items {
                    if item.body.is_none()
                        && let Ok(Some((body, _))) = c.store.get::<String>("articles", &item.id)
                    {
                        item.body = Some(body);
                    }
                }
                let _ = c.store.put("news", "recent", &c.app.news_items);
                c.app.research_status = format!(
                    "News updated ({}) {} items",
                    category.label(),
                    c.app.news_items.len()
                );
                prefetch_news_articles(c, request_tx, 6);
            }
            Err(e) => {
                c.app.research_status = format!("News fetch error: {e}");
            }
        },
        ResearchResult::Article(id, result) => match result {
            Ok(body) => {
                let _ = c.store.put("articles", &id, &body);
                if let Some(item) = c.app.news_items.iter_mut().find(|n| n.id == id) {
                    item.body = Some(body);
                }
                let _ = c.store.put("news", "recent", &c.app.news_items);
            }
            Err(e) => {
                c.app.message = format!("Article body fetch failed: {e}");
            }
        },
        ResearchResult::OlderCandles(result, asset, timeframe) => match result {
            Ok(older) => {
                if c.app.asset == asset && c.app.timeframe == timeframe {
                    let count = older.len();
                    c.app.market.merge_candles(older);
                    c.app.message = format!("Loaded {count} older historical candles");
                }
            }
            Err(e) => {
                c.app.message = format!("Older candles fetch error: {e}");
            }
        },
    }
    Ok(())
}

fn prefetch_news_articles(
    c: &mut Controller,
    request_tx: &mpsc::Sender<ResearchRequest>,
    limit: usize,
) {
    if c.offline {
        return;
    }
    let items = omt_tui::views::filtered_news_items(&c.app);
    if items.is_empty() {
        return;
    }
    let sel_idx = c.app.cursor.min(items.len().saturating_sub(1));
    let mut to_prefetch = Vec::new();
    for offset in 0..limit {
        let idx = sel_idx + offset;
        if let Some(item) = items.get(idx) {
            to_prefetch.push(item.clone());
        }
    }
    for item in to_prefetch {
        if item.body.is_none() && !item.url.is_empty() {
            if let Ok(Some((cached_body, _))) = c.store.get::<String>("articles", &item.id) {
                if let Some(real_item) = c.app.news_items.iter_mut().find(|n| n.id == item.id) {
                    real_item.body = Some(cached_body);
                }
            } else {
                let _ = request_tx.try_send(ResearchRequest::Article(item));
            }
        }
    }
}
fn apply_market(app: &mut App, mut market: omt_data::MarketState) {
    let key = format!("{}:{}", app.asset.key(), app.timeframe);
    if market.chart_key != key || market.candles.is_empty() {
        market.candles = app.market.candles.clone();
        market.candles_received_at = app.market.candles_received_at;
        market.chart_key = key;
    }
    let had_selection = app.selected_market_key.is_some();
    app.market = market;
    if !had_selection && (!app.market.quotes.is_empty() || !app.market.instruments.is_empty()) {
        app.sync_market_selection();
    }
}
fn save(c: &Controller) -> Result<()> {
    for asset in &c.app.watchlist {
        if let Some(q) = c.app.market.quotes.get(&asset.key()) {
            c.store.put_at("quote", &q.asset.key(), q, q.received_at)?;
        }
    }
    if !c.app.market.instruments.is_empty() {
        let list: Vec<&InstrumentInfo> = c.app.market.instruments.values().collect();
        c.store.put("universe", "binance_instruments", &list)?;
    }
    if !c.app.market.quotes.is_empty() {
        let mut snap: Vec<&Quote> = c.app.market.quotes.values().collect();
        snap.sort_by_key(|a| std::cmp::Reverse(a.quote_volume));
        snap.truncate(500);
        c.store.put("universe", "binance_top_quotes", &snap)?;
    }
    if !c.app.market.candles.is_empty()
        && let Some(at) = c.app.market.candles_received_at
    {
        c.store.put_at(
            "candles",
            &c.app.market.chart_key,
            &c.app.market.candles,
            at,
        )?;
    }
    if !c.app.news_items.is_empty() {
        let mut save_news = c.app.news_items.clone();
        save_news.truncate(200);
        c.store.put("news", "recent", &save_news)?;
    }
    c.store.set("watchlist", &c.app.list_name)?;
    c.store.set("asset", &c.app.asset.key())?;
    c.store.set("quote_filter", &c.app.quote_filter)?;
    c.store.set("sort", &c.app.sort)?;
    c.store.set(
        "full_width",
        if c.app.full_width { "true" } else { "false" },
    )?;
    Ok(())
}
#[tokio::main]
async fn main() -> Result<()> {
    let started = Instant::now();
    let args = Args::parse();
    let dirs = directories::ProjectDirs::from("org", "OpenMarketTerminal", "omt")
        .context("Cannot resolve user directories")?;
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| dirs.config_dir().join("config.toml"));
    let config = load_config(&config_path, args.config.is_some())?;
    let data = args
        .data_dir
        .unwrap_or_else(|| dirs.data_local_dir().into());
    std::fs::create_dir_all(&data)?;
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(data.join("omt.log"))?;
    tracing_subscriber::fmt()
        .with_writer(log)
        .with_ansi(false)
        .with_env_filter("omt_app=info,omt_providers=info")
        .init();
    let http = Http::new()?;
    if args.probe {
        if args.offline {
            anyhow::bail!("--probe requires network access; remove --offline");
        }
        println!("{}", market::probe(&http).await?);
        return Ok(());
    }
    let store = Store::open(&data.join("omt.sqlite3"))?;
    if store.watchlists()?.is_empty() {
        for symbol in ["BTCUSDT", "ETHUSDT", "SOLUSDT"] {
            store.watch(
                "default",
                &symbol.parse().map_err(anyhow::Error::msg)?,
                true,
            )?;
        }
    }
    let list = store
        .setting("watchlist")?
        .unwrap_or_else(|| "default".into());
    let mut app = App {
        name: config.name,
        ascii: config.ascii,
        list_name: list.clone(),
        watchlist: store.watchlist(&list)?,
        transactions: store.transactions()?,
        companies: store.companies()?,
        ..Default::default()
    };
    if let Some((cached_news, _)) = store.get::<Vec<NewsItem>>("news", "recent")? {
        app.news_items = cached_news;
    }
    for comp in &app.companies {
        let f_news = omt_core::company_filings_to_news(comp);
        app.news_items.extend(f_news);
    }
    app.news_items = research::dedup_news(app.news_items);
    for item in &mut app.news_items {
        if item.body.is_none()
            && let Ok(Some((body, _))) = store.get::<String>("articles", &item.id)
        {
            item.body = Some(body);
        }
    }
    if let Some(asset) = store.setting("asset")? {
        app.asset = asset.parse().map_err(anyhow::Error::msg)?;
    }
    if let Some(qf) = store.setting("quote_filter")? {
        app.quote_filter = qf;
    }
    if let Some(sort) = store.setting("sort")? {
        app.sort = sort;
    }
    if let Some(fw) = store.setting("full_width")? {
        app.full_width = fw == "true";
    }
    if let Some((instruments, _)) =
        store.get::<Vec<InstrumentInfo>>("universe", "binance_instruments")?
    {
        app.market.register_instruments(instruments);
    }
    if let Some((quotes, _)) = store.get::<Vec<Quote>>("universe", "binance_top_quotes")? {
        for mut q in quotes {
            q.status = DataStatus::Historical;
            app.market.quotes.insert(q.asset.key(), q);
        }
    }
    for asset in &app.watchlist {
        if let Some((mut q, _)) = store.get::<Quote>("quote", &asset.key())? {
            q.status = DataStatus::Historical;
            app.market.quotes.insert(asset.key(), q);
        }
    }
    app.sync_market_selection();
    app.market.status = if args.offline {
        "Offline / cached only"
    } else {
        "Connecting"
    }
    .into();
    let key = format!("{}:{}", app.asset.key(), app.timeframe);
    if let Some((bars, at)) = store.get::<Vec<Candle>>("candles", &key)? {
        let is_stale = chrono::Utc::now().signed_duration_since(at).num_seconds()
            > app.timeframe.seconds() * 2;
        app.market.chart_status = if is_stale {
            omt_core::ChartDataStatus::Stale
        } else {
            omt_core::ChartDataStatus::Ready
        };
        app.market.candles = bars;
        app.market.candles_received_at = Some(at);
        app.message = format!("Cached candles fetched {at}");
    }
    app.market.chart_key = key;
    app.account = store
        .setting("account")?
        .unwrap_or_else(|| "default".into());
    let mut c = Controller {
        app,
        store,
        offline: args.offline,
        macro_key: String::new(),
    };
    let initial = if let Some(symbol) = args.symbol {
        c.command(&symbol)?
    } else {
        None
    };
    let (subscription_tx, subscription_rx) = watch::channel(subscription(&c.app));
    let (market_tx, mut market_rx) = watch::channel(c.app.market.clone());
    let (request_tx, request_rx) = mpsc::channel(64);
    let (result_tx, mut result_rx) = mpsc::channel(64);
    let mut background = tokio::task::JoinSet::new();
    if !args.offline {
        background.spawn(market::service(http.clone(), subscription_rx, market_tx));
        let ua = std::env::var("OMT_SEC_USER_AGENT")
            .ok()
            .or(config.sec_user_agent);
        let sec = ua.as_deref().and_then(|s| Sec::new(http.clone(), s).ok());
        background.spawn(researcher(
            http.clone(),
            sec,
            ua,
            request_rx,
            result_tx.clone(),
        ));
        let h = http.clone();
        let tx = result_tx.clone();
        background.spawn(async move {
            if let Ok(catalog) = market::catalog(&h).await {
                let _ = tx.send(ResearchResult::Catalog(catalog)).await;
            }
        });
        let h_news = http.clone();
        let tx_news = result_tx.clone();
        background.spawn(async move {
            let res = research::fetch_news(&h_news, None, NewsCategory::All).await;
            let _ = tx_news
                .send(ResearchResult::News(res, NewsCategory::All, None))
                .await;
        });
        if let Some(request) = initial {
            request_tx.send(request).await?;
        }
    }
    if args.snapshot {
        let stop = tokio::time::sleep(Duration::from_secs(args.wait_seconds.min(120)));
        tokio::pin!(stop);
        loop {
            tokio::select! {_=&mut stop=>break,changed=market_rx.changed(),if !args.offline=>{if changed.is_ok(){apply_market(&mut c.app,market_rx.borrow_and_update().clone());}},Some(result)=result_rx.recv()=>{apply_research(&mut c,result,&request_tx)?;}}
        }
        let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(140, 40))?;
        terminal.draw(|f| omt_tui::render(f, &c.app))?;
        for row in terminal.backend().buffer().content.chunks(140) {
            println!(
                "{}",
                row.iter().map(|cell| cell.symbol()).collect::<String>()
            );
        }
        save(&c)?;
        return Ok(());
    }
    let original = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore();
        original(info);
    }));
    enable_raw_mode()?;
    let _guard = TerminalGuard;
    execute!(
        std::io::stdout(),
        EnterAlternateScreen,
        event::EnableMouseCapture
    )?;
    let mut terminal =
        ratatui::Terminal::new(ratatui::backend::CrosstermBackend::new(std::io::stdout()))?;
    let mut events = EventStream::new();
    let mut frame_tick = tokio::time::interval(Duration::from_millis(33));
    let mut seconds = tokio::time::interval(Duration::from_secs(1));
    let mut persistence = tokio::time::interval(Duration::from_secs(15));
    let mut dirty = true;
    let mut first = true;
    while !c.app.quit {
        tokio::select! {
        _=frame_tick.tick()=>{if dirty{let before=Instant::now();terminal.draw(|f|omt_tui::render(f,&c.app))?;let draw_us=before.elapsed().as_micros() as u64;c.app.record_draw_micros(draw_us);if first{tracing::info!(startup_ms=started.elapsed().as_millis(),"first terminal draw");first=false;}tracing::debug!(draw_us,"draw");dirty=false;}},
        _=seconds.tick()=>dirty=true,
        _=persistence.tick()=>{if let Err(e)=save(&c){c.app.message=format!("Persistence: {e}");}},
        changed=market_rx.changed(),if !args.offline=>{if changed.is_ok(){apply_market(&mut c.app,market_rx.borrow_and_update().clone());dirty=true;}},
        Some(result)=result_rx.recv()=>{if let Err(e)=apply_research(&mut c,result,&request_tx){c.app.message=e.to_string();}dirty=true;},
        event=events.next()=>{match event{Some(Ok(Event::Key(key))) if key.kind==KeyEventKind::Press=>{
            if let Some(text)=c.app.key(key){
                match c.command(&text){
                    Ok(request)=>{
                        let new=subscription(&c.app);
                        if *subscription_tx.borrow()!=new{
                            subscription_tx.send_replace(new);
                        }
                        if let Some(request)=request&& !args.offline&&request_tx.try_send(request).is_err(){
                            c.app.research_status="Research queue full; retry shortly".into();
                        }
                    },
                    Err(e)=>c.app.message=e.to_string()
                }
            }
            if c.app.workspace == Workspace::News {
                prefetch_news_articles(&mut c, &request_tx, 3);
            }
            dirty=true;
        },Some(Ok(Event::Resize(..)))=>dirty=true,Some(Ok(Event::Mouse(m)))=>{match m.kind{MouseEventKind::ScrollDown=>{c.app.cursor=c.app.cursor.saturating_add(1);if c.app.workspace==Workspace::News{prefetch_news_articles(&mut c,&request_tx,3);}},MouseEventKind::ScrollUp=>{c.app.cursor=c.app.cursor.saturating_sub(1);if c.app.workspace==Workspace::News{prefetch_news_articles(&mut c,&request_tx,3);}},_=>{}}dirty=true;},Some(Err(e))=>return Err(e.into()),None=>break,_=>{}}}
        }
    }
    save(&c)?;
    background.abort_all();
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuration_rejects_unknown_keys() {
        assert!(toml::from_str::<Config>("mystery = true").is_err());
        assert!(toml::from_str::<Config>("ascii = true").unwrap().ascii);
    }
    #[test]
    fn absent_explicit_config_fails() {
        assert!(load_config(std::path::Path::new("this-file-does-not-exist.toml"), true).is_err());
    }
}
