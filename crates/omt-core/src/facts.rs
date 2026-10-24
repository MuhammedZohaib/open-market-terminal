use crate::{FinancialFact, PeriodView};
use chrono::NaiveDate;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum StatementTab {
    #[default]
    Overview,
    Income,
    BalanceSheet,
    CashFlow,
    Ratios,
}
impl StatementTab {
    pub const ALL: [Self; 5] = [
        Self::Overview,
        Self::Income,
        Self::BalanceSheet,
        Self::CashFlow,
        Self::Ratios,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Overview => "Overview",
            Self::Income => "Income Stmt",
            Self::BalanceSheet => "Balance Sheet",
            Self::CashFlow => "Cash Flow",
            Self::Ratios => "Ratios",
        }
    }
    pub fn next(self) -> Self {
        match self {
            Self::Overview => Self::Income,
            Self::Income => Self::BalanceSheet,
            Self::BalanceSheet => Self::CashFlow,
            Self::CashFlow => Self::Ratios,
            Self::Ratios => Self::Overview,
        }
    }
    pub fn prev(self) -> Self {
        match self {
            Self::Overview => Self::Ratios,
            Self::Income => Self::Overview,
            Self::BalanceSheet => Self::Income,
            Self::CashFlow => Self::BalanceSheet,
            Self::Ratios => Self::CashFlow,
        }
    }
    pub fn metrics(self) -> &'static [&'static str] {
        match self {
            Self::Overview => &[
                "Revenue",
                "Gross profit",
                "Operating income",
                "Net income",
                "R&D expense",
                "Operating cash flow",
                "CapEx",
                "Cash",
                "Current assets",
                "Total assets",
                "Current liabilities",
                "Long-term debt",
                "Total liabilities",
                "Equity",
            ],
            Self::Income => &[
                "Revenue",
                "Cost of revenue",
                "Gross profit",
                "R&D expense",
                "SG&A expense",
                "Operating income",
                "Pretax income",
                "Income tax",
                "Net income",
            ],
            Self::BalanceSheet => &[
                "Cash",
                "Short-term investments",
                "Accounts receivable",
                "Inventory",
                "Current assets",
                "Property & equipment",
                "Total assets",
                "Accounts payable",
                "Current liabilities",
                "Long-term debt",
                "Total liabilities",
                "Retained earnings",
                "Equity",
            ],
            Self::CashFlow => &[
                "Operating cash flow",
                "Depreciation & amort",
                "CapEx",
                "Investing cash flow",
                "Share repurchases",
                "Dividends paid",
                "Financing cash flow",
            ],
            Self::Ratios => &[
                "Gross margin %",
                "Operating margin %",
                "Net margin %",
                "OCF margin %",
                "Free cash flow",
                "FCF margin %",
                "ROE %",
                "ROA %",
                "Current ratio",
                "Debt / Equity %",
            ],
        }
    }
}

