# Implementation Report

## Summary

Open Market Terminal (`omt`) is a native, local-first financial research terminal
written in Rust 2024 (`#![forbid(unsafe_code)]` across all six workspace crates).
It runs as a standalone terminal executable using Tokio, Ratatui 0.30, Crossterm
0.29, Reqwest/rustls, `tokio-tungstenite`, `rusqlite` (bundled SQLite with WAL),
and `rust_decimal`. No runtime browser, web server, Python/Node process, external
database daemon, or telemetry exists.

## Workspace architecture

| Crate | Responsibility |
|---|---|
| `omt-core` | Venue-qualified `AssetId`, `Quote` validation & bid/ask spread math, `Candle` validation, quantitative indicators (`SMA`, `EMA`, `Bollinger Bands`, `RSI`, `MACD`, `ATR`, `VWAP`, window drawdown/volatility), Treasury yield curve spread analytics, conservative SEC XBRL fact selection & fundamental ratios, decimal average-cost portfolio accounting, and command/Bloomberg-mnemonic parser |
| `omt-data` | Subscription normalization (50-instrument cap), monotonic quote ingestion, out-of-order/future timestamp rejection, closed-bar immutability, and 500-candle retention |
| `omt-providers` | HTTP client with per-host `429`/`418` `Retry-After` cooldowns, Binance Spot REST/WebSocket (`data-api.binance.vision` / `data-stream.binance.vision`), Coinbase Exchange REST/WebSocket, Alpaca IEX REST/WebSocket, SEC EDGAR ticker resolution/facts/submissions (250ms paced), US Treasury daily par-yield XML, World Bank Indicators API, and FRED API |
| `omt-storage` | Versioned SQLite (`user_version = 1`, WAL, foreign keys), watchlists, settings, dated provider cache, atomic transaction ledger additions/deletions, and atomic CSV import/export |
| `omt-tui` | Responsive terminal layout (`140x40`, `80x24`, `<110` single-pane focus mode, `<28x9` resize guard, `ascii = true` fallback), seven workspaces (`MARKET`, `CHART`, `FIN`, `NEWS`, `SCAN`, `PORT`, `ECON`), command history, and headless benchmark harness |
| `omt-app` | CLI (`--offline`, `--config`, `--data-dir`, `--snapshot`, `--wait-seconds`, `--probe`), panic-safe terminal restoration guard, background worker supervision, and periodic persistence |

## Live-verified capabilities

1. **Binance Spot Public Market Data (`--probe` and `--snapshot`)**:
   - Verified live WebSocket `24hrTicker` and `kline` streams plus 500-bar REST
     history across `BTCUSDT`, `ETHUSDT`, and `SOLUSDT`.
   - Captured real 140x40 terminal frames in [docs/terminal-frame.txt](terminal-frame.txt).
2. **Coinbase Exchange Public Market Data**:
   - Verified explicit `COINBASE:BTC-USD` live WebSocket ticker and REST candle
     retrieval without cross-venue substitution.
3. **US Treasury Yield Curve (`TREASURY` / `YCRV`)**:
   - Verified live XML retrieval and parsing of daily published US Treasury par
     yields (`1M` through `30Y`), excluding display aliases (`BC_30YEARDISPLAY`),
     with automatic `2s10s`, `3m10s`, and `5s30s` basis-point spread calculation.
4. **World Bank Indicators (`WB`)**:
   - Verified live retrieval and chronological plotting of annual country
     indicators (e.g., `WB US NY.GDP.MKTP.CD`).
5. **Offline Mode, SQLite Persistence, and Portfolio Ledger**:
   - Verified offline startup from cached SQLite quotes/candles/macro series,
     multi-account decimal average-cost accounting, fee capitalization/deduction,
     atomic CSV export/import, and ledger re-validation on `DELTX`.
6. **Quality Gates**:
   - `cargo fmt --all -- --check`: passed
   - `cargo clippy --locked --workspace --all-targets -- -D warnings`: passed (0 warnings)
   - `cargo test --locked --workspace`: 41 unit and integration tests passed

## Credential-dependent integrations (implemented and fixture-tested)

The following adapters are implemented and verified against deterministic unit
and local socket/fixture tests, but require user-supplied credentials or contact
information at runtime:

- **SEC EDGAR (`FIN` / `FA`, `NEWS` / `CN`, `INDEX`)**: Requires
  `OMT_SEC_USER_AGENT` (or `sec_user_agent` in `config.toml`) set to a real
  application name and maintained contact email per SEC fair-access rules.
  Placeholder domains (`example.*`) are rejected.
- **Alpaca US Equities (`IEX:*`)**: Requires `APCA_API_KEY_ID` and
  `APCA_API_SECRET_KEY` in the environment. Coverage is explicitly labeled as
  `IEX only (not consolidated)`.
- **FRED (`FRED <SERIES>`)**: Requires `FRED_API_KEY` in the environment.

## Known limitations and non-goals

- **No fabricated or interpolated data**: Missing metrics, unentitled feeds, or
  conflicting same-filing XBRL values remain `unavailable` / `--`, never `0`.
- **No silent cross-venue or cross-currency mixing**: `BINANCE:BTCUSDT`,
  `COINBASE:BTC-USD`, and `IEX:AAPL` are distinct assets; `USDT` and `USD` are
  never combined in portfolio totals or allocations.
- **Conservative XBRL subset**: Only direct annual (330–380 days) and direct
  quarterly (70–105 days) USD US-GAAP facts for the 14 configured concepts are
  selected. Year-to-date flows are not treated as quarters, and Q4 is not
  synthetically derived from 10-K minus 9M.
- **Explicit local screener**: `SCAN` (`EQS`) filters the active crypto watchlist
  and companies explicitly fetched into the local SQLite cache via `FIN`, `NEWS`,
  or `INDEX <TICKER>`. It does not bulk-crawl all SEC registrants on startup.
