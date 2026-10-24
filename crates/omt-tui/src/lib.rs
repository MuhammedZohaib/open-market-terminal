#![forbid(unsafe_code)]
pub mod theme;
pub mod views;
use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use omt_core::{
    AssetId, ChartDataStatus, Company, MacroObservation, NewsCategory, NewsItem, PeriodView,
    Timeframe, Venue,
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
    pub input_sug_cursor: usize,
    pub input_sug_navigated: bool,
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
    pub global_asset: AssetId,
    pub econ_axis_categorical: bool,
    pub quad_focus: usize,
    pub news_cursor: usize,
    pub fin_cursor: usize,
    pub econ_cursor: usize,
    pub port_cursor: usize,
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
            global_asset: default_crypto.clone(),
            econ_axis_categorical: true,
            quad_focus: 0,
            news_cursor: 0,
            fin_cursor: 0,
            econ_cursor: 0,
            port_cursor: 0,
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
            input_sug_cursor: 0,
            input_sug_navigated: false,
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
    pub fn visible_focus_targets(&self) -> Vec<usize> {
        if self.full_width || self.layout_mode == LayoutMode::FullWidth {
            return vec![1];
        }
        let mut targets = Vec::with_capacity(3);
        if self.show_watchlist {
            targets.push(0);
        }
        targets.push(1);
        if self.show_context {
            targets.push(2);
        }
        targets
    }
    pub fn next_focus(&mut self) {
        let targets = self.visible_focus_targets();
        if targets.len() <= 1 {
            self.focus = 1;
            return;
        }
        let current_pos = targets.iter().position(|&p| p == self.focus).unwrap_or(0);
        self.focus = targets[(current_pos + 1) % targets.len()];
    }
    pub fn prev_focus(&mut self) {
        let targets = self.visible_focus_targets();
        if targets.len() <= 1 {
            self.focus = 1;
            return;
        }
        let current_pos = targets.iter().position(|&p| p == self.focus).unwrap_or(0);
        self.focus = targets[(current_pos + targets.len() - 1) % targets.len()];
    }
    pub fn focus_label(&self) -> &'static str {
        match self.focus {
            0 => "WATCHLIST",
            1 => "MAIN WORKSPACE",
            2 => "CONTEXT INSPECTOR",
            _ => "MAIN WORKSPACE",
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
                    self.input_sug_cursor = 0;
                    self.input_sug_navigated = false;
                }
                KeyCode::Enter => {
                    let sugs = get_palette_suggestions(input, &self.catalog);
                    let final_cmd = if self.input_sug_navigated && !sugs.is_empty() {
                        let idx = self.input_sug_cursor.min(sugs.len() - 1);
                        sugs[idx].clone()
                    } else {
                        input.clone()
                    };
                    self.input = None;
                    self.history_index = None;
                    self.input_sug_cursor = 0;
                    self.input_sug_navigated = false;
                    let trimmed = final_cmd.trim();
                    if !trimmed.is_empty()
                        && self.history.last().map(String::as_str) != Some(trimmed)
                    {
                        self.history.push(trimmed.to_string());
                        if self.history.len() > 50 {
                            self.history.remove(0);
                        }
                    }
                    return Some(final_cmd);
                }
                KeyCode::Backspace => {
                    input.pop();
                    self.input_sug_cursor = 0;
                    self.input_sug_navigated = false;
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    input.clear();
                    self.input_sug_cursor = 0;
                    self.input_sug_navigated = false;
                }
                KeyCode::Up => {
                    let sugs = get_palette_suggestions(input, &self.catalog);
                    if self.input_sug_navigated && !sugs.is_empty() {
                        if self.input_sug_cursor > 0 {
                            self.input_sug_cursor -= 1;
                        } else {
                            self.input_sug_navigated = false;
                        }
                    } else if !self.history.is_empty() {
                        let idx = match self.history_index {
                            Some(i) => i.saturating_sub(1),
                            None => self.history.len() - 1,
                        };
                        self.history_index = Some(idx);
                        *input = self.history[idx].clone();
                        self.input_sug_cursor = 0;
                        self.input_sug_navigated = false;
                    }
                }
                KeyCode::Down => {
                    let sugs = get_palette_suggestions(input, &self.catalog);
                    if !sugs.is_empty() {
                        if !self.input_sug_navigated {
                            self.input_sug_navigated = true;
                            self.input_sug_cursor = 0;
                        } else {
                            self.input_sug_cursor = (self.input_sug_cursor + 1).min(sugs.len() - 1);
                        }
                    } else if let Some(i) = self.history_index {
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
                    let sugs = get_palette_suggestions(input, &self.catalog);
                    if !sugs.is_empty() {
                        let idx = self.input_sug_cursor.min(sugs.len() - 1);
                        *input = sugs[idx].clone();
                        self.input_sug_cursor = 0;
                        self.input_sug_navigated = false;
                    }
                }
                KeyCode::Char(c) => {
                    input.push(c);
                    self.input_sug_cursor = 0;
                    self.input_sug_navigated = false;
                }
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
                self.input_sug_cursor = 0;
                self.input_sug_navigated = false;
            }
            KeyCode::Char('p') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.input = Some(String::new());
                self.history_index = None;
                self.input_sug_cursor = 0;
                self.input_sug_navigated = false;
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
            KeyCode::Tab => {
                if self.layout_mode == LayoutMode::Quad {
                    self.quad_focus = (self.quad_focus + 1) % 4;
                    self.message = format!("Launchpad: Focused Pane {}/4", self.quad_focus + 1);
                } else {
                    self.next_focus();
                }
            }
            KeyCode::BackTab => {
                if self.layout_mode == LayoutMode::Quad {
                    self.quad_focus = (self.quad_focus + 3) % 4;
                    self.message = format!("Launchpad: Focused Pane {}/4", self.quad_focus + 1);
                } else {
                    self.prev_focus();
                }
            }
            KeyCode::Char('z') => {
                self.full_width = !self.full_width;
                self.layout_mode = if self.full_width {
                    LayoutMode::FullWidth
                } else {
                    LayoutMode::Standard
                };
                if self.full_width {
                    self.focus = 1;
                }
                self.message = if self.full_width {
                    "Full-width central workspace enabled (press z to restore panels, L to cycle layouts)".into()
                } else {
                    "Multi-panel workstation layout restored".into()
                };
            }
            KeyCode::Char('L') => {
                self.layout_mode = self.layout_mode.next();
                self.full_width = self.layout_mode == LayoutMode::FullWidth;
                if self.full_width {
                    self.focus = 1;
                }
                self.message = format!("Workstation layout mode: {}", self.layout_mode.label());
            }
            KeyCode::Char('W') => {
                self.show_watchlist = !self.show_watchlist;
                if !self.show_watchlist && self.focus == 0 {
                    self.focus = 1;
                }
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
                if !self.show_context && self.focus == 2 {
                    self.focus = 1;
                }
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
                if self.layout_mode == LayoutMode::Quad {
                    let cmd = match self.quad_focus {
                        0 => "MARKET".into(),
                        1 => format!("CHART {}", self.asset.key()),
                        2 => "NEWS".into(),
                        _ => {
                            if self.company.is_some() {
                                "FIN".into()
                            } else if !self.macro_data.is_empty() {
                                "ECON".into()
                            } else {
                                "PORT".into()
                            }
                        }
                    };
                    self.layout_mode = LayoutMode::Standard;
                    return Some(cmd);
                }
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
            KeyCode::Char('R')
                if matches!(
                    self.workspace,
                    Workspace::News | Workspace::Chart | Workspace::Fin
                ) =>
            {
                return Some("REFRESH".into());
            }
            KeyCode::Char('m') if self.workspace == Workspace::Econ => {
                self.econ_axis_categorical = !self.econ_axis_categorical;
                self.message = if self.econ_axis_categorical {
                    "Yield curve: Categorical Tenor Spacing active (press 'm' for proportional months)"
                        .into()
                } else {
                    "Yield curve: Proportional Maturity Axis active (press 'm' for categorical tenors)"
                        .into()
                };
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
    let border_style = if active {
        Style::default()
            .fg(theme::AMBER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme::INACTIVE_BORDER)
    };
    let border_type = if active {
        BorderType::Thick
    } else {
        BorderType::Plain
    };
    Block::bordered()
        .border_type(border_type)
        .title(title)
        .border_style(border_style)
        .style(Style::default().bg(PANEL_BG).fg(FG))
}

pub fn get_palette_suggestions(input: &str, catalog: &[String]) -> Vec<String> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return vec![
            "TOP".into(),
            "BTCUSDT GP".into(),
            "AAPL FA".into(),
            "ETHUSDT GP".into(),
            "MARKET".into(),
            "WEI".into(),
            "SCAN".into(),
            "NEWS CRYPTO".into(),
            "PORT".into(),
            "YCRV".into(),
        ];
    }
    let query = trimmed.to_ascii_uppercase();
    let mut exact_or_prefix = Vec::new();
    let mut contains = Vec::new();

    for cmd in command::COMMANDS {
        let u = cmd.to_ascii_uppercase();
        if u.starts_with(&query) {
            exact_or_prefix.push(cmd.to_string());
        } else if u.contains(&query) {
            contains.push(cmd.to_string());
        }
    }
    for sym in catalog {
        let u = sym.to_ascii_uppercase();
        if u.starts_with(&query) {
            exact_or_prefix.push(sym.clone());
        } else if u.contains(&query) {
            contains.push(sym.clone());
        }
    }

    exact_or_prefix.extend(contains);
    exact_or_prefix.dedup();
    exact_or_prefix.truncate(10);
    exact_or_prefix
}

