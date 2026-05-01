//! Player page — full-screen vinyl record player design.
//!
//! A decorative vinyl record sits centered in the page with:
//! - Song title + artist inside the record grooves
//! - Volume represented as groove density
//! - Tonearm tracks progress across the record
//! - Lyrics scroll below the record

use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
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

    pub fn set_song(&mut self, song: Song) {
        self.duration = Duration::from_millis(song.duration);
        self.song = Some(song);
        self.progress = 0.0;
        self.elapsed = Duration::ZERO;
        self.is_playing = true;
        self.current_lyric_idx = 0;
    }

    pub fn update_from_player(&mut self, position_secs: f64, duration_secs: f64, is_playing: bool) {
        self.elapsed = Duration::from_secs_f64(position_secs);
        if duration_secs > 0.0 {
            self.duration = Duration::from_secs_f64(duration_secs);
        }
        self.is_playing = is_playing;
        self.update_progress();
    }

    pub fn set_volume(&mut self, vol: u16) {
        self.volume = vol;
    }

    pub fn set_lyrics(&mut self, lyrics: Lyrics) {
        self.lyrics = Some(lyrics);
        self.current_lyric_idx = 0;
    }

    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
    }

    pub fn set_play_mode(&mut self, mode: PlayMode) {
        self.play_mode = mode;
    }

    pub fn song(&self) -> Option<&Song> {
        self.song.as_ref()
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&self, f: &mut Frame, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // top spacer
                Constraint::Length(18), // vinyl record (3 top + 12 content + 3 bottom)
                Constraint::Length(1), // separator
                Constraint::Min(3),    // lyrics
            ])
            .split(area);

        self.render_record(f, chunks[1]);

        // separator
        let sep = "─".repeat(area.width as usize);
        f.render_widget(
            Paragraph::new(Span::styled(sep, Style::default().fg(Theme::ACCENT_DIM))),
            chunks[2],
        );

        self.render_lyrics(f, chunks[3]);
    }

    /// Render the vinyl record centered in the area.
    ///
    /// Record layout (fixed 41 chars wide including tonearm):
    ///
    /// ```text
    ///          ╭─────────────────────╮
    ///        ╭─│─────────────────────│─╮
    ///      ╭─│─│─────────────────────│─│─╮
    ///      │ │ │                     │ │ │
    ///      │ │ │  Song Title         │ │ │  ╮
    ///      │ │ │  Artist · Album     │ │ │  │
    ///      │ │ │                     │ │ │  │
    ///      │ │ │   ─ ─ ─ ─ ─ ─      │ │ │  │
    ///      │ │ │    ─ ─ ─ ─ ─       │ │ │  │
    ///      │ │ │   ─ ─ ─ ─ ─ ─      │ │ │  │
    ///      │ │ │    ─ ─ ─ ─ ─       │ │ │  │
    ///      │ │ │   ─ ─ ─ ─ ─ ─      │ │ │  │
    ///      │ │ │                     │ │ │  │
    ///      │ │ │  0:45 / 3:45       │ │ │  │
    ///      │ │ │                     │ │ │  │
    ///      ╰─│─│─────────────────────│─│─╯  │
    ///        ╰─│─────────────────────│─╯    │
    ///          ╰─────────────────────╯      ╯
    /// ```
    fn render_record(&self, f: &mut Frame, area: Rect) {
        let (title, artists, _album) = self.song_info();
        let time_str = format!(
            "{} / {}",
            format_duration(self.elapsed),
            format_duration(self.duration)
        );

        // Tonearm: maps progress 0.0-1.0 to content rows 3-14
        let arm_row = ((self.progress * 11.0) as usize).clamp(0, 11) + 3;

        // Volume grooves
        let num_grooves = 1 + (self.volume as usize * 4) / 100; // 1-5
        let filled = ((self.volume as f64 / 100.0) * 10.0) as usize;
        let groove = if filled > 0 {
            "─ ".repeat(filled).trim_end().to_string()
        } else {
            "─".to_string()
        };

        let bc = Theme::ACCENT_DIM;
        let groove_w = 21; // content width inside record borders

        // Helper: content row with borders + optional tonearm
        let make_row = |row: usize, text: &str, style: Style| -> Line<'static> {
            let padded = format!("{:<width$}", text, width = groove_w);
            let arm = if row == arm_row {
                "  ╮"
            } else if row > arm_row && row <= 14 {
                "  │"
            } else {
                "   "
            };
            Line::from(vec![
                Span::styled("      │ │ │ ", Style::default().fg(bc)),
                Span::styled(padded, style),
                Span::styled(" │ │ │", Style::default().fg(bc)),
                Span::styled(arm.to_string(), Style::default().fg(Theme::MUTED)),
            ])
        };

        let blank = || make_row(0, "", Style::default());

        let mut lines: Vec<Line> = Vec::with_capacity(18);

        // ── Top borders ──
        lines.push(Line::from(Span::styled(
            format!("          ╭{}╮", "─".repeat(groove_w)),
            Style::default().fg(bc),
        )));
        lines.push(Line::from(Span::styled(
            format!("        ╭─│{}│─╮", "─".repeat(groove_w)),
            Style::default().fg(bc),
        )));
        lines.push(Line::from(Span::styled(
            format!("      ╭─│─│{}│─│─╮", "─".repeat(groove_w)),
            Style::default().fg(bc),
        )));

        // ── Content rows (12 rows: index 3-14) ──
        // Row 3: blank
        lines.push(blank());

        // Row 4: title
        if self.loading {
            const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
            let frame = SPINNER[self.loading_tick % SPINNER.len()];
            lines.push(make_row(
                4,
                &format!("{frame} Loading..."),
                Style::default().fg(Theme::ACCENT),
            ));
        } else {
            lines.push(make_row(
                4,
                &title,
                Style::default()
                    .fg(Theme::FG)
                    .add_modifier(Modifier::BOLD),
            ));
        }

        // Row 5: artist
        lines.push(make_row(5, &artists, Style::default().fg(Theme::ACCENT)));

        // Row 6: blank
        lines.push(make_row(6, "", Style::default()));

        // Rows 7-11: volume grooves (up to 5 lines)
        for i in 0..5 {
            let row = 7 + i;
            if i < num_grooves.min(5) {
                let indent = if i % 2 == 0 { " " } else { "  " };
                let g = format!("{}{}", indent, groove);
                lines.push(make_row(row, &g, Style::default().fg(Theme::MUTED)));
            } else {
                lines.push(make_row(row, "", Style::default()));
            }
        }

        // Row 12: blank
        lines.push(make_row(12, "", Style::default()));

        // Row 13: time
        lines.push(make_row(13, &time_str, Style::default().fg(Theme::FG_DIM)));

        // Row 14: blank
        lines.push(make_row(14, "", Style::default()));

        // ── Bottom borders (mirror of top) ──
        lines.push(Line::from(Span::styled(
            format!("      ╰─│─│{}│─│─╯", "─".repeat(groove_w)),
            Style::default().fg(bc),
        )));
        lines.push(Line::from(Span::styled(
            format!("        ╰─│{}│─╯", "─".repeat(groove_w)),
            Style::default().fg(bc),
        )));
        lines.push(Line::from(Span::styled(
            format!("          ╰{}╯", "─".repeat(groove_w)),
            Style::default().fg(bc),
        )));

        // Center vertically and render line by line
        let total = lines.len() as u16;
        let start_y = area.y + area.height.saturating_sub(total) / 2;
        for (i, line) in lines.into_iter().enumerate() {
            let y = start_y + i as u16;
            if y >= area.y && y < area.y + area.height {
                f.render_widget(
                    Paragraph::new(line).alignment(Alignment::Center),
                    Rect::new(area.x, y, area.width, 1),
                );
            }
        }
    }

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

    /// Lyrics with current line highlighted, centered horizontally.
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

        f.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .alignment(Alignment::Center),
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
        if self.loading {
            self.loading_tick = self.loading_tick.wrapping_add(1);
        }
        if !self.is_playing {
            return;
        }
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
