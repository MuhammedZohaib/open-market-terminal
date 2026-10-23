# Open Market Terminal (OMT) — Repository & UI Audit (`MARKET_UI_AUDIT.md`)

## Executive Summary

An end-to-end code audit of the six workspace crates (`omt-core`, `omt-data`, `omt-providers`, `omt-storage`, `omt-tui`, `omt-app`) was conducted to trace why `MARKET` (`WEI`) and `SCAN` (`EQS`) only display three instruments, how market discovery and streaming operate, how SEC filings and financial statements are parsed and rendered, and where layout and performance bottlenecks exist.

---

## Part 1 — Detailed Audit Findings

### 1. Why `MARKET` / `WEI` Only Displays Three Assets
- **Root Cause Trace**:
  1. `crates/omt-app/src/main.rs:218-226`: On initial startup, if SQLite has no watchlists, OMT seeds the `"default"` watchlist with three symbols: `BTCUSDT`, `ETHUSDT`, and `SOLUSDT`.
  2. `crates/omt-app/src/controller.rs:25-41`: `subscription(&App)` constructs a `Subscription` whose `assets` vector consists solely of `app.watchlist` plus open portfolio positions (`app.transactions`) and the single active chart symbol (`app.asset`).
  3. `crates/omt-providers/src/market.rs:136`: `run_subscription()` immediately calls `state.retain(&sub.assets)`, purging any quote from `MarketState.quotes` that is not in `sub.assets`.
  4. `crates/omt-providers/src/market.rs:260-274`: `stream_once()` only subscribes to `<symbol>@ticker` for `sub.assets`.
  5. `crates/omt-tui/src/views.rs:60-65`: `views::market()` renders `app.market.quotes.values()`, which therefore contains at most the three default watchlist symbols.

### 2. Coupling of Market Universe to the Default Watchlist
- **Finding**: Confirmed defect. There is no separation between:
  - **Watchlist**: User-curated personal monitoring list persisted in SQLite (`watch_items`).
  - **Market Universe**: Exchange-discovered tradable instruments and their latest market-wide statistics.
  - **Visible Viewport**: The slice of sorted/filtered universe rows currently visible in the terminal table.
  - **Active Detailed Subscriptions**: The focused chart kline stream and selected instrument's detailed order-book/ticker feed.
- Even the UI label in `crates/omt-tui/src/views.rs:83` explicitly hardcodes `"Limited universe: active watchlist."`

### 3. Binance Connections and Stream Subscriptions
- **Finding**:
  - `crates/omt-providers/src/market.rs:124-133`: `service()` runs `tokio::select!` over `run_subscription(...)` and `subscriptions.changed()`. Whenever the user changes the active asset (`app.asset`), timeframe (`app.timeframe`), or watchlist, `subscriptions.changed()` fires, **aborting the entire `run_subscription` task** and tearing down all active WebSockets across all venues.
  - `stream_once()` (`market.rs:260-274`) opens a combined stream URL with per-symbol `<symbol>@ticker` streams limited to 50 symbols (`Subscription::normalized` in `crates/omt-data/src/lib.rs:15`).
  - It does not initialize from `GET /api/v3/exchangeInfo` + `GET /api/v3/ticker/24hr`, nor does it subscribe to Binance's exchange-wide `!miniTicker@arr` stream.
  - `mpsc::channel(128)` (`market.rs:143`) uses `tx.try_send()` (`market.rs:328`), silently dropping events under load without incrementing a dropped/coalesced counter.

### 4. Quote Caches and Screener (`SCAN`) Updates
- **Finding**:
  - `MarketState` (`crates/omt-data/src/lib.rs:23`) stores quotes in `BTreeMap<String, Quote>`.
  - Every 33ms (`market.rs:180, 214-216`), `run_subscription` clones the entire `MarketState` into a `watch::Sender`.
  - In `crates/omt-tui/src/views.rs:53, 59-249`, `Workspace::Market` and `Workspace::Scan` both call `views::market()`.
  - On every render pass, `views::market()` collects, filters, and sorts `app.market.quotes.values()` from scratch.
  - In `Workspace::Scan`, the central pane is split between the 3-asset watchlist table (`rows[1]`) and a 10-line bottom box (`rows[2]`) for cached SEC companies. It does not support `SCAN CRYPTO` vs `SCAN EQUITIES` modes or market-wide filtering across thousands of discovered pairs.

