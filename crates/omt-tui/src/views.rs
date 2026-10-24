use super::*;
use crate::theme::CYAN;

pub fn alerts(frame: &mut Frame, app: &App, area: Rect) {
    let popup = area.inner(ratatui::layout::Margin::new(2, 2));
    frame.render_widget(Clear, popup);
    let mut lines = vec![
        Line::styled(
            "PRICE ALERTS / Esc close / j,k scroll",
            Style::default().fg(CYAN),
        ),
        Line::from("ALERT ADD AAPL ABOVE 250 | ALERT REMOVE 1"),
        Line::from("One-shot conditions, inclusive thresholds. Runs only while this app is open."),
        Line::from("Live quotes <=60s old only; cached or disconnected data cannot trigger."),
        Line::from("Saved every 15s and on clean exit. IEX prices are not consolidated."),
        Line::from(""),
    ];
    for alert in app.alerts.iter().skip(app.cursor) {
        lines.push(Line::from(format!(
            "#{} {} {} {} | {}",
            alert.id,
            alert.asset.key(),
            if alert.above { "ABOVE" } else { "BELOW" },
            alert.threshold,
            alert
                .triggered_at
                .map(|t| format!(
                    "TRIGGERED {} at {}",
                    t.format("%m-%d %H:%M UTC"),
                    alert.triggered_price.unwrap_or_default()
                ))
                .unwrap_or_else(|| "ARMED / waiting for fresh matching quote".into())
        )));
    }
    if app.alerts.is_empty() {
        lines.push(Line::from(
            "No alerts. Add a price condition with the command bar.",
        ));
    }
    frame.render_widget(
        Paragraph::new(lines)
            .block(Block::default().borders(Borders::ALL))
            .style(Style::default().bg(ELEVATED_BG).fg(FG)),
        popup,
    );
}
use omt_core::{
    ChartDataStatus, DataStatus, Filing, InstrumentInfo, Quote, TradingStatus, WorksheetColumn,
    analyze_yield_curve, atr, bollinger_bands, ema, facts, macd, portfolio::positions, rsi, sma,
    window_stats,
};
use rust_decimal::{
    Decimal,
    prelude::{FromPrimitive, ToPrimitive},
};

