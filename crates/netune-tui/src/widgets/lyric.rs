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

#[cfg(test)]
mod tests {
    use super::*;
    use netune_core::models::{LyricLine, Lyrics};

    fn make_lyrics(timestamps: &[u64]) -> Lyrics {
        Lyrics {
            lines: timestamps
                .iter()
                .map(|&t| LyricLine {
                    timestamp: t,
                    text: format!("line at {t}"),
                })
                .collect(),
            translated: None,
        }
    }

    #[test]
    fn test_lyric_widget_new() {
        let w = LyricWidget::new();
        assert!(w.lyrics().is_none());
        assert_eq!(w.current_line(), 0);
    }

    #[test]
    fn test_lyric_widget_set_lyrics() {
        let mut w = LyricWidget::new();
        let lyrics = make_lyrics(&[0, 5000, 10000]);
        w.set_lyrics(lyrics);
        let stored = w.lyrics().unwrap();
        assert_eq!(stored.lines.len(), 3);
        assert_eq!(w.current_line(), 0);
    }

    #[test]
    fn test_lyric_widget_update_position() {
        let mut w = LyricWidget::new();
        w.set_lyrics(make_lyrics(&[0, 5000, 10000]));

        w.update_position(0);
        assert_eq!(w.current_line(), 0);

        w.update_position(3000);
        assert_eq!(w.current_line(), 0);

        w.update_position(5000);
        assert_eq!(w.current_line(), 1);

        w.update_position(7000);
        assert_eq!(w.current_line(), 1);

        w.update_position(10000);
        assert_eq!(w.current_line(), 2);

        w.update_position(999999);
        assert_eq!(w.current_line(), 2);
    }

    #[test]
    fn test_lyric_widget_update_no_lyrics() {
        let mut w = LyricWidget::new();
        w.update_position(5000);
        assert_eq!(w.current_line(), 0);
    }

    #[test]
    fn test_lyric_widget_update_before_first_line() {
        let mut w = LyricWidget::new();
        w.set_lyrics(make_lyrics(&[5000, 10000]));
        w.update_position(1000);
        assert_eq!(w.current_line(), 0);
    }
}
