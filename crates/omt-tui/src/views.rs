use super::*;
use omt_core::{
    DataStatus, Filing, InstrumentInfo, Quote, TradingStatus, analyze_yield_curve, atr,
    bollinger_bands, ema, facts, macd, portfolio::positions, rsi, sma, window_stats,
};
use rust_decimal::{
    Decimal,
    prelude::{FromPrimitive, ToPrimitive},
};

fn with_commas(raw: &str) -> String {
    let (sign, rest) = if let Some(stripped) = raw.strip_prefix('-') {
        ("-", stripped)
    } else {
        ("", raw)
    };
    let (int_part, frac_part) = match rest.split_once('.') {
        Some((i, f)) => (i, Some(f)),
        None => (rest, None),
    };
    if int_part.len() <= 3 {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len() + int_part.len() / 3);
    out.push_str(sign);
    for (idx, ch) in int_part.chars().enumerate() {
        if idx > 0 && (int_part.len() - idx) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    if let Some(f) = frac_part {
        out.push('.');
        out.push_str(f);
    }
    out
}

pub fn format_price(v: Decimal) -> String {
    let a = v.abs();
    if a >= Decimal::from(1000) {
        with_commas(&format!("{:.2}", v.round_dp(2)))
    } else if a >= Decimal::ONE {
        let s4 = format!("{:.4}", v.round_dp(4));
        let trimmed = s4.trim_end_matches('0');
        let (int_p, frac_p) = trimmed.split_once('.').unwrap_or((trimmed, ""));
        if frac_p.len() < 2 {
            format!("{:.2}", v.round_dp(2))
        } else {
            format!("{int_p}.{frac_p}")
        }
    } else if a >= Decimal::new(1, 4) {
        format!("{:.6}", v.round_dp(6))
    } else {
        v.round_dp(8).normalize().to_string()
    }
}

pub fn format_signed_price(v: Decimal) -> String {
    if v > Decimal::ZERO {
        format!("+{}", format_price(v))
    } else {
        format_price(v)
    }
}

pub fn number(v: Decimal) -> String {
    let a = v.abs();
    if a >= Decimal::from(1_000_000_000) {
        format!("{:.2}B", v / Decimal::from(1_000_000_000))
    } else if a >= Decimal::from(1_000_000) {
        format!("{:.2}M", v / Decimal::from(1_000_000))
    } else if a >= Decimal::from(10_000) {
        format!("{:.1}K", v / Decimal::from(1_000))
    } else {
        v.round_dp(4).normalize().to_string()
    }
}

fn company_value(c: &Company, field: &str) -> Option<Decimal> {
    let f = facts::select(&c.facts, PeriodView::Annual);
    match field {
        "revenue" => f
            .iter()
            .copied()
            .find(|x| x.metric == "Revenue")
            .map(|x| x.value),
        "income" => f
            .iter()
            .copied()
            .find(|x| x.metric == "Net income")
            .map(|x| x.value),
        "assets" => f
            .iter()
            .copied()
            .find(|x| x.metric == "Total assets")
            .map(|x| x.value),
        _ => None,
    }
}

fn macro_x(r: &MacroObservation) -> Option<f64> {
    if r.source == "US Treasury" {
        omt_core::treasury_tenor_months(&r.series)
    } else {
        let date = if r.date.len() == 4 {
            format!("{}-01-01", r.date)
        } else {
            r.date.clone()
        };
        let day = chrono::NaiveDate::parse_from_str(&date, "%Y-%m-%d").ok()?;
        Some(day.and_hms_opt(0, 0, 0)?.and_utc().timestamp() as f64)
    }
}

pub fn filter_quote(q: &Quote, filter: &str) -> bool {
    filter_instrument_and_quote(None, Some(q), filter)
}

pub fn filter_instrument_and_quote(
    info: Option<&InstrumentInfo>,
    quote: Option<&Quote>,
    filter: &str,
) -> bool {
    let trimmed = filter.trim();
    if trimmed.is_empty() {
        return true;
    }
    for token in trimmed.split_whitespace() {
        if let Some((k, val)) = token.split_once('=') {
            let k_low = k.to_ascii_lowercase();
            let v_up = val.to_ascii_uppercase();
            match k_low.as_str() {
                "quote" | "ccy" => {
                    if v_up != "ALL" {
                        let q_ccy = info
                            .map(|i| i.quote_asset.as_str())
                            .or_else(|| quote.map(|q| q.currency.as_str()))
                            .unwrap_or("");
                        if !q_ccy.eq_ignore_ascii_case(&v_up) {
                            return false;
                        }
                    }
                }
                "base" => {
                    let b = info
                        .map(|i| i.base_asset.as_str())
                        .or_else(|| quote.map(|q| q.base_symbol()))
                        .unwrap_or("");
                    if !b.eq_ignore_ascii_case(&v_up) {
                        return false;
                    }
                }
                "status" => {
                    if v_up != "ALL" {
                        let st = info
                            .map(|i| i.status.to_string())
                            .unwrap_or_else(|| "TRADING".into());
                        if !st.eq_ignore_ascii_case(&v_up) {
                            return false;
                        }
                    }
                }
                "venue" => {
                    let ven = info
                        .map(|i| i.asset.venue.to_string())
                        .or_else(|| quote.map(|q| q.asset.venue.to_string()))
                        .unwrap_or_default();
                    if !ven.eq_ignore_ascii_case(&v_up) {
                        return false;
                    }
                }
                _ => {}
            }
            continue;
        }
        let mut matched_op = false;
        for op in ['>', '<'] {
            if let Some((field, value)) = token.split_once(op) {
                matched_op = true;
                let f_name = field.to_ascii_lowercase();
                if ["revenue", "income", "assets"].contains(&f_name.as_str()) {
                    break;
                }
                let Ok(threshold) = value.parse::<Decimal>() else {
                    return false;
                };
                let Some(q) = quote else {
                    return false;
                };
                let actual = match f_name.as_str() {
                    "price" => Some(q.price),
                    "change" | "chg" => q.effective_change_pct(),
                    "volume" | "vol" => q.volume,
                    "qvol" | "quote_volume" => q.quote_volume,
                    _ => None,
                };
                if !actual.is_some_and(|v| {
                    if op == '>' {
                        v > threshold
                    } else {
                        v < threshold
                    }
                }) {
                    return false;
                }
                break;
            }
        }
        if !matched_op {
            let q_low = token.to_ascii_lowercase();
            let key_match = info
                .map(|i| {
                    i.asset.key().to_ascii_lowercase().contains(&q_low)
                        || i.base_asset.to_ascii_lowercase().contains(&q_low)
                        || i.quote_asset.to_ascii_lowercase().contains(&q_low)
                })
                .unwrap_or(false)
                || quote
                    .map(|q| {
                        q.asset.key().to_ascii_lowercase().contains(&q_low)
                            || q.base_symbol().to_ascii_lowercase().contains(&q_low)
                    })
                    .unwrap_or(false);
            if !key_match {
                return false;
            }
        }
    }
    true
}

pub fn filtered_market_keys(app: &App) -> Vec<String> {
    let quote_filter_all =
        app.quote_filter.is_empty() || app.quote_filter.eq_ignore_ascii_case("ALL");
    let sort_mode = app.sort.as_str();
    let extract_sort_val = |q: Option<&Quote>| -> Option<Decimal> {
        let q = q?;
        match sort_mode {
            "price" => Some(q.price),
            "volume" | "vol" => q.volume,
            "qvol" | "quote_volume" => q.quote_volume.or(q.volume),
            "symbol" | "alpha" => None,
            _ => q.effective_change_pct(),
        }
    };
    let matches_entry = |info: Option<&InstrumentInfo>, quote: Option<&Quote>| -> bool {
        if app.status_filter_trading_only
            && let Some(i) = info
            && i.status != TradingStatus::Trading
        {
            return false;
        }
        if !quote_filter_all {
            let ccy = info
                .map(|i| i.quote_asset.as_str())
                .or_else(|| quote.map(|q| q.currency.as_str()))
                .unwrap_or("");
            if !ccy.eq_ignore_ascii_case(&app.quote_filter) {
                return false;
            }
        }
        if app.min_quote_volume > Decimal::ZERO {
            let qv = quote.and_then(|q| q.quote_volume).unwrap_or(Decimal::ZERO);
            if qv < app.min_quote_volume {
                return false;
            }
        }
        filter_instrument_and_quote(info, quote, &app.filter)
    };

    let mut entries: Vec<(bool, Option<Decimal>, &str)> =
        Vec::with_capacity(app.market.instruments.len().max(app.market.quotes.len()) / 2);
    for (k, info) in &app.market.instruments {
        let quote = app.market.quotes.get(k);
        if matches_entry(Some(info), quote) {
            entries.push((quote.is_some(), extract_sort_val(quote), k.as_str()));
        }
    }
    for (k, quote) in &app.market.quotes {
        if !app.market.instruments.contains_key(k) && matches_entry(None, Some(quote)) {
            entries.push((true, extract_sort_val(Some(quote)), k.as_str()));
        }
    }

    let is_alpha = matches!(sort_mode, "symbol" | "alpha");
    let asc = app.sort_ascending;
    entries.sort_unstable_by(|a, b| {
        match (a.0, b.0) {
            (true, false) => return std::cmp::Ordering::Less,
            (false, true) => return std::cmp::Ordering::Greater,
            _ => {}
        }
        let ord = if is_alpha {
            b.2.cmp(a.2)
        } else {
            a.1.cmp(&b.1)
        };
        let ord = if asc { ord } else { ord.reverse() };
        ord.then_with(|| a.2.cmp(b.2))
    });

    entries.into_iter().map(|(_, _, k)| k.to_string()).collect()
}

pub fn filtered_equities(app: &App) -> Vec<&Company> {
    let mut companies: Vec<_> = app
        .companies
        .iter()
        .filter(|c| {
            for token in app.filter.split_whitespace() {
                for op in ['>', '<'] {
                    if let Some((field, threshold)) = token.split_once(op) {
                        let f_low = field.to_ascii_lowercase();
                        if ["revenue", "income", "assets"].contains(&f_low.as_str()) {
                            let Some((v, t)) =
                                company_value(c, &f_low).zip(threshold.parse::<Decimal>().ok())
                            else {
                                return false;
                            };
                            if !(if op == '>' { v > t } else { v < t }) {
                                return false;
                            }
                        }
                    }
                }
                if !token.contains(['>', '<', '=']) {
                    let q = token.to_ascii_lowercase();
                    if !c.ticker.to_ascii_lowercase().contains(&q)
                        && !c.name.to_ascii_lowercase().contains(&q)
                    {
                        return false;
                    }
                }
            }
            true
        })
        .collect();
    companies.sort_by(|a, b| {
        let ord = if app.sort == "symbol" {
            b.ticker.cmp(&a.ticker)
        } else {
            let field = if ["revenue", "income", "assets"].contains(&app.sort.as_str()) {
                app.sort.as_str()
            } else {
                "revenue"
            };
            company_value(a, field).cmp(&company_value(b, field))
        };
        if app.sort_ascending {
            ord
        } else {
            ord.reverse()
        }
    });
    companies
}

pub fn filtered_news_items(app: &App) -> Vec<omt_core::NewsItem> {
    let mut items = app.news_items.clone();
    if let Some(c) = &app.company {
        let sec_items = omt_core::company_filings_to_news(c);
        for item in sec_items {
            if !items
                .iter()
                .any(|existing| existing.id == item.id || existing.url == item.url)
            {
                items.push(item);
            }
        }
    }
    items.retain(|item| {
        if app.news_category != NewsCategory::All {
            if app.news_category == NewsCategory::Filings {
                if !item.is_sec_filing && item.category != NewsCategory::Filings {
                    return false;
                }
            } else if item.category != app.news_category {
                return false;
            }
        }
        if item.is_sec_filing && app.filing_form_filter != "ALL" {
            let form = item.form.as_deref().unwrap_or("");
            let matched = match app.filing_form_filter.as_str() {
                "10-K/10-Q" => {
                    form.starts_with("10-K") || form.starts_with("10-Q") || form == "20-F"
                }
                "8-K" => form.starts_with("8-K") || form == "6-K",
                "PROXY" => form.contains("14A") || form.contains("13G") || form.contains("13D"),
                "INSIDER" => matches!(form, "3" | "4" | "144"),
                _ => true,
            };
            if !matched {
                return false;
            }
        }
        if let Some(ref ticker) = app.news_ticker_filter {
            let ticker_up = ticker.to_ascii_uppercase();
            let matches_ticker = item
                .ticker
                .as_deref()
                .is_some_and(|t| t.eq_ignore_ascii_case(&ticker_up))
                || item.headline.to_ascii_uppercase().contains(&ticker_up)
                || item.summary.to_ascii_uppercase().contains(&ticker_up);
            if !matches_ticker {
                return false;
            }
        }
        if !app.filter.trim().is_empty() {
            let f = app.filter.trim().to_ascii_uppercase();
            let matches_text = item.headline.to_ascii_uppercase().contains(&f)
                || item.summary.to_ascii_uppercase().contains(&f)
                || item.source.to_ascii_uppercase().contains(&f)
                || item
                    .ticker
                    .as_deref()
                    .is_some_and(|t| t.to_ascii_uppercase().contains(&f));
            if !matches_text {
                return false;
            }
        }
        true
    });
    items.sort_by_key(|a| std::cmp::Reverse(a.published_at));
    items
}

pub fn selected_news_item(app: &App) -> Option<omt_core::NewsItem> {
    let items = filtered_news_items(app);
    if items.is_empty() {
        return None;
    }
    items
        .get(app.cursor.min(items.len().saturating_sub(1)))
        .cloned()
}

pub fn filtered_filings(app: &App) -> Vec<&Filing> {
    let Some(c) = &app.company else {
        return Vec::new();
    };
    c.filings
        .iter()
        .filter(|f| match app.filing_form_filter.as_str() {
            "10-K/10-Q" => {
                f.form.starts_with("10-K") || f.form.starts_with("10-Q") || f.form == "20-F"
            }
            "8-K" => f.form.starts_with("8-K") || f.form == "6-K",
            "PROXY" => f.form.contains("14A") || f.form.contains("13G") || f.form.contains("13D"),
            "INSIDER" => matches!(f.form.as_str(), "3" | "4" | "144"),
            _ => true,
        })
        .collect()
}

pub fn active_row_count(app: &App) -> usize {
    match app.workspace {
        Workspace::Market => match app.market_mode {
            MarketMode::Wei => 12,
            _ => filtered_market_keys(app).len(),
        },
        Workspace::Scan => match app.scan_mode {
            ScanMode::Crypto => filtered_market_keys(app).len(),
            ScanMode::Equities => filtered_equities(app).len(),
        },
        Workspace::Fin => app.stmt_tab.metrics().len(),
        Workspace::News => filtered_news_items(app).len(),
        Workspace::Port => {
            let tx: Vec<_> = app
                .transactions
                .iter()
                .filter(|t| t.account == app.account)
                .cloned()
                .collect();
            positions(&tx).map(|p| p.len()).unwrap_or(0)
        }
        Workspace::Econ => app.macro_data.len(),
        Workspace::Chart => app.market.candles.len(),
    }
}

pub fn selected_market_instrument(app: &App) -> Option<String> {
    if let Some(ref sel) = app.selected_market_key
        && (app.market.quotes.contains_key(sel) || app.market.instruments.contains_key(sel))
    {
        return Some(sel.clone());
    }
    let keys = filtered_market_keys(app);
    if keys.is_empty() {
        return None;
    }
    keys.get(app.cursor.min(keys.len() - 1)).cloned()
}

pub fn selected_equity_ticker(app: &App) -> Option<String> {
    let eq = filtered_equities(app);
    eq.get(app.cursor.min(eq.len().saturating_sub(1)))
        .map(|c| c.ticker.clone())
}

pub fn selected_portfolio_asset(app: &App) -> Option<String> {
    let tx: Vec<_> = app
        .transactions
        .iter()
        .filter(|t| t.account == app.account)
        .cloned()
        .collect();
    let pos = positions(&tx).ok()?;
    pos.get(app.cursor.min(pos.len().saturating_sub(1)))
        .map(|p| p.asset.key())
}

pub fn selected_filing_url(app: &App) -> Option<String> {
    match app.workspace {
        Workspace::News => {
            let list = filtered_news_items(app);
            list.get(app.cursor.min(list.len().saturating_sub(1)))
                .map(|item| item.url.clone())
        }
        Workspace::Fin => {
            let c = app.company.as_ref()?;
            let table = facts::comparative_table(&c.facts, app.period, app.stmt_tab);
            let row = table
                .rows
                .get(app.cursor.min(table.rows.len().saturating_sub(1)))?;
            let col = app.fin_col.min(row.sources.len().saturating_sub(1));
            row.sources
                .get(col)
                .and_then(|s| *s)
                .map(|f| f.source.clone())
        }
        _ => None,
    }
}

pub fn main(frame: &mut Frame, app: &App, area: Rect) {
    match app.workspace {
        Workspace::Chart => chart(frame, app, area),
        Workspace::Market => market(frame, app, area),
        Workspace::Scan => scan(frame, app, area),
        Workspace::Fin => financials(frame, app, area),
        Workspace::News => news(frame, app, area),
        Workspace::Port => portfolio(frame, app, area),
        Workspace::Econ => economics(frame, app, area),
    }
}

fn render_market_table(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    keys: &[String],
    title: String,
    active: bool,
) -> usize {
    let now = Utc::now();
    let visible_capacity = area.height.saturating_sub(3).max(1) as usize;
    let selected_idx = if keys.is_empty() {
        0
    } else if let Some(ref sel_key) = app.selected_market_key
        && let Some(pos) = keys.iter().position(|k| k == sel_key)
    {
        pos
    } else {
        app.cursor.min(keys.len() - 1)
    };
    let win_start = if selected_idx >= visible_capacity {
        selected_idx + 1 - visible_capacity
    } else {
        0
    };
    let win_end = (win_start + visible_capacity).min(keys.len());
    let visible_slice = if keys.is_empty() {
        &[][..]
    } else {
        &keys[win_start..win_end]
    };

    let wide = area.width >= 92;
    let rows = visible_slice.iter().map(|key| {
        let info = app.market.instruments.get(key);
        let q = app.market.quotes.get(key);
        let sym = info
            .map(|i| i.asset.symbol.clone())
            .or_else(|| q.map(|q| q.asset.symbol.clone()))
            .unwrap_or_else(|| key.clone());
        let pair = info
            .map(|i| format!("{}/{}", i.base_asset, i.quote_asset))
            .or_else(|| q.map(|q| format!("{}/{}", q.base_symbol(), q.currency)))
            .unwrap_or_else(|| "--".into());
        let venue = info
            .map(|i| i.asset.venue.to_string())
            .or_else(|| q.map(|q| q.asset.venue.to_string()))
            .unwrap_or_else(|| "BINANCE".into());
        let price_str = q
            .map(|q| format_price(q.price))
            .unwrap_or_else(|| "unavailable".into());
        let chg_pct = q.and_then(|q| q.effective_change_pct());
        let chg_pct_str = chg_pct
            .map(|v| format!("{v:+.2}%"))
            .unwrap_or_else(|| "--".into());
        let chg_abs_str = q
            .and_then(|q| q.effective_change_abs())
            .map(format_signed_price)
            .unwrap_or_else(|| "--".into());
        let high_str = q
            .and_then(|q| q.high)
            .map(format_price)
            .unwrap_or_else(|| "--".into());
        let low_str = q
            .and_then(|q| q.low)
            .map(format_price)
            .unwrap_or_else(|| "--".into());
        let bvol_str = q
            .and_then(|q| q.volume)
            .map(number)
            .unwrap_or_else(|| "--".into());
        let qvol_str = q
            .and_then(|q| q.quote_volume)
            .map(number)
            .unwrap_or_else(|| "--".into());
        let status_str = if let Some(q) = q {
            format!("{:?}", q.status_at(now))
        } else if let Some(i) = info {
            i.status.to_string()
        } else {
            "No feed".into()
        };
        let dir_color = theme::direction_color(chg_pct);

        if wide {
            Row::new(vec![
                Cell::from(sym).style(Style::default().fg(FG).bold()),
                Cell::from(pair).style(Style::default().fg(SECONDARY)),
                Cell::from(price_str).style(Style::default().fg(dir_color)),
                Cell::from(chg_pct_str).style(Style::default().fg(dir_color).bold()),
                Cell::from(chg_abs_str).style(Style::default().fg(dir_color)),
                Cell::from(high_str).style(Style::default().fg(SECONDARY)),
                Cell::from(low_str).style(Style::default().fg(SECONDARY)),
                Cell::from(bvol_str).style(Style::default().fg(FG)),
                Cell::from(qvol_str).style(Style::default().fg(AMBER)),
                Cell::from(venue).style(Style::default().fg(MUTED)),
                Cell::from(status_str).style(Style::default().fg(SECONDARY)),
            ])
        } else {
            Row::new(vec![
                Cell::from(sym).style(Style::default().fg(FG).bold()),
                Cell::from(pair).style(Style::default().fg(SECONDARY)),
                Cell::from(price_str).style(Style::default().fg(dir_color)),
                Cell::from(chg_pct_str).style(Style::default().fg(dir_color).bold()),
                Cell::from(qvol_str).style(Style::default().fg(AMBER)),
                Cell::from(status_str).style(Style::default().fg(SECONDARY)),
            ])
        }
    });

    let mut state = TableState::default().with_selected(
        (!visible_slice.is_empty()).then_some(selected_idx.saturating_sub(win_start)),
    );
    if wide {
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(11),
                Constraint::Percentage(10),
                Constraint::Percentage(11),
                Constraint::Percentage(8),
                Constraint::Percentage(9),
                Constraint::Percentage(10),
                Constraint::Percentage(10),
                Constraint::Percentage(9),
                Constraint::Percentage(9),
                Constraint::Percentage(6),
                Constraint::Percentage(7),
            ],
        )
        .header(
            Row::new([
                "SYMBOL",
                "BASE/QUOTE",
                "LAST PRICE",
                "24H %",
                "24H CHG",
                "24H HIGH",
                "24H LOW",
                "BASE VOL",
                "QUOTE VOL",
                "VENUE",
                "STATUS",
            ])
            .style(theme::header_style()),
        )
        .row_highlight_style(theme::row_highlight_style())
        .block(block(title, active));
        frame.render_stateful_widget(table, area, &mut state);
    } else {
        let table = Table::new(
            rows,
            [
                Constraint::Percentage(20),
                Constraint::Percentage(16),
                Constraint::Percentage(20),
                Constraint::Percentage(14),
                Constraint::Percentage(16),
                Constraint::Percentage(14),
            ],
        )
        .header(
            Row::new(["SYMBOL", "PAIR", "LAST", "24H %", "QUOTE VOL", "STATUS"])
                .style(theme::header_style()),
        )
        .row_highlight_style(theme::row_highlight_style())
        .block(block(title, active));
        frame.render_stateful_widget(table, area, &mut state);
    }
    visible_slice.len()
}