pub fn impact(frame: &mut Frame, app: &App, area: Rect) {
    use omt_core::impact::{aligned, event_move, relationship};
    let block = Block::default()
        .borders(Borders::ALL)
        .title(" IMPACT / Event & Benchmark Analysis [EVT] │ [p] Plot Mode: 1:RelPerf 2:EventReaction 3:CAR 4:DailySpread ")
        .border_style(Style::default().fg(AMBER));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let Some(data) = &app.impact else {
        frame.render_widget(
            Paragraph::new(format!(
                "{}\n\nQuick Commands:\n  :IMPACT AAPL SPY                       (Equities: Asset vs S&P 500 ETF)\n  :IMPACT BINANCE:ETHUSDT BINANCE:BTCUSDT (Crypto: Pair benchmark)\n  :COMPARE AAPL QQQ                      (Alias for IMPACT)\n  :EVENT YYYY-MM-DD                      (Pin specific event study date)\n  :EVENT CLEAR                           (Follow selected headline date)\n\nKeys:\n  j / k   Select headline in blotter\n  e       Pin/follow selected headline date\n  o       Open headline in web browser\n  R       Refresh daily benchmark prices\n\nMethodology & Requirements:\n  • Daily completed UTC closes; non-synchronous cross-venue closes are flagged.\n  • Association, not causation. Pre-headline daily moves are included; no forecasts.",
                app.impact_status
            ))
            .style(Style::default().fg(FG))
            .wrap(Wrap { trim: true }),
            inner,
        );
        return;
    };

    let rows = aligned(data);
    let stats = relationship(&rows);

    let number = |n: Option<f64>| {
        n.map(|v| format!("{v:+.2}"))
            .unwrap_or_else(|| "N/A".into())
    };

    let range = rows
        .first()
        .zip(rows.last())
        .map(|(a, b)| format!("{} to {}", a.date, b.date))
        .unwrap_or_else(|| "No shared completed dates".into());

    let selected = app.impact_origin.as_ref().or_else(|| {
        data.news
            .get(app.cursor.min(data.news.len().saturating_sub(1)))
    });
    let date = app
        .impact_event
        .or_else(|| selected.map(|n| n.published_at.date_naive()));

    // Fallback for very small terminal areas
    if inner.height < 18 || inner.width < 50 {
        let mut lines = vec![
            Line::styled(
                format!(
                    "IMPACT: {} vs {} | DAILY",
                    data.asset.key(),
                    data.benchmark.key()
                ),
                Style::default().fg(CYAN).bold(),
            ),
            Line::from(format!(
                "Correlation {}   Beta {}   Total return {}% / {}%",
                number(stats.correlation),
                number(stats.beta),
                number(stats.asset_return),
                number(stats.benchmark_return)
            )),
            Line::from(format!(
                "Excess (pp): {}",
                date.and_then(|d| event_move(&rows, d, 1))
                    .map(|m| format!("{:+.2}", m.excess_pp))
                    .unwrap_or_else(|| "N/A".into())
            )),
        ];
        if let Some(news) = selected {
            lines.push(Line::styled(
                news.headline.clone(),
                Style::default().fg(AMBER),
            ));
        }
        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), inner);
        return;
    }

    // Dynamic vertical layout based on available viewport height
    let (top_h, table_h, chart_h) = if inner.height >= 34 {
        (6u16, 5u16, 13u16)
    } else if inner.height >= 24 {
        (5u16, 5u16, 8u16)
    } else {
        (5u16, 5u16, 0u16)
    };

    let v_chunks = if chart_h > 0 {
        Layout::vertical([
            Constraint::Length(top_h),
            Constraint::Length(table_h),
            Constraint::Length(chart_h),
            Constraint::Min(6),
        ])
        .split(inner)
    } else {
        Layout::vertical([
            Constraint::Length(top_h),
            Constraint::Length(table_h),
            Constraint::Min(6),
        ])
        .split(inner)
    };

    // 1. Executive Summary KPI Cards (Horizontal 3-column split)
    let card_cols = Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Percentage(37),
        Constraint::Percentage(30),
    ])
    .split(v_chunks[0]);

    // Card 0: Pair & Scope
    let card0_lines = vec![
        Line::from(vec![
            Span::styled(data.asset.key(), Style::default().fg(CYAN).bold()),
            Span::raw(" vs "),
            Span::styled(data.benchmark.key(), Style::default().fg(AMBER).bold()),
            Span::styled(" [DAILY]", Style::default().fg(MUTED)),
        ]),
        Line::from(vec![
            Span::styled(range.clone(), Style::default().fg(FG)),
            Span::styled(
                format!(" ({} intervals)", stats.samples),
                Style::default().fg(SECONDARY),
            ),
        ]),
        Line::from(vec![Span::styled(
            format!(
                "Saved: {} UTC │ {}",
                data.fetched_at.format("%Y-%m-%d %H:%M"),
                app.impact_status
            ),
            Style::default().fg(MUTED),
        )]),
    ];
    frame.render_widget(
        Paragraph::new(card0_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Pair & Scope ")
                    .border_style(Style::default().fg(BORDER)),
            )
            .style(Style::default().bg(PANEL_BG)),
        card_cols[0],
    );

    // Card 1: Statistical Relationship
    let corr_color = match stats.correlation {
        Some(c) if c > 0.0 => POSITIVE,
        Some(c) if c < 0.0 => NEGATIVE,
        _ => FG,
    };
    let card1_lines = vec![
        Line::from(vec![
            Span::raw("Correlation "),
            Span::styled(
                number(stats.correlation),
                Style::default().fg(corr_color).bold(),
            ),
            Span::raw("   Beta "),
            Span::styled(number(stats.beta), Style::default().fg(AMBER).bold()),
        ]),
        Line::from(vec![Span::styled(
            format!(
                "Total return {}% / {}%",
                number(stats.asset_return),
                number(stats.benchmark_return)
            ),
            Style::default().fg(FG),
        )]),
        Line::from(vec![Span::styled(
            if stats.samples < 20 {
                "Need 20 matched return intervals for correlation/beta.".to_string()
            } else {
                format!(
                    "Excess total return: {:+.2} pp",
                    stats.asset_return.unwrap_or(0.0) - stats.benchmark_return.unwrap_or(0.0)
                )
            },
            Style::default().fg(if stats.samples < 20 { AMBER } else { SECONDARY }),
        )]),
    ];
    frame.render_widget(
        Paragraph::new(card1_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Statistical Relationship ")
                    .border_style(Style::default().fg(BORDER)),
            )
            .style(Style::default().bg(PANEL_BG)),
        card_cols[1],
    );

    // Card 2: Active Event Study Anchor
    let mut card2_lines = vec![Line::from(vec![
        Span::styled(
            format!(
                "EVENT: {}",
                date.map(|d| d.to_string()).unwrap_or_else(|| "NONE".into())
            ),
            Style::default().fg(CYAN).bold(),
        ),
        Span::styled(
            if app.impact_event.is_some() {
                " [PINNED DATE]"
            } else {
                " [FOLLOWING HEADLINE]"
            },
            Style::default().fg(AMBER),
        ),
    ])];
    if let Some(news) = selected {
        card2_lines.push(Line::styled(news.headline.clone(), Style::default().fg(FG)));
        card2_lines.push(Line::styled(
            format!(
                "{} │ {}",
                news.source,
                news.published_at.format("%Y-%m-%d %H:%M UTC")
            ),
            Style::default().fg(SECONDARY),
        ));
    } else {
        card2_lines.push(Line::styled(
            "No event selected. Select a headline below or type EVENT YYYY-MM-DD",
            Style::default().fg(MUTED),
        ));
    }
    frame.render_widget(
        Paragraph::new(card2_lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Event Anchor ")
                    .border_style(Style::default().fg(BORDER)),
            )
            .style(Style::default().bg(PANEL_BG)),
        card_cols[2],
    );

    // 2. Event Window Impact Table
    let table_header = Row::new(vec![
        Cell::from("Window").style(Style::default().fg(CYAN).bold()),
        Cell::from("Subject Move").style(Style::default().fg(CYAN).bold()),
        Cell::from("Benchmark Move").style(Style::default().fg(AMBER).bold()),
        Cell::from("Excess (pp)").style(Style::default().fg(FG).bold()),
        Cell::from("Baseline -> End").style(Style::default().fg(SECONDARY).bold()),
    ])
    .height(1);

    let mut event_rows = Vec::new();
    if let Some(d) = date {
        for sessions in [1, 3, 5] {
            match event_move(&rows, d, sessions) {
                Some(m) => {
                    let a_color = if m.asset_pct >= 0.0 {
                        POSITIVE
                    } else {
                        NEGATIVE
                    };
                    let b_color = if m.benchmark_pct >= 0.0 {
                        POSITIVE
                    } else {
                        NEGATIVE
                    };
                    let x_color = if m.excess_pp >= 0.0 {
                        POSITIVE
                    } else {
                        NEGATIVE
                    };
                    event_rows.push(Row::new(vec![
                        Cell::from(format!("{sessions} bars")).style(Style::default().fg(FG)),
                        Cell::from(format!("{:+8.2}%", m.asset_pct))
                            .style(Style::default().fg(a_color)),
                        Cell::from(format!("{:+8.2}%", m.benchmark_pct))
                            .style(Style::default().fg(b_color)),
                        Cell::from(format!("{:+8.2}", m.excess_pp))
                            .style(Style::default().fg(x_color).bold()),
                        Cell::from(format!("{} -> {}", m.baseline, m.end))
                            .style(Style::default().fg(SECONDARY)),
                    ]));
                }
                None => {
                    event_rows.push(Row::new(vec![
                        Cell::from(format!("{sessions} bars")).style(Style::default().fg(FG)),
                        Cell::from("N/A - incomplete window or outside available history")
                            .style(Style::default().fg(MUTED)),
                        Cell::from(""),
                        Cell::from(""),
                        Cell::from(""),
                    ]));
                }
            }
        }
    } else {
        event_rows.push(Row::new(vec![
            Cell::from("1 bars"),
            Cell::from("No event date specified. Select headline or enter EVENT YYYY-MM-DD")
                .style(Style::default().fg(MUTED)),
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
        ]));
    }

    let event_table = Table::new(
        event_rows,
        [
            Constraint::Length(14),
            Constraint::Length(18),
            Constraint::Length(18),
            Constraint::Length(16),
            Constraint::Min(24),
        ],
    )
    .header(table_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Event Window Abnormal / Excess Return Analysis ")
            .border_style(Style::default().fg(BORDER)),
    )
    .style(Style::default().bg(PANEL_BG));
    frame.render_widget(event_table, v_chunks[1]);

    // 3. Meaningful Financial Visualizations (Comparative Plots on Demand)
    if chart_h > 0 && rows.len() >= 2 {
        let first = &rows[0];
        let a: Vec<_> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i as f64, r.asset / first.asset * 100.0))
            .collect();
        let b: Vec<_> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (i as f64, r.benchmark / first.benchmark * 100.0))
            .collect();
        let low_rel = a
            .iter()
            .chain(&b)
            .map(|p| p.1)
            .fold(f64::INFINITY, f64::min);
        let high_rel = a
            .iter()
            .chain(&b)
            .map(|p| p.1)
            .fold(f64::NEG_INFINITY, f64::max);
        let pad_rel = ((high_rel - low_rel) * 0.05).max(1.0);
        let rel_chart = Chart::new(vec![
            Dataset::default()
                .name(data.asset.key())
                .data(&a)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(CYAN)),
            Dataset::default()
                .name(data.benchmark.key())
                .data(&b)
                .graph_type(GraphType::Line)
                .style(Style::default().fg(AMBER)),
        ])
        .hidden_legend_constraints((Constraint::Percentage(100), Constraint::Percentage(100)))
        .x_axis(
            Axis::default()
                .bounds([0.0, (rows.len() - 1) as f64])
                .labels([
                    first.date.to_string(),
                    rows.last().unwrap().date.to_string(),
                ]),
        )
        .y_axis(
            Axis::default()
                .bounds([low_rel - pad_rel, high_rel + pad_rel])
                .labels([format!("{low_rel:.0}"), format!("{high_rel:.0}")]),
        )
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Relative performance / first common close = 100 ")
                .border_style(Style::default().fg(BORDER)),
        );

        // Compute Event Reaction Window Trajectory & CAR if event date is available
        let event_opt = date.and_then(|d| {
            let e_idx = rows.iter().position(|r| r.date >= d)?;
            let b_idx = e_idx.checked_sub(1)?;
            let start_idx = b_idx.saturating_sub(2);
            let end_idx = (e_idx + 4).min(rows.len().saturating_sub(1));
            let base_asset = rows[b_idx].asset;
            let base_bench = rows[b_idx].benchmark;
            if base_asset <= 0.0 || base_bench <= 0.0 {
                return None;
            }
            let mut react_asset = Vec::new();
            let mut react_bench = Vec::new();
            let mut car_pts = Vec::new();

            for (idx, r) in rows.iter().enumerate().take(end_idx + 1).skip(start_idx) {
                let x = (idx as i64 - b_idx as i64) as f64;
                let y_a = r.asset / base_asset * 100.0;
                let y_b = r.benchmark / base_bench * 100.0;
                react_asset.push((x, y_a));
                react_bench.push((x, y_b));
                if idx >= b_idx {
                    let car = (y_a - 100.0) - (y_b - 100.0);
                    car_pts.push((x, car));
                }
            }
            Some((
                d,
                b_idx,
                start_idx,
                end_idx,
                react_asset,
                react_bench,
                car_pts,
            ))
        });

        // Compute Daily Return Spread across series
        let mut spread_pts = Vec::new();
        let mut spread_zero = Vec::new();
        for i in 1..rows.len() {
            let x = i as f64;
            let r_a = (rows[i].asset / rows[i - 1].asset - 1.0) * 100.0;
            let r_b = (rows[i].benchmark / rows[i - 1].benchmark - 1.0) * 100.0;
            spread_pts.push((x, r_a - r_b));
            spread_zero.push((x, 0.0));
        }
        let spread_low = spread_pts.iter().map(|p| p.1).fold(0.0f64, f64::min);
        let spread_high = spread_pts.iter().map(|p| p.1).fold(0.0f64, f64::max);
        let spread_pad = ((spread_high - spread_low) * 0.1).max(0.5);

        // Render side-by-side dual plots on wide screens (>= 115) or single focused plot
        let is_wide = inner.width >= 115;
        if is_wide {
            let chart_cols =
                Layout::horizontal([Constraint::Percentage(54), Constraint::Percentage(46)])
                    .split(v_chunks[2]);

            // Left Plot: Full Relative Performance (Macro Perspective)
            frame.render_widget(rel_chart, chart_cols[0]);

            // Right Plot: Event Reaction / CAR / Daily Spread (Micro & Diagnostic Perspective)
            if app.impact_plot_mode == 2
                && let Some((d, _, _, _, _, _, ref car_pts)) = event_opt
            {
                let car_low = car_pts.iter().map(|p| p.1).fold(0.0f64, f64::min);
                let car_high = car_pts.iter().map(|p| p.1).fold(0.0f64, f64::max);
                let car_pad = ((car_high - car_low) * 0.1).max(0.5);
                let car_zero: Vec<_> = car_pts.iter().map(|p| (p.0, 0.0)).collect();
                let car_color = if car_pts.last().map(|p| p.1 >= 0.0).unwrap_or(true) {
                    POSITIVE
                } else {
                    NEGATIVE
                };
                let car_chart = Chart::new(vec![
                    Dataset::default()
                        .name("Zero Base")
                        .data(&car_zero)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(MUTED)),
                    Dataset::default()
                        .name("Excess (pp)")
                        .data(car_pts)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(car_color).bold()),
                ])
                .hidden_legend_constraints((
                    Constraint::Percentage(100),
                    Constraint::Percentage(100),
                ))
                .x_axis(
                    Axis::default()
                        .bounds([0.0, car_pts.last().map(|p| p.0).unwrap_or(5.0)])
                        .labels([
                            "t-1".into(),
                            "t0 Event".into(),
                            format!("t+{}", car_pts.len().saturating_sub(1)),
                        ]),
                )
                .y_axis(
                    Axis::default()
                        .bounds([car_low - car_pad, car_high + car_pad])
                        .labels([
                            format!("{car_low:+.1}"),
                            "0.0".into(),
                            format!("{car_high:+.1}"),
                        ]),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(
                            " Cumulative Abnormal Return / CAR (pp) around {d} "
                        ))
                        .border_style(Style::default().fg(car_color)),
                );
                frame.render_widget(car_chart, chart_cols[1]);
            } else if app.impact_plot_mode == 3 || event_opt.is_none() {
                let spread_chart = Chart::new(vec![
                    Dataset::default()
                        .name("Zero")
                        .data(&spread_zero)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(MUTED)),
                    Dataset::default()
                        .name("Spread %")
                        .data(&spread_pts)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(CYAN)),
                ])
                .hidden_legend_constraints((
                    Constraint::Percentage(100),
                    Constraint::Percentage(100),
                ))
                .x_axis(Axis::default().bounds([0.0, rows.len() as f64]).labels([
                    rows.first().unwrap().date.to_string(),
                    rows.last().unwrap().date.to_string(),
                ]))
                .y_axis(
                    Axis::default()
                        .bounds([spread_low - spread_pad, spread_high + spread_pad])
                        .labels([format!("{spread_low:+.1}%"), format!("{spread_high:+.1}%")]),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(" Daily Return Spread / Asset % - Benchmark % ")
                        .border_style(Style::default().fg(BORDER)),
                );
                frame.render_widget(spread_chart, chart_cols[1]);
            } else if let Some((d, b_idx, start_idx, end_idx, ref react_a, ref react_b, _)) =
                event_opt
            {
                let min_x = (start_idx as i64 - b_idx as i64) as f64;
                let max_x = (end_idx as i64 - b_idx as i64) as f64;
                let ref_100 = vec![(min_x, 100.0), (max_x, 100.0)];
                let low_react = react_a
                    .iter()
                    .chain(react_b)
                    .map(|p| p.1)
                    .fold(100.0f64, f64::min);
                let high_react = react_a
                    .iter()
                    .chain(react_b)
                    .map(|p| p.1)
                    .fold(100.0f64, f64::max);
                let pad_react = ((high_react - low_react) * 0.1).max(0.5);
                let react_chart = Chart::new(vec![
                    Dataset::default()
                        .name("t-1 Base 100")
                        .data(&ref_100)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(MUTED)),
                    Dataset::default()
                        .name(data.asset.key())
                        .data(react_a)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(CYAN).bold()),
                    Dataset::default()
                        .name(data.benchmark.key())
                        .data(react_b)
                        .graph_type(GraphType::Line)
                        .style(Style::default().fg(AMBER)),
                ])
                .hidden_legend_constraints((
                    Constraint::Percentage(100),
                    Constraint::Percentage(100),
                ))
                .x_axis(Axis::default().bounds([min_x, max_x]).labels([
                    format!("t-{}", b_idx - start_idx),
                    "t-1 (Base)".into(),
                    format!("t+{}", end_idx - b_idx),
                ]))
                .y_axis(
                    Axis::default()
                        .bounds([low_react - pad_react, high_react + pad_react])
                        .labels([
                            format!("{:.1}", low_react - pad_react),
                            "100.0".into(),
                            format!("{:.1}", high_react + pad_react),
                        ]),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(
                            " Event Reaction Window ({d}) / t-1 baseline = 100 "
                        ))
                        .border_style(Style::default().fg(CYAN)),
                );
                frame.render_widget(react_chart, chart_cols[1]);
            }
        } else {
            // Single focused plot on narrower screens or when toggled
            match app.impact_plot_mode {
                1 if event_opt.is_some() => {
                    let (d, b_idx, start_idx, end_idx, ref react_a, ref react_b, _) =
                        event_opt.unwrap();
                    let min_x = (start_idx as i64 - b_idx as i64) as f64;
                    let max_x = (end_idx as i64 - b_idx as i64) as f64;
                    let ref_100 = vec![(min_x, 100.0), (max_x, 100.0)];
                    let low_react = react_a
                        .iter()
                        .chain(react_b)
                        .map(|p| p.1)
                        .fold(100.0f64, f64::min);
                    let high_react = react_a
                        .iter()
                        .chain(react_b)
                        .map(|p| p.1)
                        .fold(100.0f64, f64::max);
                    let pad_react = ((high_react - low_react) * 0.1).max(0.5);
                    let react_chart = Chart::new(vec![
                        Dataset::default()
                            .name("t-1 Base 100")
                            .data(&ref_100)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(MUTED)),
                        Dataset::default()
                            .name(data.asset.key())
                            .data(react_a)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(CYAN).bold()),
                        Dataset::default()
                            .name(data.benchmark.key())
                            .data(react_b)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(AMBER)),
                    ])
                    .hidden_legend_constraints((
                        Constraint::Percentage(100),
                        Constraint::Percentage(100),
                    ))
                    .x_axis(Axis::default().bounds([min_x, max_x]).labels([
                        format!("t-{}", b_idx - start_idx),
                        "t-1 (Base)".into(),
                        format!("t+{}", end_idx - b_idx),
                    ]))
                    .y_axis(
                        Axis::default()
                            .bounds([low_react - pad_react, high_react + pad_react])
                            .labels([
                                format!("{:.1}", low_react - pad_react),
                                "100.0".into(),
                                format!("{:.1}", high_react + pad_react),
                            ]),
                    )
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(
                                " Event Reaction Window ({d}) / t-1 baseline = 100 "
                            ))
                            .border_style(Style::default().fg(CYAN)),
                    );
                    frame.render_widget(react_chart, v_chunks[2]);
                }
                2 if event_opt.is_some() => {
                    let (d, _, _, _, _, _, ref car_pts) = event_opt.unwrap();
                    let car_low = car_pts.iter().map(|p| p.1).fold(0.0f64, f64::min);
                    let car_high = car_pts.iter().map(|p| p.1).fold(0.0f64, f64::max);
                    let car_pad = ((car_high - car_low) * 0.1).max(0.5);
                    let car_zero: Vec<_> = car_pts.iter().map(|p| (p.0, 0.0)).collect();
                    let car_color = if car_pts.last().map(|p| p.1 >= 0.0).unwrap_or(true) {
                        POSITIVE
                    } else {
                        NEGATIVE
                    };
                    let car_chart = Chart::new(vec![
                        Dataset::default()
                            .name("Zero Base")
                            .data(&car_zero)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(MUTED)),
                        Dataset::default()
                            .name("Excess (pp)")
                            .data(car_pts)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(car_color).bold()),
                    ])
                    .hidden_legend_constraints((
                        Constraint::Percentage(100),
                        Constraint::Percentage(100),
                    ))
                    .x_axis(
                        Axis::default()
                            .bounds([0.0, car_pts.last().map(|p| p.0).unwrap_or(5.0)])
                            .labels([
                                "t-1".into(),
                                "t0 Event".into(),
                                format!("t+{}", car_pts.len().saturating_sub(1)),
                            ]),
                    )
                    .y_axis(
                        Axis::default()
                            .bounds([car_low - car_pad, car_high + car_pad])
                            .labels([
                                format!("{car_low:+.1}"),
                                "0.0".into(),
                                format!("{car_high:+.1}"),
                            ]),
                    )
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(format!(
                                " Cumulative Abnormal Return / CAR (pp) around {d} "
                            ))
                            .border_style(Style::default().fg(car_color)),
                    );
                    frame.render_widget(car_chart, v_chunks[2]);
                }
                3 => {
                    let spread_chart = Chart::new(vec![
                        Dataset::default()
                            .name("Zero")
                            .data(&spread_zero)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(MUTED)),
                        Dataset::default()
                            .name("Spread %")
                            .data(&spread_pts)
                            .graph_type(GraphType::Line)
                            .style(Style::default().fg(CYAN)),
                    ])
                    .hidden_legend_constraints((
                        Constraint::Percentage(100),
                        Constraint::Percentage(100),
                    ))
                    .x_axis(Axis::default().bounds([0.0, rows.len() as f64]).labels([
                        rows.first().unwrap().date.to_string(),
                        rows.last().unwrap().date.to_string(),
                    ]))
                    .y_axis(
                        Axis::default()
                            .bounds([spread_low - spread_pad, spread_high + spread_pad])
                            .labels([format!("{spread_low:+.1}%"), format!("{spread_high:+.1}%")]),
                    )
                    .block(
                        Block::default()
                            .borders(Borders::ALL)
                            .title(" Daily Return Spread / Asset % - Benchmark % ")
                            .border_style(Style::default().fg(BORDER)),
                    );
                    frame.render_widget(spread_chart, v_chunks[2]);
                }
                _ => {
                    frame.render_widget(rel_chart, v_chunks[2]);
                }
            }
        }
    }

    // 4. Event Headlines Blotter
    let blotter_chunk = if chart_h > 0 {
        v_chunks[3]
    } else {
        v_chunks[2]
    };
    let blotter_header = Row::new(vec![
        Cell::from(" ").style(Style::default().fg(MUTED)),
        Cell::from("Date").style(Style::default().fg(SECONDARY).bold()),
        Cell::from("Source").style(Style::default().fg(AMBER).bold()),
        Cell::from("Headline (Press 'j'/'k' to select, 'e' to pin date, 'o' to open in browser)")
            .style(Style::default().fg(FG).bold()),
    ])
    .height(1);

    let blotter_rows_count = blotter_chunk.height.saturating_sub(3) as usize;
    let cursor = app.cursor.min(data.news.len().saturating_sub(1));
    let start = cursor.saturating_sub(blotter_rows_count.saturating_sub(1) / 2);

    let mut headline_rows = Vec::new();
    for (index, news) in data
        .news
        .iter()
        .enumerate()
        .skip(start)
        .take(blotter_rows_count)
    {
        let is_selected = index == cursor;
        let marker = if is_selected { ">" } else { " " };
        let headline_style = if is_selected {
            Style::default().fg(CYAN).bold()
        } else {
            Style::default().fg(FG)
        };
        headline_rows.push(
            Row::new(vec![
                Cell::from(marker).style(if is_selected {
                    Style::default().fg(CYAN).bold()
                } else {
                    Style::default().fg(MUTED)
                }),
                Cell::from(news.published_at.format("%m-%d").to_string())
                    .style(Style::default().fg(SECONDARY)),
                Cell::from(news.source.clone()).style(Style::default().fg(AMBER)),
                Cell::from(news.headline.clone()).style(headline_style),
            ])
            .style(if is_selected {
                theme::row_highlight_style()
            } else {
                Style::default()
            }),
        );
    }
    if headline_rows.is_empty() {
        headline_rows.push(Row::new(vec![
            Cell::from(""),
            Cell::from(""),
            Cell::from(""),
            Cell::from(
                "No headlines loaded for this pair. Use EVENT YYYY-MM-DD to study known dates.",
            )
            .style(Style::default().fg(MUTED)),
        ]));
    }

    let blotter_table = Table::new(
        headline_rows,
        [
            Constraint::Length(2),
            Constraint::Length(7),
            Constraint::Length(14),
            Constraint::Min(30),
        ],
    )
    .header(blotter_header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(" HEADLINES / feed context, not verified exposure / j,k select / o source ")
            .border_style(Style::default().fg(BORDER)),
    )
    .style(Style::default().bg(PANEL_BG));
    frame.render_widget(blotter_table, blotter_chunk);
}

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
            MarketMode::Worksheet => app.worksheet.symbols.len(),
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
        Workspace::Impact => app.impact.as_ref().map(|d| d.news.len()).unwrap_or(0),
        Workspace::Chart => app.market.candles.len(),
        Workspace::Des => 1,
    }
}