pub fn suggestion_metadata(item: &str) -> (&'static str, &'static str, Color) {
    let upper = item.to_ascii_uppercase();
    if upper == "TOP" {
        (
            "NEWS",
            "Top Market News & Major Headlines",
            theme::INFO_BLUE,
        )
    } else if upper == "WEI" {
        ("INDICES", "World Equity Benchmark Indices", theme::AMBER)
    } else if upper.starts_with("MARKET") || upper.starts_with("CRYPTO") || upper == "MOST" {
        ("MARKET", "Spot Market Universe & Overview", theme::AMBER)
    } else if upper.starts_with("CHART") || upper.ends_with(" GP") || upper.contains(" GP ") {
        ("CHART", "High-Resolution Candlestick Graph", theme::CYAN)
    } else if upper.starts_with("FIN")
        || upper.ends_with(" FA")
        || upper.contains(" FA ")
        || upper == "FILINGS"
        || upper == "CF"
        || upper == "DES"
    {
        (
            "FINANCIALS",
            "SEC EDGAR Financial Facts & Filings",
            theme::POSITIVE,
        )
    } else if upper.starts_with("NEWS") {
        (
            "NEWS",
            "Live Multi-Source Financial News Feeds",
            theme::INFO_BLUE,
        )
    } else if upper.starts_with("SCAN") || upper == "EQS" {
        (
            "SCREENER",
            "Crypto & Equity Multi-Metric Screener",
            theme::AMBER,
        )
    } else if upper.starts_with("PORT")
        || upper.starts_with("BUY")
        || upper.starts_with("SELL")
        || upper == "ACCOUNT"
    {
        ("PORTFOLIO", "Local Portfolio Ledger & Orders", theme::CYAN)
    } else if upper.starts_with("ECON")
        || upper == "YCRV"
        || upper.starts_with("WB")
        || upper.starts_with("FRED")
    {
        (
            "MACRO",
            "Sovereign Yield Curves & Macro Observables",
            theme::VIOLET,
        )
    } else if upper.ends_with("USDT")
        || upper.ends_with("USDC")
        || upper.ends_with("BTC")
        || upper.ends_with("FDUSD")
        || upper.ends_with("EUR")
        || upper.contains("COINBASE:")
    {
        ("CRYPTO", "Binance Spot Trading Instrument", theme::CYAN)
    } else if upper.chars().all(|c| c.is_ascii_alphabetic()) && upper.len() <= 5 {
        ("EQUITY", "US Equities & SEC Edgar Filer", theme::POSITIVE)
    } else {
        ("COMMAND", "Terminal Command / Security", theme::SECONDARY)
    }
}