pub fn market(frame: &mut Frame, app: &App, area: Rect) {
    if app.market_mode == MarketMode::Wei {
        render_wei(frame, app, area);
        return;
    }
    let keys = filtered_market_keys(app);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(5),
    ])
    .split(area);

    let discovered = app.market.instruments.len().max(app.market.quotes.len());
    let trading = app
        .market
        .trading_instrument_count()
        .max(app.market.quotes.len());
    let visible_cap = rows[1].height.saturating_sub(3).max(1) as usize;
    let visible_count = keys.len().min(visible_cap);

    let mut advancers = 0usize;
    let mut decliners = 0usize;
    let mut unchanged = 0usize;
    let mut total_usdt_vol = Decimal::ZERO;
    for k in &keys {
        if let Some(q) = app.market.quotes.get(k) {
            match q.effective_change_pct() {
                Some(p) if p > Decimal::ZERO => advancers += 1,
                Some(p) if p < Decimal::ZERO => decliners += 1,
                _ => unchanged += 1,
            }
            if q.currency == "USDT"
                && let Some(qv) = q.quote_volume
            {
                total_usdt_vol += qv;
            }
        }
    }

    let summary_lines = vec![
        Line::from(vec![
            Span::styled(
                format!(" {} ", app.market_mode.label()),
                Style::default().fg(BG).bg(AMBER).bold(),
            ),
            Span::styled(
                format!(
                    "  Binance Spot │ {discovered} discovered pairs ({trading} trading) │ {} match filter │ {visible_count} visible",
                    keys.len()
                ),
                Style::default().fg(FG),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Quote[u]: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("[{}] ", app.quote_filter),
                Style::default().fg(AMBER).bold(),
            ),
            Span::styled("MinVol[l]: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("{} ", number(app.min_quote_volume)),
                Style::default().fg(FG),
            ),
            Span::styled("Status[x]: ", Style::default().fg(SECONDARY)),
            Span::styled(
                if app.status_filter_trading_only {
                    "TRADING "
                } else {
                    "ALL "
                },
                Style::default().fg(FG),
            ),
            Span::styled("Sort[s/o]: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!(
                    "{} {} ",
                    app.sort.to_ascii_uppercase(),
                    if app.sort_ascending { "ASC" } else { "DESC" }
                ),
                Style::default().fg(AMBER).bold(),
            ),
            Span::styled("Filter: ", Style::default().fg(SECONDARY)),
            Span::styled(
                if app.filter.is_empty() {
                    "none"
                } else {
                    &app.filter
                },
                Style::default().fg(FG),
            ),
            Span::styled("  │  Breadth: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("+{advancers} "),
                Style::default().fg(POSITIVE).bold(),
            ),
            Span::styled(
                format!("-{decliners} "),
                Style::default().fg(NEGATIVE).bold(),
            ),
            Span::styled(format!("={unchanged}"), Style::default().fg(MUTED)),
            if total_usdt_vol > Decimal::ZERO {
                Span::styled(
                    format!(" │ 24h USDT Vol: {}", number(total_usdt_vol)),
                    Style::default().fg(AMBER),
                )
            } else {
                Span::raw("")
            },
        ]),
    ];
    frame.render_widget(
        Paragraph::new(summary_lines).style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    render_market_table(
        frame,
        app,
        rows[1],
        &keys,
        format!(
            " {} — {} / {} rows (Enter: Open Chart, w: Toggle Watchlist) ",
            app.market_mode.label(),
            app.quote_filter,
            keys.len()
        ),
        app.focus == 1,
    );

    // Lower Auxiliary Movers Panel: Top Gainers | Top Losers | Most Active
    let mut quoted: Vec<&Quote> = keys
        .iter()
        .filter_map(|k| app.market.quotes.get(k))
        .collect();
    quoted.sort_by_key(|a| std::cmp::Reverse(a.effective_change_pct()));
    let top_gainers: Vec<String> = quoted
        .iter()
        .filter(|q| q.effective_change_pct().is_some_and(|p| p > Decimal::ZERO))
        .take(3)
        .map(|q| {
            format!(
                "{:<10} {:>9} {:>+6.2}%",
                q.asset.symbol,
                format_price(q.price),
                q.effective_change_pct().unwrap_or_default()
            )
        })
        .collect();
    let top_losers: Vec<String> = quoted
        .iter()
        .rev()
        .filter(|q| q.effective_change_pct().is_some_and(|p| p < Decimal::ZERO))
        .take(3)
        .map(|q| {
            format!(
                "{:<10} {:>9} {:>+6.2}%",
                q.asset.symbol,
                format_price(q.price),
                q.effective_change_pct().unwrap_or_default()
            )
        })
        .collect();
    quoted.sort_by_key(|a| std::cmp::Reverse(a.quote_volume.or(a.volume)));
    let most_active: Vec<String> = quoted
        .iter()
        .take(3)
        .map(|q| {
            format!(
                "{:<10} {:>9} {:>8}",
                q.asset.symbol,
                format_price(q.price),
                q.quote_volume
                    .or(q.volume)
                    .map(number)
                    .unwrap_or_else(|| "--".into())
            )
        })
        .collect();

    let mover_cols = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(33),
        Constraint::Percentage(34),
    ])
    .split(rows[2]);
    frame.render_widget(
        Paragraph::new(if top_gainers.is_empty() {
            "Waiting for live 24h gainers...".into()
        } else {
            top_gainers.join("\n")
        })
        .style(Style::default().fg(POSITIVE))
        .block(block(" Top Gainers (24h %) ", false)),
        mover_cols[0],
    );
    frame.render_widget(
        Paragraph::new(if top_losers.is_empty() {
            "Waiting for live 24h losers...".into()
        } else {
            top_losers.join("\n")
        })
        .style(Style::default().fg(NEGATIVE))
        .block(block(" Top Losers (24h %) ", false)),
        mover_cols[1],
    );
    frame.render_widget(
        Paragraph::new(if most_active.is_empty() {
            "Waiting for volume leaders...".into()
        } else {
            most_active.join("\n")
        })
        .style(Style::default().fg(AMBER))
        .block(block(" Most Active (24h Quote Vol) ", false)),
        mover_cols[2],
    );
}

