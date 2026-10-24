# Open Market Terminal (OMT)

A native Rust terminal for local financial research. No account, subscription,
browser, database server, telemetry or brokerage connection is required.
Public crypto feeds work without credentials. The working name is configurable.

This is an initial functional release, **not a claim of production certification**.
See [implementation report](docs/IMPLEMENTATION_REPORT.md) for verified features,
credential-dependent integrations, and remaining limitations.

Start with the **[detailed usage guide](docs/USAGE_GUIDE.md)** for a walkthrough
of credentials, provider checks, all eight workspaces, keyboard controls,
portfolio CSVs, offline use, and troubleshooting.

## Events, impact, and monitoring

The new **IMPACT** workspace connects headlines to observed price moves:

- Normalized price comparison, return correlation, beta, and 1/3/5 common-bar
  event windows with benchmark-relative performance.
- NEWS → **e** studies the selected headline date and preserves its source.
- Local one-shot price alerts, saved in SQLite; complete screener presets.
- Clearer Alpaca authentication/subscription states, idle socket keepalives,
  explicit historical ranges, and masked credential presence in SETTINGS.

No new dependencies, AI subscriptions, or model downloads are required.
These are descriptive daily studies, **not proof that an event caused a move**.
Read the [event research guide](docs/IMPACT.md) for methodology and limitations.

```text
IMPACT AAPL SPY
COMPARE BINANCE:ETHUSDT BINANCE:BTCUSDT
EVENT 2026-09-18
EVENT CLEAR
ALERT ADD IEX:AAPL ABOVE 250
ALERT LIST
ALERT REMOVE 1
```

## First session

Launch from the project root. Press `:` to open the TUI command bar, type one
command, and press Enter:

| Try | Result | Setup |
|---|---|---|
| `CHART BTCUSDT 1H` | Public crypto candles | No key |
| `CHART IEX:AAPL 1D` | Apple price history and IEX trade stream | Alpaca key ID and secret |
| `FIN AAPL` | Company statements and source metadata | SEC contact identity |
| `FRED UNRATE` | Unemployment observations in ECON | FRED API key |
| `TREASURY` | Published Treasury yield curve | No key |
| `SETTINGS` | Credential presence, feed status, and setup instructions | Presence does not validate access |

