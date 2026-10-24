#![forbid(unsafe_code)]
pub mod engine;
pub mod types;

pub use engine::{MarketEngine, ResearchRequest};
pub use types::*;

#[cfg(test)]
mod tests {
    use super::*;
    use omt_core::AssetId;

    #[tokio::test]
    async fn test_engine_initialization_offline() {
        let temp_dir =
            std::env::temp_dir().join(format!("omt_service_test_{}", std::process::id()));
        let engine = MarketEngine::new(temp_dir.clone(), true, None).expect("Engine should init");

        // Verify initial snapshot
        let snap = engine.market_snapshot();
        assert_eq!(snap.total_instruments, 0);

        // Verify searching returns empty initially
        let found = engine.search_instruments("BTC");
        assert!(found.is_empty());

        // Verify command execution
        let outcome = engine
            .execute_command("BTCUSDT GP 1H")
            .expect("Command should succeed");
        assert!(outcome.success);
        assert_eq!(outcome.workspace_target.as_deref(), Some("CHART"));
        assert_eq!(outcome.asset_target.as_deref(), Some("BINANCE:BTCUSDT"));
        assert_eq!(outcome.timeframe_target.as_deref(), Some("1h"));

        // Clean up
        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[tokio::test]
    async fn test_engine_portfolio_flow() {
        let temp_dir = std::env::temp_dir().join(format!("omt_port_test_{}", std::process::id()));
        let engine = MarketEngine::new(temp_dir.clone(), true, None).expect("Engine should init");

        let asset: AssetId = "BINANCE:BTCUSDT".parse().unwrap();
        let tx = omt_core::portfolio::Transaction {
            id: "tx-1".into(),
            account: "default".into(),
            asset,
            side: omt_core::portfolio::Side::Buy,
            quantity: rust_decimal::Decimal::from(2),
            price: rust_decimal::Decimal::from(50000),
            fee: rust_decimal::Decimal::from(10),
            currency: "USDT".into(),
            at: chrono::Utc::now(),
        };

        engine.add_transaction(tx).expect("Add tx should succeed");
        let port = engine.portfolio("default").expect("Get portfolio");
        assert_eq!(port.holdings.len(), 1);
        assert_eq!(port.holdings[0].symbol, "BTCUSDT");
        assert_eq!(port.holdings[0].quantity, "2");
        assert_eq!(port.total_cost_basis, "100010"); // 2 * 50000 + 10 fee

        let _ = std::fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_engine_initialization_sync_thread_online() {
        // Runs on a regular OS thread with NO pre-existing Tokio runtime
        let temp_dir = std::env::temp_dir().join(format!("omt_sync_test_{}", std::process::id()));
        let engine = MarketEngine::new(temp_dir.clone(), false, None)
            .expect("Engine should auto-create runtime");
        let snap = engine.market_snapshot();
        assert_eq!(snap.total_instruments, 0);
        let _ = std::fs::remove_dir_all(temp_dir);
    }
}