/// Direct reported quarters only: never interpret year-to-date flows as quarters.
/// Latest filing wins for a comparable period; conflicting same-filing values are unavailable.
pub fn select(facts: &[FinancialFact], view: PeriodView) -> Vec<&FinancialFact> {
    let mut annual_ends = BTreeSet::new();
    let mut quarterly_ends = BTreeSet::new();
    for fact in facts {
        if fact.unit == "USD"
            && ["10-K", "10-K/A", "10-Q", "10-Q/A"].contains(&fact.form.as_str())
            && let Some(start) = fact.start
        {
            let days = (fact.end - start).num_days();
            if (330..=380).contains(&days) {
                annual_ends.insert(fact.end);
            } else if (70..=105).contains(&days) {
                quarterly_ends.insert(fact.end);
            }
        }
    }
    let mut groups: BTreeMap<_, Vec<&FinancialFact>> = BTreeMap::new();
    for fact in facts {
        let valid = match fact.start {
            Some(start) => {
                let days = (fact.end - start).num_days();
                match view {
                    PeriodView::Annual => (330..=380).contains(&days),
                    PeriodView::Quarterly => (70..=105).contains(&days),
                }
            }
            None => match view {
                PeriodView::Annual => {
                    if !annual_ends.is_empty() {
                        annual_ends.contains(&fact.end)
                    } else {
                        fact.form.starts_with("10-K")
                    }
                }
                PeriodView::Quarterly => {
                    if !quarterly_ends.is_empty() || !annual_ends.is_empty() {
                        quarterly_ends.contains(&fact.end)
                            || (!annual_ends.contains(&fact.end) && fact.form.starts_with("10-Q"))
                    } else {
                        fact.form.starts_with("10-Q") || fact.form.starts_with("10-K")
                    }
                }
            },
        };
        if valid
            && fact.unit == "USD"
            && ["10-K", "10-K/A", "10-Q", "10-Q/A"].contains(&fact.form.as_str())
        {
            groups
                .entry((&fact.metric, fact.start, fact.end, &fact.unit))
                .or_default()
                .push(fact);
        }
    }
    let mut selected = Vec::new();
    for mut group in groups.into_values() {
        group.sort_by(|a, b| b.filed.cmp(&a.filed).then(b.accession.cmp(&a.accession)));
        let latest = group[0];
        let same: Vec<_> = group
            .iter()
            .filter(|f| f.filed == latest.filed && f.accession == latest.accession)
            .collect();
        if same.iter().all(|f| f.value == latest.value) {
            selected.push(latest);
        }
    }
    selected.sort_by(|a, b| b.end.cmp(&a.end).then(a.metric.cmp(&b.metric)));
    selected
}

#[derive(Debug, Clone)]
pub struct ComparativeRow<'a> {
    pub metric: &'static str,
    pub is_pct: bool,
    pub is_multiple: bool,
    pub values: Vec<Option<Decimal>>,
    pub sources: Vec<Option<&'a FinancialFact>>,
    pub yoy_pct: Option<Decimal>,
}

#[derive(Debug, Clone)]
pub struct ComparativeTable<'a> {
    pub periods: Vec<NaiveDate>,
    pub rows: Vec<ComparativeRow<'a>>,
}

