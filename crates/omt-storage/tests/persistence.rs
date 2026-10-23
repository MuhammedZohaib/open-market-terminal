use chrono::{TimeZone, Utc};
use omt_core::portfolio::{Side, Transaction};
use omt_storage::Store;
use std::path::PathBuf;
fn directory() -> PathBuf {
    let p = std::env::temp_dir().join(format!(
        "omt-test-{}-{}",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap()
    ));
    std::fs::create_dir_all(&p).unwrap();
    p
}
fn buy() -> Transaction {
    Transaction {
        id: "buy-1".into(),
        account: "research".into(),
        asset: "BTCUSDT".parse().unwrap(),
        side: Side::Buy,
        quantity: 2.into(),
        price: 100.into(),
        fee: 1.into(),
        currency: "USDT".into(),
        at: Utc.with_ymd_and_hms(2025, 1, 1, 0, 0, 0).unwrap(),
    }
}
#[test]
fn reopen_and_atomic_csv() {
    let dir = directory();
    let db = dir.join("db.sqlite");
    let csv = dir.join("trades.csv");
    {
        let mut s = Store::open(&db).unwrap();
        s.watch("long-term", &"AAPL".parse().unwrap(), true)
            .unwrap();
        s.add_transactions(&[buy()]).unwrap();
        assert_eq!(s.export_csv(&csv).unwrap(), 1);
        assert!(s.export_csv(&csv).is_err());
    }
    {
        let s = Store::open(&db).unwrap();
        assert_eq!(s.transactions().unwrap().len(), 1);
        assert_eq!(s.watchlist("long-term").unwrap().len(), 1);
    }
    let mut imported = Store::memory().unwrap();
    assert_eq!(imported.import_csv(&csv).unwrap(), 1);
    assert!(imported.import_csv(&csv).is_err());
    assert_eq!(imported.transactions().unwrap().len(), 1);
    std::fs::remove_dir_all(dir).unwrap();
}
#[test]
fn invalid_batch_leaves_no_records() {
    let mut s = Store::memory().unwrap();
    let mut sell = buy();
    sell.id = "sell".into();
    sell.side = Side::Sell;
    sell.quantity = 3.into();
    sell.at += chrono::Duration::seconds(1);
    assert!(s.add_transactions(&[buy(), sell]).is_err());
    assert_eq!(s.transactions().unwrap().len(), 0);
}
#[test]
fn delete_transaction_revalidates_ledger() {
    let mut s = Store::memory().unwrap();
    let b = buy();
    let mut sell = buy();
    sell.id = "sell-1".into();
    sell.side = Side::Sell;
    sell.quantity = 1.into();
    sell.at += chrono::Duration::seconds(1);
    s.add_transactions(&[b, sell]).unwrap();
    assert!(s.delete_transaction("buy-1").is_err());
    assert_eq!(s.transactions().unwrap().len(), 2);
    s.delete_transaction("sell-1").unwrap();
    s.delete_transaction("buy-1").unwrap();
    assert!(s.transactions().unwrap().is_empty());
}
#[test]
fn cache_timestamp_preserved() {
    let s = Store::memory().unwrap();
    let at = Utc.with_ymd_and_hms(2020, 1, 1, 0, 0, 0).unwrap();
    s.put_at("macro", "old", &vec![1, 2], at).unwrap();
    assert_eq!(s.get::<Vec<i32>>("macro", "old").unwrap().unwrap().1, at);
}
#[test]
fn future_schema_is_not_modified() {
    let dir = directory();
    let path = dir.join("future.sqlite");
    {
        let db = rusqlite::Connection::open(&path).unwrap();
        db.execute_batch("PRAGMA user_version=99;").unwrap();
    }
    assert!(Store::open(&path).is_err());
    std::fs::remove_dir_all(dir).unwrap();
}
