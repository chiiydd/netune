//! Core traits — the contract between layers.

use async_trait::async_trait;

use crate::Result;
use crate::models::*;

/// API client trait — all network operations go through here.
#[async_trait]
pub trait NeteaseClient: Send + Sync {
    /// Current login state.
    fn login_state(&self) -> &LoginState;

    /// Login with phone number and password.
    async fn login_phone(&self, phone: &str, password: &str) -> Result<UserProfile>;

    /// Login with email and password.
    async fn login_email(&self, email: &str, password: &str) -> Result<UserProfile>;

    /// Generate a QR code login key.
    async fn login_qr_generate(&self) -> Result<String>;

    /// Check QR code login status. Returns Some(profile) if scanned & confirmed.
    async fn login_qr_check(&self, key: &str) -> Result<Option<UserProfile>>;

    /// Logout.
    async fn logout(&self) -> Result<()>;

    /// Get user playlists.
    async fn user_playlists(&self, uid: u64) -> Result<Vec<Playlist>>;

    /// Get playlist detail (with tracks).
    async fn playlist_detail(&self, playlist_id: u64) -> Result<Vec<Song>>;

    /// Search songs.
    async fn search_songs(&self, keyword: &str, page: u32, size: u32) -> Result<SearchResult>;

    /// Get song playback URL.
    async fn song_url(&self, song_id: u64, quality: QualityLevel) -> Result<String>;

    /// Get lyrics.
    async fn lyrics(&self, song_id: u64) -> Result<Lyrics>;

    /// Get daily recommendations.
    async fn daily_recommend(&self) -> Result<DailyRecommend>;

    /// Get personal FM.
    async fn personal_fm(&self) -> Result<Vec<Song>>;
}

/// Player trait — audio playback operations.
#[async_trait]
pub trait AudioPlayer: Send + Sync {
    /// Play a song by streaming URL.
    async fn play(&self, url: &str) -> Result<()>;

    /// Pause playback.
    fn pause(&self);

    /// Resume playback.
    fn resume(&self);

    /// Toggle pause/resume.
    fn toggle_pause(&self);

    /// Stop playback.
    fn stop(&self);

    /// Seek to position (in seconds).
    fn seek(&self, seconds: f64) -> Result<()>;

    /// Set volume (0.0 - 1.0).
    fn set_volume(&self, volume: f32);

    /// Get current volume.
    fn volume(&self) -> f32;

    /// Get current position in seconds.
    fn position(&self) -> f64;

    /// Get total duration in seconds.
    fn duration(&self) -> f64;

    /// Is currently playing?
    fn is_playing(&self) -> bool;

    /// Is paused?
    fn is_paused(&self) -> bool;
}
