//! Player page — full-screen now-playing view.
//!
//! Three zones:
//! - **Info**: song title + artist/album.
//! - **Lyrics**: scrollable lyric lines with current line highlighted.
//! - **Controls**: progress bar + elapsed/duration + volume.

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Gauge, Paragraph, Wrap};
use ratatui::Frame;

use netune_core::models::Song;
use netune_core::models::Lyrics;
use netune_player::PlayMode;

use crate::chrome::KeyHint;
use crate::pages::PageAction;
use crate::theme::Theme;

pub struct PlayerPage {
    song: Option<Song>,
    progress: f64,
    elapsed: Duration,
    duration: Duration,
    is_playing: bool,
    loading: bool,
    /// Tick counter for the loading spinner animation.
    loading_tick: usize,
    lyrics: Option<Lyrics>,
    current_lyric_idx: usize,
    volume: u16,
    play_mode: PlayMode,
}

impl Default for PlayerPage {
    fn default() -> Self {
        Self::new()
    }
}

impl PlayerPage {
    pub fn new() -> Self {
        Self {
            song: None,
            progress: 0.0,
            elapsed: Duration::ZERO,
            duration: Duration::ZERO,
            is_playing: false,
            loading: false,
            loading_tick: 0,
            lyrics: None,
            current_lyric_idx: 0,
            volume: 80,
            play_mode: PlayMode::Sequential,
        }
    }

    /// Set the currently playing song and reset state.
    pub fn set_song(&mut self, song: Song) {
        self.duration = Duration::from_millis(song.duration);
        self.song = Some(song);
        self.progress = 0.0;
        self.elapsed = Duration::ZERO;
        self.is_playing = true;
        // Don't clear lyrics here — they will be set separately via set_lyrics.
        // Clearing would lose lyrics fetched concurrently with the URL.
        self.current_lyric_idx = 0;
    }

    /// Update playback state from the real player.
    pub fn update_from_player(&mut self, position_secs: f64, duration_secs: f64, is_playing: bool) {
        self.elapsed = Duration::from_secs_f64(position_secs);
        if duration_secs > 0.0 {
            self.duration = Duration::from_secs_f64(duration_secs);
        }
        self.is_playing = is_playing;
        self.update_progress();
    }

    /// Set volume display (0-100).
    pub fn set_volume(&mut self, vol: u16) {
        self.volume = vol;
    }

    /// Set lyrics (timestamped, from the API).
    pub fn set_lyrics(&mut self, lyrics: Lyrics) {
        self.lyrics = Some(lyrics);
        self.current_lyric_idx = 0;
    }

