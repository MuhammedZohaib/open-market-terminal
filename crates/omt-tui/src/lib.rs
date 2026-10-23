#![forbid(unsafe_code)]
pub mod theme;
pub mod views;
use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use omt_core::{
    AssetId, Company, MacroObservation, NewsCategory, NewsItem, PeriodView, Timeframe, Venue,
    command::{self, Workspace},
    facts::StatementTab,
    portfolio::Transaction,
};
use omt_data::MarketState;
use ratatui::{prelude::*, widgets::*};
use rust_decimal::Decimal;
use std::collections::HashSet;
pub use theme::{
    AMBER, BG, BORDER, ELEVATED_BG, FG, GOLD, MUTED, NEGATIVE, PANEL_BG, POSITIVE, SECONDARY,
    SELECTION,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LayoutMode {
    #[default]
    Standard,
    FullWidth,
    Split,
    Quad,
}
impl LayoutMode {
    pub fn next(self) -> Self {
        match self {
            Self::Standard => Self::FullWidth,
            Self::FullWidth => Self::Split,
            Self::Split => Self::Quad,
            Self::Quad => Self::Standard,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::FullWidth => "FULL",
            Self::Split => "SPLIT 2-PANE",
            Self::Quad => "LAUNCHPAD 2x2",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SubIndicator {
    #[default]
    Volume,
    Rsi,
    Macd,
    Atr,
}
impl SubIndicator {
    pub fn next(self) -> Self {
        match self {
            Self::Volume => Self::Rsi,
            Self::Rsi => Self::Macd,
            Self::Macd => Self::Atr,
            Self::Atr => Self::Volume,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Volume => "VOL",
            Self::Rsi => "RSI(14)",
            Self::Macd => "MACD(12,26,9)",
            Self::Atr => "ATR(14)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverlayMode {
    #[default]
    Sma,
    Ema,
    Bollinger,
    None,
}
impl OverlayMode {
    pub fn next(self) -> Self {
        match self {
            Self::Sma => Self::Ema,
            Self::Ema => Self::Bollinger,
            Self::Bollinger => Self::None,
            Self::None => Self::Sma,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Sma => "SMA20 · SMA50 +",
            Self::Ema => "EMA12 · EMA26 +",
            Self::Bollinger => "BB20(2) ^ · v",
            Self::None => "No overlay",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MarketMode {
    #[default]
    Overview,
    Crypto,
    Wei,
}
impl MarketMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Overview => "MARKET OVERVIEW",
            Self::Crypto => "CRYPTO SPOT UNIVERSE",
            Self::Wei => "WORLD EQUITY INDICES (WEI)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScanMode {
    #[default]
    Crypto,
    Equities,
}
impl ScanMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Crypto => "SCAN CRYPTO (BINANCE SPOT)",
            Self::Equities => "SCAN EQUITIES (SEC EDGAR)",
        }
    }
}

pub struct App {
    pub account: String,
    pub name: String,
    pub workspace: Workspace,
    pub asset: AssetId,
    pub timeframe: Timeframe,
    pub market: MarketState,
    pub watchlist: Vec<AssetId>,
    pub list_name: String,
    pub focus: usize,
    pub cursor: usize,
    pub watch_cursor: usize,
    pub input: Option<String>,
    pub history: Vec<String>,
    pub history_index: Option<usize>,
    pub overlay: OverlayMode,
    pub sub_indicator: SubIndicator,
    pub follow_live: bool,
    pub layout_mode: LayoutMode,
    pub show_watchlist: bool,
    pub show_context: bool,
    pub panel_width_offset: i16,
    pub des_mode: bool,
    pub diagnostics: bool,
    pub help: bool,
    pub settings: bool,
    pub message: String,
    pub quit: bool,
    pub company: Option<Company>,
    pub period: PeriodView,
    pub stmt_tab: StatementTab,
    pub fin_col: usize,
    pub filing_form_filter: String,
    pub news_items: Vec<NewsItem>,
    pub news_read_ids: HashSet<String>,
    pub news_category: NewsCategory,
    pub news_ticker_filter: Option<String>,
    pub news_reader_expanded: bool,
    pub news_scroll: u16,
    pub news_updated_at: Option<DateTime<Utc>>,
    pub recent_securities: Vec<AssetId>,
    pub last_equity_asset: AssetId,
    pub last_crypto_asset: AssetId,
    pub macro_data: Vec<MacroObservation>,
    pub transactions: Vec<Transaction>,
    pub companies: Vec<Company>,
    pub filter: String,
    pub sort: String,
    pub sort_ascending: bool,
    pub quote_filter: String,
    pub status_filter_trading_only: bool,
    pub min_quote_volume: Decimal,
    pub market_mode: MarketMode,
    pub scan_mode: ScanMode,
    pub selected_market_key: Option<String>,
    pub candle_cursor: Option<usize>,
    pub full_width: bool,
    pub workspace_history: Vec<(Workspace, AssetId)>,
    pub draw_samples_us: Vec<u64>,
    pub zoom: usize,
    pub offset: usize,
    pub ascii: bool,
    pub catalog: Vec<String>,
    pub research_status: String,
}
impl Default for App {
    fn default() -> Self {
        let default_crypto: AssetId = "BTCUSDT".parse().unwrap();
        let default_equity: AssetId = "AAPL".parse().unwrap();
        Self {
            account: "default".into(),
            name: "Open Market Terminal".into(),
            workspace: Workspace::Market,
            asset: default_crypto.clone(),
            timeframe: Timeframe::H1,
            market: MarketState {
                status: "Starting".into(),
                ..Default::default()
            },
            watchlist: vec![],
            list_name: "default".into(),
            focus: 1,
            cursor: 0,
            watch_cursor: 0,
            input: None,
            history: vec![],
            history_index: None,
            overlay: OverlayMode::Sma,
            sub_indicator: SubIndicator::Volume,
            follow_live: true,
            layout_mode: LayoutMode::Standard,
            show_watchlist: true,
            show_context: true,
            panel_width_offset: 0,
            des_mode: false,
            diagnostics: false,
            help: false,
            settings: false,
            message: "Ready — Press : or / for GO> command bar, F1 for help".into(),
            quit: false,
            company: None,
            period: PeriodView::Annual,
            stmt_tab: StatementTab::Overview,
            fin_col: 0,
            filing_form_filter: "ALL".into(),
            news_items: vec![],
            news_read_ids: HashSet::new(),
            news_category: NewsCategory::All,
            news_ticker_filter: None,
            news_reader_expanded: false,
            news_scroll: 0,
            news_updated_at: None,
            recent_securities: vec![default_crypto.clone(), default_equity.clone()],
            last_equity_asset: default_equity,
            last_crypto_asset: default_crypto,
            macro_data: vec![],
            transactions: vec![],
            companies: vec![],
            filter: String::new(),
            sort: "qvol".into(),
            sort_ascending: false,
            quote_filter: "USDT".into(),
            status_filter_trading_only: true,
            min_quote_volume: Decimal::ZERO,
            market_mode: MarketMode::Overview,
            scan_mode: ScanMode::Crypto,
            selected_market_key: None,
            candle_cursor: None,
            full_width: false,
            workspace_history: vec![],
            draw_samples_us: vec![],
            zoom: 70,
            offset: 0,
            ascii: false,
            catalog: vec![
                "BTCUSDT".into(),
                "ETHUSDT".into(),
                "SOLUSDT".into(),
                "AAPL".into(),
                "MSFT".into(),
                "COINBASE:BTC-USD".into(),
            ],
            research_status: "Not loaded".into(),
        }
    }
}
impl App {
    pub fn set_active_asset(&mut self, asset: AssetId) {
        if asset.venue == Venue::Iex {
            self.last_equity_asset = asset.clone();
        } else {
            self.last_crypto_asset = asset.clone();
        }
        self.recent_securities.retain(|a| a != &asset);
        self.recent_securities.insert(0, asset.clone());
        if self.recent_securities.len() > 8 {
            self.recent_securities.truncate(8);
        }
        self.asset = asset;
    }
    pub fn record_draw_micros(&mut self, us: u64) {
        if self.draw_samples_us.len() >= 256 {
            self.draw_samples_us.remove(0);
        }
        self.draw_samples_us.push(us);
    }
    pub fn draw_percentiles_us(&self) -> (u64, u64, u64) {
        if self.draw_samples_us.is_empty() {
            return (0, 0, 0);
        }
        let mut s = self.draw_samples_us.clone();
        s.sort_unstable();
        let n = s.len();
        (
            s[(n * 50 / 100).min(n - 1)],
            s[(n * 95 / 100).min(n - 1)],
            s[(n * 99 / 100).min(n - 1)],
        )
    }
    pub fn cycle_quote_filter(&mut self) {
        const QUOTES: &[&str] = &["USDT", "USDC", "BTC", "FDUSD", "EUR", "BNB", "ALL"];
        let pos = QUOTES
            .iter()
            .position(|q| q.eq_ignore_ascii_case(&self.quote_filter))
            .unwrap_or(0);
        self.quote_filter = QUOTES[(pos + 1) % QUOTES.len()].into();
        self.cursor = 0;
        self.sync_market_selection();
        self.message = format!("Quote currency filter: {}", self.quote_filter);
    }
    pub fn cycle_min_volume(&mut self) {
        let thresholds = [
            Decimal::ZERO,
            Decimal::from(100_000),
            Decimal::from(1_000_000),
            Decimal::from(10_000_000),
            Decimal::from(50_000_000),
        ];
        let pos = thresholds
            .iter()
            .position(|v| *v == self.min_quote_volume)
            .unwrap_or(0);
        self.min_quote_volume = thresholds[(pos + 1) % thresholds.len()];
        self.cursor = 0;
        self.sync_market_selection();
        self.message = format!("Min 24h quote volume: {}", self.min_quote_volume);
    }
    pub fn cycle_sort(&mut self) {
        if self.workspace == Workspace::Scan && self.scan_mode == ScanMode::Equities {
            const EQ_SORTS: &[&str] = &["revenue", "income", "assets", "symbol"];
            let pos = EQ_SORTS.iter().position(|s| *s == self.sort).unwrap_or(0);
            self.sort = EQ_SORTS[(pos + 1) % EQ_SORTS.len()].into();
        } else {
            const MKT_SORTS: &[&str] = &["qvol", "change", "price", "volume", "symbol"];
            let pos = MKT_SORTS.iter().position(|s| *s == self.sort).unwrap_or(0);
            self.sort = MKT_SORTS[(pos + 1) % MKT_SORTS.len()].into();
        }
        if let Some(ref key) = self.selected_market_key.clone() {
            let keys = views::filtered_market_keys(self);
            if let Some(idx) = keys.iter().position(|k| k == key) {
                self.cursor = idx;
            }
        }
        self.message = format!(
            "Sort: {} ({})",
            self.sort.to_ascii_uppercase(),
            if self.sort_ascending { "ASC" } else { "DESC" }
        );
    }
    pub fn sync_market_selection(&mut self) {
        let keys = views::filtered_market_keys(self);
        if keys.is_empty() {
            self.selected_market_key = None;
            self.cursor = 0;
        } else {
            let idx = self.cursor.min(keys.len() - 1);
            self.cursor = idx;
            self.selected_market_key = Some(keys[idx].clone());
        }
    }
    pub fn mark_selected_news_read(&mut self) {
        if let Some(item) = views::selected_news_item(self) {
            self.news_read_ids.insert(item.id);
        }
    }
    pub fn key(&mut self, key: KeyEvent) -> Option<String> {
        if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
            self.quit = true;
            return None;
        }
        if let Some(input) = self.input.as_mut() {
            match key.code {
                KeyCode::Esc => {
                    self.input = None;
                    self.history_index = None;
                }
                KeyCode::Enter => {
                    let cmd = self.input.take();
                    self.history_index = None;
                    if let Some(ref text) = cmd {
                        let trimmed = text.trim();
                        if !trimmed.is_empty()
                            && self.history.last().map(String::as_str) != Some(trimmed)
                        {
                            self.history.push(trimmed.to_string());
                            if self.history.len() > 50 {
                                self.history.remove(0);
                            }
                        }
                    }
                    return cmd;
                }
                KeyCode::Backspace => {
                    input.pop();
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.clear();
                }
                KeyCode::Up => {
                    if !self.history.is_empty() {
                        let idx = match self.history_index {
                            Some(i) => i.saturating_sub(1),
                            None => self.history.len() - 1,
                        };
                        self.history_index = Some(idx);
                        *input = self.history[idx].clone();
                    }
                }
                KeyCode::Down => {
                    if let Some(i) = self.history_index {
                        if i + 1 < self.history.len() {
                            self.history_index = Some(i + 1);
                            *input = self.history[i + 1].clone();
                        } else {
                            self.history_index = None;
                            input.clear();
                        }
                    }
                }
                KeyCode::Tab => {
                    if let Some(s) = command::suggestions(input, &self.catalog).first() {
                        *input = s.clone();
                    }
                }
                KeyCode::Char(c) => input.push(c),
                _ => {}
            }
            return None;
        }
        if self.help || self.settings {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Enter | KeyCode::F(1)
            ) {
                self.help = false;
                self.settings = false;
            }
            return None;
        }
        match key.code {
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('/') | KeyCode::Char(':') => {
                self.input = Some(String::new());
                self.history_index = None;
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input = Some(String::new());
                self.history_index = None;
            }
            KeyCode::Char('?') | KeyCode::F(1) => self.help = true,
            KeyCode::F(12) => {
                self.diagnostics = !self.diagnostics;
                self.message = if self.diagnostics {
                    "Diagnostics overlay enabled (F12 to hide)".into()
                } else {
                    "Diagnostics overlay hidden".into()
                };
            }
            KeyCode::Char(c @ '1'..='7') => {
                let target = Workspace::ALL[c as usize - '1' as usize];
                if target != self.workspace {
                    self.workspace_history
                        .push((self.workspace, self.asset.clone()));
                    self.cursor = 0;
                    self.news_scroll = 0;
                }
                self.workspace = target;
                return Some(self.workspace.name().into());
            }
            KeyCode::F(n @ 2..=8) => {
                let target = Workspace::ALL[(n - 2) as usize];
                if target != self.workspace {
                    self.workspace_history
                        .push((self.workspace, self.asset.clone()));
                    self.cursor = 0;
                    self.news_scroll = 0;
                }
                self.workspace = target;
                return Some(self.workspace.name().into());
            }
            KeyCode::Tab => self.focus = (self.focus + 1) % 3,
            KeyCode::BackTab => self.focus = (self.focus + 2) % 3,
            KeyCode::Char('z') => {
                self.full_width = !self.full_width;
                self.layout_mode = if self.full_width {
                    LayoutMode::FullWidth
                } else {
                    LayoutMode::Standard
                };
                self.message = if self.full_width {
                    "Full-width central workspace enabled (press z to restore panels, L to cycle layouts)".into()
                } else {
                    "Multi-panel workstation layout restored".into()
                };
            }
            KeyCode::Char('L') => {
                self.layout_mode = self.layout_mode.next();
                self.full_width = self.layout_mode == LayoutMode::FullWidth;
                self.message = format!("Workstation layout mode: {}", self.layout_mode.label());
            }
            KeyCode::Char('W') => {
                self.show_watchlist = !self.show_watchlist;
                self.message = format!(
                    "Left Watchlist panel: {}",
                    if self.show_watchlist {
                        "VISIBLE"
                    } else {
                        "HIDDEN"
                    }
                );
            }
            KeyCode::Char('I') => {
                self.show_context = !self.show_context;
                self.message = format!(
                    "Right Context Inspector panel: {}",
                    if self.show_context {
                        "VISIBLE"
                    } else {
                        "HIDDEN"
                    }
                );
            }
            KeyCode::Char('<') => {
                self.panel_width_offset = (self.panel_width_offset - 2).max(-8);
                self.message = format!("Side panel width offset: {:+}", self.panel_width_offset);
            }
            KeyCode::Char('>') => {
                self.panel_width_offset = (self.panel_width_offset + 2).min(16);
                self.message = format!("Side panel width offset: {:+}", self.panel_width_offset);
            }
            KeyCode::Char('d') if matches!(self.workspace, Workspace::Fin | Workspace::Chart) => {
                self.des_mode = !self.des_mode;
                self.message = if self.des_mode {
                    "Security Description <DES> view enabled (press d to toggle)".into()
                } else {
                    "Standard workspace view restored".into()
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.focus == 0 {
                    let max = self.watchlist.len().saturating_sub(1);
                    self.watch_cursor = (self.watch_cursor + 1).min(max);
                } else {
                    let max = views::active_row_count(self).saturating_sub(1);
                    self.cursor = (self.cursor + 1).min(max);
                    self.news_scroll = 0;
                    if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                        self.sync_market_selection();
                    } else if self.workspace == Workspace::News {
                        self.mark_selected_news_read();
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.focus == 0 {
                    self.watch_cursor = self.watch_cursor.saturating_sub(1);
                } else {
                    self.cursor = self.cursor.saturating_sub(1);
                    self.news_scroll = 0;
                    if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                        self.sync_market_selection();
                    } else if self.workspace == Workspace::News {
                        self.mark_selected_news_read();
                    }
                }
            }
            KeyCode::PageDown => {
                if self.workspace == Workspace::News && self.news_reader_expanded {
                    self.news_scroll = self.news_scroll.saturating_add(12);
                } else {
                    let max = views::active_row_count(self).saturating_sub(1);
                    self.cursor = (self.cursor + 20).min(max);
                    self.news_scroll = 0;
                    if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                        self.sync_market_selection();
                    }
                }
            }
            KeyCode::PageUp => {
                if self.workspace == Workspace::News && self.news_reader_expanded {
                    self.news_scroll = self.news_scroll.saturating_sub(12);
                } else {
                    self.cursor = self.cursor.saturating_sub(20);
                    self.news_scroll = 0;
                    if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                        self.sync_market_selection();
                    }
                }
            }
            KeyCode::Home => {
                if self.workspace == Workspace::Chart {
                    self.candle_cursor = None;
                    self.offset = self.market.candles.len().saturating_sub(20);
                    self.follow_live = false;
                    self.message =
                        "Jumped to earliest loaded candle (press End for live edge)".into();
                } else {
                    self.cursor = 0;
                    self.news_scroll = 0;
                    if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                        self.sync_market_selection();
                    }
                }
            }
            KeyCode::End => {
                if self.workspace == Workspace::Chart {
                    self.candle_cursor = None;
                    self.offset = 0;
                    self.follow_live = true;
                    self.message = "Anchored to latest candle (Follow-Live active)".into();
                } else {
                    let max = views::active_row_count(self).saturating_sub(1);
                    self.cursor = max;
                    self.news_scroll = 0;
                    if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                        self.sync_market_selection();
                    }
                }
            }
            KeyCode::Enter => {
                if self.focus == 0 {
                    if let Some(a) = self.watchlist.get(
                        self.watch_cursor
                            .min(self.watchlist.len().saturating_sub(1)),
                    ) {
                        return Some(format!("CHART {}", a.key()));
                    }
                } else if self.workspace == Workspace::Market {
                    if let Some(key) = views::selected_market_instrument(self) {
                        return Some(format!("CHART {key}"));
                    }
                } else if self.workspace == Workspace::Scan {
                    if self.scan_mode == ScanMode::Equities {
                        if let Some(ticker) = views::selected_equity_ticker(self) {
                            return Some(format!("FIN {ticker}"));
                        }
                    } else if let Some(key) = views::selected_market_instrument(self) {
                        return Some(format!("CHART {key}"));
                    }
                } else if self.workspace == Workspace::News {
                    self.mark_selected_news_read();
                    return Some("READ".into());
                } else if self.workspace == Workspace::Fin {
                    return Some("OPEN".into());
                } else if self.workspace == Workspace::Port
                    && let Some(key) = views::selected_portfolio_asset(self)
                {
                    return Some(format!("CHART {key}"));
                }
            }
            KeyCode::Char('o') if matches!(self.workspace, Workspace::News | Workspace::Fin) => {
                return Some("OPEN".into());
            }
            KeyCode::Char('r') if self.workspace == Workspace::News => {
                self.news_reader_expanded = !self.news_reader_expanded;
                self.mark_selected_news_read();
                return Some("READ".into());
            }
            KeyCode::Char('R') if self.workspace == Workspace::News => {
                return Some("REFRESH".into());
            }
            KeyCode::Char('c') if self.workspace == Workspace::News => {
                self.news_category = self.news_category.next();
                self.cursor = 0;
                self.news_scroll = 0;
                self.message = format!("News category: {}", self.news_category.label());
            }
            KeyCode::Char('u') if matches!(self.workspace, Workspace::Market | Workspace::Scan) => {
                self.cycle_quote_filter();
            }
            KeyCode::Char('l') if matches!(self.workspace, Workspace::Market | Workspace::Scan) => {
                self.cycle_min_volume();
            }
            KeyCode::Char('x') if matches!(self.workspace, Workspace::Market | Workspace::Scan) => {
                self.status_filter_trading_only = !self.status_filter_trading_only;
                self.cursor = 0;
                self.sync_market_selection();
                self.message = if self.status_filter_trading_only {
                    "Filter: TRADING instruments only".into()
                } else {
                    "Filter: ALL instrument statuses (TRADING, HALT, BREAK)".into()
                };
            }
            KeyCode::Char('x') if self.workspace == Workspace::Chart => {
                if self.candle_cursor.is_some() {
                    self.candle_cursor = None;
                    self.message = "Chart crosshair cursor hidden".into();
                } else {
                    self.candle_cursor = Some(0);
                    self.message = "Chart crosshair active (use [ and ] to inspect bars)".into();
                }
            }
            KeyCode::Char('s') => {
                if self.workspace == Workspace::Fin {
                    self.stmt_tab = self.stmt_tab.next();
                    self.cursor = 0;
                    self.message = format!("Financial statement view: {}", self.stmt_tab.name());
                } else if matches!(self.workspace, Workspace::Market | Workspace::Scan) {
                    self.cycle_sort();
                }
            }
            KeyCode::Char('S') if self.workspace == Workspace::Fin => {
                self.stmt_tab = self.stmt_tab.prev();
                self.cursor = 0;
                self.message = format!("Financial statement view: {}", self.stmt_tab.name());
            }
            KeyCode::Char('o') if matches!(self.workspace, Workspace::Market | Workspace::Scan) => {
                self.sort_ascending = !self.sort_ascending;
                if let Some(ref key) = self.selected_market_key.clone() {
                    let keys = views::filtered_market_keys(self);
                    if let Some(idx) = keys.iter().position(|k| k == key) {
                        self.cursor = idx;
                    }
                }
                self.message = format!(
                    "Sort order: {}",
                    if self.sort_ascending {
                        "ASCENDING"
                    } else {
                        "DESCENDING"
                    }
                );
            }
            KeyCode::Char('m') if self.workspace == Workspace::Scan => {
                self.scan_mode = match self.scan_mode {
                    ScanMode::Crypto => ScanMode::Equities,
                    ScanMode::Equities => ScanMode::Crypto,
                };
                self.cursor = 0;
                self.message = format!("Switched to {}", self.scan_mode.label());
            }
            KeyCode::Char('m') if self.workspace == Workspace::Market => {
                self.market_mode = match self.market_mode {
                    MarketMode::Overview => MarketMode::Crypto,
                    MarketMode::Crypto => MarketMode::Wei,
                    MarketMode::Wei => MarketMode::Overview,
                };
                if self.market_mode == MarketMode::Crypto && self.quote_filter == "USDT" {
                    self.quote_filter = "ALL".into();
                } else if self.market_mode == MarketMode::Overview && self.quote_filter == "ALL" {
                    self.quote_filter = "USDT".into();
                }
                self.cursor = 0;
                self.sync_market_selection();
                self.message = format!("View: {}", self.market_mode.label());
            }
            KeyCode::Char('w')
                if matches!(
                    self.workspace,
                    Workspace::Market | Workspace::Scan | Workspace::Chart
                ) =>
            {
                let target_key = if self.workspace == Workspace::Chart {
                    Some(self.asset.key())
                } else {
                    views::selected_market_instrument(self)
                };
                if let Some(key) = target_key {
                    let watched = self.watchlist.iter().any(|a| a.key() == key);
                    return Some(format!(
                        "WATCH {} {key}",
                        if watched { "REMOVE" } else { "ADD" }
                    ));
                }
            }
            KeyCode::Char('f') if self.workspace == Workspace::News => {
                const FILTERS: &[&str] = &["ALL", "10-K/10-Q", "8-K", "PROXY", "INSIDER"];
                let pos = FILTERS
                    .iter()
                    .position(|f| *f == self.filing_form_filter)
                    .unwrap_or(0);
                self.filing_form_filter = FILTERS[(pos + 1) % FILTERS.len()].into();
                self.cursor = 0;
                self.news_scroll = 0;
                self.message = format!("SEC filing form filter: {}", self.filing_form_filter);
            }
            KeyCode::Char('f') if self.workspace == Workspace::Chart => {
                self.follow_live = !self.follow_live;
                if self.follow_live {
                    self.offset = 0;
                    self.message = "Follow-Live mode enabled (anchored to latest candle)".into();
                } else {
                    self.message = "Follow-Live mode paused (historical pan)".into();
                }
            }
            KeyCode::Char('g') if self.workspace == Workspace::Chart => {
                self.sub_indicator = self.sub_indicator.next();
                self.message = format!("Chart sub-indicator: {}", self.sub_indicator.label());
            }
            KeyCode::Char('t') if self.workspace == Workspace::News => {
                if self.news_ticker_filter.is_some() {
                    self.news_ticker_filter = None;
                    self.message = "News ticker filter: ALL SECURITIES".into();
                } else {
                    let sym = self.asset.base_asset().to_string();
                    self.news_ticker_filter = Some(sym.clone());
                    self.message = format!("News ticker filter: {sym}");
                }
                self.cursor = 0;
                self.news_scroll = 0;
            }
            KeyCode::Char('t') => {
                let i = Timeframe::ALL
                    .iter()
                    .position(|t| *t == self.timeframe)
                    .unwrap_or(0);
                self.timeframe = Timeframe::ALL[(i + 1) % 6];
                return Some(format!("CHART {} {}", self.asset.key(), self.timeframe));
            }
            KeyCode::Char('i') if self.workspace == Workspace::Chart => {
                self.overlay = self.overlay.next();
                self.message = format!("Chart overlay: {}", self.overlay.label());
            }
            KeyCode::Char('[') if self.workspace == Workspace::Chart => {
                let next = self.candle_cursor.unwrap_or(0).saturating_add(1).min(240);
                self.candle_cursor = Some(next);
            }
            KeyCode::Char(']') if self.workspace == Workspace::Chart => match self.candle_cursor {
                Some(0) | None => self.candle_cursor = None,
                Some(n) => self.candle_cursor = Some(n - 1),
            },
            KeyCode::Char('[') if self.workspace == Workspace::News => {
                self.news_scroll = self.news_scroll.saturating_sub(4);
            }
            KeyCode::Char(']') if self.workspace == Workspace::News => {
                self.news_scroll = self.news_scroll.saturating_add(4);
            }
            KeyCode::Char('[') if self.workspace == Workspace::Fin => {
                self.stmt_tab = self.stmt_tab.prev();
                self.cursor = 0;
            }
            KeyCode::Char(']') if self.workspace == Workspace::Fin => {
                self.stmt_tab = self.stmt_tab.next();
                self.cursor = 0;
            }
            KeyCode::Char('a') if self.workspace == Workspace::Fin => {
                self.period = PeriodView::Annual;
                self.fin_col = 0;
            }
            KeyCode::Char('v') if self.workspace == Workspace::Fin => {
                self.period = PeriodView::Quarterly;
                self.fin_col = 0;
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.zoom = self.zoom.saturating_sub(25).max(15);
                self.message = format!("Chart zoom in (window target: {} bars)", self.zoom);
            }
            KeyCode::Char('-') => {
                self.zoom = (self.zoom + 25).min(240);
                self.message = format!("Chart zoom out (window target: {} bars)", self.zoom);
            }
            KeyCode::Left | KeyCode::Char('h') => {
                if self.workspace == Workspace::Fin {
                    self.fin_col = self.fin_col.saturating_sub(1);
                } else {
                    let prev = self.offset;
                    self.offset =
                        (self.offset + 10).min(self.market.candles.len().saturating_sub(5));
                    if self.offset > 0 {
                        self.follow_live = false;
                    }
                    if self.workspace == Workspace::Chart
                        && self.offset > prev
                        && !self.market.candles.is_empty()
                        && self.offset + 50 >= self.market.candles.len()
                    {
                        return Some("OLDER_HISTORY".into());
                    }
                }
            }
            KeyCode::Right | KeyCode::Char('l') if self.workspace == Workspace::Fin => {
                self.fin_col = (self.fin_col + 1).min(5);
            }
            KeyCode::Right => {
                self.offset = self.offset.saturating_sub(10);
                if self.offset == 0 {
                    self.follow_live = true;
                }
            }
            KeyCode::Backspace => {
                return Some("BACK".into());
            }
            KeyCode::Esc => {
                if self.candle_cursor.is_some() {
                    self.candle_cursor = None;
                } else if self.news_reader_expanded {
                    self.news_reader_expanded = false;
                } else {
                    self.focus = 1;
                }
            }
            _ => {}
        }
        None
    }
}