fn render_wei(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(6),
        Constraint::Length(5),
    ])
    .split(area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    " WORLD EQUITY INDICES <WEI> ",
                    Style::default().fg(BG).bg(AMBER).bold(),
                ),
                Span::styled(
                    "  Global Regional Equity Benchmarks & Verified Index ETF Proxies",
                    Style::default().fg(FG),
                ),
            ]),
            Line::from(Span::styled(
                " Policy: Proprietary index levels (SPX, NDX, UKX, NKY) are never fabricated or scraped without license. Press m or :MARKET for live multi-asset overview.",
                Style::default().fg(SECONDARY),
            )),
        ])
        .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    let benchmarks = [
        ("Americas", "S&P 500 Index", "SPX", "IEX:SPY", "USD"),
        ("Americas", "NASDAQ-100 Index", "NDX", "IEX:QQQ", "USD"),
        ("Americas", "Dow Jones Industrial", "INDU", "IEX:DIA", "USD"),
        ("Americas", "Russell 2000 Index", "RTY", "IEX:IWM", "USD"),
        ("EMEA", "Euro Stoxx 50", "SX5E", "IEX:FEZ", "USD"),
        ("EMEA", "FTSE 100 Index", "UKX", "IEX:EWU", "USD"),
        ("EMEA", "DAX Performance Index", "DAX", "IEX:EWG", "USD"),
        ("EMEA", "CAC 40 Index", "CAC", "IEX:EWQ", "USD"),
        ("APAC", "Nikkei 225 Index", "NKY", "IEX:EWJ", "USD"),
        ("APAC", "Hang Seng Index", "HSI", "IEX:EWH", "USD"),
        ("APAC", "S&P/ASX 200", "AS51", "IEX:EWA", "USD"),
        ("APAC", "CSI 300 Index", "SHSZ300", "IEX:ASHR", "USD"),
    ];
    let now = Utc::now();
    let table_rows = benchmarks
        .iter()
        .map(|(region, name, idx_sym, proxy_key, ccy)| {
            let q = app.market.quotes.get(*proxy_key);
            let last = q
                .map(|q| format!("{} {ccy}", format_price(q.price)))
                .unwrap_or_else(|| "unavailable".into());
            let chg = q
                .and_then(|q| q.effective_change_pct())
                .map(|v| format!("{v:+.2}%"))
                .unwrap_or_else(|| "—".into());
            let status = q
                .map(|q| format!("{:?} ({})", q.status_at(now), q.provider))
                .unwrap_or_else(|| "Unavailable (no index/IEX feed)".into());
            Row::new(vec![
                Cell::from(*region).style(Style::default().fg(AMBER)),
                Cell::from(*name).style(Style::default().fg(FG).bold()),
                Cell::from(*idx_sym).style(Style::default().fg(SECONDARY)),
                Cell::from(*proxy_key).style(Style::default().fg(SECONDARY)),
                Cell::from(last).style(Style::default().fg(FG)),
                Cell::from(chg).style(Style::default().fg(theme::direction_color(
                    q.and_then(|q| q.effective_change_pct()),
                ))),
                Cell::from(status).style(Style::default().fg(MUTED)),
            ])
        });
    let mut state = TableState::default().with_selected(Some(app.cursor.min(benchmarks.len() - 1)));
    let table = Table::new(
        table_rows,
        [
            Constraint::Percentage(11),
            Constraint::Percentage(24),
            Constraint::Percentage(10),
            Constraint::Percentage(12),
            Constraint::Percentage(14),
            Constraint::Percentage(10),
            Constraint::Percentage(19),
        ],
    )
    .header(
        Row::new([
            "REGION",
            "BENCHMARK INDEX",
            "MNEMONIC",
            "ETF PROXY",
            "PROXY LAST",
            "24H %",
            "FEED COVERAGE / STATUS",
        ])
        .style(theme::header_style()),
    )
    .row_highlight_style(theme::row_highlight_style())
    .block(block(
        " WEI — World Equity Indices & Regional Proxies ",
        app.focus == 1,
    ));
    frame.render_stateful_widget(table, rows[1], &mut state);

    frame.render_widget(
        Paragraph::new(
            " Coverage Notice: Direct cash index feeds (CME/CBOE/Eurex/TSE) require exchange licensing.\n\
              Configure APCA_API_KEY_ID in :SETTINGS to stream IEX ETF proxies, or use :MARKET / :CRYPTO for full live Binance Spot coverage and :SCAN EQUITIES for SEC EDGAR fundamentals.",
        )
        .style(Style::default().fg(SECONDARY))
        .block(block(" Source Attribution & Licensing ", false)),
        rows[2],
    );
}

pub fn scan(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);
    match app.scan_mode {
        ScanMode::Crypto => {
            let keys = filtered_market_keys(app);
            let discovered = app.market.instruments.len().max(app.market.quotes.len());
            let trading = app
                .market
                .trading_instrument_count()
                .max(app.market.quotes.len());
            let visible_cap = rows[1].height.saturating_sub(3).max(1) as usize;
            let visible_count = keys.len().min(visible_cap);

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            " SCAN CRYPTO <SRCH> ",
                            Style::default().fg(BG).bg(AMBER).bold(),
                        ),
                        Span::styled(
                            format!(
                                "  Binance Spot │ {discovered} discovered pairs ({trading} trading) │ {} match filter │ {visible_count} visible",
                                keys.len()
                            ),
                            Style::default().fg(FG),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(" Mode[m]: ", Style::default().fg(SECONDARY)),
                        Span::styled("CRYPTO ", Style::default().fg(AMBER).bold()),
                        Span::styled("│ Quote[u]: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            format!("{} ", app.quote_filter),
                            Style::default().fg(AMBER).bold(),
                        ),
                        Span::styled("│ MinVol[l]: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            format!("{} ", number(app.min_quote_volume)),
                            Style::default().fg(FG),
                        ),
                        Span::styled("│ Status[x]: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            if app.status_filter_trading_only {
                                "TRADING "
                            } else {
                                "ALL "
                            },
                            Style::default().fg(FG),
                        ),
                        Span::styled("│ Sort[s/o]: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            format!(
                                "{} {} ",
                                app.sort.to_ascii_uppercase(),
                                if app.sort_ascending { "ASC" } else { "DESC" }
                            ),
                            Style::default().fg(AMBER).bold(),
                        ),
                        Span::styled("│ Filter(:FILTER): ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            if app.filter.is_empty() {
                                "none"
                            } else {
                                &app.filter
                            },
                            Style::default().fg(FG),
                        ),
                    ]),
                ])
                .style(Style::default().bg(ELEVATED_BG)),
                rows[0],
            );

            render_market_table(
                frame,
                app,
                rows[1],
                &keys,
                format!(
                    " Crypto Universe Screener — {} matching instruments (Enter: Open Chart, m: Switch to Equities) ",
                    keys.len()
                ),
                app.focus == 1,
            );
        }
        ScanMode::Equities => {
            let matched = filtered_equities(app);
            let visible_cap = rows[1].height.saturating_sub(3).max(1) as usize;
            let selected_idx = if matched.is_empty() {
                0
            } else {
                app.cursor.min(matched.len() - 1)
            };
            let win_start = if selected_idx >= visible_capacity_clamp(visible_cap) {
                selected_idx + 1 - visible_cap
            } else {
                0
            };
            let win_end = (win_start + visible_cap).min(matched.len());
            let visible_slice = if matched.is_empty() {
                &[][..]
            } else {
                &matched[win_start..win_end]
            };
            let last_indexed = app
                .companies
                .iter()
                .map(|c| c.fetched_at)
                .max()
                .map(|d| d.format("%Y-%m-%d %H:%M UTC").to_string())
                .unwrap_or_else(|| "none indexed yet (run :INDEX AAPL or :FIN MSFT)".into());

            frame.render_widget(
                Paragraph::new(vec![
                    Line::from(vec![
                        Span::styled(
                            " SCAN EQUITIES <EQS> ",
                            Style::default().fg(BG).bg(AMBER).bold(),
                        ),
                        Span::styled(
                            format!(
                                "  SEC EDGAR Local Index │ {} indexed companies │ {} match filter │ {} visible │ Last indexed: {}",
                                app.companies.len(),
                                matched.len(),
                                visible_slice.len(),
                                last_indexed
                            ),
                            Style::default().fg(FG),
                        ),
                    ]),
                    Line::from(vec![
                        Span::styled(" Mode[m]: ", Style::default().fg(SECONDARY)),
                        Span::styled("EQUITIES ", Style::default().fg(AMBER).bold()),
                        Span::styled("│ Sort[s/o]: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            format!(
                                "{} {} ",
                                app.sort.to_ascii_uppercase(),
                                if app.sort_ascending { "ASC" } else { "DESC" }
                            ),
                            Style::default().fg(AMBER).bold(),
                        ),
                        Span::styled("│ Filter: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            if app.filter.is_empty() {
                                "none"
                            } else {
                                &app.filter
                            },
                            Style::default().fg(FG),
                        ),
                        Span::styled(
                            " │ Scope: Indexed SEC XBRL USD filers (:INDEX <TICKER>). Live SIP P/E & Market Cap unsupported without paid equity feed.",
                            Style::default().fg(MUTED),
                        ),
                    ]),
                ])
                .style(Style::default().bg(ELEVATED_BG)),
                rows[0],
            );

            let sec_rows = visible_slice.iter().map(|c| {
                let f = facts::select(&c.facts, PeriodView::Annual);
                let sum = facts::summarize(&c.facts, PeriodView::Annual);
                let rev = f.iter().copied().find(|x| x.metric == "Revenue");
                let op = f.iter().copied().find(|x| x.metric == "Operating income");
                let ni = f.iter().copied().find(|x| x.metric == "Net income");
                let assets = f.iter().copied().find(|x| x.metric == "Total assets");
                let eq = f.iter().copied().find(|x| x.metric == "Equity");
                let mgn = sum
                    .as_ref()
                    .and_then(|s| s.net_margin_pct)
                    .map(|m| format!("{m:.1}%"))
                    .unwrap_or_else(|| "--".into());
                let fcf = sum
                    .as_ref()
                    .and_then(|s| s.free_cash_flow)
                    .map(number)
                    .unwrap_or_else(|| "--".into());
                Row::new(vec![
                    Cell::from(c.ticker.clone()).style(Style::default().fg(AMBER).bold()),
                    Cell::from(c.name.clone()).style(Style::default().fg(FG)),
                    Cell::from(
                        rev.map(|x| number(x.value))
                            .unwrap_or_else(|| "unavailable".into()),
                    ),
                    Cell::from(
                        op.map(|x| number(x.value))
                            .unwrap_or_else(|| "unavailable".into()),
                    ),
                    Cell::from(
                        ni.map(|x| number(x.value))
                            .unwrap_or_else(|| "unavailable".into()),
                    ),
                    Cell::from(
                        assets
                            .map(|x| number(x.value))
                            .unwrap_or_else(|| "unavailable".into()),
                    ),
                    Cell::from(
                        eq.map(|x| number(x.value))
                            .unwrap_or_else(|| "unavailable".into()),
                    ),
                    Cell::from(mgn).style(Style::default().fg(POSITIVE)),
                    Cell::from(fcf).style(Style::default().fg(FG)),
                    Cell::from(rev.map(|x| x.end.to_string()).unwrap_or_else(|| "—".into()))
                        .style(Style::default().fg(SECONDARY)),
                    Cell::from(c.fetched_at.format("%Y-%m-%d").to_string())
                        .style(Style::default().fg(MUTED)),
                ])
            });
            let mut state = TableState::default().with_selected(
                (!visible_slice.is_empty()).then_some(selected_idx.saturating_sub(win_start)),
            );
            frame.render_stateful_widget(
                Table::new(
                    sec_rows,
                    [
                        Constraint::Percentage(7),
                        Constraint::Percentage(17),
                        Constraint::Percentage(9),
                        Constraint::Percentage(9),
                        Constraint::Percentage(9),
                        Constraint::Percentage(10),
                        Constraint::Percentage(9),
                        Constraint::Percentage(8),
                        Constraint::Percentage(8),
                        Constraint::Percentage(7),
                        Constraint::Percentage(7),
                    ],
                )
                .header(
                    Row::new([
                        "TICKER",
                        "COMPANY NAME",
                        "REVENUE",
                        "OP INC",
                        "NET INC",
                        "ASSETS",
                        "EQUITY",
                        "NET MGN",
                        "FCF",
                        "FY END",
                        "INDEXED",
                    ])
                    .style(theme::header_style()),
                )
                .row_highlight_style(theme::row_highlight_style())
                .block(block(
                    format!(
                        " SEC EQS Fundamental Screener ({} cached companies) — :INDEX <TICKER> to add, Enter to open FIN ",
                        app.companies.len()
                    ),
                    app.focus == 1,
                )),
                rows[1],
                &mut state,
            );
        }
    }
}

fn visible_capacity_clamp(cap: usize) -> usize {
    cap.max(1)
}