pub fn comparative_table(
    facts: &[FinancialFact],
    view: PeriodView,
    tab: StatementTab,
) -> ComparativeTable<'_> {
    let selected = select(facts, view);
    let mut period_set = BTreeSet::new();
    for f in &selected {
        if f.start.is_some() {
            period_set.insert(f.end);
        }
    }
    if period_set.is_empty() {
        for f in &selected {
            period_set.insert(f.end);
        }
    }
    let periods: Vec<NaiveDate> = period_set.into_iter().rev().take(6).collect();
    let find_at = |metric: &str, end: NaiveDate| -> Option<&FinancialFact> {
        selected
            .iter()
            .copied()
            .find(|f| f.metric == metric && f.end == end)
    };
    let mut rows = Vec::new();
    for &metric in tab.metrics() {
        if tab == StatementTab::Ratios {
            let mut values = Vec::with_capacity(periods.len());
            let mut sources = Vec::with_capacity(periods.len());
            for &end in &periods {
                let rev = find_at("Revenue", end);
                let gp = find_at("Gross profit", end);
                let op = find_at("Operating income", end);
                let ni = find_at("Net income", end);
                let ocf = find_at("Operating cash flow", end);
                let capex = find_at("CapEx", end);
                let ca = find_at("Current assets", end);
                let cl = find_at("Current liabilities", end);
                let assets = find_at("Total assets", end);
                let liab = find_at("Total liabilities", end);
                let eq = find_at("Equity", end);
                let fcf_val = ocf.zip(capex).and_then(|(o, c)| {
                    (o.unit == c.unit && o.start.is_some() && o.start == c.start && o.end == c.end)
                        .then_some(o.value - c.value)
                });
                let (val, src) = match metric {
                    "Gross margin %" => (gp.zip(rev).and_then(|(n, d)| margin(n, d)), gp.or(rev)),
                    "Operating margin %" => {
                        (op.zip(rev).and_then(|(n, d)| margin(n, d)), op.or(rev))
                    }
                    "Net margin %" => (ni.zip(rev).and_then(|(n, d)| margin(n, d)), ni.or(rev)),
                    "OCF margin %" => (ocf.zip(rev).and_then(|(n, d)| margin(n, d)), ocf.or(rev)),
                    "Free cash flow" => (fcf_val, ocf.or(capex)),
                    "FCF margin %" => (
                        fcf_val.zip(rev).and_then(|(f, r)| {
                            (r.value != Decimal::ZERO).then(|| f / r.value * Decimal::from(100))
                        }),
                        ocf.or(rev),
                    ),
                    "ROE %" => (
                        (view == PeriodView::Annual)
                            .then_some(())
                            .and_then(|_| ni.zip(eq))
                            .and_then(|(n, e)| {
                                (n.unit == e.unit && e.start.is_none() && e.value > Decimal::ZERO)
                                    .then(|| n.value / e.value * Decimal::from(100))
                            }),
                        ni.or(eq),
                    ),
                    "ROA %" => (
                        (view == PeriodView::Annual)
                            .then_some(())
                            .and_then(|_| ni.zip(assets))
                            .and_then(|(n, a)| {
                                (n.unit == a.unit && a.start.is_none() && a.value > Decimal::ZERO)
                                    .then(|| n.value / a.value * Decimal::from(100))
                            }),
                        ni.or(assets),
                    ),
                    "Current ratio" => (
                        ca.zip(cl).and_then(|(a, l)| {
                            (a.unit == l.unit
                                && a.start.is_none()
                                && l.start.is_none()
                                && l.value > Decimal::ZERO)
                                .then(|| a.value / l.value)
                        }),
                        ca.or(cl),
                    ),
                    "Debt / Equity %" => (
                        liab.zip(eq).and_then(|(l, e)| {
                            (l.unit == e.unit
                                && l.start.is_none()
                                && e.start.is_none()
                                && e.value > Decimal::ZERO)
                                .then(|| l.value / e.value * Decimal::from(100))
                        }),
                        liab.or(eq),
                    ),
                    _ => (None, None),
                };
                values.push(val);
                sources.push(src);
            }
            let yoy_pct = if metric == "Free cash flow" && values.len() >= 2 && periods.len() >= 2 {
                if (350..=380).contains(&(periods[0] - periods[1]).num_days()) {
                    values[0].zip(values[1]).and_then(|(cur, prev)| {
                        (prev != Decimal::ZERO)
                            .then(|| (cur - prev) / prev.abs() * Decimal::from(100))
                    })
                } else {
                    None
                }
            } else {
                None
            };
            rows.push(ComparativeRow {
                metric,
                is_pct: metric.ends_with('%'),
                is_multiple: metric == "Current ratio",
                values,
                sources,
                yoy_pct,
            });
        } else {
            let mut values = Vec::with_capacity(periods.len());
            let mut sources = Vec::with_capacity(periods.len());
            for &end in &periods {
                let fact = find_at(metric, end);
                values.push(fact.map(|f| f.value));
                sources.push(fact);
            }
            let yoy_pct = sources
                .first()
                .and_then(|f| *f)
                .and_then(|f| yoy_growth(&selected, f));
            rows.push(ComparativeRow {
                metric,
                is_pct: false,
                is_multiple: false,
                values,
                sources,
                yoy_pct,
            });
        }
    }
    ComparativeTable { periods, rows }
}
pub fn margin(numerator: &FinancialFact, denominator: &FinancialFact) -> Option<Decimal> {
    (numerator.unit == denominator.unit
        && numerator.start.is_some()
        && numerator.start == denominator.start
        && numerator.end == denominator.end
        && denominator.value != Decimal::ZERO)
        .then(|| numerator.value / denominator.value * Decimal::from(100))
}
pub fn yoy_growth(selected: &[&FinancialFact], target: &FinancialFact) -> Option<Decimal> {
    let prior = selected.iter().copied().find(|f| {
        f.metric == target.metric
            && f.unit == target.unit
            && f.start.is_some() == target.start.is_some()
            && (350..=380).contains(&(target.end - f.end).num_days())
    })?;
    (prior.value != Decimal::ZERO)
        .then(|| (target.value - prior.value) / prior.value.abs() * Decimal::from(100))
}
#[derive(Debug, Clone)]
pub struct FundamentalSummary {
    pub latest_end: chrono::NaiveDate,
    pub gross_margin_pct: Option<Decimal>,
    pub operating_margin_pct: Option<Decimal>,
    pub net_margin_pct: Option<Decimal>,
    pub ocf_margin_pct: Option<Decimal>,
    pub free_cash_flow: Option<Decimal>,
    pub roe_pct: Option<Decimal>,
    pub roa_pct: Option<Decimal>,
    pub current_ratio: Option<Decimal>,
    pub debt_to_equity_pct: Option<Decimal>,
}
pub fn summarize(facts: &[FinancialFact], view: PeriodView) -> Option<FundamentalSummary> {
    let selected = select(facts, view);
    let latest_end = selected.first()?.end;
    let find = |m: &str| {
        selected
            .iter()
            .copied()
            .find(|f| f.metric == m && f.end == latest_end)
    };
    let rev = find("Revenue");
    let gp = find("Gross profit");
    let op = find("Operating income");
    let ni = find("Net income");
    let ocf = find("Operating cash flow");
    let capex = find("CapEx");
    let assets = find("Total assets");
    let liab = find("Total liabilities");
    let eq = find("Equity");
    let ca = find("Current assets");
    let cl = find("Current liabilities");

    let gross_margin_pct = gp.zip(rev).and_then(|(n, d)| margin(n, d));
    let operating_margin_pct = op.zip(rev).and_then(|(n, d)| margin(n, d));
    let net_margin_pct = ni.zip(rev).and_then(|(n, d)| margin(n, d));
    let ocf_margin_pct = ocf.zip(rev).and_then(|(n, d)| margin(n, d));
    let free_cash_flow = ocf.zip(capex).and_then(|(o, c)| {
        (o.unit == c.unit && o.start.is_some() && o.start == c.start && o.end == c.end)
            .then_some(o.value - c.value)
    });
    let roe_pct = (view == PeriodView::Annual)
        .then_some(())
        .and_then(|_| ni.zip(eq))
        .and_then(|(n, e)| {
            (n.unit == e.unit && e.start.is_none() && e.value > Decimal::ZERO)
                .then(|| n.value / e.value * Decimal::from(100))
        });
    let roa_pct = (view == PeriodView::Annual)
        .then_some(())
        .and_then(|_| ni.zip(assets))
        .and_then(|(n, a)| {
            (n.unit == a.unit && a.start.is_none() && a.value > Decimal::ZERO)
                .then(|| n.value / a.value * Decimal::from(100))
        });
    let current_ratio = ca.zip(cl).and_then(|(a, l)| {
        (a.unit == l.unit && a.start.is_none() && l.start.is_none() && l.value > Decimal::ZERO)
            .then(|| a.value / l.value)
    });
    let debt_to_equity_pct = liab.zip(eq).and_then(|(l, e)| {
        (l.unit == e.unit && l.start.is_none() && e.start.is_none() && e.value > Decimal::ZERO)
            .then(|| l.value / e.value * Decimal::from(100))
    });
    Some(FundamentalSummary {
        latest_end,
        gross_margin_pct,
        operating_margin_pct,
        net_margin_pct,
        ocf_margin_pct,
        free_cash_flow,
        roe_pct,
        roa_pct,
        current_ratio,
        debt_to_equity_pct,
    })
}

