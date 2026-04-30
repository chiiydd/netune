//! Queue panel — floating overlay showing the current play queue.
//!
//! Displays all songs in the queue with the currently playing song highlighted.
//! Supports selecting and jumping to a song via keyboard.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState};
use ratatui::Frame;

use netune_core::models::Song;

use crate::theme::Theme;

/// Result of handling an event within the queue panel.
pub enum QueuePanelResult {
    /// The panel consumed the event and wants to close.
    Close,
    /// The panel wants to jump to a specific queue index.
    JumpTo(usize),
    /// The event was not handled by the panel.
    NotHandled,
}

pub struct QueuePanel {
    list_state: ListState,
    total: usize,
    current_index: usize,
}

impl QueuePanel {
    /// Create a new queue panel, scrolling to the currently playing song.
    pub fn new(current_index: usize, total: usize) -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(current_index));
        Self {
            list_state,
            total,
            current_index,
        }
    }

    /// Update the current playing index (called when the queue advances).
    pub fn set_current_index(&mut self, index: usize) {
        self.current_index = index;
    }

    /// Compute a centered floating area for the overlay.
    pub fn centered_rect(percent_x: u16, percent_y: u16, area: Rect) -> Rect {
        let popup_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Percentage((100 - percent_y) / 2),
                Constraint::Percentage(percent_y),
                Constraint::Percentage((100 - percent_y) / 2),
            ])
            .split(area);

        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage((100 - percent_x) / 2),
                Constraint::Percentage(percent_x),
                Constraint::Percentage((100 - percent_x) / 2),
            ])
            .split(popup_layout[1])[1]
    }

    /// Render the queue panel as a floating overlay.
    pub fn render(&self, f: &mut Frame, area: Rect, songs: &[Song]) {
        let panel_area = Self::centered_rect(60, 70, area);

        // Clear the area behind the floating panel.
        f.render_widget(Clear, panel_area);

        // Build the title with queue position info.
        let selected = self.list_state.selected().unwrap_or(0);
        let title_text = if self.total > 0 {
            format!(" ♫ Queue  ({}/{}) ", selected + 1, self.total)
        } else {
            " ♫ Queue  (empty) ".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT_DIM))
            .title(Span::styled(
                title_text,
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));

        if songs.is_empty() {
            let empty = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                "  Queue is empty",
                Style::default().fg(Theme::MUTED),
            )))
            .block(block);
            f.render_widget(empty, panel_area);
            return;
        }

        let items: Vec<ListItem> = songs
            .iter()
            .enumerate()
            .map(|(i, song)| {
                let artists = song
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                let duration = format_ms(song.duration);

                let is_current = i == self.current_index;
                let prefix = if is_current { "▶ " } else { "  " };

                let name_style = if is_current {
                    Style::default()
                        .fg(Theme::ACCENT)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::FG)
                };

                let artist_style = if is_current {
                    Style::default()
                        .fg(Theme::ACCENT_DIM)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::MUTED)
                };

                let duration_style = if is_current {
                    Style::default()
                        .fg(Theme::ACCENT_DIM)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::MUTED)
                };

                ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("{prefix}{:<width$}", song.name, width = 30),
                        name_style,
                    ),
                    Span::styled(format!("  {:<width$}", artists, width = 20), artist_style),
                    Span::styled(format!("  {duration}"), duration_style),
                ]))
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .highlight_style(Theme::selection())
            .highlight_symbol("❯ ");

        f.render_stateful_widget(list, panel_area, &mut self.list_state.clone());
    }

    /// Handle a key event while the queue panel is open.
    pub fn handle_event(&mut self, evt: &Event) -> QueuePanelResult {
        let Event::Key(k) = evt else {
            return QueuePanelResult::NotHandled;
        };
        if k.kind != KeyEventKind::Press {
            return QueuePanelResult::NotHandled;
        }

        match k.code {
            KeyCode::Esc | KeyCode::Char('q') => QueuePanelResult::Close,
            KeyCode::Down | KeyCode::Char('j') => {
                if self.total > 0 {
                    let i = self.list_state.selected().unwrap_or(0);
                    self.list_state.select(Some((i + 1) % self.total));
                }
                QueuePanelResult::NotHandled
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.total > 0 {
                    let i = self.list_state.selected().unwrap_or(0);
                    self.list_state
                        .select(Some(i.checked_sub(1).unwrap_or(self.total - 1)));
                }
                QueuePanelResult::NotHandled
            }
            KeyCode::Enter => {
                if let Some(idx) = self.list_state.selected() {
                    QueuePanelResult::JumpTo(idx)
                } else {
                    QueuePanelResult::NotHandled
                }
            }
            _ => QueuePanelResult::NotHandled,
        }
    }
}

/// Format milliseconds as `m:ss`.
fn format_ms(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_ms() {
        assert_eq!(format_ms(0), "0:00");
        assert_eq!(format_ms(180_000), "3:00");
        assert_eq!(format_ms(215_500), "3:35");
        assert_eq!(format_ms(3_661_000), "61:01");
    }

    #[test]
    fn test_queue_panel_new() {
        let panel = QueuePanel::new(2, 10);
        assert_eq!(panel.current_index, 2);
        assert_eq!(panel.total, 10);
        assert_eq!(panel.list_state.selected(), Some(2));
    }

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let centered = QueuePanel::centered_rect(60, 70, area);
        // 60% of 100 = 60, offset = (100-60)/2 = 20
        assert_eq!(centered.x, 20);
        assert_eq!(centered.width, 60);
        // 70% of 50 = 35, top padding = (50-35)/2 = 7 (integer), but
        // ratatui splits vertically so top gets Percentage(15) = 7,
        // middle gets Percentage(70) = 35, y = 1 + 7 = 8.
        assert_eq!(centered.y, 1 + area.y + (area.height - 35) / 2);
        assert_eq!(centered.height, 35);
    }
}