pub fn financials(frame: &mut Frame, app: &App, area: Rect) {
    if app.des_mode {
        if let Some(company) = &app.company {
            render_security_description(frame, app, area, company);
        } else {
            render_crypto_security_description(frame, app, area);
        }
        return;
    }
    let Some(company) = &app.company else {
        unavailable(
            frame,
            app,
            area,
            "SEC Company Financial Analysis (FA)\n\n\
             Run :FIN AAPL (or :FA MSFT) after setting OMT_SEC_USER_AGENT='YourApp contact@domain.com'.\n\
             Provides multi-period comparative statements (Overview, Income Statement, Balance Sheet, Cash Flow, Ratios)\n\
             computed strictly from verified SEC EDGAR XBRL USD filings.",
        );
        return;
    };
    let table = facts::comparative_table(&company.facts, app.period, app.stmt_tab);
    let summary = facts::summarize(&company.facts, app.period);
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    let mut tab_spans = vec![
        Span::styled(
            format!(
                " {} ({}) CIK {:010} ",
                company.name, company.ticker, company.cik
            ),
            Style::default().fg(AMBER).bold(),
        ),
        Span::styled(
            format!("│ {:?} [a/v] │ ", app.period),
            Style::default().fg(FG),
        ),
    ];
    for tab in StatementTab::ALL {
        let active = tab == app.stmt_tab;
        tab_spans.push(Span::styled(
            format!(" {} ", tab.name()),
            theme::pill_style(active),
        ));
        tab_spans.push(Span::raw(" "));
    }

    let fmt_pct = |v: Option<Decimal>| v.map(|d| format!("{d:.1}%")).unwrap_or_else(|| "--".into());
    let ratio_line = if let Some(s) = summary {
        format!(
            " Latest period {} (USD) │ Gross Mgn: {}  Op Mgn: {}  Net Mgn: {}  OCF Mgn: {}  FCF: {}  ROE: {}  ROA: {}  Curr: {}  D/E: {}",
            s.latest_end,
            fmt_pct(s.gross_margin_pct),
            fmt_pct(s.operating_margin_pct),
            fmt_pct(s.net_margin_pct),
            fmt_pct(s.ocf_margin_pct),
            s.free_cash_flow.map(number).unwrap_or_else(|| "--".into()),
            fmt_pct(s.roe_pct),
            fmt_pct(s.roa_pct),
            s.current_ratio
                .map(|d| format!("{d:.2}x"))
                .unwrap_or_else(|| "--".into()),
            fmt_pct(s.debt_to_equity_pct),
        )
    } else {
        format!(" No compatible reported USD facts for {:?}", app.period)
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(tab_spans),
            Line::from(Span::styled(ratio_line, Style::default().fg(SECONDARY))),
        ])
        .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    let max_periods = if rows[1].width >= 95 { 5 } else { 3 };
    let shown_periods: Vec<_> = table.periods.iter().copied().take(max_periods).collect();
    let active_col = app.fin_col.min(shown_periods.len().saturating_sub(1));

    let mut headers = vec![Cell::from("METRIC (USD)").style(theme::header_style())];
    for (idx, p) in shown_periods.iter().enumerate() {
        let label = if idx == active_col {
            format!("▶{p}")
        } else {
            p.to_string()
        };
        headers.push(Cell::from(label).style(theme::header_style()));
    }
    headers.push(Cell::from("YOY %").style(theme::header_style()));

    let data_rows = table.rows.iter().map(|r| {
        let mut cells = vec![Cell::from(r.metric).style(Style::default().fg(FG).bold())];
        for (idx, val) in r.values.iter().take(shown_periods.len()).enumerate() {
            let formatted = match val {
                Some(v) if r.is_pct => format!("{v:.1}%"),
                Some(v) if r.is_multiple => format!("{v:.2}x"),
                Some(v) => number(*v),
                None => "—".into(),
            };
            let cell_style = if idx == active_col {
                Style::default().fg(AMBER).bold()
            } else {
                Style::default().fg(FG)
            };
            cells.push(Cell::from(formatted).style(cell_style));
        }
        let yoy_str = r
            .yoy_pct
            .map(|g| format!("{g:+.1}%"))
            .unwrap_or_else(|| "—".into());
        cells.push(
            Cell::from(yoy_str).style(Style::default().fg(theme::direction_color(r.yoy_pct))),
        );
        Row::new(cells)
    });

    let mut widths = vec![Constraint::Percentage(26)];
    let col_pct = (62 / shown_periods.len().max(1)) as u16;
    for _ in 0..shown_periods.len() {
        widths.push(Constraint::Percentage(col_pct));
    }
    widths.push(Constraint::Percentage(12));

    let mut state = TableState::default()
        .with_selected((!table.rows.is_empty()).then_some(app.cursor.min(table.rows.len() - 1)));
    frame.render_stateful_widget(
        Table::new(data_rows, widths)
            .header(Row::new(headers))
            .row_highlight_style(theme::row_highlight_style())
            .block(block(
                format!(
                    " {} — {} ({:?}) │ s/[ / ]: Switch Statement │ ←/→: Select Period Column │ j/k: Select Metric ",
                    company.ticker,
                    app.stmt_tab.name(),
                    app.period
                ),
                app.focus == 1,
            )),
        rows[1],
        &mut state,
    );
}

