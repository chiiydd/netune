//! QR code rendering widget using Unicode block characters.

use qrcode::QrCode;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
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

        // Total size in modules including quiet zone on both sides.
        let total_modules = modules + QUIET_ZONE * 2;

        // Each terminal row covers 2 QR modules (top + bottom via half-block chars).
        let total_rows = (total_modules + 1) / 2;

        let avail_w = area.width as usize;
        let avail_h = area.height as usize;

        // Center the QR code in the available area.
        let offset_x = avail_w.saturating_sub(total_modules) / 2;
        let offset_y = avail_h.saturating_sub(total_rows) / 2;

        for cy in 0..avail_h {
            for cx in 0..avail_w {
                let cell_x = area.x + cx as u16;
                let cell_y = area.y + cy as u16;
                if cell_x >= area.right() || cell_y >= area.bottom() {
                    continue;
                }

                // Map terminal cell to QR module coordinates (if in range).
                let (top_dark, bot_dark) = if cx >= offset_x && cy >= offset_y {
                    let gx = cx - offset_x;
                    let gy = cy - offset_y;
                    let module_top = gy * 2;
                    let module_bot = module_top + 1;

                    let t = gx < total_modules && module_top < total_modules
                        && is_dark(&colors, modules, gx, module_top);
                    let b = gx < total_modules && module_bot < total_modules
                        && is_dark(&colors, modules, gx, module_bot);
                    (t, b)
                } else {
                    (false, false)
                };

                if let Some(cell) = buf.cell_mut((cell_x, cell_y)) {
                    let ch = match (top_dark, bot_dark) {
                        (true, true) => '█',
                        (true, false) => '▀',
                        (false, true) => '▄',
                        (false, false) => ' ',
                    };
                    cell.set_char(ch);
                }
            }
        }
    }
}

/// Check if a module at (gx, gy) in the total grid is dark, accounting for quiet zone.
fn is_dark(colors: &[bool], modules: usize, gx: usize, gy: usize) -> bool {
    if gx >= QUIET_ZONE
        && gx < modules + QUIET_ZONE
        && gy >= QUIET_ZONE
        && gy < modules + QUIET_ZONE
    {
        colors[(gy - QUIET_ZONE) * modules + (gx - QUIET_ZONE)]
    } else {
        false // quiet zone is light
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::buffer::Buffer as TuiBuffer;
    use ratatui::layout::Rect;

    #[test]
    fn test_qr_rendering() {
        let widget = QrCodeWidget::new("https://example.com");
        let area = Rect::new(0, 0, 40, 20);
        let mut buf = TuiBuffer::empty(area);

        widget.render(area, &mut buf);

        // Verify that some cells have dark block characters.
        let has_dark = (0..area.height).any(|y| {
            (0..area.width).any(|x| {
                buf.cell((x, y))
                    .is_some_and(|c| matches!(c.symbol(), "█" | "▀" | "▄"))
            })
        });
        assert!(has_dark, "QR code should contain dark modules");

        // Verify that some cells have the space character (light modules).
        let has_light = (0..area.height).any(|y| {
            (0..area.width).any(|x| {
                buf.cell((x, y))
                    .is_some_and(|c| c.symbol() == " ")
            })
        });
        assert!(has_light, "QR code should contain light modules");
    }

    #[test]
    fn test_qr_quiet_zone() {
        let widget = QrCodeWidget::new("https://example.com");
        let area = Rect::new(0, 0, 50, 25);
        let mut buf = TuiBuffer::empty(area);

        widget.render(area, &mut buf);

        let corner = buf.cell((0, 0)).unwrap();
        assert_eq!(
            corner.symbol(),
            " ",
            "Corner cells should be light (quiet zone or outside QR)"
        );
    }

    #[test]
    fn test_qr_content_is_url() {
        let url = "https://music.163.com/login?codekey=testkey123";
        let qr = QrCode::new(url.as_bytes());
        assert!(qr.is_ok(), "Full URL should produce a valid QR code");

        let qr = qr.unwrap();
        assert!(qr.width() > 0, "QR code should have non-zero width");
    }
}
