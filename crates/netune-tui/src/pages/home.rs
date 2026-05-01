//! Main menu page — entry point for all features.
//!
//! Body is a centered menu list. Top bar and statusline come from
//! the app chrome (`crate::chrome`).

use crossterm::event::{Event, KeyCode, KeyEventKind};
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, List, ListItem, ListState};

use netune_core::models::{Playlist, UserProfile};

use crate::chrome::KeyHint;
use crate::theme::Theme;

use super::PageAction;

/// One row in the home menu — the action it triggers is attached so the
/// activate logic doesn't depend on the row's index.
struct MenuItem {
    label: &'static str,
    desc: &'static str,
    action: MenuAction,
}

#[derive(Clone, Copy)]
enum MenuAction {
    NowPlaying,
    Playlists,
    Search,
    DailyRecommend,
    PersonalFm,
    Login,
    Logout,
    Settings,
    Quit,
}

pub struct HomePage {
    list_state: ListState,
    user: Option<UserProfile>,
    playlists: Vec<Playlist>,
}

impl Default for HomePage {
    fn default() -> Self {
        Self::new()
    }
}

impl HomePage {
    pub fn new() -> Self {
        let mut list_state = ListState::default();
        list_state.select(Some(0));
        Self {
            list_state,
            user: None,
            playlists: Vec::new(),
        }
    }

    pub fn set_user(&mut self, user: Option<UserProfile>) {
        self.user = user;
        self.list_state.select(Some(0));
    }

    pub fn set_playlists(&mut self, playlists: Vec<Playlist>) {
        self.playlists = playlists;
    }

