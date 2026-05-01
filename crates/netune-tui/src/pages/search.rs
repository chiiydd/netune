//! Search page — query input + results list.
//!
//! Two modes:
//! - **Input**: typing a search query, Enter triggers search.
//! - **Normal**: navigating results with j/k.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use netune_core::models::Song;

use crate::chrome::KeyHint;
use crate::pages::PageAction;
use crate::theme::Theme;

const SPINNER_FRAMES: &[&str] = &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SearchMode {
    Input,
    Normal,
}

pub struct SearchPage {
    mode: SearchMode,
    query: String,
    results: Vec<Song>,
    list_state: ListState,
    /// Whether a search request is currently in-flight.
    loading: bool,
    /// Spinner frame index for loading animation.
    spinner_idx: usize,
}

impl Default for SearchPage {
    fn default() -> Self {
        Self::new()
    }
}

impl SearchPage {
    pub fn new() -> Self {
        Self {
            mode: SearchMode::Input,
            query: String::new(),
            results: Vec::new(),
            list_state: ListState::default(),
            loading: false,
            spinner_idx: 0,
        }
    }

    /// Get the current query.
    pub fn query(&self) -> &str {
        &self.query
    }

    /// Set search results (called by App after API response).
    pub fn set_results(&mut self, results: Vec<Song>) {
        self.results = results;
        self.list_state.select(if self.results.is_empty() {
            None
        } else {
            Some(0)
        });
        self.mode = SearchMode::Normal;
        self.loading = false;
    }

    /// Set the loading spinner state.
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            self.spinner_idx = 0;
        }
    }

    /// Advance the spinner animation. Called from `Page::tick()`.
    pub fn tick(&mut self) {
        if self.loading {
            self.spinner_idx = (self.spinner_idx + 1) % SPINNER_FRAMES.len();
        }
    }

    /// Get the currently selected song.
    pub fn selected_song(&self) -> Option<&Song> {
        let idx = self.list_state.selected()?;
        self.results.get(idx)
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(3), Constraint::Min(1)])
            .split(area);

        self.render_input(f, chunks[0]);
        self.render_results(f, chunks[1]);
    }

    fn render_input(&self, f: &mut Frame, area: Rect) {
        let border_color = if self.mode == SearchMode::Input {
            Theme::ACCENT
        } else {
            Theme::ACCENT_DIM
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                " Search ",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));

        let cursor = if self.mode == SearchMode::Input {
            "▏"
        } else {
            ""
        };
        let input = Paragraph::new(Line::from(vec![
            Span::styled(&self.query, Style::default().fg(Theme::FG)),
            Span::styled(cursor, Style::default().fg(Theme::ACCENT)),
        ]))
        .block(block);

        f.render_widget(input, area);
    }

    fn render_results(&mut self, f: &mut Frame, area: Rect) {
        if self.loading {
            let frame = SPINNER_FRAMES[self.spinner_idx];
            let msg = format!("{frame} Searching…");
            let items = vec![ListItem::new(Line::from(Span::styled(
                format!("  {msg}"),
                Style::default().fg(Theme::ACCENT),
            )))];
            let list = List::new(items).block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::ACCENT_DIM))
                    .title(Span::styled(
                        " Results ",
                        Style::default()
                            .fg(Theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )),
            );
            f.render_widget(list, area);
            return;
        }
        let items: Vec<ListItem> = if self.results.is_empty() {
            let msg = if self.query.is_empty() {
                "Type a query and press Enter"
            } else {
                "No results"
            };
            vec![ListItem::new(Line::from(Span::styled(
                format!("  {msg}"),
                Style::default().fg(Theme::MUTED),
            )))]
        } else {
            self.results
                .iter()
                .map(|song| {
                    let artists = song
                        .artists
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    ListItem::new(vec![
                        Line::from(vec![
                            Span::raw("  "),
                            Span::styled(&song.name, Style::default().add_modifier(Modifier::BOLD)),
                        ]),
                        Line::from(vec![
                            Span::raw("    "),
                            Span::styled(
                                format!("{artists} — {}", song.album.name),
                                Style::default().fg(Theme::MUTED),
                            ),
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
                        " Results ",
                        Style::default()
                            .fg(Theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Theme::selection())
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, area, &mut self.list_state);
    }

    // ── Events ──────────────────────────────────────────────────────────────

    pub fn handle_event(&mut self, evt: &Event) -> PageAction {
        let Event::Key(k) = evt else {
            return PageAction::None;
        };
        if k.kind != KeyEventKind::Press {
            return PageAction::None;
        }

        match self.mode {
            SearchMode::Input => self.handle_input(k.code),
            SearchMode::Normal => self.handle_normal(k.code),
        }
    }

    fn handle_input(&mut self, code: KeyCode) -> PageAction {
        match code {
            KeyCode::Esc => {
                self.mode = SearchMode::Normal;
            }
            KeyCode::Enter => {
                if !self.query.is_empty() {
                    return PageAction::Search(self.query.clone());
                }
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char(c) => {
                self.query.push(c);
            }
            _ => {}
        }
        PageAction::None
    }

    fn handle_normal(&mut self, code: KeyCode) -> PageAction {
        let len = self.results.len();
        match code {
            KeyCode::Esc => {
                return PageAction::Pop;
            }
            KeyCode::Char('i') => {
                self.mode = SearchMode::Input;
            }
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
                if let Some(song) = self.selected_song().cloned() {
                    return PageAction::PlaySong(song);
                }
            }
            _ => {}
        }
        PageAction::None
    }

    // ── Chrome contract ─────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        match self.mode {
            SearchMode::Input => ("INPUT".into(), Theme::MODE_SEARCH),
            SearchMode::Normal => ("NORMAL".into(), Theme::MODE_NORMAL),
        }
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        if self.query.is_empty() {
            vec![Span::styled(
                "empty query",
                Style::default().fg(Theme::MUTED),
            )]
        } else {
            vec![
                Span::styled("\"", Style::default().fg(Theme::MUTED)),
                Span::styled(self.query.clone(), Theme::accent_bold()),
                Span::styled("\"", Style::default().fg(Theme::MUTED)),
                Span::raw("  "),
                Span::styled(
                    format!("{} results", self.results.len()),
                    Style::default().fg(Theme::MUTED),
                ),
            ]
        }
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        match self.mode {
            SearchMode::Input => vec![KeyHint::new("⏎", "search"), KeyHint::new("Esc", "navigate")],
            SearchMode::Normal => vec![
                KeyHint::new("j/k", "move"),
                KeyHint::new("⏎", "play"),
                KeyHint::new("i", "edit"),
                KeyHint::new("Esc", "back"),
            ],
        }
    }
}