pub(crate) fn block<'a>(title: impl Into<Line<'a>>, active: bool) -> Block<'a> {
    Block::bordered()
        .title(title)
        .border_style(Style::default().fg(if active { AMBER } else { BORDER }))
        .style(Style::default().bg(PANEL_BG).fg(FG))
}

pub fn render(frame: &mut Frame, app: &App) {
    render_inner(frame, app);
    if app.ascii {
        for cell in &mut frame.buffer_mut().content {
            let sym = cell.symbol();
            if !sym.is_ascii() {
                let replacement = match sym {
                    "─" | "━" | "—" | "–" | "┈" => "-",
                    "│" | "┃" | "▆" | "┊" | "║" => "|",
                    "┌" | "┐" | "└" | "┘" | "├" | "┤" | "┬" | "┴" | "┼" => "+",
                    "█" | "▓" | "▒" | "░" | "▪" => "#",
                    "▲" => "^",
                    "▼" => "v",
                    "•" | "·" | "●" => ".",
                    "←" => "<",
                    "→" | "▶" => ">",
                    _ => ".",
                };
                cell.set_symbol(replacement);
            }
        }
    }
}

fn workspace_provider_summary(app: &App) -> (&'static str, String, Color) {
    let now = Utc::now();
    match app.workspace {
        Workspace::Market => match app.market_mode {
            MarketMode::Wei => ("Global Indices", "Delayed / Verify Feed".into(), AMBER),
            _ => {
                let status = app
                    .market
                    .venue_status
                    .get("BINANCE")
                    .map(String::as_str)
                    .unwrap_or(app.market.status.as_str());
                let trading = app.market.trading_instrument_count();
                let count_label = if trading > 0 {
                    format!("{status} ({trading} pairs)")
                } else {
                    format!("{status} ({} quotes)", app.market.quotes.len())
                };
                let color = if status == "Live" { POSITIVE } else { AMBER };
                ("Binance Spot", count_label, color)
            }
        },
        Workspace::Scan => match app.scan_mode {
            ScanMode::Crypto => {
                let status = app
                    .market
                    .venue_status
                    .get("BINANCE")
                    .map(String::as_str)
                    .unwrap_or("Ready");
                let trading = app.market.trading_instrument_count();
                (
                    "Binance Spot",
                    format!("{status} ({trading} indexed)"),
                    if status == "Live" { POSITIVE } else { AMBER },
                )
            }
            ScanMode::Equities => (
                "SEC EDGAR",
                format!("{} cached filers", app.companies.len()),
                POSITIVE,
            ),
        },
        Workspace::Chart => {
            let venue = app.asset.venue.to_string();
            let q_status = app
                .market
                .quotes
                .get(&app.asset.key())
                .map(|q| q.status_at(now));
            let label = match q_status {
                Some(s) => format!("{s:?}"),
                None => app
                    .market
                    .venue_status
                    .get(&venue)
                    .cloned()
                    .unwrap_or_else(|| app.market.status.clone()),
            };
            let color = q_status.map(theme::status_color).unwrap_or(AMBER);
            (
                match app.asset.venue {
                    omt_core::Venue::Binance => "Binance Spot",
                    omt_core::Venue::Coinbase => "Coinbase Exchange",
                    omt_core::Venue::Iex => "Alpaca IEX",
                },
                label,
                color,
            )
        }
        Workspace::Fin => {
            let status = if let Some(c) = &app.company {
                format!("CIK {:010} ({})", c.cik, c.fetched_at.format("%H:%M UTC"))
            } else {
                app.research_status.clone()
            };
            ("SEC EDGAR", status, POSITIVE)
        }
        Workspace::News => {
            let count = views::filtered_news_items(app).len();
            let status = if let Some(ts) = app.news_updated_at {
                format!("{} stories ({})", count, ts.format("%H:%M UTC"))
            } else if let Some(c) = &app.company {
                format!("CIK {:010} ({} filings)", c.cik, c.filings.len())
            } else {
                format!("{count} cached stories")
            };
            ("Multi-Feed / SEC", status, POSITIVE)
        }
        Workspace::Port => (
            "Local SQLite Ledger",
            format!("Acct: {} ({} tx)", app.account, app.transactions.len()),
            POSITIVE,
        ),
        Workspace::Econ => {
            let src = app
                .macro_data
                .first()
                .map(|m| m.source.as_str())
                .unwrap_or("US Treasury / Macro");
            let status = app
                .macro_data
                .first()
                .map(|m| format!("Obs: {}", m.date))
                .unwrap_or_else(|| app.research_status.clone());
            (
                if src == "US Treasury" {
                    "US Treasury"
                } else {
                    "Official Macro"
                },
                status,
                POSITIVE,
            )
        }
    }
}

