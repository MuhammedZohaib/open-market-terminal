#![forbid(unsafe_code)]
use omt_core::{Candle, NewsItem, Quote, portfolio::Transaction};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketQuoteDto {
    pub key: String,
    pub symbol: String,
    pub venue: String,
    pub asset_class: String,
    pub currency: String,
    pub base_asset: Option<String>,
    pub price: String,
    pub change_24h: Option<String>,
    pub change_percent_24h: Option<f64>,
    pub high_24h: Option<String>,
    pub low_24h: Option<String>,
    pub volume_24h: Option<String>,
    pub quote_volume_24h: Option<String>,
    pub bid: Option<String>,
    pub ask: Option<String>,
    pub spread: Option<String>,
    pub spread_bps: Option<f64>,
    pub vwap: Option<String>,
    pub status: String,
    pub event_at: String,
    pub received_at: String,
}

impl MarketQuoteDto {
    pub fn from_quote(q: &Quote) -> Self {
        let spread = q.spread().map(|s| s.to_string());
        let spread_bps = q.spread_bps().and_then(|bps| bps.to_string().parse().ok());

        Self {
            key: q.asset.key(),
            symbol: q.asset.symbol.clone(),
            venue: q.asset.venue.to_string(),
            asset_class: format!("{:?}", q.asset.asset_class),
            currency: q.currency.clone(),
            base_asset: q.base_asset.clone(),
            price: q.price.to_string(),
            change_24h: q.effective_change_abs().map(|c| c.to_string()),
            change_percent_24h: q
                .effective_change_pct()
                .and_then(|d| d.to_string().parse().ok()),
            high_24h: q.high.map(|h| h.to_string()),
            low_24h: q.low.map(|l| l.to_string()),
            volume_24h: q.volume.map(|v| v.to_string()),
            quote_volume_24h: q.quote_volume.map(|qv| qv.to_string()),
            bid: q.bid.map(|b| b.to_string()),
            ask: q.ask.map(|a| a.to_string()),
            spread,
            spread_bps,
            vwap: q.vwap.map(|w| w.to_string()),
            status: format!("{:?}", q.status),
            event_at: q.event_at.to_rfc3339(),
            received_at: q.received_at.to_rfc3339(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MarketSnapshotDto {
    pub quotes: Vec<MarketQuoteDto>,
    pub total_instruments: usize,
    pub active_instruments: usize,
    pub top_gainers: Vec<MarketQuoteDto>,
    pub top_losers: Vec<MarketQuoteDto>,
    pub most_active: Vec<MarketQuoteDto>,
    pub connection_status: String,
    pub last_snapshot_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CandleDto {
    pub time: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
    pub is_closed: bool,
}

impl CandleDto {
    pub fn from_candle(c: &Candle) -> Self {
        Self {
            time: c.open_at.timestamp(),
            open: c.open.to_string().parse().unwrap_or(0.0),
            high: c.high.to_string().parse().unwrap_or(0.0),
            low: c.low.to_string().parse().unwrap_or(0.0),
            close: c.close.to_string().parse().unwrap_or(0.0),
            volume: c.volume.to_string().parse().unwrap_or(0.0),
            is_closed: c.closed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndicatorsDto {
    pub sma20: Option<f64>,
    pub sma50: Option<f64>,
    pub ema12: Option<f64>,
    pub ema26: Option<f64>,
    pub rsi14: Option<f64>,
    pub macd: Option<f64>,
    pub macd_signal: Option<f64>,
    pub macd_hist: Option<f64>,
    pub atr14: Option<f64>,
    pub bb_upper: Option<f64>,
    pub bb_middle: Option<f64>,
    pub bb_lower: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartDataDto {
    pub asset_key: String,
    pub symbol: String,
    pub venue: String,
    pub timeframe: String,
    pub candles: Vec<CandleDto>,
    pub indicators: Vec<IndicatorsDto>,
    pub status: String,
    pub error_message: Option<String>,
    pub received_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstrumentSearchItemDto {
    pub key: String,
    pub symbol: String,
    pub venue: String,
    pub asset_class: String,
    pub quote_currency: String,
    pub base_asset: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewsItemDto {
    pub id: String,
    pub headline: String,
    pub source: String,
    pub url: String,
    pub published_at: String,
    pub category: String,
    pub ticker: Option<String>,
    pub summary: String,
    pub body: Option<String>,
    pub is_sec_filing: bool,
    pub form: Option<String>,
}

impl NewsItemDto {
    pub fn from_core(n: &NewsItem) -> Self {
        Self {
            id: n.id.clone(),
            headline: n.headline.clone(),
            source: n.source.clone(),
            url: n.url.clone(),
            published_at: n.published_at.to_rfc3339(),
            category: n.category.label().to_string(),
            ticker: n.ticker.clone(),
            summary: n.summary.clone(),
            body: n.body.clone(),
            is_sec_filing: n.is_sec_filing,
            form: n.form.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementRowDto {
    pub concept: String,
    pub label: String,
    pub values: Vec<Option<String>>,
    pub is_header: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatementMatrixDto {
    pub periods: Vec<String>,
    pub rows: Vec<StatementRowDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecFilingDto {
    pub form: String,
    pub filed: String,
    pub report_date: Option<String>,
    pub accession: String,
    pub description: String,
    pub document: String,
    pub url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompanyFactsDto {
    pub ticker: String,
    pub cik: String,
    pub name: String,
    pub facts_count: usize,
    pub income_statement_annual: StatementMatrixDto,
    pub income_statement_quarterly: StatementMatrixDto,
    pub balance_sheet_annual: StatementMatrixDto,
    pub balance_sheet_quarterly: StatementMatrixDto,
    pub cash_flow_annual: StatementMatrixDto,
    pub cash_flow_quarterly: StatementMatrixDto,
    pub ratios_annual: StatementMatrixDto,
    pub ratios_quarterly: StatementMatrixDto,
    pub recent_filings: Vec<SecFilingDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoldingDto {
    pub asset_key: String,
    pub symbol: String,
    pub venue: String,
    pub quantity: String,
    pub average_cost: String,
    pub cost_basis: String,
    pub current_price: Option<String>,
    pub market_value: Option<String>,
    pub unrealized_pnl: Option<String>,
    pub unrealized_pnl_percent: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransactionDto {
    pub id: String,
    pub account: String,
    pub asset_key: String,
    pub symbol: String,
    pub side: String,
    pub quantity: String,
    pub price: String,
    pub fee: String,
    pub currency: String,
    pub at: String,
    pub total_amount: String,
}

impl TransactionDto {
    pub fn from_transaction(t: &Transaction) -> Self {
        let total = (t.quantity * t.price) + t.fee;
        Self {
            id: t.id.clone(),
            account: t.account.clone(),
            asset_key: t.asset.key(),
            symbol: t.asset.symbol.clone(),
            side: format!("{:?}", t.side).to_ascii_uppercase(),
            quantity: t.quantity.to_string(),
            price: t.price.to_string(),
            fee: t.fee.to_string(),
            currency: t.currency.clone(),
            at: t.at.to_rfc3339(),
            total_amount: total.to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortfolioSummaryDto {
    pub account: String,
    pub total_cost_basis: String,
    pub total_market_value: String,
    pub total_unrealized_pnl: String,
    pub total_unrealized_pnl_percent: Option<f64>,
    pub holdings: Vec<HoldingDto>,
    pub transactions: Vec<TransactionDto>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryTenorDto {
    pub tenor: String,
    pub yield_value: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreasuryCurveDto {
    pub observation_date: String,
    pub tenors: Vec<TreasuryTenorDto>,
    pub spread_2y_10y: Option<f64>,
    pub spread_3m_10y: Option<f64>,
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandOutcomeDto {
    pub success: bool,
    pub message: String,
    pub workspace_target: Option<String>,
    pub asset_target: Option<String>,
    pub timeframe_target: Option<String>,
    pub statement_tab: Option<String>,
    pub quote_filter: Option<String>,
    pub alert_message: Option<String>,
}