    /// Build the menu items based on current login state.
    fn menu_items(&self) -> Vec<MenuItem> {
        let mut items = Vec::new();

        items.push(MenuItem {
            label: "Now Playing",
            desc: "Go to the player page",
            action: MenuAction::NowPlaying,
        });

        if self.user.is_some() {
            items.push(MenuItem {
                label: "My Playlists",
                desc: "Browse your playlists and collections",
                action: MenuAction::Playlists,
            });
        }

        items.push(MenuItem {
            label: "Search",
            desc: "Search for songs, albums, and artists",
            action: MenuAction::Search,
        });

        if self.user.is_some() {
            items.push(MenuItem {
                label: "Daily Recommend",
                desc: "Personalized daily mix for you",
                action: MenuAction::DailyRecommend,
            });
            items.push(MenuItem {
                label: "Personal FM",
                desc: "Endless radio tailored to your taste",
                action: MenuAction::PersonalFm,
            });
        }

        if self.user.is_some() {
            items.push(MenuItem {
                label: "Logout",
                desc: "Sign out of your account",
                action: MenuAction::Logout,
            });
        } else {
            items.push(MenuItem {
                label: "Login",
                desc: "Sign in to your Netease account",
                action: MenuAction::Login,
            });
        }

        items.push(MenuItem {
            label: "Settings",
            desc: "App preferences and configuration",
            action: MenuAction::Settings,
        });
        items.push(MenuItem {
            label: "Quit",
            desc: "Exit netune",
            action: MenuAction::Quit,
        });

        items
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        let menu_items = self.menu_items();

        // Reserve space for user info if logged in
        let user_info_h: u16 = if self.user.is_some() { 3 } else { 0 };
        let menu_h = (menu_items.len() * 2 + 2) as u16;
        let menu_w = 64u16.min(area.width.saturating_sub(4));
        let pad_v = area.height.saturating_sub(menu_h + user_info_h) / 2;
        let pad_h = (area.width.saturating_sub(menu_w)) / 2;

        let v = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(pad_v),
                Constraint::Length(user_info_h),
                Constraint::Length(menu_h),
                Constraint::Min(0),
            ])
            .split(area);
        let h = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(pad_h),
                Constraint::Length(menu_w),
                Constraint::Min(0),
            ])
            .split(v[2]);

        // Render user info if logged in
        if let Some(ref user) = self.user {
            let h_user = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([
                    Constraint::Length(pad_h),
                    Constraint::Length(menu_w),
                    Constraint::Min(0),
                ])
                .split(v[1]);

            let mut lines = vec![Line::from(vec![
                Span::raw("  "),
                Span::styled(
                    &user.nickname,
                    Style::default()
                        .fg(Theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ),
            ])];
            if let Some(ref avatar) = user.avatar_url {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(avatar.as_str(), Style::default().fg(Theme::MUTED)),
                ]));
            }

            let block = Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(Theme::ACCENT_DIM))
                .title(Span::styled(
                    " User ",
                    Style::default()
                        .fg(Theme::ACCENT)
                        .add_modifier(Modifier::BOLD),
                ));
            let user_widget = ratatui::widgets::Paragraph::new(lines).block(block);
            f.render_widget(user_widget, h_user[1]);
        }

        let items: Vec<ListItem> = menu_items
            .iter()
            .map(|item| {
                ListItem::new(vec![
                    Line::from(vec![
                        Span::raw("  "),
                        Span::styled(
                            item.label.to_owned(),
                            Style::default().add_modifier(Modifier::BOLD),
                        ),
                    ]),
                    Line::from(vec![
                        Span::raw("    "),
                        Span::styled(item.desc, Style::default().fg(Theme::MUTED)),
                    ]),
                ])
            })
            .collect();

        let list = List::new(items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(Style::default().fg(Theme::ACCENT_DIM))
                    .title(Span::styled(
                        " ♫ Main menu ",
                        Style::default()
                            .fg(Theme::ACCENT)
                            .add_modifier(Modifier::BOLD),
                    )),
            )
            .highlight_style(Theme::selection())
            .highlight_symbol("▶ ");

        f.render_stateful_widget(list, h[1], &mut self.list_state);
    }

    pub fn handle_event(&mut self, evt: &Event) -> PageAction {
        let Event::Key(k) = evt else {
            return PageAction::None;
        };
        if k.kind != KeyEventKind::Press {
            return PageAction::None;
        }
        let len = self.menu_items().len();
        match k.code {
            KeyCode::Down | KeyCode::Char('j') => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state.select(Some((i + 1) % len));
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let i = self.list_state.selected().unwrap_or(0);
                self.list_state
                    .select(Some(i.checked_sub(1).unwrap_or(len - 1)));
            }
            KeyCode::Enter | KeyCode::Char('l') => return self.activate(),
            KeyCode::Char('q') | KeyCode::Esc => return PageAction::Quit,
            KeyCode::Tab => return PageAction::ToggleQueuePanel,
            _ => {}
        }
        PageAction::None
    }

    fn activate(&self) -> PageAction {
        let menu_items = self.menu_items();
        let action = menu_items
            .get(self.list_state.selected().unwrap_or(0))
            .map(|item| item.action)
            .unwrap_or(MenuAction::Quit);
        match action {
            MenuAction::NowPlaying => {
                PageAction::Push(super::Page::Player(super::PlayerPage::new()))
            }
            MenuAction::Playlists => {
                let mut pp = super::PlaylistPage::new();
                pp.set_playlists(self.playlists.clone());
                PageAction::Push(super::Page::Playlist(pp))
            }
            MenuAction::Search => PageAction::Push(super::Page::Search(super::SearchPage::new())),
            MenuAction::DailyRecommend => {
                PageAction::FetchDailyRecommend
            }
            MenuAction::PersonalFm => {
                PageAction::Push(super::Page::Player(super::PlayerPage::new()))
            }
            MenuAction::Login => PageAction::Push(super::Page::Login(super::LoginPage::new())),
            MenuAction::Logout => PageAction::Pop,
            MenuAction::Settings => {
                PageAction::Push(super::Page::Settings(super::SettingsPage::new()))
            }
            MenuAction::Quit => PageAction::Quit,
        }
    }

    // ── Chrome contract ──────────────────────────────────────────────────────

    pub fn mode(&self) -> (String, Color) {
        ("MENU".into(), Theme::MODE_NORMAL)
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        let menu_items = self.menu_items();
        let label = menu_items
            .get(self.list_state.selected().unwrap_or(0))
            .map(|item| item.label)
            .unwrap_or("");
        vec![Span::styled(
            label.to_owned(),
            Style::default()
                .fg(Theme::ACCENT)
                .add_modifier(Modifier::BOLD),
        )]
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("j/k", "move"),
            KeyHint::new("⏎", "select"),
            KeyHint::new("q", "quit"),
        ]
    }
}
