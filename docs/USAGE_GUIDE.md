# Open Market Terminal usage guide

This guide covers the current OMT terminal application: setup, credentials,
navigation, all eight workspaces, local portfolio records, and troubleshooting.
Commands below describe the implemented application rather than planned features.

## Contents

- [First launch](#first-launch)
- [Credentials and configuration](#credentials-and-configuration)
- [Check your data providers](#check-your-data-providers)
- [Navigation and command entry](#navigation-and-command-entry)
- [Symbols and venues](#symbols-and-venues)
- [MARKET: discover instruments](#market-discover-instruments)
- [CHART: inspect prices](#chart-inspect-prices)
- [FIN: company fundamentals](#fin-company-fundamentals)
- [NEWS: headlines and filings](#news-headlines-and-filings)
- [SCAN: filter instruments and companies](#scan-filter-instruments-and-companies)
- [PORT: watchlists and portfolio records](#port-watchlists-and-portfolio-records)
- [ECON: economic observations](#econ-economic-observations)
- [Storage, caching, and offline use](#storage-caching-and-offline-use)
- [Troubleshooting](#troubleshooting)
- [Command-line reference](#command-line-reference)
- [Example research sessions](#example-research-sessions)

## First launch

Run shell commands in a terminal from the project root. Rust and a native C/C++
compiler are required to build; see the [README](../README.md#build-and-launch).

```sh
cargo build --locked --release
cargo run --release
```

In this Windows workspace, the helper configures the local compiler paths:

```powershell
Set-Location C:\work\bbt
.\scripts\dev.ps1 build --locked --release
.\target\release\omt.exe
```

Running `scripts/dev.ps1` without arguments runs the release application through
Cargo. Close an existing `omt.exe` before rebuilding that same Windows executable.

Start with a reasonably wide terminal, such as 140 columns by 40 rows. Press `?`
for help. Press `:` to open the command bar, type `CHART BTCUSDT 1H`, then press
Enter. Public crypto feeds require no keys. Network or regional restrictions may
still prevent a provider from responding.

In the rest of this guide, examples such as `CHART BTCUSDT 1H` are **TUI commands**:
press `:` first and enter one command at a time. Do not type a leading colon into
the command bar. Code blocks marked `sh` or `powershell` are shell commands.

## Credentials and configuration

### What the keys enable

| Setting | Source and purpose | Benefit in OMT | Example TUI command |
|---|---|---|---|
| `APCA_API_KEY_ID` and `APCA_API_SECRET_KEY` | Alpaca credentials for IEX stock and ETF data | Historical candles and live trade prices for equity charts and watched instruments | `CHART IEX:AAPL 1D` |
| `FRED_API_KEY` | Federal Reserve Economic Data API key | Published economic series with dates, units, and frequency | `FRED UNRATE` |
| `OMT_SEC_USER_AGENT` | SEC contact identity, not an API key | Company statements, regulatory filings, and local equity screening | `FIN AAPL` |

Alpaca supplies the price history used by chart indicators. The integration uses
IEX data, not the consolidated tape from every US exchange. It does not submit
orders or connect the local portfolio ledger to a broker.

A paper-only Alpaca account can access IEX data; a funded live account is not
required. Copy a matching API key ID and secret from the same account. The
application requests historical data from `https://data.alpaca.markets/v2` and
streams IEX trades from `wss://stream.data.alpaca.markets/v2/iex`.
`https://paper-api.alpaca.markets/v2` is the separate paper trading/account API;
changing the data URL to that URL is not an authentication fix.
See [Alpaca paper trading](https://docs.alpaca.markets/us/docs/paper-trading) and
[market data authentication and coverage](https://docs.alpaca.markets/us/docs/about-market-data-api).

FRED adds economic context: for example, unemployment and GDP observations can be
read alongside price charts. These series update on their publishers' schedules;
they are not streaming market ticks. Obtain your own key through
[FRED's API documentation](https://fred.stlouisfed.org/docs/api/fred/).

Binance Spot, Coinbase Exchange, Treasury yields, and World Bank indicators do
not require these keys. SEC access requires your maintained contact identity.

### Create or edit `.env`

For a new setup, copy [`.env.example`](../.env.example) to `.env` in the project
root. If `.env` already exists, edit it; copying the example over it would replace
your existing values. The following values are placeholders:

```dotenv
APCA_API_KEY_ID=your_alpaca_key_id
APCA_API_SECRET_KEY=your_matching_alpaca_secret
FRED_API_KEY=your_fred_key
OMT_SEC_USER_AGENT="Your Research App your-real-contact@your-domain.org"
```

Remove or comment out unused provider variables instead of leaving empty values.
An empty variable may still be sent to a provider and rejected.

The loader searches the **current working directory and then its parents** for
the nearest `.env`. It does not search relative to the executable or `--data-dir`.
Launch from the project root to use the project file consistently. Existing
process environment variables override values in `.env`, including old values
in a shell or parent process.

Restart OMT after editing `.env`. Rebuilding is not required just to change keys.
`REFRESH` does not reload `.env`. The file is ignored by Git; the example contains
no real credentials. Do not put keys into command arguments or shared snapshots.

`SETTINGS` displays provider instructions. It currently has no credential editor,
masked credential status screen, or dedicated validation button. Use actual data
commands to check each integration.

### Optional application settings

Copy [examples/config.toml](../examples/config.toml) to your platform configuration
directory, or explicitly select it:

```sh
omt --config examples/config.toml
```

```toml
name = "My Research Terminal"
ascii = false
# sec_user_agent = "Your Research App your-real-contact@your-domain.org"
```

`OMT_SEC_USER_AGENT` overrides `sec_user_agent`. Unknown configuration keys are
rejected. An explicitly supplied `--config` path must exist; the default config
file is optional. Credentials belong in the environment or `.env`, not TOML.

## Check your data providers

Restart from the project root after configuring credentials, and run without
`--offline`. Enter the commands below through the TUI command bar.

| Provider | Test | What success looks like |
|---|---|---|
| Public Binance | `CHART BTCUSDT 1H` | Candle history appears; provider state and subsequent updates indicate connectivity |
| Public Coinbase | `CHART COINBASE:BTC-USD 1H` | Coinbase candle history appears |
| Alpaca IEX | `CHART IEX:AAPL 1D` | Equity candles appear; the header reports Alpaca IEX stream state |
| Alpaca ETF data | `CHART IEX:SPY 1D` | ETF candles appear using the same credentials |
| FRED | `FRED UNRATE` | ECON shows dated unemployment observations and a `FRED UNRATE / fetched ... UTC` message after a successful fetch |
| SEC | `FIN AAPL` | Company statements load with source metadata |
| Treasury | `TREASURY` | ECON displays a dated yield curve |
| World Bank | `WB US NY.GDP.MKTP.CD` | ECON displays dated US GDP observations |

Candles or observations marked cached do not prove current credentials work.
Company data can be reused for six hours and macro data for 24 hours. Check fetch
timestamps and connection/error status, not just whether numbers are visible.
Outside trading hours, an IEX stream need not deliver a new trade immediately.

For a clean credential check without existing cached data, quit the regular
session and launch a separate profile from the same project directory:

```powershell
.\target\release\omt.exe --data-dir artifacts/key-check-01
```

Use a new directory name for a subsequent clean check, then run the TUI test
commands. This creates a separate SQLite profile while using the same `.env`.

Alpaca HTTP **401** indicates rejected authentication; **403** indicates denied
access. The chart diagnostics distinguish these responses from missing keys.
If you see an older generic setup warning, close the application, rebuild, and
restart the updated executable. A FRED failure appears in the ECON status text;
an HTTP 400 alone does not identify whether the key or series was invalid.

## Navigation and command entry

| Key | Action |
|---|---|
| `:`, `/`, `Ctrl+P` | Open the GO command bar |
| `Tab` in the command bar | Complete the current command/symbol suggestion |
| `Up` in the command bar | Recall history, or move within an active suggestion selection |
| `Down` in the command bar | Select suggestions when available; otherwise move forward through history |
| `Ctrl+U` in the command bar | Clear the input |
| `Enter` in the command bar | Execute typed input or the selected suggestion |
| `Esc` in the command bar | Cancel input |
| `1`–`8` or `F2`–`F9` | Open MARKET, CHART, FIN, NEWS, SCAN, PORT, ECON, IMPACT respectively |
| `Tab` / `Shift+Tab` | Cycle panel focus; in Launchpad, cycle its four panes |
| `Up` / `Down`, `k` / `j` | Move the selected row; watchlist focus uses its own cursor |
| `PgUp` / `PgDn` | Move table selection by pages; scroll an expanded news reader |
| `Backspace` | Return to the previous workspace/instrument (`BACK`) |
| `Esc` | Close help/settings, dismiss chart crosshair or expanded news reader, or focus the central panel |
| `?` / `F1` | Display help |
| `F12` | Toggle ingest/draw telemetry diagnostics |
| `q` / `Ctrl+C` | Quit with terminal restoration |

Typing in the command bar takes precedence over ordinary shortcuts. Command
keywords are case insensitive; keyboard shortcuts such as `w` and `W` differ.
Names such as watchlists/accounts should be single tokens. Use explicit date/time
strings where shown rather than local date formats.

### Layouts

| Command or key | Effect |
|---|---|
| `LAYOUT STANDARD` | Standard workstation layout |
| `LAYOUT FULL` or `z` | Full-width workspace; `z` toggles back |
| `LAYOUT SPLIT` | Split layout |
| `LAUNCHPAD` or `LAYOUT QUAD` | Four-pane overview; Tab selects a pane and Enter opens it |
| Uppercase `L` | Cycle layout modes |
| Uppercase `W` or `LAYOUT WATCH` | Toggle the watchlist panel |
| Uppercase `I` or `LAYOUT CONTEXT` | Toggle the context inspector |
| `<` / `>` | Adjust side-panel width |

On narrow terminals, some panes collapse and focus controls which pane is shown.
Widen the terminal or use `z` if tables or instructions are clipped. `ascii = true`
in TOML simplifies glyphs. Available space limits how much information fits.

## Symbols and venues

| Input | Meaning |
|---|---|
| `BTCUSDT` or `BINANCE:BTCUSDT` | Binance BTC priced in USDT |
| `COINBASE:BTC-USD` | Coinbase BTC priced in USD |
| `AAPL` or `IEX:AAPL` | Apple equity through Alpaca IEX |
| `IEX:SPY` | SPY ETF through Alpaca IEX |

Unqualified symbols ending in USDT/USDC default to Binance; hyphenated products
default to Coinbase; other symbols initially default to IEX. The controller can
also resolve known Binance instruments from discovered market data. For other
crypto quote currencies and ambiguous symbols, use an explicit venue prefix.
Venues are kept separate, and USDT is not treated as USD.

Both command-first and instrument-first chart syntax work:

```text
CHART BTCUSDT 1H
BTCUSDT GP 1H
AAPL FA
AAPL DES
```

An optional trailing `<GO>` is accepted. Pressing Enter is sufficient.

## MARKET: discover instruments

Use `MARKET` for the overview and `CRYPTO ALL` for the Binance Spot universe.
The symbol count is discovered from the provider and changes over time.

```text
MARKET USDT
MARKET GAINERS
MARKET LOSERS
MOST
CRYPTO ALL
QUOTE USDC
MINVOL 1000000
STATUS TRADING
SORT qvol DESC
```

`MOST` sorts by 24-hour quote volume. `MINVOL` is measured in each instrument's
quote currency, not automatically converted to USD. `STATUS ALL` includes other
exchange trading statuses. Select a row and press Enter to chart it; lowercase
`w` adds/removes the selected instrument from the active watchlist.

In MARKET and SCAN, `u` cycles quote currency, `l` cycles minimum quote volume,
`x` toggles trading-status filtering, `s` cycles sort column, and `o` reverses sort
direction. `m` cycles MARKET overview, crypto, and WEI modes.

`WEI` lists regional benchmark names and their ETF proxies. ETF prices are not
cash index levels. Unavailable rows and the coverage notice are expected when
quotes are absent. The view is not a comprehensive licensed global index feed;
use `CHART IEX:SPY 1D` to check an individual proxy's data directly.

## CHART: inspect prices

```text
CHART BTCUSDT 1H
CHART COINBASE:BTC-USD 1H
CHART IEX:AAPL 1D
IND SMA
IND EMA
IND BB
IND NONE
```

Timeframes are `1M`, `5M`, `15M`, `1H`, `4H`, and `1D`; `M` means minutes in this
command context. Coinbase does not support the app's 4-hour interval. `GIP`
opens a 5-minute chart by default.

| Key | Chart action |
|---|---|
| `t` | Cycle timeframe |
| `i` | Cycle price overlay: SMA, EMA, Bollinger Bands, or none |
| `g` | Cycle lower-panel volume, RSI, MACD, or ATR |
| `+` / `-` | Zoom in/out |
| `Left` / `Right` | Pan history toward older/newer bars |
| `Home` / `End` | Earliest loaded history / latest edge |
| `f` | Toggle follow-live anchoring |
| `x` | Toggle candle crosshair |
| `[` / `]` | Inspect an older/newer candle |
| `d` | Toggle instrument description |
| Uppercase `R` | Request a chart refresh |

Indicators require enough bars; missing results can mean insufficient history.
Historical candles and current trade prices have separate update paths. Binance
can update its active candle from klines; Coinbase and IEX candle history refresh
through REST approximately once a minute. A chart is not evidence that every
displayed field updates on every trade.

`REFRESH` does not reread credentials. If a chart remains stuck after a transient
failure, changing timeframe/instrument rebuilds its subscription; restart after
changing `.env`. Read the error shown rather than assuming all empty charts are
authentication failures.

## FIN: company fundamentals

Configure your SEC contact before requesting company data:

```text
FIN AAPL
FIN AAPL INCOME
FIN AAPL BALANCE
FIN AAPL CASH
FIN AAPL RATIOS
DES AAPL
```

The statement tabs are OVERVIEW, INCOME, BALANCE, CASH, and RATIOS. Use `s` or `]`
for the next tab, `S` or `[` for the previous tab. `a` selects annual periods;
`v` selects direct quarterly periods. Left/Right selects a period column and
Up/Down selects a metric. The context inspector shows concept, unit, dates,
accession, and source information. `o` or Enter opens the selected source when
available. `d` toggles the description view.

SEC statements are separate from Alpaca prices: valid Alpaca keys do not replace
the SEC contact setting. OMT uses available US-GAAP facts and compatible reporting
periods. Missing ratios remain unavailable; year-to-date flows are not silently
treated as single quarters, and fourth quarters are not derived. Unsupported
company reporting formats may leave statement gaps.

## NEWS: headlines and filings

```text
TOP
NEWS CRYPTO
NEWS MACRO
NEWS EQUITIES
NEWS MARKETS
NEWS EARNINGS
NEWS AAPL
FILINGS AAPL
```

`TOP` opens broad financial headlines. Category commands filter the news view;
company commands can also fetch SEC disclosures when the contact is configured.
Sources include configured public RSS feeds and SEC submissions, rather than a
complete commercial newswire. Source availability and article extraction vary.

| Key or command | News action |
|---|---|
| `Up` / `Down` | Select an item |
| `Enter` or `READ` | Read the item and toggle expanded/split reader |
| `o` or `OPEN` | Open the item's source in the browser |
| `[` / `]` | Scroll reader text |
| `PgUp` / `PgDn` | Scroll an expanded reader by a larger amount |
| `c` | Cycle news category |
| `t` | Toggle ticker filter |
| `f` | Cycle SEC forms: ALL, 10-K/10-Q, 8-K, PROXY, INSIDER |
| Uppercase `R` | Request a refresh |

If results disappear, check category, ticker, and filing-form filters. Article
text can be unavailable even when a headline and source URL exist. Use `o` for
the original source. Company refreshes can reuse the SEC cache.

## SCAN: filter instruments and companies

Crypto screening uses discovered Binance market data:

```text
SCAN CRYPTO
QUOTE USDT
MINVOL 1000000
FILTER qvol>1000000 change>2
SORT change DESC
```

Equity screening uses **locally fetched SEC companies**, not every listed stock:

```text
INDEX AAPL
INDEX MSFT
SCAN EQUITIES
FILTER revenue>1000000000
SORT revenue DESC
```

Wait for each company request to finish before expecting it in the screener.
`INDEX` deliberately fetches one company, and `FIN` can also populate the local
company cache. Enter opens a crypto result in CHART or an equity result in FIN.
`m` switches the two screener modes; `EQS` opens equities.

| Filter/sort field | Meaning |
|---|---|
| `price` | Crypto price in quote currency |
| `change` (`chg` in filters) | 24-hour percentage change |
| `volume` / `vol` | Base-asset volume |
| `qvol` / `quote_volume` | Quote-asset volume |
| `revenue`, `income`, `assets` | Available company fundamentals |
| `symbol` / `alpha` | Alphabetical sorting |

Use simple `>` and `<` comparisons without spaces inside a comparison. Multiple
comparisons are separated by spaces. Use canonical fields in examples and check
the resulting table. This is not a SQL expression language.

```text
FILTER CLEAR
PRESET SAVE large-revenue
PRESET LOAD large-revenue
```

A preset saves the expression, sort field/direction, crypto/equity mode, quote
currency, minimum volume, and trading-status filter. Legacy expression/sort
presets still load. Credentials are never included.
Use `FILTER CLEAR` when switching modes if an earlier expression hides results.

## PORT: watchlists and portfolio records

### Watchlists

```text
WATCHLIST long-term
WATCH ADD IEX:AAPL
WATCH ADD BTCUSDT
WATCH REMOVE BTCUSDT
```

`WATCHLIST` selects a named local list. `w` toggles an instrument in MARKET, crypto
SCAN, or CHART. Focus the watchlist using Tab, select a symbol, and press Enter
to chart it. Watchlists and portfolio accounts are separate concepts.

### Record local transactions

```text
ACCOUNT practice
BUY BTCUSDT 0.1 60000 2 2025-01-01T12:00:00Z
SELL BTCUSDT 0.05 65000 1 2025-01-02T12:00:00Z
PORT
```

These commands save local records; they do not execute trades, including paper
orders. Use a separate account such as `practice` for examples.

Syntax: `BUY|SELL SYMBOL QUANTITY PRICE [FEE] [RFC3339_TIMESTAMP]`. If omitted,
fee defaults to zero and timestamp defaults to the current time. To supply a
timestamp, also supply the fee, using `0` if appropriate. Fees use quote currency.

The portfolio calculates decimal average-cost positions and realized/unrealized
P&L. Buy fees enter cost basis; sell fees reduce proceeds. Selling more than the
recorded holdings is rejected. Short positions and currency conversion are not
supported. Treat unavailable/stale valuations separately from zero values.

`DELTX transaction-id` removes a record by ID and validates the remaining ledger.
Use IDs from the fills display or CSV export; IDs are generated automatically for
TUI buys/sells. Removing a buy that would invalidate a later sell can fail.

### CSV import and export

```text
EXPORT practice-transactions-01.csv
IMPORT corrected-transactions.csv
```

Relative paths resolve from the launch directory. Paths containing spaces can
be enclosed in double quotes. Exports cover the ledger, including accounts, and
require a new filename; they do not overwrite an existing file.

```csv
id,account,asset,side,quantity,price,fee,currency,timestamp
example-buy-1,practice,BINANCE:BTCUSDT,Buy,0.1,60000,2,USDT,2025-01-01T12:00:00Z
```

Imports validate the complete ledger before committing. Duplicate IDs, oversells,
invalid amounts, currency mismatches, and future timestamps fail without a
partial import. An export is not an idempotent import: importing it back into the
same database can fail on duplicate IDs. Keep exports as backups or transfer them
to a clean profile. This ledger is not a tax accounting system.

## ECON: economic observations

| TUI command | Dataset |
|---|---|
| `TREASURY` or `YCRV` | Latest published US Treasury par yields by maturity |
| `WB US NY.GDP.MKTP.CD` | US GDP in current USD from World Bank |
| `WB PK FP.CPI.TOTL.ZG` | Pakistan annual CPI inflation from World Bank |
| `FRED UNRATE` | US unemployment rate |
| `FRED GDP` | US gross domestic product |

FRED series identifiers and World Bank indicator codes are different namespaces.
Use the provider's series identifier rather than a free-text search phrase.
The app requests up to 100 observations for these macro integrations. See the
official FRED series pages for [UNRATE](https://fred.stlouisfed.org/series/UNRATE)
and [GDP](https://fred.stlouisfed.org/series/GDP).

Use Up/Down to select a maturity or observation. On Treasury curves, `m` toggles
categorical tenor spacing versus proportional maturity spacing. Available curves
include 2s10s, 3m10s, and 5s30s spread analysis. Read units and publication dates;
the values are dated observations rather than current executable rates.

The header shows series metadata and fetch/cache status. `REFRESH` currently has
no dedicated ECON network-fetch override. Re-entering a macro command respects
the 24-hour cache; use a separate profile for an uncached provider test.

## Storage, caching, and offline use

OMT stores local watchlists, transactions, cached candles, company facts, news,
and macro observations in SQLite. Default Windows locations are normally:

- Configuration: `%APPDATA%\OpenMarketTerminal\omt\config\config.toml`
- Data: `%LOCALAPPDATA%\OpenMarketTerminal\omt\data`

macOS and Linux use their platform configuration/local-data directories. Override
the data location with `--data-dir PATH`; that directory holds `omt.sqlite3` and
`omt.log`. OMT periodically persists state and saves on normal shutdown.

| Data | Refresh/cache behavior |
|---|---|
| Company facts and filings | Reuse cached data for up to six hours |
| Macro observations | Reuse cached data for up to 24 hours |
| Active candle history | Approximately 60-second provider refresh; Binance also uses live klines |
| Portfolio ledger | Persistent local records, not an expiring provider cache |

```sh
omt --offline
omt --offline --data-dir artifacts/practice-profile
```

Offline mode suppresses provider requests and uses saved data. Uncached symbols
or series remain unavailable. Cached values keep their timestamps; they should
not be interpreted as fresh prices. In a clean profile, offline data views will
usually be empty.

Back up the data directory with OMT closed, and keep separate CSV ledger exports.
Do not delete the main database to troubleshoot credentials. Use a clean alternate
profile instead. Settings in `.env` and TOML are separate files to retain as needed.

## Troubleshooting

| Symptom | Check or action |
|---|---|
| Still says credentials are missing | Confirm exact variable names, nonempty values, no leading `#`, and launch directory. Restart after changes. Check for overriding shell environment variables. |
| `.env` syntax error on launch | Use one `KEY=value` per line; quote text containing spaces. Parser errors intentionally omit offending values. |
| Alpaca HTTP 401 | Verify the ID and secret are a matching pair from the same account. Check copied values and environment overrides. Restart after correction. |
| Alpaca HTTP 403 | Check the account's data permissions and the requested IEX feed. |
| No new IEX trades, but history works | Check provider state and market activity; historical retrieval and live streaming are separate checks. |
| FRED says to set `FRED_API_KEY` | Set that independent key in `.env` and restart; Alpaca credentials do not authorize FRED. |
| FRED HTTP 400 | Check both API key and series ID, using a known ID such as `UNRATE`. The generic HTTP status does not identify the exact cause. |
| FIN/filings fail or stay empty | Supply a real SEC contact via `OMT_SEC_USER_AGENT` or TOML. Check ticker, connectivity, and reporting coverage. |
| Empty equities screener | Fetch companies with `INDEX AAPL`/`FIN AAPL`, wait for completion, and clear incompatible filters. |
| Empty crypto results | Check quote currency, minimum quote volume, status, expression, and Binance connectivity. |
| Missing fields, ratios, or indicators | Coverage, compatible reporting periods, or enough history may be absent. Unavailable is not zero. |
| Coinbase 4-hour chart unavailable | Select `1H` or `1D`, or use a supported venue/interval. |
| Rate-limit notice | Allow provider cooldown/backoff; rapid repeated requests will not bypass the limit. |
| Chart refresh stays loading | Allow the periodic history fetch or switch timeframe/instrument to rebuild the subscription. Restart after credential changes. |
| NEWS shows no items | Reset category/ticker/form filters; verify sources and SEC contact for filings. |
| Text is clipped | Widen the terminal, press `z`, or adjust panes with `W`, `I`, `<`, and `>`. |
| Windows build cannot replace `omt.exe` | Close that executable, then rebuild. Changing `.env` alone only requires a restart. |
| Snapshot is empty | Run online and give providers time with `--wait-seconds`; an immediate snapshot may precede responses. |

Press F12 for telemetry. Inspect `omt.log` in the selected data directory for
application diagnostics. Avoid sharing credentials when sending troubleshooting
material. Not every network failure is a credential failure: regional access,
timeouts, unsupported symbols, and provider outages can also prevent data access.

## Command-line reference

The shell examples use `omt` for an installed executable. In this workspace,
replace it with `.\target\release\omt.exe` on Windows or `./target/release/omt`
on Unix, or prefix arguments with `cargo run --release --`.

| Argument | Effect |
|---|---|
| `[SYMBOL_OR_COMMAND]` | Optional startup symbol or quoted TUI command |
| `--config PATH` | Use a specific TOML configuration file |
| `--data-dir PATH` | Select an isolated local profile |
| `--offline` | Disable provider network work |
| `--snapshot` | Render a 140×40 text frame and exit |
| `--wait-seconds N` | Wait for providers before rendering a snapshot |
| `--probe` | Check public Binance history and a real WebSocket quote |
| `--help` / `--version` | Show command-line help / version |

```sh
omt "CHART IEX:AAPL 1D"
omt "FRED UNRATE" --data-dir artifacts/fred-check-01
omt --probe
omt "CHART BTCUSDT 1H" --snapshot --wait-seconds 15 > terminal-frame.txt
omt "CHART BTCUSDT 1H" --offline --snapshot
```

`--probe` is a Binance check, not an Alpaca/FRED credential tester, and cannot be
combined with `--offline`. Snapshots use the real renderer but do not by
themselves validate live connectivity. Provider responses remain asynchronous.

## Example research sessions

### Explore crypto without keys

1. Open `CRYPTO ALL` and select a quote currency with `QUOTE USDT`.
2. Apply `MINVOL 1000000` and `SORT qvol DESC`.
3. Select a result and press Enter to chart it.
4. Cycle timeframe with `t`, overlay with `i`, and lower indicator with `g`.
5. Press `w` to add the active instrument to your watchlist.

### Inspect a company and its market prices

1. Configure both Alpaca credentials and your SEC contact; restart.
2. Open `CHART IEX:AAPL 1D` and check provider status.
3. Open `FIN AAPL INCOME`; compare annual/quarterly periods with `a`/`v`.
4. Open `FILINGS AAPL`; select a filing, press Enter to read or `o` to open its source.
5. Open `SCAN EQUITIES` to screen the companies you have fetched locally.

### Add macroeconomic context

1. Run `TREASURY` and inspect maturity rates and spreads.
2. Configure `FRED_API_KEY`, restart, and run `FRED UNRATE`.
3. Run `FRED GDP` to inspect published output observations.
4. Run `WB PK FP.CPI.TOTL.ZG` for World Bank annual inflation observations.
5. Compare publication dates, units, and frequency before interpreting trends.

## IMPACT and alerts

Workspace **8 / F9** opens event research. Try `IMPACT AAPL SPY` or the public-data
example `IMPACT BINANCE:ETHUSDT BINANCE:BTCUSDT`. `COMPARE` is an alias.
The panel shows correlation, beta, returns, event windows, and a normalized chart
when enough screen space is available. Use z to expand it.

In NEWS, press **e** or enter `STUDY` to study the selected headline. Its source
and date remain attached. In IMPACT, j/k selects headlines, o opens the source,
and R refreshes. `EVENT YYYY-MM-DD` chooses a date; `EVENT CLEAR` resumes following
headlines. Reopening IMPACT preserves a loaded study. Saved pairs work offline.

Event windows use common completed daily observations, so they include movement
before publication and do not prove causation. IEX equity prices are raw and
corporate actions can distort returns. Read the [methodology guide](IMPACT.md).

Use `ALERT ADD AAPL ABOVE 250`, `ALERT LIST`, and `ALERT REMOVE 1` for local
one-shot price alerts. Thresholds are inclusive, require fresh live quotes, and
run only while the terminal is open. Rules and triggered records are stored in
the profile. The guide covers persistence timing and data limitations.

SETTINGS shows credential presence without displaying values, alongside feed
status. Presence is not proof of authentication. Alpaca now reports successful
authentication/subscription separately from trade receipt; a subscribed feed may
be quiet outside market hours even while historical charts remain available.

For source coverage and access details, see [PROVIDERS.md](PROVIDERS.md).
For implementation limits and validation history, see
[IMPLEMENTATION_REPORT.md](IMPLEMENTATION_REPORT.md).
