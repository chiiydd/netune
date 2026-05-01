//! Central color palette for the TUI.
//!
//! All pages pull colors from `Theme::*` so the look stays coherent.

use ratatui::style::{Color, Modifier, Style};

pub struct Theme;

impl Theme {
    // ── Brand ────────────────────────────────────────────────────────────────
    pub const ACCENT: Color = Color::Cyan;
    pub const ACCENT_DIM: Color = Color::Rgb(80, 130, 150);

    // ── Semantic ─────────────────────────────────────────────────────────────
    pub const SUCCESS: Color = Color::Green;
    pub const WARNING: Color = Color::Yellow;
    pub const DANGER: Color = Color::Red;
    pub const INFO: Color = Color::Blue;
    pub const PLAYING: Color = Color::Magenta;

    // ── Neutral ──────────────────────────────────────────────────────────────
    pub const FG: Color = Color::White;
    pub const FG_DIM: Color = Color::Rgb(180, 180, 180);
    pub const MUTED: Color = Color::Rgb(128, 128, 128);
    pub const BG: Color = Color::Reset;

    // ── Selection ────────────────────────────────────────────────────────────
    pub const SEL_FG: Color = Color::Black;
    pub const SEL_BG: Color = Color::Cyan;

    // ── Mode-badge backgrounds ───────────────────────────────────────────────
    pub const MODE_NORMAL: Color = Color::Cyan;
    pub const MODE_PLAYING: Color = Color::Magenta;
    pub const MODE_LOADING: Color = Color::Yellow;
    pub const MODE_SEARCH: Color = Color::Blue;

    // ── Pre-built styles ─────────────────────────────────────────────────────
    pub fn accent_bold() -> Style {
        Style::default()
            .fg(Self::ACCENT)
            .add_modifier(Modifier::BOLD)
    }
    pub fn muted() -> Style {
        Style::default().fg(Self::MUTED)
    }
    pub fn selection() -> Style {
        Style::default()
            .fg(Self::SEL_FG)
            .bg(Self::SEL_BG)
            .add_modifier(Modifier::BOLD)
    }
}
