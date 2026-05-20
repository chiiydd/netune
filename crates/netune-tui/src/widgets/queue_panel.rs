//! Queue panel — floating overlay showing the current play queue.
//!
//! Displays all songs in the queue with the currently playing song highlighted.
//! Supports selecting and jumping to a song via keyboard.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, List, ListItem, ListState};

use netune_core::models::Song;

use crate::theme::Theme;

/// Result of handling an event within the queue panel.
#[derive(Debug, PartialEq, Eq)]
pub enum QueuePanelResult {
    /// The panel consumed the event and wants to close.
    Close,
    /// The panel wants to jump to a specific queue index.
    JumpTo(usize),
    /// The panel wants to remove a specific queue index.
    Remove(usize),
    /// The panel wants to clear the whole queue.
    Clear,
    /// The panel wants to open the player page.
    GoPlayer,
    /// The event was not handled by the panel.
    NotHandled,
}

pub struct QueuePanel {
    list_state: ListState,
    total: usize,
    current_index: usize,
    confirm_clear: bool,
}

impl QueuePanel {
    /// Create a new queue panel, scrolling to the currently playing song.
    pub fn new(current_index: usize, total: usize) -> Self {
        let mut list_state = ListState::default();
        list_state.select(if total == 0 {
            None
        } else {
            Some(current_index.min(total - 1))
        });
        Self {
            list_state,
            total,
            current_index,
            confirm_clear: false,
        }
    }

    /// Update the current playing index (called when the queue advances).
    pub fn set_current_index(&mut self, index: usize) {
        self.current_index = index;
    }

    /// Update queue metadata after the backing queue changes.
    pub fn sync_queue_state(&mut self, current_index: usize, total: usize) {
        self.current_index = current_index;
        self.total = total;
        self.confirm_clear = false;
        self.list_state.select(if total == 0 {
            None
        } else {
            Some(
                self.list_state
                    .selected()
                    .unwrap_or(current_index)
                    .min(total - 1),
            )
        });
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
        let title_text = if self.confirm_clear {
            " ♫ Queue  Clear? y/n ".to_string()
        } else if self.total > 0 {
            format!(
                " ♫ Queue  ({}/{})  Enter play  d delete  D clear  p player ",
                selected + 1,
                self.total
            )
        } else {
            " ♫ Queue  (empty)  Tab close ".to_string()
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT_DIM()))
            .title(Span::styled(
                title_text,
                Style::default()
                    .fg(Theme::ACCENT())
                    .add_modifier(Modifier::BOLD),
            ));

        if songs.is_empty() {
            let empty = ratatui::widgets::Paragraph::new(Line::from(Span::styled(
                "  Queue is empty",
                Style::default().fg(Theme::MUTED()),
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
                        .fg(Theme::ACCENT())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::FG())
                };

                let artist_style = if is_current {
                    Style::default()
                        .fg(Theme::ACCENT_DIM())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::MUTED())
                };

                let duration_style = if is_current {
                    Style::default()
                        .fg(Theme::ACCENT_DIM())
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Theme::MUTED())
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

        if self.confirm_clear {
            match k.code {
                KeyCode::Char('y') | KeyCode::Char('Y') => {
                    self.confirm_clear = false;
                    return QueuePanelResult::Clear;
                }
                KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                    self.confirm_clear = false;
                    return QueuePanelResult::NotHandled;
                }
                _ => return QueuePanelResult::NotHandled,
            }
        }

        match k.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Tab => QueuePanelResult::Close,
            KeyCode::Char('p') => QueuePanelResult::GoPlayer,
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
            KeyCode::Char('d') => self
                .list_state
                .selected()
                .map(QueuePanelResult::Remove)
                .unwrap_or(QueuePanelResult::NotHandled),
            KeyCode::Char('D') => {
                self.confirm_clear = true;
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
    use crossterm::event::KeyEvent;

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
    fn queue_panel_empty_queue_has_no_selection() {
        let mut panel = QueuePanel::new(0, 0);

        assert_eq!(panel.list_state.selected(), None);
        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('d'))),
            QueuePanelResult::NotHandled
        );
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, crossterm::event::KeyModifiers::NONE))
    }

    #[test]
    fn queue_panel_d_removes_selected_item() {
        let mut panel = QueuePanel::new(1, 3);

        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('d'))),
            QueuePanelResult::Remove(1)
        );
    }

    #[test]
    fn queue_panel_shift_d_requires_confirmation_before_clear() {
        let mut panel = QueuePanel::new(0, 3);

        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('D'))),
            QueuePanelResult::NotHandled
        );
        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('y'))),
            QueuePanelResult::Clear
        );
    }

    #[test]
    fn queue_panel_clear_confirmation_can_be_cancelled() {
        let mut panel = QueuePanel::new(0, 3);

        panel.handle_event(&key(KeyCode::Char('D')));
        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('n'))),
            QueuePanelResult::NotHandled
        );
        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('y'))),
            QueuePanelResult::NotHandled
        );
    }

    #[test]
    fn queue_panel_p_opens_player() {
        let mut panel = QueuePanel::new(0, 3);

        assert_eq!(
            panel.handle_event(&key(KeyCode::Char('p'))),
            QueuePanelResult::GoPlayer
        );
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
