#![forbid(unsafe_code)]
use chrono::{DateTime, Utc};
use omt_core::{
    AssetId, Candle, ChartDataStatus, DataStatus, Decimal, InstrumentInfo, Quote, Timeframe,
    TradingStatus,
};
use std::collections::{BTreeMap, HashSet};
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Subscription {
    pub assets: Vec<AssetId>,
    pub chart: AssetId,
    pub timeframe: Timeframe,
}
impl Subscription {
    pub fn normalized(mut self) -> Self {
        self.assets.retain(|asset| *asset != self.chart);
        self.assets.sort_by_key(AssetId::key);
        self.assets.dedup();
        self.assets.truncate(49);
        self.assets.push(self.chart.clone());
        self.assets.sort_by_key(AssetId::key);
        self
    }
}
#[derive(Default, Debug, Clone)]
pub struct MarketState {
    pub instruments: BTreeMap<String, InstrumentInfo>,
    pub quotes: BTreeMap<String, Quote>,
    pub candles: Vec<Candle>,
    pub candles_received_at: Option<DateTime<Utc>>,
    pub chart_key: String,
    pub chart_status: ChartDataStatus,
    pub chart_error: Option<String>,
    pub status: String,
    pub venue_status: BTreeMap<String, String>,
    pub rejected: u64,
    pub accepted: u64,
    pub coalesced: u64,
    pub reconnects: u64,
    pub last_ingest_micros: u128,
    pub last_normalize_micros: u128,
    pub last_message_at: Option<DateTime<Utc>>,
    pub last_snapshot_at: Option<DateTime<Utc>>,
    pub clock_skew_ms: Option<i64>,
    pub ingest_samples_us: Vec<u64>,
}
impl MarketState {
    pub fn register_instruments(&mut self, list: Vec<InstrumentInfo>) {
        for info in list {
            self.instruments.insert(info.asset.key(), info);
        }
    }
    pub fn trading_instrument_count(&self) -> usize {
        self.instruments
            .values()
            .filter(|i| i.status == TradingStatus::Trading)
            .count()
    }
    pub fn ingest(&mut self, mut quote: Quote) -> bool {
        let start = std::time::Instant::now();
        let key = quote.asset.key();
        if let Some(info) = self.instruments.get(&key) {
            if quote.base_asset.is_none() {
                quote.base_asset = Some(info.base_asset.clone());
            }
            if quote.currency == "UNKNOWN" && !info.quote_asset.is_empty() {
                quote.currency = info.quote_asset.clone();
            }
        }
        if quote.validate().is_err()
            || self
                .quotes
                .get(&key)
                .is_some_and(|old| old.event_at >= quote.event_at)
        {
            self.rejected += 1;
            return false;
        }
        if quote.event_at > quote.received_at {
            self.clock_skew_ms = Some((quote.event_at - quote.received_at).num_milliseconds());
        }
        if let Some(old) = self.quotes.get(&key) {
            if quote.vwap.is_none() {
                quote.vwap = old.vwap;
            }
            if quote.base_asset.is_none() {
                quote.base_asset = old.base_asset.clone();
            }
            if quote.bid.is_none()
                && quote.ask.is_none()
                && let (Some(b), Some(a)) = (old.bid, old.ask)
                && b <= quote.price
                && quote.price <= a
            {
                quote.bid = Some(b);
                quote.ask = Some(a);
            }
        }
        self.last_message_at = Some(quote.received_at);
        self.quotes.insert(key, quote);
        self.accepted += 1;
        let elapsed_us = start.elapsed().as_micros();
        self.last_ingest_micros = elapsed_us;
        if self.ingest_samples_us.len() >= 512 {
            self.ingest_samples_us.remove(0);
        }
        self.ingest_samples_us.push(elapsed_us as u64);
        true
    }
    pub fn apply_book_ticker(
        &mut self,
        key: &str,
        bid: Decimal,
        ask: Decimal,
        received_at: DateTime<Utc>,
    ) -> bool {
        if bid <= Decimal::ZERO || ask <= Decimal::ZERO || ask < bid {
            self.rejected += 1;
            return false;
        }
        if let Some(q) = self.quotes.get_mut(key) {
            q.bid = Some(bid);
            q.ask = Some(ask);
            q.received_at = received_at;
            q.status = DataStatus::Live;
            self.last_message_at = Some(received_at);
            self.accepted += 1;
            true
        } else {
            false
        }
    }
    pub fn ingest_percentiles_us(&self) -> (u64, u64, u64) {
        if self.ingest_samples_us.is_empty() {
            return (0, 0, 0);
        }
        let mut s = self.ingest_samples_us.clone();
        s.sort_unstable();
        let n = s.len();
        (
            s[(n * 50 / 100).min(n - 1)],
            s[(n * 95 / 100).min(n - 1)],
            s[(n * 99 / 100).min(n - 1)],
        )
    }
    pub fn workspace_feed_status(&self, venue_hint: &str) -> String {
        if let Some(s) = self.venue_status.get(venue_hint) {
            return format!("{venue_hint}: {s}");
        }
        if self.status.is_empty() {
            "Idle".into()
        } else {
            self.status.clone()
        }
    }
    pub fn candle(&mut self, bar: Candle) {
        if !bar.validate() {
            self.rejected += 1;
            return;
        }
        let max_keep = self.candles.len().clamp(500, 2000);
        match self.candles.last_mut() {
            Some(last) if last.open_at == bar.open_at => {
                if !last.closed || bar.closed {
                    *last = bar;
                }
            }
            Some(last) if last.open_at > bar.open_at => {
                self.rejected += 1;
            }
            _ => self.candles.push(bar),
        }
        if self.candles.len() > max_keep {
            self.candles.drain(..self.candles.len() - max_keep);
        }
        self.candles_received_at = Some(chrono::Utc::now());
        self.chart_status = ChartDataStatus::Ready;
        self.chart_error = None;
    }
    pub fn merge_candles(&mut self, incoming: Vec<Candle>) {
        let mut by_time: BTreeMap<DateTime<Utc>, Candle> = BTreeMap::new();
        for b in self.candles.drain(..) {
            if b.validate() {
                by_time.insert(b.open_at, b);
            }
        }
        for b in incoming {
            if b.validate() {
                match by_time.get_mut(&b.open_at) {
                    Some(existing) => {
                        if !existing.closed || b.closed {
                            *existing = b;
                        }
                    }
                    None => {
                        by_time.insert(b.open_at, b);
                    }
                }
            }
        }
        let mut merged: Vec<Candle> = by_time.into_values().collect();
        if merged.len() > 2000 {
            merged.drain(..merged.len() - 2000);
        }
        self.candles = merged;
        self.candles_received_at = Some(chrono::Utc::now());
        if !self.candles.is_empty() {
            self.chart_status = ChartDataStatus::Ready;
            self.chart_error = None;
        }
    }
    pub fn retain(&mut self, assets: &[AssetId]) {
        let active: HashSet<_> = assets.iter().map(AssetId::key).collect();
        self.quotes.retain(|k, _| active.contains(k));
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subscriptions_are_deduplicated_and_keep_chart() {
        let chart: AssetId = "BINANCE:ZZZUSDT".parse().unwrap();
        let mut assets: Vec<AssetId> = (0..80)
            .map(|i| format!("BINANCE:T{i}USDT").parse().unwrap())
            .collect();
        assets.push(assets[0].clone());
        let sub = Subscription {
            assets,
            chart: chart.clone(),
            timeframe: Timeframe::H1,
        }
        .normalized();
        assert_eq!(sub.assets.len(), 50);
        assert!(sub.assets.contains(&chart));
        assert_eq!(sub.clone(), sub.normalized());
    }
    #[test]
    fn closed_candle_cannot_be_reopened_and_history_is_bounded() {
        let mut state = MarketState::default();
        let bar = Candle {
            open_at: chrono::Utc::now(),
            open: 10.into(),
            high: 12.into(),
            low: 9.into(),
            close: 11.into(),
            volume: 2.into(),
            closed: true,
        };
        state.candle(bar.clone());
        let mut partial = bar.clone();
        partial.closed = false;
        partial.close = 10.into();
        state.candle(partial);
        assert!(state.candles[0].closed);
        assert_eq!(state.candles[0].close, 11.into());
        for i in 1..700 {
            let mut b = bar.clone();
            b.open_at += chrono::Duration::minutes(i);
            state.candle(b);
        }
        assert_eq!(state.candles.len(), 500);
        state.candle(bar);
        assert_eq!(state.rejected, 1);
    }
    use chrono::Utc;
    use omt_core::DataStatus;
    #[test]
    fn rejects_duplicates_and_future() {
        let mut s = MarketState::default();
        let q = Quote {
            asset: "BTCUSDT".parse().unwrap(),
            price: 100.into(),
            currency: "USDT".into(),
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
            base_asset: None,
            provider: "test".into(),
            coverage: "test".into(),
            event_at: Utc::now(),
            received_at: Utc::now(),
            status: DataStatus::Live,
            delay_seconds: Some(0),
        };
        assert!(s.ingest(q.clone()));
        assert!(!s.ingest(q.clone()));
        assert!(s.apply_book_ticker("BINANCE:BTCUSDT", 99.into(), 101.into(), Utc::now()));
        assert_eq!(s.quotes["BINANCE:BTCUSDT"].bid, Some(99.into()));
        assert_eq!(s.quotes["BINANCE:BTCUSDT"].ask, Some(101.into()));
        let mut old = q.clone();
        old.event_at -= chrono::Duration::seconds(1);
        assert!(!s.ingest(old));
        let mut future = q;
        future.event_at += chrono::Duration::hours(1);
        assert!(!s.ingest(future));
        assert_eq!(s.quotes.len(), 1);
    }
}
