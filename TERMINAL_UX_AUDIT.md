# Open Market Terminal (OMT) — Terminal UX, Chart & News Repository Audit (`TERMINAL_UX_AUDIT.md`)

**Audit Date:** 2026-10-24  
**Scope:** Root-cause trace of the half-empty candlestick chart, workspace layout rigidity, command/context workflow gaps, and SEC-only `NEWS` view across `omt-core`, `omt-data`, `omt-providers`, `omt-storage`, `omt-tui`, and `omt-app`.

---

## 1. Root Cause of the Half-Empty Chart (`CHART` / `GP`)

### 1.1 Fixed `app.zoom` Cap Overrides Viewport Capacity
- **Location:** `crates/omt-tui/src/lib.rs:172` and `crates/omt-tui/src/views.rs:2025-2029`
- **Code Trace:**
  ```rust
  let end = app.market.candles.len().saturating_sub(app.offset);
  let count = app
      .zoom
      .min((inner.width.saturating_sub(14) / 2) as usize)
      .min(end);
  ```
- **Finding:** `App::default()` initialized `zoom: 70` as a static candle count limit (and pressing `+` reduced `zoom` by 10 down to 40). On a wide terminal (e.g., 180 or 240 columns where `inner.width` is 178–238 columns and can fit 82–112 candles at 2 cells/candle, or 164–224 candles at 1 cell/candle), `count` was clamped to `app.zoom` (40–70 bars), leaving over half of the horizontal plotting area unused.

### 1.2 Left-Aligned Candle Rendering Instead of Right-Anchored Time Axis
- **Location:** `crates/omt-tui/src/views.rs:2128-2129`
- **Code Trace:**
  ```rust
  for (i, b) in bars.iter().enumerate() {
      let x = inner.x + i as u16 * 2;
  ```
- **Finding:** Every candle was drawn starting from the left border (`inner.x + i * 2`) rather than anchoring the newest visible candle (`bars.last()`) to the right edge of the plotting area (`plot_right = inner.right() - y_axis_width - right_margin`). Whenever `count * stride < plot_width` (either because `zoom` was smaller than the viewport capacity or because an instrument had fewer historical bars than the viewport width), the candles clustered on the far left of the chart, leaving an empty void between the latest candle and the Y-axis price scale on the right.

### 1.3 Fixed 500-Candle History Cap & No Backfill Pagination
- **Location:** `crates/omt-providers/src/market.rs:326, 596` and `crates/omt-data/src/lib.rs:169`
- **Finding:**
  1. `market::history()` requested `limit=500` from Binance `/api/v3/klines` (which supports up to `limit=1000` per request and `endTime` pagination).
  2. Both `MarketState::candle()` and `FeedEvent::History` truncated `state.candles` to `500` elements.
  3. Scrolling left (`←` / `h`) beyond cached bars did not trigger an asynchronous backfill request for older historical candles before the earliest cached `open_at`.

### 1.4 Missing Chart Interactions & Visual Distinctions
- **Location:** `crates/omt-tui/src/lib.rs:418-435, 578-627` and `crates/omt-tui/src/views.rs:2077-2263`
- **Finding:**
  1. `Home` in `Workspace::Chart` set `self.offset = 0` (the newest candle) instead of jumping to the earliest loaded candle, and `End` modified `self.cursor` (table row index) instead of jumping to the latest candle (`offset = 0`).
  2. No explicit `follow_live` state (`L` key) existed to distinguish live-tracking mode from historical pan mode.
  3. Forming candles (`!b.closed`) were rendered identically (`█`) to closed candles rather than using a distinct glyph (`▒`) and badge.
  4. Time intervals were plotted strictly by slice index `i` without detecting missing timeframe intervals (gaps) or rendering multiple intermediate X-axis time ticks aligned to actual candle columns.
  5. Crosshair (`[` / `]` / `X`) only drew a vertical line (`┊`) without a horizontal price crosshair line (`┈`) or Y-axis cursor price callout.

---

## 2. Workspace Layout & Information Density Audit

### 2.1 Uniform 3-Column Split Across All Functions
- **Location:** `crates/omt-tui/src/lib.rs:889-912`
- **Finding:** Whenever `area.width >= 112 && !app.full_width`, `render_inner` applied the same 3-column layout (`Watchlist` 22–24 cols + `Center` + `Context` 32–34 cols) to every workspace:
  - On **CHART (`GP`)**, 56 columns were consumed by sidebars by default instead of allowing the candlestick chart to dominate the viewport.
  - On **NEWS (`CN`)**, there was no split Headline Feed + Article/Filing Reader layout in the main workspace area.
  - Users could not independently collapse/expand the left watchlist or right provenance/source panel (`S`), adjust split widths (`<` / `>`), or switch to split-screen (`SPLIT`) or 2×2 multi-panel Launchpad (`QUAD` / `2X2`) layouts.

---

## 3. `NEWS` Workspace Audit (Raw SEC Filing Table vs. Financial News Terminal)