### 5. Broader Symbol Discovery Limitations
- **Finding**:
  - `market::catalog()` (`crates/omt-providers/src/market.rs:100-111`) fetches `GET /api/v3/exchangeInfo` once at startup, filters `status == "TRADING"`, and returns only `Vec<String>` (`"BINANCE:<symbol>"`) for command autocomplete. It discards `baseAsset`, `quoteAsset`, `status`, price/quantity precision, and trading permissions.
  - `AssetId::currency()` (`crates/omt-core/src/lib.rs:38-41`) only checks six suffixes (`["USDT", "USDC", "BTC", "ETH", "EUR", "BNB"]`). Pairs quoted in `FDUSD`, `TRY`, `BRL`, `JPY`, `GBP`, `AUD`, etc. resolve to `"UNKNOWN"`.
  - `AssetId::from_str()` (`crates/omt-core/src/lib.rs:70-74`) only routes unprefixed symbols to `Venue::Binance` if they end with `USDT` or `USDC`. Unprefixed crypto pairs like `ETHBTC`, `SOLBTC`, or `BTCFDUSD` are misclassified as `Venue::Iex` equities.

### 6. SEC Filings (`NEWS`) and Financial Facts (`FIN`)
- **SEC Filings (`NEWS`) Trace**:
  - `crates/omt-tui/src/views.rs:372-375`: `filings()` uses `.skip(app.cursor.min(company.filings.len().saturating_sub(1)))` on a plain `List` without `ListState` selection highlighting, and `crates/omt-tui/src/lib.rs:206-217` does not reset `app.cursor` on workspace switches nor bound `app.cursor` to the current list length. If `app.cursor` was incremented in another view (or scrolled down), `NEWS` silently slices off the newest filings from the top of the list (e.g., hiding 2026 filings and starting at an older date).
  - `crates/omt-providers/src/research.rs:178-216`: `parse_filings()` does not extract `reportDate` or `primaryDocDescription` from SEC `submissions/CIK...json`, and does not explicitly sort filings by `(filed, accession)` descending.
  - `crates/omt-app/src/controller.rs:274-281`: Cached `Company` records in SQLite are reused for 6 hours even if cached prior to schema/field additions.
- **SEC Financial Statements (`FIN`) Trace**:
  - `crates/omt-core/src/facts.rs:9-18`: Balance sheet instant facts (`fact.start.is_none()`) have `view == PeriodView::Quarterly || fact.form.starts_with("10-K")`, which mixes 10-K and 10-Q instants without verifying fiscal period alignment.
  - `crates/omt-tui/src/views.rs:277-361`: `financials()` renders a flat vertical list of individual facts sorted by date rather than a comparative multi-period financial statement matrix (metrics as rows, fiscal periods as columns) with statement category views (`Overview`, `Income Statement`, `Balance Sheet`, `Cash Flow`, `Ratios`).

### 7. UI Components Causing Unused Space, Inconsistent Alignment, or Unreadable Data
- **Global Header (`crates/omt-tui/src/lib.rs:297-310`)**: Renders `app.market.status` verbatim. When `app.asset` is an equity (e.g., `IEX:AAPL` in `FIN` or `NEWS`) and `APCA_API_KEY_ID` is unset, `IEX: Set APCA_API_KEY_ID for IEX equities` is displayed in the global header across every workspace.
- **Fixed 3-Column Proportions (`crates/omt-tui/src/lib.rs:326-335`)**: Allocates a 28-col left watchlist and 31-col right panel on all screens regardless of workspace content.
- **Missing Table Selection State (`crates/omt-tui/src/views.rs`)**: Central tables in `MARKET`, `SCAN`, `FIN`, `NEWS`, `PORT`, and `ECON` do not use `StatefulWidget` (`TableState` / `ListState`) or `row_highlight_style`. Instead, scrolling chops off top rows via `.skip(app.cursor)`.
- **`WEI` Naming Misuse (`crates/omt-core/src/command.rs:35`)**: `Workspace::Market.mnemonic()` returns `"WEI"` (World Equity Indices) for a view showing three Binance crypto tickers.
- **`NEWS` Raw URLs (`crates/omt-tui/src/views.rs:376-381`)**: Displays multiline raw URLs instead of a structured tabular blotter with safe browser opening.
- **`PORT` Empty Space (`crates/omt-tui/src/views.rs:457-495`)**: A single holding stretches to fill the entire main table area without an account/allocation summary strip.

