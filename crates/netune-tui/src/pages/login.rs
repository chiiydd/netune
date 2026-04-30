//! Login page — phone number + password form.
//!
//! A single mode with two fields (Phone / Password).
//! Tab switches focus, Enter submits, Esc pops back.

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Clear, Paragraph};

use crate::chrome::KeyHint;
use crate::pages::PageAction;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LoginField {
    Phone,
    Password,
}

pub struct LoginPage {
    phone: String,
    password: String,
    focus: LoginField,
    loading: bool,
    error: Option<String>,
}

impl Default for LoginPage {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginPage {
    pub fn new() -> Self {
        Self {
            phone: String::new(),
            password: String::new(),
            focus: LoginField::Phone,
            loading: false,
            error: None,
        }
    }

    fn submit(&mut self) {
        if self.phone.is_empty() {
            self.error = Some("Phone number is required".into());
            return;
        }
        if self.password.is_empty() {
            self.error = Some("Password is required".into());
            return;
        }
        self.error = None;
        self.loading = true;
        // TODO: call NeteaseClient::login() when API is wired up.
    }

    // ── Rendering ───────────────────────────────────────────────────────────

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        // Center a card: ~40 wide, ~10 tall (content rows + borders).
        let card_w = 42.min(area.width.saturating_sub(4));
        let card_h = 12.min(area.height.saturating_sub(2));

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
        // Clear background behind card.
        f.render_widget(Clear, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(Theme::ACCENT))
            .title(Span::styled(
                " Login ",
                Style::default()
                    .fg(Theme::ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));

        let inner = block.inner(area);
        f.render_widget(block, area);

        // Split inner area: title-line, phone input, password input, error, spacer.
        let rows = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // phone label
                Constraint::Length(1), // phone input
                Constraint::Length(1), // spacer
                Constraint::Length(1), // password label
                Constraint::Length(1), // password input
                Constraint::Min(0),    // flex
                Constraint::Length(1), // error / hint
            ])
            .split(inner);

        // ── Phone field ─────────────────────────────────────────────────────
        let phone_style = if self.focus == LoginField::Phone {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG_DIM)
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("  Phone", phone_style))),
            rows[0],
        );

        let phone_cursor = if self.focus == LoginField::Phone {
            "▏"
        } else {
            ""
        };
        let phone_border_color = if self.focus == LoginField::Phone {
            Theme::ACCENT
        } else {
            Theme::ACCENT_DIM
        };
        let phone_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(phone_border_color));
        let phone_text = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(&self.phone, Style::default().fg(Theme::FG)),
            Span::styled(phone_cursor, Style::default().fg(Theme::ACCENT)),
        ]))
        .block(phone_block);
        f.render_widget(phone_text, rows[1]);

        // ── Password field ──────────────────────────────────────────────────
        let pw_style = if self.focus == LoginField::Password {
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Theme::FG_DIM)
        };
        f.render_widget(
            Paragraph::new(Line::from(Span::styled("  Password", pw_style))),
            rows[3],
        );

        let masked: String = self.password.chars().map(|_| '*').collect();
        let pw_cursor = if self.focus == LoginField::Password {
            "▏"
        } else {
            ""
        };
        let pw_border_color = if self.focus == LoginField::Password {
            Theme::ACCENT
        } else {
            Theme::ACCENT_DIM
        };
        let pw_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(pw_border_color));
        let pw_text = Paragraph::new(Line::from(vec![
            Span::raw(" "),
            Span::styled(masked, Style::default().fg(Theme::FG)),
            Span::styled(pw_cursor, Style::default().fg(Theme::ACCENT)),
        ]))
        .block(pw_block);
        f.render_widget(pw_text, rows[4]);

        // ── Error / status line ─────────────────────────────────────────────
        let status = if let Some(ref err) = self.error {
            Line::from(Span::styled(
                format!("  ✘ {err}"),
                Style::default().fg(Theme::DANGER),
            ))
        } else if self.loading {
            Line::from(Span::styled(
                "  Logging in…",
                Style::default().fg(Theme::WARNING),
            ))
        } else {
            Line::from(Span::styled(
                "  Tab: switch  Enter: submit  Esc: back",
                Style::default().fg(Theme::MUTED),
            ))
        };
        f.render_widget(Paragraph::new(status), rows[6]);
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
            KeyCode::Esc => return PageAction::Pop,
            KeyCode::Tab => {
                self.focus = match self.focus {
                    LoginField::Phone => LoginField::Password,
                    LoginField::Password => LoginField::Phone,
                };
                self.error = None;
            }
            KeyCode::Enter => {
                self.submit();
            }
            KeyCode::Backspace => {
                match self.focus {
                    LoginField::Phone => {
                        self.phone.pop();
                    }
                    LoginField::Password => {
                        self.password.pop();
                    }
                }
                self.error = None;
            }
            KeyCode::Char(c) => {
                match self.focus {
                    LoginField::Phone => self.phone.push(c),
                    LoginField::Password => self.password.push(c),
                }
                self.error = None;
            }
            _ => {}
        }
        PageAction::None
    }

    // ── Chrome contract ─────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        if self.loading {
            ("LOADING".into(), Theme::MODE_LOADING)
        } else {
            ("LOGIN".into(), Theme::MODE_NORMAL)
        }
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        if self.phone.is_empty() {
            vec![Span::styled(
                "not logged in",
                Style::default().fg(Theme::MUTED),
            )]
        } else {
            vec![
                Span::styled("phone: ", Style::default().fg(Theme::MUTED)),
                Span::styled(self.phone.clone(), Theme::accent_bold()),
            ]
        }
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("Tab", "field"),
            KeyHint::new("⏎", "login"),
            KeyHint::new("Esc", "back"),
        ]
    }
}
