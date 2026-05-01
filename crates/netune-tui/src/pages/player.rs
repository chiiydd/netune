//! Player page — MP3 player device design.
//!
//! A classic MP3 player rendered with box-drawing characters:
//! - Song title + artist in the display area
//! - Progress bar with elapsed/total time
//! - Playback controls (prev, play/pause, next, shuffle)
//! - Volume bar
//! - Lyrics scroll below the player

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

use unicode_width::UnicodeWidthStr;

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
                Constraint::Length(14), // MP3 player box
                Constraint::Length(1), // separator
                Constraint::Min(3),    // lyrics
            ])
            .split(area);

        self.render_player(f, chunks[1]);

        // separator
        let sep = "─".repeat(area.width as usize);
        f.render_widget(
            Paragraph::new(Span::styled(sep, Style::default().fg(Theme::ACCENT_DIM))),
            chunks[2],
        );

        self.render_lyrics(f, chunks[3]);
    }

    /// Render the MP3 player device centered in the area.
    ///
    /// ```text
    ///  ╭──────────────────────────────────────────────╮
    ///  │                                              │
    ///  │          Song Title Here                     │
    ///  │          Artist Name · Album Name            │
    ///  │                                              │
    ///  │     ▶ ━━━━━━━━━━━━━●━━━━━━━━━━━━━━━━        │
    ///  │              1:23 / 3:45                     │
    ///  │                                              │
    ///  │           ◄◄   ▶/❚❚   ►►   🔀              │
    ///  │                                              │
    ///  │       vol ████████░░ 80%                     │
    ///  │                                              │
    ///  ╰──────────────────────────────────────────────╯
    ///  ```
    fn render_player(&self, f: &mut Frame, area: Rect) {
        let (title, artists, album) = self.song_info();
        let bc = Theme::ACCENT_DIM;
        let box_w = 48usize; // inner content width
        let border = format!("╭{}╮", "─".repeat(box_w));
        let bottom_border = format!("╰{}╯", "─".repeat(box_w));

        let mut lines: Vec<Line> = Vec::with_capacity(14);

        // Line 0: top border
        lines.push(Line::from(Span::styled(border, Style::default().fg(bc))));

        // Line 1: blank
        lines.push(self.make_boxed_line("", Style::default(), bc, box_w));

        if self.song.is_none() {
            // No song
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));
            lines.push(self.make_boxed_line("No song playing", Style::default().fg(Theme::MUTED), bc, box_w));
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));
        } else {
            // ── Song info (always shown when song exists) ──
            // Line 2: title
            lines.push(self.make_boxed_line(
                &title,
                Style::default().fg(Theme::FG).add_modifier(Modifier::BOLD),
                bc,
                box_w,
            ));

            // Line 3: artist · album
            let info = if album.is_empty() {
                artists.clone()
            } else {
                format!("{artists} · {album}")
            };
            lines.push(self.make_boxed_line(&info, Style::default().fg(Theme::ACCENT), bc, box_w));

            // Line 4: blank
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));

            // ── Progress bar or loading spinner ──
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));

            if self.loading {
                const SPINNER: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
                let frame = SPINNER[self.loading_tick % SPINNER.len()];
                let bar_width = 36usize;
                let spinner_text = format!("{frame} Loading...");
                let spinner_w = display_width(&spinner_text);
                let pad = bar_width.saturating_sub(spinner_w);
                let left = pad / 2;
                let right = pad - left;
                let prog_line = Line::from(vec![
                    Span::styled(" ".repeat(left), Style::default()),
                    Span::styled(spinner_text, Style::default().fg(Theme::ACCENT)),
                    Span::styled(" ".repeat(right), Style::default()),
                ]);
                lines.push(self.make_boxed_line_spans(prog_line, bc, box_w));
                lines.push(self.make_boxed_line("--:-- / --:--", Style::default().fg(Theme::FG_DIM), bc, box_w));
            } else {
                let bar_width = 36usize;
                let elapsed_str = format_duration(self.elapsed);
                let total_str = format_duration(self.duration);
                let time_str = format!("{elapsed_str} / {total_str}");

                let filled = (self.progress * bar_width as f64) as usize;
                let filled = filled.min(bar_width);
                let empty = bar_width.saturating_sub(filled);
                let filled_bar: String = "━".repeat(filled.saturating_sub(1));
                let empty_bar: String = "░".repeat(empty);
                let prog_line = Line::from(vec![
                    Span::styled("▶ ", Style::default().fg(Theme::ACCENT)),
                    Span::styled(filled_bar, Style::default().fg(Theme::ACCENT)),
                    Span::styled("●", Style::default().fg(Theme::ACCENT)),
                    Span::styled(empty_bar, Style::default().fg(Theme::MUTED)),
                ]);
                lines.push(self.make_boxed_line_spans(prog_line, bc, box_w));
                lines.push(self.make_boxed_line(&time_str, Style::default().fg(Theme::FG_DIM), bc, box_w));
            }

            // Line 7: blank
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));

            // ── Playback controls ──
            let controls = if self.is_playing {
                format!("⏮  ⏸  ⏭  {}", self.play_mode_symbol())
            } else {
                format!("⏮  ▶  ⏭  {}", self.play_mode_symbol())
            };
            lines.push(self.make_boxed_line(&controls, Style::default().fg(Theme::MUTED), bc, box_w));

            // Line 9: blank
            lines.push(self.make_boxed_line("", Style::default(), bc, box_w));

            // ── Volume bar ──
            let vol_bar_w = 16usize;
            let vol_filled = (self.volume as usize * vol_bar_w) / 100;
            let vol_empty = vol_bar_w.saturating_sub(vol_filled);
            let vol_str = format!(
                "vol {}{} {}%",
                "█".repeat(vol_filled),
                "░".repeat(vol_empty),
                self.volume
            );
            lines.push(self.make_boxed_line(&vol_str, Style::default().fg(Theme::FG_DIM), bc, box_w));
        }

        // Line 11: blank
        lines.push(self.make_boxed_line("", Style::default(), bc, box_w));

        // Line 12: bottom border
        lines.push(Line::from(Span::styled(bottom_border, Style::default().fg(bc))));

        // Center vertically and render
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

    /// Helper: build a box row `│ text... │` with centered-ish padding.
    fn make_boxed_line(&self, text: &str, style: Style, bc: ratatui::style::Color, box_w: usize) -> Line<'static> {
        let text_w = display_width(text);
        let total_pad = box_w.saturating_sub(text_w);
        let left_pad = total_pad / 2;
        let right_pad = total_pad - left_pad;
        Line::from(vec![
            Span::styled("│", Style::default().fg(bc)),
            Span::styled(" ".repeat(left_pad), Style::default()),
            Span::styled(text.to_string(), style),
            Span::styled(" ".repeat(right_pad), Style::default()),
            Span::styled("│", Style::default().fg(bc)),
        ])
    }

    /// Helper: embed a rich Line inside a box row.
    fn make_boxed_line_spans(&self, inner: Line<'static>, bc: ratatui::style::Color, box_w: usize) -> Line<'static> {
        let text_w: usize = inner.spans.iter().map(|s| display_width(&s.content)).sum();
        let total_pad = box_w.saturating_sub(text_w);
        let left_pad = total_pad / 2;
        let right_pad = total_pad - left_pad;
        let mut spans = vec![Span::styled("│", Style::default().fg(bc))];
        spans.push(Span::styled(" ".repeat(left_pad), Style::default()));
        spans.extend(inner.spans);
        spans.push(Span::styled(" ".repeat(right_pad), Style::default()));
        spans.push(Span::styled("│", Style::default().fg(bc)));
        Line::from(spans)
    }

    fn play_mode_symbol(&self) -> &'static str {
        match self.play_mode {
            PlayMode::Sequential => "→",
            PlayMode::LoopAll => "⟳",
            PlayMode::LoopOne => "⟳¹",
            PlayMode::Shuffle => "⤮",
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
            KeyCode::Char('n') => PageAction::PlayNext,
            KeyCode::Char('p') => PageAction::PlayPrev,
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
            KeyHint::new("n", "next"),
            KeyHint::new("p", "prev"),
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

fn display_width(s: &str) -> usize {
    UnicodeWidthStr::width(s)
}
