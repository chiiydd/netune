//! Application state and main event loop.
//!
//! The `App` is the central coordinator: it owns the API client, audio player,
//! play queue, and config.  Pages return `PageAction` values describing what
//! they want; `apply_action` executes those actions (API calls, playback
//! control) and feeds results back into the appropriate pages.

use std::time::Duration;

use color_eyre::Result;
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::Backend;
use ratatui::layout::{Constraint, Direction, Layout};

use netune_api::NeteaseApiClient;
use netune_core::config::Config;
use netune_core::models::{Song, UserProfile};
use netune_core::traits::{AudioPlayer, NeteaseClient};
use netune_player::{NetunePlayer, PlayQueue};

use crate::chrome;
use crate::pages::Page;
use crate::pages::PageAction;

pub struct App {
    /// Page navigation stack — last element is the active page.
    pub page_stack: Vec<Page>,
    /// Whether the app should exit.
    pub should_quit: bool,
    /// Netease API client (created at startup).
    api_client: Option<NeteaseApiClient>,
    /// Audio player (created at startup).
    player: Option<NetunePlayer>,
    /// Current playback queue.
    play_queue: PlayQueue,
    /// Application configuration.
    config: Config,
    /// Logged-in user profile (if authenticated).
    user: Option<UserProfile>,
}

impl App {
    pub fn new() -> Self {
        Self {
            page_stack: vec![Page::home()],
            should_quit: false,
            api_client: Some(NeteaseApiClient::new()),
            player: Some(NetunePlayer::new()),
            play_queue: PlayQueue::new(),
            config: Config::default(),
            user: None,
        }
    }

