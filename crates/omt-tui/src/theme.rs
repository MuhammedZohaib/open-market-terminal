use omt_core::DataStatus;
use ratatui::style::{Color, Modifier, Style};
use rust_decimal::Decimal;

/// Centralized Bloomberg-inspired dark workstation palette.
pub const BG: Color = Color::Rgb(11, 14, 18); // #0B0E12
pub const PANEL_BG: Color = Color::Rgb(16, 21, 27); // #10151B
pub const ELEVATED_BG: Color = Color::Rgb(23, 29, 37); // #171D25
pub const BORDER: Color = Color::Rgb(52, 59, 69); // #343B45
pub const FG: Color = Color::Rgb(227, 231, 237); // #E3E7ED
pub const SECONDARY: Color = Color::Rgb(152, 163, 179); // #98A3B3
pub const MUTED: Color = Color::Rgb(103, 117, 134); // #677586
pub const AMBER: Color = Color::Rgb(244, 181, 68); // #F4B544
pub const GOLD: Color = AMBER;
pub const POSITIVE: Color = Color::Rgb(74, 198, 154); // #4AC69A
pub const NEGATIVE: Color = Color::Rgb(235, 107, 120); // #EB6B78
pub const SELECTION: Color = Color::Rgb(37, 54, 73); // #253649
pub const INFO_BLUE: Color = Color::Rgb(110, 168, 229);
pub const CYAN: Color = Color::Rgb(86, 204, 242); // #56CCF2 Electric Cyan for controls
pub const CYAN_BG: Color = Color::Rgb(14, 38, 52); // #0E2634 Subtle cyan elevated container
pub const VIOLET: Color = Color::Rgb(178, 148, 235); // #B294EB
pub const ACTIVE_FOCUS: Color = AMBER;
pub const INACTIVE_BORDER: Color = Color::Rgb(44, 52, 64); // #2C3440 Soft muted border

pub fn direction_color(value: Option<Decimal>) -> Color {
    match value {
        Some(v) if v > Decimal::ZERO => POSITIVE,
        Some(v) if v < Decimal::ZERO => NEGATIVE,
        _ => FG,
    }
}

pub fn status_color(status: DataStatus) -> Color {
    match status {
        DataStatus::Live => POSITIVE,
        DataStatus::Delayed => AMBER,
        DataStatus::Stale => AMBER,
        DataStatus::Disconnected => NEGATIVE,
        DataStatus::Historical => SECONDARY,
    }
}

pub fn header_style() -> Style {
    Style::default()
        .fg(AMBER)
        .bg(ELEVATED_BG)
        .add_modifier(Modifier::BOLD)
}

pub fn row_highlight_style() -> Style {
    Style::default()
        .bg(SELECTION)
        .fg(FG)
        .add_modifier(Modifier::BOLD)
}

pub fn pill_style(active: bool) -> Style {
    if active {
        Style::default()
            .fg(BG)
            .bg(AMBER)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(SECONDARY).bg(ELEVATED_BG)
    }
}

pub fn keycap_style() -> Style {
    Style::default().fg(CYAN).add_modifier(Modifier::BOLD)
}

pub fn keycap_label_style() -> Style {
    Style::default().fg(SECONDARY)
}

pub fn control_pill_style() -> Style {
    Style::default().fg(CYAN).bg(CYAN_BG).bold()
}
