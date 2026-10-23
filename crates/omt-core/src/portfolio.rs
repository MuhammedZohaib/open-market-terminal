use crate::AssetId;
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Side {
    Buy,
    Sell,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transaction {
    pub id: String,
    pub account: String,
    pub asset: AssetId,
    pub side: Side,
    pub quantity: Decimal,
    pub price: Decimal,
    pub fee: Decimal,
    pub currency: String,
    pub at: DateTime<Utc>,
}
#[derive(Debug, Clone)]
pub struct Position {
    pub account: String,
    pub asset: AssetId,
    pub currency: String,
    pub quantity: Decimal,
    pub cost: Decimal,
    pub realized: Decimal,
}
impl Position {
    pub fn average_cost(&self) -> Option<Decimal> {
        if self.quantity > Decimal::ZERO {
            self.cost.checked_div(self.quantity)
        } else {
            None
        }
    }
    pub fn unrealized(&self, price: Decimal, currency: &str) -> Option<Decimal> {
        if currency != self.currency {
            return None;
        }
        self.quantity.checked_mul(price)?.checked_sub(self.cost)
    }
    pub fn market_value(&self, price: Decimal, currency: &str) -> Option<Decimal> {
        if currency != self.currency {
            return None;
        }
        self.quantity.checked_mul(price)
    }
    pub fn unrealized_pct(&self, price: Decimal, currency: &str) -> Option<Decimal> {
        if self.cost <= Decimal::ZERO {
            return None;
        }
        let u = self.unrealized(price, currency)?;
        u.checked_div(self.cost)
            .and_then(|v| v.checked_mul(Decimal::from(100)))
    }
    pub fn total_pnl(&self, price: Decimal, currency: &str) -> Option<Decimal> {
        self.unrealized(price, currency)
            .and_then(|u| self.realized.checked_add(u))
    }
}
#[derive(Debug, thiserror::Error, PartialEq)]
pub enum AccountingError {
    #[error("Amount exceeds the supported accounting precision")]
    Overflow,
    #[error("Quantity and price must be positive, fees nonnegative")]
    InvalidAmount,
    #[error("Transaction currency does not match asset")]
    Currency,
    #[error("Cannot sell more than the owned quantity (no short positions)")]
    Oversell,
    #[error("Duplicate transaction identifier")]
    Duplicate,
    #[error("Transaction identifier/account missing or timestamp in future")]
    InvalidIdentity,
}
pub fn positions(transactions: &[Transaction]) -> Result<Vec<Position>, AccountingError> {
    let mut sorted: Vec<_> = transactions.iter().collect();
    sorted.sort_by_key(|t| t.at);
    let mut result: BTreeMap<(String, String), Position> = BTreeMap::new();
    let mut ids = std::collections::HashSet::new();
    for t in sorted {
        if !ids.insert(&t.id) {
            return Err(AccountingError::Duplicate);
        }
        if t.id.is_empty()
            || t.account.is_empty()
            || t.at > Utc::now() + chrono::Duration::seconds(60)
        {
            return Err(AccountingError::InvalidIdentity);
        }
        if t.quantity <= Decimal::ZERO || t.price <= Decimal::ZERO || t.fee < Decimal::ZERO {
            return Err(AccountingError::InvalidAmount);
        }
        if t.currency != t.asset.currency() {
            return Err(AccountingError::Currency);
        }
        let p = result
            .entry((t.account.clone(), t.asset.key()))
            .or_insert_with(|| Position {
                account: t.account.clone(),
                asset: t.asset.clone(),
                currency: t.currency.clone(),
                quantity: Decimal::ZERO,
                cost: Decimal::ZERO,
                realized: Decimal::ZERO,
            });
        match t.side {
            Side::Buy => {
                p.quantity = p
                    .quantity
                    .checked_add(t.quantity)
                    .ok_or(AccountingError::Overflow)?;
                let cost = t
                    .quantity
                    .checked_mul(t.price)
                    .and_then(|v| v.checked_add(t.fee))
                    .ok_or(AccountingError::Overflow)?;
                p.cost = p.cost.checked_add(cost).ok_or(AccountingError::Overflow)?;
            }
            Side::Sell => {
                if t.quantity > p.quantity {
                    return Err(AccountingError::Oversell);
                }
                let removed = if t.quantity == p.quantity {
                    p.cost
                } else {
                    p.cost
                        .checked_mul(t.quantity)
                        .and_then(|v| v.checked_div(p.quantity))
                        .ok_or(AccountingError::Overflow)?
                };
                let proceeds = t
                    .quantity
                    .checked_mul(t.price)
                    .and_then(|v| v.checked_sub(t.fee))
                    .and_then(|v| v.checked_sub(removed))
                    .ok_or(AccountingError::Overflow)?;
                p.realized = p
                    .realized
                    .checked_add(proceeds)
                    .ok_or(AccountingError::Overflow)?;
                p.cost -= removed;
                p.quantity -= t.quantity;
            }
        }
    }
    Ok(result.into_values().collect())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn overflow_is_reported_and_accounts_do_not_mix() {
        let mut large = tx(Side::Buy, 1, 1, 0, "large");
        large.quantity = Decimal::MAX;
        large.price = 2.into();
        assert_eq!(positions(&[large]).unwrap_err(), AccountingError::Overflow);
        let buy = tx(Side::Buy, 1, 10, 0, "a");
        let mut sell = tx(Side::Sell, 1, 11, 0, "b");
        sell.account = "other".into();
        assert_eq!(
            positions(&[buy, sell]).unwrap_err(),
            AccountingError::Oversell
        );
    }
    fn tx(side: Side, qty: i64, price: i64, fee: i64, id: &str) -> Transaction {
        Transaction {
            id: id.into(),
            account: "default".into(),
            asset: "BTCUSDT".parse().unwrap(),
            side,
            quantity: qty.into(),
            price: price.into(),
            fee: fee.into(),
            currency: "USDT".into(),
            at: Utc::now(),
        }
    }
    #[test]
    fn average_cost_and_fees() {
        let p = positions(&[
            tx(Side::Buy, 2, 100, 2, "a"),
            tx(Side::Buy, 2, 200, 2, "b"),
            tx(Side::Sell, 1, 200, 1, "c"),
        ])
        .unwrap();
        assert_eq!(p[0].quantity, Decimal::from(3));
        assert_eq!(p[0].cost, Decimal::from(453));
        assert_eq!(p[0].realized, Decimal::from(48));
        assert_eq!(p[0].unrealized(200.into(), "USDT"), Some(147.into()));
        assert_eq!(p[0].unrealized(200.into(), "USD"), None);
    }
    #[test]
    fn rejects_invalid() {
        assert_eq!(
            positions(&[tx(Side::Sell, 1, 1, 0, "a")]).unwrap_err(),
            AccountingError::Oversell
        );
        let mut t = tx(Side::Buy, 1, 1, 0, "a");
        t.currency = "USD".into();
        assert_eq!(positions(&[t]).unwrap_err(), AccountingError::Currency);
    }
}
