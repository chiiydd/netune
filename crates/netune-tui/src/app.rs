//! Application state and main event loop.
//!
//! The `App` is the central coordinator: it owns the API client, audio player,
//! play queue, and config.  Pages return `PageAction` values describing what
//! they want; `apply_action` executes those actions (API calls, playback
//! control) and feeds results back into the appropriate pages.

use std::sync::Arc;
use std::time::{Duration, Instant};

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

use netune_core::models::Lyrics;
use netune_core::models::SearchResult;

use crate::chrome;
use crate::pages::Page;
use crate::pages::PageAction;
use crate::widgets::queue_panel::{QueuePanel, QueuePanelResult};

/// Path to the persisted queue file (~/.netune/queue.json).
fn queue_file_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".netune")
        .join("queue.json")
}

/// Path to the persisted user profile (~/.netune/user.json).
fn profile_file_path() -> std::path::PathBuf {
    dirs::home_dir()
        .unwrap_or_default()
        .join(".netune")
        .join("user.json")
}

fn save_profile(profile: &netune_core::models::UserProfile) {
    let path = profile_file_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(profile) {
        let _ = std::fs::write(&path, json);
    }
}

fn load_profile() -> Option<netune_core::models::UserProfile> {
    let path = profile_file_path();
    let content = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Result of a background song-loading task.
struct PendingPlayResult {
    song_id: u64,
    _song: Song,
    /// Audio bytes — `None` when we already played from cache (cache hit).
    audio_bytes: Option<Vec<u8>>,
    lyrics: Option<Lyrics>,
    /// Decoded cover image protocol (if available).
    cover_protocol: Option<ratatui_image::protocol::Protocol>,
}

/// Result of a background pre-cache task (audio + lyrics + cover).
struct PreCacheResult {
    song_id: u64,
    audio_bytes: Vec<u8>,
    lyrics: Option<Lyrics>,
    cover_bytes: Option<Vec<u8>>,
}

/// Decision for how to handle a TogglePause action.
enum ToggleAction {
    /// Normal toggle — player has active state.
    Toggle,
    /// Song is loading — ignore the press.
    Ignore,
    /// State lost — replay the given song (or None if no current song).
    Replay(Option<Song>),
}

pub struct App {
    /// Page navigation stack — last element is the active page.
    pub page_stack: Vec<Page>,
    /// Whether the app should exit.
    pub should_quit: bool,
    /// Netease API client (created at startup).
    api_client: Option<Arc<NeteaseApiClient>>,
    /// Audio player (created at startup).
    player: Option<NetunePlayer>,
    /// Current playback queue.
    play_queue: PlayQueue,
    /// Application configuration.
    config: Config,
    /// Logged-in user profile (if authenticated).
    user: Option<UserProfile>,
    /// Floating queue panel overlay (None = hidden).
    queue_panel: Option<QueuePanel>,
    /// Pre-fetched audio data: song_id → audio bytes.
    /// Used to eliminate download delay when switching songs.
    audio_cache: crate::audio_cache::DiskAudioCache,
    /// Background task fetching song URL + lyrics + audio bytes.
    pending_play: Option<tokio::task::JoinHandle<PendingPlayResult>>,
    /// Background task for search query.
    pending_search: Option<tokio::task::JoinHandle<netune_core::Result<SearchResult>>>,
    /// Background task for pre-caching next song (audio + lyrics + cover).
    pending_precache: Option<tokio::task::JoinHandle<Option<PreCacheResult>>>,
}

impl App {
    pub fn new() -> Self {
        Self {
            page_stack: vec![Page::home()],
            should_quit: false,
            api_client: Some(Arc::new(NeteaseApiClient::new())),
            player: Some(NetunePlayer::new()),
            play_queue: PlayQueue::new(),
            config: Config::default(),
            user: None,
            queue_panel: None,
            audio_cache: crate::audio_cache::DiskAudioCache::new(),
            pending_play: None,
            pending_search: None,
            pending_precache: None,
        }
    }

    // ── High-level actions ──────────────────────────────────────────────

    // ── Helpers ─────────────────────────────────────────────────────────

    /// Play a song: check cache or spawn a background task for URL + lyrics +
    /// audio download so the UI never blocks on network I/O.
    async fn do_play_song(&mut self, song: Song) {
        // Abort any in-flight background task (user picked a new song).
        if let Some(handle) = self.pending_play.take() {
            handle.abort();
        }
        if let Some(handle) = self.pending_precache.take() {
            handle.abort();
        }

        // Stop currently playing audio immediately to avoid desync when
        // the user skips songs faster than new audio loads.
        if let Some(ref player) = self.player {
            player.stop();
        }

        // Immediately show the player page with loading state.
        self.update_player_page_for(song.clone());
        for page in &mut self.page_stack {
            if let Page::Player(pp) = page {
                pp.set_loading(true);
                break;
            }
        }

        let client = self.api_client.clone();
        let Some(client) = client else {
            tracing::warn!("Cannot play song — no API client");
            self.set_player_loading(false);
            return;
        };

        // Clone everything the background task needs — avoids borrow issues.
        // ALL disk I/O happens inside the spawned task.
        let player = self.player.clone();
        let quality = self.config.quality;
        let volume = self.config.volume;
        let song_id = song.id;
        let cover_url = song.album.cover_url.clone();
        let cache_dir = self.audio_cache.dir().clone();
        let picker = self.page_stack.iter().find_map(|page| {
            if let Page::Player(pp) = page {
                Some(pp.picker().clone())
            } else {
                None
            }
        });

        // Spawn background task — do_play_song returns IMMEDIATELY.
        self.pending_play = Some(tokio::spawn(async move {
            // ── Disk cache reads (all parallel) ──
            let audio_path = cache_dir.join(format!("{song_id}.mp3"));
            let lyrics_path = cache_dir.join(format!("{song_id}.lrc"));
            let cover_path = cache_dir.join(format!("{song_id}.cover"));

            let (audio_result, lyrics_bytes, cover_bytes) = tokio::join!(
                tokio::fs::read(&audio_path),
                tokio::fs::read(&lyrics_path),
                tokio::fs::read(&cover_path),
            );

            let cached_audio = audio_result.ok();
            let cached_lyrics = lyrics_bytes.ok();
            let cached_cover = cover_bytes.ok();

            // ── Audio playback ──
            let mut audio_for_cache = None;
            if let Some(bytes) = cached_audio {
                tracing::info!(song_id, "Playing from audio cache");
                if let Some(ref p) = player {
                    if let Err(e) = p.play_from_bytes(bytes).await {
                        tracing::warn!(error = %e, "Playback from cache failed");
                    } else {
                        p.set_volume(volume);
                    }
                }
            } else {
                // Cache miss: fetch URL + download + play.
                let url_result = client.song_url(song_id, quality).await;
                match url_result {
                    Ok(url) => match client.http_client().get(&url).send().await {
                        Ok(resp) => match resp.bytes().await {
                            Ok(b) => {
                                let bytes = b.to_vec();
                                let play_bytes = bytes.clone();
                                audio_for_cache = Some(bytes);
                                if let Some(ref p) = player {
                                    if let Err(e) = p.play_from_bytes(play_bytes).await {
                                        tracing::warn!(error = %e, "Playback failed");
                                    } else {
                                        p.set_volume(volume);
                                    }
                                }
                            }
                            Err(e) => tracing::warn!(error = %e, "Failed to read audio body"),
                        },
                        Err(e) => tracing::warn!(error = %e, "Failed to download audio"),
                    },
                    Err(e) => tracing::warn!(error = %e, song_id, "Failed to get song URL"),
                }
            }

            // ── Fetch any missing metadata ──
            let lyrics = match cached_lyrics {
                Some(ref bytes) => serde_json::from_slice::<Lyrics>(bytes).ok(),
                None => client.lyrics(song_id).await.ok(),
            };
            let cover = match cached_cover {
                Some(bytes) => Some(bytes),
                None => {
                    if let Some(url) = cover_url {
                        match client.http_client().get(&url).send().await {
                            Ok(resp) => resp.bytes().await.ok().map(|b| b.to_vec()),
                            Err(_) => None,
                        }
                    } else {
                        None
                    }
                }
            };

            // Cache cover bytes to disk for future plays.
            if let Some(ref bytes) = cover {
                let cover_path = cache_dir.join(format!("{song_id}.cover"));
                let _ = tokio::fs::write(&cover_path, bytes).await;
            }

            // Decode cover image in the background task so the event loop
            // isn't blocked by CPU-intensive image processing.
            let cover_protocol = cover.and_then(|bytes| {
                use ratatui::layout::Size;
                use ratatui_image::Resize;
                let picker = picker?;
                let t = std::time::Instant::now();
                let img = image::load_from_memory(&bytes).ok()?;
                let size = Size::new(8, 6);
                let protocol = picker.new_protocol(img, size, Resize::Fit(None)).ok();
                if protocol.is_some() {
                    tracing::info!(ms = t.elapsed().as_millis(), "Cover decoded in background task");
                }
                protocol
            });

            PendingPlayResult {
                song_id,
                _song: song,
                audio_bytes: audio_for_cache,
                lyrics,
                cover_protocol,
            }
        }));
    }

    /// Check whether the background play task has completed; if so, apply its
    /// results (start playback, set lyrics, clear loading).
    async fn poll_pending_play(&mut self) {
        let Some(ref handle) = self.pending_play else {
            return;
        };
        if !handle.is_finished() {
            return;
        }
        // Take the handle out so we can await it.
        let Some(handle) = self.pending_play.take() else {
            return;
        };
        match handle.await {
            Ok(result) => {
                let t_process = Instant::now();
                // Check if the current player page is still showing this song.
                let current_song_id = self.page_stack.iter().find_map(|page| {
                    if let Page::Player(pp) = page {
                        pp.song().map(|s| s.id)
                    } else {
                        None
                    }
                });
                if current_song_id != Some(result.song_id) {
                    tracing::info!(expected = result.song_id, actual = ?current_song_id, "Stale play result, skipping");
                    return;
                }

                // Cache audio bytes if provided (cache-miss path).
                // NOTE: play_from_bytes is now called inside the spawned task,
                // NOT here. We only cache and apply metadata.
                if let Some(ref bytes) = result.audio_bytes {
                    self.audio_cache.put(result.song_id, bytes).await;
                }

                // Apply lyrics (and cache them).
                if let Some(ref lyrics) = result.lyrics {
                    if let Ok(json) = serde_json::to_vec(lyrics) {
                        self.audio_cache.put_lyrics(result.song_id, &json).await;
                    }
                    let lyrics = lyrics.clone();
                    for page in &mut self.page_stack {
                        if let Page::Player(pp) = page {
                            pp.set_lyrics(lyrics);
                            break;
                        }
                    }
                }

                // Apply cover art (already decoded in background task).
                if let Some(protocol) = result.cover_protocol {
                    for page in &mut self.page_stack {
                        if let Page::Player(pp) = page {
                            pp.set_cover(protocol);
                            break;
                        }
                    }
                } else {
                    tracing::debug!("No cover available");
                }

                // Clear loading state.
                self.set_player_loading(false);

                // Trigger background pre-cache for the next song.
                self.pre_cache_next();

                tracing::info!(ms = t_process.elapsed().as_millis(), "poll_pending_play processed");
            }
            Err(e) => {
                if e.is_cancelled() {
                    tracing::info!("Background play task cancelled (song changed)");
                    // Don't clear loading — the new song's task will handle it.
                } else {
                    tracing::error!("Background play task panicked: {e}");
                    self.set_player_loading(false);
                }
            }
        }
    }

    /// Set or clear the loading spinner on the current PlayerPage.
    fn set_player_loading(&mut self, loading: bool) {
        for page in &mut self.page_stack {
            if let Page::Player(pp) = page {
                pp.set_loading(loading);
                break;
            }
        }
    }

    /// Check whether the background search task has completed; if so, apply
    /// results and clear loading state.
    async fn poll_pending_search(&mut self) {
        let Some(ref handle) = self.pending_search else {
            return;
        };
        if !handle.is_finished() {
            return;
        }
        let Some(handle) = self.pending_search.take() else {
            return;
        };
        match handle.await {
            Ok(Ok(result)) => {
                tracing::info!(
                    count = result.songs.len(),
                    total = result.total,
                    "Search OK"
                );
                if let Some(Page::Search(sp)) = self.page_stack.last_mut() {
                    sp.set_results(result.songs);
                }
            }
            Ok(Err(e)) => {
                tracing::warn!(error = %e, "Search failed");
                if let Some(Page::Search(sp)) = self.page_stack.last_mut() {
                    sp.set_results(Vec::new());
                }
            }
            Err(e) => {
                tracing::error!("Background search task panicked: {e}");
                if let Some(Page::Search(sp)) = self.page_stack.last_mut() {
                    sp.set_results(Vec::new());
                }
            }
        }
    }

    /// Pre-cache the next song in the queue so it plays instantly when reached.
    fn pre_cache_next(&mut self) {
        let Some(ref client) = self.api_client else {
            return;
        };
        let next_song = match self.play_queue.peek_next() {
            Some(s) => s.clone(),
            None => return,
        };

        // Already cached — nothing to do.
        if self.audio_cache.contains(next_song.id) {
            return;
        }

        // Abort any in-flight pre-cache task.
        if let Some(handle) = self.pending_precache.take() {
            handle.abort();
        }

        let song_id = next_song.id;
        let quality = self.config.quality;
        let cover_url = next_song.album.cover_url.clone();
        let client = Arc::clone(client);

        self.pending_precache = Some(tokio::spawn(async move {
            // Download audio, lyrics, and cover in parallel.
            let url = match client.song_url(song_id, quality).await {
                Ok(u) => u,
                Err(e) => {
                    tracing::warn!(error = %e, song_id, "Pre-cache: failed to get song URL");
                    return None;
                }
            };

            let audio_fut = async {
                match client.http_client().get(&url).send().await {
                    Ok(resp) => resp.bytes().await.map(|b| b.to_vec()),
                    Err(e) => Err(e),
                }
            };
            let lyrics_fut = client.lyrics(song_id);
            let cover_fut = async {
                if let Some(ref curl) = cover_url {
                    match client.http_client().get(curl).send().await {
                        Ok(resp) => resp.bytes().await.ok().map(|b| b.to_vec()),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            };

            let (audio_result, lyrics_result, cover_bytes) =
                tokio::join!(audio_fut, lyrics_fut, cover_fut);

            let audio_bytes = match audio_result {
                Ok(b) => b,
                Err(e) => {
                    tracing::warn!(error = %e, song_id, "Pre-cache: failed to download audio");
                    return None;
                }
            };

            let lyrics = lyrics_result
                .map_err(|e| tracing::warn!(error = %e, song_id, "Pre-cache: failed to fetch lyrics"))
                .ok();

            tracing::info!(
                song_id,
                audio_len = audio_bytes.len(),
                has_lyrics = lyrics.is_some(),
                has_cover = cover_bytes.is_some(),
                "Pre-cache: downloaded next song"
            );
            Some(PreCacheResult {
                song_id,
                audio_bytes,
                lyrics,
                cover_bytes,
            })
        }));
    }

    /// Check whether the background pre-cache task has completed; if so, write to disk.
    async fn poll_pending_precache(&mut self) {
        let Some(ref handle) = self.pending_precache else {
            return;
        };
        if !handle.is_finished() {
            return;
        }
        let Some(handle) = self.pending_precache.take() else {
            return;
        };
        match handle.await {
            Ok(Some(result)) => {
                self.audio_cache.put(result.song_id, &result.audio_bytes).await;
                if let Some(ref lyrics) = result.lyrics {
                    if let Ok(json) = serde_json::to_vec(lyrics) {
                        self.audio_cache.put_lyrics(result.song_id, &json).await;
                    }
                }
                if let Some(ref cover_bytes) = result.cover_bytes {
                    self.audio_cache.put_cover(result.song_id, cover_bytes).await;
                }
                tracing::info!(song_id = result.song_id, "Pre-cache: written to disk cache");
            }
            Ok(None) => {}
            Err(e) => {
                if !e.is_cancelled() {
                    tracing::warn!("Pre-cache task panicked: {e}");
                }
            }
        }
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
                pp.clear_lyrics();
                found = true;
                break;
            }
        }
        // If no PlayerPage exists, push one.
        if !found {
            let mut pp = crate::pages::player::PlayerPage::new();
            pp.set_song(song);
            pp.set_play_mode(self.play_queue.mode());
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

        // Auto-advance: when the current song finishes, play the next one.
        if let Some(ref player) = self.player {
            let pos = player.position();
            let dur = player.duration();
            // Song finished: position within a 1.5s window past duration
            if dur > 0.0 && pos >= dur - 0.5 && pos < dur + 2.0 {
                tracing::info!(pos, dur, "Song finished, auto-advancing");
                return PageAction::PlayNext;
            }
        }

        if let Some(page) = self.page_stack.last_mut() {
            page.tick()
        } else {
            PageAction::None
        }
    }

    // ── Main loop ───────────────────────────────────────────────────────

    pub async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<()>
    where
        B::Error: Send + Sync + 'static,
    {
        // Auto-login from saved profile + cookies.
        {
            let cookie_path = dirs::home_dir()
                .unwrap_or_default()
                .join(".netune")
                .join("cookies.txt");

            if cookie_path.exists() {
                if let Some(ref client) = self.api_client {
                    let _ = client.load_cookies(&cookie_path);
                }
            }

            if let Some(profile) = load_profile() {
                tracing::info!(nickname = %profile.nickname, "Auto-login from saved profile");
                self.user = Some(profile.clone());
                for page in &mut self.page_stack {
                    if let Page::Home(hp) = page {
                        hp.set_user(Some(profile));
                        break;
                    }
                }
                self.fetch_user_playlists().await;
            }
        }

        // Restore queue from last session.
        let queue_path = queue_file_path();
        match PlayQueue::load_from_file(&queue_path) {
            Ok(queue) => {
                tracing::info!(songs = queue.len(), "Restored queue from file");
                self.play_queue = queue;
            }
            Err(e) => {
                // Not fatal — start with an empty queue.
                tracing::warn!(error = %e, "Could not restore queue (starting fresh)");
            }
        }

        loop {
            let t_draw = Instant::now();
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

                    // Render queue panel overlay on top if open.
                    if let Some(ref qp) = self.queue_panel {
                        let full_area = f.area();
                        let songs = self.play_queue.songs();
                        qp.render(f, full_area, songs);
                    }
                }
            })?;
            let draw_ms = t_draw.elapsed().as_millis();

            if !event::poll(Duration::from_millis(100))? {
                let t_idle = Instant::now();
                let tick_action = self.tick();
                if !matches!(tick_action, PageAction::None) {
                    self.apply_action(tick_action).await;
                }
                self.poll_pending_play().await;
                self.poll_pending_search().await;
                self.poll_pending_precache().await;
                let idle_ms = t_idle.elapsed().as_millis();
                if draw_ms > 16 || idle_ms > 16 {
                    tracing::warn!(draw_ms, idle_ms, "Slow frame (idle)");
                }
                continue;
            }

            let t_read = Instant::now();
            let evt = event::read()?;
            let read_ms = t_read.elapsed().as_millis();

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

            let action = if self.queue_panel.is_some() {
                // Queue panel is open — forward events to it first.
                let result = self.queue_panel.as_mut().unwrap().handle_event(&evt);
                match result {
                    QueuePanelResult::Close => {
                        self.queue_panel = None;
                        PageAction::None
                    }
                    QueuePanelResult::JumpTo(idx) => {
                        self.queue_panel = None;
                        PageAction::JumpToQueueItem(idx)
                    }
                    QueuePanelResult::NotHandled => {
                        // Queue panel didn't handle it — still consume the
                        // event so it doesn't leak to the underlying page
                        // (except Ctrl+C which is handled above).
                        PageAction::None
                    }
                }
            } else if let Some(page) = self.page_stack.last_mut() {
                page.handle_event(&evt).await
            } else {
                PageAction::None
            };

            let t_apply = Instant::now();
            self.apply_action(action).await;
            let apply_ms = t_apply.elapsed().as_millis();

            let t_poll = Instant::now();
            self.poll_pending_play().await;
            self.poll_pending_search().await;
            self.poll_pending_precache().await;
            let poll_ms = t_poll.elapsed().as_millis();

            if draw_ms > 16 || read_ms > 16 || apply_ms > 16 || poll_ms > 16 {
                tracing::warn!(draw_ms, read_ms, apply_ms, poll_ms, "Slow frame (event)");
            }

            if self.should_quit {
                break;
            }
        }

        // Persist queue for next session.
        if let Err(e) = self.play_queue.save_to_file(&queue_file_path()) {
            tracing::warn!(error = %e, "Failed to save queue on exit");
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
                let is_player = matches!(page, Page::Player(_));
                self.page_stack.push(page);
                // Auto-generate QR code when login page opens.
                if is_login {
                    self.do_qr_refresh().await;
                }
                // Sync current playback state into a newly pushed PlayerPage.
                if is_player {
                    self.sync_player_state();
                    for page in &mut self.page_stack {
                        if let Page::Player(pp) = page {
                            pp.set_play_mode(self.play_queue.mode());
                            if pp.song().is_none() {
                                if let Some(song) = self.play_queue.current() {
                                    pp.set_song(song.clone());
                                }
                            }
                            break;
                        }
                    }
                    // If nothing is playing but the queue has songs, start
                    // playing from the current position.
                    let should_autoplay = self.player.as_ref().is_none_or(|p| !p.is_playing())
                        && self.play_queue.current().is_some();
                    if should_autoplay {
                        if let Some(song) = self.play_queue.current().cloned() {
                            self.do_play_song(song).await;
                        }
                    }
                }
            }
            PageAction::Pop => {
                if self.page_stack.len() > 1 {
                    self.page_stack.pop();
                } else {
                    self.should_quit = true;
                }
            }
            PageAction::Logout => {
                let _ = std::fs::remove_file(profile_file_path());
                self.user = None;
                for page in &mut self.page_stack {
                    if let Page::Home(hp) = page {
                        hp.set_user(None);
                        break;
                    }
                }
                self.page_stack.push(Page::Login(crate::pages::login::LoginPage::new()));
            }
            PageAction::Replace(page) => {
                if let Some(top) = self.page_stack.last_mut() {
                    *top = page;
                }
            }

            // ── Browser cookie import ───────────────────────────────────
            PageAction::BrowserImportConfirm(browser) => {
                let Some(Page::Login(lp)) = self.page_stack.last_mut() else {
                    return;
                };
                let Some(ref client) = self.api_client else {
                    lp.set_error("No API client configured".into());
                    return;
                };
                match client.import_browser_cookies(&browser).await {
                    Ok(Some(profile)) => {
                        tracing::info!(
                            nickname = %profile.nickname,
                            uid = profile.uid,
                            "Browser cookie import succeeded"
                        );
                        lp.set_success();
                        self.user = Some(profile.clone());
                        // Save cookies for future auto-login.
                        let cookie_path = dirs::home_dir()
                            .unwrap_or_default()
                            .join(".netune")
                            .join("cookies.txt");
                        if let Some(parent) = cookie_path.parent() {
                            let _ = std::fs::create_dir_all(parent);
                        }
                        if let Err(e) = client.save_cookies(&cookie_path) {
                            tracing::warn!(error = %e, "Failed to save cookies");
                        }
                        save_profile(&profile);
                        // Navigate to home and update user info.
                        self.page_stack.clear();
                        let mut home = crate::pages::home::HomePage::new();
                        home.set_user(Some(profile));
                        self.page_stack.push(Page::Home(home));
                        // Fetch playlists in background.
                        self.fetch_user_playlists().await;
                    }
                    Ok(None) => {
                        lp.set_error("Browser import returned no profile".into());
                    }
                    Err(e) => {
                        tracing::warn!(error = %e, "Browser cookie import failed");
                        if let Some(Page::Login(lp)) = self.page_stack.last_mut() {
                            let msg = format!(
                                "{e}\n\
                                 • Make sure you are logged into music.163.com\n\
                                 • Close the browser before importing\n\
                                 • Try a different browser"
                            );
                            lp.set_error(msg);
                        }
                    }
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
                        // Save cookies for future auto-login.
                        if let Some(ref client) = self.api_client {
                            let cookie_path = dirs::home_dir()
                                .unwrap_or_default()
                                .join(".netune")
                                .join("cookies.txt");
                            if let Some(parent) = cookie_path.parent() {
                                let _ = std::fs::create_dir_all(parent);
                            }
                            if let Err(e) = client.save_cookies(&cookie_path) {
                                tracing::warn!(error = %e, "Failed to save cookies");
                            }
                        }
                        save_profile(&profile);
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
                // Abort any in-flight search.
                if let Some(handle) = self.pending_search.take() {
                    handle.abort();
                }
                sp.set_loading(true);
                let client = Arc::clone(client);
                self.pending_search = Some(tokio::spawn(async move {
                    client.search_songs(&keyword, 0, 30).await
                }));
            }

            // ── Search results ready ─────────────────────────────────────
            PageAction::SearchReady(songs) => {
                let Some(Page::Search(sp)) = self.page_stack.last_mut() else {
                    return;
                };
                sp.set_results(songs);
            }

            // ── Play song ───────────────────────────────────────────────
            PageAction::PlaySong(song) => {
                self.do_play_song(song).await;
            }

            // ── Add to queue ─────────────────────────────────────────────
            PageAction::AddToQueue(song) => {
                tracing::info!(song_id = song.id, title = %song.name, "Added to queue");
                self.play_queue.push(song);
            }

            // ── Play queue ──────────────────────────────────────────────
            PageAction::PlayQueue(songs) => {
                self.play_queue.load(songs);
                self.do_play_next().await;
            }

            // ── Play queue from specific index ────────────────────────
            PageAction::PlayQueueFrom(songs, idx) => {
                self.play_queue.load(songs);
                if let Some(song) = self.play_queue.skip_to(idx) {
                    let song = song.clone();
                    self.do_play_song(song).await;
                }
            }

            // ── Auto-advance to next song ───────────────────────────────
            PageAction::PlayNext => {
                self.do_play_next().await;
            }

            // ── Previous track ──────────────────────────────────────────
            PageAction::PlayPrev => {
                self.do_play_prev().await;
            }

            // ── Fetch playlist detail ───────────────────────────────────
            PageAction::FetchPlaylistDetail(playlist_id) => {
                let Some(ref client) = self.api_client else {
                    return;
                };
                match client.playlist_detail(playlist_id).await {
                    Ok(tracks) => {
                        self.play_queue.load(tracks.clone());
                        self.do_play_next().await;
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
                // Determine what to do WITHOUT holding a borrow on self.player.
                let toggle_action: Option<ToggleAction> = self.player.as_ref().map(|player| {
                    if player.duration() > 0.0 {
                        ToggleAction::Toggle
                    } else if self.pending_play.is_some() {
                        ToggleAction::Ignore
                    } else {
                        // No active state and no pending load — need recovery.
                        let song = self.page_stack.iter().find_map(|page| {
                            if let Page::Player(pp) = page {
                                pp.song().cloned()
                            } else {
                                None
                            }
                        });
                        ToggleAction::Replay(song)
                    }
                });

                if let Some(action) = toggle_action {
                    match action {
                        ToggleAction::Toggle => {
                            if let Some(ref player) = self.player {
                                player.toggle_pause();
                            }
                        }
                        ToggleAction::Ignore => {
                            tracing::debug!("TogglePause ignored: song is loading");
                        }
                        ToggleAction::Replay(Some(song)) => {
                            tracing::warn!("TogglePause with no active state, replaying current song");
                            self.do_play_song(song).await;
                        }
                        ToggleAction::Replay(None) => {
                            tracing::debug!("TogglePause: no current song to replay");
                        }
                    }
                    // Update UI immediately
                    if let Some(ref player) = self.player {
                        let playing = player.is_playing();
                        let vol = (player.volume() * 100.0) as u16;
                        for page in &mut self.page_stack {
                            if let Page::Player(pp) = page {
                                pp.update_from_player(player.position(), player.duration(), playing);
                                pp.set_volume(vol);
                                break;
                            }
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

            // ── Theme switch ─────────────────────────────────────────────
            PageAction::SetTheme(name) => {
                tracing::info!(theme = %name, "Switching theme");
                crate::theme::Theme::set_theme(&name);
            }

            // ── Cycle play mode ───────────────────────────────────────
            PageAction::CyclePlayMode => {
                self.play_queue.cycle_mode();
                let mode = self.play_queue.mode();
                tracing::info!(?mode, "Play mode changed");
                for page in &mut self.page_stack {
                    if let Page::Player(pp) = page {
                        pp.set_play_mode(mode);
                        break;
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

            // ── Queue panel toggle ─────────────────────────────────────
            PageAction::ToggleQueuePanel => {
                if self.queue_panel.is_some() {
                    self.queue_panel = None;
                    tracing::info!("Queue panel closed");
                } else {
                    let idx = self.play_queue.current_index();
                    let total = self.play_queue.len();
                    self.queue_panel = Some(QueuePanel::new(idx, total));
                    tracing::info!("Queue panel opened");
                }
            }

            // ── Jump to queue item ──────────────────────────────────────
            PageAction::JumpToQueueItem(index) => {
                if self.play_queue.jump(index).is_some() {
                    let song = self.play_queue.current().cloned();
                    if let Some(song) = song {
                        self.do_play_song(song).await;
                    }
                }
            }
        }
    }
}
