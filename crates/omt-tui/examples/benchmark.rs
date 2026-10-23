//! Deterministic synthetic workload, exclusively for benchmarks; never normal app data.
use chrono::{Duration, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use omt_core::{
    AssetClass, AssetId, Candle, DataStatus, InstrumentInfo, Quote, TradingStatus, Venue,
    command::Workspace,
};
use omt_tui::App;
use ratatui::{Terminal, backend::TestBackend};
use rust_decimal::Decimal;
use std::time::Instant;

fn pct(samples: &mut [f64], p: usize) -> f64 {
    samples.sort_by(f64::total_cmp);
    samples[(samples.len() * p / 100).min(samples.len() - 1)]
}

fn make_quote(
    asset: AssetId,
    price: Decimal,
    open: Decimal,
    qvol: Decimal,
    epoch: chrono::DateTime<Utc>,
    ms: i64,
) -> Quote {
    Quote {
        currency: asset.currency().into(),
        base_asset: Some(asset.base_asset().into()),
        asset,
        price,
        change_pct: Some((price - open) / open * Decimal::from(100)),
        change_abs: Some(price - open),
        open_24h: Some(open),
        high: Some(price + Decimal::from(5)),
        low: Some((price - Decimal::from(5)).max(Decimal::ONE)),
        volume: Some(Decimal::from(1000)),
        quote_volume: Some(qvol),
        bid: Some((price - Decimal::new(1, 1)).max(Decimal::ONE)),
        ask: Some(price + Decimal::new(1, 1)),
        vwap: Some(price),
        provider: "BENCHMARK ONLY".into(),
        coverage: "SYNTHETIC TEST".into(),
        event_at: epoch + Duration::milliseconds(ms),
        received_at: Utc::now(),
        status: DataStatus::Historical,
        delay_seconds: None,
    }
}

fn main() {
    let epoch = Utc::now() - Duration::days(1);
    let mut app = App {
        workspace: Workspace::Chart,
        ..Default::default()
    };
    app.watchlist = (0..3)
        .map(|i| format!("BINANCE:WATCH{i}USDT").parse::<AssetId>().unwrap())
        .collect();
    app.asset = app.watchlist[0].clone();
    for (i, a) in app.watchlist.iter().enumerate() {
        app.market.ingest(make_quote(
            a.clone(),
            Decimal::from(100 + i as i64),
            Decimal::from(98 + i as i64),
            Decimal::from(1_000_000),
            epoch,
            1,
        ));
    }
    let mem_3_assets_bytes = app.market.quotes.len() * std::mem::size_of::<Quote>()
        + app.watchlist.len() * std::mem::size_of::<AssetId>();

    // Populate 500 historical candles for chart benchmark
    app.market.candles = (0..500)
        .map(|i| Candle {
            open_at: epoch + Duration::minutes(i),
            open: (100 + i % 20).into(),
            high: (103 + i % 20).into(),
            low: (98 + i % 20).into(),
            close: (101 + i % 20).into(),
            volume: (1000 + i % 100).into(),
            closed: true,
        })
        .collect();

    let mut terminal = Terminal::new(TestBackend::new(140, 40)).unwrap();
    let mut ingestion = Vec::with_capacity(10_000);
    let mut chart_render = Vec::with_capacity(500);
    let mut combined = Vec::with_capacity(500);
    let overall = Instant::now();

    // Benchmark 1: Chart rendering + 10-symbol streaming updates
    let bench_assets: Vec<AssetId> = (0..10)
        .map(|i| format!("BINANCE:TEST{i}USDT").parse::<AssetId>().unwrap())
        .collect();
    for frame in 0..500 {
        let first_receive = Instant::now();
        for asset in &bench_assets {
            let quote = make_quote(
                asset.clone(),
                Decimal::from(100 + frame % 20),
                Decimal::from(99),
                Decimal::from(5_000_000),
                epoch,
                10 + frame,
            );
            let t = Instant::now();
            assert!(app.market.ingest(quote));
            ingestion.push(t.elapsed().as_secs_f64() * 1e6);
        }
        let t = Instant::now();
        terminal.draw(|f| omt_tui::render(f, &app)).unwrap();
        chart_render.push(t.elapsed().as_secs_f64() * 1000.0);
        combined.push(first_receive.elapsed().as_secs_f64() * 1000.0);
    }

    // Benchmark 2: Full 3,500-symbol Binance Spot Universe discovery, throughput, and virtualized MARKET navigation
    let quotes_ccy = ["USDT", "USDC", "BTC", "FDUSD", "EUR"];
    let mut instruments = Vec::with_capacity(3500);
    for i in 0..3500 {
        let q_ccy = quotes_ccy[i % quotes_ccy.len()];
        let sym = format!("SYM{i:04}{q_ccy}");
        instruments.push(InstrumentInfo {
            asset: AssetId {
                symbol: sym,
                asset_class: AssetClass::Crypto,
                venue: Venue::Binance,
            },
            base_asset: format!("SYM{i:04}"),
            quote_asset: q_ccy.into(),
            status: if i % 10 == 0 {
                TradingStatus::Break
            } else {
                TradingStatus::Trading
            },
            price_precision: 4,
            quantity_precision: 4,
            provider: "BENCHMARK ONLY".into(),
            coverage: "SYNTHETIC TEST".into(),
        });
    }
    app.market.register_instruments(instruments.clone());

    let bulk_start = Instant::now();
    for (i, info) in instruments.iter().enumerate() {
        if info.status == TradingStatus::Trading {
            let q = make_quote(
                info.asset.clone(),
                Decimal::from(10 + (i % 500) as i64),
                Decimal::from(9 + (i % 500) as i64),
                Decimal::from((i as i64 + 1) * 50_000),
                epoch,
                2000 + i as i64,
            );
            let t = Instant::now();
            app.market.ingest(q);
            ingestion.push(t.elapsed().as_secs_f64() * 1e6);
        }
    }
    let bulk_secs = bulk_start.elapsed().as_secs_f64();
    let throughput_eps = (app.market.quotes.len() as f64) / bulk_secs.max(1e-6);

    let mem_universe_bytes = app.market.instruments.len()
        * (std::mem::size_of::<String>() + std::mem::size_of::<InstrumentInfo>() + 64)
        + app.market.quotes.len()
            * (std::mem::size_of::<String>() + std::mem::size_of::<Quote>() + 64);

    app.workspace = Workspace::Market;
    app.quote_filter = "ALL".into();
    app.sync_market_selection();
    let mut market_render = Vec::with_capacity(200);
    let mut nav_latency_us = Vec::with_capacity(200);
    for step in 0..200 {
        let nav_t = Instant::now();
        app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        nav_latency_us.push(nav_t.elapsed().as_secs_f64() * 1e6);

        // Simulate incoming miniTicker batch while navigating
        let idx = step % instruments.len();
        let q = make_quote(
            instruments[idx].asset.clone(),
            Decimal::from(25 + (step % 30) as i64),
            Decimal::from(20),
            Decimal::from(9_000_000),
            epoch,
            10_000 + step as i64,
        );
        app.market.ingest(q);

        let draw_t = Instant::now();
        terminal.draw(|f| omt_tui::render(f, &app)).unwrap();
        market_render.push(draw_t.elapsed().as_secs_f64() * 1000.0);
    }

    println!(
        "Synthetic benchmark only: 3,500 discovered instruments, 500 chart frames, 200 full-universe market frames"
    );
    println!("mem_3_watched_bytes={mem_3_assets_bytes}");
    println!("mem_3500_universe_bytes={mem_universe_bytes}");
    println!("universe_throughput_updates_per_sec={throughput_eps:.0}");
    println!("ingest_p50_us={:.3}", pct(&mut ingestion, 50));
    println!("ingest_p95_us={:.3}", pct(&mut ingestion, 95));
    println!("ingest_p99_us={:.3}", pct(&mut ingestion, 99));
    println!("draw_chart_p50_ms={:.3}", pct(&mut chart_render, 50));
    println!("draw_chart_p95_ms={:.3}", pct(&mut chart_render, 95));
    println!("draw_chart_p99_ms={:.3}", pct(&mut chart_render, 99));
    println!("draw_market_3500_p50_ms={:.3}", pct(&mut market_render, 50));
    println!("draw_market_3500_p95_ms={:.3}", pct(&mut market_render, 95));
    println!("draw_market_3500_p99_ms={:.3}", pct(&mut market_render, 99));
    println!("nav_under_load_p95_us={:.3}", pct(&mut nav_latency_us, 95));
    println!(
        "batch_receive_to_buffer_p95_ms={:.3}",
        pct(&mut combined, 95)
    );
    println!("total_seconds={:.3}", overall.elapsed().as_secs_f64());
    println!(
        "retained_instruments={} retained_quotes={} retained_candles={}",
        app.market.instruments.len(),
        app.market.quotes.len(),
        app.market.candles.len()
    );
    println!(
        "Excludes network, 33ms coalescing/frame schedules, terminal transport and physical display latency."
    );
}
