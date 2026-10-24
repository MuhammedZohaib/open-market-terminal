//! Daily event studies. Associations are descriptive, never causal estimates.
use crate::{AssetId, Candle, NewsItem};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::prelude::ToPrimitive;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpactData {
    pub asset: AssetId,
    pub benchmark: AssetId,
    pub prices: Vec<Candle>,
    pub benchmark_prices: Vec<Candle>,
    pub news: Vec<NewsItem>,
    pub fetched_at: DateTime<Utc>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct PairedClose {
    pub date: NaiveDate,
    pub asset: f64,
    pub benchmark: f64,
}

/// Use completed UTC daily bars only; intersect dates before computing returns.
pub fn aligned(data: &ImpactData) -> Vec<PairedClose> {
    fn closes(bars: &[Candle], now: DateTime<Utc>) -> BTreeMap<NaiveDate, f64> {
        bars.iter()
            .filter(|b| b.closed && b.validate() && b.open_at.date_naive() < now.date_naive())
            .filter_map(|b| {
                b.close
                    .to_f64()
                    .filter(|v| v.is_finite() && *v > 0.0)
                    .map(|v| (b.open_at.date_naive(), v))
            })
            .collect()
    }
    let a = closes(&data.prices, data.fetched_at);
    let b = closes(&data.benchmark_prices, data.fetched_at);
    a.into_iter()
        .filter_map(|(date, asset)| {
            b.get(&date).map(|benchmark| PairedClose {
                date,
                asset,
                benchmark: *benchmark,
            })
        })
        .collect()
}

#[derive(Debug, Clone)]
pub struct Relationship {
    pub samples: usize,
    pub correlation: Option<f64>,
    pub beta: Option<f64>,
    pub asset_return: Option<f64>,
    pub benchmark_return: Option<f64>,
}

pub fn relationship(rows: &[PairedClose]) -> Relationship {
    let returns: Vec<_> = rows
        .windows(2)
        .map(|w| {
            (
                w[1].asset / w[0].asset - 1.0,
                w[1].benchmark / w[0].benchmark - 1.0,
            )
        })
        .collect();
    let n = returns.len();
    let mut result = Relationship {
        samples: n,
        correlation: None,
        beta: None,
        asset_return: None,
        benchmark_return: None,
    };
    if let (Some(first), Some(last)) = (rows.first(), rows.last()) {
        result.asset_return = Some((last.asset / first.asset - 1.0) * 100.0);
        result.benchmark_return = Some((last.benchmark / first.benchmark - 1.0) * 100.0);
    }
    if n < 20 {
        return result;
    }
    let ax = returns.iter().map(|r| r.0).sum::<f64>() / n as f64;
    let ay = returns.iter().map(|r| r.1).sum::<f64>() / n as f64;
    let (mut xx, mut yy, mut xy) = (0.0, 0.0, 0.0);
    for (x, y) in returns {
        xx += (x - ax).powi(2);
        yy += (y - ay).powi(2);
        xy += (x - ax) * (y - ay);
    }
    if yy > 1e-18 {
        result.beta = Some(xy / yy);
    }
    if xx > 1e-18 && yy > 1e-18 {
        result.correlation = Some((xy / (xx * yy).sqrt()).clamp(-1.0, 1.0));
    }
    result
}

#[derive(Debug, Clone)]
pub struct EventMove {
    pub baseline: NaiveDate,
    pub end: NaiveDate,
    pub asset_pct: f64,
    pub benchmark_pct: f64,
    pub excess_pp: f64,
}

/// Includes the event date's session, using the preceding common close as baseline.
/// Reject missing coverage rather than silently picking a distant price.
pub fn event_move(rows: &[PairedClose], date: NaiveDate, sessions: usize) -> Option<EventMove> {
    if sessions == 0 {
        return None;
    }
    let start = rows.iter().position(|r| r.date >= date)?;
    let base = rows.get(start.checked_sub(1)?)?;
    if (date - base.date).num_days() > 7 || (rows[start].date - date).num_days() > 7 {
        return None;
    }
    let end = rows.get(start.checked_add(sessions - 1)?)?;
    if (end.date - rows[start].date).num_days() > (sessions as i64 * 3 + 7) {
        return None;
    }
    let asset_pct = (end.asset / base.asset - 1.0) * 100.0;
    let benchmark_pct = (end.benchmark / base.benchmark - 1.0) * 100.0;
    Some(EventMove {
        baseline: base.date,
        end: end.date,
        asset_pct,
        benchmark_pct,
        excess_pp: asset_pct - benchmark_pct,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alignment_excludes_partial_future_missing_and_invalid_prices() {
        let at = DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let bars: Vec<_> = (0..5)
            .map(|i| Candle {
                open_at: at + chrono::Duration::days(i),
                open: 100.into(),
                high: 100.into(),
                low: 100.into(),
                close: 100.into(),
                volume: 10.into(),
                closed: i != 2,
            })
            .collect();
        let data = ImpactData {
            asset: "AAPL".parse().unwrap(),
            benchmark: "SPY".parse().unwrap(),
            prices: bars.clone(),
            benchmark_prices: vec![bars[0].clone(), bars[2].clone(), bars[4].clone()],
            news: vec![],
            fetched_at: at + chrono::Duration::days(4),
            warnings: vec![],
        };
        let rows = aligned(&data);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].date, at.date_naive());
    }
    fn rows() -> Vec<PairedClose> {
        let mut price = 100.0;
        (0..31)
            .map(|i| {
                price *= 1.0 + (i as f64 % 5.0 - 1.5) / 100.0;
                PairedClose {
                    date: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap() + chrono::Duration::days(i),
                    asset: price,
                    benchmark: price * 2.0,
                }
            })
            .collect()
    }
    #[test]
    fn correlation_uses_returns_and_requires_samples() {
        let rows = rows();
        let r = relationship(&rows);
        assert_eq!(r.samples, 30);
        assert!((r.correlation.unwrap() - 1.0).abs() < 1e-10);
        assert!((r.beta.unwrap() - 1.0).abs() < 1e-10);
        assert!(relationship(&rows[..10]).correlation.is_none());
        let constant: Vec<_> = rows
            .into_iter()
            .map(|mut r| {
                r.benchmark = 100.0;
                r
            })
            .collect();
        assert!(relationship(&constant).correlation.is_none());
        assert!(relationship(&constant).beta.is_none());
    }
    #[test]
    fn event_uses_preceding_close_and_never_invents_future() {
        let rows = rows();
        let m = event_move(&rows, rows[10].date, 3).unwrap();
        assert_eq!(m.baseline, rows[9].date);
        assert_eq!(m.end, rows[12].date);
        assert!(m.excess_pp.abs() < 1e-10);
        assert!(event_move(&rows, rows[0].date, 1).is_none());
        assert!(event_move(&rows, rows[30].date, 3).is_none());
        assert!(event_move(&rows, rows[10].date, 0).is_none());
    }
}
