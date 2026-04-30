//! Playlist page — two views:
//!
//! - **List**: browse user playlists (name + track count).
//! - **Tracks**: browse songs in the selected playlist (name + artist + duration).

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use netune_core::models::{Playlist, Song};

use crate::chrome::KeyHint;
use crate::theme::Theme;
use crate::pages::PageAction;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PageView {
    List,
    Tracks,
}

pub struct PlaylistPage {
    view: PageView,
    playlists: Vec<Playlist>,
    tracks: Vec<Song>,
    list_state: ListState,
    tracks_state: ListState,
    /// Index into `playlists` of the currently selected playlist (for context display).
    selected_playlist: Option<usize>,
}

impl Default for PlaylistPage {
    fn default() -> Self {
        Self::new()
    }
}

impl PlaylistPage {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            view: PageView::List,
            playlists: Vec::new(),
            tracks: Vec::new(),
            list_state,
            tracks_state: ListState::default(),
            selected_playlist: None,
        }
    }

    /// Called when the API provides playlists (wired up later).
    pub fn set_playlists(&mut self, playlists: Vec<Playlist>) {
        self.playlists = playlists;
        if self.playlists.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    fn open_playlist(&mut self) {
        let Some(idx) = self.list_state.selected() else {
            return;
        };
        if self.playlists.get(idx).is_none() {
            return;
        }
        self.selected_playlist = Some(idx);
        // TODO: fetch tracks from API when wired up.
        self.tracks.clear();
        self.tracks_state = ListState::default();
        if !self.tracks.is_empty() {
            self.tracks_state.select(Some(0));
        }
        self.view = PageView::Tracks;
    }

    fn back_to_list(&mut self) {
        self.view = PageView::List;
        self.tracks.clear();
        // Restore list selection to the playlist we were viewing.
        if let Some(idx) = self.selected_playlist {
            self.list_state.select(Some(idx));
        }
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        match self.view {
            PageView::List => self.render_playlists(f, area),
            PageView::Tracks => self.render_tracks(f, area),
        }
    }

    fn render_playlists(&mut self, f: &mut Frame, area: Rect) {
        let items: Vec<ListItem> = if self.playlists.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "  No playlists loaded",
                Style::default().fg(Theme::MUTED),
            )))]
        } else {
            self.playlists
                .iter()
                .map(|pl| {
                    let track_label = if pl.track_count == 1 {
                        "1 track".to_string()
                    } else {
                        format!("{} tracks", pl.track_count)
                    };
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(
                                &pl.name,
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                        ]),
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(track_label, Style::default().fg(Theme::MUTED)),
                        ]),
                    ])
                })
                .collect()
        };

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::ACCENT_DIM))
                    .title(Span::styled(
                        " Playlists ",
                        Style::default()
                            .fg(Theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Theme::selection())
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    fn render_tracks(&mut self, f: &mut Frame, area: Rect) {
        let playlist_name = self
            .selected_playlist
            .and_then(|i| self.playlists.get(i))
            .map(|p| p.name.as_str())
            .unwrap_or("Playlist");

        let items: Vec<ListItem> = if self.tracks.is_empty() {
            vec![ListItem::new(Line::from(Span::styled(
                "  No tracks",
                Style::default().fg(Theme::MUTED),
            )))]
        } else {
            self.tracks
                .iter()
                .enumerate()
                .map(|(i, song)| {
                    let artists = song
                        .artists
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let duration = format_duration(song.duration);
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::styled(
                                format!("  {:3} ", i + 1),
                                Style::default().fg(Theme::MUTED),
                            ),
                            Span::styled(
                                &song.name,
                                Style::default().add_modifier(Modifier::BOLD),
                            ),
                            Span::raw("  "),
                            Span::styled(duration, Style::default().fg(Theme::FG_DIM)),
                        ]),
                        Line::from(vec![
                            Span::raw("       "),
                            Span::styled(artists, Style::default().fg(Theme::MUTED)),
                        ]),
                    ])
                })
                .collect()
        };

        let title = format!(" {} ", playlist_name);

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::ACCENT_DIM))
                    .title(Span::styled(
                        title,
                        Style::default()
                            .fg(Theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Theme::selection())
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.tracks_state);
    }

    // ── Events ──────────────────────────────────────────────────────────────

    pub fn handle_event(&mut self, evt: &Event) -> PageAction {
        let Event::Key(k) = evt else {
            return PageAction::None;
        };
        if k.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        match self.view {
            PageView::List => self.handle_list(k.code),
            PageView::Tracks => self.handle_tracks(k.code),
        }
    }

    fn handle_list(&mut self, code: KeyCode) -> PageAction {
        let len = self.playlists.len();
        match code {
            KeyCode::Down | KeyCode::Char('j') if len > 0 => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some((i + 1) % len));
            }
            KeyCode::Up | KeyCode::Char('k') if len > 0 => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state
                    .select(Some(i.checked_sub(1).unwrap_or(len - 1)));
            }
            KeyCode::Enter if len > 0 => {
                self.open_playlist();
            }
            KeyCode::Esc | KeyCode::Char('h') => return PageAction::Pop,
            _ => {}
        }
        PageAction::None
    }

    fn handle_tracks(&mut self, code: KeyCode) -> PageAction {
        let len = self.tracks.len();
        match code {
            KeyCode::Down | KeyCode::Char('j') if len > 0 => {
                let i = self.tracks_state.selected().unwrap_or(0);
                self.tracks_state.select(Some((i + 1) % len));
            }
            KeyCode::Up | KeyCode::Char('k') if len > 0 => {
                let i = self.tracks_state.selected().unwrap_or(0);
                self.tracks_state
                    .select(Some(i.checked_sub(1).unwrap_or(len - 1)));
            }
            KeyCode::Enter if len > 0 => {
                // TODO: play selected track via PageAction when player is wired up.
            }
            KeyCode::Esc | KeyCode::Char('h') => {
                self.back_to_list();
            }
            _ => {}
        }
        PageAction::None
    }

    // ── Chrome contract ─────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        match self.view {
            PageView::List => ("PLAYLISTS".into(), Theme::MODE_NORMAL),
            PageView::Tracks => ("TRACKS".into(), Theme::MODE_NORMAL),
        }
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        match self.view {
            PageView::List => {
                let count = self.playlists.len();
                vec![Span::styled(
                    format!("{count} playlists"),
                    Style::default().fg(Theme::MUTED),
                )]
            }
            PageView::Tracks => {
                let name = self
                    .selected_playlist
                    .and_then(|i| self.playlists.get(i))
                    .map(|p| p.name.as_str())
                    .unwrap_or("");
                vec![
                    Span::styled(name.to_owned(), Theme::accent_bold()),
                    Span::raw("  "),
                    Span::styled(
                        format!("{} tracks", self.tracks.len()),
                        Style::default().fg(Theme::MUTED),
                    ),
                ]
            }
        }
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        match self.view {
            PageView::List => vec![
                KeyHint::new("j/k", "move"),
                KeyHint::new("⏎", "open"),
                KeyHint::new("Esc", "back"),
            ],
            PageView::Tracks => vec![
                KeyHint::new("j/k", "move"),
                KeyHint::new("⏎", "play"),
                KeyHint::new("Esc/h", "back"),
            ],
        }
    }
}

/// Format milliseconds as `m:ss`.
fn format_duration(ms: u64) -> String {
    let total_secs = ms / 1000;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}
