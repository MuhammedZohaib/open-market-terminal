# Open Market Terminal (OMT)

A native Rust terminal for local financial research. No account, subscription,
browser, database server, telemetry or brokerage connection is required.
Public crypto feeds work without credentials. The working name is configurable.

This is an initial functional release, **not a claim of production certification**.
See [implementation report](docs/IMPLEMENTATION_REPORT.md) for verified features,
credential-dependent integrations, and remaining limitations.

## Build and launch

Install current stable [Rust](https://rustup.rs/). Building bundled SQLite and TLS
requires the platform's native compiler (Visual Studio C++ Build Tools on Windows,
Xcode Command Line Tools on macOS, GCC/Clang on Linux). These are build-time tools;
the application needs no separately managed database or language runtime.

```sh
cargo build --locked --release
cargo run --release
cargo run --release -- BTCUSDT
cargo run --release -- "FIN AAPL"
cargo install --locked --path crates/omt-app
omt --help
omt --version
omt --offline
```

The binary is `target/release/omt` (`omt.exe` on Windows). The release workflow
packages standalone executables for Linux x86_64, macOS ARM64, and Windows x86_64.
When hosted in GitHub, download the matching `omt-<platform>` workflow artifact,
extract it with its license notices, and run it in a terminal. No hosted downloads
have been published from this local implementation.

In this Windows workspace, a user-local Rust GNU toolchain and a compiler in
`.tools/` were installed to perform verification. `.cargo/config.toml` points to
that compiler and is deliberately ignored; other developers should use their
own standard compiler setup. `scripts/dev.ps1` configures this session's PATH.

## Seven workspaces

| Workspace | Mnemonic | Function |
|---|---|---|
| MARKET | `MKT` / `CRYPTO` / `WEI` / `MOST` | Full Binance Spot market discovery (`3,700+` symbols, `1,380+` trading pairs), exchange-wide `!miniTicker@arr` streaming, quote-currency/liquidity/status filters, breadth & top movers (`MARKET`/`CRYPTO`), plus honest World Equity Indices reference view (`WEI`) |
| CHART | `GP` / `GIP` | Actual OHLCV, terminal candles/volume, candle crosshair cursor (`[`/`]`), SMA20/50, EMA12/26, Bollinger Bands(20,2), RSI14, ATR14, MACD, VWAP, six Binance timeframes, zoom/pan |
| FIN | `FA` / `DES` | 29 SEC US-GAAP concepts across 5 comparative multi-period statement views (`OVERVIEW`, `INCOME`, `BALANCE`, `CASH`, `RATIOS`), direct annual/quarterly periods, YoY growth, margins/FCF/ROE/ROA/liquidity ratios, source references |
| NEWS | `CN` / `CF` | Up to 200 recent SEC submissions across 27 corporate, proxy, beneficial-ownership, and insider forms (`10-K`, `10-Q`, `8-K`, `DEF 14A`, `SD`, `13G`, `Form 4`, etc.) with report date, description, form filters, and browser launch (`Enter`/`o`) |
| SCAN | `EQS` / `SRCH` | Dual-mode market-wide screener: `SCAN CRYPTO` across all discovered Binance Spot pairs (price, 24h%, base/quote volume, quote currency, status) and `SCAN EQUITIES` across locally indexed SEC filers (`revenue`, `income`, `assets`) |
| PORT | `PRTU` | Multiple local accounts/watchlists, account summary bar (Market Value, Cost Basis, Unrealized & Realized P&L), decimal average-cost accounting, fees, recent fills blotter, `DELTX`, CSV import/export |
| ECON | `ECST` / `YCRV` | Published Treasury yield curve with 2s10s/3m10s/5s30s spreads & curve regime, interactive maturity/observation table (`j/k`), World Bank series, optional FRED series, dated charts |

Only cached/fetched companies are screened in `SCAN EQUITIES`. `INDEX AAPL` deliberately
fetches one company. The program does not issue thousands of SEC calls at startup.
No sector classification, global equity universe, consolidated US prices or
comprehensive news feed is claimed. Missing data is unavailable, never a fabricated zero.

## Keyboard and commands

