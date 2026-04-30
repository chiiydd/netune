//! Application state and main event loop.

use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};

use crate::chrome;
use crate::pages::{Page, PageAction};

pub struct App {
    pub page_stack: Vec<Page>,
    pub should_quit: bool,
}

impl App {
    pub fn new() -> Self {
        Self {
            page_stack: vec![Page::home()],
            should_quit: false,
        }
    }

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal
                .draw(|f| {
                    if let Some(page) = self.page_stack.last_mut() {
                        let area = f.area();
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(1),
                                Constraint::Min(1),
                                Constraint::Length(1),
                            ])
                            .split(area);

                        let title = page.title();
                        chrome::render_titlebar(f, chunks[0], title);

                        page.render(f, chunks[1]);

                        let (mode, mode_color) = page.mode();
                        let context = page.context();
                        let hints = page.hints();
                        chrome::render_statusline(f, chunks[2], &mode, mode_color, context, &hints);
                    }
                })?;

            if !event::poll(Duration::from_millis(100))? {
                if let Some(page) = self.page_stack.last_mut() {
                    page.tick();
                }
                continue;
            }

            let evt = event::read()?;

            if let Event::Key(k) = &evt {
                if k.kind == KeyEventKind::Press {
                    match (k.code, k.modifiers) {
                        (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
                            self.should_quit = true;
                        }
                        (KeyCode::Char('q'), KeyModifiers::NONE) | (KeyCode::Esc, _)
                            if self.page_stack.len() == 1 =>
                        {
                            self.should_quit = true;
                        }
                        _ => {}
                    }
                }
            }

            if self.should_quit {
                break;
            }

            let action = if let Some(page) = self.page_stack.last_mut() {
                page.handle_event(&evt).await
            } else {
                PageAction::None
            };

            self.apply_action(action).await;

            if self.should_quit {
                break;
            }
        }
        Ok(())
    }

    async fn apply_action(&mut self, action: PageAction) {
        match action {
            PageAction::None => {}
            PageAction::Quit => self.should_quit = true,
            PageAction::Push(page) => self.page_stack.push(page),
            PageAction::Pop => {
                if self.page_stack.len() > 1 {
                    self.page_stack.pop();
                } else {
                    self.should_quit = true;
                }
            }
            PageAction::Replace(page) => {
                if let Some(top) = self.page_stack.last_mut() {
                    *top = page;
                }
            }
        }
    }
}