fn render_security_description(frame: &mut Frame, app: &App, area: Rect, company: &Company) {
    let summary = facts::summarize(&company.facts, app.period);
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .split(area);

    let hdr = vec![
        Line::from(vec![
            Span::styled(
                format!(
                    " SECURITY DESCRIPTION <DES> — {} ({}) ",
                    company.name, company.ticker
                ),
                Style::default().fg(BG).bg(AMBER).bold(),
            ),
            Span::styled(
                format!(
                    "  CIK: {:010} │ Venue: IEX (US Equity) │ Period: {:?}",
                    company.cik, app.period
                ),
                Style::default().fg(FG),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " Jurisdiction: SEC EDGAR US GAAP XBRL │ Currency: USD │ Verified Financial Reporting",
                Style::default().fg(SECONDARY),
            ),
            Span::styled(
                " │ Press 'd' to toggle Comparative Statements",
                Style::default().fg(AMBER),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(hdr).style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    let f = facts::select(&company.facts, PeriodView::Annual);
    let rev = f
        .iter()
        .copied()
        .find(|x| x.metric == "Revenue")
        .map(|x| number(x.value))
        .unwrap_or_else(|| "--".into());
    let op = f
        .iter()
        .copied()
        .find(|x| x.metric == "Operating income")
        .map(|x| number(x.value))
        .unwrap_or_else(|| "--".into());
    let ni = f
        .iter()
        .copied()
        .find(|x| x.metric == "Net income")
        .map(|x| number(x.value))
        .unwrap_or_else(|| "--".into());
    let assets = f
        .iter()
        .copied()
        .find(|x| x.metric == "Total assets")
        .map(|x| number(x.value))
        .unwrap_or_else(|| "--".into());
    let equity = f
        .iter()
        .copied()
        .find(|x| x.metric == "Equity")
        .map(|x| number(x.value))
        .unwrap_or_else(|| "--".into());

    let fmt_pct = |v: Option<Decimal>| v.map(|d| format!("{d:.1}%")).unwrap_or_else(|| "--".into());
    let gross_mgn = summary.as_ref().and_then(|s| s.gross_margin_pct);
    let op_mgn = summary.as_ref().and_then(|s| s.operating_margin_pct);
    let net_mgn = summary.as_ref().and_then(|s| s.net_margin_pct);
    let fcf = summary
        .as_ref()
        .and_then(|s| s.free_cash_flow)
        .map(number)
        .unwrap_or_else(|| "--".into());
    let roe = summary.as_ref().and_then(|s| s.roe_pct);
    let roa = summary.as_ref().and_then(|s| s.roa_pct);
    let curr = summary
        .as_ref()
        .and_then(|s| s.current_ratio)
        .map(|d| format!("{d:.2}x"))
        .unwrap_or_else(|| "--".into());
    let de = summary.as_ref().and_then(|s| s.debt_to_equity_pct);

    let desc_lines = vec![
        Line::from(vec![Span::styled(
            "1) CORE FINANCIAL SCALE (USD)",
            Style::default().fg(AMBER).bold(),
        )]),
        Line::from(format!(
            "   Revenue: {:>14}    Operating Income: {:>14}    Net Income: {:>14}",
            rev, op, ni
        )),
        Line::from(format!(
            "   Total Assets: {:>9}    Stockholders Equity: {:>8}    Free Cash Flow: {:>10}",
            assets, equity, fcf
        )),
        Line::from(""),
        Line::from(vec![Span::styled(
            "2) OPERATING MARGINS & CAPITAL EFFICIENCY",
            Style::default().fg(AMBER).bold(),
        )]),
        Line::from(format!(
            "   Gross Margin: {:>9}    Operating Margin: {:>10}    Net Margin: {:>14}",
            fmt_pct(gross_mgn),
            fmt_pct(op_mgn),
            fmt_pct(net_mgn)
        )),
        Line::from(format!(
            "   Return on Equity: {:>5}    Return on Assets: {:>9}    Current Ratio: {:>11}    D/E: {:>8}",
            fmt_pct(roe),
            fmt_pct(roa),
            curr,
            fmt_pct(de)
        )),
        Line::from(""),
        Line::from(vec![Span::styled(
            "3) REGULATORY DISCLOSURE ARCHIVE (SEC EDGAR)",
            Style::default().fg(AMBER).bold(),
        )]),
        Line::from(format!(
            "   Total Cached Filings: {} │ Recent Forms: 10-K, 10-Q, 8-K, DEF 14A",
            company.filings.len()
        )),
        Line::from(format!(
            "   Latest Filing Date: {} │ Accession: {}",
            company
                .filings
                .first()
                .map(|f| f.filed.to_string())
                .unwrap_or_else(|| "--".into()),
            company
                .filings
                .first()
                .map(|f| f.accession.as_str())
                .unwrap_or("--")
        )),
    ];

    frame.render_widget(
        Paragraph::new(desc_lines)
            .wrap(Wrap { trim: false })
            .block(block(
                format!(" Bloomberg Description <DES> — {} ", company.ticker),
                app.focus == 1,
            )),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new("Press 'd' or 's' to return to comparative statements (Overview, Income, Balance, Cash, Ratios). Press Enter or 'o' to open filing in browser.")
            .style(Style::default().fg(SECONDARY))
            .block(block(" Navigation ", false)),
        rows[2],
    );
}

fn render_crypto_security_description(frame: &mut Frame, app: &App, area: Rect) {
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(3),
    ])
    .split(area);

    let key = app.asset.key();
    let quote = app.market.quotes.get(&key);
    let info = app.market.instruments.get(&key);

    let hdr = vec![
        Line::from(vec![
            Span::styled(
                format!(" SECURITY DESCRIPTION <DES> — {} ", app.asset.symbol),
                Style::default().fg(BG).bg(AMBER).bold(),
            ),
            Span::styled(
                format!(
                    "  Venue: {} │ Asset Class: {:?} │ Currency: {}",
                    app.asset.venue,
                    app.asset.asset_class,
                    app.asset.currency()
                ),
                Style::default().fg(FG),
            ),
        ]),
        Line::from(vec![
            Span::styled(
                " Market Coverage: Binance Spot Global Exchange Feed │ Verified Pricing Feed",
                Style::default().fg(SECONDARY),
            ),
            Span::styled(
                " │ Press 'd' to toggle Candlestick Chart",
                Style::default().fg(AMBER),
            ),
        ]),
    ];
    frame.render_widget(
        Paragraph::new(hdr).style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    let price_str = quote
        .map(|q| format_price(q.price))
        .unwrap_or_else(|| "--".into());
    let chg_str = quote
        .and_then(|q| q.effective_change_pct())
        .map(|v| format!("{v:+.2}%"))
        .unwrap_or_else(|| "--".into());
    let high_str = quote
        .and_then(|q| q.high)
        .map(format_price)
        .unwrap_or_else(|| "--".into());
    let low_str = quote
        .and_then(|q| q.low)
        .map(format_price)
        .unwrap_or_else(|| "--".into());
    let base_vol = quote
        .and_then(|q| q.volume)
        .map(number)
        .unwrap_or_else(|| "--".into());
    let quote_vol = quote
        .and_then(|q| q.quote_volume)
        .map(number)
        .unwrap_or_else(|| "--".into());
    let status_str = info
        .map(|i| match i.status {
            TradingStatus::Trading => "TRADING",
            TradingStatus::Halt => "HALT",
            TradingStatus::Break => "BREAK",
            _ => "OTHER",
        })
        .unwrap_or("TRADING");
    let base_ccy = info
        .map(|i| i.base_asset.as_str())
        .unwrap_or(app.asset.base_asset());
    let quote_ccy = info
        .map(|i| i.quote_asset.as_str())
        .unwrap_or(app.asset.currency());

    let desc_lines = vec![
        Line::from(vec![Span::styled(
            "1) INSTRUMENT PROFILE & VENUE SPECIFICATIONS",
            Style::default().fg(AMBER).bold(),
        )]),
        Line::from(format!(
            "   Pair: {:>14}    Venue: {:>14}    Trading Status: {:>14}",
            app.asset.symbol, "Binance Spot", status_str
        )),
        Line::from(format!(
            "   Base Asset: {:>8}    Quote Asset: {:>6}    Settlement: {:>10}",
            base_ccy, quote_ccy, "Immediate Spot"
        )),
        Line::from(""),
        Line::from(vec![Span::styled(
            "2) ROLLING 24-HOUR MARKET STATISTICS",
            Style::default().fg(AMBER).bold(),
        )]),
        Line::from(format!(
            "   Latest Price: {:>10}    24h Change: {:>9}    24h High: {:>14}",
            price_str, chg_str, high_str
        )),
        Line::from(format!(
            "   24h Low: {:>15}    Base Vol: {:>11}    Quote Vol (24h): {:>8}",
            low_str, base_vol, quote_vol
        )),
        Line::from(""),
        Line::from(vec![Span::styled(
            "3) CHART & INDICATOR ENGINE METRICS",
            Style::default().fg(AMBER).bold(),
        )]),
        Line::from(format!(
            "   Cached History: {} bars │ Active Timeframe: {} │ Chart Key: {}",
            app.market.candles.len(),
            app.timeframe,
            app.market.chart_key
        )),
        Line::from(format!(
            "   Active Overlay: {} │ Sub-Indicator Strip: {}",
            app.overlay.label(),
            app.sub_indicator.label()
        )),
    ];

    frame.render_widget(
        Paragraph::new(desc_lines)
            .wrap(Wrap { trim: false })
            .block(block(
                format!(
                    " Bloomberg Description <DES> — {} (Crypto Spot) ",
                    app.asset.symbol
                ),
                app.focus == 1,
            )),
        rows[1],
    );

    frame.render_widget(
        Paragraph::new("Press 'd' to return to interactive candlestick graph. Press 't' to cycle timeframe, 'i' for overlay, 'g' for sub-indicator.")
            .style(Style::default().fg(SECONDARY))
            .block(block(" Navigation ", false)),
        rows[2],
    );
}

pub fn news(frame: &mut Frame, app: &App, area: Rect) {
    let items = filtered_news_items(app);
    let rows = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    let active_cat = app.news_category;
    let mut cat_spans = vec![
        Span::styled(
            " NEWS & DISCLOSURES <CN> ",
            Style::default().fg(BG).bg(AMBER).bold(),
        ),
        Span::raw(" "),
    ];
    for cat in NewsCategory::ALL {
        let is_active = cat == active_cat;
        cat_spans.push(Span::styled(
            format!(" {} ", cat.code()),
            theme::pill_style(is_active),
        ));
        cat_spans.push(Span::raw(" "));
    }
    cat_spans.push(Span::styled("│ ", Style::default().fg(BORDER)));
    let ticker_badge = if let Some(ref t) = app.news_ticker_filter {
        format!("[TICKER: {t} (press t to clear)]")
    } else {
        format!("[ALL TICKERS (press t to lock {})]", app.asset.base_asset())
    };
    cat_spans.push(Span::styled(
        ticker_badge,
        Style::default().fg(AMBER).bold(),
    ));
    cat_spans.push(Span::raw(" "));
    if active_cat == NewsCategory::Filings || app.company.is_some() {
        cat_spans.push(Span::styled(
            format!("[FORM: {} (f)] ", app.filing_form_filter),
            Style::default().fg(SECONDARY),
        ));
    }
    cat_spans.push(Span::styled(
        format!("│ {} stories ", items.len()),
        Style::default().fg(FG).bold(),
    ));
    if let Some(up) = app.news_updated_at {
        cat_spans.push(Span::styled(
            format!("│ Fresh: {} UTC ", up.format("%H:%M")),
            Style::default().fg(POSITIVE),
        ));
    }

    let sub_banner = Line::from(vec![
        Span::styled(
            " Sources: Yahoo Finance · CNBC · Federal Reserve · CoinDesk/Cointelegraph · SEC EDGAR │ ",
            Style::default().fg(SECONDARY),
        ),
        Span::styled(
            "c: Category │ t: Ticker │ f: Form │ Enter/r: Read Story │ [/]: Scroll │ o: Browser │ R: Refresh",
            Style::default().fg(AMBER),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(vec![Line::from(cat_spans), sub_banner])
            .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    if items.is_empty() {
        unavailable(
            frame,
            app,
            rows[1],
            "Financial News & SEC Regulatory Disclosure Reader (TOP / CN)\n\n\
             No stories match the active filters.\n\n\
             Controls:\n\
             • Press 'c' to cycle news categories (ALL, MKT, EQTY, CRYP, MACR, EARN, SEC)\n\
             • Press 't' to clear or toggle the ticker filter\n\
             • Press 'R' to refresh live multi-source feeds\n\
             • Commands: :TOP, :NEWS CRYPTO, :NEWS MACRO, :NEWS AAPL, :FILINGS MSFT\n\n\
             Attribution Policy: Zero fabricated reporting. Headlines and texts originate strictly from verified public feeds and official SEC EDGAR archives.",
        );
        return;
    }

    let selected_idx = app.cursor.min(items.len().saturating_sub(1));
    let selected_item = items.get(selected_idx).cloned();

    if app.news_reader_expanded {
        render_news_reader_pane(frame, app, rows[1], selected_item.as_ref(), true);
    } else if rows[1].width >= 88 {
        let cols = Layout::horizontal([Constraint::Percentage(52), Constraint::Percentage(48)])
            .split(rows[1]);
        render_news_table(frame, app, cols[0], &items, selected_idx);
        render_news_reader_pane(frame, app, cols[1], selected_item.as_ref(), false);
    } else {
        render_news_table(frame, app, rows[1], &items, selected_idx);
    }
}

fn render_news_table(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    items: &[omt_core::NewsItem],
    selected_idx: usize,
) {
    let visible_cap = area.height.saturating_sub(3).max(1) as usize;
    let win_start = if selected_idx >= visible_cap {
        selected_idx + 1 - visible_cap
    } else {
        0
    };
    let win_end = (win_start + visible_cap).min(items.len());
    let slice = &items[win_start..win_end];
    let now = Utc::now();

    let rows = slice.iter().map(|item| {
        let age_mins = now.signed_duration_since(item.published_at).num_minutes();
        let age_str = if age_mins < 60 {
            format!("{}m", age_mins.max(1))
        } else if age_mins < 1440 {
            format!("{}h", age_mins / 60)
        } else {
            format!("{}d", age_mins / 1440)
        };
        let time_str = if age_mins < 1440 {
            item.published_at.format("%H:%M").to_string()
        } else {
            item.published_at.format("%m-%d").to_string()
        };
        let is_read = app.news_read_ids.contains(&item.id);
        let read_glyph = if is_read { "·" } else { "●" };
        let read_color = if is_read { MUTED } else { AMBER };

        let cat_color = match item.category {
            NewsCategory::Markets => AMBER,
            NewsCategory::Equities => POSITIVE,
            NewsCategory::Crypto => theme::INFO_BLUE,
            NewsCategory::Macro => GOLD,
            NewsCategory::Earnings => Color::Rgb(102, 204, 204),
            NewsCategory::Filings => Color::Rgb(255, 153, 51),
            NewsCategory::All => FG,
        };

        let src_short = match item.source.as_str() {
            s if s.contains("SEC") => "EDGAR",
            s if s.contains("Yahoo") => "Yahoo",
            s if s.contains("CNBC") => "CNBC",
            s if s.contains("Federal") => "Fed",
            s if s.contains("CoinDesk") => "CoinDesk",
            s if s.contains("Cointelegraph") => "CoinTel",
            s => s,
        };

        Row::new(vec![
            Cell::from(time_str).style(Style::default().fg(SECONDARY)),
            Cell::from(src_short).style(Style::default().fg(SECONDARY)),
            Cell::from(item.ticker.clone().unwrap_or_else(|| "--".into()))
                .style(Style::default().fg(AMBER).bold()),
            Cell::from(item.category.code()).style(Style::default().fg(cat_color).bold()),
            Cell::from(read_glyph).style(Style::default().fg(read_color).bold()),
            Cell::from(item.headline.clone()).style(Style::default().fg(if is_read {
                SECONDARY
            } else {
                FG
            })),
            Cell::from(age_str).style(Style::default().fg(MUTED)),
        ])
    });

    let mut state = TableState::default()
        .with_selected((!slice.is_empty()).then_some(selected_idx.saturating_sub(win_start)));

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(8),
            Constraint::Length(8),
            Constraint::Length(6),
            Constraint::Length(3),
            Constraint::Min(24),
            Constraint::Length(5),
        ],
    )
    .header(
        Row::new(["TIME", "SRC", "TICKER", "CAT", "ST", "HEADLINE", "AGE"])
            .style(theme::header_style()),
    )
    .row_highlight_style(theme::row_highlight_style())
    .block(block(
        format!(
            " {} Headlines ({} shown) │ Enter/r: Read │ j/k: Select ",
            app.news_category.label(),
            items.len()
        ),
        app.focus == 1 && !app.news_reader_expanded,
    ));
    frame.render_stateful_widget(table, area, &mut state);
}

fn render_news_reader_pane(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    item: Option<&omt_core::NewsItem>,
    expanded: bool,
) {
    let Some(item) = item else {
        frame.render_widget(
            Paragraph::new("Select a news headline or SEC filing on the left to read full text.")
                .style(Style::default().fg(SECONDARY))
                .block(block(" Story Reader ", false)),
            area,
        );
        return;
    };

    let title_prefix = if expanded {
        " [EXPANDED READER — Press 'r' or Esc to return to split table] "
    } else {
        " Story & Disclosure Reader (Press 'r' to expand) "
    };

    let mut lines = Vec::new();

    // Headline Header
    lines.push(Line::from(Span::styled(
        item.headline.clone(),
        Style::default().fg(AMBER).bold(),
    )));
    lines.push(Line::from(vec![
        Span::styled(
            format!("Source: {} │ ", item.source),
            Style::default().fg(SECONDARY),
        ),
        Span::styled(
            format!("Category: {} │ ", item.category.label()),
            Style::default().fg(SECONDARY),
        ),
        Span::styled(
            format!("Ticker: {} │ ", item.ticker.as_deref().unwrap_or("GLOBAL")),
            Style::default().fg(AMBER),
        ),
        Span::styled(
            format!(
                "Published: {} UTC",
                item.published_at.format("%Y-%m-%d %H:%M:%S")
            ),
            Style::default().fg(FG),
        ),
    ]));

    if item.is_sec_filing {
        let form_badge = item.form.as_deref().unwrap_or("EDGAR");
        let acc = item.accession.as_deref().unwrap_or("N/A");
        lines.push(Line::from(Span::styled(
            format!("OFFICIAL SEC REGULATORY DISCLOSURE — FORM {form_badge} │ Accession: {acc}"),
            Style::default().fg(Color::Rgb(255, 153, 51)).bold(),
        )));
    } else {
        lines.push(Line::from(Span::styled(
            format!(
                "PUBLIC FINANCIAL NEWS FEED — Attribution: Grounded in {}",
                item.source
            ),
            Style::default().fg(Color::Rgb(102, 204, 204)),
        )));
    }
    lines.push(Line::from(Span::styled(
        format!("URL: {}", item.url),
        Style::default().fg(MUTED),
    )));
    lines.push(Line::from(Span::styled(
        "─".repeat(area.width.saturating_sub(4) as usize),
        Style::default().fg(BORDER),
    )));

    // Body or Summary Content
    let full_content = if let Some(ref body) = item.body {
        body.as_str()
    } else {
        item.summary.as_str()
    };

    let mut body_paragraphs: Vec<&str> = full_content.split("\n\n").collect();
    if item.body.is_none() {
        body_paragraphs.push("────────────────────────────────────────────────────────────");
        body_paragraphs.push("Action:\n• Press Enter or 'r' to fetch & extract full text into this reader.\n• Press 'o' to open original canonical story URL in your browser.\n• Scroll reader using '[' and ']' or PageUp / PageDown.");
    }

    let skip_lines = app.news_scroll as usize;
    let mut current_line_idx = 0usize;
    for p in body_paragraphs {
        for l in p.lines() {
            if current_line_idx >= skip_lines {
                lines.push(Line::from(Span::styled(
                    l.to_string(),
                    Style::default().fg(FG),
                )));
            }
            current_line_idx += 1;
        }
        if current_line_idx >= skip_lines {
            lines.push(Line::from(""));
        }
        current_line_idx += 1;
    }

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block(
                title_prefix,
                app.news_reader_expanded || app.focus == 1,
            )),
        area,
    );
}

pub fn portfolio(frame: &mut Frame, app: &App, area: Rect) {
    let transactions: Vec<_> = app
        .transactions
        .iter()
        .filter(|t| t.account == app.account)
        .cloned()
        .collect();
    let Ok(positions) = positions(&transactions) else {
        unavailable(frame, app, area, "Invalid portfolio records; see status");
        return;
    };
    if positions.is_empty() {
        unavailable(
            frame,
            app,
            area,
            "Local Portfolio & Position Blotter (PRTU) — No Holdings Yet\n\n\
             Record transactions locally (persisted in SQLite with full average-cost accounting):\n\
             • :BUY BTCUSDT 0.1 60000 2 2025-01-01T12:00:00Z\n\
             • :SELL BTCUSDT 0.05 65000 1\n\
             • :DELTX <transaction_id>   │   :ACCOUNT <name>\n\
             • :IMPORT path.csv          │   :EXPORT new.csv\n\
              Strict currency separation (USD vs USDT); no synthetic holdings or cross-currency mixing.",
        );
        return;
    }

    let now = Utc::now();
    let mut totals =
        std::collections::BTreeMap::<String, (Decimal, Decimal, Decimal, Decimal)>::new();
    for p in &positions {
        let entry = totals.entry(p.currency.clone()).or_default();
        entry.1 += p.cost;
        entry.2 += p.realized;
        if let Some(q) = app
            .market
            .quotes
            .get(&p.asset.key())
            .filter(|q| matches!(q.status_at(now), DataStatus::Live | DataStatus::Delayed))
        {
            let mv = p.quantity * q.price;
            entry.0 += mv;
            entry.3 += mv - p.cost;
        }
    }

    let summary_text = totals
        .iter()
        .map(|(ccy, (mv, cost, real, unreal))| {
            let pct = if *cost > Decimal::ZERO && *mv > Decimal::ZERO {
                format!(" ({:+.2}%)", *unreal / *cost * Decimal::from(100))
            } else {
                String::new()
            };
            format!(
                "{ccy}: Value {} │ Basis {} │ Unrealized {}{pct} │ Realized {}",
                if *mv > Decimal::ZERO {
                    number(*mv)
                } else {
                    "unpriced".into()
                },
                number(*cost),
                if *mv > Decimal::ZERO {
                    format_signed_price(*unreal)
                } else {
                    "unavailable".into()
                },
                format_signed_price(*real)
            )
        })
        .collect::<Vec<_>>()
        .join("   ║   ");

    let data = positions.iter().map(|p| {
        let q = app.market.quotes.get(&p.asset.key());
        let valid =
            q.filter(|q| matches!(q.status_at(now), DataStatus::Live | DataStatus::Delayed));
        let allocation = valid
            .and_then(|q| {
                totals
                    .get(&p.currency)
                    .filter(|(mv, ..)| *mv > Decimal::ZERO)
                    .map(|(mv, ..)| {
                        format!("{:.1}%", p.quantity * q.price / *mv * Decimal::from(100))
                    })
            })
            .unwrap_or_else(|| "--".into());
        let avg = p
            .average_cost()
            .map(format_price)
            .unwrap_or_else(|| "--".into());
        let mkt_val = valid
            .and_then(|q| p.market_value(q.price, &q.currency))
            .map(number)
            .unwrap_or_else(|| "unavailable".into());
        let unreal = valid.and_then(|q| p.unrealized(q.price, &q.currency));
        let unreal_pct = valid.and_then(|q| p.unrealized_pct(q.price, &q.currency));
        let unreal_str = match (unreal, unreal_pct) {
            (Some(u), Some(pct)) => format!("{} ({pct:+.1}%)", number(u)),
            (Some(u), None) => number(u),
            _ => "unavailable".into(),
        };
        Row::new(vec![
            Cell::from(p.asset.key()).style(Style::default().fg(AMBER).bold()),
            Cell::from(number(p.quantity)).style(Style::default().fg(FG)),
            Cell::from(avg).style(Style::default().fg(SECONDARY)),
            Cell::from(format!("{} {}", number(p.cost), p.currency)).style(Style::default().fg(FG)),
            Cell::from(mkt_val).style(Style::default().fg(FG).bold()),
            Cell::from(number(p.realized))
                .style(Style::default().fg(theme::direction_color(Some(p.realized)))),
            Cell::from(unreal_str).style(Style::default().fg(theme::direction_color(unreal))),
            Cell::from(allocation).style(Style::default().fg(AMBER)),
            Cell::from(
                q.map(|q| format!("{} ({:?})", format_price(q.price), q.status_at(now)))
                    .unwrap_or_else(|| "No quote".into()),
            )
            .style(Style::default().fg(SECONDARY)),
        ])
    });

    let holdings_h = (positions.len() as u16 + 4).clamp(6, area.height.saturating_sub(10).max(6));
    let rows = if area.height >= 16 {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(holdings_h),
            Constraint::Min(6),
        ])
        .split(area)
    } else {
        Layout::vertical([
            Constraint::Length(2),
            Constraint::Min(5),
            Constraint::Length(0),
        ])
        .split(area)
    };

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    format!(" PORTFOLIO <PRTU> — Account: {} ", app.account),
                    Style::default().fg(BG).bg(AMBER).bold(),
                ),
                Span::styled(
                    format!(
                        "  {} open positions │ {} recorded fills",
                        positions.len(),
                        transactions.len()
                    ),
                    Style::default().fg(FG),
                ),
            ]),
            Line::from(Span::styled(
                format!(" {summary_text}"),
                Style::default().fg(SECONDARY),
            )),
        ])
        .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    let mut pos_state =
        TableState::default().with_selected(Some(app.cursor.min(positions.len() - 1)));
    frame.render_stateful_widget(
        Table::new(
            data,
            [
                Constraint::Percentage(15),
                Constraint::Percentage(8),
                Constraint::Percentage(10),
                Constraint::Percentage(13),
                Constraint::Percentage(11),
                Constraint::Percentage(9),
                Constraint::Percentage(14),
                Constraint::Percentage(7),
                Constraint::Percentage(13),
            ],
        )
        .header(
            Row::new([
                "ASSET",
                "QTY",
                "AVG COST",
                "COST BASIS",
                "MKT VALUE",
                "REALIZED",
                "UNREALIZED P&L",
                "ALLOC*",
                "LAST / STATUS",
            ])
            .style(theme::header_style()),
        )
        .row_highlight_style(theme::row_highlight_style())
        .block(block(
            format!(
                " Holdings ({}) — *Allocation within identical quote currency (Enter: Open Chart) ",
                app.account
            ),
            app.focus == 1,
        )),
        rows[1],
        &mut pos_state,
    );

    if rows[2].height > 0 {
        let max_tx = rows[2].height.saturating_sub(3).max(1) as usize;
        let tx_rows = transactions.iter().rev().take(max_tx).map(|t| {
            let side_color = if t.side == omt_core::portfolio::Side::Buy {
                POSITIVE
            } else {
                NEGATIVE
            };
            Row::new(vec![
                Cell::from(t.id.clone()).style(Style::default().fg(SECONDARY)),
                Cell::from(t.at.format("%Y-%m-%d %H:%M:%S").to_string()),
                Cell::from(format!("{:?}", t.side)).style(Style::default().fg(side_color).bold()),
                Cell::from(t.asset.key()).style(Style::default().fg(AMBER)),
                Cell::from(number(t.quantity)),
                Cell::from(format!("{} {}", format_price(t.price), t.currency)),
                Cell::from(number(t.fee)).style(Style::default().fg(MUTED)),
            ])
        });
        frame.render_widget(
            Table::new(
                tx_rows,
                [
                    Constraint::Percentage(22),
                    Constraint::Percentage(18),
                    Constraint::Percentage(8),
                    Constraint::Percentage(18),
                    Constraint::Percentage(10),
                    Constraint::Percentage(14),
                    Constraint::Percentage(10),
                ],
            )
            .header(
                Row::new(["TX ID", "UTC TIME", "SIDE", "ASSET", "QTY", "PRICE", "FEE"])
                    .style(theme::header_style()),
            )
            .block(block(
                " Execution Blotter / Recent Fills (:DELTX <id>) ",
                false,
            )),
            rows[2],
        );
    }
}