fn shortcut_keycaps_for_workspace(workspace: Workspace) -> Vec<(&'static str, &'static str)> {
    match workspace {
        Workspace::Market => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("Enter", "Chart"),
            ("u", "Quote"),
            ("l", "MinVol"),
            ("s", "Sort"),
            ("o", "Order"),
            ("m", "Mode"),
            ("w", "Watch"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
        Workspace::Chart => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("t", "Timeframe"),
            ("i", "Overlay"),
            ("g", "SubInd"),
            ("x", "Crosshair"),
            ("+/-", "Zoom"),
            ("h/l", "Pan"),
            ("f", "FollowLive"),
            ("w", "Watch"),
            ("z", "Full"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
        Workspace::Fin => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("a", "Annual"),
            ("v", "Quarterly"),
            ("s/[/]", "StmtTab"),
            ("←/→", "Period"),
            ("j/k", "Metric"),
            ("d", "Profile"),
            ("Enter", "Filing"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
        Workspace::News => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("c", "Category"),
            ("t", "Ticker"),
            ("f", "FormFilter"),
            ("Enter/r", "Read"),
            ("R", "Refresh"),
            ("[/]", "Scroll"),
            ("o", "Browser"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
        Workspace::Scan => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("m", "Crypto/Eq"),
            ("u", "Quote"),
            ("l", "MinVol"),
            ("x", "Status"),
            ("s", "Sort"),
            ("o", "Order"),
            ("Enter", "Open"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
        Workspace::Port => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("j/k", "Select"),
            ("Enter", "Chart"),
            (":BUY/:SELL", "Orders"),
            (":ACCOUNT", "Acct"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
        Workspace::Econ => vec![
            ("/", "Search"),
            (":", "GO>"),
            ("Tab", "Focus"),
            ("j/k", "Tenor"),
            (":YCRV", "Treasury"),
            (":WB", "WorldBank"),
            (":FRED", "Fred"),
            ("L", "Layout"),
            ("F1", "Help"),
        ],
    }
}

fn render_shortcut_footer_line(workspace: Workspace, available_width: u16) -> Line<'static> {
    let pairs = shortcut_keycaps_for_workspace(workspace);
    let mut spans = Vec::new();
    let mut current_len = 0;

    for (key, label) in pairs {
        let piece_len = 2 + key.len() + 2 + label.len() + 2;
        if current_len + piece_len > available_width as usize && !spans.is_empty() {
            break;
        }
        spans.push(Span::styled(
            " [",
            Style::default().fg(theme::INACTIVE_BORDER),
        ));
        spans.push(Span::styled(key, theme::keycap_style()));
        spans.push(Span::styled(
            "] ",
            Style::default().fg(theme::INACTIVE_BORDER),
        ));
        spans.push(Span::styled(label, theme::keycap_label_style()));
        spans.push(Span::raw("  "));
        current_len += piece_len;
    }
    Line::from(spans)
}

fn render_centered_command_palette(frame: &mut Frame, app: &App, area: Rect, input: &str) {
    let max_w = 84u16.min(area.width);
    let popup_w = if area.width <= 40 {
        area.width.saturating_sub(2).max(10)
    } else {
        (area.width.saturating_sub(4)).min(max_w)
    };
    let max_h = 16u16.min(area.height);
    let popup_h = if area.height <= 10 {
        area.height.saturating_sub(2).max(6)
    } else {
        (area.height.saturating_sub(2)).min(max_h)
    };
    let popup_x = area.x + (area.width.saturating_sub(popup_w)) / 2;
    let popup_y = area.y + (area.height.saturating_sub(popup_h)) / 3;
    let popup = Rect::new(popup_x, popup_y, popup_w, popup_h);

    frame.render_widget(Clear, popup);

    let sugs = get_palette_suggestions(input, &app.catalog);
    let count_text = if input.trim().is_empty() {
        "Default Shortcuts".to_string()
    } else {
        format!("{} results", sugs.len())
    };

    let outer_block = Block::bordered()
        .border_type(BorderType::Double)
        .border_style(
            Style::default()
                .fg(theme::AMBER)
                .add_modifier(Modifier::BOLD),
        )
        .style(Style::default().bg(theme::ELEVATED_BG).fg(theme::FG))
        .title(Line::from(vec![
            Span::styled(
                " ❯_ ",
                Style::default().fg(theme::BG).bg(theme::AMBER).bold(),
            ),
            Span::styled(
                " COMMAND RUN BAR & SYMBOL SEARCH ",
                Style::default().fg(theme::AMBER).bold(),
            ),
            Span::styled(
                format!("({count_text}) "),
                Style::default().fg(theme::MUTED),
            ),
        ]))
        .title_alignment(Alignment::Left)
        .title(Line::from(Span::styled(
            " [Esc] Dismiss ",
            Style::default().fg(theme::SECONDARY),
        )));

    let inner = outer_block.inner(popup);
    frame.render_widget(outer_block, popup);

    if inner.height < 4 || inner.width < 20 {
        return;
    }

    let chunks = Layout::vertical([
        Constraint::Length(1),
        Constraint::Length(1),
        Constraint::Min(2),
        Constraint::Length(1),
    ])
    .split(inner);

    let prompt_spans = vec![
        Span::styled(" ❯ ", Style::default().fg(theme::CYAN).bold()),
        Span::styled(input.to_string(), Style::default().fg(theme::FG).bold()),
        Span::styled("█", Style::default().fg(theme::AMBER)),
        Span::raw(" "),
        if input.trim().is_empty() {
            Span::styled(
                "Type command (GP, FA, TOP, YCRV) or symbol (BTCUSDT, AAPL)...",
                Style::default().fg(theme::MUTED),
            )
        } else {
            Span::raw("")
        },
    ];
    frame.render_widget(Paragraph::new(Line::from(prompt_spans)), chunks[0]);

    frame.render_widget(
        Paragraph::new(Line::from(Span::styled(
            "─".repeat(chunks[1].width as usize),
            Style::default().fg(theme::INACTIVE_BORDER),
        ))),
        chunks[1],
    );

    let visible_rows = chunks[2].height as usize;
    let mut sug_lines = Vec::new();

    if sugs.is_empty() {
        sug_lines.push(Line::from(vec![
            Span::styled("   ", Style::default()),
            Span::styled(
                format!("No commands or symbols matching \"{input}\""),
                Style::default().fg(theme::MUTED),
            ),
        ]));
    } else {
        let active_idx = if app.input_sug_navigated {
            app.input_sug_cursor.min(sugs.len().saturating_sub(1))
        } else {
            usize::MAX
        };

        for (idx, item) in sugs.iter().take(visible_rows).enumerate() {
            let is_selected = idx == active_idx;
            let (cat, desc, cat_color) = suggestion_metadata(item);

            let bg = if is_selected {
                theme::SELECTION
            } else {
                theme::ELEVATED_BG
            };

            let prefix_glyph = if is_selected { " ▶ " } else { "   " };
            let prefix_style = if is_selected {
                Style::default().fg(theme::CYAN).bg(bg).bold()
            } else {
                Style::default().bg(bg)
            };

            let cat_style = Style::default().fg(cat_color).bg(bg).bold();
            let item_style = if is_selected {
                Style::default().fg(theme::FG).bg(bg).bold()
            } else {
                Style::default().fg(theme::FG).bg(bg)
            };
            let desc_style = Style::default()
                .fg(if is_selected {
                    theme::SECONDARY
                } else {
                    theme::MUTED
                })
                .bg(bg);

            let cat_pill = format!("[{:^8}]", cat);
            let item_col = format!(" {:<14}", item);

            sug_lines.push(Line::from(vec![
                Span::styled(prefix_glyph, prefix_style),
                Span::styled(cat_pill, cat_style),
                Span::styled(item_col, item_style),
                Span::styled(format!(" {desc}"), desc_style),
            ]));
        }
    }
    frame.render_widget(Paragraph::new(sug_lines), chunks[2]);

    let footer_spans = vec![
        Span::styled(" [", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Enter", theme::keycap_style()),
        Span::styled("] ", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Execute   ", theme::keycap_label_style()),
        Span::styled("[", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Tab", theme::keycap_style()),
        Span::styled("] ", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Complete   ", theme::keycap_label_style()),
        Span::styled("[", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("↑/↓", theme::keycap_style()),
        Span::styled("] ", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Select   ", theme::keycap_label_style()),
        Span::styled("[", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Esc", theme::keycap_style()),
        Span::styled("] ", Style::default().fg(theme::INACTIVE_BORDER)),
        Span::styled("Dismiss", theme::keycap_label_style()),
    ];
    frame.render_widget(Paragraph::new(Line::from(footer_spans)), chunks[3]);
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
            .map(|p| {
                let arrow = if p > Decimal::ZERO {
                    "▲ "
                } else if p < Decimal::ZERO {
                    "▼ "
                } else {
                    ""
                };
                format!(" {arrow}{p:+.2}%")
            })
            .unwrap_or_default();
        format!(
            "{} {} {}{}",
            q.asset.key(),
            views::format_price(q.price),
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

    let recent_trail = if app.recent_securities.len() > 1 && area.width >= 135 {
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
                " OMT PRO ",
                Style::default()
                    .fg(BG)
                    .bg(AMBER)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                format!(" {} <{}> ", app.workspace.name(), app.workspace.mnemonic()),
                Style::default().fg(AMBER).add_modifier(Modifier::BOLD),
            ),
            Span::styled("│ ", Style::default().fg(theme::INACTIVE_BORDER)),
            Span::styled(quote_badge, Style::default().fg(quote_color).bold()),
            Span::styled(" │ ", Style::default().fg(theme::INACTIVE_BORDER)),
            Span::styled(
                format!("[{}] ", app.layout_mode.label()),
                Style::default().fg(theme::CYAN).bold(),
            ),
            Span::styled(recent_trail, Style::default().fg(MUTED)),
            Span::styled(format!("{provider_name}: "), Style::default().fg(SECONDARY)),
            Span::styled("● ", Style::default().fg(health_color)),
            Span::styled(feed_health, Style::default().fg(health_color)),
            Span::styled(" │ ", Style::default().fg(theme::INACTIVE_BORDER)),
            Span::styled(
                format!("{} UTC", Utc::now().format("%Y-%m-%d %H:%M:%S")),
                Style::default().fg(SECONDARY),
            ),
        ]))
        .style(Style::default().bg(ELEVATED_BG)),
        rows[0],
    );

    // Row 1: High-Contrast Workspace Function Tabs & Live Keyboard Focus Ribbon
    let is_compact_ribbon = area.width < 100;
    let mut nav_spans = Vec::new();

    for (i, w) in Workspace::ALL.iter().enumerate() {
        let active = *w == app.workspace;
        let num_str = format!("{}", i + 1);
        if active {
            let label = if is_compact_ribbon {
                format!(" {}:{} ", num_str, w.mnemonic())
            } else {
                format!(" {}:{}[{}] ", num_str, w.name(), w.mnemonic())
            };
            nav_spans.push(Span::styled(
                label,
                Style::default()
                    .fg(theme::BG)
                    .bg(theme::AMBER)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            nav_spans.push(Span::styled(" ", Style::default().bg(theme::BG)));
            nav_spans.push(Span::styled(format!("[{num_str}]"), theme::keycap_style()));
            let label = if is_compact_ribbon {
                format!(":{} ", w.mnemonic())
            } else {
                format!(":{} ", w.name())
            };
            nav_spans.push(Span::styled(
                label,
                Style::default().fg(theme::SECONDARY).bg(theme::BG),
            ));
        }
    }

    nav_spans.push(Span::styled(
        " │ ",
        Style::default().fg(theme::INACTIVE_BORDER),
    ));

    // Active Keyboard Focus Badge
    let (focus_label, focus_col) = match app.focus {
        0 => ("WATCHLIST", theme::CYAN),
        1 => ("MAIN WORKSPACE", theme::AMBER),
        2 => ("CONTEXT INSPECTOR", theme::CYAN),
        _ => ("MAIN WORKSPACE", theme::AMBER),
    };

    nav_spans.push(Span::styled(
        " ⬢ FOCUS: ",
        Style::default().fg(focus_col).bold(),
    ));
    nav_spans.push(Span::styled(
        format!("{focus_label} "),
        Style::default().fg(theme::FG).bold(),
    ));
    nav_spans.push(Span::styled("[Tab] ", Style::default().fg(theme::MUTED)));

    nav_spans.push(Span::styled(
        " │ ",
        Style::default().fg(theme::INACTIVE_BORDER),
    ));

    // Quick Search Trigger Pill
    if app.input.is_some() {
        nav_spans.push(Span::styled(
            " ❯_ COMMAND RUN BAR ACTIVE ",
            Style::default().fg(theme::BG).bg(theme::AMBER).bold(),
        ));
    } else {
        nav_spans.push(Span::styled(
            " [ / ] SEARCH / GO > ",
            theme::control_pill_style(),
        ));
    }

    frame.render_widget(
        Paragraph::new(Line::from(nav_spans)).style(Style::default().bg(theme::BG)),
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
            let fourth = if app.company.is_some() {
                Workspace::Fin
            } else if !app.macro_data.is_empty() {
                Workspace::Econ
            } else {
                Workspace::Port
            };

            if main_area.width < 90 || main_area.height < 18 {
                let active_ws = match app.quad_focus {
                    0 => Workspace::Market,
                    1 => Workspace::Chart,
                    2 => Workspace::News,
                    _ => fourth,
                };
                let chunks =
                    Layout::vertical([Constraint::Length(1), Constraint::Min(4)]).split(main_area);
                frame.render_widget(
                    Paragraph::new(Line::from(vec![
                        Span::styled(
                            format!(
                                " LAUNCHPAD 2×2 [PANE {}/4: {}] ",
                                app.quad_focus + 1,
                                active_ws.name()
                            ),
                            Style::default().fg(BG).bg(theme::CYAN).bold(),
                        ),
                        Span::styled(
                            " Tab: Cycle Panes │ Enter: Full Workspace │ L: Layout Mode",
                            Style::default().fg(SECONDARY),
                        ),
                    ]))
                    .style(Style::default().bg(ELEVATED_BG)),
                    chunks[0],
                );
                render_workspace_pane(frame, app, active_ws, chunks[1]);
            } else {
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

                if app.market.candles.is_empty()
                    && app.market.chart_status != ChartDataStatus::Loading
                {
                    views::render_market_pulse_card(frame, app, top_cols[1], app.quad_focus == 1);
                } else {
                    render_workspace_pane(frame, app, Workspace::Chart, top_cols[1]);
                }

                render_workspace_pane(frame, app, Workspace::News, bot_cols[0]);
                render_workspace_pane(frame, app, fourth, bot_cols[1]);
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

    // Row 3: Status Message + Interactive Keycap Control Ribbon
    let footer_shortcuts = render_shortcut_footer_line(app.workspace, area.width);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    " STATUS ",
                    Style::default().fg(theme::BG).bg(theme::AMBER).bold(),
                ),
                Span::styled(format!(" {}", app.message), Style::default().fg(theme::FG)),
            ]),
            footer_shortcuts,
        ])
        .style(Style::default().bg(theme::BG)),
        rows[3],
    );

    if let Some(input) = &app.input {
        render_centered_command_palette(frame, app, area, input);
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
    #[test]
    fn centered_command_palette_and_suggestion_navigation() {
        let mut app = App::default();

        // 1. Open command bar with '/'
        app.key(KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE));
        assert_eq!(app.input.as_deref(), Some(""));
        assert_eq!(app.input_sug_cursor, 0);
        assert!(!app.input_sug_navigated);

        // Render on 120x35 and check that centered command palette is rendered
        let mut t = Terminal::new(TestBackend::new(120, 35)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("COMMAND RUN BAR & SYMBOL SEARCH"));
        assert!(text.contains("Default Shortcuts"));
        assert!(text.contains("TOP"));
        assert!(text.contains("BTCUSDT GP"));
        assert!(text.contains("[Enter] Execute"));
        assert!(text.contains("[Tab] Complete"));

        // 2. Type "BTC"
        for c in "BTC".chars() {
            app.key(KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE));
        }
        assert_eq!(app.input.as_deref(), Some("BTC"));
        assert_eq!(app.input_sug_cursor, 0);

        t.draw(|f| render(f, &app)).unwrap();
        let text_btc: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_btc.contains("BTCUSDT"));
        assert!(text_btc.contains("CRYPTO"));

        // 3. Navigate down through suggestions
        app.key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(app.input_sug_navigated);
        assert_eq!(app.input_sug_cursor, 0);

        // 4. Tab autocompletes the selected suggestion
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.input.as_deref(), Some("BTCUSDT"));
        assert!(!app.input_sug_navigated);

        // 5. Enter executes the autocompleted command
        let executed = app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(executed, Some("BTCUSDT".into()));
        assert!(app.input.is_none());

        // 6. Esc key dismisses opened command palette
        app.key(KeyEvent::new(KeyCode::Char(':'), KeyModifiers::NONE));
        assert!(app.input.is_some());
        app.key(KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert!(app.input.is_none());
    }
    #[test]
    fn multi_tab_navigation_and_visible_focus_cycling() {
        let mut app = App::default();

        // Standard mode: targets are [0: Watchlist, 1: Main, 2: Context]
        assert_eq!(app.focus, 1);
        assert_eq!(app.focus_label(), "MAIN WORKSPACE");

        app.next_focus();
        assert_eq!(app.focus, 2);
        assert_eq!(app.focus_label(), "CONTEXT INSPECTOR");

        app.next_focus();
        assert_eq!(app.focus, 0);
        assert_eq!(app.focus_label(), "WATCHLIST");

        app.next_focus();
        assert_eq!(app.focus, 1);

        app.prev_focus();
        assert_eq!(app.focus, 0);

        // Toggle Watchlist off (W): targets become [1: Main, 2: Context]
        app.key(KeyEvent::new(KeyCode::Char('W'), KeyModifiers::NONE));
        assert!(!app.show_watchlist);
        assert_eq!(app.focus, 1); // Reset from hidden panel 0 to 1

        app.next_focus();
        assert_eq!(app.focus, 2);
        app.next_focus();
        assert_eq!(app.focus, 1); // Skips watchlist!

        // Toggle Context Inspector off (I): only [1: Main] remains
        app.key(KeyEvent::new(KeyCode::Char('I'), KeyModifiers::NONE));
        assert!(!app.show_context);
        app.next_focus();
        assert_eq!(app.focus, 1);
        app.prev_focus();
        assert_eq!(app.focus, 1);

        // Restore panels
        app.show_watchlist = true;
        app.show_context = true;

        // Verify Tab Ribbon and Focus Badge rendering
        let mut t = Terminal::new(TestBackend::new(140, 35)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text.contains("OMT PRO"));
        assert!(text.contains("FOCUS: MAIN WORKSPACE"));
        assert!(text.contains("[ / ] SEARCH"));
    }
    #[test]
    fn command_palette_responsive_geometry_across_screen_sizes() {
        let app = App {
            input: Some("BTC".into()),
            ..Default::default()
        };

        for (w, h) in [(80, 24), (120, 35), (180, 45), (240, 60)] {
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| render(f, &app)).unwrap();
            let text: String = t
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            assert!(text.contains("COMMAND RUN BAR"));
            assert!(text.contains("BTC"));
        }
    }

    #[test]
    fn test_launchpad_adaptive_rendering_and_quad_focus_cycling() {
        let mut app = App {
            layout_mode: LayoutMode::Quad,
            ..Default::default()
        };

        // Focus cycling in Quad mode
        assert_eq!(app.quad_focus, 0);
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.quad_focus, 1);
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.quad_focus, 2);
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.quad_focus, 3);
        app.key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(app.quad_focus, 0);

        app.key(KeyEvent::new(KeyCode::BackTab, KeyModifiers::NONE));
        assert_eq!(app.quad_focus, 3);

        // Enter expands focused quadrant into full workspace
        app.quad_focus = 0;
        let cmd = app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(cmd, Some("MARKET".into()));
        assert_eq!(app.layout_mode, LayoutMode::Standard);

        app.layout_mode = LayoutMode::Quad;
        app.quad_focus = 2;
        let cmd = app.key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert_eq!(cmd, Some("NEWS".into()));

        // Responsive geometry test across screen dimensions
        for (w, h) in [(80, 24), (100, 30), (120, 40), (160, 50), (200, 60)] {
            app.layout_mode = LayoutMode::Quad;
            let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
            t.draw(|f| render(f, &app)).unwrap();
            let text: String = t
                .backend()
                .buffer()
                .content
                .iter()
                .map(|c| c.symbol())
                .collect();
            if w < 90 || h < 18 {
                assert!(text.contains("LAUNCHPAD 2×2 [PANE"));
            } else {
                assert!(text.contains("MARKET") || text.contains("MKT"));
                // Fallback widget appears when chart has no candles
                assert!(text.contains("Market Pulse") || text.contains("MARKET PULSE"));
            }
        }
    }

    #[test]
    fn test_chart_diagnostics_actionable_states() {
        let mut app = App {
            workspace: Workspace::Chart,
            asset: "AAPL".parse().unwrap(),
            ..Default::default()
        };

        // 1. AuthenticationRequired
        app.market.chart_status = ChartDataStatus::AuthenticationRequired;
        let mut t = Terminal::new(TestBackend::new(140, 40)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text_auth: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_auth.contains("AUTHENTICATION REQUIRED"));
        assert!(text_auth.contains("APCA_API_KEY_ID"));

        // 2. Loading
        app.market.chart_status = ChartDataStatus::Loading;
        t.draw(|f| render(f, &app)).unwrap();
        let text_load: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_load.contains("LOADING HISTORICAL CANDLES"));

        // 3. Error with actionable message
        app.market.chart_status = ChartDataStatus::Error;
        app.market.chart_error = Some("Connection refused by peer".into());
        t.draw(|f| render(f, &app)).unwrap();
        let text_err: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();
        assert!(text_err.contains("UNABLE TO LOAD HISTORICAL CANDLES"));
        assert!(text_err.contains("Connection refused"));
        assert!(text_err.contains("Press 'R' to retry"));
    }

    #[test]
    fn test_financials_secondary_analysis_region() {
        use chrono::NaiveDate;
        use omt_core::FinancialFact;

        let facts = vec![
            FinancialFact {
                metric: "Revenue".into(),
                concept: "Revenues".into(),
                value: Decimal::from(391_035_000_000i64),
                unit: "USD".into(),
                start: Some(NaiveDate::from_ymd_opt(2023, 10, 1).unwrap()),
                end: NaiveDate::from_ymd_opt(2024, 9, 28).unwrap(),
                filed: NaiveDate::from_ymd_opt(2024, 10, 31).unwrap(),
                accession: "0000320193-24-000106".into(),
                form: "10-K".into(),
                source: "SEC EDGAR".into(),
            },
            FinancialFact {
                metric: "Net income".into(),
                concept: "NetIncomeLoss".into(),
                value: Decimal::from(93_736_000_000i64),
                unit: "USD".into(),
                start: Some(NaiveDate::from_ymd_opt(2023, 10, 1).unwrap()),
                end: NaiveDate::from_ymd_opt(2024, 9, 28).unwrap(),
                filed: NaiveDate::from_ymd_opt(2024, 10, 31).unwrap(),
                accession: "0000320193-24-000106".into(),
                form: "10-K".into(),
                source: "SEC EDGAR".into(),
            },
        ];

        let company = Company {
            ticker: "AAPL".into(),
            cik: 320193,
            name: "Apple Inc.".into(),
            facts,
            filings: vec![],
            fetched_at: Utc::now(),
        };

        let app = App {
            workspace: Workspace::Fin,
            company: Some(company),
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

        assert!(text.contains("HISTORICAL TRAJECTORY"));
        assert!(text.contains("XBRL FACT AUDIT"));
        assert!(text.contains("Revenue"));
    }

    #[test]
    fn test_news_three_pane_mode_on_wide_screens() {
        let items = vec![NewsItem {
            id: "news-wide-1".into(),
            headline: "Treasury Yields Stable Following Economic Data".into(),
            summary: "Treasury yields held steady after GDP report.".into(),
            body: Some(
                "Detailed report on treasury market dynamics and inflation expectations.".into(),
            ),
            source: "CNBC".into(),
            url: "https://example.com/news/treasury".into(),
            published_at: Utc::now(),
            fetched_at: Utc::now(),
            ticker: Some("US10Y".into()),
            category: NewsCategory::Macro,
            is_sec_filing: false,
            form: None,
            accession: None,
            report_date: None,
        }];

        let app = App {
            workspace: Workspace::News,
            news_items: items,
            ..Default::default()
        };

        // On 160x40 (>= 150), renders 3 panes: Headlines, Reader, and Publisher Context
        let mut t = Terminal::new(TestBackend::new(160, 40)).unwrap();
        t.draw(|f| render(f, &app)).unwrap();
        let text: String = t
            .backend()
            .buffer()
            .content
            .iter()
            .map(|c| c.symbol())
            .collect();

        assert!(text.contains("Headlines"));
        assert!(text.contains("Story & Disclosure Reader"));
        assert!(text.contains("Story Context"));
        assert!(text.contains("CNBC"));
        assert!(text.contains("US10Y"));
    }
}
