//! Player page — stub for B组(Claude Code) to implement.

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::Span;

use crate::chrome::KeyHint;
use crate::theme::Theme;
use crate::pages::PageAction;

pub struct PlayerPage {
    // TODO: B组(Claude Code) — add state fields (progress, lyrics, etc.)
}

impl PlayerPage {
    pub fn new() -> Self {
        Self {}
    }

    pub fn render(&self, f: &mut Frame, area: Rect) {
        // TODO: B组(Claude Code) — implement player UI
        use ratatui::widgets::{Block, Borders, Paragraph};
        use ratatui::text::Line;
        let block = Block::default()
            .title(" Now Playing ")
            .borders(Borders::ALL);
        let text = Paragraph::new(Line::from("TODO: B组(Claude Code) 待实现"))
            .block(block);
        f.render_widget(text, area);
    }

    pub fn handle_event(&self, _evt: &Event) -> PageAction {
        // TODO: B组(Claude Code) — handle keyboard events
        PageAction::None
    }

    pub fn tick(&self) {
        // TODO: B组(Claude Code) — update playback progress
    }

    pub fn mode(&self) -> (String, Color) {
        ("Normal".to_string(), Theme::MODE_NORMAL)
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        vec![]
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("q", "quit"),
        ]
    }
}