fn render_inner(frame: &mut Frame, app: &App) {
    let area = frame.area();
    frame.render_widget(Block::default().style(Style::default().bg(BG).fg(FG)), area);
    if area.width < 28 || area.height < 9 {
        frame.render_widget(
            Paragraph::new("OMT: enlarge terminal to 28 x 9").style(Style::default().fg(AMBER)),
            area,
        );
        return;
    }
    let rows = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(3),
        Constraint::Length(2),
    ])
    .split(area);

    // Row 0: Compact Global Bloomberg-inspired Status Header
    let (provider_name, feed_health, health_color) = workspace_provider_summary(app);
    let active_quote = app.market.quotes.get(&app.asset.key());
    let quote_badge = if let Some(q) = active_quote {
        let chg = q
            .effective_change_pct()
            .map(|p| format!(" {p:+.2}%"))
            .unwrap_or_default();
        format!(
            "{} {} {}{}",
            q.asset.key(),
            q.price.normalize(),
            q.currency,
            chg
        )
    } else if let Some(c) = &app.company {
        format!("{}:{} ({})", app.asset.venue, c.ticker, c.name)
    } else {
        app.asset.key()
    };
    let quote_color = active_quote
        .and_then(|q| q.effective_change_pct())
        .map(|p| theme::direction_color(Some(p)))
        .unwrap_or(FG);

    let recent_trail = if app.recent_securities.len() > 1 && area.width >= 130 {
        let trail_symbols: Vec<String> = app
            .recent_securities
            .iter()
            .take(4)
            .map(|a| a.symbol.clone())
            .collect();
        format!("Recent: {} │ ", trail_symbols.join(" · "))
    } else {
        String::new()
    };

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(
                " OMT ",
                Style::default()
                    .fg(BG)
                    .bg(AMBER)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} <{}> ", app.workspace.name(), app.workspace.mnemonic()),
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│ ", Style::default().fg(BORDER)),
            Span::styled(quote_badge, Style::default().fg(quote_color).bold()),
            Span::styled(" │ ", Style::default().fg(BORDER)),
            Span::styled(
                format!("[{}] ", app.layout_mode.label()),
                Style::default().fg(SECONDARY).bold(),
            ),
            Span::styled(recent_trail, Style::default().fg(MUTED)),
            Span::styled(format!("{provider_name}: "), Style::default().fg(SECONDARY)),
            Span::styled(feed_health, Style::default().fg(health_color)),
            Span::styled(" │ ", Style::default().fg(BORDER)),
            Span::styled(
                format!("{} UTC", Utc::now().format("%Y-%m-%d %H:%M:%S")),
                Style::default().fg(SECONDARY),
            ),
        ]))
        .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    // Row 1: Function Tabs + First-Class GO > Command Bar
    let mut nav_spans: Vec<Span> = Workspace::ALL
        .iter()
        .enumerate()
        .map(|(i, w)| {
            let active = *w == app.workspace;
            Span::styled(
                format!(" {}:{}[{}] ", i + 1, w.name(), w.mnemonic()),
                if active {
                    Style::default()
                        .fg(BG)
                        .bg(AMBER)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(SECONDARY).bg(BG)
                },
            )
        })
        .collect();
    nav_spans.push(Span::styled(" │ ", Style::default().fg(BORDER)));
    if let Some(input) = &app.input {
        nav_spans.push(Span::styled(
            " GO > ",
            Style::default()
                .fg(BG)
                .bg(AMBER)
                .add_modifier(Modifier::BOLD),
        ));
        nav_spans.push(Span::styled(
            format!(" {input}█ "),
            Style::default().fg(AMBER).bg(SELECTION).bold(),
        ));
    } else {
        nav_spans.push(Span::styled(
            " GO > ",
            Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
        ));
        nav_spans.push(Span::styled(
            "Enter command or security (e.g. BTCUSDT GP, AAPL FA, TOP, NEWS CRYPTO, LAUNCHPAD)",
            Style::default().fg(MUTED),
        ));
    }
    frame.render_widget(
        Paragraph::new(Line::from(nav_spans)).style(Style::default().bg(BG)),
        rows[1],
    );

    // Row 2: Workspace-Adaptive Main Research Area
    let main_area = rows[2];
    match app.layout_mode {
        LayoutMode::FullWidth => {
            main_panel(frame, app, main_area);
        }
        LayoutMode::Split => {
            if main_area.width >= 100 {
                let cols =
                    Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)])
                        .split(main_area);
                main_panel(frame, app, cols[0]);
                render_secondary_panel(frame, app, cols[1]);
            } else {
                main_panel(frame, app, main_area);
            }
        }
        LayoutMode::Quad => {
            if main_area.width >= 100 && main_area.height >= 16 {
                let v_split =
                    Layout::vertical([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(main_area);
                let top_cols =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(v_split[0]);
                let bot_cols =
                    Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                        .split(v_split[1]);
                render_workspace_pane(frame, app, Workspace::Market, top_cols[0]);
                render_workspace_pane(frame, app, Workspace::Chart, top_cols[1]);
                render_workspace_pane(frame, app, Workspace::News, bot_cols[0]);
                let fourth = if app.company.is_some() {
                    Workspace::Fin
                } else if !app.macro_data.is_empty() {
                    Workspace::Econ
                } else {
                    Workspace::Port
                };
                render_workspace_pane(frame, app, fourth, bot_cols[1]);
            } else {
                main_panel(frame, app, main_area);
            }
        }
        LayoutMode::Standard => {
            let is_news = app.workspace == Workspace::News;
            let collapse_context = is_news && main_area.width < 165;
            let show_w = app.show_watchlist && main_area.width >= 96;
            let show_c = app.show_context && main_area.width >= 116 && !collapse_context;

            if (show_w || show_c) && !app.full_width {
                let base_left = 22i16 + app.panel_width_offset;
                let base_right = 30i16 + app.panel_width_offset;
                let left_w = (base_left.max(16) as u16).min(36);
                let right_w = (base_right.max(20) as u16).min(44);

                let mut constraints = Vec::new();
                if show_w {
                    constraints.push(Constraint::Length(left_w));
                }
                constraints.push(Constraint::Min(40));
                if show_c {
                    constraints.push(Constraint::Length(right_w));
                }
                let columns = Layout::horizontal(constraints).split(main_area);

                let mut col_idx = 0;
                if show_w {
                    watch(frame, app, columns[col_idx]);
                    col_idx += 1;
                }
                main_panel(frame, app, columns[col_idx]);
                col_idx += 1;
                if show_c {
                    context(frame, app, columns[col_idx]);
                }
            } else if app.focus == 0 && !app.full_width {
                watch(frame, app, main_area);
            } else if app.focus == 2 && !app.full_width {
                context(frame, app, main_area);
            } else {
                main_panel(frame, app, main_area);
            }
        }
    }

    // Row 3: Status Line + Context-Sensitive Shortcut Footer
    let shortcuts = match app.workspace {
        Workspace::Market => {
            "/ Search  : GO>  Tab Focus  Enter Chart  u QuoteCcy  l MinVol  s Sort  o Order  m Mode  w Watch  L Layout  F1 Help"
        }
        Workspace::Chart => {
            "/ Search  : GO>  t Timeframe  i Overlay  g SubInd  x Crosshair  +/- Zoom  h/l Pan  f FollowLive  w Watch  z Full  L Layout"
        }
        Workspace::Fin => {
            "/ Search  : GO>  a Annual  v Quarterly  s/[/] StmtTab  ←/→ Period  j/k Metric  d Profile  Enter/o Filing  L Layout"
        }
        Workspace::News => {
            "/ Search  : GO>  c Category  t TickerFilter  f FormFilter  Enter/r Read  R Refresh  [/] Scroll  o Browser  L Layout"
        }
        Workspace::Scan => {
            "/ Search  : GO>  m Crypto/Eq  u QuoteCcy  l MinVol  x Status  s Sort  o Order  Enter Open  L Layout  F1 Help"
        }
        Workspace::Port => {
            "/ Search  : GO>  j/k Select  Enter Chart  :BUY/:SELL/:DELTX  :ACCOUNT  :IMPORT/:EXPORT  L Layout  F1 Help"
        }
        Workspace::Econ => {
            "/ Search  : GO>  j/k Tenor  :YCRV (Treasury)  :WB US NY.GDP.MKTP.CD  :FRED <SERIES>  L Layout  F1 Help"
        }
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(" STATUS ", Style::default().fg(AMBER).bold()),
                Span::styled(&app.message, Style::default().fg(FG)),
            ]),
            Line::from(Span::styled(
                format!(" {shortcuts}"),
                Style::default().fg(SECONDARY).bg(ELEVATED_BG),
            )),
        ]),
        rows[3],
    );

    if let Some(input) = &app.input {
        let popup = Rect::new(
            area.x + 2,
            area.y + 2,
            area.width.saturating_sub(4).min(78),
            12.min(area.height.saturating_sub(3)),
        );
        frame.render_widget(Clear, popup);
        let sugs = command::suggestions(input, &app.catalog);
        let mut lines = vec![Line::from(vec![
            Span::styled(" GO > ", Style::default().fg(BG).bg(AMBER).bold()),
            Span::styled(format!(" {input}█"), Style::default().fg(AMBER).bold()),
        ])];
        lines.push(Line::from(Span::styled(
            " ── Suggestions (Tab to complete, Up/Down for history, Enter to execute) ──",
            Style::default().fg(MUTED),
        )));
        for (idx, s) in sugs.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(
                    if idx == 0 { " ▶ " } else { "   " },
                    Style::default().fg(AMBER),
                ),
                Span::styled(s.clone(), Style::default().fg(FG)),
            ]));
        }
        frame.render_widget(
            Paragraph::new(lines)
                .block(block(" Command & Instrument Search (<GO>) ", true))
                .style(Style::default().bg(ELEVATED_BG).fg(FG)),
            popup,
        );
    }
    if app.diagnostics {
        let popup = Rect::new(
            area.x + 4,
            area.y + 3,
            area.width.saturating_sub(8).min(74),
            16.min(area.height.saturating_sub(6)),
        );
        frame.render_widget(Clear, popup);
        let (p50, p95, p99) = app.market.ingest_percentiles_us();
        let (dp50, dp95, dp99) = app.draw_percentiles_us();
        let diag_text = format!(
            "Workstation Diagnostics & Runtime Performance (F12 to close)\n\n\
             Terminal Frame: {} cols x {} rows\n\
             Layout Mode:    {} (Watch: {} | Source: {} | Width Offset: {:+})\n\
             Workspace:      {} <{}> | Active Asset: {}\n\
             Follow-Live:    {} | Chart Offset: {} | Zoom: {}\n\
             Candles Cached: {} bars (Chart key: {})\n\
             Quotes Cached:  {} instruments (Trading: {})\n\
             News Stories:   {} loaded (Unread: {})\n\
             Draw Latency:   P50 {:.2}ms  P95 {:.2}ms  P99 {:.2}ms\n\
             Feed Ingest:    P50 {}us  P95 {}us  P99 {}us\n\
             Coalesce Ratio: {} accepted / {} coalesced events\n\
             Persistence:    SQLite WAL mode local ledger (Acct: {})",
            area.width,
            area.height,
            app.layout_mode.label(),
            if app.show_watchlist { "ON" } else { "OFF" },
            if app.show_context { "ON" } else { "OFF" },
            app.panel_width_offset,
            app.workspace.name(),
            app.workspace.mnemonic(),
            app.asset.key(),
            app.follow_live,
            app.offset,
            app.zoom,
            app.market.candles.len(),
            app.market.chart_key,
            app.market.quotes.len(),
            app.market.trading_instrument_count(),
            app.news_items.len(),
            app.news_items.len().saturating_sub(app.news_read_ids.len()),
            dp50 as f64 / 1000.0,
            dp95 as f64 / 1000.0,
            dp99 as f64 / 1000.0,
            p50,
            p95,
            p99,
            app.market.accepted,
            app.market.coalesced,
            app.account
        );
        frame.render_widget(
            Paragraph::new(diag_text)
                .wrap(Wrap { trim: false })
                .block(block(" Telemetry & System Diagnostics (F12) ", true))
                .style(Style::default().bg(ELEVATED_BG).fg(FG)),
            popup,
        );
    }
    if app.help || app.settings {
        let popup = Rect::new(area.x + 2, area.y + 2, area.width - 4, area.height - 4);
        frame.render_widget(Clear, popup);
        let text = if app.settings { SETTINGS_TEXT } else { HELP };
        let title = if app.settings {
            " Workstation Settings & Provider Configuration (Esc to close) "
        } else {
            " Open Market Terminal — Function & Keyboard Reference (Esc to close) "
        };
        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .block(block(title, true))
                .style(Style::default().bg(ELEVATED_BG).fg(FG)),
            popup,
        );
    }
}

