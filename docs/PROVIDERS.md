# Provider capabilities and coverage

| Source | Quotes | Streaming | Historical bars | Company facts/filings | Macro | Access |
|---|---|---|---|---|---|---|
| Binance Spot | Full exchange symbol discovery (`exchangeInfo`) + 24h snapshot (`ticker/24hr`) | Exchange-wide `!miniTicker@arr` + selective `@ticker`, `@bookTicker`, and `@kline_<tf>` | Six requested intervals (`1m`, `5m`, `15m`, `1h`, `4h`, `1d`), 500 bars | No | No | Public |
| Coinbase Exchange | Exchange ticker | Ticker/heartbeat | `1m`/`5m`/`15m`/`1h`/`1d`, up to 300 bars | No | No | Public |
| Alpaca | Last IEX trade | IEX trade feed | IEX bars | No | No | User credentials and entitlement |
| SEC EDGAR | No | No | No | Ticker→CIK, 29 US-GAAP concepts, 27 regulatory/insider filing forms | No | Maintained contact User-Agent |
| US Treasury | No | No | No | No | Latest published par-yield curve (1M–30Y) | Public |
| World Bank | No | No | No | No | Country/indicator, up to 100 annual observations | Public |
| FRED | No | No | No | No | Series metadata + up to 100 observations | User API key |

Binance uses the dedicated public-only REST (`https://data-api.binance.vision`) and
WebSocket (`wss://data-stream.binance.vision`) market-data hosts. No trading APIs.
At startup, OMT queries `GET /api/v3/exchangeInfo` to discover all Spot trading
pairs (`3,736` total symbols, `1,383+` actively `TRADING` across `USDT`, `USDC`,
`BTC`, `FDUSD`, `ETH`, `BNB`, `EUR`, `TRY`, and other quote currencies) and
initializes 24-hour market statistics via `GET /api/v3/ticker/24hr` (reconciled
every 5 minutes). Live updates arrive over a persistent `!miniTicker@arr` stream
and update all matching symbols in the market universe without disconnecting when
the user switches active chart symbols or timeframes. A separate selective detail
stream subscribes to `<chart>@kline_<tf>`, `<chart>@ticker`, and up to 12
`<symbol>@bookTicker` streams.

WebSockets have ping/pong, 90-second inactivity detection, exponential retry with
jitter and planned 23-hour rotation. REST 429/418 installs a per-host cooldown
using numeric or HTTP-date Retry-After. Unsupported symbols/data remain absent.
Connection failure never silently changes the venue. Unchanged quotes on a live
Binance Spot stream remain `Live` between 5-minute snapshot reconciliations and
are never falsely marked `Disconnected` simply because no trade occurred in the
last 15 seconds. When the upstream ticker or `@bookTicker` supplies best bid/ask
(`b`/`a` on Binance, `best_bid`/`best_ask` on Coinbase) or 24-hour VWAP (`w` on
Binance), those fields and the derived bid-ask spread (in quote units and bps)
are displayed; IEX trade ticks leave bid/ask/VWAP absent rather than inventing
order-book depth.

Binance klines update the active candle. Coinbase and IEX candles refresh through
REST each minute; their live trade ticks do not invent candle volume. Coinbase 4h
is explicitly unavailable. No IEX consolidated coverage, market cap or P/E is
manufactured. Alpaca permanent auth/permission/subscription-limit errors stop the
stream retry loop, and unconfigured Alpaca credentials do not spam global header
warnings in unrelated workspaces.

SEC requests are serialized with at least 250ms spacing. Ticker mappings are
retained in the research worker during the session. Facts/filings are privately
cached in SQLite. Twenty-nine USD/shares/USD-per-share US-GAAP concepts are
normalized across 5 comparative statement views (`OVERVIEW`, `INCOME`, `BALANCE`,
`CASH`, `RATIOS`). Annual durations are 330–380 days, direct quarters 70–105 days.
YTD flows are not treated as quarters. Q4 is not derived. Instant balance-sheet
facts remain explicitly instant and are matched to the reporting form (`10-K` for
annual columns, `10-Q` for quarterly columns) so comparative prior-year 10-K
instant facts do not contaminate quarterly columns. Latest filed accession wins
for identical metric/unit/start/end. Conflicting values in the selected accession
are unavailable. Revenue concept fallbacks have explicit priority. Margins require
matching units and exact start/end dates. Every displayed fact has its concept,
accession, filing date and original filing-directory URL in the context pane.

Submissions parse up to 200 recent SEC filings across 27 corporate, proxy,
registration, beneficial ownership, and insider forms (`10-K`, `10-Q`, `8-K`,
`DEF 14A`, `DEFA14A`, `SD`, `SCHEDULE 13G`, `SCHEDULE 13D`, `S-8`, `S-3`, `424B2`,
`Form 3`, `Form 4`, `Form 144`, etc.), preserving `reportDate`,
`primaryDocDescription`, and single-level XSL document paths (such as
`xslF345X06/form4.xml`), sorted newest-first by `(filingDate, accessionNumber)`.

World Bank's standard GDP/current-USD, GDP growth, CPI inflation and unemployment
series have explicit fallback units when the API unit field is empty. Other
series say to consult indicator metadata; arbitrary-series frequency discovery
is not implemented (the indicator endpoint is treated as annual). FRED metadata
supplies units/frequency; geography may require the original series page.

Official documentation and terms/access references:

- [Binance market-data hosts](https://github.com/binance/binance-spot-api-docs/blob/master/faqs/market_data_only.md)
- [Binance WebSocket streams](https://developers.binance.com/docs/binance-spot-api-docs/web-socket-streams)
- [Coinbase Exchange](https://docs.cdp.coinbase.com/exchange/introduction/welcome)
- [Alpaca documentation](https://docs.alpaca.markets/us/)
- [SEC APIs and fair access](https://www.sec.gov/search-filings/edgar-application-programming-interfaces)
- [World Bank API](https://datahelpdesk.worldbank.org/knowledgebase/articles/889392)
- [Treasury XML feed](https://home.treasury.gov/treasury-daily-interest-rate-xml-feed)
- [FRED API](https://fred.stlouisfed.org/docs/api/fred/)

These integrations enable private local research. Public availability does not
imply permission to redistribute, relicense or commercially rehost the data.
No bulk market-data redistribution is provided. Check upstream terms before
exporting/sharing cached provider data. CSV export covers the user's own ledger.