    // ── High-level actions ──────────────────────────────────────────────

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Play a song: fetch its streaming URL, start playback, update pages.
    async fn do_play_song(&mut self, song: Song) {
        let Some(ref client) = self.api_client else {
            tracing::warn!("Cannot play song — no API client");
            return;
        };
        let quality = self.config.quality;

        // Fetch streaming URL and lyrics concurrently to avoid UI freeze.
        let url_fut = client.song_url(song.id, quality);
        let lyrics_fut = client.lyrics(song.id);
        let (url_result, lyrics_result) = tokio::join!(url_fut, lyrics_fut);

        let url = match url_result {
            Ok(u) => u,
            Err(e) => {
                tracing::warn!(error = %e, song_id = song.id, "Failed to get song URL");
                return;
            }
        };

        // Start playback.
        if let Some(ref player) = self.player {
            if let Err(e) = player.play(&url).await {
                tracing::warn!(error = %e, "Playback failed");
                return;
            }
            player.set_volume(self.config.volume);
        }

        // Update queue: replace with single song and set as current.
        self.play_queue.load(vec![song.clone()]);

        // Update the player page.
        self.update_player_page_for(song);

        // Set lyrics on the player page (if fetched successfully).
        match lyrics_result {
            Ok(lyrics) => {
                for page in &mut self.page_stack {
                    if let Page::Player(pp) = page {
                        pp.set_lyrics(lyrics);
                        break;
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch lyrics");
            }
        }

        tracing::info!("Now playing");
    }

    /// Jump to the next song in the queue.
    async fn do_play_next(&mut self) {
        let Some(song) = self.play_queue.advance().cloned() else {
            return;
        };
        self.do_play_song(song).await;
    }

    /// Jump to the previous song in the queue.
    async fn do_play_prev(&mut self) {
        let Some(song) = self.play_queue.prev().cloned() else {
            return;
        };
        self.do_play_song(song).await;
    }

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Ensure a `PlayerPage` exists on the stack and set its song info.
    fn update_player_page_for(&mut self, song: Song) {
        // Check if there's already a PlayerPage on the stack.
        let mut found = false;
        for page in &mut self.page_stack {
            if let Page::Player(pp) = page {
                pp.set_song(song.clone());
                found = true;
                break;
            }
        }
        // If no PlayerPage exists, push one.
        if !found {
            let mut pp = crate::pages::player::PlayerPage::new();
            pp.set_song(song);
            self.page_stack.push(Page::Player(pp));
        }
    }

    /// Sync player state into any visible PlayerPage.
    fn sync_player_state(&mut self) {
        let Some(ref player) = self.player else {
            return;
        };
        let pos = player.position();
        let dur = player.duration();
        let playing = player.is_playing();
        for page in &mut self.page_stack {
            if let Page::Player(pp) = page {
                pp.update_from_player(pos, dur, playing);
            }
        }
    }

    /// Fetch user playlists after successful login.
    async fn fetch_user_playlists(&mut self) {
        let Some(ref client) = self.api_client else {
            return;
        };
        let Some(ref user) = self.user else {
            return;
        };
        match client.user_playlists(user.uid).await {
            Ok(playlists) => {
                tracing::info!(count = playlists.len(), "Fetched user playlists");
                for page in &mut self.page_stack {
                    match page {
                        Page::Playlist(pp) => pp.set_playlists(playlists.clone()),
                        Page::Home(hp) => hp.set_playlists(playlists.clone()),
                        _ => {}
                    }
                }
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to fetch playlists");
            }
        }
    }

    // ── QR Login helpers ──────────────────────────────────────────────

    /// Generate a new QR code for the login page.
    async fn do_qr_refresh(&mut self) {
        let Some(Page::Login(lp)) = self.page_stack.last_mut() else {
            return;
        };
        let Some(ref client) = self.api_client else {
            lp.set_error("No API client configured".into());
            return;
        };
        match client.login_qr_generate().await {
            Ok(unikey) => {
                tracing::info!("QR code generated");
                lp.set_qr_key(unikey);
            }
            Err(e) => {
                tracing::warn!(error = %e, "QR generate failed");
                lp.set_error(e.to_string());
            }
        }
    }

    // ── Tick ────────────────────────────────────────────────────────────

    fn tick(&mut self) -> PageAction {
        self.sync_player_state();
        if let Some(page) = self.page_stack.last_mut() {
            page.tick()
        } else {
            PageAction::None
        }
    }

    // ── Main loop ───────────────────────────────────────────────────────

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()> {
        loop {
            terminal.draw(|f| {
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
                let tick_action = self.tick();
                if !matches!(tick_action, PageAction::None) {
                    self.apply_action(tick_action).await;
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

    // ── Action dispatch ─────────────────────────────────────────────────

    async fn apply_action(&mut self, action: PageAction) {
        match action {
            PageAction::None => {}
            PageAction::Quit => self.should_quit = true,
            PageAction::Push(page) => {
                let is_login = matches!(page, Page::Login(_));
                self.page_stack.push(page);
                // Auto-generate QR code when login page opens.
                if is_login {
                    self.do_qr_refresh().await;
                }
            }
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

            // ── QR Login: generate new QR code ──────────────────────────
            PageAction::QrRefresh => {
                self.do_qr_refresh().await;
            }

            // ── QR Login: poll scan status ──────────────────────────────
            PageAction::QrCheckPoll => {
                let Some(Page::Login(lp)) = self.page_stack.last_mut() else {
                    return;
                };
                let Some(ref unikey) = lp.unikey.clone() else {
                    return;
                };
                let Some(ref client) = self.api_client else {
                    return;
                };
                match client.login_qr_check(&unikey).await {
                    Ok(Some(profile)) => {
                        tracing::info!(nickname = %profile.nickname, uid = profile.uid, "QR login succeeded");
                        lp.set_success();
                        self.user = Some(profile.clone());
                        // Navigate to home and update user info.
                        self.page_stack.clear();
                        let mut home = crate::pages::home::HomePage::new();
                        home.set_user(Some(profile));
                        self.page_stack.push(Page::Home(home));
                        // Fetch playlists in background.
                        self.fetch_user_playlists().await;
                    }
                    Ok(None) => {
                        // Still waiting — state is managed by the tick logic.
                        // If we got Ok(None) and it wasn't an error, it could be
                        // 801 (waiting) or 802 (scanned). We can't distinguish
                        // from the trait, so we treat both as "waiting".
                    }
                    Err(e) => {
                        let msg = e.to_string();
                        if msg.contains("expired") || msg.contains("800") {
                            tracing::info!("QR code expired");
                            lp.set_expired(msg);
                        } else {
                            tracing::warn!(error = %e, "QR check error");
                            lp.set_error(msg);
                        }
                    }
                }
            }

            // ── Search ──────────────────────────────────────────────────
            PageAction::Search(keyword) => {
                let Some(Page::Search(sp)) = self.page_stack.last_mut() else {
                    return;
                };
                let Some(ref client) = self.api_client else {
                    sp.set_results(Vec::new());
                    return;
                };
                match client.search_songs(&keyword, 0, 30).await {
                    Ok(result) => {
                        tracing::info!(count = result.songs.len(), total = result.total, "Search OK");
                        sp.set_results(result.songs);
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Search failed");
                        sp.set_results(Vec::new());
                    }
                }
            }

            // ── Play song ───────────────────────────────────────────────
            PageAction::PlaySong(song) => {
                self.do_play_song(song).await;
            }

            // ── Play queue ──────────────────────────────────────────────
            PageAction::PlayQueue(songs) => {
                self.play_queue.load(songs);
                self.do_play_next().await;
            }

            // ── Fetch playlist detail ───────────────────────────────────
            PageAction::FetchPlaylistDetail(playlist_id) => {
                let Some(ref client) = self.api_client else {
                    return;
                };
                match client.playlist_detail(playlist_id).await {
                    Ok(tracks) => {
                        self.play_queue.load(tracks.clone());
                        for page in &mut self.page_stack {
                            if let Page::Playlist(pp) = page {
                                pp.set_tracks(tracks);
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to fetch playlist detail");
                    }
                }
            }

            // ── Player controls ────────────────────────────────────────
            PageAction::TogglePause => {
                if let Some(ref player) = self.player {
                    player.toggle_pause();
                    // Update UI immediately
                    let playing = player.is_playing();
                    let vol = (player.volume() * 100.0) as u16;
                    for page in &mut self.page_stack {
                        if let Page::Player(pp) = page {
                            pp.update_from_player(
                                player.position(),
                                player.duration(),
                                playing,
                            );
                            pp.set_volume(vol);
                            break;
                        }
                    }
                }
            }
            PageAction::Seek(delta) => {
                if let Some(ref player) = self.player {
                    let _ = player.seek(delta);
                }
            }
            PageAction::SetVolume(vol) => {
                if let Some(ref player) = self.player {
                    player.set_volume(vol as f32 / 100.0);
                    for page in &mut self.page_stack {
                        if let Page::Player(pp) = page {
                            pp.set_volume(vol);
                            break;
                        }
                    }
                }
            }

            // ── Fetch daily recommend ──────────────────────────────────
            PageAction::FetchDailyRecommend => {
                let Some(ref client) = self.api_client else {
                    return;
                };
                match client.daily_recommend().await {
                    Ok(recommend) => {
                        self.play_queue.load(recommend.songs.clone());
                        let mut pp = super::pages::playlist::PlaylistPage::new();
                        pp.set_tracks(recommend.songs);
                        self.page_stack.push(Page::Playlist(pp));
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Failed to fetch daily recommend");
                    }
                }
            }
        }
    }
}
