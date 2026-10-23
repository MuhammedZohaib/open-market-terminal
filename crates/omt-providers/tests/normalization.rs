use omt_core::{PeriodView, facts};
use omt_providers::research::{Sec, parse_facts, parse_world_bank};
#[test]
fn treasury_display_alias_is_not_a_maturity() {
    let xml = "<feed><properties><NEW_DATE>2026-10-22T00:00:00</NEW_DATE><BC_1MONTH>4.11</BC_1MONTH><BC_1_5MONTH>4.10</BC_1_5MONTH><BC_30YEAR>5.29</BC_30YEAR><BC_30YEARDISPLAY>5.29</BC_30YEARDISPLAY></properties></feed>";
    let rows = omt_providers::research::parse_treasury(xml).unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(rows[1].series, "BC_1_5MONTH");
    assert_eq!(omt_core::treasury_tenor_months(&rows[1].series), Some(1.5));
    assert_eq!(omt_core::treasury_tenor_months("BC_30YEARDISPLAY"), None);
}
#[test]
fn amendments_and_direct_quarters() {
    let json =
        serde_json::from_str(include_str!("../../../tests/fixtures/sec-facts.json")).unwrap();
    let data = parse_facts(&json, 1).unwrap();
    let annual = facts::select(&data, PeriodView::Annual);
    let revenue = annual.iter().find(|f| f.metric == "Revenue").unwrap();
    assert_eq!(revenue.value.to_string(), "1100");
    assert!(revenue.source.ends_with("000000000125000002/"));
    let quarterly = facts::select(&data, PeriodView::Quarterly);
    let revenue = quarterly.iter().find(|f| f.metric == "Revenue").unwrap();
    assert_eq!(revenue.value.to_string(), "250");
}
#[test]
fn resolution_keeps_cik() {
    let mapping = serde_json::json!({"0":{"ticker":"AAPL","cik_str":320193,"title":"Apple Inc."}});
    assert_eq!(Sec::resolve(&mapping, "aapl").unwrap().0, 320193);
    assert!(Sec::resolve(&mapping, "MISSING").is_err());
}
#[test]
fn empty_and_null_macro_observations() {
    let v = serde_json::json!([{"lastupdated":"2025-01-01"},[{"value":null},{"value":10,"date":"2024","indicator":{"value":"GDP"},"country":{"value":"United States"},"unit":""}]]);
    let r = parse_world_bank(&v, "NY.GDP.MKTP.CD").unwrap();
    assert_eq!(r.len(), 1);
    assert_eq!(r[0].unit, "current USD");
    assert_eq!(r[0].date, "2024");
}
