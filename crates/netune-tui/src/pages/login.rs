//! Login page — QR code scan login.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::chrome::KeyHint;
use crate::pages::PageAction;
use crate::theme::Theme;
use crate::widgets::QrCodeWidget;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QrLoginState {
    WaitingForQr,
    WaitingScan,
    Scanned,
    Success,
    Expired,
}

pub struct LoginPage {
    pub unikey: Option<String>,
    pub qr_state: QrLoginState,
    pub qr_url: Option<String>,
    pub error: Option<String>,
    tick_count: u64,
}

impl Default for LoginPage {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginPage {
    pub fn new() -> Self {
        Self {
            unikey: None,
            qr_state: QrLoginState::WaitingForQr,
            qr_url: None,
            error: None,
            tick_count: 0,
        }
    }

    /// Called when a new QR key is received.
    pub fn set_qr_key(&mut self, unikey: String) {
        self.qr_url = Some(format!(
            "https://music.163.com/login?codekey={unikey}&callback=close"
        ));
        self.unikey = Some(unikey);
        self.qr_state = QrLoginState::WaitingScan;
        self.error = None;
    }

    /// Called when QR scan state changes to scanned/confirming.
    pub fn set_scanned(&mut self) {
        self.qr_state = QrLoginState::Scanned;
    }

    /// Called on successful login.
    pub fn set_success(&mut self) {
        self.qr_state = QrLoginState::Success;
    }

    /// Called when QR code expires.
    pub fn set_expired(&mut self, msg: String) {
        self.qr_state = QrLoginState::Expired;
        self.error = Some(msg);
    }

    /// Called on API error.
    pub fn set_error(&mut self, msg: String) {
        self.error = Some(msg);
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let card_w = 40.min(area.width.saturating_sub(4));
        let card_h = 24.min(area.height.saturating_sub(2));

        let outer = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(card_h),
                Constraint::Fill(1),
            ])
            .split(area);

        let inner = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Fill(1),
                Constraint::Length(card_w),
                Constraint::Fill(1),
            ])
            .split(outer[1]);

        let card_area = inner[1];
        self.render_card(f, card_area);
    }

    fn render_card(&self, f: &mut Frame, area: Rect) {
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT))
            .title(Span::styled(
                " QR Login ",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Layout: QR area + spacer + status text + hint
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(1),    // QR code
                Constraint::Length(1), // spacer
                Constraint::Length(1), // status
                Constraint::Length(1), // hint
            ])
            .split(inner);

        // ── QR Code ──────────────────────────────────────────────────────
        if let Some(ref url) = self.qr_url {
            let qr_widget = QrCodeWidget::new(url);
            f.render_widget(qr_widget, rows[0]);
        }

        // ── Status text ──────────────────────────────────────────────────
        let (status_text, status_color) = match &self.qr_state {
            QrLoginState::WaitingForQr => ("正在获取二维码...".to_string(), Theme::WARNING),
            QrLoginState::WaitingScan => {
                ("请使用网易云音乐 App 扫码".to_string(), Theme::FG)
            }
            QrLoginState::Scanned => {
                ("已扫码，请在手机上确认".to_string(), Theme::INFO)
            }
            QrLoginState::Success => ("登录成功!".to_string(), Theme::SUCCESS),
            QrLoginState::Expired => {
                let msg = self
                    .error
                    .as_deref()
                    .unwrap_or("二维码已过期，按 R 重新获取");
                (msg.to_string(), Theme::DANGER)
            }
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled(
                format!("  {status_text}"),
                Style::default().fg(status_color),
            ))),
            rows[2],
        );

        // ── Error line (if not in expired state) ─────────────────────────
        if let Some(ref err) = self.error {
            if self.qr_state != QrLoginState::Expired {
                f.render_widget(
                    Paragraph::new(Line::from(Span::styled(
                        format!("  ✘ {err}"),
                        Style::default().fg(Theme::DANGER),
                    ))),
                    rows[3],
                );
            }
        }
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
            KeyCode::Esc => PageAction::Pop,
            KeyCode::Char('r') | KeyCode::Char('R') => {
                self.qr_state = QrLoginState::WaitingForQr;
                self.qr_url = None;
                self.unikey = None;
                self.error = None;
                PageAction::QrRefresh
            }
            _ => PageAction::None,
        }
    }

    // ── Tick ────────────────────────────────────────────────────────────────

    pub fn tick(&mut self) -> PageAction {
        self.tick_count = self.tick_count.wrapping_add(1);
        // Poll every ~2 seconds (120 ticks at ~60 Hz / 100ms poll).
        if self.tick_count % 120 == 0
            && matches!(
                self.qr_state,
                QrLoginState::WaitingScan | QrLoginState::Scanned
            )
        {
            PageAction::QrCheckPoll
        } else {
            PageAction::None
        }
    }

    // ── Chrome contract ─────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        let color = match self.qr_state {
            QrLoginState::WaitingForQr => Theme::MODE_LOADING,
            QrLoginState::WaitingScan => Theme::MODE_NORMAL,
            QrLoginState::Scanned => Theme::INFO,
            QrLoginState::Success => Theme::SUCCESS,
            QrLoginState::Expired => Theme::DANGER,
        };
        ("QR LOGIN".into(), color)
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        let text = match &self.qr_state {
            QrLoginState::WaitingForQr => "获取中",
            QrLoginState::WaitingScan => "等待扫码",
            QrLoginState::Scanned => "已扫码",
            QrLoginState::Success => "已登录",
            QrLoginState::Expired => "已过期",
        };
        vec![Span::styled(text.to_string(), Style::default().fg(Theme::MUTED))]
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("R", "refresh"),
            KeyHint::new("Esc", "back"),
        ]
    }
}