#[derive(Debug, Clone)]
pub struct MetricHistoryPoint {
    pub period_end: NaiveDate,
    pub period_label: String,
    pub value: Decimal,
    pub concept: String,
    pub form: String,
    pub accession: String,
    pub unit: String,
    pub filed: NaiveDate,
    pub start: Option<NaiveDate>,
}

#[derive(Debug, Clone)]
pub struct MetricAuditDetails {
    pub metric_name: String,
    pub points: Vec<MetricHistoryPoint>,
    pub min_value: Option<Decimal>,
    pub max_value: Option<Decimal>,
    pub latest_yoy_pct: Option<Decimal>,
    pub unit: String,
}

pub fn metric_audit(
    facts: &[FinancialFact],
    view: PeriodView,
    metric_name: &str,
) -> Option<MetricAuditDetails> {
    let selected = select(facts, view);
    let mut matching: Vec<&FinancialFact> = selected
        .iter()
        .copied()
        .filter(|f| f.metric == metric_name)
        .collect();
    if matching.is_empty() {
        return None;
    }
    matching.sort_by_key(|f| f.end);
    let unit = matching
        .first()
        .map(|f| f.unit.clone())
        .unwrap_or_else(|| "USD".into());
    let mut points = Vec::new();
    let mut min_val: Option<Decimal> = None;
    let mut max_val: Option<Decimal> = None;

    for f in &matching {
        let label = match view {
            PeriodView::Annual => f.end.format("%Y").to_string(),
            PeriodView::Quarterly => f.end.format("%Y-Q%q").to_string(),
        };
        min_val = Some(min_val.map(|m| m.min(f.value)).unwrap_or(f.value));
        max_val = Some(max_val.map(|m| m.max(f.value)).unwrap_or(f.value));
        points.push(MetricHistoryPoint {
            period_end: f.end,
            period_label: label,
            value: f.value,
            concept: f.concept.clone(),
            form: f.form.clone(),
            accession: f.accession.clone(),
            unit: f.unit.clone(),
            filed: f.filed,
            start: f.start,
        });
    }

    let latest_yoy_pct = if matching.len() >= 2 {
        let target = matching[matching.len() - 1];
        yoy_growth(&matching, target)
    } else {
        None
    };

    Some(MetricAuditDetails {
        metric_name: metric_name.to_string(),
        points,
        min_value: min_val,
        max_value: max_val,
        latest_yoy_pct,
        unit,
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;
    fn fact(start: &str, end: &str, value: i64, filed: &str) -> FinancialFact {
        FinancialFact {
            metric: "Revenue".into(),
            concept: "Revenues".into(),
            value: value.into(),
            unit: "USD".into(),
            start: Some(NaiveDate::parse_from_str(start, "%Y-%m-%d").unwrap()),
            end: NaiveDate::parse_from_str(end, "%Y-%m-%d").unwrap(),
            filed: NaiveDate::parse_from_str(filed, "%Y-%m-%d").unwrap(),
            accession: "one".into(),
            form: "10-Q".into(),
            source: "SEC".into(),
        }
    }
    #[test]
    fn rejects_ytd_as_quarter() {
        let f = vec![
            fact("2024-01-01", "2024-06-30", 200, "2024-07-30"),
            fact("2024-04-01", "2024-06-30", 100, "2024-07-30"),
        ];
        assert_eq!(select(&f, PeriodView::Quarterly).len(), 1);
        assert_eq!(
            select(&f, PeriodView::Quarterly)[0].value,
            Decimal::from(100)
        );
    }
    #[test]
    fn amendments_conflicts_and_margins() {
        let a = fact("2024-01-01", "2024-03-31", 100, "2024-05-01");
        let b = fact("2024-01-01", "2024-03-31", 110, "2024-06-01");
        let mut c = b.clone();
        c.value = 111.into();
        assert_eq!(
            select(&[a.clone(), b.clone()], PeriodView::Quarterly)[0].value,
            110.into()
        );
        assert!(select(&[b, c], PeriodView::Quarterly).is_empty());
        let other = fact("2024-04-01", "2024-06-30", 100, "2024-07-30");
        assert!(margin(&a, &other).is_none());
        let y1 = fact("2023-01-01", "2023-12-31", 1000, "2024-02-01");
        let y2 = fact("2024-01-01", "2024-12-31", 1250, "2025-02-01");
        let mut ni = y2.clone();
        ni.metric = "Net income".into();
        ni.value = 250.into();
        let all = [y1, y2, ni];
        let sel = select(&all, PeriodView::Annual);
        let latest_rev = sel.iter().copied().find(|f| f.metric == "Revenue").unwrap();
        assert_eq!(yoy_growth(&sel, latest_rev), Some(Decimal::from(25)));
        let sum = summarize(&all, PeriodView::Annual).unwrap();
        assert_eq!(sum.net_margin_pct, Some(Decimal::from(20)));
    }
    #[test]
    fn annual_and_quarterly_separation_and_comparative_matrix() {
        let mut facts = vec![
            // Annual 10-K revenue & balance sheet
            FinancialFact {
                metric: "Revenue".into(),
                concept: "Revenues".into(),
                value: Decimal::from(400_000_000_000i64),
                unit: "USD".into(),
                start: Some(NaiveDate::from_ymd_opt(2024, 10, 1).unwrap()),
                end: NaiveDate::from_ymd_opt(2025, 9, 27).unwrap(),
                filed: NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                accession: "0000320193-25-000079".into(),
                form: "10-K".into(),
                source: "SEC".into(),
            },
            FinancialFact {
                metric: "Total assets".into(),
                concept: "Assets".into(),
                value: Decimal::from(365_000_000_000i64),
                unit: "USD".into(),
                start: None,
                end: NaiveDate::from_ymd_opt(2025, 9, 27).unwrap(),
                filed: NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                accession: "0000320193-25-000079".into(),
                form: "10-K".into(),
                source: "SEC".into(),
            },
            // Comparative prior-year balance sheet inside 10-K that should NOT leak into Quarterly 10-Q
            FinancialFact {
                metric: "Total assets".into(),
                concept: "Assets".into(),
                value: Decimal::from(352_000_000_000i64),
                unit: "USD".into(),
                start: None,
                end: NaiveDate::from_ymd_opt(2024, 9, 28).unwrap(),
                filed: NaiveDate::from_ymd_opt(2025, 11, 1).unwrap(),
                accession: "0000320193-25-000079".into(),
                form: "10-K".into(),
                source: "SEC".into(),
            },
            // Quarterly 10-Q revenue & balance sheet
            FinancialFact {
                metric: "Revenue".into(),
                concept: "Revenues".into(),
                value: Decimal::from(124_000_000_000i64),
                unit: "USD".into(),
                start: Some(NaiveDate::from_ymd_opt(2025, 9, 28).unwrap()),
                end: NaiveDate::from_ymd_opt(2025, 12, 27).unwrap(),
                filed: NaiveDate::from_ymd_opt(2026, 1, 30).unwrap(),
                accession: "0000320193-26-000006".into(),
                form: "10-Q".into(),
                source: "SEC".into(),
            },
            FinancialFact {
                metric: "Total assets".into(),
                concept: "Assets".into(),
                value: Decimal::from(379_000_000_000i64),
                unit: "USD".into(),
                start: None,
                end: NaiveDate::from_ymd_opt(2025, 12, 27).unwrap(),
                filed: NaiveDate::from_ymd_opt(2026, 1, 30).unwrap(),
                accession: "0000320193-26-000006".into(),
                form: "10-Q".into(),
                source: "SEC".into(),
            },
        ];
        // Add Gross profit for ratio calculation
        facts.push(FinancialFact {
            metric: "Gross profit".into(),
            concept: "GrossProfit".into(),
            value: Decimal::from(58_000_000_000i64),
            unit: "USD".into(),
            start: Some(NaiveDate::from_ymd_opt(2025, 9, 28).unwrap()),
            end: NaiveDate::from_ymd_opt(2025, 12, 27).unwrap(),
            filed: NaiveDate::from_ymd_opt(2026, 1, 30).unwrap(),
            accession: "0000320193-26-000006".into(),
            form: "10-Q".into(),
            source: "SEC".into(),
        });

        let q_table = comparative_table(&facts, PeriodView::Quarterly, StatementTab::Overview);
        assert_eq!(
            q_table.periods,
            vec![NaiveDate::from_ymd_opt(2025, 12, 27).unwrap()]
        );
        let q_rev = q_table.rows.iter().find(|r| r.metric == "Revenue").unwrap();
        assert_eq!(q_rev.values[0], Some(Decimal::from(124_000_000_000i64)));

        let a_table = comparative_table(&facts, PeriodView::Annual, StatementTab::BalanceSheet);
        assert_eq!(
            a_table.periods,
            vec![NaiveDate::from_ymd_opt(2025, 9, 27).unwrap()]
        );
        let a_assets = a_table
            .rows
            .iter()
            .find(|r| r.metric == "Total assets")
            .unwrap();
        assert_eq!(a_assets.values[0], Some(Decimal::from(365_000_000_000i64)));

        let r_table = comparative_table(&facts, PeriodView::Quarterly, StatementTab::Ratios);
        let gm = r_table
            .rows
            .iter()
            .find(|r| r.metric == "Gross margin %")
            .unwrap();
        assert!(gm.values[0].is_some());
    }

    #[test]
    fn test_metric_audit_extracts_full_history_and_provenance() {
        let facts = vec![
            FinancialFact {
                metric: "Revenue".into(),
                concept: "Revenues".into(),
                value: Decimal::from(383_000_000_000i64),
                unit: "USD".into(),
                start: Some(NaiveDate::from_ymd_opt(2022, 10, 1).unwrap()),
                end: NaiveDate::from_ymd_opt(2023, 9, 30).unwrap(),
                filed: NaiveDate::from_ymd_opt(2023, 11, 3).unwrap(),
                accession: "0000320193-23-000106".into(),
                form: "10-K".into(),
                source: "SEC".into(),
            },
            FinancialFact {
                metric: "Revenue".into(),
                concept: "Revenues".into(),
                value: Decimal::from(391_000_000_000i64),
                unit: "USD".into(),
                start: Some(NaiveDate::from_ymd_opt(2023, 10, 1).unwrap()),
                end: NaiveDate::from_ymd_opt(2024, 9, 28).unwrap(),
                filed: NaiveDate::from_ymd_opt(2024, 11, 1).unwrap(),
                accession: "0000320193-24-000106".into(),
                form: "10-K".into(),
                source: "SEC".into(),
            },
        ];

        let audit = metric_audit(&facts, PeriodView::Annual, "Revenue").unwrap();
        assert_eq!(audit.metric_name, "Revenue");
        assert_eq!(audit.points.len(), 2);
        assert_eq!(audit.points[0].period_label, "2023");
        assert_eq!(audit.points[0].value, Decimal::from(383_000_000_000i64));
        assert_eq!(audit.points[0].concept, "Revenues");
        assert_eq!(audit.points[1].period_label, "2024");
        assert_eq!(audit.points[1].value, Decimal::from(391_000_000_000i64));
        assert_eq!(audit.min_value, Some(Decimal::from(383_000_000_000i64)));
        assert_eq!(audit.max_value, Some(Decimal::from(391_000_000_000i64)));
        assert!(audit.latest_yoy_pct.is_some());
    }
}