pub fn economics(frame: &mut Frame, app: &App, area: Rect) {
    if app.macro_data.is_empty() {
        unavailable(
            frame,
            app,
            area,
            "Macroeconomic & Fixed Income Workstation (ECST / YCRV)\n\n\
             Commands:\n\
             • :TREASURY or :YCRV        US Treasury Daily Par Yield Curve (1M to 30Y maturities)\n\
             • :WB US NY.GDP.MKTP.CD     World Bank Sovereign Indicators (GDP, CPI, Unemployment)\n\
             • :FRED GDP                 St. Louis Fed FRED Series (requires FRED_API_KEY in :SETTINGS)",
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(3),
        Constraint::Min(8),
        Constraint::Length(10),
    ])
    .split(area);
    let first = &app.macro_data[0];
    let curve_info = analyze_yield_curve(&app.macro_data)
        .map(|yc| {
            format!(
                " │ 2s10s: {} bps  3m10s: {} bps  5s30s: {} bps │ Regime: {}",
                yc.spread_2s10s_bps
                    .map(|v| format!("{v:+.0}"))
                    .unwrap_or_else(|| "--".into()),
                yc.spread_3m10s_bps
                    .map(|v| format!("{v:+.0}"))
                    .unwrap_or_else(|| "--".into()),
                yc.spread_5s30s_bps
                    .map(|v| format!("{v:+.0}"))
                    .unwrap_or_else(|| "--".into()),
                yc.regime
            )
        })
        .unwrap_or_default();

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    format!(" {} ", first.source.to_ascii_uppercase()),
                    Style::default().fg(BG).bg(AMBER).bold(),
                ),
                Span::styled(
                    format!("  {}{}", first.label, curve_info),
                    Style::default().fg(FG).bold(),
                ),
            ]),
            Line::from(Span::styled(
                format!(
                    " Geography: {} │ Unit: {} │ Frequency: {} │ Published: {} │ {}",
                    first.geography,
                    first.unit,
                    first.frequency,
                    first.updated.as_deref().unwrap_or(&first.date),
                    app.research_status
                ),
                Style::default().fg(SECONDARY),
            )),
        ])
        .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    let treasury = first.source == "US Treasury";
    let mut data: Vec<_> = app
        .macro_data
        .iter()
        .take(100)
        .filter_map(|r| Some((macro_x(r)?, r.value.to_f64()?)))
        .collect();
    data.sort_by(|a, b| a.0.total_cmp(&b.0));
    if data.is_empty() {
        return;
    }
    let xmin = data[0].0;
    let xmax = data[data.len() - 1].0.max(xmin + 1.0);
    let min = data.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max = data.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let pad = ((max - min) * 0.12).max(0.05);

    let selected_idx = app.cursor.min(app.macro_data.len() - 1);
    let selected_point: Vec<(f64, f64)> = app
        .macro_data
        .get(selected_idx)
        .and_then(|r| Some((macro_x(r)?, r.value.to_f64()?)))
        .into_iter()
        .collect();

    let x_labels = if treasury {
        vec![
            "1M (0.1y)".to_string(),
            "5Y".to_string(),
            "10Y".to_string(),
            "20Y".to_string(),
            format!("{:.0}Y", xmax / 12.0),
        ]
    } else {
        let d_min = app
            .macro_data
            .iter()
            .map(|r| r.date.clone())
            .min()
            .unwrap_or_default();
        let d_max = app
            .macro_data
            .iter()
            .map(|r| r.date.clone())
            .max()
            .unwrap_or_default();
        vec![d_min, d_max]
    };
    let mid = (min + max) * 0.5;
    let chart = Chart::new(vec![
        Dataset::default()
            .name("Yield / Series")
            .data(&data)
            .graph_type(GraphType::Line)
            .marker(if app.ascii {
                symbols::Marker::Dot
            } else {
                symbols::Marker::Braille
            })
            .style(Style::default().fg(AMBER)),
        Dataset::default()
            .name("Tenor Markers")
            .data(&data)
            .graph_type(GraphType::Scatter)
            .marker(symbols::Marker::Dot)
            .style(Style::default().fg(POSITIVE)),
        Dataset::default()
            .name("Selected")
            .data(&selected_point)
            .graph_type(GraphType::Scatter)
            .marker(symbols::Marker::Block)
            .style(Style::default().fg(NEGATIVE)),
    ])
    .x_axis(
        Axis::default()
            .title(Span::styled(
                if treasury {
                    "Maturity Scale (Months -> Years)"
                } else {
                    "Observation Date"
                },
                Style::default().fg(SECONDARY),
            ))
            .bounds([xmin, xmax])
            .labels(x_labels),
    )
    .y_axis(Axis::default().bounds([min - pad, max + pad]).labels([
        format!("{min:.2}%"),
        format!("{mid:.2}%"),
        format!("{max:.2}%"),
    ]))
    .block(block(
        if treasury {
            " US Treasury Par Yield Curve (Proportional Maturity Axis) — j/k to Inspect Tenor "
        } else {
            " Historical Macroeconomic Series (Chronological) "
        },
        app.focus == 1,
    ));
    frame.render_widget(chart, rows[1]);

    // Underlying Maturity / Observation Table with Stateful Selection
    let visible_cap = rows[2].height.saturating_sub(3).max(1) as usize;
    let win_start = if selected_idx >= visible_cap {
        selected_idx + 1 - visible_cap
    } else {
        0
    };
    let win_end = (win_start + visible_cap).min(app.macro_data.len());
    let slice = &app.macro_data[win_start..win_end];
    let y2_val = app
        .macro_data
        .iter()
        .find(|r| r.series == "BC_2YEAR")
        .map(|r| r.value);

    let obs_rows = slice.iter().map(|r| {
        let vs_2y = if treasury {
            y2_val
                .map(|y2| format!("{:+.0} bps", (r.value - y2) * Decimal::from(100)))
                .unwrap_or_else(|| "—".into())
        } else {
            "—".into()
        };
        Row::new(vec![
            Cell::from(if treasury {
                r.label.clone()
            } else {
                r.series.clone()
            })
            .style(Style::default().fg(AMBER).bold()),
            Cell::from(r.date.clone()).style(Style::default().fg(FG)),
            Cell::from(format!("{}", r.value.round_dp(3).normalize()))
                .style(Style::default().fg(POSITIVE).bold()),
            Cell::from(r.unit.clone()).style(Style::default().fg(SECONDARY)),
            Cell::from(vs_2y).style(Style::default().fg(FG)),
            Cell::from(r.source.clone()).style(Style::default().fg(MUTED)),
        ])
    });
    let mut table_state =
        TableState::default().with_selected(Some(selected_idx.saturating_sub(win_start)));
    frame.render_stateful_widget(
        Table::new(
            obs_rows,
            [
                Constraint::Percentage(16),
                Constraint::Percentage(16),
                Constraint::Percentage(16),
                Constraint::Percentage(22),
                Constraint::Percentage(14),
                Constraint::Percentage(16),
            ],
        )
        .header(
            Row::new([
                if treasury { "TENOR" } else { "SERIES" },
                "OBS DATE",
                "VALUE",
                "UNIT",
                if treasury { "SPREAD VS 2Y" } else { "DELTA" },
                "SOURCE",
            ])
            .style(theme::header_style()),
        )
        .row_highlight_style(theme::row_highlight_style())
        .block(block(
            " Maturity & Observation Table (j/k to select) ",
            false,
        )),
        rows[2],
        &mut table_state,
    );
}

fn unavailable(frame: &mut Frame, app: &App, area: Rect, text: &str) {
    frame.render_widget(
        Paragraph::new(format!("{text}\n\nStatus: {}", app.research_status))
            .wrap(Wrap { trim: false })
            .block(block(
                format!(" {} <{}> ", app.workspace.name(), app.workspace.mnemonic()),
                app.focus == 1,
            )),
        area,
    );
}

pub fn chart(frame: &mut Frame, app: &App, area: Rect) {
    if app.des_mode {
        if let Some(company) = &app.company {
            render_security_description(frame, app, area, company);
        } else {
            render_crypto_security_description(frame, app, area);
        }
        return;
    }
    frame.render_widget(CandleChart { app }, area);
}

struct CandleChart<'a> {
    app: &'a App,
}