fn render_workspace_pane(frame: &mut Frame, app: &App, workspace: Workspace, area: Rect) {
    match workspace {
        Workspace::Chart => views::chart(frame, app, area),
        Workspace::Market => views::market(frame, app, area),
        Workspace::Scan => views::scan(frame, app, area),
        Workspace::Fin => views::financials(frame, app, area),
        Workspace::News => views::news(frame, app, area),
        Workspace::Port => views::portfolio(frame, app, area),
        Workspace::Econ => views::economics(frame, app, area),
    }
}

fn render_secondary_panel(frame: &mut Frame, app: &App, area: Rect) {
    let companion = match app.workspace {
        Workspace::Chart => Workspace::News,
        Workspace::Market => Workspace::Chart,
        Workspace::Fin => Workspace::News,
        Workspace::News => Workspace::Chart,
        Workspace::Scan => Workspace::Chart,
        Workspace::Port => Workspace::Chart,
        Workspace::Econ => Workspace::Chart,
    };
    render_workspace_pane(frame, app, companion, area);
}

fn watch(frame: &mut Frame, app: &App, area: Rect) {
    let split = if area.height >= 14 {
        Layout::vertical([Constraint::Min(6), Constraint::Length(6)]).split(area)
    } else {
        Layout::vertical([Constraint::Min(4), Constraint::Length(0)]).split(area)
    };
    let now = Utc::now();
    let rows = app.watchlist.iter().map(|a| {
        let quote = app.market.quotes.get(&a.key());
        let price = quote
            .map(|q| views::format_price(q.price))
            .unwrap_or_else(|| "—".into());
        let chg_val = quote.and_then(|q| q.effective_change_pct());
        let chg = chg_val
            .map(|v| format!("{v:+.2}%"))
            .unwrap_or_else(|| "--".into());
        let color = theme::direction_color(chg_val);
        Row::new(vec![
            Cell::from(a.symbol.clone()).style(Style::default().fg(if *a == app.asset {
                AMBER
            } else {
                FG
            })),
            Cell::from(price).style(Style::default().fg(FG)),
            Cell::from(chg).style(Style::default().fg(color)),
        ])
    });
    let sel = (!app.watchlist.is_empty())
        .then(|| app.watch_cursor.min(app.watchlist.len().saturating_sub(1)));
    let mut state = TableState::default().with_selected(sel);
    let table = Table::new(
        rows,
        [
            Constraint::Percentage(40),
            Constraint::Percentage(34),
            Constraint::Percentage(26),
        ],
    )
    .header(Row::new(["SYMBOL", "LAST", "24H%"]).style(theme::header_style()))
    .row_highlight_style(theme::row_highlight_style())
    .block(block(
        format!(" Watch: {} ({}) ", app.list_name, app.watchlist.len()),
        app.focus == 0,
    ));
    frame.render_stateful_widget(table, split[0], &mut state);

    if split[1].height > 0 {
        let (p50, p95, p99) = app.market.ingest_percentiles_us();
        let (_, draw_p95, _) = app.draw_percentiles_us();
        let trading = app.market.trading_instrument_count();
        let summary = format!(
            " Universe: {} pairs\n Active:   {} quotes\n Ingest P50/95/99: {}/{}/{}us\n Draw P95: {:.2}ms | Rec: {}",
            if trading > 0 {
                trading
            } else {
                app.market.instruments.len()
            },
            app.market.quotes.len(),
            p50,
            p95,
            p99,
            draw_p95 as f64 / 1000.0,
            now.format("%H:%M:%S")
        );
        frame.render_widget(
            Paragraph::new(summary)
                .style(Style::default().fg(SECONDARY))
                .block(block(" Feed Telemetry ", false)),
            split[1],
        );
    }
}