| Key | Action |
|---|---|
| `/`, `:`, `Ctrl+P` | Open `GO >` command & symbol entry bar (`Up`/`Down` recalls history, `Ctrl+U` clears) |
| `Tab` while editing | Complete first matching command or symbol |
| `Tab`, `Shift+Tab` | Cycle watchlist, central workspace, and context inspector panes |
| Arrows, `j/k`, `PgUp/PgDn`, `Home/End` | Move row selection in virtualized tables |
| `Enter` | Execute command, open selected row in `CHART`, open `SCAN EQUITIES` row in `FIN`, or open SEC filing in browser |
| `Backspace` | Return to previous workspace & instrument (`BACK`) |
| `Esc` | Close overlay, clear candle cursor, or return focus to central panel |
| `1`–`7`, `F2`–`F8` | Select workspace (`MARKET`, `CHART`, `FIN`, `NEWS`, `SCAN`, `PORT`, `ECON`) |
| `u`, `l`, `x` in MARKET/SCAN | Cycle Quote Currency (`USDT`/`USDC`/`BTC`/`FDUSD`/`EUR`/`ALL`), Min 24h Quote Volume (`l`), Trading Status (`x`) |
| `s`, `o` in MARKET/SCAN | Cycle sort column (`s`) and toggle Ascending/Descending order (`o`) |
| `m` in MARKET / SCAN | Cycle `Overview`/`Crypto`/`WEI` in `MARKET`, or toggle `SCAN CRYPTO` / `SCAN EQUITIES` in `SCAN` |
| `w` | Toggle selected instrument in active watchlist |
| `t`, `i`, `[`, `]` in CHART | Cycle timeframe (`t`), overlay (`i`), move candle crosshair cursor (`[` and `]`) |
| `+`, `-`, Left, Right | Zoom and pan candles in `CHART`, or select period column (`←`/`→`) in `FIN` |
| `a`, `v`, `s`, `[`/`]` in FIN | Annual (`a`), direct quarterly (`v`), cycle statement tab (`OVERVIEW`/`INCOME`/`BALANCE`/`CASH`/`RATIOS`) |
| `f`, `o` in NEWS | Cycle filing form filter (`ALL`/`10-K/10-Q`/`8-K`/`PROXY`/`INSIDER`), open filing URL (`o`/`Enter`) |
| `z` | Toggle full-width central panel |
| `?`, `F1` | Help |
| `q`, `Ctrl+C` | Quit / safe shutdown |

Typing in an overlay takes precedence over normal shortcuts. Mouse wheel scrolls
lists. Below 112 columns (or when `z` full-width mode is active), `Tab` switches
between full-width panes. Below 28x9, a resize hint is displayed. `ascii = true`
offers simpler glyphs.

```text
MARKET USDT
MARKET GAINERS
CRYPTO ALL
CRYPTO BTC
WEI
BTCUSDT
CHART BTCUSDT 1H
BTCUSDT GP 1H <GO>
GIP BTCUSDT <GO>
IND BB
CHART COINBASE:BTC-USD 1H
FIN AAPL
FIN AAPL BALANCE
AAPL FA <GO>
AAPL DES <GO>
NEWS AAPL
AAPL CN <GO>
OPEN
BACK
SCAN CRYPTO
SCAN EQUITIES
EQS <GO>
QUOTE USDC
MINVOL 1000000
STATUS TRADING
FILTER qvol>1000000 change>2
FILTER revenue>1000000000
FILTER CLEAR
SORT qvol DESC
SORT revenue
WATCH ADD ETHUSDT
WATCH REMOVE SOLUSDT
WATCHLIST long-term
ACCOUNT research
BUY BTCUSDT 0.1 60000 2 2025-01-01T12:00:00Z
SELL BTCUSDT 0.05 65000
DELTX my-buy-1
EXPORT new-transactions.csv
IMPORT new-transactions.csv
PRESET SAVE large-revenue
PRESET LOAD large-revenue
INDEX AAPL
TREASURY
YCRV <GO>
WB US NY.GDP.MKTP.CD
WB PK FP.CPI.TOTL.ZG
FRED UNRATE
SETTINGS
```

Commands are case insensitive; symbols are normalized. `BINANCE:BTCUSDT`,
`COINBASE:BTC-USD` and `IEX:AAPL` remain separate assets. An unqualified symbol
ending in USDT/USDC defaults to Binance, a hyphenated product to Coinbase, and
other symbols to IEX. Other Binance pairs need an explicit venue prefix.

