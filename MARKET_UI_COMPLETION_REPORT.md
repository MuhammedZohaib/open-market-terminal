# Open Market Terminal (`omt`) — Market Discovery, Streaming, and Bloomberg-Inspired UI Completion Report

## 1. Audit Summary

Prior to this refinement and completion milestone, a comprehensive repository audit ([`MARKET_UI_AUDIT.md`](file:///c:/work/bbt/MARKET_UI_AUDIT.md)) traced 10 core architectural and UI areas across `omt-core`, `omt-data`, `omt-providers`, `omt-storage`, `omt-tui`, and `omt-app`:

1. **Three-Asset Market View**: `MARKET` and `SCAN` iterated exclusively over `app.market.quotes`, which was populated only by the 3-asset default watchlist (`BTCUSDT`, `ETHUSDT`, `SOLUSDT`) and actively purged via `state.retain(&sub.assets)` whenever subscriptions changed.
2. **Watchlist vs. Market Universe Coupling**: There was no separation between personal watchlists, provider-discovered market universes, visible table viewports, and selective detail subscriptions.
3. **Binance Connectivity**: The provider subscribed only to per-asset `<symbol>@ticker` streams for watched assets on a single combined WebSocket URL, tearing down and reconnecting the entire feed whenever the user switched chart symbols or timeframes.
4. **Quote Caches & Screener Scope**: `SCAN` filtered only watched quotes (`app.market.quotes`) or locally cached companies without exposing `SCAN CRYPTO` vs. `SCAN EQUITIES` modes or dynamic universe coverage metrics.
5. **Symbol Discovery**: `AssetId::from_str` recognized only `USDT` and `USDC` suffixes for Binance and rejected pairs quoted in `BTC`, `ETH`, `BNB`, `FDUSD`, `EUR`, `TRY`, `BRL`, and `JPY` unless manually prefixed.
6. **SEC Filings & Financial Facts**:
   - `NEWS AAPL` appeared to start at `2026-01-02` because `views::filings` sliced `.skip(app.cursor)` without resetting `app.cursor` on workspace switch (skipping the 9 newer 2026 `10-Q` and `8-K` filings when `cursor == 9`), and `parse_filings` dropped `DEF 14A`, `SD`, `SCHEDULE 13G`, `Form 4`, and any `primaryDocument` containing an XSL subdirectory slash (e.g., `xslF345X06/form4.xml`).
   - `FIN` extracted 14 US-GAAP concepts and rendered them as a flat single-fact-per-row list rather than a multi-period comparative financial statement matrix.
7. **UI Density & Visual Hierarchy**: Equal 3-column splits (`24% / 50% / 26%`), raw `Paragraph` lists without `TableState` row highlights, unconfigured Alpaca credential warnings in the global header, and a hidden command bar reduced usability and visual density.
8. **Render Cadence & Virtualization**: While `service()` coalesced at 33 ms, `views.rs` formatted and sorted all rows on every draw without virtualized viewport slicing or pre-extracted sort keys.
9. **Selection Stability**: Keyboard selection used a raw integer `app.cursor`, jumping to unrelated instruments whenever live streaming updates re-ranked sorted tables.
10. **Latency & Clock Skew**: `Quote::status_at` marked any crypto quote older than 15 seconds as `Stale` and >60 seconds as `Disconnected`, falsely marking less-frequently traded pairs as disconnected even while the WebSocket feed was live.

---

## 2. Root Cause of the Three-Asset Market View & Resolution

### Root Cause
1. At first launch (`crates/omt-app/src/main.rs`), `omt` seeded the `"default"` watchlist with `["BTCUSDT", "ETHUSDT", "SOLUSDT"]`.
2. `subscription(&App)` built `Subscription.assets` solely from `app.watchlist`, portfolio transactions, and `app.asset`, truncating at 50 assets.
3. `market::service` subscribed only to `sub.assets` and called `state.retain(&sub.assets)` on every subscription change, deleting any non-watchlist quotes from `MarketState.quotes`.
4. `views::market` and `views::scan` rendered `app.market.quotes` directly.

### Resolution
We separated state and lifecycles into four distinct architectural layers:
- **Personal Watchlist (`app.watchlist`)**: User-curated instruments persisted in SQLite (`watchlist` table), rendered in the left Watchlist pane and manageable via `w` or `:WATCH ADD/REMOVE`.
- **Provider Market Universe (`MarketState.instruments` & `MarketState.quotes`)**: Full symbol metadata discovered from `GET /api/v3/exchangeInfo` (`3,696` symbols, `1,383` actively `TRADING`) and 24-hour market statistics from `GET /api/v3/ticker/24hr`, updated continuously via `!miniTicker@arr` without `state.retain()` pruning.
- **Virtualized Table Viewport**: Only the visible window slice (`visible_cap` rows around the active selection) is materialized into Ratatui `Row`/`Cell` widgets during rendering.
- **Selective Detail Subscriptions (`Subscription.assets` & `Subscription.chart`)**: Capped at 50 instruments for `<chart>@kline_<tf>`, `<chart>@ticker`, and up to 12 `<symbol>@bookTicker` streams so thousands of discovered symbols never spawn thousands of individual WebSocket connections.

---

## 3. Binance Discovery and Streaming Architecture

Implemented in [`crates/omt-providers/src/market.rs`](file:///c:/work/bbt/crates/omt-providers/src/market.rs) and [`crates/omt-data/src/lib.rs`](file:///c:/work/bbt/crates/omt-data/src/lib.rs):

1. **Full Symbol Discovery (`exchange_info` / `parse_exchange_info`)**:
   - Queries `GET https://data-api.binance.vision/api/v3/exchangeInfo`.
   - Normalizes `symbol`, `baseAsset`, `quoteAsset`, `status` (`TRADING`, `HALT`, `BREAK`), `PRICE_FILTER.tickSize` (`price_precision`), and `LOT_SIZE.stepSize` (`quantity_precision`) into `InstrumentInfo`.
   - Recognizes all Binance Spot quote currencies (`USDT`, `USDC`, `FDUSD`, `BTC`, `ETH`, `BNB`, `EUR`, `TRY`, `BRL`, `JPY`, `DAI`, `TUSD`, `GBP`, `AUD`, `PLN`, `ARS`, `ZAR`).
   - Distinguishes distinct market instruments (e.g., `BINANCE:BTCUSDT`, `BINANCE:BTCUSDC`, `BINANCE:BTCFDUSD`, and `COINBASE:BTC-USD`).
2. **Initial 24-Hour Snapshot & Periodic Reconciliation (`tickers_24hr` / `parse_tickers_24hr`)**:
   - Queries `GET https://data-api.binance.vision/api/v3/ticker/24hr` on startup and reconciles every 5 minutes (respecting Binance request weights and `Retry-After` cooldowns).
   - Populates initial `price`, `open_24h`, `change_abs`, `change_pct`, `high`, `low`, `volume` (base), `quote_volume`, `bid`, `ask`, and `vwap`.
3. **Persistent Exchange-Wide Streaming (`stream_binance_aggregate` / `binance_mini_ticker_arr`)**:
   - Connects to `wss://data-stream.binance.vision/ws/!miniTicker@arr` in a dedicated background task that remains connected across chart symbol and timeframe changes.
   - Normalizes incoming `24hrMiniTicker` arrays (`c`, `o`, `h`, `l`, `v`, `q`, `E`), computes exact rolling 24-hour `change_abs = c - o` and `change_pct = (c - o) / o * 100`, preserves uncrossed `@bookTicker` `bid`/`ask` and snapshot `vwap`, and updates `MarketState.quotes` for all changed symbols while preserving unchanged instruments from the snapshot.
   - `Quote::status_at` uses a 360-second window against `received_at.max(event_at)` for Binance Spot so quiet trading pairs on a healthy feed remain `Live` between 5-minute snapshot reconciliations rather than falsely flipping to `Disconnected`.
4. **Decoupled Selective Detail Streams (`stream_once`)**:
   - Subscribes to `<chart>@kline_<tf>`, `<chart>@ticker`, and up to 12 `<symbol>@bookTicker` streams on a separate task that can cleanly re-subscribe when the user changes the chart asset or timeframe.

---

## 4. Bloomberg-Inspired UI/UX Redesign Summary

Implemented in [`crates/omt-tui/src/theme.rs`](file:///c:/work/bbt/crates/omt-tui/src/theme.rs), [`crates/omt-tui/src/lib.rs`](file:///c:/work/bbt/crates/omt-tui/src/lib.rs), and [`crates/omt-tui/src/views.rs`](file:///c:/work/bbt/crates/omt-tui/src/views.rs):

- **Centralized Visual System (`theme.rs`)**:
  - Uses explicit workstation tokens: `#0B0E12` (`BG`), `#10151B` (`PANEL_BG`), `#171D25` (`ELEVATED_BG`), `#343B45` (`BORDER`), `#E3E7ED` (`FG`), `#98A3B3` (`SECONDARY`), `#677586` (`MUTED`), `#F4B544` (`AMBER`), `#4AC69A` (`POSITIVE`), `#EB6B78` (`NEGATIVE`), and `#253649` (`SELECTION`).
  - Financial gains and losses include explicit `+` / `-` signs alongside green/red color semantics.
- **Compact Global Status Header & `GO >` Command Bar**:
  - Row 0 displays `OMT`, active workspace + mnemonic (`MARKET <MKT>`, `CHART <GP>`, `FIN <FA>`, `NEWS <CN>`, `SCAN <EQS>`, `PORT <PRTU>`, `ECON <ECST>`), active instrument quote badge, workspace-specific provider health (without irrelevant Alpaca credential warnings), and UTC clock.
  - Row 1 displays compact function tabs (`1:MARKET[MKT]` .. `7:ECON[ECST]`) and a prominent `GO >` command entry bar supporting Bloomberg-style `<SYMBOL> <FUNCTION> <GO>` and `<FUNCTION> <SYMBOL> <GO>` syntax with autocomplete and history.
- **Corrected `MKT` / `CRYPTO` / `WEI` Semantics**:
  - `MARKET` (`MKT`) provides the multi-asset market overview (defaulting to high-liquidity `USDT` Spot pairs with instant切换 to `USDC`, `BTC`, `FDUSD`, `EUR`, or `ALL`).
  - `CRYPTO [ALL|USDT|USDC|BTC]` opens the full Binance Spot universe explorer.
  - `WEI` opens the dedicated **World Equity Indices** view displaying regional benchmarks (`SPX`, `NDX`, `INDU`, `RTY`, `SX5E`, `UKX`, `DAX`, `CAC`, `NKY`, `HSI`, `AS51`, `SHSZ300`), their ETF proxies (`IEX:SPY`, `IEX:QQQ`, etc.), and an explicit policy banner stating when live index/IEX feeds are unconfigured rather than fabricating index levels or showing 3 crypto pairs under the `WEI` label.
- **Market-Wide `MARKET` & `SCAN` Workspaces**:
  - Dynamic coverage bar: `Binance Spot | 3696 discovered pairs (1383 trading) | 516 match filter | 26 visible`.
  - Interactive filter pills (`u` Quote Currency, `l` Min 24h Quote Volume, `x` Trading Status, `s` Sort Column, `o` Asc/Desc Order, `m` Mode) plus breadth metrics (Advancers / Decliners / Unchanged / 24h USDT Turnover) and Top Gainers / Top Losers / Most Active auxiliary panels.
  - Stable selection by instrument ID (`selected_market_key`) so streaming price updates and sort flips never jump the user's selection to a different asset.
- **Enhanced `CHART`, `PORT`, and `ECON` Workspaces**:
  - `CHART` adds an OHLCV / 24h header strip, interactive candle crosshair cursor (`[` and `]`), and full-width toggle (`z`).
  - `PORT` adds an Account Summary bar (Market Value, Cost Basis, Unrealized P&L, Realized P&L, Priced Positions) above the virtualized Holdings and Fills Blotter tables.
  - `ECON` pairs the proportional-maturity Treasury yield curve chart (`2s10s`, `3m10s`, `5s30s` spreads and curve regime) with an interactive Maturity & Observation `Table` (`j`/`k` to inspect each tenor).

---

## 5. SEC Filing and Financial-Statement Improvements

Implemented in [`crates/omt-providers/src/research.rs`](file:///c:/work/bbt/crates/omt-providers/src/research.rs) and [`crates/omt-core/src/facts.rs`](file:///c:/work/bbt/crates/omt-core/src/facts.rs):

1. **`NEWS` (SEC Filings) Root-Cause Fix & Redesign**:
   - Reset `app.cursor = 0` on workspace/company transitions and replaced the raw paragraph slice with a stateful virtualized `Table` (`FILED DATE`, `FORM`, `DESCRIPTION / ITEMS`, `REPORT PERIOD`, `DOCUMENT / SOURCE`).
   - Expanded `parse_filings` to support 27 corporate, proxy, registration, beneficial ownership, and insider forms (`10-K`, `10-Q`, `8-K`, `20-F`, `40-F`, `6-K`, `DEF 14A`, `DEFA14A`, `PRE 14A`, `SD`, `S-1`, `S-3`, `S-4`, `S-8`, `424B2`, `424B5`, `SC 13G`, `SC 13G/A`, `SCHEDULE 13G`, `SCHEDULE 13G/A`, `SC 13D`, `SC 13D/A`, `SCHEDULE 13D`, `SCHEDULE 13D/A`, `3`, `4`, `144` plus `/A` amendments).
   - Preserved `reportDate`, `primaryDocDescription`, and single-level XSL document paths (`xslF345X06/form4.xml`), sorted newest-first by `(filed, accession)`.
   - Added form category filter (`f`: `ALL`, `10-K/10-Q`, `8-K`, `PROXY`, `INSIDER`) and safe browser launch (`Enter` / `o` / `:OPEN`).
2. **`FIN` (Financial Statements) Comparative Matrix**:
   - Expanded US-GAAP normalization from 14 to 29 concepts across 5 statement tabs (`OVERVIEW`, `INCOME`, `BALANCE`, `CASH`, `RATIOS`), switchable via `s`, `[` / `]`, or `:FIN AAPL BALANCE`.
   - Renders up to 5 comparative reporting periods side-by-side (`FY2025`, `FY2024`, `FY2023`, etc. or `Q1..Q4` 10-Q columns) with a `YoY %` column and `←`/`→` period column selector.
   - Enforces strict period matching so comparative prior-year instant facts inside a `10-K` do not leak into standalone `10-Q` quarterly columns.

---

## 6. Performance Metrics and Benchmark Methodology

Measured in release mode (`cargo run --release -p omt-tui --example benchmark` and `target/release/omt.exe` on Windows x86_64 GNU toolchain):

| Metric | Measured Value | Notes |
|---|---|---|
| **Cold startup (`--offline --snapshot`)** | **`307.68 ms`** | Process launch, SQLite WAL init, cached universe load, and first `140x40` render |
| **Warm startup (`--offline --snapshot`)** | **`199.16 ms`** | Warm OS page cache startup to full frame output |
| **Memory: 3 watched assets** | **`1,272 B`** | Heap footprint of 3 `Quote` entries |
| **Memory: 3,500-instrument universe** | **`2,330,240 B` (`~2.22 MiB`)** | 3,500 `InstrumentInfo` + 3,183 active `Quote` entries |
| **Universe batch throughput** | **`435,597 updates/sec`** | Validation + `BTreeMap` ingestion rate across full universe |
| **Quote ingest P50 / P95 / P99** | **`1.20 µs` / `2.30 µs` / `4.70 µs`** | Monotonic `Instant` per-quote ingestion latency (`25,000` samples) |
| **`CHART` draw P50 / P95 / P99** | **`2.61 ms` / `3.56 ms` / `3.98 ms`** | Full `140x40` candlestick + volume + 6 indicators (`500` frames) |
| **`MARKET` (3,500 rows) draw P50 / P95 / P99** | **`4.61 ms` / `7.29 ms` / `8.10 ms`** | Full `140x40` filter + pre-extracted sort + virtualized table + movers (`200` frames) |
| **Navigation under streaming load P95** | **`5.85 ms` (`5,854.6 µs`)** | Keyboard cursor move + sort-order flip + full frame draw during live batch ingestion |
| **Batch receive-to-buffer P95** | **`3.59 ms`** | 50 quote ingestions + 1 full `140x40` frame draw |

---

## 7. Automated Test Results

All **48 automated unit and integration tests** across all 6 workspace crates pass with zero warnings:

- **`omt-core` (11 unit tests)**: Command & Bloomberg mnemonic parsing (`MKT`, `CRYPTO ALL`, `WEI`, `SCAN CRYPTO`, `EQS`, `FIN AAPL BALANCE`, `MARKET GAINERS`), venue & quote-currency identity (`BTCUSDT`, `ETHBTC`, `BTCFDUSD`, `COINBASE:BTC-USD`), annual vs. quarterly balance-sheet separation & comparative multi-period matrix across all 5 statement tabs, YTD rejection, amendment/conflict handling, technical indicators (`SMA`, `EMA`, `BB`, `RSI`, `ATR`, `MACD`, `VWAP`), yield-curve analytics, and portfolio average-cost accounting.
- **`omt-data` (3 unit tests)**: Subscription deduplication, bounded candle history, closed-candle immutability, duplicate/out-of-order/future quote rejection, and `@bookTicker` top-of-book enrichment.
- **`omt-providers` (9 unit tests + 9 integration tests in `http_failures.rs` & `normalization.rs`)**: Full `exchangeInfo` parsing (including `TRADING` vs `BREAK` status and tick/lot precision), 24h ticker snapshot filtering, `!miniTicker@arr` WebSocket array streaming and snapshot reconciliation, non-watchlist symbol retention, WebSocket ping/pong/close lifecycle, SEC filing freshness & newest-first sorting (including 2026 filings, `DEF 14A`, `Form 4` XSL paths, and path-traversal rejection), US Treasury negative/null XML parsing, and HTTP 429 `Retry-After` cooldown enforcement.
- **`omt-storage` (1 unit test + 5 integration tests in `persistence.rs`)**: SQLite WAL persistence, watchlists, cache timestamp preservation, atomic CSV import/export, and schema forward-compatibility.
- **`omt-tui` (7 unit tests)**: Virtualized rendering of 3,500 instruments in `MARKET` and `SCAN`, quote-currency filtering (`USDT`, `USDC`, `ALL`), exchange status filtering (`TRADING` vs `BREAK`), stable keyboard selection (`selected_market_key`) across sort-order flips, numeric formatting with thousands separators, `WEI` honesty verification, full-width toggle (`z`), ASCII fallback rendering, and responsive terminal resizing (`140x40`, `80x24`, `28x9`, `12x4`).
- **`omt-app` (3 unit/integration tests)**: End-to-end controller command routing (`CRYPTO ALL`, `WEI`, `EQS`, `SCAN CRYPTO`, `QUOTE`, `MINVOL`, `STATUS`, `FILTER`, `SORT`, `NEWS AAPL` cursor reset, `FIN AAPL BALANCE`, `BACK`), and strict TOML configuration validation.

---

## 8. Remaining Provider Limitations

1. **World Equity Indices (`WEI`)**: Proprietary real-time index levels (`SPX`, `NDX`, `UKX`, `NKY`, `DAX`, `HSI`) require commercial exchange licensing and are not available from unauthenticated public feeds. `WEI` displays regional benchmark metadata and maps US ETF proxies (`IEX:SPY`, `IEX:QQQ`, `IEX:DIA`, `IEX:IWM`, etc.) when Alpaca IEX credentials are configured, while explicitly reporting `Unavailable (no index/IEX feed)` when unconfigured.
2. **US Equities (`SCAN EQUITIES` / `IEX`)**: Real-time IEX equity trades/bars require user-supplied `APCA_API_KEY_ID` and `APCA_API_SECRET_KEY` environment variables and cover the IEX venue only (not consolidated SIP tape). `SCAN EQUITIES` screens locally cached SEC EDGAR filers (`INDEX <TICKER>` or `:FIN <TICKER>`) rather than issuing thousands of startup requests against SEC EDGAR.
3. **SEC EDGAR Fair Access**: Requires a user-configured contact string via `OMT_SEC_USER_AGENT` or `sec_user_agent` in `config.toml`, throttled to <= 4 requests/second (`250ms` minimum spacing).

---

## 9. Verification Commands and Output Summaries

```powershell
# 1. Formatting check (0 diffs)
powershell -ExecutionPolicy Bypass -File scripts/dev.ps1 fmt --all -- --check

# 2. Strict Clippy lint gate across all workspace targets (0 warnings)
powershell -ExecutionPolicy Bypass -File scripts/dev.ps1 clippy --locked --workspace --all-targets -- -D warnings

# 3. Full automated regression test suite (48 passed, 0 failed)
powershell -ExecutionPolicy Bypass -File scripts/dev.ps1 test --locked --workspace

# 4. Release benchmark harness
powershell -ExecutionPolicy Bypass -File scripts/dev.ps1 run --release -p omt-tui --example benchmark
# Output:
# mem_3_watched_bytes=1272
# mem_3500_universe_bytes=2330240
# universe_throughput_updates_per_sec=435597
# ingest_p50_us=1.200 ingest_p95_us=2.300 ingest_p99_us=4.700
# draw_chart_p50_ms=2.607 draw_chart_p95_ms=3.556 draw_chart_p99_ms=3.977
# draw_market_3500_p50_ms=4.608 draw_market_3500_p95_ms=7.286 draw_market_3500_p99_ms=8.097
# nav_under_load_p95_us=5854.600

# 5. Live Binance Spot discovery + kline + WebSocket probe
.\target\release\omt.exe --probe
# Output:
# Binance verified: 3696 symbols (1383 trading), 500 historical candles; 81421.40000000 USDT at 2026-10-23 19:06:18.016 UTC (received 2026-10-23 19:06:17.763730400 UTC).

# 6. Live 140x40 terminal snapshot capture
.\target\release\omt.exe "MARKET USDT" --snapshot --wait-seconds 8 > docs/terminal-frame.txt
```
