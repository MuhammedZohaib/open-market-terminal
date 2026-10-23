# Open Market Terminal — implementation plan

## Architecture
Rust 2024 Cargo workspace: `omt-core` owns decimal financial models, commands and accounting; `omt-providers` owns HTTP/WebSocket normalization; `omt-data` coalesces subscriptions and rejects old updates; `omt-storage` owns versioned SQLite and cache; `omt-tui` owns state, keyboard routing and Ratatui rendering; `omt-app` wires background workers, configuration and terminal lifecycle. No network work runs in rendering. Bounded channels and latest-value snapshots limit memory.

## Dependencies
Tokio for asynchronous I/O; Ratatui 0.30/Crossterm 0.29 for native terminal rendering; Reqwest/rustls and tokio-tungstenite for HTTP/WebSocket; Serde/JSON/TOML for formats; rusqlite with bundled SQLite for portable embedded storage (build-time C compiler only); rust_decimal for accounting; chrono for UTC/date semantics; clap for CLI; anyhow/thiserror for error boundaries; tracing for diagnostics; directories for platform paths; CSV for transaction interchange. Pin resolved compatible releases in Cargo.lock and audit resolved license metadata. Use stable Rust. No runtime web server, browser, Python, Node, or database service.

## Providers and rights
Binance public market-data endpoints: venue-specific crypto ticker, exchange catalog, bars and live klines. Coinbase provides explicit alternate venue quotes/candles; never substitute assets silently. SEC requires a maintained contact User-Agent, paced requests and conservative XBRL selection. Treasury/World Bank are official dated macro sources. Alpaca IEX and FRED require environment credentials. Open-source code licensing does not grant rights to redistribute upstream data. Cache privately on the user's machine; provider terms govern use. Provider coverage and validation status will be recorded in docs/PROVIDERS.md.

## Ordered milestones
1. Runnable foundation: workspace, configuration, responsive terminal, commands, input and terminal restoration.
2. Real market slice: public Binance streaming, bounded/coalesced state, REST candles, candle/volume/SMA rendering, reconnect and offline states.
3. Persistence/company research: migration, watchlists, private cache, SEC identity/facts/filings; optional equity integration.
4. Research: limited local screener, macro series, decimal average-cost portfolio, atomic CSV import/export.
5. Release quality: tests, measured benchmark harness, CI for three operating systems, docs, release build and honest final report.

Each milestone must pass formatting, compilation, strict Clippy and relevant tests before being marked verified. Prefer a correct vertical slice. Incomplete capabilities must be explicitly disclosed.

## Measurement
Use monotonic Instant timings for ingestion and drawing; deterministic synthetic input exists only in tests/benchmarks. Measure headless rendering/startup separately from real terminal visibility. Report workload, machine, build mode and percentile method; never infer exchange latency. Check real public feed and historical response independently. Test offline startup and bounded update behavior.

## Risks and limitations
Repository initially empty; Rust and native compiler absent from PATH. Installing a user-local GNU toolchain before verification. Public feeds may be region-blocked; SEC live testing requires actual maintained user contact, never fabricated identification. No financial metrics from incompatible dates/currencies, no synthetic production prices, no implied comprehensive equity coverage. Cross-platform CI is configured here but remote CI results cannot be claimed locally. Precision, source lineage and unavailable states take priority over breadth.

## Progress
- Initial inspection complete. Empty directory; implementation started.
- Foundation compiled and terminal snapshots rendered at 140x40, 80x24, 28x9 and 12x4. Input isolation, command history, function keys (`F1`–`F8`), Bloomberg mnemonics (`GP`, `GIP`, `FA`, `DES`, `CN`, `EQS`, `PRTU`, `ECST`, `WEI`, `YCRV`, `<GO>`), and navigation tests pass. Local GCC resolved the initial LLVM/GNU linker mismatch.
- Market vertical slice verified with an actual Binance WebSocket quote and 500 historical BTCUSDT hourly candles. The real chart snapshot includes volume, selectable overlays (`SMA20/50`, `EMA12/26`, `Bollinger Bands`), quantitative indicators (`RSI14`, `ATR14`, `MACD`, `VWAP`, window drawdown/volatility), bid/ask spread metrics, and source timestamps.
- Persistence, 14-concept SEC US-GAAP XBRL adapter/ratios/YoY growth, decimal average-cost portfolio with recent fills blotter and validated transaction deletion (`DELTX`), atomic CSV import/export, local screening (`SCAN`/`EQS`), and official macro adapters (`TREASURY` with `2s10s`/`3m10s`/`5s30s` spreads, `WB`, `FRED`) implemented and tested. SEC/Alpaca/FRED live checks remain dependent on supplied contact or credentials and are not claimed as live-verified without user credentials.
- Release quality complete: cross-platform CI and release workflows configured, license inventory (`docs/dependency-licenses.json` and `THIRD_PARTY_NOTICES.md`) generated, 41 workspace unit/integration tests passing with zero Clippy warnings, live Binance/Coinbase/Treasury/World Bank checks completed, captured frame saved to `docs/terminal-frame.txt`, and measured benchmark documented in `docs/PERFORMANCE.md` and `docs/IMPLEMENTATION_REPORT.md`.