fn main_panel(frame: &mut Frame, app: &App, area: Rect) {
    views::main(frame, app, area);
}

fn context(frame: &mut Frame, app: &App, area: Rect) {
    let inspect_key = if matches!(app.workspace, Workspace::Market | Workspace::Scan) {
        views::selected_market_instrument(app).unwrap_or_else(|| app.asset.key())
    } else {
        app.asset.key()
    };
    let q = app.market.quotes.get(&inspect_key);
    let info = app.market.instruments.get(&inspect_key);
    let text = if let Some(text) = views::context_text(app) {
        text
    } else if let Some(q) = q {
        let spread = match (q.spread(), q.spread_bps()) {
            (Some(s), Some(bps)) => format!("{} ({bps:.1} bps)", s.normalize()),
            _ => "—".into(),
        };
        let status_line = info
            .map(|i| {
                format!(
                    "{}/{} | {} | Tick 1e-{}",
                    i.base_asset, i.quote_asset, i.status, i.price_precision
                )
            })
            .unwrap_or_else(|| format!("{}/{}", q.base_symbol(), q.currency));
        let skew_line = app
            .market
            .clock_skew_ms
            .map(|ms| format!("Clock skew: +{ms} ms (exchange ahead)"))
            .unwrap_or_else(|| "Clock sync: nominal".into());
        let (p50, p95, p99) = app.market.ingest_percentiles_us();
        format!(
            "{}\n{}\nLast: {} {}\n24h Chg: {} ({})\n\nSource: {}\nCoverage: {}\nStatus: {:?}\nEvent: {}\nReceived: {}\n{}\n\nBid: {}\nAsk: {}\nSpread: {}\n24h VWAP: {}\n24h High: {}\n24h Low: {}\nRange pos: {}\nBase Vol: {}\nQuote Vol: {}\n\nTelemetry:\nIngest P50/95/99: {}/{}/{} us\nAccepted/Coalesced: {}/{}",
            q.asset.key(),
            status_line,
            views::format_price(q.price),
            q.currency,
            q.effective_change_abs()
                .map(views::format_signed_price)
                .unwrap_or_else(|| "—".into()),
            q.effective_change_pct()
                .map(|v| format!("{v:+.2}%"))
                .unwrap_or_else(|| "—".into()),
            q.provider,
            q.coverage,
            q.status_at(Utc::now()),
            q.event_at.format("%H:%M:%S.%3f UTC"),
            q.received_at.format("%H:%M:%S.%3f UTC"),
            skew_line,
            q.bid.map(views::format_price).unwrap_or_else(|| "—".into()),
            q.ask.map(views::format_price).unwrap_or_else(|| "—".into()),
            spread,
            q.vwap
                .map(views::format_price)
                .unwrap_or_else(|| "—".into()),
            q.high
                .map(views::format_price)
                .unwrap_or_else(|| "—".into()),
            q.low.map(views::format_price).unwrap_or_else(|| "—".into()),
            q.range_pct()
                .map(|r| format!("{r:.1}% of 24h range"))
                .unwrap_or_else(|| "—".into()),
            q.volume.map(views::number).unwrap_or_else(|| "—".into()),
            q.quote_volume
                .map(|v| format!("{} {}", views::number(v), q.currency))
                .unwrap_or_else(|| "—".into()),
            p50,
            p95,
            p99,
            app.market.accepted,
            app.market.coalesced,
        )
    } else {
        format!(
            "{}\n\nNo live quote in cache.\n\nActive Providers:\n• Binance Spot: Public exchange-wide discovery & !miniTicker@arr stream\n• SEC EDGAR: Public XBRL company facts & regulatory filings\n• US Treasury / World Bank: Public macro & yield curves\n• Coinbase Exchange: Public crypto pairs\n• Alpaca IEX: Optional (configure in :SETTINGS)\n\n{}",
            inspect_key, app.research_status
        )
    };
    frame.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .block(block(" Instrument & Source Context ", app.focus == 2)),
        area,
    );
}

