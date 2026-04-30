//! QR code rendering widget using Unicode half-block characters.

use qrcode::QrCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::Widget;

/// Minimum quiet zone width in modules (QR spec requires >= 4).
const QUIET_ZONE: usize = 4;

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

        let modules = qr.width();
        let colors: Vec<bool> = qr
            .to_colors()
            .iter()
            .map(|c| matches!(c, qrcode::Color::Dark))
            .collect();

        // Total size including quiet zone on both sides.
        let total_modules = modules + QUIET_ZONE * 2;

        // Each terminal cell is 1 wide × 2 tall in module units.
        let qr_cols = total_modules;
        let qr_rows = (total_modules + 1) / 2;

        let avail_w = area.width as usize;
        let avail_h = area.height as usize;

        // Center the QR code in the available area.
        let offset_x = avail_w.saturating_sub(qr_cols) / 2;
        let offset_y = avail_h.saturating_sub(qr_rows) / 2;

        for cy in 0..avail_h {
            for cx in 0..avail_w {
                // Position in the total (padded) QR grid.
                let gx = cx.wrapping_sub(offset_x);
                let gy = cy.wrapping_sub(offset_y);

                // Both vertical modules for this cell.
                let top_my = gy * 2;
                let bot_my = gy * 2 + 1;

                let top_dark = if gx < qr_cols && top_my < total_modules {
                    let mx = gx;
                    let my = top_my;
                    // Map to module coordinates (strip quiet zone).
                    if mx >= QUIET_ZONE
                        && mx < modules + QUIET_ZONE
                        && my >= QUIET_ZONE
                        && my < modules + QUIET_ZONE
                    {
                        let idx = (my - QUIET_ZONE) * modules + (mx - QUIET_ZONE);
                        colors[idx]
                    } else {
                        false // quiet zone = light
                    }
                } else {
                    false // outside QR = light
                };

                let bot_dark = if gx < qr_cols && bot_my < total_modules {
                    let mx = gx;
                    let my = bot_my;
                    if mx >= QUIET_ZONE
                        && mx < modules + QUIET_ZONE
                        && my >= QUIET_ZONE
                        && my < modules + QUIET_ZONE
                    {
                        let idx = (my - QUIET_ZONE) * modules + (mx - QUIET_ZONE);
                        colors[idx]
                    } else {
                        false
                    }
                } else {
                    false
                };

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
