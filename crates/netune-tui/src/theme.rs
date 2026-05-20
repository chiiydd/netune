//! Central color palette for the TUI.
//!
//! All pages pull colors from `Theme::*` so the look stays coherent.
//! Theme is runtime-switchable via `Theme::set_theme("Dark" | "Light" | "Dracula")`.
//!
//! Uses an atomic index into a const array — zero lock overhead on every read.

use std::sync::atomic::{AtomicU8, Ordering};

use ratatui::style::{Color, Modifier, Style};

struct ThemeColors {
    accent: Color,
    accent_dim: Color,
    success: Color,
    warning: Color,
    danger: Color,
    info: Color,
    playing: Color,
    fg: Color,
    fg_dim: Color,
    muted: Color,
    bg: Color,
    sel_fg: Color,
    sel_bg: Color,
    mode_normal: Color,
    mode_playing: Color,
    mode_loading: Color,
    mode_search: Color,
}

const THEMES: [ThemeColors; 3] = [
    // Dark (index 0 — default)
    ThemeColors {
        accent: Color::Cyan,
        accent_dim: Color::Rgb(80, 130, 150),
        success: Color::Green,
        warning: Color::Yellow,
        danger: Color::Red,
        info: Color::Blue,
        playing: Color::Magenta,
        fg: Color::White,
        fg_dim: Color::Rgb(180, 180, 180),
        muted: Color::Rgb(128, 128, 128),
        bg: Color::Reset,
        sel_fg: Color::Black,
        sel_bg: Color::Cyan,
        mode_normal: Color::Cyan,
        mode_playing: Color::Magenta,
        mode_loading: Color::Yellow,
        mode_search: Color::Blue,
    },
    // Light (index 1)
    ThemeColors {
        accent: Color::Rgb(0, 100, 160),
        accent_dim: Color::Rgb(100, 150, 180),
        success: Color::Rgb(0, 128, 0),
        warning: Color::Rgb(180, 130, 0),
        danger: Color::Rgb(180, 0, 0),
        info: Color::Rgb(0, 0, 180),
        playing: Color::Rgb(140, 0, 140),
        fg: Color::Black,
        fg_dim: Color::Rgb(80, 80, 80),
        muted: Color::Rgb(128, 128, 128),
        bg: Color::Reset,
        sel_fg: Color::White,
        sel_bg: Color::Rgb(0, 100, 160),
        mode_normal: Color::Rgb(0, 100, 160),
        mode_playing: Color::Rgb(140, 0, 140),
        mode_loading: Color::Rgb(180, 130, 0),
        mode_search: Color::Rgb(0, 0, 180),
    },
    // Dracula (index 2)
    ThemeColors {
        accent: Color::Rgb(139, 233, 253),
        accent_dim: Color::Rgb(98, 114, 164),
        success: Color::Rgb(80, 250, 123),
        warning: Color::Rgb(241, 250, 140),
        danger: Color::Rgb(255, 85, 85),
        info: Color::Rgb(98, 114, 164),
        playing: Color::Rgb(255, 121, 198),
        fg: Color::Rgb(248, 248, 242),
        fg_dim: Color::Rgb(98, 114, 164),
        muted: Color::Rgb(98, 114, 164),
        bg: Color::Reset,
        sel_fg: Color::Rgb(40, 42, 54),
        sel_bg: Color::Rgb(139, 233, 253),
        mode_normal: Color::Rgb(139, 233, 253),
        mode_playing: Color::Rgb(255, 121, 198),
        mode_loading: Color::Rgb(241, 250, 140),
        mode_search: Color::Rgb(98, 114, 164),
    },
];

/// Current theme index (atomic for lock-free reads).
static THEME_IDX: AtomicU8 = AtomicU8::new(0);

#[inline]
fn current() -> &'static ThemeColors {
    &THEMES[THEME_IDX.load(Ordering::Relaxed) as usize]
}

pub struct Theme;

#[allow(non_snake_case)]
impl Theme {
    /// Switch the global theme at runtime.
    pub fn set_theme(name: &str) {
        let idx: u8 = match name {
            "Light" => 1,
            "Dracula" => 2,
            _ => 0, // Dark
        };
        THEME_IDX.store(idx, Ordering::Relaxed);
    }

    // ── Color getters ───────────────────────────────────────────────────────
    pub fn ACCENT() -> Color { current().accent }
    pub fn ACCENT_DIM() -> Color { current().accent_dim }
    pub fn SUCCESS() -> Color { current().success }
    pub fn WARNING() -> Color { current().warning }
    pub fn DANGER() -> Color { current().danger }
    pub fn INFO() -> Color { current().info }
    pub fn PLAYING() -> Color { current().playing }
    pub fn FG() -> Color { current().fg }
    pub fn FG_DIM() -> Color { current().fg_dim }
    pub fn MUTED() -> Color { current().muted }
    pub fn BG() -> Color { current().bg }
    pub fn SEL_FG() -> Color { current().sel_fg }
    pub fn SEL_BG() -> Color { current().sel_bg }
    pub fn MODE_NORMAL() -> Color { current().mode_normal }
    pub fn MODE_PLAYING() -> Color { current().mode_playing }
    pub fn MODE_LOADING() -> Color { current().mode_loading }
    pub fn MODE_SEARCH() -> Color { current().mode_search }

    // ── Pre-built styles ────────────────────────────────────────────────────
    pub fn accent_bold() -> Style {
        Style::default()
            .fg(Self::ACCENT())
            .add_modifier(Modifier::BOLD)
    }
    pub fn muted_style() -> Style {
        Style::default().fg(Self::MUTED())
    }
    pub fn selection() -> Style {
        Style::default()
            .fg(Self::SEL_FG())
            .bg(Self::SEL_BG())
            .add_modifier(Modifier::BOLD)
    }
}