Press `?` for help, `1`–`8` to switch workspaces, and `q` to quit. Cached values
alone do not confirm valid credentials; follow the guide's
[provider checks](docs/USAGE_GUIDE.md#check-your-data-providers) for a fresh test.

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

### Quick Install (Prebuilt Binaries)

- **Linux & macOS (One-line installer)**:
  ```sh
  curl -proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.sh | bash
  ```

- **Windows (PowerShell installer)**:
  ```powershell
  irm https://raw.githubusercontent.com/MuhammedZohaib/open-market-terminal/main/scripts/install.ps1 | iex
  ```

- **Debian / Ubuntu (`.deb`)**:
  Download `.deb` from [Releases](https://github.com/MuhammedZohaib/open-market-terminal/releases) and install via `sudo dpkg -i omt_<version>_<arch>.deb`.

- **Homebrew (macOS & Linux)**:
  ```sh
  brew tap MuhammedZohaib/open-market-terminal https://github.com/MuhammedZohaib/open-market-terminal
  brew install omt
  ```

Pre-compiled standalone packages and SHA-256 checksums are published across 9 target architectures (Linux x86_64 glibc/musl, Linux ARM64 glibc/musl, macOS Apple Silicon/Intel/Universal 2, Windows x64/ARM64). See the [Release & Distribution Guide](docs/RELEASE_AND_DISTRIBUTION.md) for full pipeline specifications and verification instructions.

In this Windows workspace, a user-local Rust GNU toolchain and a compiler in
`.tools/` were installed to perform verification. `.cargo/config.toml` points to
that compiler and is deliberately ignored; other developers should use their
own standard compiler setup. `scripts/dev.ps1` configures this session's PATH.

```powershell
.\scripts\dev.ps1 build --locked --release
.\target\release\omt.exe
```

Close a running `omt.exe` before rebuilding the same executable on Windows.

## Seven workspaces

| Workspace | Mnemonic | Function |
|---|---|---|
| MARKET | `MKT` / `CRYPTO` / `WEI` / `MOST` | Binance Spot market discovery, exchange-wide `!miniTicker@arr` streaming, quote-currency/liquidity/status filters, breadth & top movers (`MARKET`/`CRYPTO`), plus World Equity Indices reference view (`WEI`); symbol counts vary with provider listings |
| CHART | `GP` / `GIP` | Actual OHLCV, terminal candles/volume, candle crosshair cursor (`[`/`]`), SMA20/50, EMA12/26, Bollinger Bands(20,2), RSI14, ATR14, MACD, VWAP, six Binance timeframes, zoom/pan |
| FIN | `FA` / `DES` | 29 SEC US-GAAP concepts across 5 comparative multi-period statement views (`OVERVIEW`, `INCOME`, `BALANCE`, `CASH`, `RATIOS`), direct annual/quarterly periods, YoY growth, margins/FCF/ROE/ROA/liquidity ratios, source references |
| NEWS | `CN` / `CF` / `TOP` / `FILINGS` | Public financial RSS headlines, categories, article reader, and recent SEC submissions with report date, description, and form filters; `Enter` reads an item, `o` opens its source |
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
| `/`, `:`, `Ctrl+P` | Open `GO >` command & symbol entry bar (`Up` recalls history, `Down` selects available suggestions, `Ctrl+U` clears) |
| `Tab` while editing | Complete the selected command or symbol suggestion |
| `Tab`, `Shift+Tab` | Cycle watchlist, central workspace, and context inspector panes |
| Arrows, `j/k`, `PgUp/PgDn`, `Home/End` | Move row selection in virtualized tables |
| `Enter` | Execute command; open MARKET/crypto SCAN rows in CHART, equity SCAN rows in FIN, read NEWS items, or open a FIN source |
| `Backspace` | Return to previous workspace & instrument (`BACK`) |
| `Esc` | Close overlay, clear candle cursor, or return focus to central panel |
| `1`–`8`, `F2`–`F9` | Select workspace (`MARKET`, `CHART`, `FIN`, `NEWS`, `SCAN`, `PORT`, `ECON`, `IMPACT`) |
| `u`, `l`, `x` in MARKET/SCAN | Cycle Quote Currency (`USDT`/`USDC`/`BTC`/`FDUSD`/`EUR`/`ALL`), Min 24h Quote Volume (`l`), Trading Status (`x`) |
| `s`, `o` in MARKET/SCAN | Cycle sort column (`s`) and toggle Ascending/Descending order (`o`) |
| `m` in MARKET / SCAN | Cycle `Overview`/`Crypto`/`WEI` in `MARKET`, or toggle `SCAN CRYPTO` / `SCAN EQUITIES` in `SCAN` |
| `w` | Toggle selected instrument in active watchlist |
| `t`, `i`, `g`, `x`, `[`, `]` in CHART | Cycle timeframe (`t`), overlay (`i`), lower indicator (`g`), toggle crosshair (`x`), inspect candles (`[` and `]`) |
| `+`, `-`, Left, Right | Zoom and pan candles in `CHART`, or select period column (`←`/`→`) in `FIN` |
| `a`, `v`, `s`, `[`/`]` in FIN | Annual (`a`), direct quarterly (`v`), cycle statement tab (`OVERVIEW`/`INCOME`/`BALANCE`/`CASH`/`RATIOS`) |
| `c`, `t`, `f`, `o` in NEWS | Cycle category, toggle ticker filter, cycle filing form filter, open source URL (`o`) |
| `z` | Toggle full-width central panel |
| `L`, `W`, `I` | Cycle layouts, toggle watchlist, toggle context inspector (uppercase) |
| `R` in NEWS/CHART/FIN | Request refresh; does not reload `.env` and company requests can reuse cache |
| `F12` | Toggle diagnostics and ingest/draw telemetry |
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
equivalents on macOS/Linux. At startup the app loads `.env` from the current
working directory or its nearest parent before starting background workers.
Copy [.env.example](.env.example) to `.env` in the project root and fill in your
credentials. Existing environment variables take priority over `.env` defaults.
The local `.env` is excluded from Git; keep real keys out of tracked files.
Edit an existing `.env` rather than copying the example over it. Remove/comment
unused variables instead of leaving empty values. Restart after changing keys;
rebuilding is unnecessary for credential changes. The search is relative to the
launch directory, not the executable or `--data-dir`.

| Variable | Use |
|---|---|
| `OMT_SEC_USER_AGENT` | App name and your maintained contact email; mandatory before SEC calls |
| `APCA_API_KEY_ID`, `APCA_API_SECRET_KEY` | Your Alpaca data credentials; IEX entitlement required |
| `FRED_API_KEY` | Your own FRED API key |

Alpaca powers equity/ETF price charts; FRED powers published economic series.
An Alpaca paper-only account can access IEX market data without a funded live
account. OMT uses `https://data.alpaca.markets/v2` for history and
`wss://stream.data.alpaca.markets/v2/iex` for trades. The separate
`https://paper-api.alpaca.markets/v2` endpoint is for paper trading/account
operations; OMT does not submit broker orders.
See [Alpaca's paper account documentation](https://docs.alpaca.markets/us/docs/paper-trading).

To check keys inside the TUI, run `CHART IEX:AAPL 1D` and `FRED UNRATE`.
Look for provider connection/fetch status rather than cached values. HTTP 401
means rejected authentication; HTTP 403 means denied access. `SETTINGS` is an
instruction screen, not a credential test. See the guide's
[credential setup](docs/USAGE_GUIDE.md#credentials-and-configuration) and
[troubleshooting](docs/USAGE_GUIDE.md#troubleshooting).

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