pub const SETTINGS_TEXT: &str = "Workstation & Data Provider Configuration\n\n\
1. Zero-Key Public Providers (Enabled by Default)\n\
   • Binance Spot: Full symbol discovery (GET /api/v3/exchangeInfo), 24h snapshots (/api/v3/ticker/24hr),\n\
     exchange-wide !miniTicker@arr streaming, historical klines, and selective @bookTicker/@ticker feeds.\n\
   • US Treasury: Daily published par yield curve rates (1M through 30Y maturities).\n\
   • World Bank Indicators: Global macroeconomic time series (e.g. :WB US NY.GDP.MKTP.CD).\n\
   • Coinbase Exchange: Public Spot crypto feeds (e.g. COINBASE:BTC-USD).\n\n\
2. SEC EDGAR Fundamental & Regulatory Filings (Free, User-Agent Required by SEC Fair Access Policy)\n\
   • Environment variable: OMT_SEC_USER_AGENT='YourAppName contact@yourdomain.com'\n\
   • Or set `sec_user_agent` in config.toml.\n\
   • Powers :FIN <TICKER> (FA), :NEWS <TICKER> (CN), and :SCAN EQUITIES (EQS).\n\n\
3. Optional Accredited Providers (Not Required for Core Workstation Operation)\n\
   • Alpaca IEX Equities: Set APCA_API_KEY_ID and APCA_API_SECRET_KEY for real-time IEX stock trades & bars.\n\
   • FRED (St. Louis Fed): Set FRED_API_KEY for :FRED <SERIES_ID> economic series.\n\n\
4. Local Storage & Privacy\n\
   • All watchlists, portfolio transactions, and caches are stored locally in SQLite (WAL mode).\n\
   • Zero telemetry, zero cloud accounts, zero fabricated market data.";

