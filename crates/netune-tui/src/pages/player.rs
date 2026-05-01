//! Player page — full-screen vinyl record player design.
//!
//! A decorative vinyl record sits centered in the page with:
//! - Song title + artist inside the record grooves
//! - Volume represented as groove density
//! - Tonearm tracks progress across the record
//! - Lyrics scroll below the record

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};
use ratatui::Frame;

use netune_core::models::Lyrics;
use netune_core::models::Song;
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
                Constraint::Length(1), // top spacer
                Constraint::Length(16), // vinyl record
                Constraint::Length(1), // separator
                Constraint::Min(3),    // lyrics
            ])
            .split(area);

        self.render_record(f, chunks[1]);

        // separator line
        let sep = "─".repeat(area.width as usize);
        f.render_widget(
            Paragraph::new(Span::styled(sep, Style::default().fg(Theme::ACCENT_DIM))),
            chunks[2],
        );

        self.render_lyrics(f, chunks[3]);
    }

    /// Build and render the vinyl record centered in the area.
    fn render_record(&self, f: &mut Frame, area: Rect) {
        let (title, artists, _album) = self.song_info();
        let time_str = format!(
            "{} / {}",
            format_duration(self.elapsed),
            format_duration(self.duration)
        );

        // Tonearm position: maps progress (0.0-1.0) to content rows 3-13
        let arm_row = ((self.progress * 10.0) as usize).clamp(0, 10) + 3;

        // Volume grooves: number of groove lines varies with volume
        let num_grooves = 1 + (self.volume as usize * 4) / 100; // 1-5 groove lines
        let groove_inner_w = 23; // width inside the record for grooves
        let filled = (self.volume as usize * groove_inner_w) / 100;
        let groove_line = format!("{}{}", "─ ".repeat(filled / 2), "─");

        // Build record lines (16 total)
        let mut lines: Vec<Line> = Vec::with_capacity(16);

        // Row 0: outer top cap
        lines.push(Line::from(Span::styled(
            format!("    ╭{}╮", "─".repeat(33)),
            Style::default().fg(Theme::ACCENT_DIM),
        )));
        // Row 1: second rim
        lines.push(Line::from(Span::styled(
            format!("  ╭─{}│─╮", "─".repeat(30)),
            Style::default().fg(Theme::ACCENT_DIM),
        )));
        // Row 2: third rim
        lines.push(Line::from(Span::styled(
            format!("╭─│─{}│─│╮", "─".repeat(28)),
            Style::default().fg(Theme::ACCENT_DIM),
        )));

        // Content rows (rows 3-13, 11 rows)
        let mut content: Vec<(String, bool)> = Vec::with_capacity(11);
        // Row 3: blank
        content.push((String::new(), false));
        // Row 4: title
        if self.loading {
            const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let frame = SPINNER[self.loading_tick % SPINNER.len()];
            content.push((format!("{frame} Loading..."), true));
        } else {
            content.push((title, true));
        }
        // Row 5: artist
        content.push((artists, true));
        // Row 6: blank
        content.push((String::new(), false));
        // Rows 7-(6+num_grooves): volume grooves
        for i in 0..num_grooves.min(5) {
            let indent = if i % 2 == 0 { "  " } else { "   " };
            content.push((format!("{indent}{groove_line}"), true));
        }
        // Fill remaining groove slots with blank to keep consistent row count
        for _ in num_grooves..5 {
            content.push((String::new(), false));
        }
        // Row 12: blank (after grooves)
        content.push((String::new(), false));
        // Row 13: time
        content.push((time_str, true));

        // Render content rows with side borders and optional tonearm
        for (i, (text, _)) in content.iter().enumerate() {
            let row = i + 3; // absolute row index
            let arm = if row == arm_row { "──╯" } else { "   " };
            let arm_style = if row == arm_row {
                Style::default().fg(Theme::MUTED)
            } else {
                Style::default()
            };

            let title_style = if row == 4 {
                Style::default()
                    .fg(Theme::FG)
                    .add_modifier(Modifier::BOLD)
            } else if row == 5 {
                Style::default().fg(Theme::ACCENT)
            } else if row == 13 {
                Style::default().fg(Theme::FG_DIM)
            } else {
                Style::default().fg(Theme::MUTED)
            };

            let padded = format!("{:<28}", text);
            lines.push(Line::from(vec![
                Span::styled("│  ", Style::default().fg(Theme::ACCENT_DIM)),
                Span::styled(padded, title_style),
                Span::styled("│", Style::default().fg(Theme::ACCENT_DIM)),
                Span::styled(arm, arm_style),
            ]));
        }

        // Row 14: third rim bottom
        lines.push(Line::from(Span::styled(
            format!("╰─│─{}│─│╯", "─".repeat(28)),
            Style::default().fg(Theme::ACCENT_DIM),
        )));
        // Row 15: second rim bottom
        lines.push(Line::from(Span::styled(
            format!("  ╰─{}│─╯", "─".repeat(30)),
            Style::default().fg(Theme::ACCENT_DIM),
        )));
        // Row 16: outer bottom cap
        lines.push(Line::from(Span::styled(
            format!("    ╰{}╯", "─".repeat(33)),
            Style::default().fg(Theme::ACCENT_DIM),
        )));

        // Center vertically and render line by line
        let total = lines.len() as u16;
        let start_y = area.y + area.height.saturating_sub(total) / 2;
        for (i, line) in lines.into_iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.y && y < area.y + area.height {
                f.render_widget(
                    Paragraph::new(line),
                    Rect::new(area.x, y, area.width, 1),
                );
            }
        }
    }

    /// Extract (title, artists, album) from the current song.
    fn song_info(&self) -> (String, String, String) {
        match &self.song {
            Some(song) => {
                let artists = song
                    .artists
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                (song.name.clone(), artists, song.album.name.clone())
            }
            None => (
                "No song playing".to_string(),
                String::new(),
                String::new(),
            ),
        }
    }

    /// Lyrics with current line highlighted, no border.
    fn render_lyrics(&self, f: &mut Frame, area: Rect) {
        let inner_height = area.height as usize;

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

        f.render_widget(Paragraph::new(lines).wrap(Wrap { trim: false }), area);
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
            KeyCode::Left => PageAction::Seek(-5.0),
            KeyCode::Right => PageAction::Seek(5.0),
            KeyCode::Up => PageAction::SetVolume(self.volume.saturating_add(5).min(100)),
            KeyCode::Down => PageAction::SetVolume(self.volume.saturating_sub(5)),
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

    pub fn mode(&self) -> (String, ratatui::style::Color) {
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
