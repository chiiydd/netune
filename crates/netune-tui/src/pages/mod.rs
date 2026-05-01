//! Page router — each variant holds one full-screen page.

pub mod home;
pub mod login;
pub mod player;
pub mod playlist;
pub mod search;
pub mod settings;

use crossterm::event::Event;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::text::Span;

use crate::chrome::KeyHint;

use self::home::HomePage;
use self::login::LoginPage;
use self::player::PlayerPage;
use self::playlist::PlaylistPage;
use self::search::SearchPage;
use self::settings::SettingsPage;

use netune_core::models::Song;

/// Actions a page can return to the app loop.
pub enum PageAction {
    None,
    Quit,
    Push(Page),
    Pop,
    Replace(Page),

    // ── Cross-page actions (handled by App) ────────────────────────────
    /// QR login: poll scan status.
    QrCheckPoll,
    /// QR login: refresh QR code.
    QrRefresh,
    /// Browser cookie import: import cookies from a local browser.
    BrowserImportConfirm(String),
    /// Search page submits a query.
    Search(String),
    /// Background search task completed with results.
    SearchReady(Vec<Song>),
    /// Play a specific song (from search results or playlist tracks).
    PlaySong(Song),
    /// Load playlist tracks and set them as the play queue.
    PlayQueue(Vec<Song>),
    /// Add a single song to the play queue without starting playback.
    AddToQueue(Song),
    /// Fetch playlist detail tracks from API.
    FetchPlaylistDetail(u64),
    /// Fetch daily recommend songs from API.
    FetchDailyRecommend,
    /// Toggle pause/resume on the audio player.
    TogglePause,
    /// Seek forward/backward by N seconds.
    Seek(f64),
    /// Set volume (0-100).
    SetVolume(u16),
    /// Toggle queue panel visibility.
    ToggleQueuePanel,
    /// Jump to a specific song in the queue by index.
    JumpToQueueItem(usize),
    /// Cycle play mode (Sequential → LoopAll → LoopOne → Shuffle).
    CyclePlayMode,
}

/// Settings page focus fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsField {
    Quality,
    Theme,
    Cache,
    Device,
}

/// The active full-screen page.
pub enum Page {
    Home(HomePage),
    Playlist(PlaylistPage),
    Player(PlayerPage),
    Search(SearchPage),
    Login(LoginPage),
    Settings(SettingsPage),
}

impl Page {
    pub fn home() -> Self {
        Page::Home(HomePage::new())
    }

    pub fn render(&mut self, f: &mut Frame, area: Rect) {
        match self {
            Page::Home(p) => p.render(f, area),
            Page::Playlist(p) => p.render(f, area),
            Page::Player(p) => p.render(f, area),
            Page::Search(p) => p.render(f, area),
            Page::Login(p) => p.render(f, area),
            Page::Settings(p) => p.render(f, area),
        }
    }

    pub async fn handle_event(&mut self, evt: &Event) -> PageAction {
        match self {
            Page::Home(p) => p.handle_event(evt),
            Page::Playlist(p) => p.handle_event(evt),
            Page::Player(p) => p.handle_event(evt),
            Page::Search(p) => p.handle_event(evt),
            Page::Login(p) => p.handle_event(evt),
            Page::Settings(p) => p.handle_event(evt),
        }
    }

    pub fn tick(&mut self) -> PageAction {
        match self {
            Page::Login(p) => p.tick(),
            Page::Player(p) => { p.tick(); PageAction::None }
            Page::Search(p) => { p.tick(); PageAction::None }
            _ => PageAction::None,
        }
    }

    // ── Chrome contract ──────────────────────────────────────────────────────

    pub fn title(&self) -> &'static str {
        match self {
            Page::Home(_) => "Home",
            Page::Playlist(_) => "Playlist",
            Page::Player(_) => "Now Playing",
            Page::Search(_) => "Search",
            Page::Login(_) => "Login",
            Page::Settings(_) => "Settings",
        }
    }

    pub fn mode(&self) -> (String, Color) {
        match self {
            Page::Home(p) => p.mode(),
            Page::Playlist(p) => p.mode(),
            Page::Player(p) => p.mode(),
            Page::Search(p) => p.mode(),
            Page::Login(p) => p.mode(),
            Page::Settings(p) => p.mode(),
        }
    }

    pub fn context(&self) -> Vec<Span<'static>> {
        match self {
            Page::Home(p) => p.context(),
            Page::Playlist(p) => p.context(),
            Page::Player(p) => p.context(),
            Page::Search(p) => p.context(),
            Page::Login(p) => p.context(),
            Page::Settings(p) => p.context(),
        }
    }

    pub fn hints(&self) -> Vec<KeyHint> {
        match self {
            Page::Home(p) => p.hints(),
            Page::Playlist(p) => p.hints(),
            Page::Player(p) => p.hints(),
            Page::Search(p) => p.hints(),
            Page::Login(p) => p.hints(),
            Page::Settings(p) => p.hints(),
        }
    }
}
