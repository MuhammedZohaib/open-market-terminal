#![forbid(unsafe_code)]
use omt_core::{AssetId, Timeframe, portfolio::Transaction};
use omt_service::{
    ChartDataDto, CommandOutcomeDto, CompanyFactsDto, InstrumentSearchItemDto,
    MarketEngine, MarketSnapshotDto, NewsItemDto, PortfolioSummaryDto,
    TransactionDto, TreasuryCurveDto,
};
use std::sync::Arc;
use tauri::State;

pub struct AppState {
    pub engine: Arc<MarketEngine>,
}

#[tauri::command]
pub fn get_market_snapshot(state: State<'_, AppState>) -> Result<MarketSnapshotDto, String> {
    Ok(state.engine.market_snapshot())
}

#[tauri::command]
pub fn get_historical_candles(
    state: State<'_, AppState>,
    asset_key: String,
    timeframe: String,
) -> Result<ChartDataDto, String> {
    let asset: AssetId = asset_key
        .parse()
        .map_err(|e: String| format!("Invalid asset key: {}", e))?;
    let tf: Timeframe = timeframe
        .parse()
        .map_err(|e: String| format!("Invalid timeframe: {}", e))?;
    state
        .engine
        .historical_candles(&asset, tf)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn search_instruments(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<InstrumentSearchItemDto>, String> {
    Ok(state.engine.search_instruments(&query))
}

#[tauri::command]
pub fn get_company_facts(
    state: State<'_, AppState>,
    ticker: String,
) -> Result<Option<CompanyFactsDto>, String> {
    state.engine.company_facts(&ticker).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_news(
    state: State<'_, AppState>,
    category: Option<String>,
    ticker: Option<String>,
) -> Result<Vec<NewsItemDto>, String> {
    let cat = category.and_then(|c| match c.to_ascii_uppercase().as_str() {
        "CRYPTO" => Some(omt_core::NewsCategory::Crypto),
        "MACRO" => Some(omt_core::NewsCategory::Macro),
        "MARKETS" => Some(omt_core::NewsCategory::Markets),
        "EQUITIES" => Some(omt_core::NewsCategory::Equities),
        "EARNINGS" => Some(omt_core::NewsCategory::Earnings),
        "FILINGS" | "SEC" => Some(omt_core::NewsCategory::Filings),
        _ => Some(omt_core::NewsCategory::All),
    });
    Ok(state.engine.news(cat, ticker.as_deref()))
}

#[tauri::command]
pub fn get_article(state: State<'_, AppState>, id: String) -> Result<Option<String>, String> {
    state.engine.article(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_treasury_curve(
    state: State<'_, AppState>,
) -> Result<Option<TreasuryCurveDto>, String> {
    state.engine.treasury_curve().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_portfolio(
    state: State<'_, AppState>,
    account: String,
) -> Result<PortfolioSummaryDto, String> {
    state.engine.portfolio(&account).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn add_transaction(
    state: State<'_, AppState>,
    transaction: TransactionDto,
) -> Result<(), String> {
    let asset: AssetId = transaction
        .asset_key
        .parse()
        .map_err(|e: String| format!("Invalid asset key: {}", e))?;
    let side = if transaction.side.eq_ignore_ascii_case("SELL") {
        omt_core::portfolio::Side::Sell
    } else {
        omt_core::portfolio::Side::Buy
    };
    let quantity: rust_decimal::Decimal = transaction
        .quantity
        .parse()
        .map_err(|e| format!("Invalid quantity: {}", e))?;
    let price: rust_decimal::Decimal = transaction
        .price
        .parse()
        .map_err(|e| format!("Invalid price: {}", e))?;
    let fee: rust_decimal::Decimal = transaction
        .fee
        .parse()
        .unwrap_or(rust_decimal::Decimal::ZERO);
    let at = chrono::DateTime::parse_from_rfc3339(&transaction.at)
        .map(|dt| dt.with_timezone(&chrono::Utc))
        .unwrap_or_else(|_| chrono::Utc::now());

    let tx = Transaction {
        id: transaction.id,
        account: transaction.account,
        asset,
        side,
        quantity,
        price,
        fee,
        currency: transaction.currency,
        at,
    };

    state.engine.add_transaction(tx).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn delete_transaction(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine.delete_transaction(&id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn execute_terminal_command(
    state: State<'_, AppState>,
    command: String,
) -> Result<CommandOutcomeDto, String> {
    state.engine.execute_command(&command).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_watchlists(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let s = state.engine.store.lock().map_err(|_| "Store lock failed")?;
    s.watchlists().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_watchlist(state: State<'_, AppState>, name: String) -> Result<Vec<String>, String> {
    let s = state.engine.store.lock().map_err(|_| "Store lock failed")?;
    let assets = s.watchlist(&name).map_err(|e| e.to_string())?;
    Ok(assets.into_iter().map(|a| a.key()).collect())
}

#[tauri::command]
pub fn watch_asset(
    state: State<'_, AppState>,
    name: String,
    asset_key: String,
    add: bool,
) -> Result<(), String> {
    let asset: AssetId = asset_key
        .parse()
        .map_err(|e: String| format!("Invalid asset key: {}", e))?;
    let s = state.engine.store.lock().map_err(|_| "Store lock failed")?;
    s.watch(&name, &asset, add).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> Result<Option<String>, String> {
    let s = state.engine.store.lock().map_err(|_| "Store lock failed")?;
    s.setting(&key).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn save_setting(
    state: State<'_, AppState>,
    key: String,
    value: String,
) -> Result<(), String> {
    let s = state.engine.store.lock().map_err(|_| "Store lock failed")?;
    s.set(&key, &value).map_err(|e| e.to_string())
}