pub fn selected_market_instrument(app: &App) -> Option<String> {
    if app.market_mode == MarketMode::Worksheet {
        return app
            .worksheet
            .symbols
            .get(
                app.cursor
                    .min(app.worksheet.symbols.len().saturating_sub(1)),
            )
            .map(|a| a.key());
    }
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
        Workspace::Impact => app
            .impact_origin
            .as_ref()
            .or_else(|| {
                app.impact.as_ref()?.news.get(
                    app.cursor
                        .min(app.impact.as_ref()?.news.len().saturating_sub(1)),
                )
            })
            .map(|n| n.url.clone()),
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
        Workspace::Impact => impact(frame, app, area),
        Workspace::Des => security_description(frame, app, area),
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

pub fn render_worksheet(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area);

    let symbols = &app.worksheet.symbols;
    let mut advancers = 0usize;
    let mut decliners = 0usize;
    let mut unchanged = 0usize;
    for asset in symbols {
        if let Some(q) = app.market.quotes.get(&asset.key()) {
            match q.effective_change_pct() {
                Some(p) if p > Decimal::ZERO => advancers += 1,
                Some(p) if p < Decimal::ZERO => decliners += 1,
                _ => unchanged += 1,
            }
        }
    }

    let cols_summary = app
        .worksheet
        .columns
        .iter()
        .map(|c| c.label())
        .collect::<Vec<_>>()
        .join(" ");

    let header_lines = vec![
        Line::from(vec![
            Span::styled(" WORKSHEET ", Style::default().fg(BG).bg(AMBER).bold()),
            Span::styled(
                format!(
                    "  [{}]  │  {} securities tracked",
                    app.worksheet.name,
                    symbols.len()
                ),
                Style::default().fg(FG).bold(),
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
            Span::styled(
                format!("  │  Cols: {cols_summary}"),
                Style::default().fg(MUTED),
            ),
        ]),
        Line::from(vec![
            Span::styled(" Drilldown: ", Style::default().fg(SECONDARY)),
            Span::styled(
                "[Enter/g] GP  [d] DES  [f] FA  [n] CN  [x] Del  │  ",
                Style::default().fg(AMBER).bold(),
            ),
            Span::styled("Commands: ", Style::default().fg(SECONDARY)),
            Span::styled(
                "W ADD <SYM>  │  W DEL <SYM>  │  W COL <COLS>  │  W CREATE <NAME>  │  W LIST",
                Style::default().fg(FG),
            ),
        ]),
    ];

    frame.render_widget(
        Paragraph::new(header_lines).style(Style::default().bg(ELEVATED_BG)),
        chunks[0],
    );

    if symbols.is_empty() {
        let empty_msg = vec![
            Line::from(""),
            Line::styled(
                format!(
                    "  No securities currently in worksheet '{}'",
                    app.worksheet.name
                ),
                Style::default().fg(AMBER).bold(),
            ),
            Line::from(""),
            Line::styled("  Quick Actions:", Style::default().fg(FG).bold()),
            Line::from(vec![
                Span::styled("  • Add security:     ", Style::default().fg(SECONDARY)),
                Span::styled("W ADD AAPL", Style::default().fg(AMBER)),
                Span::styled("  or  ", Style::default().fg(MUTED)),
                Span::styled("W ADD BTCUSDT", Style::default().fg(AMBER)),
                Span::styled("  (or type ticker first: ", Style::default().fg(MUTED)),
                Span::styled("AAPL W ADD", Style::default().fg(AMBER)),
                Span::styled(")", Style::default().fg(MUTED)),
            ]),
            Line::from(vec![
                Span::styled("  • Switch worksheet: ", Style::default().fg(SECONDARY)),
                Span::styled("W DEFAULT", Style::default().fg(FG)),
                Span::styled("  or  ", Style::default().fg(MUTED)),
                Span::styled("W TECH", Style::default().fg(FG)),
            ]),
            Line::from(vec![
                Span::styled("  • Create new list:  ", Style::default().fg(SECONDARY)),
                Span::styled("W CREATE TECH_MAJORS", Style::default().fg(FG)),
            ]),
            Line::from(vec![
                Span::styled("  • List worksheets:  ", Style::default().fg(SECONDARY)),
                Span::styled("W LIST", Style::default().fg(FG)),
            ]),
            Line::from(vec![
                Span::styled("  • Configure columns:", Style::default().fg(SECONDARY)),
                Span::styled(
                    "W COL SYM LAST CHG% VOL BID ASK SPREAD NEWS",
                    Style::default().fg(FG),
                ),
            ]),
        ];
        frame.render_widget(
            Paragraph::new(empty_msg).block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(format!(" Worksheet: {} (Empty) ", app.worksheet.name))
                    .border_style(Style::default().fg(BORDER)),
            ),
            chunks[1],
        );
        return;
    }

    let visible_capacity = chunks[1].height.saturating_sub(3).max(1) as usize;
    let selected_idx = app.cursor.min(symbols.len().saturating_sub(1));
    let win_start = if selected_idx >= visible_capacity {
        selected_idx + 1 - visible_capacity
    } else {
        0
    };
    let win_end = (win_start + visible_capacity).min(symbols.len());
    let visible_slice = &symbols[win_start..win_end];

    let rows = visible_slice.iter().enumerate().map(|(slice_idx, asset)| {
        let is_selected = slice_idx + win_start == selected_idx;
        let is_loaded_global = *asset == app.asset;
        let q = app.market.quotes.get(&asset.key());
        let chg_pct = q.and_then(|q| q.effective_change_pct());
        let dir_color = theme::direction_color(chg_pct);

        let cells: Vec<Cell> = app
            .worksheet
            .columns
            .iter()
            .map(|col| match col {
                WorksheetColumn::Symbol => {
                    let sym_color = if is_loaded_global {
                        AMBER
                    } else if is_selected {
                        CYAN
                    } else {
                        FG
                    };
                    Cell::from(asset.symbol.clone()).style(Style::default().fg(sym_color).bold())
                }
                WorksheetColumn::Last => {
                    let s = q
                        .map(|q| format_price(q.price))
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(FG))
                }
                WorksheetColumn::ChangePct => {
                    let s = chg_pct
                        .map(|p| format!("{p:+.2}%"))
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(dir_color).bold())
                }
                WorksheetColumn::ChangeAbs => {
                    let s = q
                        .and_then(|q| q.effective_change_abs())
                        .map(format_signed_price)
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(dir_color))
                }
                WorksheetColumn::Volume => {
                    let s = q
                        .and_then(|q| q.volume)
                        .map(number)
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(FG))
                }
                WorksheetColumn::QuoteVolume => {
                    let s = q
                        .and_then(|q| q.quote_volume)
                        .map(number)
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(AMBER))
                }
                WorksheetColumn::Bid => {
                    let s = q
                        .and_then(|q| q.bid)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(SECONDARY))
                }
                WorksheetColumn::Ask => {
                    let s = q
                        .and_then(|q| q.ask)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(SECONDARY))
                }
                WorksheetColumn::SpreadBps => {
                    let s = q
                        .and_then(|q| q.spread_bps())
                        .map(|b| format!("{b:.1}"))
                        .unwrap_or_else(|| "—".into());
                    Cell::from(s).style(Style::default().fg(MUTED))
                }
                WorksheetColumn::PeRatio => {
                    let pe = app
                        .companies
                        .iter()
                        .find(|c| c.ticker.eq_ignore_ascii_case(&asset.symbol))
                        .or_else(|| {
                            app.company
                                .as_ref()
                                .filter(|c| c.ticker.eq_ignore_ascii_case(&asset.symbol))
                        })
                        .and_then(|c| {
                            let ni = c.facts.iter().find(|f| f.metric == "Net income")?.value;
                            let price = q?.price;
                            if ni > Decimal::ZERO {
                                Some(format!("{:.1}x", price))
                            } else {
                                None
                            }
                        })
                        .unwrap_or_else(|| "—".into());
                    Cell::from(pe).style(Style::default().fg(FG))
                }
                WorksheetColumn::MarketCap => {
                    let mcap = app
                        .companies
                        .iter()
                        .find(|c| c.ticker.eq_ignore_ascii_case(&asset.symbol))
                        .or_else(|| {
                            app.company
                                .as_ref()
                                .filter(|c| c.ticker.eq_ignore_ascii_case(&asset.symbol))
                        })
                        .and_then(|c| {
                            let rev = c.facts.iter().find(|f| f.metric == "Revenue")?.value;
                            Some(number(rev))
                        })
                        .unwrap_or_else(|| "—".into());
                    Cell::from(mcap).style(Style::default().fg(SECONDARY))
                }
                WorksheetColumn::NewsCount => {
                    let count = app
                        .news_items
                        .iter()
                        .filter(|n| {
                            if let Some(ref t) = n.ticker
                                && (t.eq_ignore_ascii_case(&asset.symbol)
                                    || t.eq_ignore_ascii_case(asset.base_asset()))
                            {
                                return true;
                            }
                            n.headline.contains(&asset.symbol)
                                || n.headline.contains(asset.base_asset())
                        })
                        .count();
                    let s = if count > 0 {
                        format!("{count}")
                    } else {
                        "—".into()
                    };
                    Cell::from(s).style(Style::default().fg(CYAN))
                }
            })
            .collect();

        Row::new(cells)
    });

    let total_weight: u32 = app
        .worksheet
        .columns
        .iter()
        .map(|c| c.default_weight())
        .sum();
    let constraints: Vec<Constraint> = app
        .worksheet
        .columns
        .iter()
        .map(|c| Constraint::Ratio(c.default_weight(), total_weight.max(1)))
        .collect();

    let header_cells: Vec<Cell> = app
        .worksheet
        .columns
        .iter()
        .map(|c| Cell::from(c.label()))
        .collect();

    let mut state =
        TableState::default().with_selected(Some(selected_idx.saturating_sub(win_start)));

    let table = Table::new(rows, constraints)
        .header(Row::new(header_cells).style(theme::header_style()))
        .row_highlight_style(theme::row_highlight_style())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(format!(
                    " Worksheet: {} — {} / {} securities (Enter/g: GP, d: DES, f: FA, n: CN) ",
                    app.worksheet.name,
                    selected_idx + 1,
                    symbols.len()
                ))
                .border_style(Style::default().fg(if app.focus == 1 { AMBER } else { BORDER })),
        );

    frame.render_stateful_widget(table, chunks[1], &mut state);
}