pub const HELP: &str = "Keyboard Navigation & Controls\n\
/ or : or Ctrl+P  Open GO> command bar (Tab autocompletes, Up/Down cycles history)\n\
1-7 or F2-F8      Switch workspaces: 1:MARKET[MKT] 2:CHART[GP] 3:FIN[FA] 4:NEWS[CN] 5:SCAN[EQS] 6:PORT[PRTU] 7:ECON[ECST]\n\
Tab / Shift+Tab   Cycle panel focus (Watchlist / Central Workspace / Context Inspector)\n\
Up/Down or j/k    Move row selection (PgUp/PgDn/Home/End for fast table navigation)\n\
Enter             Open row in CHART / read article in NEWS / open filing in browser\n\
L / z             Cycle layout mode: Standard / Full-Width / Split 2-Pane / Launchpad 2x2\n\
W / I             Toggle left Watchlist panel (W) / right Context Inspector panel (I)\n\
< / >             Adjust side panel width offset (-8 to +16 columns)\n\
u / l / x         In MARKET & SCAN: cycle Quote Currency (u), Min Liquidity (l), Trading Status (x)\n\
s / o             In MARKET & SCAN: cycle Sort column (s), toggle Ascending/Descending order (o)\n\
m                 In MARKET: cycle Overview / Crypto / WEI; In SCAN: toggle SCAN CRYPTO / SCAN EQUITIES\n\
w                 Toggle selected instrument in active Watchlist\n\
t / i / g         In CHART: cycle Timeframe (t), Overlay (i), cycle Sub-Indicator [Vol/RSI/MACD/ATR] (g)\n\
x / [ / ]         In CHART: toggle 2D Crosshair (x), move candle inspection cursor ([ and ])\n\
+/-               In CHART: zoom in (+) or out (-) candle capacity\n\
h/l or Left/Right In CHART: pan historical candles; Home (earliest loaded), End (latest / follow-live)\n\
f                 In CHART: toggle Follow-Live mode (anchor to newest forming candle)\n\
d                 In FIN & CHART: toggle Security Description & Profile (<DES>)\n\
a / v / s / ←/→   In FIN: Annual (a), Quarterly (v), cycle Statement Tab (s), select Period Column (←/→)\n\
c / t / f         In NEWS: cycle Category (c), toggle Ticker Filter (t), cycle SEC Form Filter (f)\n\
r / R / [ / ]     In NEWS: toggle Expanded Reader (r), Refresh news feeds (R), scroll Reader text ([ / ])\n\
F12               Toggle System Diagnostics & Ingest/Draw Telemetry overlay\n\
F1 or ?           Open Help   |   q or Ctrl+C   Safe exit\n\n\
Commands & Bloomberg Mnemonics (case-insensitive, trailing <GO> optional)\n\
MARKET [USDT|USDC|BTC|ALL|GAINERS|LOSERS|ACTIVE]   CRYPTO [ALL|USDT|USDC|BTC]   WEI   MOST\n\
CHART/GP <SYMBOL> [1m|5m|15m|1h|4h|1d]   GIP <SYMBOL>   IND SMA|EMA|BB|NONE\n\
TOP   NEWS [ALL|MARKETS|EQUITIES|CRYPTO|MACRO|EARNINGS|SEC]   CN [TICKER]   FILINGS [TICKER]\n\
FIN/FA <TICKER> [OVERVIEW|INCOME|BALANCE|CASH|RATIOS]   DES <SYMBOL>\n\
SCAN CRYPTO   SCAN EQUITIES / EQS   FILTER <expr|CLEAR>   QUOTE <CCY|ALL>   MINVOL <N>   SORT <FIELD> [ASC|DESC]\n\
LAUNCHPAD   LAYOUT STANDARD|FULL|SPLIT|QUAD|WATCH|SOURCE\n\
WATCH ADD <SYMBOL>   WATCH REMOVE <SYMBOL>   WATCHLIST <NAME>   INDEX <TICKER>\n\
BUY <SYM> <QTY> <PRICE> [FEE] [TS]   SELL <SYM> <QTY> <PRICE> [FEE] [TS]   DELTX <ID>\n\
ACCOUNT <NAME>   IMPORT <CSV>   EXPORT <CSV>   TREASURY/YCRV   WB <CTRY> <SERIES>   FRED <SERIES>   SETTINGS";

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn navigation_and_completion() {
        let mut app = App::default();
        app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::SHIFT));
        assert_eq!(app.focus, 0);
        app.key(KeyEvent::new(KeyCode::Char('7'), KeyModifiers::NONE));
        assert_eq!(app.workspace, Workspace::Econ);
        app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        for c in "ETH".chars() {
            app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.input.as_deref(), Some("ETHUSDT"));
        assert_eq!(
            app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some("ETHUSDT".into())
        );
        app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(app.quit);
    }
    #[test]
    fn command_history_and_function_keys() {
        let mut app = App::default();
        assert_eq!(
            app.key(KeyEvent::new(KeyCode::F(3), KeyModifiers::NONE)),
            Some("CHART".into())
        );
        assert_eq!(app.workspace, Workspace::Chart);
        app.key(KeyEvent::new(KeyCode::Char('i'), KeyModifiers::NONE));
        assert_eq!(app.overlay, OverlayMode::Ema);
        app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
        for c in "YCRV".chars() {
            app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(
            app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            Some("YCRV".into())
        );
        app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
        app.key(KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(app.input.as_deref(), Some("YCRV"));
    }
    #[test]
    fn all_workspaces_and_ascii_render_without_data() {
        for workspace in Workspace::ALL {
            let app = App {
                workspace,
                ascii: true,
                ..Default::default()
            };
            for (w, h) in [(140, 40), (40, 12)] {
                let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
                t.draw(|f| render(f, &app)).unwrap();
                assert!(
                    t.backend()
                        .buffer()
                        .content
                        .iter()
                        .all(|c| c.symbol().is_ascii())
                );
            }
        }
    }
    use ratatui::backend::TestBackend;
    #[test]
    fn input_does_not_quit() {
        let mut a = App {
            input: Some(String::new()),
            ..Default::default()
        };
        a.key(KeyEvent::new(KeyCode::Char('q'), KeyModifiers::NONE));
        assert!(!a.quit);
        assert_eq!(a.input.as_deref(), Some("q"));
    }
    #[test]
    fn responsive_render() {
        for (w, h) in [(140, 40), (80, 24), (28, 9), (12, 4)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| render(f, &App::default())).unwrap();
            assert!(
                t.backend()
                    .buffer()
                    .content
                    .iter()
                    .any(|c| c.symbol() != " ")
            );
        }
    }
    #[test]
    fn virtualized_thousands_of_rows_filtering_sorting_and_stable_selection() {
        let mut app = App::default();
        let now = Utc::now();
        let mut instruments = Vec::with_capacity(3500);
        for i in 0..3500 {
            let quote_ccy = match i % 4 {
                0 => "USDT",
                1 => "USDC",
                2 => "BTC",
                _ => "FDUSD",
            };
            let base = format!("COIN{i:04}");
            let symbol = format!("{base}{quote_ccy}");
            let asset: AssetId = format!("BINANCE:{symbol}").parse().unwrap();
            let status = if i % 50 == 0 && i > 0 {
                omt_core::TradingStatus::Break
            } else {
                omt_core::TradingStatus::Trading
            };
            instruments.push(omt_core::InstrumentInfo {
                asset: asset.clone(),
                base_asset: base.clone(),
                quote_asset: quote_ccy.into(),
                status,
                price_precision: 4,
                quantity_precision: 2,
                provider: "Binance Spot".into(),
                coverage: "Binance exchange only".into(),
            });
            app.market.ingest(omt_core::Quote {
                asset,
                price: omt_core::Decimal::from(10 + (i % 500)),
                currency: quote_ccy.into(),
                change_pct: Some(omt_core::Decimal::from((i as i64 % 41) - 20)),
                change_abs: Some(omt_core::Decimal::from((i as i64 % 11) - 5)),
                open_24h: Some(omt_core::Decimal::from(10)),
                high: Some(omt_core::Decimal::from(600)),
                low: Some(omt_core::Decimal::from(5)),
                volume: Some(omt_core::Decimal::from(1000 + i as i64)),
                quote_volume: Some(omt_core::Decimal::from((i as i64 + 1) * 50_000)),
                bid: None,
                ask: None,
                vwap: None,
                base_asset: Some(base),
                provider: "Binance Spot".into(),
                coverage: "Binance exchange only".into(),
                event_at: now,
                received_at: now,
                status: omt_core::DataStatus::Live,
                delay_seconds: Some(0),
            });
        }
        app.market.register_instruments(instruments);
        assert_eq!(app.market.instruments.len(), 3500);
        assert_eq!(app.market.quotes.len(), 3500);

        // Default quote filter is USDT and status_filter_trading_only is true
        let usdt_keys = views::filtered_market_keys(&app);
        assert!(usdt_keys.len() > 800 && usdt_keys.len() < 900);
        assert!(usdt_keys.iter().all(|k| k.ends_with("USDT")));

        // Move keyboard selection to row 5 and record selected_market_key
        for _ in 0..5 {
            app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        }
        let chosen_key = app.selected_market_key.clone().unwrap();
        assert_eq!(
            views::selected_market_instrument(&app).as_deref(),
            Some(chosen_key.as_str())
        );

        // Toggle sort order ('o'): cursor must reposition to keep chosen_key selected
        app.key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));
        assert_eq!(
            views::selected_market_instrument(&app).as_deref(),
            Some(chosen_key.as_str())
        );

        // Cycle quote filter to USDC ('u') and then BTC ('u') and ALL
        app.key(KeyEvent::new(KeyCode::Char('u'), KeyModifiers::NONE));
        assert_eq!(app.quote_filter, "USDC");
        assert!(
            views::filtered_market_keys(&app)
                .iter()
                .all(|k| k.ends_with("USDC"))
        );

        app.quote_filter = "ALL".into();
        let all_trading = views::filtered_market_keys(&app).len();
        app.key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE)); // include BREAK/HALT
        let all_statuses = views::filtered_market_keys(&app).len();
        assert_eq!(all_statuses, 3500);
        assert!(all_statuses > all_trading);

        // Render 3,500-instrument table in both Market and Scan workspaces via virtualized Table
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        app.workspace = Workspace::Scan;
        t.draw(|f| render(f, &app)).unwrap();
    }
    #[test]
    fn numeric_formatting_and_wei_honesty() {
        use std::str::FromStr;
        assert_eq!(
            views::format_price(omt_core::Decimal::from_str("68425.5").unwrap()),
            "68,425.50"
        );
        assert_eq!(
            views::format_price(omt_core::Decimal::from_str("0.00001234").unwrap()),
            "0.00001234"
        );
        assert_eq!(
            views::format_signed_price(omt_core::Decimal::from_str("-125.4").unwrap()),
            "-125.40"
        );
        assert_eq!(
            views::number(omt_core::Decimal::from(2_500_000_000i64)),
            "2.50B"
        );

        let mut app = App {
            workspace: Workspace::Market,
            market_mode: MarketMode::Wei,
            ..Default::default()
        };
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("WORLD EQUITY INDICES"));
        assert!(text.contains("Proprietary index levels"));

        // Verify full-width toggle ('z')
        app.key(KeyEvent::new(KeyCode::Char('z'), KeyModifiers::NONE));
        assert!(app.full_width);
        t.draw(|f| render(f, &app)).unwrap();
    }
    #[test]
    fn chart_dynamic_capacity_and_right_anchoring() {
        use chrono::{Duration, Utc};
        use omt_core::{Candle, Decimal};

        let now = Utc::now();
        let mut candles = Vec::new();
        for i in 0..120 {
            let t = now - Duration::hours(120 - i);
            let p = Decimal::from(65000 + i * 10);
            candles.push(Candle {
                open_at: t,
                open: p,
                high: p + Decimal::from(50),
                low: p - Decimal::from(50),
                close: p + Decimal::from(20),
                volume: Decimal::from(100),
                closed: true,
            });
        }

        let mut app = App {
            workspace: Workspace::Chart,
            ..Default::default()
        };
        app.market.candles = candles;

        // Render on a 180x45 wide terminal
        let mut t180 = Terminal::new(TestBackend::new(180, 45)).unwrap();
        t180.draw(|f| render(f, &app)).unwrap();
        let text180: String = t180
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text180.contains("FOLLOW-LIVE"));
        assert!(!text180.contains("[LIMITED HISTORY"));

        // Limited history state: when only 15 candles are present
        app.market.candles.truncate(15);
        t180.draw(|f| render(f, &app)).unwrap();
        let text_lim: String = t180
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_lim.contains("[LIMITED HISTORY: 15 bars available]"));

        // Test crosshair inspection cursor
        app.candle_cursor = Some(0);
        t180.draw(|f| render(f, &app)).unwrap();
        let text_cross: String = t180
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_cross.contains("INSPECT"));

        // Cycle sub-indicators: Volume -> Rsi -> Macd -> Atr
        assert_eq!(app.sub_indicator, SubIndicator::Volume);
        app.key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(app.sub_indicator, SubIndicator::Rsi);
        app.key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(app.sub_indicator, SubIndicator::Macd);
        app.key(KeyEvent::new(KeyCode::Char('g'), KeyModifiers::NONE));
        assert_eq!(app.sub_indicator, SubIndicator::Atr);
        t180.draw(|f| render(f, &app)).unwrap();
    }
    #[test]
    fn workstation_adaptive_layouts_across_dimensions() {
        let mut app = App::default();
        let sizes = [(80, 24), (120, 30), (180, 45), (240, 60)];
        let modes = [
            LayoutMode::Standard,
            LayoutMode::FullWidth,
            LayoutMode::Split,
            LayoutMode::Quad,
        ];

        for &(w, h) in &sizes {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            for &mode in &modes {
                app.layout_mode = mode;
                app.full_width = mode == LayoutMode::FullWidth;
                for &ws in &Workspace::ALL {
                    app.workspace = ws;
                    t.draw(|f| render(f, &app)).unwrap();
                }
            }
        }
    }
    #[test]
    fn news_workspace_split_table_and_reader() {
        use chrono::Utc;
        use omt_core::{NewsCategory, NewsItem};

        let now = Utc::now();
        let items = vec![
            NewsItem {
                id: "item-0".into(),
                published_at: now + chrono::Duration::hours(1),
                fetched_at: now,
                source: "Reuters".into(),
                ticker: None,
                category: NewsCategory::Markets,
                headline: "Global Equity Benchmarks Rally on Easing Inflation Prints".into(),
                summary: "Broad indices advanced across European and Asian trading sessions.".into(),
                body: None,
                url: "https://reuters.com/markets-rally".into(),
                is_sec_filing: false,
                form: None,
                accession: None,
                report_date: None,
            },
            NewsItem {
                id: "item-1".into(),
                published_at: now,
                fetched_at: now,
                source: "CNBC".into(),
                ticker: Some("AAPL".into()),
                category: NewsCategory::Equities,
                headline: "Apple Unveils Next-Generation Silicon in Cupertino Event".into(),
                summary: "Apple announced new M-series architecture with improved neural engine."
                    .into(),
                body: Some(
                    "Full article body text detailing the product specifications and market reaction."
                        .into(),
                ),
                url: "https://cnbc.com/apple-event".into(),
                is_sec_filing: false,
                form: None,
                accession: None,
                report_date: None,
            },
            NewsItem {
                id: "item-2".into(),
                published_at: now - chrono::Duration::hours(2),
                fetched_at: now,
                source: "SEC EDGAR".into(),
                ticker: Some("NVDA".into()),
                category: NewsCategory::Filings,
                headline: "NVIDIA Corp - Form 10-Q Quarterly Report".into(),
                summary: "Quarterly period ended July 2026 reporting strong data center growth."
                    .into(),
                body: None,
                url: "https://www.sec.gov/Archives/edgar/data/1045810/sample.htm".into(),
                is_sec_filing: true,
                form: Some("10-Q".into()),
                accession: Some("0001045810-26-000050".into()),
                report_date: None,
            },
            NewsItem {
                id: "item-3".into(),
                published_at: now - chrono::Duration::hours(4),
                fetched_at: now,
                source: "CoinDesk".into(),
                ticker: Some("BTCUSDT".into()),
                category: NewsCategory::Crypto,
                headline: "Bitcoin Hashrate Touches Fresh Record Amid Network Expansion".into(),
                summary: "Mining metrics indicate expanding institutional infrastructure deployment."
                    .into(),
                body: None,
                url: "https://coindesk.com/btc-hashrate".into(),
                is_sec_filing: false,
                form: None,
                accession: None,
                report_date: None,
            },
        ];

        let mut app = App {
            workspace: Workspace::News,
            news_items: items,
            ..Default::default()
        };

        // Render split view on 140x40
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text_split: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_split.contains("Apple Unveils"));
        assert!(text_split.contains("NVIDIA Corp"));

        // Select item-1 (Apple) so full article body and headline appear in reader pane
        app.cursor = 1;
        t.draw(|f| render(f, &app)).unwrap();
        let text_sel: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_sel.contains("Full article body text"));
        assert!(text_sel.contains("Apple Unveils Next-Generation"));

        // Test category filtering ('c')
        app.key(KeyEvent::new(KeyCode::Char('c'), KeyModifiers::NONE));
        let filtered = views::filtered_news_items(&app);
        assert!(!filtered.is_empty());

        // Test ticker filtering ('t')
        app.news_category = NewsCategory::All;
        app.news_ticker_filter = Some("AAPL".into());
        let aapl_news = views::filtered_news_items(&app);
        assert_eq!(aapl_news.len(), 1);
        assert_eq!(aapl_news[0].id, "item-1");

        // Test expanded reader view ('r')
        app.key(KeyEvent::new(KeyCode::Char('r'), KeyModifiers::NONE));
        assert!(app.news_reader_expanded);
        t.draw(|f| render(f, &app)).unwrap();
        let text_reader: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_reader.contains("EXPANDED READER"));
    }
    #[test]
    fn security_description_des_tearsheet() {
        let mut app = App {
            workspace: Workspace::Chart,
            des_mode: true,
            ..Default::default()
        };

        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("SECURITY DESCRIPTION"));
        assert!(text.contains("Crypto Spot"));

        // In Fin workspace
        app.workspace = Workspace::Fin;
        t.draw(|f| render(f, &app)).unwrap();
        let text_fin: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_fin.contains("SECURITY DESCRIPTION"));
    }
}
