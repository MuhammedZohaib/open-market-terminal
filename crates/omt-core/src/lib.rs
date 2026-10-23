#![forbid(unsafe_code)]
pub mod command;
pub mod facts;
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
pub fn clean_html_to_text(raw: &str) -> String {
    let mut cleaned = raw.to_string();
    for tag in ["script", "style", "head", "svg", "noscript", "ix:header"] {
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
            let t = tag_buf
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_matches('/')
                .to_ascii_lowercase();
            if matches!(
                t.as_str(),
                "p" | "div"
                    | "br"
                    | "tr"
                    | "li"
                    | "h1"
                    | "h2"
                    | "h3"
                    | "h4"
                    | "h5"
                    | "h6"
                    | "section"
                    | "article"
                    | "blockquote"
            ) {
                text.push('\n');
            } else if matches!(t.as_str(), "td" | "th") {
                text.push_str("  ");
            }
        } else if in_tag {
            if tag_buf.len() < 24 {
                tag_buf.push(ch);
            }
        } else {
            text.push(ch);
        }
    }
    let decoded = decode_html_entities(&text);
    let mut paragraphs = Vec::new();
    for line in decoded.lines() {
        let collapsed = line.split_whitespace().collect::<Vec<_>>().join(" ");
        if collapsed.len() >= 2 {
            paragraphs.push(collapsed);
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
}