pub fn market(frame: &mut Frame, app: &App, area: Rect) {
    if app.market_mode == MarketMode::Worksheet {
        render_worksheet(frame, app, area);
        return;
    }
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
            Span::styled(" Quote: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("[{}] ", app.quote_filter),
                Style::default().fg(AMBER).bold(),
            ),
            Span::styled("MinVol: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("{} ", number(app.min_quote_volume)),
                Style::default().fg(FG),
            ),
            Span::styled("Status: ", Style::default().fg(SECONDARY)),
            Span::styled(
                if app.status_filter_trading_only {
                    "TRADING "
                } else {
                    "ALL "
                },
                Style::default().fg(FG),
            ),
            Span::styled("Sort: ", Style::default().fg(SECONDARY)),
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
                        Span::styled(" Mode: ", Style::default().fg(SECONDARY)),
                        Span::styled("CRYPTO ", Style::default().fg(AMBER).bold()),
                        Span::styled("│ Quote: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            format!("{} ", app.quote_filter),
                            Style::default().fg(AMBER).bold(),
                        ),
                        Span::styled("│ MinVol: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            format!("{} ", number(app.min_quote_volume)),
                            Style::default().fg(FG),
                        ),
                        Span::styled("│ Status: ", Style::default().fg(SECONDARY)),
                        Span::styled(
                            if app.status_filter_trading_only {
                                "TRADING "
                            } else {
                                "ALL "
                            },
                            Style::default().fg(FG),
                        ),
                        Span::styled("│ Sort: ", Style::default().fg(SECONDARY)),
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
                        Span::styled(" Mode: ", Style::default().fg(SECONDARY)),
                        Span::styled("EQUITIES ", Style::default().fg(AMBER).bold()),
                        Span::styled("│ Sort: ", Style::default().fg(SECONDARY)),
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
    let stmt_h = (table.rows.len() as u16 + 3).clamp(7, 12);
    let rows = if area.height >= 18 {
        Layout::vertical([
            Constraint::Length(3),
            Constraint::Length(stmt_h),
            Constraint::Min(6),
        ])
        .split(area)
    } else {
        Layout::vertical([Constraint::Length(3), Constraint::Min(6)]).split(area)
    };

    let mut tab_spans = vec![
        Span::styled(
            format!(
                " {} ({}) CIK {:010} ",
                company.name, company.ticker, company.cik
            ),
            Style::default().fg(AMBER).bold(),
        ),
        Span::styled(format!("│ {:?} │ ", app.period), Style::default().fg(FG)),
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

    if rows.len() > 2 && rows[2].height >= 4 {
        render_fin_secondary_analysis(
            frame,
            app,
            rows[2],
            company,
            &table,
            &shown_periods,
            active_col,
        );
    }
}

fn render_fin_secondary_analysis(
    frame: &mut Frame,
    app: &App,
    area: Rect,
    company: &Company,
    table: &facts::ComparativeTable,
    shown_periods: &[chrono::NaiveDate],
    active_col: usize,
) {
    let sel_idx = app.cursor.min(table.rows.len().saturating_sub(1));
    let sel_row = table.rows.get(sel_idx);
    let sel_metric = sel_row.map(|r| r.metric).unwrap_or("Revenue");
    let audit = facts::metric_audit(&company.facts, app.period, sel_metric);
    let sel_fact = sel_row.and_then(|r| r.sources.get(active_col).copied().flatten());

    if app.fin_chart_expanded && !shown_periods.is_empty() {
        let mut rev_data = Vec::new();
        let mut net_data = Vec::new();
        let mut sel_data = Vec::new();

        let rev_row = table.rows.iter().find(|r| r.metric.contains("Revenue"));
        let net_row = table.rows.iter().find(|r| r.metric.contains("Net Income"));

        for (i, _) in shown_periods.iter().enumerate() {
            let x = i as f64;
            if let Some(r) = rev_row
                && let Some(Some(v)) = r.values.get(i)
            {
                rev_data.push((x, v.to_f64().unwrap_or(0.0) / 1e9));
            }
            if let Some(r) = net_row
                && let Some(Some(v)) = r.values.get(i)
            {
                net_data.push((x, v.to_f64().unwrap_or(0.0) / 1e9));
            }
            if let Some(r) = sel_row
                && let Some(Some(v)) = r.values.get(i)
            {
                sel_data.push((x, v.to_f64().unwrap_or(0.0) / 1e9));
            }
        }

        let mut datasets = Vec::new();
        if !rev_data.is_empty() {
            datasets.push(
                Dataset::default()
                    .name("Revenue ($B)")
                    .data(&rev_data)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(CYAN).bold()),
            );
        }
        if !net_data.is_empty() {
            datasets.push(
                Dataset::default()
                    .name("Net Income ($B)")
                    .data(&net_data)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(AMBER).bold()),
            );
        }
        if sel_metric != "Revenue" && sel_metric != "Net Income" && !sel_data.is_empty() {
            datasets.push(
                Dataset::default()
                    .name(format!("{sel_metric} ($B)"))
                    .data(&sel_data)
                    .graph_type(GraphType::Line)
                    .style(Style::default().fg(POSITIVE).bold()),
            );
        }

        if !datasets.is_empty() {
            let low = rev_data
                .iter()
                .chain(&net_data)
                .chain(&sel_data)
                .map(|p| p.1)
                .fold(0.0f64, f64::min);
            let high = rev_data
                .iter()
                .chain(&net_data)
                .chain(&sel_data)
                .map(|p| p.1)
                .fold(1.0f64, f64::max);
            let pad = ((high - low) * 0.1).max(0.5);
            let x_labels: Vec<String> = shown_periods
                .iter()
                .map(|d| d.format("%Y-%m").to_string())
                .collect();

            let fin_chart = Chart::new(datasets)
                .hidden_legend_constraints((
                    Constraint::Percentage(100),
                    Constraint::Percentage(100),
                ))
                .x_axis(
                    Axis::default()
                        .bounds([0.0, shown_periods.len().saturating_sub(1) as f64])
                        .labels(x_labels),
                )
                .y_axis(
                    Axis::default()
                        .bounds([low - pad, high + pad])
                        .labels([format!("{low:.1}B"), format!("{high:.1}B")]),
                )
                .block(
                    Block::default()
                        .borders(Borders::ALL)
                        .title(format!(
                            " {} Financial Multi-Period Visual Trends ($B USD) │ Press 'p' to return to statement audit ",
                            company.ticker
                        ))
                        .border_style(Style::default().fg(CYAN)),
                );
            frame.render_widget(fin_chart, area);
            return;
        }
    }

    let cols = if area.width >= 120 {
        Layout::horizontal([
            Constraint::Percentage(34),
            Constraint::Percentage(36),
            Constraint::Percentage(30),
        ])
        .split(area)
    } else {
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).split(area)
    };

    // 1. Metric Historical Trajectory & Sparkline
    let mut trend_lines = vec![
        Line::from(vec![Span::styled(
            format!(" HISTORICAL TRAJECTORY: {sel_metric} "),
            Style::default().fg(BG).bg(AMBER).bold(),
        )]),
        Line::from(""),
    ];

    if let Some(ref a) = audit {
        let max_val = a.max_value.unwrap_or(Decimal::ONE);
        let min_val = a.min_value.unwrap_or(Decimal::ZERO);
        let bar_width = 16usize;

        for pt in &a.points {
            let ratio = if max_val > Decimal::ZERO {
                ((pt.value / max_val).to_f64().unwrap_or(0.0)).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let filled = (ratio * bar_width as f64).round() as usize;
            let empty = bar_width.saturating_sub(filled);
            let bar_str = format!("{}{}", "█".repeat(filled), "░".repeat(empty));

            trend_lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<8} ", pt.period_label),
                    Style::default().fg(AMBER),
                ),
                Span::styled(
                    format!("{:>14} ", number(pt.value)),
                    Style::default().fg(FG).bold(),
                ),
                Span::styled(bar_str, Style::default().fg(theme::CYAN)),
            ]));
        }
        trend_lines.push(Line::from(""));
        let yoy_str = a
            .latest_yoy_pct
            .map(|y| format!("{y:+.1}%"))
            .unwrap_or_else(|| "--".into());
        trend_lines.push(Line::from(vec![
            Span::styled(" Range: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("{} to {}", number(min_val), number(max_val)),
                Style::default().fg(FG),
            ),
            Span::styled(" │ Latest YoY: ", Style::default().fg(SECONDARY)),
            Span::styled(
                yoy_str,
                Style::default()
                    .fg(theme::direction_color(a.latest_yoy_pct))
                    .bold(),
            ),
        ]));
    } else {
        trend_lines.push(Line::from(Span::styled(
            "Derived ratio metric — computed from comparative primary statement records.",
            Style::default().fg(SECONDARY),
        )));
    }

    frame.render_widget(
        Paragraph::new(trend_lines).block(block(format!(" Metric Trend ({}) ", sel_metric), false)),
        cols[0],
    );

    // 2. XBRL Fact Audit & Exact Unrounded Values
    let mut audit_lines = vec![
        Line::from(vec![
            Span::styled(
                " XBRL FACT AUDIT ",
                Style::default().fg(BG).bg(AMBER).bold(),
            ),
            Span::styled(
                format!(" Period: {:?} ", shown_periods.get(active_col)),
                Style::default().fg(SECONDARY),
            ),
        ]),
        Line::from(""),
    ];

    if let Some(fact) = sel_fact {
        audit_lines.push(Line::from(vec![
            Span::styled("Exact Reported Value: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("{} {}", fact.value, fact.unit),
                Style::default().fg(FG).bold(),
            ),
        ]));
        audit_lines.push(Line::from(vec![
            Span::styled("US-GAAP Taxonomy Tag: ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("us-gaap:{}", fact.concept),
                Style::default().fg(theme::CYAN).bold(),
            ),
        ]));
        audit_lines.push(Line::from(vec![
            Span::styled("Reporting Interval:   ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!(
                    "{} -> {} ({})",
                    fact.start
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| "Point-in-Time".into()),
                    fact.end,
                    if fact.start.is_some() {
                        "Duration Flow"
                    } else {
                        "Instant Balance"
                    }
                ),
                Style::default().fg(FG),
            ),
        ]));
        audit_lines.push(Line::from(vec![
            Span::styled("Filing Form & Date:   ", Style::default().fg(SECONDARY)),
            Span::styled(
                format!("Form {} │ Filed {}", fact.form, fact.filed),
                Style::default().fg(AMBER),
            ),
        ]));
        audit_lines.push(Line::from(vec![
            Span::styled("SEC Accession Number: ", Style::default().fg(SECONDARY)),
            Span::styled(fact.accession.clone(), Style::default().fg(MUTED)),
        ]));
        audit_lines.push(Line::from(vec![
            Span::styled("Provenance Archive:   ", Style::default().fg(SECONDARY)),
            Span::styled(fact.source.clone(), Style::default().fg(POSITIVE)),
        ]));
    } else {
        audit_lines.push(Line::from(Span::styled(
            format!("No standalone primary XBRL fact for '{sel_metric}' in this period.\nConsolidated via standard GAAP comparative period accounting."),
            Style::default().fg(SECONDARY),
        )));
    }

    frame.render_widget(
        Paragraph::new(audit_lines).block(block(" Fact Audit & XBRL Provenance ", false)),
        cols[1],
    );

    // 3. SEC Filings History Archive
    if cols.len() > 2 {
        let max_filings = cols[2].height.saturating_sub(4).max(1) as usize;
        let filing_rows = company.filings.iter().take(max_filings).map(|f| {
            Row::new(vec![
                Cell::from(f.form.clone()).style(Style::default().fg(AMBER).bold()),
                Cell::from(f.filed.format("%Y-%m-%d").to_string()).style(Style::default().fg(FG)),
                Cell::from(
                    f.report_date
                        .map(|d| d.format("%Y-%m-%d").to_string())
                        .unwrap_or_else(|| "--".into()),
                )
                .style(Style::default().fg(SECONDARY)),
                Cell::from(f.description.clone()).style(Style::default().fg(MUTED)),
            ])
        });

        frame.render_widget(
            Table::new(
                filing_rows,
                [
                    Constraint::Length(8),
                    Constraint::Length(12),
                    Constraint::Length(12),
                    Constraint::Min(12),
                ],
            )
            .header(Row::new(["FORM", "FILED", "REPORT", "DOCUMENT"]).style(theme::header_style()))
            .block(block(" SEC Filings Archive (Press 'o' to open) ", false)),
            cols[2],
        );
    }
}

