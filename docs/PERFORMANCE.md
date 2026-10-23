# Performance Methodology and Measured Results

## Scope and boundaries

Performance measurements in Open Market Terminal separate local deterministic
computation and headless buffer rendering from network transport, upstream
exchange latency, 33ms frame coalescing schedules, and terminal emulator display
pipelines. No synthetic benchmark number is presented as exchange round-trip or
end-to-end physical glass latency.

## Architecture for bounded latency and memory

1. **Decoupled async I/O**: Tokio worker tasks (`omt-providers`) manage REST
   requests and WebSocket streams:
   - Task 1 performs full Binance Spot universe discovery (`GET /api/v3/exchangeInfo`)
     and 5-minute 24h snapshot reconciliation (`GET /api/v3/ticker/24hr`).
   - Task 2 maintains a persistent exchange-wide `!miniTicker@arr` stream so
     active symbol/timeframe switches never disconnect the universe feed.
   - Task 3 manages selective detail subscriptions (`<chart>@kline_<tf>`,
     `<chart>@ticker`, and up to 12 `<symbol>@bookTicker` streams).
   The Ratatui rendering path (`omt-tui`) never awaits network I/O or database queries.
2. **Bounded queues and state coalescing**:
   - Feed workers emit into a bounded 256-item `mpsc` channel.
   - A 33ms coalescing loop updates a latest-value `watch` snapshot (`MarketState`),
     so rendering runs at a predictable 30 FPS cadence rather than redrawing on
     every incoming WebSocket frame.
   - Research requests use a bounded 8-item queue and a 16-item result channel.
3. **Full universe retention with virtualized rendering**:
   - `MarketState` retains all discovered exchange instruments (`instruments`)
     and active quotes (`quotes`) without truncating the market universe to the
     personal watchlist.
   - Tables in `MARKET`, `SCAN`, `FIN`, `NEWS`, `PORT`, and `ECON` use virtualized
     viewport slicing so only visible rows are formatted into Ratatui `Row`/`Cell`
     allocations per frame.
   - Historical and streaming OHLCV state retains at most 500 candles per active
     chart key.
   - Command history retains at most 50 entries.
4. **Monotonic timing and clock-skew detection**:
   - Quote ingestion and batch normalization record monotonic `std::time::Instant`
     elapsed microseconds (`last_ingest_micros`, `last_normalize_micros`, and a
     512-sample rolling window for P50/P95/P99 telemetry).
   - Upstream exchange timestamps (`event_at`) are never subtracted from local
     wall clock (`received_at`) to compute network latency; if `event_at > received_at`,
     clock skew is explicitly flagged (`clock_skew_ms`).
   - Startup-to-first-draw (`startup_ms`) and per-frame draw duration (`draw_us`
     with rolling P50/P95/P99) are tracked using `std::time::Instant`.

## Reproducible benchmark harness

Run the deterministic synthetic benchmark in release mode:

```sh
cargo run --release -p omt-tui --example benchmark
```

### Workload specification

- **Universe**: 3,500 discovered instruments across `USDT`, `USDC`, `BTC`, and `FDUSD`
  quote currencies (`3,183` actively trading with live quotes, plus `BREAK` status rows)
- **Memory comparison**: Exact heap footprint of 3 watched assets vs. 3,500-instrument
  full exchange universe
- **Candle history**: 500 closed OHLCV bars loaded in `Workspace::Chart`
- **Quote ingestion**: 25,000 quote validations and `MarketState::ingest` updates
- **Rendering**:
  - 500 full `140x40` headless `TestBackend` frames of the `CHART` workspace
    (candlesticks, volume, SMA20/50, EMA12/26, Bollinger Bands(20,2), RSI(14),
    ATR(14), MACD(12,26,9), and window statistics)
  - 200 full `140x40` headless `TestBackend` frames of the `MARKET` workspace
    over the 3,500-instrument universe with filtering, sorting, breadth bar,
    gainers/losers/active movers, and virtualized viewport rendering
  - 200 keyboard navigation steps (`Down` + `o` sort-order flip + full frame draw)
    under simultaneous streaming quote updates
- **Percentile method**: Exact sorted-sample index (`P50`, `P95`, `P99`).

### Measured results (Release build, Windows x86_64 GNU toolchain)

| Metric | Measured Value | Notes |
|---|---|---|
| `mem_3_watched_bytes` | `1,272 B` | 3-asset watchlist quote state |
| `mem_3500_universe_bytes` | `2,330,240 B` (`~2.22 MiB`) | 3,500 `InstrumentInfo` + 3,183 `Quote` entries |
| `universe_throughput_updates_per_sec` | `435,597 updates/s` | Full-universe batch quote ingestion throughput |
| `ingest_p50_us` / `p95_us` / `p99_us` | `1.200 µs` / `2.300 µs` / `4.700 µs` | Single quote validation + `BTreeMap` update (`25,000` samples) |
| `draw_chart_p50_ms` / `p95_ms` / `p99_ms` | `2.607 ms` / `3.556 ms` / `3.977 ms` | Full `140x40` `CHART` frame + indicators (`500` frames) |
| `draw_market_3500_p50_ms` / `p95_ms` / `p99_ms` | `4.608 ms` / `7.286 ms` / `8.097 ms` | Full `140x40` `MARKET` frame over 3,500 instruments (`200` frames) |
| `nav_under_load_p95_us` | `5,854.600 µs` (`5.855 ms`) | Key navigation + sort-order flip + draw under streaming load |
| `batch_receive_to_buffer_p95_ms` | `3.590 ms` | 50 quote ingestions + 1 full `140x40` frame draw |
| `total_seconds` | `3.057 s` | Total wall time for entire multi-stage benchmark suite |

### Exclusions

The benchmark harness explicitly excludes:
- Network DNS, TLS handshake, HTTP, and WebSocket transit time
- The 33ms frame tick interval used in the interactive loop
- Terminal emulator escape-sequence parsing and GPU/display scanout
