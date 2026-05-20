//! App-level chrome: top title bar and bottom statusline.
//!
//! Pages expose `title()`, `mode()`, `context()`, and `hints()`;
//! the app composes those into consistent chrome.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use unicode_width::UnicodeWidthChar;

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
        Span::styled(" ♫ ", Style::default().fg(Theme::ACCENT())),
        Span::styled(
            "netune",
            Style::default()
                .fg(Theme::ACCENT())
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled("  netease cloud music", Style::default().fg(Theme::MUTED())),
    ]);
    f.render_widget(Paragraph::new(left), cols[0]);

    let right = Line::from(vec![
        Span::styled(
            page_title.to_owned(),
            Style::default()
                .fg(Theme::FG_DIM())
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
    context_tick: usize,
    hints: &[KeyHint],
) {
    let mut hint_spans: Vec<Span> = Vec::new();
    for (i, h) in hints.iter().enumerate() {
        if i > 0 {
            hint_spans.push(Span::styled("  ·  ", Style::default().fg(Theme::MUTED())));
        }
        hint_spans.push(Span::styled(
            h.key,
            Style::default()
                .fg(Theme::ACCENT())
                .add_modifier(Modifier::BOLD),
        ));
        hint_spans.push(Span::styled(
            format!(" {}", h.label),
            Style::default().fg(Theme::FG_DIM()),
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

    let ctx_spans = floating_context_spans(&context, cols[1].width as usize, context_tick);
    f.render_widget(Paragraph::new(Line::from(ctx_spans)), cols[1]);

    f.render_widget(
        Paragraph::new(Line::from(hint_spans)).alignment(Alignment::Right),
        cols[2],
    );
}

fn floating_context_spans(
    context: &[Span<'static>],
    width: usize,
    tick: usize,
) -> Vec<Span<'static>> {
    if width == 0 {
        return Vec::new();
    }

    let mut cells: Vec<(char, Style)> = Vec::new();
    for ch in "  ".chars() {
        cells.push((ch, Style::default()));
    }
    for span in context {
        for ch in span.content.chars() {
            cells.push((ch, span.style));
        }
    }

    let total_width: usize = cells
        .iter()
        .map(|(ch, _)| UnicodeWidthChar::width(*ch).unwrap_or(0))
        .sum();
    if total_width <= width {
        let mut rendered = vec![Span::styled("  ", Style::default())];
        rendered.extend(context.iter().cloned());
        return rendered;
    }

    for ch in "   ".chars() {
        cells.push((ch, Style::default()));
    }

    let offset = (tick / 3) % cells.len();
    let mut rendered: Vec<Span<'static>> = Vec::new();
    let mut current = String::new();
    let mut current_style: Option<Style> = None;
    let mut used = 0usize;

    for i in 0..cells.len() {
        let (ch, style) = cells[(offset + i) % cells.len()];
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
        if ch_width != 0 && used + ch_width > width {
            break;
        }

        if current_style.is_some_and(|s| s != style) && !current.is_empty() {
            rendered.push(Span::styled(
                std::mem::take(&mut current),
                current_style.unwrap(),
            ));
        }
        current_style = Some(style);
        current.push(ch);
        used += ch_width;
    }

    if !current.is_empty() {
        rendered.push(Span::styled(current, current_style.unwrap_or_default()));
    }

    rendered
}

#[cfg(test)]
mod tests {
    use super::*;
    use unicode_width::UnicodeWidthStr;

    fn spans_width(spans: &[Span<'static>]) -> usize {
        spans
            .iter()
            .map(|span| UnicodeWidthStr::width(span.content.as_ref()))
            .sum()
    }

    #[test]
    fn floating_context_keeps_short_context_unchanged() {
        let context = vec![Span::styled(
            "Short song",
            Style::default().fg(Theme::ACCENT()),
        )];
        let rendered = floating_context_spans(&context, 20, 0);

        assert_eq!(spans_width(&rendered), 12);
        assert_eq!(rendered[0].content.as_ref(), "  ");
        assert_eq!(rendered[1].content.as_ref(), "Short song");
    }

    #[test]
    fn floating_context_never_exceeds_available_width() {
        let context = vec![
            Span::styled("飞跃经济舱 (LIVE版)", Style::default().fg(Theme::ACCENT())),
            Span::styled(
                " — very long artist name",
                Style::default().fg(Theme::MUTED()),
            ),
        ];

        for tick in 0..60 {
            let rendered = floating_context_spans(&context, 16, tick);
            assert!(
                spans_width(&rendered) <= 16,
                "context exceeded width at tick {tick}: {rendered:?}"
            );
        }
    }

    #[test]
    fn floating_context_scrolls_long_context_over_time() {
        let context = vec![Span::styled(
            "This is a very long statusline song title",
            Style::default().fg(Theme::ACCENT()),
        )];

        let first = floating_context_spans(&context, 14, 0);
        let second = floating_context_spans(&context, 14, 9);

        assert_ne!(
            first
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>(),
            second
                .iter()
                .map(|span| span.content.as_ref())
                .collect::<String>()
        );
    }
}