impl Widget for CandleChart<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let app = self.app;
        let live_label = if app.follow_live && app.offset == 0 {
            "LIVE"
        } else {
            "HIST PAN"
        };
        let outer = block(
            format!(
                " {} <GP> │ {} │ {} │ {} │ Overlay[i]: {} │ Sub[g]: {} ",
                app.asset.key(),
                app.timeframe,
                app.asset.currency(),
                live_label,
                app.overlay.label(),
                app.sub_indicator.label()
            ),
            app.focus == 1,
        );
        let inner = outer.inner(area);
        outer.render(area, buf);
        if inner.width < 18 || inner.height < 8 {
            return;
        }
        if app.market.candles.is_empty() {
            Paragraph::new(
                "No historical candles loaded for this instrument.\n\
                 Waiting for provider response or local SQLite cache.\n\
                 Controls:\n\
                 • t: Timeframe (1m, 5m, 15m, 1h, 4h, 1d)\n\
                 • i: Overlay (SMA, EMA, Bollinger, None)\n\
                 • g: Sub-Indicator (Volume, RSI, MACD, ATR)\n\
                 • +/-: Zoom in / Zoom out\n\
                 • h/l: Pan historical bars",
            )
            .style(Style::default().fg(SECONDARY))
            .render(inner, buf);
            return;
        }

        // Horizontal plotting geometry
        let y_axis_width = 13u16;
        let right_margin = 2u16;
        let plot_width = inner
            .width
            .saturating_sub(y_axis_width + right_margin)
            .max(2);
        let plot_left = inner.x;
        let plot_right = inner.x + plot_width;

        // Determine zoom stride and candle body width
        let (stride, body_w) = if app.zoom > 140 {
            (1usize, 1u16)
        } else if app.zoom >= 50 {
            (2usize, 1u16)
        } else if app.zoom >= 25 {
            (3usize, 2u16)
        } else {
            (4usize, 3u16)
        };

        let capacity = (plot_width as usize / stride).max(1);
        let end = app.market.candles.len().saturating_sub(app.offset).max(1);
        let count = capacity.min(end);
        let start = end - count;
        let bars = &app.market.candles[start..end];
        if bars.is_empty() {
            return;
        }

        let cursor_idx = app
            .candle_cursor
            .map(|c| bars.len().saturating_sub(1 + c))
            .unwrap_or_else(|| bars.len() - 1);
        let inspected = &bars[cursor_idx];

        // Vertical geometry
        let sub_h = if inner.height >= 18 { 4u16 } else { 2u16 };
        let plot_y = inner.y + 1;
        let plot_h = inner.height.saturating_sub(3 + sub_h).max(3);
        let sub_y = plot_y + plot_h;

        // Price scaling across visible bars
        let prices: Vec<_> = bars
            .iter()
            .filter_map(|b| Some((b.low.to_f64()?, b.high.to_f64()?)))
            .collect();
        let low = prices.iter().map(|v| v.0).fold(f64::INFINITY, f64::min);
        let high = prices.iter().map(|v| v.1).fold(f64::NEG_INFINITY, f64::max);
        let pad = ((high - low) * 0.05)
            .max(high.abs() * 0.00001)
            .max(0.000001);
        let min = low - pad;
        let max = high + pad;
        let y = |v: f64| {
            plot_y
                + (((max - v) / (max - min)) * (plot_h - 1) as f64).clamp(0.0, (plot_h - 1) as f64)
                    as u16
        };

        // Top OHLCV + Cursor Inspection Strip
        let inspect_end = start + cursor_idx + 1;
        let rsi14 = rsi(&app.market.candles[..inspect_end], 14)
            .last()
            .and_then(|v| *v)
            .map(|v| format!("{v:.1}"))
            .unwrap_or_else(|| "--".into());
        let atr14 = atr(&app.market.candles[..inspect_end], 14)
            .last()
            .and_then(|v| *v)
            .map(format_price)
            .unwrap_or_else(|| "--".into());
        let bar_chg = if inspected.open > Decimal::ZERO {
            (inspected.close - inspected.open) / inspected.open * Decimal::from(100)
        } else {
            Decimal::ZERO
        };

        buf.set_string(
            inner.x,
            inner.y,
            format!(
                "{}{} UTC │ O:{} H:{} L:{} C:{} ({:+.2}%) V:{} │ RSI14:{} ATR14:{} │ Zoom:{} (stride {}c/bar)",
                if app.candle_cursor.is_some() { "[INSPECT] " } else { "" },
                inspected.open_at.format("%Y-%m-%d %H:%M"),
                format_price(inspected.open),
                format_price(inspected.high),
                format_price(inspected.low),
                format_price(inspected.close),
                bar_chg,
                number(inspected.volume),
                rsi14,
                atr14,
                app.zoom,
                stride
            ),
            Style::default().fg(if app.candle_cursor.is_some() {
                AMBER
            } else {
                FG
            }),
        );

        // If bars.len() < capacity, render honest limited-history badge
        if bars.len() < capacity {
            buf.set_string(
                inner.x + 1,
                plot_y + 1,
                format!("[LIMITED HISTORY: {} bars available]", bars.len()),
                Style::default().fg(MUTED),
            );
        }

        // Draw horizontal 2D crosshair line if cursor is active
        if app.candle_cursor.is_some()
            && let Some(c_f64) = inspected.close.to_f64()
        {
            let cursor_y = y(c_f64);
            let crosshair_sym = if app.ascii { "-" } else { "┈" };
            for col in plot_left..plot_right {
                buf.set_string(col, cursor_y, crosshair_sym, Style::default().fg(BORDER));
            }
        }

        // Compute right-aligned horizontal coordinate x for each bar i
        let x_last = plot_right.saturating_sub(stride as u16);
        let tf_secs = app.timeframe.seconds().max(60);
        let mut bar_x_coords = vec![0u16; bars.len()];
        let mut cur_slot = 0usize;
        bar_x_coords[bars.len() - 1] = x_last;
        for k in (0..bars.len() - 1).rev() {
            let delta = (bars[k + 1].open_at - bars[k].open_at)
                .num_seconds()
                .max(tf_secs);
            let gap = ((delta + tf_secs / 2) / tf_secs).clamp(1, 4) as usize;
            cur_slot += gap;
            let offset_cols = (cur_slot * stride) as u16;
            bar_x_coords[k] = x_last.saturating_sub(offset_cols);
        }

        // Indicators for overlays
        let ma20 = sma(&app.market.candles, 20);
        let ma50 = sma(&app.market.candles, 50);
        let ema12 = ema(&app.market.candles, 12);
        let ema26 = ema(&app.market.candles, 26);
        let bb20 = bollinger_bands(&app.market.candles, 20, 2);

        // Render each visible bar
        for (i, b) in bars.iter().enumerate() {
            let x = bar_x_coords[i];
            if x < plot_left || x >= plot_right {
                continue;
            }
            let is_cursor = app.candle_cursor.is_some() && i == cursor_idx;
            if is_cursor {
                for row in plot_y..sub_y + sub_h {
                    buf.set_string(
                        x,
                        row,
                        if app.ascii { ":" } else { "┊" },
                        Style::default().fg(BORDER).bg(SELECTION),
                    );
                }
            }
            let color = theme::direction_color(Some(b.close - b.open));
            let bg = if is_cursor { SELECTION } else { PANEL_BG };
            let Some((l, h)) = prices.get(i) else {
                continue;
            };

            // Wick
            let wick_x = x + (body_w.saturating_sub(1) / 2);
            for row in y(*h)..=y(*l) {
                buf.set_string(
                    wick_x,
                    row,
                    if app.ascii { "|" } else { "│" },
                    Style::default().fg(color).bg(bg),
                );
            }

            // Candle Body
            let body_glyph = if !b.closed {
                if app.ascii { ":" } else { "▒" }
            } else if app.ascii {
                "#"
            } else {
                "█"
            };
            if let (Some(o), Some(c)) = (b.open.to_f64(), b.close.to_f64()) {
                let top_y = y(o.max(c));
                let bot_y = y(o.min(c));
                for row in top_y..=bot_y {
                    for bw in 0..body_w {
                        buf.set_string(x + bw, row, body_glyph, Style::default().fg(color).bg(bg));
                    }
                }
            }

            // Overlays
            match app.overlay {
                OverlayMode::Sma => {
                    for (ma, glyph, c) in [(&ma20, ".", AMBER), (&ma50, "+", theme::INFO_BLUE)] {
                        if let Some(v) = ma[start + i]
                            .and_then(|d| d.to_f64())
                            .filter(|v| *v >= min && *v <= max)
                        {
                            buf.set_string(
                                x.saturating_add(body_w),
                                y(v),
                                glyph,
                                Style::default().fg(c),
                            );
                        }
                    }
                }
                OverlayMode::Ema => {
                    for (ma, glyph, c) in [
                        (&ema12, ".", AMBER),
                        (&ema26, "+", Color::Rgb(102, 204, 204)),
                    ] {
                        if let Some(v) = ma[start + i]
                            .and_then(|d| d.to_f64())
                            .filter(|v| *v >= min && *v <= max)
                        {
                            buf.set_string(
                                x.saturating_add(body_w),
                                y(v),
                                glyph,
                                Style::default().fg(c),
                            );
                        }
                    }
                }
                OverlayMode::Bollinger => {
                    if let Some((lower, mid, upper)) = bb20[start + i] {
                        for (val, glyph, c) in [
                            (lower, "v", theme::INFO_BLUE),
                            (mid, ".", AMBER),
                            (upper, "^", theme::INFO_BLUE),
                        ] {
                            if let Some(v) = val.to_f64().filter(|v| *v >= min && *v <= max) {
                                buf.set_string(
                                    x.saturating_add(body_w),
                                    y(v),
                                    glyph,
                                    Style::default().fg(c),
                                );
                            }
                        }
                    }
                }
                OverlayMode::None => {}
            }
        }

        // Sub-indicator Panel (Volume, RSI, MACD, or ATR)
        let sub_border_y = sub_y.saturating_sub(1);
        for col in plot_left..plot_right + right_margin + y_axis_width {
            buf.set_string(col, sub_border_y, "─", Style::default().fg(BORDER));
        }

        match app.sub_indicator {
            SubIndicator::Volume => {
                let volume_max = bars
                    .iter()
                    .filter_map(|b| b.volume.to_f64())
                    .fold(0.0, f64::max);
                for (i, b) in bars.iter().enumerate() {
                    let x = bar_x_coords[i];
                    if x < plot_left || x >= plot_right {
                        continue;
                    }
                    let color = theme::direction_color(Some(b.close - b.open));
                    let v = b.volume.to_f64().unwrap_or(0.0);
                    let h = if volume_max > 0.0 {
                        ((v / volume_max) * sub_h as f64).ceil() as u16
                    } else {
                        0
                    };
                    for n in 0..h.min(sub_h) {
                        let row = sub_y + sub_h - 1 - n;
                        for bw in 0..body_w {
                            buf.set_string(
                                x + bw,
                                row,
                                if app.ascii { "|" } else { "▆" },
                                Style::default().fg(color),
                            );
                        }
                    }
                }
                buf.set_string(
                    inner.right().saturating_sub(13),
                    sub_y,
                    format!(
                        " VOL: {:>6}",
                        number(bars.iter().map(|b| b.volume).max().unwrap_or(Decimal::ZERO))
                    ),
                    Style::default().fg(SECONDARY),
                );
            }
            SubIndicator::Rsi => {
                let rsi_vals = rsi(&app.market.candles, 14);
                for (i, _) in bars.iter().enumerate() {
                    let x = bar_x_coords[i];
                    if x < plot_left || x >= plot_right {
                        continue;
                    }
                    if let Some(r_f) = rsi_vals
                        .get(start + i)
                        .and_then(|v| *v)
                        .and_then(|v| v.to_f64())
                    {
                        let rsi_y = sub_y + (((100.0 - r_f) / 100.0) * (sub_h - 1) as f64) as u16;
                        let c = if r_f >= 70.0 {
                            NEGATIVE
                        } else if r_f <= 30.0 {
                            POSITIVE
                        } else {
                            AMBER
                        };
                        buf.set_string(x, rsi_y, "•", Style::default().fg(c));
                    }
                }
                buf.set_string(
                    inner.right().saturating_sub(13),
                    sub_y,
                    " RSI70: OB",
                    Style::default().fg(NEGATIVE),
                );
                buf.set_string(
                    inner.right().saturating_sub(13),
                    sub_y + sub_h - 1,
                    " RSI30: OS",
                    Style::default().fg(POSITIVE),
                );
            }
            SubIndicator::Macd => {
                let macd_vals = macd(&app.market.candles, 12, 26, 9);
                for (i, _) in bars.iter().enumerate() {
                    let x = bar_x_coords[i];
                    if x < plot_left || x >= plot_right {
                        continue;
                    }
                    if let Some((_, _, h)) = macd_vals.get(start + i).and_then(|v| *v) {
                        let hist_f = h.to_f64().unwrap_or(0.0);
                        let c = if hist_f >= 0.0 { POSITIVE } else { NEGATIVE };
                        let row = if hist_f >= 0.0 {
                            sub_y
                        } else {
                            sub_y + sub_h.saturating_sub(1)
                        };
                        buf.set_string(
                            x,
                            row,
                            if app.ascii { "#" } else { "┃" },
                            Style::default().fg(c),
                        );
                    }
                }
                buf.set_string(
                    inner.right().saturating_sub(13),
                    sub_y,
                    " MACD(12,26,9)",
                    Style::default().fg(SECONDARY),
                );
            }
            SubIndicator::Atr => {
                let atr_vals = atr(&app.market.candles, 14);
                let visible_atrs: Vec<f64> = (0..bars.len())
                    .filter_map(|i| {
                        atr_vals
                            .get(start + i)
                            .and_then(|v| *v)
                            .and_then(|v| v.to_f64())
                    })
                    .collect();
                let atr_min = visible_atrs.iter().copied().fold(f64::INFINITY, f64::min);
                let atr_max = visible_atrs
                    .iter()
                    .copied()
                    .fold(f64::NEG_INFINITY, f64::max);
                let atr_range = (atr_max - atr_min).max(0.000001);
                for (i, _) in bars.iter().enumerate() {
                    let x = bar_x_coords[i];
                    if x < plot_left || x >= plot_right {
                        continue;
                    }
                    if let Some(a_f) = atr_vals
                        .get(start + i)
                        .and_then(|v| *v)
                        .and_then(|v| v.to_f64())
                    {
                        let atr_y =
                            sub_y + (((atr_max - a_f) / atr_range) * (sub_h - 1) as f64) as u16;
                        buf.set_string(x, atr_y, "•", Style::default().fg(GOLD));
                    }
                }
                buf.set_string(
                    inner.right().saturating_sub(13),
                    sub_y,
                    format!(
                        " ATR: {:>6}",
                        format_price(Decimal::from_f64(atr_max).unwrap_or(Decimal::ZERO))
                    ),
                    Style::default().fg(GOLD),
                );
            }
        }

        // Multi-tick Y-axis labels
        let decimals = if max >= 1000.0 {
            2
        } else if max >= 1.0 {
            4
        } else {
            6
        };
        let tick_count = (plot_h / 3).clamp(3, 8);
        for i in 0..tick_count {
            let row = plot_y + i * (plot_h - 1) / (tick_count - 1);
            let value = max - (max - min) * i as f64 / (tick_count - 1) as f64;
            buf.set_string(
                inner.right().saturating_sub(13),
                row,
                format!(" {:>11.*}", decimals, value),
                Style::default().fg(SECONDARY),
            );
        }

        // Highlight latest close price on Y-axis
        let last_close = bars[bars.len() - 1].close;
        if let Some(lc_f) = last_close.to_f64() {
            let lc_y = y(lc_f);
            buf.set_string(
                inner.right().saturating_sub(13),
                lc_y,
                format!("▶{:>11.*}", decimals, lc_f),
                Style::default().fg(AMBER).bg(SELECTION).bold(),
            );
        }

        // Multi-tick X-axis timestamp scale
        let axis_y = inner.bottom() - 2;
        let mut tick_x = plot_left + 1;
        while tick_x < plot_right {
            if let Some((idx, _)) = bar_x_coords
                .iter()
                .enumerate()
                .min_by_key(|(_, bx)| (**bx as i32 - tick_x as i32).abs())
            {
                let dt = bars[idx].open_at;
                let fmt_time = if app.timeframe == Timeframe::D1 {
                    dt.format("%Y-%m-%d").to_string()
                } else {
                    dt.format("%m-%d %H:%M").to_string()
                };
                buf.set_string(tick_x, axis_y, fmt_time, Style::default().fg(MUTED));
            }
            tick_x += 22;
        }

        // Bottom status & analytics footer
        let last = &bars[bars.len() - 1];
        let stats = window_stats(bars);
        let ret_str = stats
            .as_ref()
            .and_then(|s| s.return_pct)
            .map(|r| format!("Win Ret {r:+.2}% │ "))
            .unwrap_or_default();
        let vwap_str = stats
            .as_ref()
            .and_then(|s| s.vwap)
            .map(|v| format!("VWAP {} │ ", format_price(v)))
            .unwrap_or_default();
        let live_indicator = if app.follow_live {
            "FOLLOW-LIVE"
        } else {
            "PAN-MODE"
        };
        let forming_indicator = if last.closed {
            "Closed"
        } else {
            "Forming(▒)"
        };

        buf.set_string(
            inner.x,
            inner.bottom() - 1,
            format!(
                "{}Last {} ({}) │ {}Shown: {}/{} bars (Capacity {}) │ Cursor: {}/{} │ [{}] │ +/-:Zoom h/l:Pan f:FollowLive",
                vwap_str,
                format_price(last.close),
                forming_indicator,
                ret_str,
                bars.len(),
                app.market.candles.len(),
                capacity,
                cursor_idx + 1,
                bars.len(),
                live_indicator
            ),
            Style::default().fg(SECONDARY),
        );
    }
}

