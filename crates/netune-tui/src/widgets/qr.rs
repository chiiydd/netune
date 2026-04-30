//! QR code rendering widget using Unicode half-block characters.

use qrcode::QrCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

pub struct QrCodeWidget {
    pub data: String,
}

impl QrCodeWidget {
    pub fn new(data: &str) -> Self {
        Self {
            data: data.to_string(),
        }
    }
}

impl Widget for QrCodeWidget {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let qr = match QrCode::new(self.data.as_bytes()) {
            Ok(qr) => qr,
            Err(_) => return,
        };

        // Convert to boolean grid (dark = true).
        let width = qr.width();
        let colors: Vec<bool> = qr
            .to_colors()
            .iter()
            .map(|c| matches!(c, qrcode::Color::Dark))
            .collect();

        // Each terminal cell covers 2 vertical modules using ▀ (upper half block).
        // Background = cell bg, foreground = cell fg.
        // Module pair (top, bottom) → ▀ with fg=bottom_color, bg=top_color.
        let modules_per_col = width;
        let modules_per_row = width;
        let cell_cols = modules_per_col.min(area.width as usize);
        let cell_rows = ((modules_per_row + 1) / 2).min(area.height as usize);

        for cy in 0..cell_rows {
            for cx in 0..cell_cols {
                let top_idx = (cy * 2) * modules_per_col + cx;
                let bot_idx = (cy * 2 + 1) * modules_per_col + cx;

                let top_dark = colors.get(top_idx).copied().unwrap_or(false);
                let bot_dark = colors.get(bot_idx).copied().unwrap_or(false);

                let fg = if bot_dark { Color::Black } else { Color::White };
                let bg = if top_dark { Color::Black } else { Color::White };

                let cell_x = area.x + cx as u16;
                let cell_y = area.y + cy as u16;
                if cell_x < area.right() && cell_y < area.bottom() {
                    if let Some(cell) = buf.cell_mut((cell_x, cell_y)) {
                        cell.set_char('▀');
                        cell.set_style(Style::default().fg(fg).bg(bg));
                    }
                }
            }
        }
    }
}