    /// Set loading state (shown while audio is buffering).
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    /// Set the current play mode (for UI display).
    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
    }

    /// Get the current song (for context display).
    pub fn song(&self) -> Option<&Song> {
        self.song.as_ref()
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(4),
                Constraint::Min(3),
                Constraint::Length(5),
            ])
            .split(area);

        self.render_info(f, chunks[0]);

        if self.loading {
            const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let frame = SPINNER[self.loading_tick % SPINNER.len()];
            let loading_text = Paragraph::new(Line::from(vec![
                Span::styled(
                    format!("  {frame} "),
                    Style::default().fg(Theme::ACCENT),
                ),
                Span::styled(
                    "Loading audio...",
                    Style::default()
                        .fg(Theme::MUTED)
                        .add_modifier(Modifier::ITALIC),
                ),
            ]))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::ACCENT_DIM))
                    .title(Span::styled(
                        " ♫ Lyrics ",
                        Style::default()
                            .fg(Theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )),
            );
            f.render_widget(loading_text, chunks[1]);
        } else {
            self.render_lyrics(f, chunks[1]);
        }

        self.render_controls(f, chunks[2]);
    }

    fn render_info(&self, f: &mut Frame, area: Rect) {
        let (title, artists, album) = match &self.song {
            Some(song) => {
                let artists = song
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                (song.name.clone(), artists, song.album.name.clone())
            }
            None => ("No song playing".to_string(), String::new(), String::new()),
        };

        let status_icon = if self.is_playing { "▶" } else { "⏸" };

        let lines = vec![
            Line::from(vec![
                Span::styled(
                    format!(" {status_icon}  "),
                    Style::default()
                        .fg(if self.is_playing {
                            Theme::PLAYING
                        } else {
                            Theme::MUTED
                        })
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled(
                    title,
                    Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::raw("     "),
                Span::styled(artists, Style::default().fg(Theme::ACCENT)),
                if album.is_empty() {
                    Span::raw("")
                } else {
                    Span::styled(format!(" — {album}"), Style::default().fg(Theme::MUTED))
                },
            ]),
        ];

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT_DIM))
            .title(Span::styled(
                " Now Playing ",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));

        f.render_widget(Paragraph::new(lines).block(block), area);
    }

    fn render_lyrics(&self, f: &mut Frame, area: Rect) {
        let inner_height = area.height.saturating_sub(2) as usize;

        let lines: Vec<Line> = match &self.lyrics {
            None => vec![Line::from(Span::styled(
                "  Loading lyrics…",
                Style::default().fg(Theme::MUTED),
            ))],
            Some(lyrics) if lyrics.lines.is_empty() => vec![Line::from(Span::styled(
                "  No lyrics available",
                Style::default().fg(Theme::MUTED),
            ))],
            Some(lyrics) => {
                let total = lyrics.lines.len();
                let idx = self.current_lyric_idx.min(total.saturating_sub(1));

                // Center current lyric line in the visible area.
                let half = inner_height / 2;
                let start = idx.saturating_sub(half);
                let end = (start + inner_height).min(total);

                lyrics.lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        let abs_idx = start + i;
                        let text = &line.text;
                        if abs_idx == idx {
                            Line::from(Span::styled(
                                format!("▶ {text}"),
                                Style::default()
                                    .fg(Theme::ACCENT)
                                    .add_modifier(Modifier::BOLD),
                            ))
                        } else {
                            Line::from(Span::styled(
                                format!("  {text}"),
                                Style::default().fg(Theme::MUTED),
                            ))
                        }
                    })
                    .collect()
            }
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT_DIM))
            .title(Span::styled(
                " Lyrics ",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));

        f.render_widget(
            Paragraph::new(lines)
                .block(block)
                .wrap(Wrap { trim: false }),
            area,
        );
    }

    fn render_controls(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(1),
                Constraint::Length(2),
            ])
            .split(area);

        self.render_progress(f, chunks[0]);
        self.render_play_mode(f, chunks[1]);
        self.render_volume(f, chunks[2]);
    }

    fn render_progress(&self, f: &mut Frame, area: Rect) {
        let elapsed_str = format_duration(self.elapsed);
        let duration_str = format_duration(self.duration);
        let label = format!("{elapsed_str} / {duration_str}");

        let gauge = Gauge::default()
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::ACCENT_DIM)),
            )
            .gauge_style(Style::default().fg(Theme::ACCENT).bg(Color::Black))
            .ratio(self.progress)
            .label(label);

        f.render_widget(gauge, area);
    }

    fn render_play_mode(&self, f: &mut Frame, area: Rect) {
        let (icon, label, color) = match self.play_mode {
            PlayMode::Sequential => ("▶▷", "顺序", Theme::MUTED),
            PlayMode::LoopAll => ("🔁", "全部循环", Theme::ACCENT),
            PlayMode::LoopOne => ("🔂", "单曲循环", Theme::WARNING),
            PlayMode::Shuffle => ("🔀", "随机", Theme::SUCCESS),
        };

        let line = Line::from(vec![
            Span::styled("  ", Style::default()),
            Span::styled(icon, Style::default().fg(color)),
            Span::styled(" ", Style::default()),
            Span::styled(label, Style::default().fg(color).add_modifier(Modifier::BOLD)),
        ]);

        f.render_widget(Paragraph::new(line), area);
    }

    fn render_volume(&self, f: &mut Frame, area: Rect) {
        let vol = self.volume as usize;
        let bar_width = area.width.saturating_sub(4) as usize;
        let filled = (vol * bar_width) / 100;
        let empty = bar_width.saturating_sub(filled);

        let bar_line = Line::from(vec![
            Span::styled(" 🔊 ", Style::default().fg(Theme::MUTED)),
            Span::styled("━".repeat(filled), Style::default().fg(Theme::ACCENT)),
            Span::styled("─".repeat(empty), Style::default().fg(Theme::MUTED)),
            Span::styled(format!(" {vol}%"), Style::default().fg(Theme::FG_DIM)),
        ]);

        f.render_widget(Paragraph::new(bar_line), area);
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
            KeyCode::Esc | KeyCode::Char('q') => PageAction::Pop,
            KeyCode::Char(' ') => PageAction::TogglePause,
            KeyCode::Char('m') => PageAction::CyclePlayMode,
            KeyCode::Left => {
                PageAction::Seek(-5.0)
            }
            KeyCode::Right => {
                PageAction::Seek(5.0)
            }
            KeyCode::Up => {
                PageAction::SetVolume(self.volume.saturating_add(5).min(100))
            }
            KeyCode::Down => {
                PageAction::SetVolume(self.volume.saturating_sub(5))
            }
            KeyCode::Tab => PageAction::ToggleQueuePanel,
            _ => PageAction::None,
        }
    }

    // ── Tick ────────────────────────────────────────────────────────────────

    pub fn tick(&mut self) {
        // Advance loading spinner animation.
        if self.loading {
            self.loading_tick = self.loading_tick.wrapping_add(1);
        }
        if !self.is_playing {
            return;
        }
        // Advance by ~100 ms (the app tick interval).
        let tick_dur = Duration::from_millis(100);
        self.elapsed = (self.elapsed + tick_dur).min(self.duration);
        self.update_progress();
        self.update_lyric_idx();
    }

    fn update_progress(&mut self) {
        if self.duration.is_zero() {
            self.progress = 0.0;
        } else {
            self.progress = self.elapsed.as_secs_f64() / self.duration.as_secs_f64();
            self.progress = self.progress.clamp(0.0, 1.0);
        }
    }

    fn update_lyric_idx(&mut self) {
        let Some(ref lyrics) = self.lyrics else {
            return;
        };
        if lyrics.lines.is_empty() {
            return;
        }
        let position_ms = self.elapsed.as_millis() as u64;
        // Find the last lyric line whose timestamp <= current position.
        self.current_lyric_idx = lyrics
            .lines
            .iter()
            .position(|line| line.timestamp > position_ms)
            .unwrap_or(lyrics.lines.len())
            .saturating_sub(1);
    }

    // ── Chrome contract ─────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        if self.is_playing {
            ("PLAYING".into(), Theme::MODE_PLAYING)
        } else {
            ("PAUSED".into(), Theme::MODE_NORMAL)
        }
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        match &self.song {
            Some(song) => {
                let artists = song
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                vec![
                    Span::styled(song.name.clone(), Theme::accent_bold()),
                    Span::styled(format!(" — {artists}"), Style::default().fg(Theme::MUTED)),
                ]
            }
            None => vec![Span::styled("no song", Style::default().fg(Theme::MUTED))],
        }
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("Space", "pause"),
            KeyHint::new("←/→", "seek"),
            KeyHint::new("↑/↓", "vol"),
            KeyHint::new("m", "mode"),
            KeyHint::new("Esc", "back"),
        ]
    }
}

/// Format a `Duration` as `m:ss`.
fn format_duration(d: Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}