CSV header:

```csv
id,account,asset,side,quantity,price,fee,currency,timestamp
my-buy-1,research,BINANCE:BTCUSDT,Buy,0.1,60000,2,USDT,2025-01-01T12:00:00Z
```

Imports validate the complete ledger before an atomic commit. Duplicate IDs,
oversells, invalid amounts, future timestamps and currency mismatches fail without
partial import. Exports require a new filename and never overwrite existing files.
Fees are in the quote currency; buys capitalize fees, sells deduct fees from
proceeds. No short positions or FX conversion. Allocation is among priced assets
of the selected account and same currency; unavailable/stale prices are excluded.
USDT is not USD. This is a research ledger, not tax accounting software.

## Configuration and providers

Copy [examples/config.toml](examples/config.toml) to the platform config directory,
or supply `--config PATH`. Personal databases live in the platform local-data
directory; use `--data-dir PATH` to isolate another profile.

On Windows the directories are normally `%APPDATA%\OpenMarketTerminal\omt\config`
and `%LOCALAPPDATA%\OpenMarketTerminal\omt\data`. `directories` supplies the native
equivalents on macOS/Linux. Credentials are read from the environment only:

| Variable | Use |
|---|---|
| `OMT_SEC_USER_AGENT` | App name and your maintained contact email; mandatory before SEC calls |
| `APCA_API_KEY_ID`, `APCA_API_SECRET_KEY` | Your Alpaca data credentials; IEX entitlement required |
| `FRED_API_KEY` | Your own FRED API key |

SEC contact can alternatively be configured as `sec_user_agent`. No contact or
credential is invented or shipped. [Provider matrix](docs/PROVIDERS.md) describes
coverage and terms. Quote events retain upstream and local timestamps. Crypto
staleness is 15 seconds, IEX 120 seconds; off-hours IEX prices can become stale.
Cached quotes are historical. Treasury/World Bank/FRED observations are dated
published data, never live market ticks.

Company cache refresh: six hours; macro cache refresh: 24 hours; active candles
refresh every 60 seconds plus Binance kline events. Offline cache remains readable
with its original fetch time. Private financial records are never expired. Bad
nonessential JSON cache entries are ignored; corrupt financial records/database
errors are surfaced rather than silently replaced. Back up the data directory with
the application closed, and keep CSV exports of your ledger.

## Verification and reproducible terminal capture

```sh
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
omt --probe
omt "CHART BTCUSDT" --snapshot --wait-seconds 8 > terminal-frame.txt
omt "CHART BTCUSDT" --offline --snapshot
cargo run --release -p omt-tui --example benchmark
```

`--probe` requires a real Binance historical response and a real WebSocket quote.
Snapshots render the actual Ratatui widgets through a 140x40 terminal buffer;
they are not mock screenshots. See [captured frame](docs/terminal-frame.txt).
Synthetic values exist only in tests and the explicit benchmark example.

## Architecture and contribution

Six crates separate financial models/calculations, bounded data state, provider
adapters, SQLite, Ratatui widgets, and executable wiring. Tokio workers handle
network requests; the renderer never waits on network I/O. A bounded 256-event
queue feeds a coalesced latest-state watch channel; research queues are bounded.
The full discovered Binance Spot universe (`3,690+` symbols) and 500 active candles
are retained in memory, while selective detail subscriptions are capped at 50
instruments and table views use virtualized viewport rendering. The frame cadence
is capped at 33ms and clock-only refresh is once a second. Rendering uses terminal
diffs, not a full escape-sequence repaint for each tick.

Contributions should include relevant fixture tests, preserve attribution and
missing-value semantics, and pass the commands above. Do not commit secrets or
invent live data. Keep calculation changes independent of terminal widgets.
Cross-platform CI is configured; remote CI results require running it in a hosted
repository. See [performance methodology](docs/PERFORMANCE.md).

MIT licensed application code. Dependencies retain their own licenses; see
[inventory](docs/dependency-licenses.json) and [notices](THIRD_PARTY_NOTICES.md).
The code license grants no rights to redistribute provider data. Users must
observe upstream terms, attribution, access limits and their own entitlements.
