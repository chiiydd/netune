//! App-level chrome: top title bar and bottom statusline.
//!
//! Pages expose `title()`, `mode()`, `context()`, and `hints()`;
//! the app composes those into consistent chrome.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;

/// A single key→action hint shown in the statusline.
#[derive(Clone)]
pub struct KeyHint {
    pub key: &'static str,
    pub label: &'static str,
}

impl KeyHint {
    pub const fn new(key: &'static str, label: &'static str) -> Self {
        Self { key, label }
    }
}

/// Render the 1-row title bar.
pub fn render_titlebar(f: &mut Frame, area: Rect, page_title: &str) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(30)])
        .split(area);

    let left = Line::from(vec![
        Span::styled(" ♫ ", Style::default().fg(Theme::ACCENT)),
        Span::styled(
            "netune",
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  netease cloud music", Style::default().fg(Theme::MUTED)),
    ]);
    f.render_widget(Paragraph::new(left), cols[0]);

    let right = Line::from(vec![
        Span::styled(
            page_title.to_owned(),
            Style::default()
                .fg(Theme::FG_DIM)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("   "),
    ]);
    f.render_widget(Paragraph::new(right).alignment(Alignment::Right), cols[1]);
}

/// Render the 1-row statusline.
pub fn render_statusline(
    f: &mut Frame,
    area: Rect,
    mode: &str,
    mode_color: Color,
    context: Vec<Span<'static>>,
    hints: &[KeyHint],
) {
    let mut hint_spans: Vec<Span> = Vec::new();
    for (i, h) in hints.iter().enumerate() {
        if i > 0 {
            hint_spans.push(Span::styled("  ·  ", Style::default().fg(Theme::MUTED)));
        }
        hint_spans.push(Span::styled(
            h.key,
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        ));
        hint_spans.push(Span::styled(
            format!(" {}", h.label),
            Style::default().fg(Theme::FG_DIM),
        ));
    }
    hint_spans.push(Span::raw(" "));

    let hint_w_chars: usize = hints
        .iter()
        .map(|h| h.key.chars().count() + 1 + h.label.chars().count())
        .sum::<usize>()
        + hints.len().saturating_sub(1) * 5
        + 1;
    let hint_w = (hint_w_chars as u16).min(area.width.saturating_sub(20));

    let mode_text = format!(" {mode} ");
    let mode_w = mode_text.chars().count() as u16;

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(mode_w),
            Constraint::Min(1),
            Constraint::Length(hint_w),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(Line::from(Span::styled(
            mode_text,
            Style::default()
                .bg(mode_color)
                .fg(Color::Black)
                .add_modifier(Modifier::BOLD),
        ))),
        cols[0],
    );

    let mut ctx_spans: Vec<Span> = vec![Span::styled("  ", Style::default())];
    ctx_spans.extend(context);
    f.render_widget(Paragraph::new(Line::from(ctx_spans)), cols[1]);

    f.render_widget(
        Paragraph::new(Line::from(hint_spans)).alignment(Alignment::Right),
        cols[2],
    );
}
