//! Settings page — audio device, quality, theme, and cache management.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};
use ratatui::Frame;

use netune_core::models::QualityLevel;

use crate::chrome::KeyHint;
use crate::pages::{PageAction, SettingsField};
use crate::theme::Theme;

/// Theme selection (UI-only, maps to index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AppTheme {
    Dark,
    Light,
    Dracula,
}

impl AppTheme {
    fn label(&self) -> &'static str {
        match self {
            Self::Dark => "Dark",
            Self::Light => "Light",
            Self::Dracula => "Dracula",
        }
    }

    const ALL: &'static [AppTheme] = &[Self::Dark, Self::Light, Self::Dracula];
}

/// All quality options we cycle through.
const QUALITY_OPTIONS: &[QualityLevel] = &[
    QualityLevel::Standard,
    QualityLevel::Higher,
    QualityLevel::ExHigh,
    QualityLevel::Lossless,
];

pub struct SettingsPage {
    /// Current audio device name (display only for now).
    audio_device: String,
    /// Selected audio quality.
    quality_idx: usize,
    /// Selected theme index.
    theme_idx: usize,
    /// Cache size display string.
    cache_size: String,
    /// Which field is focused.
    focus: SettingsField,
}

impl Default for SettingsPage {
    fn default() -> Self {
        Self::new()
    }
}

impl SettingsPage {
    pub fn new() -> Self {
        Self {
            audio_device: "Default".to_string(),
            quality_idx: 2, // ExHigh
            theme_idx: 0,   // Dark
            cache_size: "0 MB".to_string(),
            focus: SettingsField::Device,
        }
    }

    fn quality(&self) -> QualityLevel {
        QUALITY_OPTIONS[self.quality_idx]
    }

