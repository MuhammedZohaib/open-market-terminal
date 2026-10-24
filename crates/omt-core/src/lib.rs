#![forbid(unsafe_code)]
pub mod alerts;
pub mod command;
pub mod facts;
pub mod impact;
pub mod portfolio;
use chrono::{DateTime, NaiveDate, Utc};
pub use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AssetClass {
    Equity,
    Etf,
    Crypto,
    Forex,
    Index,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub enum Venue {
    Binance,
    Coinbase,
    Iex,
}
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId {
    pub symbol: String,
    pub asset_class: AssetClass,
    pub venue: Venue,
}
pub const BINANCE_QUOTES: &[&str] = &[
    "FDUSD", "USDT", "USDC", "TUSD", "BUSD", "USDP", "EURI", "AEUR", "BIDR", "IDRT", "BVND",
    "DOGE", "BTC", "ETH", "BNB", "SOL", "XRP", "TRX", "DOT", "DAI", "PAX", "VAI", "EUR", "TRY",
    "BRL", "JPY", "GBP", "AUD", "CAD", "CHF", "PLN", "RON", "ARS", "COP", "MXN", "CZK", "ZAR",
    "UAH", "IDR", "RUB", "NGN", "INR", "HKD", "SGD", "NZD",
];
impl AssetId {
    pub fn key(&self) -> String {
        format!("{}:{}", self.venue, self.symbol)
    }
    pub fn currency(&self) -> &str {
        match self.venue {
            Venue::Iex => "USD",
            Venue::Coinbase => self.symbol.rsplit('-').next().unwrap_or("UNKNOWN"),
            Venue::Binance => BINANCE_QUOTES
                .iter()
                .copied()
                .find(|s| self.symbol.len() > s.len() && self.symbol.ends_with(s))
                .unwrap_or("UNKNOWN"),
        }
    }
    pub fn base_asset(&self) -> &str {
        match self.venue {
            Venue::Iex => &self.symbol,
            Venue::Coinbase => self
                .symbol
                .split_once('-')
                .map(|(b, _)| b)
                .unwrap_or(&self.symbol),
            Venue::Binance => {
                let q = self.currency();
                if q != "UNKNOWN" && self.symbol.ends_with(q) {
                    &self.symbol[..self.symbol.len() - q.len()]
                } else {
                    &self.symbol
                }
            }
        }
    }
}
impl fmt::Display for Venue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Binance => "BINANCE",
            Self::Coinbase => "COINBASE",
            Self::Iex => "IEX",
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum LinkGroup {
    #[default]
    A,
    B,
    None,
}

impl LinkGroup {
    pub const ALL: [Self; 3] = [Self::A, Self::B, Self::None];
    pub fn label(self) -> &'static str {
        match self {
            Self::A => "LG: A",
            Self::B => "LG: B",
            Self::None => "LG: --",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::None,
            Self::None => Self::A,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentId {
    pub canonical_id: String,
    pub symbol: String,
    pub name: Option<String>,
    pub asset_class: AssetClass,
    pub venue: Option<String>,
    pub currency: Option<String>,
    pub base_asset: Option<String>,
    pub quote_asset: Option<String>,
    pub cik: Option<String>,
    pub provider_ids: std::collections::HashMap<String, String>,
}

impl InstrumentId {
    pub fn from_asset(asset: &AssetId) -> Self {
        let mut provider_ids = std::collections::HashMap::new();
        provider_ids.insert(asset.venue.to_string(), asset.symbol.clone());
        Self {
            canonical_id: format!("{}:{}", asset.venue, asset.symbol),
            symbol: asset.symbol.clone(),
            name: None,
            asset_class: asset.asset_class,
            venue: Some(asset.venue.to_string()),
            currency: Some(asset.currency().to_string()),
            base_asset: Some(asset.base_asset().to_string()),
            quote_asset: if asset.asset_class == AssetClass::Crypto {
                Some(asset.currency().to_string())
            } else {
                None
            },
            cik: None,
            provider_ids,
        }
    }

    pub fn to_asset(&self) -> Result<AssetId, String> {
        self.canonical_id.parse()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentContext {
    pub global: AssetId,
    pub local_selection: Option<String>,
    pub link_group: LinkGroup,
    pub recent: Vec<AssetId>,
}

impl InstrumentContext {
    pub fn new(global: AssetId) -> Self {
        Self {
            recent: vec![global.clone()],
            global,
            local_selection: None,
            link_group: LinkGroup::A,
        }
    }

    pub fn set_global(&mut self, asset: AssetId) {
        self.recent.retain(|a| a != &asset);
        self.recent.insert(0, asset.clone());
        if self.recent.len() > 16 {
            self.recent.truncate(16);
        }
        self.global = asset;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub struct ProviderCapabilities {
    pub quotes: bool,
    pub streaming: bool,
    pub historical_bars: bool,
    pub fundamentals: bool,
    pub filings: bool,
    pub news: bool,
    pub macro_series: bool,
    pub yield_curve: bool,
}

impl ProviderCapabilities {
    pub fn binance_spot() -> Self {
        Self {
            quotes: true,
            streaming: true,
            historical_bars: true,
            fundamentals: false,
            filings: false,
            news: false,
            macro_series: false,
            yield_curve: false,
        }
    }

    pub fn sec_edgar() -> Self {
        Self {
            quotes: false,
            streaming: false,
            historical_bars: false,
            fundamentals: true,
            filings: true,
            news: false,
            macro_series: false,
            yield_curve: false,
        }
    }

    pub fn alpaca_iex() -> Self {
        Self {
            quotes: true,
            streaming: true,
            historical_bars: true,
            fundamentals: false,
            filings: false,
            news: false,
            macro_series: false,
            yield_curve: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum WorksheetColumn {
    Symbol,
    Last,
    ChangePct,
    ChangeAbs,
    Volume,
    QuoteVolume,
    Bid,
    Ask,
    SpreadBps,
    PeRatio,
    MarketCap,
    NewsCount,
}

impl WorksheetColumn {
    pub fn label(self) -> &'static str {
        match self {
            Self::Symbol => "SYMBOL",
            Self::Last => "LAST",
            Self::ChangePct => "CHG%",
            Self::ChangeAbs => "CHG",
            Self::Volume => "VOLUME",
            Self::QuoteVolume => "TURNOVER",
            Self::Bid => "BID",
            Self::Ask => "ASK",
            Self::SpreadBps => "SPREAD(bps)",
            Self::PeRatio => "P/E",
            Self::MarketCap => "MCAP",
            Self::NewsCount => "NEWS",
        }
    }

    pub fn default_weight(self) -> u32 {
        match self {
            Self::Symbol => 14,
            Self::Last => 12,
            Self::ChangePct => 10,
            Self::ChangeAbs => 10,
            Self::Volume => 11,
            Self::QuoteVolume => 11,
            Self::Bid => 10,
            Self::Ask => 10,
            Self::SpreadBps => 9,
            Self::PeRatio => 8,
            Self::MarketCap => 10,
            Self::NewsCount => 8,
        }
    }

    pub fn parse_column(token: &str) -> Option<Self> {
        let t = token.to_ascii_uppercase();
        match t.as_str() {
            "SYM" | "SYMBOL" | "TICKER" => Some(Self::Symbol),
            "LAST" | "PRICE" => Some(Self::Last),
            "CHG%" | "CHANGE%" | "CHGPCT" | "PCT" => Some(Self::ChangePct),
            "CHG" | "CHANGE" | "NET" => Some(Self::ChangeAbs),
            "VOL" | "VOLUME" => Some(Self::Volume),
            "QVOL" | "QUOTE_VOL" | "TURNOVER" | "QUOTE" => Some(Self::QuoteVolume),
            "BID" => Some(Self::Bid),
            "ASK" => Some(Self::Ask),
            "SPREAD" | "SPREADBPS" | "BPS" => Some(Self::SpreadBps),
            "PE" | "P/E" | "PERATIO" => Some(Self::PeRatio),
            "MCAP" | "MARKETCAP" | "CAP" => Some(Self::MarketCap),
            "NEWS" | "HEADLINES" | "NEWSCOUNT" => Some(Self::NewsCount),
            _ => None,
        }
    }

    pub fn default_columns() -> Vec<Self> {
        vec![
            Self::Symbol,
            Self::Last,
            Self::ChangePct,
            Self::Volume,
            Self::Bid,
            Self::Ask,
            Self::SpreadBps,
            Self::NewsCount,
        ]
    }

    pub fn all() -> &'static [Self] {
        &[
            Self::Symbol,
            Self::Last,
            Self::ChangePct,
            Self::ChangeAbs,
            Self::Volume,
            Self::QuoteVolume,
            Self::Bid,
            Self::Ask,
            Self::SpreadBps,
            Self::PeRatio,
            Self::MarketCap,
            Self::NewsCount,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Worksheet {
    pub name: String,
    pub columns: Vec<WorksheetColumn>,
    pub symbols: Vec<AssetId>,
}

impl Worksheet {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            columns: WorksheetColumn::default_columns(),
            symbols: Vec::new(),
        }
    }

    pub fn add(&mut self, asset: AssetId) -> bool {
        if self.symbols.iter().any(|a| a == &asset) {
            false
        } else {
            self.symbols.push(asset);
            true
        }
    }

    pub fn remove(&mut self, asset: &AssetId) -> bool {
        if let Some(pos) = self.symbols.iter().position(|a| a == asset) {
            self.symbols.remove(pos);
            true
        } else {
            false
        }
    }
}

impl Default for Worksheet {
    fn default() -> Self {
        Self::new("DEFAULT")
    }
}
impl FromStr for AssetId {
    type Err = String;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let value = value.trim().to_ascii_uppercase();
        let (venue, symbol) = if let Some((v, s)) = value.split_once(':') {
            (
                match v {
                    "BINANCE" => Venue::Binance,
                    "COINBASE" => Venue::Coinbase,
                    "IEX" => Venue::Iex,
                    _ => return Err("Venue must be BINANCE, COINBASE or IEX".into()),
                },
                s,
            )
        } else if value.contains('-') {
            (Venue::Coinbase, value.as_str())
        } else if ["USDT", "USDC", "FDUSD", "TUSD", "BUSD", "EURI"]
            .iter()
            .any(|q| value.len() > q.len() && value.ends_with(q))
            || (value.len() >= 6
                && ["BTC", "ETH", "BNB", "EUR", "TRY", "BRL", "JPY"]
                    .iter()
                    .any(|q| value.ends_with(q)))
        {
            (Venue::Binance, value.as_str())
        } else {
            (Venue::Iex, value.as_str())
        };
        if symbol.is_empty()
            || symbol.len() > 24
            || !symbol
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        {
            return Err("Invalid asset symbol".into());
        }
        Ok(Self {
            symbol: symbol.into(),
            asset_class: if venue == Venue::Iex {
                AssetClass::Equity
            } else {
                AssetClass::Crypto
            },
            venue,
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum TradingStatus {
    #[default]
    Trading,
    Halt,
    Break,
    PreTrading,
    PostTrading,
    EndOfDay,
    AuctionMatch,
    Unknown,
}
impl TradingStatus {
    pub fn from_binance(s: &str) -> Self {
        match s.to_ascii_uppercase().as_str() {
            "TRADING" => Self::Trading,
            "HALT" => Self::Halt,
            "BREAK" => Self::Break,
            "PRE_TRADING" => Self::PreTrading,
            "POST_TRADING" => Self::PostTrading,
            "END_OF_DAY" => Self::EndOfDay,
            "AUCTION_MATCH" => Self::AuctionMatch,
            _ => Self::Unknown,
        }
    }
}
impl fmt::Display for TradingStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Trading => "TRADING",
            Self::Halt => "HALT",
            Self::Break => "BREAK",
            Self::PreTrading => "PRE",
            Self::PostTrading => "POST",
            Self::EndOfDay => "EOD",
            Self::AuctionMatch => "AUCTION",
            Self::Unknown => "UNKNOWN",
        })
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentInfo {
    pub asset: AssetId,
    pub base_asset: String,
    pub quote_asset: String,
    pub status: TradingStatus,
    pub price_precision: u32,
    pub quantity_precision: u32,
    pub provider: String,
    pub coverage: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DataStatus {
    Live,
    Delayed,
    Stale,
    Disconnected,
    Historical,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChartDataStatus {
    #[default]
    NotRequested,
    Loading,
    Ready,
    Stale,
    Empty,
    Unsupported,
    AuthenticationRequired,
    RateLimited,
    Error,
}
impl ChartDataStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::NotRequested => "NOT REQUESTED",
            Self::Loading => "LOADING",
            Self::Ready => "READY",
            Self::Stale => "STALE CACHE",
            Self::Empty => "EMPTY",
            Self::Unsupported => "UNSUPPORTED",
            Self::AuthenticationRequired => "AUTH REQUIRED",
            Self::RateLimited => "RATE LIMITED",
            Self::Error => "ERROR",
        }
    }
    pub fn is_actionable_error(self) -> bool {
        matches!(
            self,
            Self::Unsupported | Self::AuthenticationRequired | Self::RateLimited | Self::Error
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ProviderStatus {
    #[default]
    Live,
    Delayed,
    Cached,
    Stale,
    Loading,
    Unavailable,
    AuthRequired,
    Error,
}
impl ProviderStatus {
    pub fn label(self) -> &'static str {
        match self {
            Self::Live => "LIVE",
            Self::Delayed => "DELAYED",
            Self::Cached => "CACHED",
            Self::Stale => "STALE",
            Self::Loading => "LOADING",
            Self::Unavailable => "UNAVAILABLE",
            Self::AuthRequired => "AUTH REQ",
            Self::Error => "ERROR",
        }
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Quote {
    pub asset: AssetId,
    pub price: Decimal,
    pub currency: String,
    pub change_pct: Option<Decimal>,
    #[serde(default)]
    pub change_abs: Option<Decimal>,
    #[serde(default)]
    pub open_24h: Option<Decimal>,
    pub high: Option<Decimal>,
    pub low: Option<Decimal>,
    pub volume: Option<Decimal>,
    #[serde(default)]
    pub quote_volume: Option<Decimal>,
    #[serde(default)]
    pub bid: Option<Decimal>,
    #[serde(default)]
    pub ask: Option<Decimal>,
    #[serde(default)]
    pub vwap: Option<Decimal>,
    #[serde(default)]
    pub base_asset: Option<String>,
    pub provider: String,
    pub coverage: String,
    pub event_at: DateTime<Utc>,
    pub received_at: DateTime<Utc>,
    pub status: DataStatus,
    pub delay_seconds: Option<u64>,
}
impl Quote {
    pub fn validate(&self) -> Result<(), String> {
        if self.price <= Decimal::ZERO {
            return Err("Nonpositive quote".into());
        }
        let inferred = self.asset.currency();
        if self.currency.is_empty() || (inferred != "UNKNOWN" && self.currency != inferred) {
            return Err("Quote currency mismatch".into());
        }
        if self.event_at > self.received_at + chrono::Duration::seconds(60) {
            return Err("Upstream timestamp is in the future".into());
        }
        if let (Some(bid), Some(ask)) = (self.bid, self.ask)
            && (bid <= Decimal::ZERO || ask <= Decimal::ZERO || ask < bid)
        {
            return Err("Invalid bid/ask quote bounds".into());
        }
        Ok(())
    }
    pub fn status_at(&self, now: DateTime<Utc>) -> DataStatus {
        let threshold = match self.asset.venue {
            Venue::Iex => 120,
            Venue::Binance => 360,
            Venue::Coinbase => 60,
        };
        if matches!(
            self.status,
            DataStatus::Historical | DataStatus::Disconnected
        ) {
            self.status
        } else if now
            .signed_duration_since(self.received_at.max(self.event_at))
            .num_seconds()
            > threshold
        {
            DataStatus::Stale
        } else {
            self.status
        }
    }
    pub fn base_symbol(&self) -> &str {
        self.base_asset
            .as_deref()
            .unwrap_or_else(|| self.asset.base_asset())
    }
    pub fn effective_change_abs(&self) -> Option<Decimal> {
        self.change_abs.or_else(|| {
            self.open_24h
                .filter(|o| *o > Decimal::ZERO)
                .map(|o| self.price - o)
        })
    }
    pub fn effective_change_pct(&self) -> Option<Decimal> {
        self.change_pct.or_else(|| {
            self.open_24h
                .filter(|o| *o > Decimal::ZERO)
                .map(|o| (self.price - o) / o * Decimal::from(100))
        })
    }
    pub fn spread(&self) -> Option<Decimal> {
        let (bid, ask) = (self.bid?, self.ask?);
        (bid > Decimal::ZERO && ask >= bid).then_some(ask - bid)
    }
    pub fn spread_bps(&self) -> Option<Decimal> {
        let (bid, ask) = (self.bid?, self.ask?);
        if bid <= Decimal::ZERO || ask < bid {
            return None;
        }
        let mid = (bid + ask) / Decimal::from(2);
        (mid > Decimal::ZERO).then(|| (ask - bid) / mid * Decimal::from(10_000))
    }
    pub fn range_pct(&self) -> Option<Decimal> {
        let (low, high) = (self.low?, self.high?);
        if high <= low || self.price < low || self.price > high {
            return None;
        }
        Some((self.price - low) / (high - low) * Decimal::from(100))
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Timeframe {
    M1,
    M5,
    M15,
    H1,
    H4,
    D1,
}
impl Timeframe {
    pub const ALL: [Self; 6] = [Self::M1, Self::M5, Self::M15, Self::H1, Self::H4, Self::D1];
    pub fn seconds(self) -> i64 {
        match self {
            Self::M1 => 60,
            Self::M5 => 300,
            Self::M15 => 900,
            Self::H1 => 3600,
            Self::H4 => 14400,
            Self::D1 => 86400,
        }
    }
}
impl fmt::Display for Timeframe {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::M1 => "1m",
            Self::M5 => "5m",
            Self::M15 => "15m",
            Self::H1 => "1h",
            Self::H4 => "4h",
            Self::D1 => "1d",
        })
    }
}
impl FromStr for Timeframe {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|t| t.to_string() == s.to_ascii_lowercase())
            .ok_or_else(|| "Timeframe: 1m 5m 15m 1h 4h 1d".into())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Candle {
    pub open_at: DateTime<Utc>,
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub volume: Decimal,
    pub closed: bool,
}
impl Candle {
    pub fn validate(&self) -> bool {
        self.low <= self.open
            && self.low <= self.close
            && self.high >= self.open
            && self.high >= self.close
            && self.high >= self.low
            && self.volume >= Decimal::ZERO
    }
}
pub fn sma(bars: &[Candle], window: usize) -> Vec<Option<Decimal>> {
    if window == 0 {
        return vec![None; bars.len()];
    }
    let mut sum = Decimal::ZERO;
    bars.iter()
        .enumerate()
        .map(|(i, b)| {
            sum += b.close;
            if i >= window {
                sum -= bars[i - window].close;
            }
            if i + 1 >= window {
                Some(sum / Decimal::from(window))
            } else {
                None
            }
        })
        .collect()
}
pub fn ema(bars: &[Candle], window: usize) -> Vec<Option<Decimal>> {
    if window == 0 || bars.is_empty() {
        return vec![None; bars.len()];
    }
    let k = Decimal::from(2) / Decimal::from(window + 1);
    let mut sum = Decimal::ZERO;
    let mut current: Option<Decimal> = None;
    bars.iter()
        .enumerate()
        .map(|(i, b)| {
            if i + 1 < window {
                sum += b.close;
                None
            } else if i + 1 == window {
                sum += b.close;
                let seed = sum / Decimal::from(window);
                current = Some(seed);
                Some(seed)
            } else {
                let prev = current.unwrap_or(b.close);
                let next = (b.close - prev) * k + prev;
                current = Some(next);
                Some(next)
            }
        })
        .collect()
}
pub fn bollinger_bands(
    bars: &[Candle],
    window: usize,
    mult: u32,
) -> Vec<Option<(Decimal, Decimal, Decimal)>> {
    use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
    if window == 0 {
        return vec![None; bars.len()];
    }
    let means = sma(bars, window);
    bars.iter()
        .enumerate()
        .map(|(i, _)| {
            let mid = means[i]?;
            let slice = &bars[i + 1 - window..=i];
            let mid_f = mid.to_f64()?;
            let var = slice
                .iter()
                .filter_map(|b| b.close.to_f64())
                .map(|c| {
                    let d = c - mid_f;
                    d * d
                })
                .sum::<f64>()
                / window as f64;
            let sd = Decimal::from_f64(var.sqrt() * mult as f64)?;
            Some((mid - sd, mid, mid + sd))
        })
        .collect()
}
pub fn rsi(bars: &[Candle], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 || bars.len() <= period {
        return vec![None; bars.len()];
    }
    let mut out = vec![None; bars.len()];
    let mut avg_gain = Decimal::ZERO;
    let mut avg_loss = Decimal::ZERO;
    for i in 1..=period {
        let diff = bars[i].close - bars[i - 1].close;
        if diff > Decimal::ZERO {
            avg_gain += diff;
        } else {
            avg_loss += -diff;
        }
    }
    let p = Decimal::from(period);
    avg_gain /= p;
    avg_loss /= p;
    let calc_rsi = |g: Decimal, l: Decimal| -> Decimal {
        if l == Decimal::ZERO {
            if g == Decimal::ZERO {
                Decimal::from(50)
            } else {
                Decimal::from(100)
            }
        } else {
            let rs = g / l;
            Decimal::from(100) - (Decimal::from(100) / (Decimal::ONE + rs))
        }
    };
    out[period] = Some(calc_rsi(avg_gain, avg_loss));
    let p_minus_1 = Decimal::from(period - 1);
    for i in (period + 1)..bars.len() {
        let diff = bars[i].close - bars[i - 1].close;
        let (gain, loss) = if diff > Decimal::ZERO {
            (diff, Decimal::ZERO)
        } else {
            (Decimal::ZERO, -diff)
        };
        avg_gain = (avg_gain * p_minus_1 + gain) / p;
        avg_loss = (avg_loss * p_minus_1 + loss) / p;
        out[i] = Some(calc_rsi(avg_gain, avg_loss));
    }
    out
}
pub fn macd(
    bars: &[Candle],
    fast: usize,
    slow: usize,
    signal: usize,
) -> Vec<Option<(Decimal, Decimal, Decimal)>> {
    if fast == 0 || slow == 0 || signal == 0 || fast >= slow || bars.is_empty() {
        return vec![None; bars.len()];
    }
    let fast_ema = ema(bars, fast);
    let slow_ema = ema(bars, slow);
    let mut macd_line = vec![None; bars.len()];
    for i in 0..bars.len() {
        if let (Some(f), Some(s)) = (fast_ema[i], slow_ema[i]) {
            macd_line[i] = Some(f - s);
        }
    }
    let k = Decimal::from(2) / Decimal::from(signal + 1);
    let mut sum = Decimal::ZERO;
    let mut count = 0usize;
    let mut sig_val: Option<Decimal> = None;
    macd_line
        .into_iter()
        .map(|m| {
            let m = m?;
            count += 1;
            if count < signal {
                sum += m;
                None
            } else if count == signal {
                sum += m;
                let s = sum / Decimal::from(signal);
                sig_val = Some(s);
                Some((m, s, m - s))
            } else {
                let prev = sig_val.unwrap_or(m);
                let s = (m - prev) * k + prev;
                sig_val = Some(s);
                Some((m, s, m - s))
            }
        })
        .collect()
}
pub fn atr(bars: &[Candle], period: usize) -> Vec<Option<Decimal>> {
    if period == 0 || bars.is_empty() {
        return vec![None; bars.len()];
    }
    let mut trs = Vec::with_capacity(bars.len());
    for (i, b) in bars.iter().enumerate() {
        let hl = b.high - b.low;
        let tr = if i == 0 {
            hl
        } else {
            let prev_c = bars[i - 1].close;
            let hc = (b.high - prev_c).abs();
            let lc = (b.low - prev_c).abs();
            hl.max(hc).max(lc)
        };
        trs.push(tr);
    }
    let mut out = vec![None; bars.len()];
    if bars.len() < period {
        return out;
    }
    let p = Decimal::from(period);
    let p_minus_1 = Decimal::from(period - 1);
    let mut cur: Decimal = trs[..period].iter().copied().sum::<Decimal>() / p;
    out[period - 1] = Some(cur);
    for i in period..bars.len() {
        cur = (cur * p_minus_1 + trs[i]) / p;
        out[i] = Some(cur);
    }
    out
}
pub fn vwap(bars: &[Candle]) -> Option<Decimal> {
    let mut pv = Decimal::ZERO;
    let mut vol = Decimal::ZERO;
    for b in bars {
        let typical = (b.high + b.low + b.close) / Decimal::from(3);
        pv += typical * b.volume;
        vol += b.volume;
    }
    (vol > Decimal::ZERO).then(|| pv / vol)
}
#[derive(Debug, Clone)]
pub struct WindowStats {
    pub open: Decimal,
    pub high: Decimal,
    pub low: Decimal,
    pub close: Decimal,
    pub return_pct: Option<Decimal>,
    pub volume: Decimal,
    pub vwap: Option<Decimal>,
    pub max_drawdown_pct: Decimal,
    pub volatility_pct: Option<Decimal>,
}
pub fn window_stats(bars: &[Candle]) -> Option<WindowStats> {
    use rust_decimal::prelude::{FromPrimitive, ToPrimitive};
    let first = bars.first()?;
    let last = bars.last()?;
    let mut high = first.high;
    let mut low = first.low;
    let mut volume = Decimal::ZERO;
    let mut peak = first.close;
    let mut max_dd = Decimal::ZERO;
    for b in bars {
        if b.high > high {
            high = b.high;
        }
        if b.low < low {
            low = b.low;
        }
        volume += b.volume;
        if b.close > peak {
            peak = b.close;
        }
        if peak > Decimal::ZERO {
            let dd = (peak - b.close) / peak * Decimal::from(100);
            if dd > max_dd {
                max_dd = dd;
            }
        }
    }
    let return_pct = (first.open > Decimal::ZERO)
        .then(|| (last.close - first.open) / first.open * Decimal::from(100));
    let mut rets = Vec::new();
    for pair in bars.windows(2) {
        if pair[0].close > Decimal::ZERO
            && let Some(r) =
                ((pair[1].close - pair[0].close) / pair[0].close * Decimal::from(100)).to_f64()
        {
            rets.push(r);
        }
    }
    let volatility_pct = if rets.len() >= 2 {
        let mean = rets.iter().sum::<f64>() / rets.len() as f64;
        let var = rets
            .iter()
            .map(|r| {
                let d = *r - mean;
                d * d
            })
            .sum::<f64>()
            / (rets.len() - 1) as f64;
        Decimal::from_f64(var.sqrt())
    } else {
        None
    };
    Some(WindowStats {
        open: first.open,
        high,
        low,
        close: last.close,
        return_pct,
        volume,
        vwap: vwap(bars),
        max_drawdown_pct: max_dd,
        volatility_pct,
    })
}
#[derive(Debug, Clone)]
pub struct YieldCurveAnalytics {
    pub spread_2s10s_bps: Option<Decimal>,
    pub spread_3m10s_bps: Option<Decimal>,
    pub spread_5s30s_bps: Option<Decimal>,
    pub min_yield: Option<(String, Decimal)>,
    pub max_yield: Option<(String, Decimal)>,
    pub regime: &'static str,
}
pub fn analyze_yield_curve(obs: &[MacroObservation]) -> Option<YieldCurveAnalytics> {
    if obs.is_empty() || obs[0].source != "US Treasury" {
        return None;
    }
    let find = |s: &str| obs.iter().find(|r| r.series == s).map(|r| r.value);
    let y3m = find("BC_3MONTH");
    let y2y = find("BC_2YEAR");
    let y5y = find("BC_5YEAR");
    let y10y = find("BC_10YEAR");
    let y30y = find("BC_30YEAR");
    let bps = Decimal::from(100);
    let spread_2s10s_bps = y10y.zip(y2y).map(|(a, b)| (a - b) * bps);
    let spread_3m10s_bps = y10y.zip(y3m).map(|(a, b)| (a - b) * bps);
    let spread_5s30s_bps = y30y.zip(y5y).map(|(a, b)| (a - b) * bps);
    let min_yield = obs
        .iter()
        .min_by_key(|r| r.value)
        .map(|r| (r.label.clone(), r.value));
    let max_yield = obs
        .iter()
        .max_by_key(|r| r.value)
        .map(|r| (r.label.clone(), r.value));
    let regime = match spread_2s10s_bps {
        Some(s) if s < Decimal::from(-5) => "Inverted (2s10s < 0)",
        Some(s) if s <= Decimal::from(10) => "Flat / Transition",
        Some(_) => "Normal (Upward Sloping)",
        None => "Partial Curve",
    };
    Some(YieldCurveAnalytics {
        spread_2s10s_bps,
        spread_3m10s_bps,
        spread_5s30s_bps,
        min_yield,
        max_yield,
        regime,
    })
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PeriodView {
    Annual,
    Quarterly,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinancialFact {
    pub metric: String,
    pub concept: String,
    pub value: Decimal,
    pub unit: String,
    pub start: Option<NaiveDate>,
    pub end: NaiveDate,
    pub filed: NaiveDate,
    pub accession: String,
    pub form: String,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Company {
    pub ticker: String,
    pub cik: u64,
    pub name: String,
    pub facts: Vec<FinancialFact>,
    pub filings: Vec<Filing>,
    pub fetched_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filing {
    pub form: String,
    pub filed: NaiveDate,
    #[serde(default)]
    pub report_date: Option<NaiveDate>,
    #[serde(default)]
    pub description: String,
    pub accession: String,
    pub document: String,
    pub url: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum NewsCategory {
    #[default]
    All,
    Markets,
    Equities,
    Crypto,
    Macro,
    Earnings,
    Filings,
}
impl NewsCategory {
    pub const ALL: [Self; 7] = [
        Self::All,
        Self::Markets,
        Self::Equities,
        Self::Crypto,
        Self::Macro,
        Self::Earnings,
        Self::Filings,
    ];
    pub fn label(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Markets => "MARKETS",
            Self::Equities => "EQUITIES",
            Self::Crypto => "CRYPTO",
            Self::Macro => "MACRO",
            Self::Earnings => "EARNINGS",
            Self::Filings => "SEC FILINGS",
        }
    }
    pub fn code(self) -> &'static str {
        match self {
            Self::All => "ALL",
            Self::Markets => "MKT",
            Self::Equities => "EQTY",
            Self::Crypto => "CRYP",
            Self::Macro => "MACR",
            Self::Earnings => "EARN",
            Self::Filings => "SEC",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::All => Self::Markets,
            Self::Markets => Self::Equities,
            Self::Equities => Self::Crypto,
            Self::Crypto => Self::Macro,
            Self::Macro => Self::Earnings,
            Self::Earnings => Self::Filings,
            Self::Filings => Self::All,
        }
    }
    pub fn from_token(token: &str) -> Option<Self> {
        match token.trim().to_ascii_uppercase().as_str() {
            "ALL" | "TOP" => Some(Self::All),
            "MARKETS" | "MARKET" | "MKT" => Some(Self::Markets),
            "EQUITIES" | "EQUITY" | "EQTY" | "STOCKS" => Some(Self::Equities),
            "CRYPTO" | "CRYP" | "BTC" => Some(Self::Crypto),
            "MACRO" | "MACR" | "ECON" | "RATES" | "FED" => Some(Self::Macro),
            "EARNINGS" | "EARN" => Some(Self::Earnings),
            "FILINGS" | "FILING" | "SEC" | "EDGAR" => Some(Self::Filings),
            _ => None,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NewsItem {
    pub id: String,
    pub published_at: DateTime<Utc>,
    pub fetched_at: DateTime<Utc>,
    pub source: String,
    pub ticker: Option<String>,
    pub category: NewsCategory,
    pub headline: String,
    pub summary: String,
    #[serde(default)]
    pub body: Option<String>,
    pub url: String,
    #[serde(default)]
    pub is_sec_filing: bool,
    #[serde(default)]
    pub form: Option<String>,
    #[serde(default)]
    pub accession: Option<String>,
    #[serde(default)]
    pub report_date: Option<NaiveDate>,
}
pub fn company_filings_to_news(company: &Company) -> Vec<NewsItem> {
    company
        .filings
        .iter()
        .map(|f| {
            let published_at = f
                .filed
                .and_hms_opt(16, 0, 0)
                .map(|dt| dt.and_utc())
                .unwrap_or(company.fetched_at);
            let period_note = f
                .report_date
                .map(|d| format!(" (Report Period: {d})"))
                .unwrap_or_default();
            let headline = format!(
                "{} — {}{}",
                f.form, f.description, period_note
            );
            let summary = format!(
                "Issuer: {} ({}) | CIK {:010}\nForm: {} — {}\nFiling Date: {}{}\nAccession: {}\nPrimary Document: {}\n\nSource: Official SEC EDGAR Regulatory Filing (not independent journalism).\nSelect this filing or press 'r' to inspect extracted disclosure text, or press 'o' to open in browser.",
                company.name,
                company.ticker,
                company.cik,
                f.form,
                f.description,
                f.filed,
                f.report_date
                    .map(|d| format!(" | Report Period: {d}"))
                    .unwrap_or_default(),
                f.accession,
                f.document
            );
            NewsItem {
                id: format!("sec:{}:{}", company.ticker, f.accession),
                published_at,
                fetched_at: company.fetched_at,
                source: "SEC EDGAR".into(),
                ticker: Some(company.ticker.clone()),
                category: NewsCategory::Filings,
                headline,
                summary,
                body: None,
                url: f.url.clone(),
                is_sec_filing: true,
                form: Some(f.form.clone()),
                accession: Some(f.accession.clone()),
                report_date: f.report_date,
            }
        })
        .collect()
}
pub fn decode_html_entities(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(amp) = rest.find('&') {
        out.push_str(&rest[..amp]);
        let after = &rest[amp + 1..];
        if let Some(semi) = after.find(';') {
            let entity = &after[..semi];
            if entity.len() <= 10 {
                let replacement = match entity {
                    "amp" => Some("&"),
                    "lt" => Some("<"),
                    "gt" => Some(">"),
                    "quot" => Some("\""),
                    "apos" | "#39" | "#039" | "#x27" => Some("'"),
                    "nbsp" | "#160" | "#xa0" => Some(" "),
                    "mdash" | "#8212" | "#x2014" => Some("—"),
                    "ndash" | "#8211" | "#x2013" => Some("–"),
                    "rsquo" | "lsquo" | "#8217" | "#8216" => Some("'"),
                    "ldquo" | "rdquo" | "#8220" | "#8221" => Some("\""),
                    "hellip" | "#8230" => Some("..."),
                    "bull" | "#8226" => Some("•"),
                    _ if entity.starts_with('#') => {
                        let code = if let Some(hex) = entity
                            .strip_prefix("#x")
                            .or_else(|| entity.strip_prefix("#X"))
                        {
                            u32::from_str_radix(hex, 16).ok()
                        } else {
                            entity[1..].parse::<u32>().ok()
                        };
                        if let Some(ch) = code.and_then(char::from_u32) {
                            if !ch.is_control() || ch == '\n' {
                                out.push(ch);
                            } else {
                                out.push(' ');
                            }
                            rest = &after[semi + 1..];
                            continue;
                        }
                        None
                    }
                    _ => None,
                };
                if let Some(rep) = replacement {
                    out.push_str(rep);
                    rest = &after[semi + 1..];
                    continue;
                }
            }
        }
        out.push('&');
        rest = after;
    }
    out.push_str(rest);
    out
}
pub fn sanitize_terminal_text(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\x1b' {
            if let Some(&next) = chars.peek() {
                if next == '[' {
                    chars.next();
                    for c in chars.by_ref() {
                        if c.is_ascii_alphabetic() || c == '~' {
                            break;
                        }
                    }
                    continue;
                } else if next == ']' {
                    chars.next();
                    let mut prev = ' ';
                    for c in chars.by_ref() {
                        if c == '\x07' || (prev == '\x1b' && c == '\\') {
                            break;
                        }
                        prev = c;
                    }
                    continue;
                }
            }
            continue;
        }
        if ch.is_control() && ch != '\n' && ch != '\r' && ch != '\t' {
            continue;
        }
        out.push(ch);
    }
    out
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExtractedArticle {
    pub title: Option<String>,
    pub author: Option<String>,
    pub published_time: Option<String>,
    pub canonical_url: Option<String>,
    pub body: String,
    pub is_excerpt: bool,
}

pub fn extract_article_content(raw_html: &str, url: &str) -> ExtractedArticle {
    extract_article_content_with_headline(raw_html, url, None)
}

fn find_matching_title_offset(raw_html: &str, candidates: &[&str]) -> Option<usize> {
    let lower = raw_html.to_ascii_lowercase();

    // 1. Look for <h1> tags and check for match against candidate titles
    let mut search_from = 0;
    while let Some(rel_start) = lower[search_from..].find("<h1") {
        let h1_start = search_from + rel_start;
        if let Some(tag_close_rel) = lower[h1_start..].find('>') {
            let inner_start = h1_start + tag_close_rel + 1;
            if let Some(h1_end_rel) = lower[inner_start..].find("</h1>") {
                let inner_end = inner_start + h1_end_rel;
                let full_h1_end = inner_end + 5;
                let h1_text = clean_html_to_text(&raw_html[inner_start..inner_end]);
                let h1_trimmed = h1_text.trim();
                let h1_lower = h1_trimmed.to_ascii_lowercase();

                for cand in candidates {
                    let cand_trim = cand.trim();
                    if cand_trim.is_empty() {
                        continue;
                    }
                    let cand_lower = cand_trim.to_ascii_lowercase();
                    if h1_lower == cand_lower
                        || h1_lower.contains(&cand_lower)
                        || cand_lower.contains(&h1_lower)
                    {
                        return Some(full_h1_end);
                    }
                    let cand_words: Vec<&str> = cand_lower
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|w| w.len() >= 4)
                        .collect();
                    let h1_words: Vec<&str> = h1_lower
                        .split(|c: char| !c.is_alphanumeric())
                        .filter(|w| w.len() >= 4)
                        .collect();
                    if !cand_words.is_empty() {
                        let matched_count =
                            cand_words.iter().filter(|w| h1_words.contains(w)).count();
                        if matched_count >= 3
                            || (matched_count * 2 >= cand_words.len() && matched_count >= 2)
                        {
                            return Some(full_h1_end);
                        }
                    }
                }

                // If candidate did not match directly, but this is a substantial H1 headline (>= 15 chars, >= 3 words)
                if h1_lower.len() >= 15
                    && h1_lower.split_whitespace().count() >= 3
                    && !h1_lower.contains("404")
                    && !h1_lower.contains("not found")
                    && !h1_lower.contains("sign in")
                    && !h1_lower.contains("cookie")
                    && !h1_lower.contains("subscribe")
                {
                    return Some(full_h1_end);
                }

                search_from = full_h1_end;
                continue;
            }
        }
        search_from = h1_start + 3;
    }

    // 2. If no <h1> matched, search for candidate title text directly in HTML
    for cand in candidates {
        let cand_trim = cand.trim();
        if cand_trim.len() < 10 {
            continue;
        }
        let cand_lower = cand_trim.to_ascii_lowercase();
        let words: Vec<&str> = cand_lower.split_whitespace().collect();
        let query = if words.len() >= 4 {
            words[..4].join(" ")
        } else {
            cand_lower.clone()
        };
        if let Some(pos) = lower.find(&query) {
            if let Some(tag_end) = lower[pos..].find('>') {
                return Some(pos + tag_end + 1);
            }
            return Some(pos + query.len());
        }
    }

    None
}

pub fn extract_article_content_with_headline(
    raw_html: &str,
    url: &str,
    expected_headline: Option<&str>,
) -> ExtractedArticle {
    let lower = raw_html.to_ascii_lowercase();

    let extract_meta = |key_attr: &str, key_val: &str| -> Option<String> {
        let pat1 = format!("{key_attr}=\"{key_val}\"");
        let pat2 = format!("{key_attr}='{key_val}'");
        let pos = lower.find(&pat1).or_else(|| lower.find(&pat2))?;
        let tag_start = raw_html[..pos].rfind('<')?;
        let tag_end_rel = raw_html[pos..].find('>')?;
        let meta_tag = &raw_html[tag_start..pos + tag_end_rel];
        let meta_lower = meta_tag.to_ascii_lowercase();
        for content_pat in ["content=\"", "content='"] {
            if let Some(c_pos) = meta_lower.find(content_pat) {
                let start = c_pos + content_pat.len();
                let quote = content_pat.chars().last().unwrap();
                if let Some(c_end) = meta_tag[start..].find(quote) {
                    let val = decode_html_entities(meta_tag[start..start + c_end].trim());
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
        None
    };

    let extract_link_rel = |rel_val: &str| -> Option<String> {
        let pat1 = format!("rel=\"{rel_val}\"");
        let pat2 = format!("rel='{rel_val}'");
        let pos = lower.find(&pat1).or_else(|| lower.find(&pat2))?;
        let tag_start = raw_html[..pos].rfind('<')?;
        let tag_end_rel = raw_html[pos..].find('>')?;
        let tag = &raw_html[tag_start..pos + tag_end_rel];
        let tag_lower = tag.to_ascii_lowercase();
        for href_pat in ["href=\"", "href='"] {
            if let Some(h_pos) = tag_lower.find(href_pat) {
                let start = h_pos + href_pat.len();
                let quote = href_pat.chars().last().unwrap();
                if let Some(h_end) = tag[start..].find(quote) {
                    let val = decode_html_entities(tag[start..start + h_end].trim());
                    if !val.is_empty() {
                        return Some(val);
                    }
                }
            }
        }
        None
    };

    let extract_time_datetime = || -> Option<String> {
        if let Some(pos) = lower.find("datetime=\"") {
            let start = pos + 10;
            if let Some(end) = raw_html[start..].find('"') {
                return Some(decode_html_entities(raw_html[start..start + end].trim()));
            }
        }
        None
    };

    let meta_title = extract_meta("property", "og:title")
        .or_else(|| extract_meta("name", "twitter:title"))
        .or_else(|| extract_meta("name", "title"))
        .or_else(|| {
            if let Some(t_start) = lower.find("<title>")
                && let Some(t_end) = lower[t_start..].find("</title>")
            {
                let t = decode_html_entities(&raw_html[t_start + 7..t_start + t_end]);
                return Some(t.split('|').next().unwrap_or(&t).trim().to_string());
            }
            None
        });

    let title = expected_headline
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .or(meta_title);

    let author = extract_meta("name", "author")
        .or_else(|| extract_meta("property", "article:author"))
        .or_else(|| extract_meta("name", "byl"));

    let published_time = extract_meta("property", "article:published_time")
        .or_else(|| extract_meta("name", "pubdate"))
        .or_else(extract_time_datetime);

    let canonical_url =
        extract_link_rel("canonical").or_else(|| extract_meta("property", "og:url"));

    // Find the matching title / headline in document and discard everything above it
    let candidate_titles: Vec<&str> = [expected_headline, title.as_deref()]
        .into_iter()
        .flatten()
        .collect();
    let title_anchor = find_matching_title_offset(raw_html, &candidate_titles);

    let slice = if let Some(anchor) = title_anchor {
        // Discard all pre-headline header, navigation, and marquee markup
        let after_title = &raw_html[anchor..];
        let lower_after = &lower[anchor..];
        if let Some(a_end) = lower_after.find("</article>") {
            &after_title[..a_end]
        } else if let Some(m_end) = lower_after.find("</main>") {
            &after_title[..m_end]
        } else {
            after_title
        }
    } else if let Some(a_start) = lower.find("<article") {
        if let Some(a_end) = lower[a_start..].find("</article>") {
            &raw_html[a_start..a_start + a_end + 10]
        } else {
            &raw_html[a_start..]
        }
    } else if let Some(m_start) = lower.find("<main") {
        if let Some(m_end) = lower[m_start..].find("</main>") {
            &raw_html[m_start..m_start + m_end + 7]
        } else {
            &raw_html[m_start..]
        }
    } else if let Some(d_start) = lower
        .find("class=\"article-body")
        .or_else(|| lower.find("class=\"story-content"))
        .or_else(|| lower.find("class=\"post-content"))
        .or_else(|| lower.find("id=\"article-body"))
    {
        let tag_start = raw_html[..d_start].rfind('<').unwrap_or(d_start);
        &raw_html[tag_start..]
    } else {
        raw_html
    };

    let cleaned = clean_html_to_text(slice);

    let is_end_of_article_marker = |line: &str| -> bool {
        let l = line.to_ascii_lowercase();
        let trimmed = l.trim();
        let test_str = trimmed.trim_start_matches(['▶', '•', '│', ' ', '-', '*']);
        test_str.starts_with("read next")
            || test_str.starts_with("read more")
            || test_str.starts_with("related stories")
            || test_str.starts_with("related articles")
            || test_str.starts_with("related posts")
            || test_str.starts_with("more from")
            || test_str.starts_with("more stories")
            || test_str.starts_with("trending stories")
            || test_str.starts_with("trending now")
            || test_str.starts_with("popular stories")
            || test_str.starts_with("recommended for you")
            || test_str.starts_with("recommended stories")
            || test_str.starts_with("around the web")
            || test_str.starts_with("join the discussion")
            || test_str.starts_with("comments")
            || test_str.starts_with("leave a comment")
            || test_str.starts_with("post a comment")
            || test_str.starts_with("view comments")
            || test_str.starts_with("sign up for our newsletter")
            || test_str.starts_with("subscribe to our newsletter")
            || test_str.starts_with("all rights reserved")
            || test_str.starts_with("copyright ©")
            || test_str.starts_with("© copyright")
            || test_str.starts_with("terms of use")
            || test_str.starts_with("terms of service")
            || test_str.starts_with("privacy policy")
            || test_str.starts_with("disclosures")
            || test_str.starts_with("disclaimer:")
            || test_str.starts_with("unauthorized reproduction is strictly prohibited")
    };

    let is_boilerplate = |line: &str| -> bool {
        let l = line.to_ascii_lowercase();
        let test_str = l.trim_start_matches(['▶', '•', '│', ' ', '-']);
        let words: Vec<&str> = test_str.split_whitespace().collect();
        let count = words.len();
        if count == 0 {
            return true;
        }
        // Subheadings marked with ▶ are legitimate section dividers if >= 2 words
        if line.starts_with('▶') && count >= 2 {
            return false;
        }
        // Bullets or quotes with content are legitimate
        if (line.starts_with('•') || line.starts_with('│')) && count >= 3 {
            return false;
        }
        // Eliminate single-word navigation remnants (e.g. "Home", "Markets", "News", "Share", "Like", "Follow")
        if count <= 1 {
            return true;
        }
        // Eliminate short navigation phrases (<= 3 words without sentence punctuation)
        if count <= 3
            && !test_str.ends_with('.')
            && !test_str.ends_with('?')
            && !test_str.ends_with('!')
            && !test_str.ends_with('"')
            && !test_str.ends_with('\'')
            && matches!(
                words[0],
                "home"
                    | "menu"
                    | "search"
                    | "markets"
                    | "market"
                    | "news"
                    | "crypto"
                    | "business"
                    | "tech"
                    | "investing"
                    | "politics"
                    | "opinion"
                    | "video"
                    | "audio"
                    | "podcast"
                    | "share"
                    | "like"
                    | "tweet"
                    | "follow"
                    | "read"
                    | "related"
                    | "trending"
                    | "popular"
                    | "top"
                    | "latest"
                    | "breaking"
                    | "sign"
                    | "log"
                    | "subscribe"
                    | "click"
                    | "learn"
                    | "watch"
                    | "listen"
                    | "view"
                    | "see"
                    | "photo"
                    | "image"
                    | "source"
                    | "credit"
                    | "advertisement"
                    | "sponsored"
                    | "ad"
                    | "comments"
                    | "comment"
                    | "tags"
                    | "category"
                    | "categories"
                    | "topics"
                    | "topic"
                    | "more"
                    | "next"
                    | "previous"
                    | "back"
                    | "skip"
                    | "close"
            )
        {
            return true;
        }
        // Breadcrumbs, navigation paths, and sharing bars
        if count <= 10
            && (test_str.contains(" > ")
                || test_str.contains(" | ")
                || test_str.contains(" • ")
                || test_str.contains(" · ")
                || test_str.contains(" » ")
                || test_str.contains(" // "))
        {
            return true;
        }
        test_str.starts_with("sign up for")
            || test_str.starts_with("sign in")
            || test_str.starts_with("newsletter")
            || test_str.starts_with("follow us on")
            || test_str.starts_with("follow us")
            || test_str.starts_with("subscribe to")
            || test_str.starts_with("subscribe now")
            || test_str.starts_with("all rights reserved")
            || test_str.starts_with("terms of use")
            || test_str.starts_with("terms of service")
            || test_str.starts_with("privacy policy")
            || test_str.starts_with("cookie preferences")
            || test_str.starts_with("cookie policy")
            || test_str.starts_with("we use cookies")
            || test_str.starts_with("share this article")
            || test_str.starts_with("share this")
            || test_str.starts_with("share on")
            || test_str.starts_with("read more:")
            || test_str.starts_with("read next:")
            || test_str.starts_with("click here to")
            || test_str.starts_with("click here")
            || test_str.starts_with("loading...")
            || test_str.starts_with("advertisement")
            || test_str.starts_with("ad:")
            || test_str.starts_with("sponsored")
            || test_str.starts_with("promoted:")
            || test_str.starts_with("photo by")
            || test_str.starts_with("photo credit")
            || test_str.starts_with("image credit")
            || test_str.starts_with("getty images")
            || test_str.contains("unauthorized reproduction is strictly prohibited")
            || test_str.contains("this story has been shared")
    };

    let mut filtered_paragraphs: Vec<String> = Vec::new();
    let mut prev_line = String::new();

    for p in cleaned.split("\n\n") {
        let trimmed = p.trim();
        if trimmed.len() < 2 {
            continue;
        }
        if is_end_of_article_marker(trimmed) {
            break;
        }
        if is_boilerplate(trimmed) {
            continue;
        }
        if trimmed == prev_line {
            continue;
        }
        if let Some(ref t) = title
            && trimmed.eq_ignore_ascii_case(t.trim())
            && !filtered_paragraphs.is_empty()
        {
            continue;
        }
        prev_line = trimmed.to_string();
        filtered_paragraphs.push(trimmed.to_string());
    }

    let joined = filtered_paragraphs.join("\n\n");
    let sanitized = sanitize_terminal_text(&joined);
    let bounded: String = sanitized.chars().take(16384).collect();

    if bounded.trim().len() < 40 {
        ExtractedArticle {
            title,
            author,
            published_time,
            canonical_url,
            body: format!(
                "Full-text programmatic extraction restricted by publisher. Press 'o' to open the original article in your browser.\n\nCanonical Source: {url}"
            ),
            is_excerpt: true,
        }
    } else {
        ExtractedArticle {
            title,
            author,
            published_time,
            canonical_url,
            body: bounded,
            is_excerpt: false,
        }
    }
}

pub fn clean_html_to_text(raw: &str) -> String {
    let mut cleaned = raw.to_string();
    for tag in [
        "script",
        "style",
        "head",
        "svg",
        "noscript",
        "ix:header",
        "nav",
        "header",
        "footer",
        "aside",
        "form",
        "button",
        "iframe",
        "dialog",
        "select",
        "option",
        "menu",
        "figure",
        "figcaption",
        "picture",
        "canvas",
        "video",
        "audio",
        "object",
        "embed",
    ] {
        let open_pat = format!("<{tag}");
        let close_pat = format!("</{tag}>");
        loop {
            let lower = cleaned.to_ascii_lowercase();
            let Some(start) = lower.find(&open_pat) else {
                break;
            };
            if let Some(end_rel) = lower[start..].find(&close_pat) {
                let end = start + end_rel + close_pat.len();
                cleaned.replace_range(start..end, " ");
            } else {
                break;
            }
        }
    }
    let mut text = String::with_capacity(cleaned.len().min(65536));
    let mut in_tag = false;
    let mut tag_buf = String::new();
    for ch in cleaned.chars() {
        if ch == '<' {
            in_tag = true;
            tag_buf.clear();
        } else if ch == '>' && in_tag {
            in_tag = false;
            let trimmed = tag_buf.trim();
            let is_closing = trimmed.starts_with('/');
            let t = trimmed
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches('/')
                .to_ascii_lowercase();
            if is_closing {
                if matches!(
                    t.as_str(),
                    "p" | "div"
                        | "section"
                        | "article"
                        | "h1"
                        | "h2"
                        | "h3"
                        | "h4"
                        | "h5"
                        | "h6"
                        | "blockquote"
                ) {
                    text.push_str("\n\n");
                } else if t == "li" || t == "tr" {
                    text.push('\n');
                }
            } else {
                match t.as_str() {
                    "p" | "section" | "article" => text.push_str("\n\n"),
                    "br" | "tr" => text.push('\n'),
                    "li" => text.push_str("\n• "),
                    "h1" => text.push_str("\n\n"),
                    "h2" | "h3" | "h4" | "h5" | "h6" => text.push_str("\n\n▶ "),
                    "blockquote" => text.push_str("\n\n│ "),
                    "td" | "th" => text.push_str("  "),
                    _ => {}
                }
            }
        } else if in_tag {
            if tag_buf.len() < 32 {
                tag_buf.push(ch);
            }
        } else {
            text.push(ch);
        }
    }
    let decoded = decode_html_entities(&text);
    let mut paragraphs = Vec::new();
    for block in decoded.split("\n\n") {
        let trimmed = block.trim();
        if trimmed.is_empty() {
            continue;
        }
        let mut clean_lines = Vec::new();
        for line in trimmed.lines() {
            let line_trim = line.trim();
            if line_trim.is_empty() {
                continue;
            }
            let (prefix, body) = if let Some(stripped) = line_trim.strip_prefix('•') {
                ("• ", stripped)
            } else if let Some(stripped) = line_trim.strip_prefix('▶') {
                ("▶ ", stripped)
            } else if let Some(stripped) = line_trim.strip_prefix('│') {
                ("│ ", stripped)
            } else {
                ("", line_trim)
            };
            let collapsed = body.split_whitespace().collect::<Vec<_>>().join(" ");
            if !collapsed.is_empty() {
                if prefix.is_empty() {
                    clean_lines.push(collapsed);
                } else {
                    clean_lines.push(format!("{prefix}{collapsed}"));
                }
            }
        }
        if !clean_lines.is_empty() {
            let combined = clean_lines.join("\n");
            if combined.len() >= 2 {
                paragraphs.push(combined);
            }
        }
    }
    paragraphs.join("\n\n")
}
pub fn summarize_sec_filing_document(form: &str, description: &str, raw_html: &str) -> String {
    let plain = clean_html_to_text(raw_html);
    let mut headings = Vec::new();
    for line in plain.lines() {
        let trimmed = line.trim();
        let upper = trimmed.to_ascii_uppercase();
        if (upper.starts_with("ITEM ")
            || upper.starts_with("PART I")
            || upper.starts_with("PART II")
            || upper.contains("RESULTS OF OPERATIONS")
            || upper.contains("OTHER EVENTS")
            || upper.contains("FINANCIAL STATEMENTS")
            || upper.contains("MANAGEMENT'S DISCUSSION"))
            && trimmed.len() <= 120
            && !headings.contains(&trimmed.to_string())
        {
            headings.push(trimmed.to_string());
            if headings.len() >= 8 {
                break;
            }
        }
    }
    let mut out = format!(
        "SEC EDGAR REGULATORY DISCLOSURE — FORM {form}\nDescription: {description}\nAttribution: Grounded directly in retrieved SEC EDGAR filing text (not journalistic reporting).\n"
    );
    if !headings.is_empty() {
        out.push_str("\nDetected Disclosure Sections:\n");
        for h in &headings {
            out.push_str(&format!("  • {h}\n"));
        }
    }
    out.push_str("\nExtracted Filing Text:\n");
    let body_excerpt: String = plain.chars().take(6000).collect();
    if body_excerpt.trim().is_empty() {
        out.push_str("(Structured XML/XBRL filing without narrative body text; press 'o' to open original SEC filing in browser.)");
    } else {
        out.push_str(&body_excerpt);
    }
    out
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MacroObservation {
    pub series: String,
    pub label: String,
    pub value: Decimal,
    pub unit: String,
    pub geography: String,
    pub date: String,
    pub frequency: String,
    pub source: String,
    pub updated: Option<String>,
}
#[derive(Debug, Clone)]
pub struct Capabilities {
    pub name: &'static str,
    pub quotes: bool,
    pub stream: bool,
    pub candles: bool,
    pub fundamentals: bool,
    pub filings: bool,
    pub macro_data: bool,
    pub credentials: bool,
}
/// Official Treasury maturity fields; display aliases are not additional tenors.
pub fn treasury_tenor_months(series: &str) -> Option<f64> {
    match series {
        "BC_1MONTH" => Some(1.0),
        "BC_1_5MONTH" => Some(1.5),
        "BC_2MONTH" => Some(2.0),
        "BC_3MONTH" => Some(3.0),
        "BC_4MONTH" => Some(4.0),
        "BC_6MONTH" => Some(6.0),
        "BC_1YEAR" => Some(12.0),
        "BC_2YEAR" => Some(24.0),
        "BC_3YEAR" => Some(36.0),
        "BC_5YEAR" => Some(60.0),
        "BC_7YEAR" => Some(84.0),
        "BC_10YEAR" => Some(120.0),
        "BC_20YEAR" => Some(240.0),
        "BC_30YEAR" => Some(360.0),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn venue_identity() {
        assert_ne!(
            "BTCUSDT".parse::<AssetId>().unwrap().key(),
            "COINBASE:BTC-USD".parse::<AssetId>().unwrap().key()
        );
        assert!("AAPL/../../".parse::<AssetId>().is_err());
    }
    #[test]
    fn timeframes() {
        assert_eq!("1H".parse::<Timeframe>().unwrap().seconds(), 3600);
        assert!("2w".parse::<Timeframe>().is_err());
    }
    #[test]
    fn technical_indicators_and_window_stats() {
        let bars: Vec<Candle> = (0..30)
            .map(|i| Candle {
                open_at: Utc::now(),
                open: Decimal::from(100 + i),
                high: Decimal::from(102 + i),
                low: Decimal::from(99 + i),
                close: Decimal::from(101 + i),
                volume: Decimal::from(10),
                closed: true,
            })
            .collect();
        let e = ema(&bars, 10);
        assert!(e[8].is_none());
        assert!(e[9].is_some());
        let bb = bollinger_bands(&bars, 20, 2);
        assert!(bb[18].is_none());
        let (low, mid, high) = bb[19].unwrap();
        assert!(low < mid && mid < high);
        let r = rsi(&bars, 14);
        assert_eq!(r[14], Some(Decimal::from(100)));
        let m = macd(&bars, 5, 10, 3);
        assert!(m.last().unwrap().is_some());
        let a = atr(&bars, 14);
        assert_eq!(a[13], Some(Decimal::from(3)));
        let stats = window_stats(&bars).unwrap();
        assert_eq!(stats.open, Decimal::from(100));
        assert_eq!(stats.close, Decimal::from(130));
        assert!(stats.vwap.is_some());
        assert_eq!(stats.max_drawdown_pct, Decimal::ZERO);
    }
    #[test]
    fn quote_spread_and_yield_curve_analytics() {
        let mut q = Quote {
            asset: "BTCUSDT".parse().unwrap(),
            price: Decimal::from(100),
            currency: "USDT".into(),
            change_pct: Some(Decimal::from(2)),
            change_abs: Some(Decimal::from(2)),
            open_24h: Some(Decimal::from(98)),
            high: Some(Decimal::from(110)),
            low: Some(Decimal::from(90)),
            volume: Some(Decimal::from(50)),
            quote_volume: Some(Decimal::from(5000)),
            bid: Some(Decimal::from(99)),
            ask: Some(Decimal::from(101)),
            vwap: Some(Decimal::from(100)),
            base_asset: Some("BTC".into()),
            provider: "Binance Spot".into(),
            coverage: "Binance exchange only".into(),
            event_at: Utc::now(),
            received_at: Utc::now(),
            status: DataStatus::Live,
            delay_seconds: Some(0),
        };
        assert!(q.validate().is_ok());
        assert_eq!(q.base_symbol(), "BTC");
        assert_eq!(q.effective_change_abs(), Some(Decimal::from(2)));
        let ethbtc: AssetId = "ETHBTC".parse().unwrap();
        assert_eq!(ethbtc.venue, Venue::Binance);
        assert_eq!(ethbtc.currency(), "BTC");
        assert_eq!(ethbtc.base_asset(), "ETH");
        let fdusd: AssetId = "BTCFDUSD".parse().unwrap();
        assert_eq!(fdusd.venue, Venue::Binance);
        assert_eq!(fdusd.currency(), "FDUSD");
        assert_eq!(fdusd.base_asset(), "BTC");
        assert_eq!(q.spread(), Some(Decimal::from(2)));
        assert_eq!(q.spread_bps(), Some(Decimal::from(200)));
        assert_eq!(q.range_pct(), Some(Decimal::from(50)));
        q.bid = Some(Decimal::from(105));
        assert!(q.validate().is_err());

        let curve = vec![
            MacroObservation {
                series: "BC_3MONTH".into(),
                label: "3MONTH".into(),
                value: "4.20".parse().unwrap(),
                unit: "percent per annum".into(),
                geography: "United States".into(),
                date: "2026-10-22".into(),
                frequency: "Daily".into(),
                source: "US Treasury".into(),
                updated: None,
            },
            MacroObservation {
                series: "BC_2YEAR".into(),
                label: "2YEAR".into(),
                value: "4.00".parse().unwrap(),
                unit: "percent per annum".into(),
                geography: "United States".into(),
                date: "2026-10-22".into(),
                frequency: "Daily".into(),
                source: "US Treasury".into(),
                updated: None,
            },
            MacroObservation {
                series: "BC_10YEAR".into(),
                label: "10YEAR".into(),
                value: "4.50".parse().unwrap(),
                unit: "percent per annum".into(),
                geography: "United States".into(),
                date: "2026-10-22".into(),
                frequency: "Daily".into(),
                source: "US Treasury".into(),
                updated: None,
            },
        ];
        let yc = analyze_yield_curve(&curve).unwrap();
        assert_eq!(yc.spread_2s10s_bps, Some(Decimal::from(50)));
        assert_eq!(yc.spread_3m10s_bps, Some(Decimal::from(30)));
        assert_eq!(yc.regime, "Normal (Upward Sloping)");
    }

    #[test]
    fn test_article_extraction_cleans_boilerplate_and_scripts() {
        let html = r#"
            <!DOCTYPE html>
            <html>
            <head>
                <title>Apple Reports Record Q4 Revenue | TechNews</title>
                <meta property="og:title" content="Apple Reports Record Q4 Revenue" />
                <meta name="author" content="Jane Doe" />
                <meta property="article:published_time" content="2026-10-23T16:30:00Z" />
                <link rel="canonical" href="https://example.com/apple-q4-results" />
                <style>body { font-family: sans-serif; }</style>
                <script>console.log("tracking pixel");</script>
            </head>
            <body>
                <header>
                    <nav class="nav-menu">
                        <a href="/home">Home</a>
                        <a href="/markets">Markets</a>
                        <button>Sign In</button>
                    </nav>
                </header>
                <article>
                    <h1 class="headline">Apple Reports Record Q4 Revenue</h1>
                    <div class="share-buttons">Share on Twitter | Share on Facebook | Copy Link</div>
                    <p class="body-p">Apple Inc. reported fourth-quarter revenue of $94.9 billion, up 6 percent year-over-year.</p>
                    <div class="ad-banner">Advertisement: Buy Crypto Now!</div>
                    <p class="body-p">Services revenue reached an all-time record of $25.0 billion during the fiscal quarter.</p>
                    <p class="newsletter-cta">Sign up for our daily morning newsletter to get breaking news.</p>
                    <p class="body-p">The company returned over $29 billion to shareholders during the quarter through dividends and repurchases.</p>
                    <aside class="sidebar">
                        <h3>Trending Stories</h3>
                        <ul><li>Fed raises rates</li><li>Bitcoin hits new high</li></ul>
                    </aside>
                    <footer>
                        <p>All rights reserved. Unauthorized reproduction is strictly prohibited.</p>
                    </footer>
                </article>
            </body>
            </html>
        "#;

        let extracted = extract_article_content(html, "https://example.com/apple-q4-results");
        assert_eq!(
            extracted.title.as_deref(),
            Some("Apple Reports Record Q4 Revenue")
        );
        assert_eq!(extracted.author.as_deref(), Some("Jane Doe"));
        assert_eq!(
            extracted.published_time.as_deref(),
            Some("2026-10-23T16:30:00Z")
        );
        assert_eq!(
            extracted.canonical_url.as_deref(),
            Some("https://example.com/apple-q4-results")
        );
        assert!(!extracted.is_excerpt);

        // Core facts are preserved
        assert!(
            extracted
                .body
                .contains("reported fourth-quarter revenue of $94.9 billion")
        );
        assert!(
            extracted
                .body
                .contains("Services revenue reached an all-time record of $25.0 billion")
        );
        assert!(
            extracted
                .body
                .contains("returned over $29 billion to shareholders")
        );

        // Boilerplate and navigation remnants are removed
        assert!(!extracted.body.contains("Sign In"));
        assert!(!extracted.body.contains("Share on Twitter"));
        assert!(!extracted.body.contains("Buy Crypto Now"));
        assert!(!extracted.body.contains("Sign up for our daily"));
        assert!(!extracted.body.contains("All rights reserved"));
        assert!(!extracted.body.contains("tracking pixel"));
        assert!(!extracted.body.contains("Home"));
        assert!(!extracted.body.contains("Markets"));
        assert!(!extracted.body.contains("Trending Stories"));
    }

    #[test]
    fn test_article_extraction_with_headline_anchoring_and_formatting() {
        let html = r#"
            <html>
            <body>
                <div class="top-nav">
                    <span>Markets</span> | <span>Crypto</span> | <span>Tech</span>
                    <a href="/login">Sign In</a>
                </div>
                <div class="trending-bar">Trending: Gold hits all-time high</div>
                <article>
                    <header>
                        <h1 class="story-title">Federal Reserve Lowers Benchmark Rate by 25 Basis Points</h1>
                        <div class="byline">By John Smith • Published October 24, 2026</div>
                    </header>
                    <p>The Federal Reserve lowered its benchmark interest rate by a quarter percentage point on Thursday, continuing its policy easing.</p>
                    <h2>Economic Context</h2>
                    <p>Officials cited moderating inflation pressures and balanced labor market conditions for the decision.</p>
                    <ul>
                        <li>Federal funds rate target range lowered to 4.25%-4.50%</li>
                        <li>Discount rate adjusted downward accordingly</li>
                    </ul>
                    <blockquote>Inflation has made significant progress toward our two percent objective over the past year.</blockquote>
                    <p>The committee noted that economic activity has continued to expand at a solid pace.</p>
                    <div class="comments-section">
                        <h3>Comments (14)</h3>
                        <p>User1: Expected move by the Fed.</p>
                    </div>
                </article>
            </body>
            </html>
        "#;
        let extracted = extract_article_content_with_headline(
            html,
            "https://example.com/fed-cuts",
            Some("Federal Reserve Lowers Benchmark Rate"),
        );
        assert_eq!(
            extracted.title.as_deref(),
            Some("Federal Reserve Lowers Benchmark Rate")
        );
        assert!(!extracted.is_excerpt);
        // Verified facts preserved
        assert!(
            extracted
                .body
                .contains("Federal Reserve lowered its benchmark interest rate")
        );
        // Subheading formatted
        assert!(extracted.body.contains("▶ Economic Context"));
        // Bullets formatted
        assert!(
            extracted
                .body
                .contains("• Federal funds rate target range lowered")
        );
        // Blockquote formatted
        assert!(
            extracted
                .body
                .contains("│ Inflation has made significant progress")
        );
        // Pre-headline nav discarded
        assert!(!extracted.body.contains("top-nav"));
        assert!(!extracted.body.contains("Sign In"));
        assert!(!extracted.body.contains("Trending: Gold"));
        // Post-article comments discarded
        assert!(!extracted.body.contains("User1: Expected move"));
    }

    #[test]
    fn test_sanitize_terminal_text_strips_ansi_and_osc() {
        let malicious = "Normal text \x1b[31;1mRed Bold Text\x1b[0m and \x1b]0;Evil Title\x07OSC attack \x00\x08clean";
        let sanitized = sanitize_terminal_text(malicious);
        assert!(!sanitized.contains("\x1b"));
        assert!(!sanitized.contains("\x07"));
        assert!(!sanitized.contains("\x00"));
        assert!(!sanitized.contains("\x08"));
        assert!(sanitized.contains("Normal text"));
        assert!(sanitized.contains("Red Bold Text"));
        assert!(sanitized.contains("clean"));
    }

    #[test]
    fn test_chart_data_status_labels_and_actionable_errors() {
        assert_eq!(ChartDataStatus::NotRequested.label(), "NOT REQUESTED");
        assert_eq!(ChartDataStatus::Loading.label(), "LOADING");
        assert_eq!(ChartDataStatus::Ready.label(), "READY");
        assert_eq!(ChartDataStatus::Stale.label(), "STALE CACHE");
        assert_eq!(
            ChartDataStatus::AuthenticationRequired.label(),
            "AUTH REQUIRED"
        );
        assert_eq!(ChartDataStatus::RateLimited.label(), "RATE LIMITED");
        assert_eq!(ChartDataStatus::Unsupported.label(), "UNSUPPORTED");
        assert_eq!(ChartDataStatus::Error.label(), "ERROR");

        assert!(!ChartDataStatus::Ready.is_actionable_error());
        assert!(!ChartDataStatus::Loading.is_actionable_error());
        assert!(ChartDataStatus::AuthenticationRequired.is_actionable_error());
        assert!(ChartDataStatus::RateLimited.is_actionable_error());
        assert!(ChartDataStatus::Error.is_actionable_error());
    }

    #[test]
    fn test_worksheet_and_column_model() {
        assert_eq!(WorksheetColumn::Symbol.label(), "SYMBOL");
        assert_eq!(WorksheetColumn::Last.label(), "LAST");
        assert_eq!(
            WorksheetColumn::parse_column("SYM"),
            Some(WorksheetColumn::Symbol)
        );
        assert_eq!(
            WorksheetColumn::parse_column("CHG%"),
            Some(WorksheetColumn::ChangePct)
        );
        assert_eq!(
            WorksheetColumn::parse_column("TURNOVER"),
            Some(WorksheetColumn::QuoteVolume)
        );
        assert_eq!(
            WorksheetColumn::parse_column("SPREAD"),
            Some(WorksheetColumn::SpreadBps)
        );
        assert_eq!(WorksheetColumn::parse_column("UNKNOWN_XYZ"), None);

        let mut ws = Worksheet::new("TECH");
        assert_eq!(ws.name, "TECH");
        assert_eq!(ws.columns, WorksheetColumn::default_columns());
        assert!(ws.symbols.is_empty());

        let aapl: AssetId = "AAPL".parse().unwrap();
        let msft: AssetId = "MSFT".parse().unwrap();

        assert!(ws.add(aapl.clone()));
        assert!(!ws.add(aapl.clone())); // duplicate rejected
        assert_eq!(ws.symbols.len(), 1);

        assert!(ws.add(msft.clone()));
        assert_eq!(ws.symbols.len(), 2);

        assert!(ws.remove(&aapl));
        assert!(!ws.remove(&aapl)); // already removed
        assert_eq!(ws.symbols.len(), 1);
        assert_eq!(ws.symbols[0], msft);

        // Serialize and deserialize round-trip
        let json = serde_json::to_string(&ws).unwrap();
        let deserialized: Worksheet = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, deserialized);
    }
}