pub fn security_description(frame: &mut Frame, app: &App, area: Rect) {
    if let Some(company) = &app.company {
        render_security_description(frame, app, area, company);
    } else {
        render_crypto_security_description(frame, app, area);
    }
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
        Line::from(vec![Span::styled(
            " Jurisdiction: SEC EDGAR US GAAP XBRL │ Currency: USD │ Verified Financial Reporting",
            Style::default().fg(SECONDARY),
        )]),
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
        Line::from(vec![Span::styled(
            " Market Coverage: Binance Spot Global Exchange Feed │ Verified Pricing Feed",
            Style::default().fg(SECONDARY),
        )]),
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
        format!("[TICKER: {t}]")
    } else {
        "[ALL TICKERS]".to_string()
    };
    cat_spans.push(Span::styled(
        ticker_badge,
        Style::default().fg(AMBER).bold(),
    ));
    cat_spans.push(Span::raw(" "));
    if active_cat == NewsCategory::Filings || app.company.is_some() {
        cat_spans.push(Span::styled(
            format!("[FORM: {}] ", app.filing_form_filter),
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

    let sub_banner = Line::from(vec![Span::styled(
        " Sources: Yahoo Finance · CNBC · Federal Reserve · CoinDesk/Cointelegraph · SEC EDGAR │ Verified Reporting",
        Style::default().fg(SECONDARY),
    )]);

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
    } else if rows[1].width >= 125 {
        let cols = Layout::horizontal([
            Constraint::Percentage(40),
            Constraint::Percentage(36),
            Constraint::Percentage(24),
        ])
        .split(rows[1]);
        render_news_table(frame, app, cols[0], &items, selected_idx);
        render_news_reader_pane(frame, app, cols[1], selected_item.as_ref(), false);
        render_news_context_pane(frame, app, cols[2], selected_item.as_ref());
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
    for (p_idx, p) in body_paragraphs.iter().enumerate() {
        let p_trimmed = p.trim();
        if p_idx == 0 && p_trimmed.eq_ignore_ascii_case(item.headline.trim()) {
            continue;
        }
        for l in p.lines() {
            if current_line_idx >= skip_lines {
                let trimmed = l.trim();
                let line_span = if trimmed.starts_with('▶') {
                    Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(CYAN).bold(),
                    ))
                } else if trimmed.starts_with('•') {
                    Line::from(vec![
                        Span::styled("• ", Style::default().fg(AMBER).bold()),
                        Span::styled(
                            trimmed.trim_start_matches(['•', ' ']).to_string(),
                            Style::default().fg(FG),
                        ),
                    ])
                } else if trimmed.starts_with('│') {
                    Line::from(Span::styled(
                        l.to_string(),
                        Style::default().fg(Color::Rgb(160, 205, 230)),
                    ))
                } else {
                    Line::from(Span::styled(l.to_string(), Style::default().fg(FG)))
                };
                lines.push(line_span);
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

fn render_news_context_pane(
    frame: &mut Frame,
    _app: &App,
    area: Rect,
    item: Option<&omt_core::NewsItem>,
) {
    let Some(item) = item else {
        frame.render_widget(
            Paragraph::new("Select a news story to inspect publisher context.")
                .style(Style::default().fg(SECONDARY))
                .block(block(" Story Context ", false)),
            area,
        );
        return;
    };

    let mut lines = vec![
        Line::from(vec![Span::styled(
            " PUBLISHER CONTEXT ",
            Style::default().fg(BG).bg(AMBER).bold(),
        )]),
        Line::from(""),
        Line::from(vec![
            Span::styled("Publisher: ", Style::default().fg(SECONDARY)),
            Span::styled(item.source.clone(), Style::default().fg(FG).bold()),
        ]),
        Line::from(vec![
            Span::styled("Category:  ", Style::default().fg(SECONDARY)),
            Span::styled(item.category.label(), Style::default().fg(AMBER)),
        ]),
        Line::from(vec![
            Span::styled("Ticker:    ", Style::default().fg(SECONDARY)),
            Span::styled(
                item.ticker.as_deref().unwrap_or("GLOBAL"),
                Style::default().fg(FG).bold(),
            ),
        ]),
        Line::from(vec![
            Span::styled("Published: ", Style::default().fg(SECONDARY)),
            Span::styled(
                item.published_at
                    .format("%Y-%m-%d %H:%M:%S UTC")
                    .to_string(),
                Style::default().fg(FG),
            ),
        ]),
        Line::from(""),
    ];

    if item.is_sec_filing {
        lines.push(Line::from(Span::styled(
            "SEC EDGAR Provenance:",
            Style::default().fg(theme::CYAN).bold(),
        )));
        lines.push(Line::from(vec![
            Span::styled("Form:      ", Style::default().fg(SECONDARY)),
            Span::styled(
                item.form.as_deref().unwrap_or("EDGAR"),
                Style::default().fg(AMBER).bold(),
            ),
        ]));
        if let Some(ref acc) = item.accession {
            lines.push(Line::from(vec![
                Span::styled("Accession: ", Style::default().fg(SECONDARY)),
                Span::styled(acc.clone(), Style::default().fg(MUTED)),
            ]));
        }
        lines.push(Line::from(""));
    }

    let extraction_status = if item.body.is_some() {
        "FULL ARTICLE EXTRACTED"
    } else {
        "FEED SUMMARY / EXCERPT"
    };

    lines.push(Line::from(vec![
        Span::styled("Reader State: ", Style::default().fg(SECONDARY)),
        Span::styled(
            extraction_status,
            Style::default()
                .fg(if item.body.is_some() { POSITIVE } else { AMBER })
                .bold(),
        ),
    ]));
    lines.push(Line::from(""));

    lines.push(Line::from(Span::styled(
        "Available Shortcuts:",
        Style::default().fg(SECONDARY),
    )));
    lines.push(Line::from(vec![
        Span::styled(" [Enter/r] ", theme::keycap_style()),
        Span::styled(" Read / Expand", theme::keycap_label_style()),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" [o]       ", theme::keycap_style()),
        Span::styled(" Open in Browser", theme::keycap_label_style()),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" [c]       ", theme::keycap_style()),
        Span::styled(" Cycle Category", theme::keycap_label_style()),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" [t]       ", theme::keycap_style()),
        Span::styled(" Filter by Ticker", theme::keycap_label_style()),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" [R]       ", theme::keycap_style()),
        Span::styled(" Refresh Feeds", theme::keycap_label_style()),
    ]));

    let block_title = if area.width >= 30 {
        " Story Context & Provenance "
    } else {
        " Story Context "
    };

    frame.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block(block_title, false)),
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

    let (header_h, holdings_h) = if area.height >= 20 {
        (
            4u16,
            (positions.len() as u16 + 4).clamp(6, area.height.saturating_sub(11).max(6)),
        )
    } else {
        (
            3u16,
            (positions.len() as u16 + 4).clamp(6, area.height.saturating_sub(10).max(6)),
        )
    };
    let rows = if area.height >= 16 {
        Layout::vertical([
            Constraint::Length(header_h),
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

    let mut header_lines = vec![
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
    ];

    if header_h >= 4 {
        let mut alloc_spans = vec![Span::styled(" Allocation: ", Style::default().fg(MUTED))];
        for p in &positions {
            let q = app.market.quotes.get(&p.asset.key());
            let valid =
                q.filter(|q| matches!(q.status_at(now), DataStatus::Live | DataStatus::Delayed));
            if let Some(q) = valid
                && let Some((total_mv, ..)) = totals.get(&p.currency)
                && *total_mv > Decimal::ZERO
            {
                let weight_pct = (p.quantity * q.price / *total_mv * Decimal::from(100))
                    .to_f64()
                    .unwrap_or(0.0);
                if weight_pct >= 0.5 {
                    let bar_len = ((weight_pct / 100.0) * 12.0).round().max(1.0) as usize;
                    alloc_spans.push(Span::styled(
                        format!(" {} ", p.asset.symbol),
                        Style::default().fg(AMBER).bold(),
                    ));
                    alloc_spans.push(Span::styled(
                        format!("{weight_pct:.1}% "),
                        Style::default().fg(FG),
                    ));
                    alloc_spans.push(Span::styled("█".repeat(bar_len), Style::default().fg(CYAN)));
                    alloc_spans.push(Span::raw(" "));
                }
            }
        }
        header_lines.push(Line::from(alloc_spans));
    }

    frame.render_widget(
        Paragraph::new(header_lines).style(Style::default().bg(ELEVATED_BG)),
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
        let sel_pos_idx = app.cursor.min(positions.len().saturating_sub(1));
        let sel_pos = positions.get(sel_pos_idx);

        let cols = if rows[2].width >= 120 {
            Layout::horizontal([
                Constraint::Percentage(28),
                Constraint::Percentage(34),
                Constraint::Percentage(38),
            ])
            .split(rows[2])
        } else {
            Layout::horizontal([Constraint::Percentage(45), Constraint::Percentage(55)])
                .split(rows[2])
        };

        // Subpanel 1: Text-Based Asset Allocation Bar
        let _total_cost_sum: Decimal = totals.values().map(|(_, c, ..)| *c).sum();
        let total_mv_sum: Decimal = totals.values().map(|(mv, ..)| *mv).sum();
        let bar_len = (cols[0].width.saturating_sub(6) as usize).clamp(8, 28);
        let mut bar_spans = Vec::new();

        if positions.len() == 1 {
            let p = &positions[0];
            bar_spans.push(Span::styled(
                format!(" [{}] 100% ", p.asset.symbol),
                Style::default().fg(AMBER).bold(),
            ));
            bar_spans.push(Span::styled(
                "█".repeat(bar_len),
                Style::default().fg(AMBER),
            ));
        } else if total_mv_sum > Decimal::ZERO {
            bar_spans.push(Span::styled(" [", Style::default().fg(SECONDARY)));
            let colors = [
                AMBER,
                theme::CYAN,
                POSITIVE,
                Color::Rgb(255, 153, 51),
                Color::Rgb(102, 204, 204),
            ];
            for (i, p) in positions.iter().enumerate() {
                let q = app.market.quotes.get(&p.asset.key());
                let mv = q.map(|q| p.quantity * q.price).unwrap_or(Decimal::ZERO);
                let ratio = (mv / total_mv_sum).to_f64().unwrap_or(0.0).clamp(0.0, 1.0);
                let chars = (ratio * bar_len as f64).round() as usize;
                let color = colors[i % colors.len()];
                if chars > 0 {
                    bar_spans.push(Span::styled("█".repeat(chars), Style::default().fg(color)));
                }
            }
            bar_spans.push(Span::styled("]", Style::default().fg(SECONDARY)));
        }

        let mut alloc_lines = vec![
            Line::from(vec![Span::styled(
                " ASSET ALLOCATION ",
                Style::default().fg(BG).bg(AMBER).bold(),
            )]),
            Line::from(""),
            Line::from(bar_spans),
            Line::from(""),
        ];

        for p in &positions {
            let q = app.market.quotes.get(&p.asset.key());
            let mv = q.map(|q| p.quantity * q.price).unwrap_or(Decimal::ZERO);
            let pct_str = if total_mv_sum > Decimal::ZERO {
                format!("{:.1}%", mv / total_mv_sum * Decimal::from(100))
            } else {
                "--".into()
            };
            alloc_lines.push(Line::from(vec![
                Span::styled(
                    format!(" {:<8} ", p.asset.symbol),
                    Style::default().fg(AMBER),
                ),
                Span::styled(
                    format!("{:>10} ", number(mv)),
                    Style::default().fg(FG).bold(),
                ),
                Span::styled(format!("({pct_str})"), Style::default().fg(SECONDARY)),
            ]));
        }

        frame.render_widget(
            Paragraph::new(alloc_lines).block(block(" Portfolio Allocation ", false)),
            cols[0],
        );

        // Subpanel 2: Selected Position Deep Dive
        if cols.len() > 2 {
            let mut detail_lines = vec![
                Line::from(vec![
                    Span::styled(
                        " POSITION DETAILS ",
                        Style::default().fg(BG).bg(AMBER).bold(),
                    ),
                    Span::styled(
                        format!(" {} ", sel_pos.map(|p| p.asset.key()).unwrap_or_default()),
                        Style::default().fg(SECONDARY),
                    ),
                ]),
                Line::from(""),
            ];

            if let Some(p) = sel_pos {
                let q = app.market.quotes.get(&p.asset.key());
                let valid_q = q
                    .filter(|q| matches!(q.status_at(now), DataStatus::Live | DataStatus::Delayed));
                let mv = valid_q.map(|q| p.quantity * q.price);
                let unreal = valid_q.and_then(|q| p.unrealized(q.price, &q.currency));
                let unreal_pct = valid_q.and_then(|q| p.unrealized_pct(q.price, &q.currency));

                detail_lines.push(Line::from(vec![
                    Span::styled("Position Size:   ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        format!("{} {}", number(p.quantity), p.asset.symbol),
                        Style::default().fg(FG).bold(),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Average Cost:    ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        p.average_cost()
                            .map(format_price)
                            .unwrap_or_else(|| "--".into()),
                        Style::default().fg(AMBER),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Current Price:   ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        q.map(|q| format_price(q.price))
                            .unwrap_or_else(|| "unavailable".into()),
                        Style::default().fg(FG).bold(),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Total Cost Basis:", Style::default().fg(SECONDARY)),
                    Span::styled(
                        format!("{} {}", number(p.cost), p.currency),
                        Style::default().fg(FG),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Market Value:    ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        mv.map(number).unwrap_or_else(|| "unavailable".into()),
                        Style::default().fg(FG).bold(),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Unrealized P&L:  ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        unreal
                            .map(format_signed_price)
                            .unwrap_or_else(|| "--".into()),
                        Style::default().fg(theme::direction_color(unreal)).bold(),
                    ),
                    Span::styled(
                        unreal_pct
                            .map(|p| format!(" ({p:+.2}%)"))
                            .unwrap_or_default(),
                        Style::default().fg(theme::direction_color(unreal)),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Realized Profit: ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        format_signed_price(p.realized),
                        Style::default().fg(theme::direction_color(Some(p.realized))),
                    ),
                ]));
                detail_lines.push(Line::from(vec![
                    Span::styled("Quote Freshness: ", Style::default().fg(SECONDARY)),
                    Span::styled(
                        q.map(|q| format!("{:?} ({})", q.status_at(now), q.provider))
                            .unwrap_or_else(|| "No Quote Feed".into()),
                        Style::default().fg(MUTED),
                    ),
                ]));
            }

            frame.render_widget(
                Paragraph::new(detail_lines).block(block(" Holding Deep Dive ", false)),
                cols[1],
            );
        }

        // Subpanel 3: Execution Blotter Table
        let blotter_idx = if cols.len() > 2 { 2 } else { 1 };
        let max_tx = cols[blotter_idx].height.saturating_sub(3).max(1) as usize;
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
            cols[blotter_idx],
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
    let selected_idx = app.cursor.min(app.macro_data.len().saturating_sub(1));
    let (data, xmin, xmax, x_labels, selected_point) = if treasury && app.econ_axis_categorical {
        let mut pts: Vec<(f64, f64)> = Vec::new();
        for (i, r) in app.macro_data.iter().take(100).enumerate() {
            if let Some(v) = r.value.to_f64() {
                pts.push((i as f64, v));
            }
        }
        let xmax = pts.len().saturating_sub(1).max(1) as f64;
        let mut labels = Vec::new();
        for (i, r) in app.macro_data.iter().take(100).enumerate() {
            if i == 0
                || i == 2
                || i == 4
                || i == 5
                || i == 6
                || i == 8
                || i == 10
                || i == pts.len().saturating_sub(1)
            {
                labels.push(r.label.clone());
            }
        }
        let sel_pt: Vec<(f64, f64)> = app
            .macro_data
            .get(selected_idx)
            .and_then(|r| Some((selected_idx as f64, r.value.to_f64()?)))
            .into_iter()
            .collect();
        (pts, 0.0, xmax, labels, sel_pt)
    } else {
        let mut pts: Vec<_> = app
            .macro_data
            .iter()
            .take(100)
            .filter_map(|r| Some((macro_x(r)?, r.value.to_f64()?)))
            .collect();
        pts.sort_by(|a, b| a.0.total_cmp(&b.0));
        let xmin = pts.first().map(|p| p.0).unwrap_or(0.0);
        let xmax = pts.last().map(|p| p.0).unwrap_or(1.0).max(xmin + 1.0);
        let labels = if treasury {
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
        let sel_pt: Vec<(f64, f64)> = app
            .macro_data
            .get(selected_idx)
            .and_then(|r| Some((macro_x(r)?, r.value.to_f64()?)))
            .into_iter()
            .collect();
        (pts, xmin, xmax, labels, sel_pt)
    };
    if data.is_empty() {
        return;
    }
    let min = data.iter().map(|p| p.1).fold(f64::INFINITY, f64::min);
    let max = data.iter().map(|p| p.1).fold(f64::NEG_INFINITY, f64::max);
    let pad = ((max - min) * 0.12).max(0.05);
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
            format!(
                " US Treasury Par Yield Curve ({}) — m: Toggle Axis Mode │ j/k: Inspect Tenor ",
                if app.econ_axis_categorical {
                    "Categorical Tenor Spacing"
                } else {
                    "Proportional Maturity Axis"
                }
            )
        } else {
            " Historical Macroeconomic Series (Chronological) ".into()
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

fn equity_auth_diagnostic(app: &App) -> (&'static str, &'static str) {
    match app.market.chart_error.as_deref().unwrap_or("") {
        error if error.contains("401") => (
            "ALPACA REJECTED CREDENTIALS (HTTP 401)",
            "Both credentials were loaded, but Alpaca rejected the request.\n\
             Verify APCA_API_KEY_ID and APCA_API_SECRET_KEY are a matching pair in .env.\n\
             Restart the app after changing .env.",
        ),
        error if error.contains("403") => (
            "ALPACA DATA ACCESS DENIED (HTTP 403)",
            "Alpaca denied this data request. Check your account's IEX data permissions.\n\
             If you change credentials in .env, restart the app.",
        ),
        _ => (
            "AUTHENTICATION REQUIRED FOR EQUITY DATA",
            "Set both APCA_API_KEY_ID and APCA_API_SECRET_KEY in .env or the system environment.\n\
             Restart the app after changing .env.",
        ),
    }
}

pub fn render_market_pulse_card(frame: &mut Frame, app: &App, area: Rect, active: bool) {
    let btc_q = app.market.quotes.get("BINANCE:BTCUSDT");
    let eth_q = app.market.quotes.get("BINANCE:ETHUSDT");
    let sol_q = app.market.quotes.get("BINANCE:SOLUSDT");

    let status_desc = match app.market.chart_status {
        ChartDataStatus::AuthenticationRequired => {
            format!(
                "Chart for {}: {}",
                app.asset.key(),
                equity_auth_diagnostic(app).0
            )
        }
        ChartDataStatus::Error => {
            let err = app.market.chart_error.as_deref().unwrap_or("Network error");
            format!("Chart for {}: {err} (Press 'R' to retry)", app.asset.key())
        }
        ChartDataStatus::Unsupported => {
            format!(
                "Chart for {}: Unsupported timeframe or venue",
                app.asset.key()
            )
        }
        _ => {
            format!(
                "Chart for {}: Press 'R' to refresh or :CHART BTCUSDT",
                app.asset.key()
            )
        }
    };

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                " MARKET PULSE & BENCHMARKS ",
                Style::default().fg(BG).bg(AMBER).bold(),
            ),
            Span::styled(
                format!("  {} ", status_desc),
                Style::default().fg(SECONDARY),
            ),
        ]),
        Line::from(""),
    ];

    let fmt_quote = |sym: &str, q: Option<&Quote>| -> Vec<Span<'static>> {
        if let Some(q) = q {
            let chg_pct = q.effective_change_pct();
            let color = theme::direction_color(chg_pct);
            let arrow = match chg_pct {
                Some(p) if p > Decimal::ZERO => "▲ +",
                Some(p) if p < Decimal::ZERO => "▼ ",
                _ => "  ",
            };
            let pct_str = chg_pct
                .map(|p| format!("{p:.2}%"))
                .unwrap_or_else(|| "--".into());
            vec![
                Span::styled(format!(" {sym}: "), Style::default().fg(FG).bold()),
                Span::styled(
                    format!("{} ", format_price(q.price)),
                    Style::default().fg(FG),
                ),
                Span::styled(
                    format!("{arrow}{pct_str} "),
                    Style::default().fg(color).bold(),
                ),
                Span::styled(" │ ", Style::default().fg(theme::INACTIVE_BORDER)),
            ]
        } else {
            vec![
                Span::styled(format!(" {sym}: "), Style::default().fg(MUTED)),
                Span::styled("waiting for feed │ ", Style::default().fg(MUTED)),
            ]
        }
    };

    let mut benchmark_spans = Vec::new();
    benchmark_spans.extend(fmt_quote("BTC", btc_q));
    benchmark_spans.extend(fmt_quote("ETH", eth_q));
    benchmark_spans.extend(fmt_quote("SOL", sol_q));
    lines.push(Line::from(benchmark_spans));
    lines.push(Line::from(""));

    let mut advancers = 0usize;
    let mut decliners = 0usize;
    let mut total_vol = Decimal::ZERO;
    for q in app.market.quotes.values() {
        if let Some(p) = q.effective_change_pct() {
            if p > Decimal::ZERO {
                advancers += 1;
            } else if p < Decimal::ZERO {
                decliners += 1;
            }
        }
        if let Some(qv) = q.quote_volume {
            total_vol += qv;
        }
    }
    let total_active = advancers + decliners;
    let adv_pct = if total_active > 0 {
        (advancers as f64 / total_active as f64) * 100.0
    } else {
        50.0
    };

    let bar_len = (area.width.saturating_sub(26) as usize).clamp(10, 36);
    let adv_chars = ((adv_pct / 100.0) * bar_len as f64).round() as usize;
    let dec_chars = bar_len.saturating_sub(adv_chars);

    lines.push(Line::from(vec![
        Span::styled(" Market Breadth: ", Style::default().fg(SECONDARY)),
        Span::styled(
            format!("+{advancers} "),
            Style::default().fg(POSITIVE).bold(),
        ),
        Span::styled("█".repeat(adv_chars), Style::default().fg(POSITIVE)),
        Span::styled("█".repeat(dec_chars), Style::default().fg(NEGATIVE)),
        Span::styled(
            format!(" -{decliners} "),
            Style::default().fg(NEGATIVE).bold(),
        ),
        Span::styled(
            format!("({adv_pct:.0}% adv) │ Vol: {} USDT", number(total_vol)),
            Style::default().fg(SECONDARY),
        ),
    ]));
    lines.push(Line::from(""));

    let mut sorted_quotes: Vec<&Quote> = app.market.quotes.values().collect();
    sorted_quotes.sort_by(|a, b| {
        let pa = a.effective_change_pct().unwrap_or(Decimal::ZERO);
        let pb = b.effective_change_pct().unwrap_or(Decimal::ZERO);
        pb.cmp(&pa)
    });

    lines.push(Line::from(vec![
        Span::styled(" Top Gainers: ", Style::default().fg(POSITIVE).bold()),
        Span::styled(
            sorted_quotes
                .iter()
                .take(3)
                .map(|q| {
                    let p = q.effective_change_pct().unwrap_or_default();
                    format!("{} {:+.1}%", q.base_symbol(), p)
                })
                .collect::<Vec<_>>()
                .join("  ·  "),
            Style::default().fg(FG),
        ),
    ]));
    lines.push(Line::from(vec![
        Span::styled(" Top Decliners: ", Style::default().fg(NEGATIVE).bold()),
        Span::styled(
            sorted_quotes
                .iter()
                .rev()
                .take(3)
                .map(|q| {
                    let p = q.effective_change_pct().unwrap_or_default();
                    format!("{} {:+.1}%", q.base_symbol(), p)
                })
                .collect::<Vec<_>>()
                .join("  ·  "),
            Style::default().fg(FG),
        ),
    ]));

    frame.render_widget(
        Paragraph::new(lines).block(block(
            format!(
                " LAUNCHPAD: Market Pulse & Diagnostics ({}) — 'R' to Retry Chart ",
                app.asset.key()
            ),
            active,
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
        let stale_badge = if app.market.chart_status == ChartDataStatus::Stale {
            " │ [STALE CACHE - Press R to refresh]"
        } else {
            ""
        };
        let outer = block(
            format!(
                " {} <GP> │ {} │ {} │ {}{}{} │ Overlay: {} │ Sub: {} ",
                app.asset.key(),
                app.timeframe,
                app.asset.currency(),
                live_label,
                stale_badge,
                "",
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
            let msg = match app.market.chart_status {
                ChartDataStatus::AuthenticationRequired => {
                    let (title, guidance) = equity_auth_diagnostic(app);
                    format!(
                        "{title}\n\n\
                         {} historical candles unavailable.\n\n\
                         {guidance}\n\n\
                         Alternative Feeds:\n\
                         • Run :CHART BTCUSDT (Binance Spot public discovery)\n\
                         • Run :CHART COINBASE:BTC-USD (Coinbase public exchange)\n\
                         • Run :FIN {} (SEC EDGAR public XBRL fundamentals)\n\n\
                         Press 'R' to retry the current credentials.",
                        app.asset.key(),
                        app.asset.symbol
                    )
                }
                ChartDataStatus::Loading => {
                    format!(
                        "LOADING HISTORICAL CANDLES\n\n\
                         Fetching {} {} bars from {}...\n\
                         Waiting for provider response and local SQLite cache synchronization.\n\n\
                         Press 'R' to re-request candles or 't' to change timeframe.",
                        app.asset.key(),
                        app.timeframe,
                        app.asset.venue
                    )
                }
                ChartDataStatus::Unsupported => {
                    format!(
                        "UNSUPPORTED TIMEFRAME OR INSTRUMENT\n\n\
                         Provider {} does not supply {} candles for {}.\n\n\
                         • Press 't' to cycle timeframe (1m, 5m, 15m, 1h, 1d)\n\
                         • Run :CHART BTCUSDT for full Binance Spot coverage",
                        app.asset.venue,
                        app.timeframe,
                        app.asset.key()
                    )
                }
                ChartDataStatus::RateLimited => {
                    format!(
                        "RATE LIMIT REACHED (HTTP 429/418)\n\n\
                         Provider {} rate limit exceeded. Automatic backoff in progress.\n\
                         Cooling down requests to preserve API integrity.\n\n\
                         Press 'R' to retry once cooldown expires.",
                        app.asset.venue
                    )
                }
                ChartDataStatus::Empty => {
                    format!(
                        "NO HISTORICAL DATA RETURNED\n\n\
                         Provider returned 0 bars for {} on timeframe {}.\n\n\
                         • Press 't' to cycle timeframe\n\
                         • Press 'R' to re-request\n\
                         • Run :MARKET to select an actively traded pair",
                        app.asset.key(),
                        app.timeframe
                    )
                }
                ChartDataStatus::Error => {
                    let err = app
                        .market
                        .chart_error
                        .as_deref()
                        .unwrap_or("Network request failed");
                    format!(
                        "UNABLE TO LOAD HISTORICAL CANDLES\n\n\
                         Error: {err}\n\n\
                         • Press 'R' to retry\n\
                         • Press 't' to cycle timeframe\n\
                         • Run :CHART BTCUSDT to verify network connectivity",
                    )
                }
                ChartDataStatus::Stale | ChartDataStatus::NotRequested | ChartDataStatus::Ready => {
                    format!(
                        "No historical candles loaded for {}.\n\
                         Waiting for provider response or local SQLite cache.\n\n\
                         Controls:\n\
                         • t: Timeframe (1m, 5m, 15m, 1h, 4h, 1d)\n\
                         • i: Overlay (SMA, EMA, Bollinger, None)\n\
                         • g: Sub-Indicator (Volume, RSI, MACD, ATR)\n\
                         • +/-: Zoom in / Zoom out\n\
                         • h/l: Pan historical bars\n\
                         • R: Force refresh",
                        app.asset.key()
                    )
                }
            };
            Paragraph::new(msg)
                .style(
                    Style::default().fg(if app.market.chart_status.is_actionable_error() {
                        AMBER
                    } else {
                        SECONDARY
                    }),
                )
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
        Workspace::Market => {
            if app.market_mode == MarketMode::Worksheet {
                let asset = app.worksheet.symbols.get(
                    app.cursor
                        .min(app.worksheet.symbols.len().saturating_sub(1)),
                )?;
                let q = app.market.quotes.get(&asset.key());
                let now = Utc::now();
                Some(format!(
                    "WORKSHEET INSPECTOR\nList: {} (Item {} of {})\n\nSymbol: {}\nVenue: {}\nAsset Class: {:?}\nBase / Quote: {} / {}\n\nMarket Depth & Pricing:\nLast: {}\n24h Change: {}\n24h High: {}\n24h Low: {}\nBid: {}\nAsk: {}\nSpread: {}\nVolume: {}\nTurnover: {}\n\nRelated Functions:\n[Enter / g] GP  Price Graph\n[d]         DES Security Description\n[f]         FA  Financial Analysis\n[n]         CN  Company News\n[x]         Remove from Worksheet\n\nData Status: {}",
                    app.worksheet.name,
                    app.cursor + 1,
                    app.worksheet.symbols.len(),
                    asset.symbol,
                    asset.venue,
                    asset.asset_class,
                    asset.base_asset(),
                    asset.currency(),
                    q.map(|x| format!("{} {}", format_price(x.price), x.currency))
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.effective_change_pct())
                        .map(|p| format!("{p:+.2}%"))
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.high)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.low)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.bid)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.ask)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.spread_bps())
                        .map(|b| format!("{b:.1} bps"))
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.volume)
                        .map(number)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.quote_volume)
                        .map(number)
                        .unwrap_or_else(|| "—".into()),
                    q.map(|x| format!("{:?}", x.status_at(now)))
                        .unwrap_or_else(|| "No feed".into())
                ))
            } else if let Some(key) = selected_market_instrument(app) {
                let q = app.market.quotes.get(&key);
                let info = app.market.instruments.get(&key);
                let now = Utc::now();
                Some(format!(
                    "MARKET INSPECTOR\nInstrument: {}\nVenue: {}\nStatus: {}\n\nPrice: {}\n24h Change: {}\n24h High: {}\n24h Low: {}\nBid: {}\nAsk: {}\nSpread: {}\nVolume: {}\nQuote Volume: {}\n\nRelated Functions:\n[Enter] GP  Chart\n[d]     DES Description\n[w]     Add to Worksheet\n\nFeed Freshness: {}",
                    key,
                    info.map(|i| i.asset.venue.to_string())
                        .unwrap_or_else(|| "BINANCE".into()),
                    info.map(|i| i.status.to_string())
                        .unwrap_or_else(|| "Trading".into()),
                    q.map(|x| format!("{} {}", format_price(x.price), x.currency))
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.effective_change_pct())
                        .map(|p| format!("{p:+.2}%"))
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.high)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.low)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.bid)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.ask)
                        .map(format_price)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.spread_bps())
                        .map(|b| format!("{b:.1} bps"))
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.volume)
                        .map(number)
                        .unwrap_or_else(|| "—".into()),
                    q.and_then(|x| x.quote_volume)
                        .map(number)
                        .unwrap_or_else(|| "—".into()),
                    q.map(|x| format!("{:?}", x.status_at(now)))
                        .unwrap_or_else(|| "No feed".into())
                ))
            } else {
                None
            }
        }
        Workspace::Des => {
            if let Some(c) = &app.company {
                Some(format!(
                    "{}\nCIK {:010}\nTicker: {}\nVenue: IEX (US Equity)\n\nSEC Reporting:\nFilings: {}\nJurisdiction: US GAAP XBRL\nPeriod: {:?}\nLink Group: {}\n\nRelated Functions:\n[2:GP] Price Graph\n[3:FA] Financial Analysis\n[4:CN] Company News\n[5:EQS] Screener\n[O] Open Filing\n\nFetched: {}",
                    c.name,
                    c.cik,
                    c.ticker,
                    c.filings.len(),
                    app.period,
                    app.link_group.label(),
                    c.fetched_at.format("%Y-%m-%d %H:%M UTC")
                ))
            } else {
                let key = app.asset.key();
                let q = app.market.quotes.get(&key);
                Some(format!(
                    "{}\nVenue: {}\nAsset Class: Crypto Spot\nBase: {}\nQuote: {}\n\nMarket Depth:\nLast: {}\nBid: {}\nAsk: {}\nSpread: {}\nLink Group: {}\n\nRelated Functions:\n[2:GP] Price Graph\n[4:CN] News Feeds\n[1:MKT] Market Overview\n[5:EQS] Crypto Screener\n\nData Status: Live",
                    app.asset.symbol,
                    app.asset.venue,
                    app.asset.base_asset(),
                    app.asset.currency(),
                    q.map(|x| number(x.price)).unwrap_or_else(|| "--".into()),
                    q.and_then(|x| x.bid).map(number).unwrap_or_else(|| "--".into()),
                    q.and_then(|x| x.ask).map(number).unwrap_or_else(|| "--".into()),
                    q.and_then(|x| x.spread_bps())
                        .map(|b| format!("{b:.1} bps"))
                        .unwrap_or_else(|| "--".into()),
                    app.link_group.label()
                ))
            }
        }
        _ => None,
    }
}
