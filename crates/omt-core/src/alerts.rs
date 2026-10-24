use crate::{AssetId, DataStatus, Decimal, Quote};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriceAlert {
    pub id: u64,
    pub asset: AssetId,
    pub above: bool,
    pub threshold: Decimal,
    pub created_at: DateTime<Utc>,
    pub triggered_at: Option<DateTime<Utc>>,
    pub triggered_price: Option<Decimal>,
}
impl PriceAlert {
    /// One-shot threshold condition; never evaluate cached, delayed or stale prices.
    pub fn evaluate(&mut self, quote: &Quote, now: DateTime<Utc>) -> bool {
        let age = now.signed_duration_since(quote.event_at).num_seconds();
        let received_age = now.signed_duration_since(quote.received_at).num_seconds();
        if self.triggered_at.is_some()
            || quote.asset != self.asset
            || quote.status != DataStatus::Live
            || !(0..=60).contains(&age)
            || !(0..=60).contains(&received_age)
            || quote.event_at < self.created_at
            || quote.price <= Decimal::ZERO
        {
            return false;
        }
        let hit = if self.above {
            quote.price >= self.threshold
        } else {
            quote.price <= self.threshold
        };
        if hit {
            self.triggered_at = Some(now);
            self.triggered_price = Some(quote.price);
        }
        hit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alerts_require_new_fresh_live_matching_quotes_and_fire_once() {
        let now = Utc::now();
        let asset: AssetId = "AAPL".parse().unwrap();
        let template = PriceAlert {
            id: 1,
            asset: asset.clone(),
            above: true,
            threshold: 100.into(),
            created_at: now - chrono::Duration::seconds(10),
            triggered_at: None,
            triggered_price: None,
        };
        let mut q = Quote {
            asset,
            price: 100.into(),
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
            base_asset: None,
            provider: "fixture".into(),
            coverage: "fixture".into(),
            event_at: now,
            received_at: now,
            status: DataStatus::Historical,
            delay_seconds: Some(0),
        };
        assert!(!template.clone().evaluate(&q, now));
        q.status = DataStatus::Disconnected;
        assert!(!template.clone().evaluate(&q, now));
        q.status = DataStatus::Live;
        q.event_at = now - chrono::Duration::seconds(61);
        assert!(!template.clone().evaluate(&q, now));
        q.event_at = now - chrono::Duration::seconds(11);
        assert!(!template.clone().evaluate(&q, now));
        q.event_at = now + chrono::Duration::seconds(2);
        assert!(!template.clone().evaluate(&q, now));
        q.event_at = now;
        q.asset = "MSFT".parse().unwrap();
        assert!(!template.clone().evaluate(&q, now));
        q.asset = "AAPL".parse().unwrap();
        let mut alert = template.clone();
        assert!(alert.evaluate(&q, now));
        assert!(!alert.evaluate(&q, now));
        assert_eq!(alert.triggered_price, Some(100.into()));
        let mut below = template;
        below.above = false;
        q.price = 99.into();
        assert!(below.evaluate(&q, now));
    }
}
