//! Playback progress bar widget.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;

/// A horizontal progress bar showing playback position.
pub struct ProgressBar {
    /// Current position (0.0 - 1.0).
    pub progress: f64,
    /// Elapsed time text (e.g., "1:23").
    pub elapsed: String,
    /// Total duration text (e.g., "3:45").
    pub total: String,
}

impl Widget for ProgressBar {
    fn render(self, area: Rect, _buf: &mut Buffer) {
        if area.height == 0 || area.width < 20 {
            return;
        }
        // TODO: B组(Claude Code) — implement progress bar rendering
        // Format: " 1:23 ━━━━━━━━●━━━━━━━━━━━━ 3:45 "
    }
}