    fn theme_label(&self) -> &'static str {
        AppTheme::ALL[self.theme_idx].label()
    }

    fn cycle_quality_left(&mut self) {
        self.quality_idx = self
            .quality_idx
            .checked_sub(1)
            .unwrap_or(QUALITY_OPTIONS.len() - 1);
    }

    fn cycle_quality_right(&mut self) {
        self.quality_idx = (self.quality_idx + 1) % QUALITY_OPTIONS.len();
    }

    fn cycle_theme_left(&mut self) {
        self.theme_idx = self
            .theme_idx
            .checked_sub(1)
            .unwrap_or(AppTheme::ALL.len() - 1);
    }

    fn cycle_theme_right(&mut self) {
        self.theme_idx = (self.theme_idx + 1) % AppTheme::ALL.len();
    }

    fn clear_cache(&mut self) {
        // TODO: actually clear cache when storage is wired up.
        self.cache_size = "0 MB".to_string();
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // title
                Constraint::Min(1),    // settings list
                Constraint::Length(1), // hints
            ])
            .split(area);

        self.render_title(f, chunks[0]);
        self.render_fields(f, chunks[1]);
        self.render_hints(f, chunks[2]);
    }

    fn render_title(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT))
            .title(Span::styled(
                " 设置 ",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "  Settings  —  manage your preferences",
                Style::default().fg(Theme::FG_DIM),
            )))
            .block(block),
            area,
        );
    }

    fn render_fields(&self, f: &mut Frame, area: Rect) {
        // Split into 4 rows, one per setting field.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3), // device
                Constraint::Length(3), // quality
                Constraint::Length(3), // theme
                Constraint::Length(3), // cache
                Constraint::Min(0),    // flex
            ])
            .split(area);

        self.render_device_field(f, rows[0]);
        self.render_quality_field(f, rows[1]);
        self.render_theme_field(f, rows[2]);
        self.render_cache_field(f, rows[3]);
    }

    fn field_border(&self, field: SettingsField) -> Style {
        let color = if self.focus == field {
            Theme::ACCENT
        } else {
            Theme::ACCENT_DIM
        };
        Style::default().fg(color)
    }

    fn field_title_style(&self, field: SettingsField) -> Style {
        if self.focus == field {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG_DIM)
        }
    }

    fn render_device_field(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.field_border(SettingsField::Device))
            .title(Span::styled(
                " 🔊 Audio Device ",
                self.field_title_style(SettingsField::Device),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let value_style = if self.focus == SettingsField::Device {
            Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG)
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(&self.audio_device, value_style),
                Span::styled("  (read-only)", Style::default().fg(Theme::MUTED)),
            ])),
            inner,
        );
    }

    fn render_quality_field(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.field_border(SettingsField::Quality))
            .title(Span::styled(
                " 🎵 Audio Quality ",
                self.field_title_style(SettingsField::Quality),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let quality_label = self.quality().label();
        let is_focused = self.focus == SettingsField::Quality;
        let arrow_style = if is_focused {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::MUTED)
        };
        let value_style = if is_focused {
            Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG)
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("◀ ", arrow_style),
                Span::styled(quality_label, value_style),
                Span::styled(" ▶", arrow_style),
                Span::styled(
                    format!("  ({:.0} kbps)", self.quality().bitrate() as f64 / 1000.0),
                    Style::default().fg(Theme::MUTED),
                ),
            ])),
            inner,
        );
    }

    fn render_theme_field(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.field_border(SettingsField::Theme))
            .title(Span::styled(
                " 🎨 Theme ",
                self.field_title_style(SettingsField::Theme),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let is_focused = self.focus == SettingsField::Theme;
        let arrow_style = if is_focused {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::MUTED)
        };
        let value_style = if is_focused {
            Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG)
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled("◀ ", arrow_style),
                Span::styled(self.theme_label(), value_style),
                Span::styled(" ▶", arrow_style),
            ])),
            inner,
        );
    }

    fn render_cache_field(&self, f: &mut Frame, area: Rect) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(self.field_border(SettingsField::Cache))
            .title(Span::styled(
                " 💾 Cache ",
                self.field_title_style(SettingsField::Cache),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        let is_focused = self.focus == SettingsField::Cache;
        let btn_style = if is_focused {
            Style::default()
                .fg(Theme::DANGER)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG_DIM)
        };

        f.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    format!("{} ", self.cache_size),
                    Style::default().fg(Theme::FG),
                ),
                Span::styled("[ 清理 ]", btn_style),
            ])),
            inner,
        );
    }

    fn render_hints(&self, f: &mut Frame, area: Rect) {
        let hints_text = "  Tab/↑↓: switch  ←→: change  Enter: action  q/Esc: back";
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                hints_text,
                Style::default().fg(Theme::MUTED),
            ))),
            area,
        );
    }

    // ── Events ──────────────────────────────────────────────────────────────

    pub fn handle_event(&mut self, evt: &Event) -> PageAction {
        let Event::Key(k) = evt else {
            return PageAction::None;
        };
        if k.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => return PageAction::Pop,

            KeyCode::Tab | KeyCode::Down => {
                self.focus = match self.focus {
                    SettingsField::Device => SettingsField::Quality,
                    SettingsField::Quality => SettingsField::Theme,
                    SettingsField::Theme => SettingsField::Cache,
                    SettingsField::Cache => SettingsField::Device,
                };
            }
            KeyCode::Up => {
                self.focus = match self.focus {
                    SettingsField::Device => SettingsField::Cache,
                    SettingsField::Quality => SettingsField::Device,
                    SettingsField::Theme => SettingsField::Quality,
                    SettingsField::Cache => SettingsField::Theme,
                };
            }
            KeyCode::Left => match self.focus {
                SettingsField::Quality => self.cycle_quality_left(),
                SettingsField::Theme => self.cycle_theme_left(),
                _ => {}
            },
            KeyCode::Right => match self.focus {
                SettingsField::Quality => self.cycle_quality_right(),
                SettingsField::Theme => self.cycle_theme_right(),
                _ => {}
            },
            KeyCode::Enter => {
                if self.focus == SettingsField::Cache {
                    self.clear_cache();
                }
            }
            _ => {}
        }
        PageAction::None
    }

    // ── Chrome contract ─────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        ("SETTINGS".into(), Theme::MODE_NORMAL)
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        vec![
            Span::styled("quality: ", Style::default().fg(Theme::MUTED)),
            Span::styled(self.quality().label().to_owned(), Theme::accent_bold()),
            Span::raw("  "),
            Span::styled("theme: ", Style::default().fg(Theme::MUTED)),
            Span::styled(self.theme_label().to_owned(), Theme::accent_bold()),
        ]
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("Tab/↑↓", "field"),
            KeyHint::new("←/→", "change"),
            KeyHint::new("⏎", "action"),
            KeyHint::new("Esc", "back"),
        ]
    }
}
