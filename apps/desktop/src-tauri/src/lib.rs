#![forbid(unsafe_code)]
pub mod commands;

use commands::AppState;
use directories::ProjectDirs;
use omt_service::MarketEngine;
use tauri::Emitter;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let dirs = ProjectDirs::from("org", "OpenMarketTerminal", "omt")
        .expect("Cannot resolve user directories");
    let data_dir = dirs.data_local_dir().to_path_buf();
    let sec_ua = std::env::var("OMT_SEC_USER_AGENT").ok();

    let engine = MarketEngine::new(data_dir, false, sec_ua)
        .expect("Failed to initialize Open Market Terminal engine");

    let engine_for_stream = engine.clone();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_window_state::Builder::default().build())
        .manage(AppState {
            engine: engine.clone(),
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_market_snapshot,
            commands::get_historical_candles,
            commands::search_instruments,
            commands::get_company_facts,
            commands::get_news,
            commands::get_article,
            commands::get_treasury_curve,
            commands::get_portfolio,
            commands::add_transaction,
            commands::delete_transaction,
            commands::execute_terminal_command,
            commands::get_watchlists,
            commands::get_watchlist,
            commands::watch_asset,
            commands::get_setting,
            commands::save_setting,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let mut quote_rx = engine_for_stream.quote_broadcast_tx.subscribe();

            // Background task to emit batched quote updates to webview
            tauri::async_runtime::spawn(async move {
                while let Ok(quotes) = quote_rx.recv().await {
                    let _ = handle.emit("quote_stream", quotes);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
