# Event research in Open Market Terminal

IMPACT connects news to observed price moves and benchmark comparisons. It answers
what moved around an event, not what caused the move. No AI service is required.

## Workflow

1. In NEWS select a story and press **e** or enter `STUDY`. Its date and source
   stay attached to the study. Macro stories use the current instrument.
2. Choose a comparison with `IMPACT AAPL SPY` or
   `IMPACT BINANCE:ETHUSDT BINANCE:BTCUSDT`. `COMPARE` is an alias. A new pair
   resets the event selection. Equity history requires Alpaca credentials.
3. Select headlines with j/k, toggle pin/follow date with **e**, open source with o, or enter `EVENT YYYY-MM-DD`.
   `EVENT CLEAR` resumes headline selection. R refreshes daily benchmark prices.
4. Inspect subject return, benchmark return, and excess across 1/3/5 common daily
   observations. Check the displayed baseline/end dates. N/A is missing data, not zero.
5. Check correlation, beta, and the normalized chart for the broader relationship.
   Compare other plausible explanations using FIN, FRED, TREASURY, and other news.

Useful questions include whether an earnings-day move exceeded the broad market,
or whether ETH diverged from BTC around a regulatory announcement. The app does
not automatically infer economic exposures or claim a headline caused a move.

## Calculations and limits

Only validated, positive, completed daily closes before the fetch date are used.
Dates are intersected before computing returns. Returns span adjacent shared
dates, which may be more than one calendar day apart. There is no interpolation.

- Simple return: close[t] / close[t-1] minus 1.
- Correlation: Pearson correlation of paired returns, not price levels.
- Beta: covariance of subject/benchmark returns divided by benchmark variance.
- Both require 20 matched return intervals. Constant series produce N/A where
  variance is insufficient. Neither statistic is a forecast.
- Chart:
  - **Mode 1 (REL)**: Normalized relative performance starting at 100 on first common date.
  - **Mode 2 (EVENT)**: Event reaction window trajectory (t-2 baseline through t+5 event bars) re-indexed to baseline = 100.0.
  - **Mode 3 (CAR)**: Cumulative Abnormal Return path (excess percentage points over benchmark).
  - **Mode 4 (SPREAD)**: Daily return spread (Asset return % minus Benchmark return %).
  - On wide viewports (>= 115 columns), dual side-by-side charts render Macro Relative Performance alongside the Micro Event Reaction / CAR path.
  - Press `p` or enter `:PLOT [REL | EVENT | CAR | SPREAD]` to cycle or jump to specific visualization modes.
- Event baseline: the common close immediately before the event's UTC date.
- Event window: first 1, 3, or 5 common bars on or after that date.
- Excess: subject percentage return minus benchmark percentage return, expressed
  in percentage points. It is not beta-adjusted abnormal return or causal alpha.

Missing baselines/end bars and distant gaps produce N/A. These conservative guards
are not a full exchange calendar. Daily windows include pre-headline movement;
after-hours stories are not automatically shifted to the next exchange session.
SEC filing timestamps are approximate. Multiple events can share a window.

Equities use IEX-only raw prices. Splits and dividends can distort comparisons.
Mixed venues/currencies have different close times and valuation bases. There is
no FX conversion or total-return adjustment. History is provider-bounded (up to
1,000 Binance or 500 IEX bars; Coinbase may return fewer). Correlation uses the
available intersection, not a promised lookback, and is sensitive to market regime.

RSS coverage includes broad market/macro context as well as ticker feeds. Feed
association does not prove exposure. Coverage is incomplete, some feeds can fail
independently, and historical news archives are not provided.

## Cache and failure behavior

Studies save in SQLite by subject/benchmark pair with fetch timestamps. For
example, `omt --offline "IMPACT AAPL SPY"` loads a previously saved pair without
network requests. Reopening IMPACT preserves a loaded study; R refreshes online.
A failed refresh retains the old result and shows the failure. A different pair
never displays the previous pair's prices while loading. News failure can leave a
price comparison with a warning; history failure prevents a fresh study.

## Local alerts

```
ALERT ADD IEX:AAPL ABOVE 250
ALERT ADD BINANCE:BTCUSDT BELOW 60000
ALERT LIST
ALERT REMOVE 1
```

Inclusive thresholds trigger once on the first qualifying live quote. These are
conditions, not crossing detectors. Source and receipt timestamps must both be
at most 60 seconds old, and the event must be after rule creation. Cached,
disconnected, and stale prices cannot trigger. Armed symbols are prioritized in
the bounded subscription. There are at most 40 saved rules; remove old records
to free space. In the list, j/k scroll and Esc closes.

The terminal must remain open and online. There are no email notifications,
desktop background service, or brokerage orders. New/deleted rules save immediately;
triggered state saves every 15 seconds and on clean exit. A crash before saving
can cause another trigger after restart.

## Scope

This adds event studies, comparisons, alerts, full screener presets, and clearer
provider states without new dependencies. It does not reproduce Bloomberg's
licensed global data, proprietary reporting, estimates, messaging, or execution.
The next data priorities are adjusted histories, exchange calendars, structured
economic/earnings releases with actual/consensus/prior values, intraday event
windows, and multi-asset exposures. An AI attribution layer needs that evidence first.

References: [Bloomberg news workflow](https://professional.bloomberg.com/products/bloomberg-terminal/news/)
and [Alpaca historical bars](https://docs.alpaca.markets/us/reference/stockbars).