### 8. Rendering & Streaming Work Analysis
- **Finding**:
  - `run_subscription` clones the entire `MarketState` every 33ms (`market.rs:215`), and `main.rs:340` clones it again into `App`.
  - `views::market()` re-filters and re-sorts all quotes on every 33ms draw call. With 2,500+ Binance Spot pairs, sorting on every frame both wastes CPU and causes active rankings to jump under the user's selection cursor.
  - `CandleChart::render()` and `views::context_text()` recompute `sma`, `ema`, `bollinger_bands`, `rsi`, `atr`, and `macd` on every frame draw.

### 9. Global Asset Selection Impact Across Workspaces
- **Finding**:
  - Selecting an equity (`FIN AAPL`) overwrites `app.asset`, which triggers `subscription_tx` to restart `run_subscription`, killing the Binance stream and attempting an IEX stream connection.
  - Pressing `Enter` in the central panel (`focus == 1`) of `MARKET` or `SCAN` does not open the selected instrument in `CHART` or `FIN`.

### 10. Timestamp & Feed Latency Instrumentation
- **Finding**:
  - `MarketState` only records `accepted`, `rejected`, and `last_ingest_micros`.
  - There is no tracking of P50/P95/P99 internal update latency, terminal redraw duration percentiles, exchange-vs-local wall-clock skew detection, coalesced/dropped event counts, or reconnection counts.
  - `Quote::status_at()` marks any crypto quote with `now - event_at > 15s` as `Stale`, conflating quiet trading pairs with a disconnected feed.

---

## Part 2 — Implementation Priorities & Execution Plan

1. **Decouple Watchlists, Market Universe, Viewport, and Detailed Subscriptions (`omt-core`, `omt-data`)**:
   - Introduce `InstrumentInfo` (symbol, base_asset, quote_asset, status, price_precision, qty_precision, venue, asset_class).
   - Separate `Watchlist` (persisted user list), `MarketUniverse` (exchange-wide instruments + quotes + indexed views), `Viewport` (virtualized table state + stable selected instrument key), and `DetailedSubscription` (active chart + focused detail symbol).
2. **Full Binance Spot Discovery & Exchange-Wide Streaming (`omt-providers`)**:
   - Fetch `GET /api/v3/exchangeInfo` and `GET /api/v3/ticker/24hr` at startup and periodic reconciliation intervals.
   - Stream `!miniTicker@arr` on a persistent WebSocket connection independent of chart symbol/timeframe changes.
   - Maintain a separate lightweight subscription channel for the active chart kline (`<symbol>@kline_<tf>`) and selected instrument `<symbol>@bookTicker` / `<symbol>@ticker` so changing chart symbols never drops the exchange-wide market stream.
3. **Fix Workspace Semantics (`MARKET`, `CRYPTO`, `WEI`, `SCAN`)**:
   - Make `MARKET` the multi-asset overview & high-liquidity market monitor, `CRYPTO` (command/mode) the full Binance Spot universe explorer (`USDT`, `USDC`, `BTC`, `ALL`, etc.), and `WEI` a dedicated World Equity Indices view with honest source/coverage attribution.
   - Upgrade `SCAN` to support `SCAN CRYPTO` (full Binance Spot universe with price, 24h%, quote volume, quote asset, and status filters) and `SCAN EQUITIES` (local SEC EDGAR + equity index with explicit coverage counts).
4. **Repair SEC Filings & Comparative Financial Statements (`omt-core`, `omt-providers`, `omt-tui`)**:
   - Extract `reportDate` and `primaryDocDescription` in `parse_filings`, sort newest-first by `(filed, accession)`, and bound table cursor navigation properly.
   - Upgrade `FIN` to render comparative multi-period financial statement tables (Metrics × Fiscal Periods) across `Overview`, `Income Statement`, `Balance Sheet`, `Cash Flow`, and `Ratios`, with XBRL provenance in the right detail pane.
   - Make feed status workspace-specific and move optional Alpaca/FRED credential setup guidance to `SETTINGS`.
5. **Bloomberg-Inspired UI Redesign & Performance Engineering (`omt-tui`, `omt-app`)**:
   - Centralize the visual theme (`#0B0E12` background, `#10151B` panel, `#171D25` elevated surface, `#F4B544` command amber, `#4AC69A` positive, `#EB6B78` negative, `#253649` selection).
   - Redesign header, `GO >` command bar, context-sensitive footer, and workspace-adaptive panel layouts.
   - Virtualize large tables, stabilize selection by `AssetId::key()`, decouple sort order refreshes from raw incoming ticks, and instrument monotonic P50/P95/P99 ingest & draw metrics plus clock-skew detection.