pub fn context_text(app: &App) -> Option<String> {
    match app.workspace {
        Workspace::Fin => app.company.as_ref().map(|c| {
            let table = facts::comparative_table(&c.facts, app.period, app.stmt_tab);
            if let Some(row) = table.rows.get(app.cursor.min(table.rows.len().saturating_sub(1))) {
                let col = app.fin_col.min(table.periods.len().saturating_sub(1));
                let period_label = table
                    .periods
                    .get(col)
                    .map(|d| d.to_string())
                    .unwrap_or_else(|| "—".into());
                if let Some(Some(fact)) = row.sources.get(col) {
                    format!(
                        "{}\nCIK {:010}\n\nStatement: {}\nMetric: {}\nSelected Period: {}\nValue: {} {}\nExact: {}\nLatest YoY: {}\n\nXBRL Provenance:\nConcept: us-gaap:{}\nForm: {}\nPeriod Start: {}\nPeriod End: {}\nFiling Date: {}\nAccession: {}\n\nArchive URL (Enter/'o' to open):\n{}\n\nFetched: {}",
                        c.name,
                        c.cik,
                        app.stmt_tab.name(),
                        row.metric,
                        period_label,
                        number(fact.value),
                        fact.unit,
                        fact.value,
                        row.yoy_pct
                            .map(|g| format!("{g:+.2}%"))
                            .unwrap_or_else(|| "—".into()),
                        fact.concept,
                        fact.form,
                        fact.start
                            .map(|d| d.to_string())
                            .unwrap_or_else(|| "Instant (Balance Sheet)".into()),
                        fact.end,
                        fact.filed,
                        fact.accession,
                        fact.source,
                        c.fetched_at.format("%Y-%m-%d %H:%M UTC")
                    )
                } else {
                    format!(
                        "{}\nCIK {:010}\n\nStatement: {}\nMetric: {}\nPeriod: {}\nValue: unavailable in direct XBRL USD facts\n\nSwitch period column with ←/→ or statement tab with 's'.",
                        c.name,
                        c.cik,
                        app.stmt_tab.name(),
                        row.metric,
                        period_label
                    )
                }
            } else {
                "No compatible reported facts.\nMissing values remain unavailable.".into()
            }
        }),
        Workspace::News => {
            if let Some(item) = selected_news_item(app) {
                let is_sec = if item.is_sec_filing {
                    format!(
                        "Form: {}\nAccession: {}\nType: Official SEC EDGAR Regulatory Disclosure",
                        item.form.as_deref().unwrap_or("EDGAR"),
                        item.accession.as_deref().unwrap_or("N/A")
                    )
                } else {
                    format!("Category: {}\nType: Public Financial News Wire", item.category.label())
                };
                Some(format!(
                    "{}\n\n{}\nSource: {}\nTicker: {}\nPublished: {} UTC\nFetched: {} UTC\n\nCanonical URL:\n{}\n\nSummary:\n{}\n\nPress Enter or 'r' to read full text in terminal reader.\nPress 'o' to open in browser.",
                    item.headline,
                    is_sec,
                    item.source,
                    item.ticker.as_deref().unwrap_or("GLOBAL"),
                    item.published_at.format("%Y-%m-%d %H:%M:%S"),
                    item.fetched_at.format("%Y-%m-%d %H:%M:%S"),
                    item.url,
                    item.summary
                ))
            } else {
                Some("No news stories match active category/filters.\nPress 'R' to refresh or 'c' to cycle category.".into())
            }
        }
        Workspace::Scan if app.scan_mode == ScanMode::Equities => {
            let matched = filtered_equities(app);
            matched
                .get(app.cursor.min(matched.len().saturating_sub(1)))
                .map(|c| {
                    let sum = facts::summarize(&c.facts, PeriodView::Annual);
                    format!(
                        "{} ({})\nCIK {:010}\n\nLatest FY End: {}\nGross Margin: {}\nOperating Margin: {}\nNet Margin: {}\nFree Cash Flow: {}\nROE: {}\nCurrent Ratio: {}\nDebt/Equity: {}\n\nFilings Cached: {}\nFacts Cached: {}\nLast Indexed: {}\n\nPress Enter to open full comparative financial statements in FIN.",
                        c.name,
                        c.ticker,
                        c.cik,
                        sum.as_ref()
                            .map(|s| s.latest_end.to_string())
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.gross_margin_pct)
                            .map(|v| format!("{v:.1}%"))
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.operating_margin_pct)
                            .map(|v| format!("{v:.1}%"))
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.net_margin_pct)
                            .map(|v| format!("{v:.1}%"))
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.free_cash_flow)
                            .map(number)
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.roe_pct)
                            .map(|v| format!("{v:.1}%"))
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.current_ratio)
                            .map(|v| format!("{v:.2}x"))
                            .unwrap_or_else(|| "—".into()),
                        sum.as_ref()
                            .and_then(|s| s.debt_to_equity_pct)
                            .map(|v| format!("{v:.1}%"))
                            .unwrap_or_else(|| "—".into()),
                        c.filings.len(),
                        c.facts.len(),
                        c.fetched_at.format("%Y-%m-%d %H:%M UTC")
                    )
                })
        }
        Workspace::Port => {
            let tx: Vec<_> = app
                .transactions
                .iter()
                .filter(|t| t.account == app.account)
                .cloned()
                .collect();
            let pos = positions(&tx).ok()?;
            let p = pos.get(app.cursor.min(pos.len().saturating_sub(1)))?;
            let q = app.market.quotes.get(&p.asset.key());
            let now = Utc::now();
            let valid = q.filter(|q| {
                matches!(
                    q.status_at(now),
                    DataStatus::Live | DataStatus::Delayed
                )
            });
            Some(format!(
                "Position Detail ({})\nAsset: {}\nVenue: {}\nQuote Currency: {}\n\nQuantity: {}\nAverage Cost: {}\nCost Basis: {} {}\nMarket Value: {}\nUnrealized P&L: {} ({})\nRealized P&L: {} {}\nTotal P&L: {}\n\nLive Quote: {}\nPress Enter to open position in CHART.",
                p.account,
                p.asset.key(),
                p.asset.venue,
                p.currency,
                p.quantity.normalize(),
                p.average_cost()
                    .map(format_price)
                    .unwrap_or_else(|| "—".into()),
                format_price(p.cost),
                p.currency,
                valid
                    .and_then(|q| p.market_value(q.price, &q.currency))
                    .map(|v| format!("{} {}", format_price(v), p.currency))
                    .unwrap_or_else(|| "unavailable".into()),
                valid
                    .and_then(|q| p.unrealized(q.price, &q.currency))
                    .map(format_signed_price)
                    .unwrap_or_else(|| "unavailable".into()),
                valid
                    .and_then(|q| p.unrealized_pct(q.price, &q.currency))
                    .map(|v| format!("{v:+.2}%"))
                    .unwrap_or_else(|| "—".into()),
                format_signed_price(p.realized),
                p.currency,
                valid
                    .and_then(|q| p.total_pnl(q.price, &q.currency))
                    .map(|v| format!("{} {}", format_signed_price(v), p.currency))
                    .unwrap_or_else(|| "unavailable".into()),
                q.map(|q| format!("{} {} ({:?})", format_price(q.price), q.currency, q.status_at(now)))
                    .unwrap_or_else(|| "No feed".into())
            ))
        }
        Workspace::Econ if !app.macro_data.is_empty() => {
            let first = &app.macro_data[0];
            let selected = &app.macro_data[app.cursor.min(app.macro_data.len() - 1)];
            if let Some(yc) = analyze_yield_curve(&app.macro_data) {
                Some(format!(
                    "US Treasury Yield Curve\nObs Date: {}\nRegime: {}\n\nSelected Tenor: {}\nPar Yield: {}%\nTenor Months: {}\n\nKey Benchmark Spreads:\n2s10s: {}\n3m10s: {}\n5s30s: {}\n\nCurve Extremes:\nMin Tenor: {}\nMax Tenor: {}\n\nSource: {}\nTenors Published: {}",
                    first.date,
                    yc.regime,
                    selected.label,
                    selected.value.round_dp(3).normalize(),
                    omt_core::treasury_tenor_months(&selected.series)
                        .map(|m| format!("{m:.1}m"))
                        .unwrap_or_else(|| "—".into()),
                    yc.spread_2s10s_bps
                        .map(|v| format!("{v:+.1} bps"))
                        .unwrap_or_else(|| "—".into()),
                    yc.spread_3m10s_bps
                        .map(|v| format!("{v:+.1} bps"))
                        .unwrap_or_else(|| "—".into()),
                    yc.spread_5s30s_bps
                        .map(|v| format!("{v:+.1} bps"))
                        .unwrap_or_else(|| "—".into()),
                    yc.min_yield
                        .map(|(l, v)| format!("{l} ({v}%)"))
                        .unwrap_or_else(|| "—".into()),
                    yc.max_yield
                        .map(|(l, v)| format!("{l} ({v}%)"))
                        .unwrap_or_else(|| "—".into()),
                    first.source,
                    app.macro_data.len()
                ))
            } else {
                let latest = &app.macro_data[0];
                let prior = app.macro_data.get(1);
                let pop = prior.and_then(|p| {
                    (p.value != Decimal::ZERO)
                        .then(|| (latest.value - p.value) / p.value.abs() * Decimal::from(100))
                });
                Some(format!(
                    "{}\n{}\n\nSelected ({}): {} {}\nLatest ({}): {} {}\nPrior: {}\nChange: {}\nFrequency: {}\n\nSource: {}\nObservations: {}",
                    latest.label,
                    latest.geography,
                    selected.date,
                    number(selected.value),
                    selected.unit,
                    latest.date,
                    number(latest.value),
                    latest.unit,
                    prior
                        .map(|p| format!("{} ({})", number(p.value), p.date))
                        .unwrap_or_else(|| "—".into()),
                    pop.map(|c| format!("{c:+.2}%"))
                        .unwrap_or_else(|| "—".into()),
                    latest.frequency,
                    latest.source,
                    app.macro_data.len()
                ))
            }
        }
        Workspace::Chart if !app.market.candles.is_empty() => {
            let end = app.market.candles.len().saturating_sub(app.offset).max(1);
            let slice = &app.market.candles[..end];
            let stats = window_stats(slice)?;
            let r14 = rsi(slice, 14).last().and_then(|v| *v);
            let m12_26 = macd(slice, 12, 26, 9).last().and_then(|v| *v);
            let a14 = atr(slice, 14).last().and_then(|v| *v);
            let q = app.market.quotes.get(&app.asset.key());
            let quote_hdr = if let Some(q) = q {
                format!(
                    "{} ({:?})\nLast: {} {}\nBid/Ask: {} / {}\nSource: {}\nEvent: {}",
                    q.asset.key(),
                    q.status_at(Utc::now()),
                    format_price(q.price),
                    q.currency,
                    q.bid.map(format_price).unwrap_or_else(|| "—".into()),
                    q.ask.map(format_price).unwrap_or_else(|| "—".into()),
                    q.provider,
                    q.event_at.format("%H:%M:%S UTC")
                )
            } else {
                format!("{}\nNo live quote", app.asset.key())
            };
            Some(format!(
                "{}\n\nWindow Analytics ({}):\nOpen: {}\nHigh: {}\nLow: {}\nClose: {}\nReturn: {}\nVWAP: {}\nMax DD: {:.2}%\nBar Vol SD: {}\n\nTechnical Indicators:\nOverlay: {}\nRSI(14): {}\nATR(14): {}\nMACD(12,26,9): {}",
                quote_hdr,
                app.timeframe,
                format_price(stats.open),
                format_price(stats.high),
                format_price(stats.low),
                format_price(stats.close),
                stats
                    .return_pct
                    .map(|r| format!("{r:+.2}%"))
                    .unwrap_or_else(|| "—".into()),
                stats.vwap.map(format_price).unwrap_or_else(|| "—".into()),
                stats.max_drawdown_pct,
                stats
                    .volatility_pct
                    .map(|v| format!("{v:.2}%"))
                    .unwrap_or_else(|| "—".into()),
                app.overlay.label(),
                r14.map(|v| format!("{v:.1}")).unwrap_or_else(|| "—".into()),
                a14.map(format_price).unwrap_or_else(|| "—".into()),
                m12_26
                    .map(|(m, s, h)| format!("{:.2} / sig {:.2} / hist {:+.2}", m, s, h))
                    .unwrap_or_else(|| "—".into())
            ))
        }
        _ => None,
    }
}