### 3.1 SEC-Only Implementation in `Workspace::News`
- **Location:** `crates/omt-tui/src/views.rs:460, 1372-1485`
- **Finding:**
  - `Workspace::News` invoked `filings(frame, app, area)`, which required `app.company` (`Some(Company)`) and otherwise rendered an `unavailable` placeholder.
  - Opening `NEWS` on startup (when `BTCUSDT` is selected) or running `NEWS BTCUSDT`, `NEWS CRYPTO`, `NEWS MACRO`, or `NEWS EQUITIES` showed an empty/unavailable screen because no multi-source financial news model or RSS/Atom/JSON news ingestion existed.
  - Selecting a filing only offered opening the URL in an external browser (`OPEN`) rather than fetching and parsing the filing/article text into a clean, HTML-stripped terminal reader pane.

---

## 4. Command Grammar & Persistent Security Context Audit

### 4.1 Security-First & Bloomberg Sector Syntax Gaps
- **Location:** `crates/omt-core/src/command.rs:115-262` and `crates/omt-app/src/controller.rs:88-326`
- **Finding:**
  - Commands such as `FILINGS AAPL`, `NEWS AAPL SEC`, `NEWS CRYPTO`, `NEWS MACRO`, `NEWS EQUITIES`, `TOP`, `DES` (without argument, using active security context), and Bloomberg-style sector-qualified commands (`AAPL US EQUITY`, `AAPL US EQUITY FA <GO>`, `BTCUSDT CURNCY GP <GO>`) were not parsed.
  - `DES AAPL` routed directly to `Workspace::Fin` on whatever `stmt_tab` was previously active instead of a dedicated **Security Description (`DES`)** overview/profile tab.
  - Switching between crypto (`BTCUSDT`) and equity (`AAPL`) workspaces did not preserve separate `last_equity_asset` / `last_crypto_asset` context pointers or a `recent_securities` history list.

---

## 5. Implementation Priority Plan

1. **P0 — Fix `CHART` (`GP`) Viewport Calculation, Right-Alignment, Zoom/Pan, Backfill & Crosshair**
   - Compute dynamic visible candle capacity from `plot_width` at the active zoom stride (`1`, `2`, `3`, or `4` cells/candle).
   - Right-anchor the newest visible candle to `plot_right = inner.right() - y_axis_width - right_margin` even when history is shorter than the viewport, displaying an honest limited-history banner when `candles.len() < capacity`.
   - Increase initial Binance klines fetch to `1000` bars and implement automatic asynchronous older-history pagination (`endTime`) when scrolling left toward the start of cached history.
   - Add `follow_live` mode (`L`), `Home` (earliest candle) / `End` (latest candle), `X` crosshair toggle with 2D crosshair lines (`┊` and `┈`) and axis callouts, distinct forming candle rendering (`▒`), time-gap detection, and selectable lower oscillator sub-pane (`Volume + RSI / MACD / ATR`).
2. **P0 — Adaptive Bloomberg Workstation Layouts & Multi-Pane Modes**
   - Make `CHART`, `NEWS`, `FIN`, `MARKET`, `SCAN`, `PORT`, and `ECON` use workspace-appropriate layouts.
   - Add independent panel visibility toggles (`S` for source/provenance panel, `W` / `LAYOUT` for watchlist), adjustable split widths (`<` / `>`), `SPLIT` (2-pane linked view), and `QUAD` / `2X2` (4-pane Bloomberg Launchpad monitor), persisted in SQLite.
3. **P0 — Bloomberg Command Grammar, `DES` Security Description & Persistent Context**
   - Support security-first, function-first, and sector-qualified commands (`AAPL`, `AAPL DES`, `AAPL FA`, `AAPL GP`, `AAPL CN`, `AAPL FILINGS`, `AAPL US EQUITY FA <GO>`, `FILINGS AAPL`, `NEWS AAPL SEC`, `NEWS CRYPTO`, `NEWS MACRO`, `NEWS EQUITIES`, `TOP`).
   - Add `StatementTab::Description` (`DES`) in `FIN` and persistent security context tracking (`recent_securities`, `last_equity_asset`, `last_crypto_asset`).
4. **P0 — Redesign `NEWS` Into a Full Financial News & SEC Filings Terminal with Reader**
   - Build a unified `NewsItem` model and multi-source RSS/Atom/JSON + SEC EDGAR provider (`Yahoo Finance`, `CNBC`, `Federal Reserve`, `CoinDesk`/`Cointelegraph`, `SEC EDGAR`, `GDELT`) with local SQLite caching, deduplication, read/unread state, ticker/category/source filtering, and an integrated HTML-stripped Article & SEC Filing Reader pane (`r` to expand reader).
5. **P1 — Verify Across All Terminal Sizes (80×24, 120×30, 180×45, 240×60) & Document Reports**
   - Add automated layout, chart right-alignment, command grammar, news parsing, and filing extraction regression tests, run all quality gates (`fmt`, `clippy`, `test`, `--probe`, benchmarks), and generate `TERMINAL_UX_REPORT.md` and `MVP_COMPLETION_REPORT.md`.
