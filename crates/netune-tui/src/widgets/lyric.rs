//! Lyrics display widget.

use netune_core::models::Lyrics;

/// State for the lyrics display widget.
pub struct LyricWidget {
    lyrics: Option<Lyrics>,
    current_line: usize,
}

impl LyricWidget {
    pub fn new() -> Self {
        Self {
            lyrics: None,
            current_line: 0,
        }
    }

    pub fn set_lyrics(&mut self, lyrics: Lyrics) {
        self.lyrics = Some(lyrics);
        self.current_line = 0;
    }

    /// Update current line based on playback position (in ms).
    pub fn update_position(&mut self, position_ms: u64) {
        if let Some(ref lyrics) = self.lyrics {
            self.current_line = lyrics
                .lines
                .iter()
                .position(|line| line.timestamp > position_ms)
                .unwrap_or(lyrics.lines.len())
                .saturating_sub(1);
        }
    }

    pub fn current_line(&self) -> usize {
        self.current_line
    }

    pub fn lyrics(&self) -> Option<&Lyrics> {
        self.lyrics.as_ref()
    }
}
