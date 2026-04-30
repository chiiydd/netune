//! Core traits — the contract between layers.

use async_trait::async_trait;

use crate::Result;
use crate::models::*;

/// API client trait — all network operations go through here.
#[async_trait]
pub trait NeteaseClient: Send + Sync {
    /// Current login state.
    fn login_state(&self) -> &LoginState;

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NetuneError;
    use crate::models::{LoginState, Lyrics, Playlist, QualityLevel, SearchResult, Song, UserProfile};
    use std::sync::Arc;

    /// A minimal mock implementation of `NeteaseClient` for compile-time
    /// trait-bound verification.
    struct MockNeteaseClient {
        login_state: LoginState,
    }

    #[async_trait]
    impl NeteaseClient for MockNeteaseClient {
        fn login_state(&self) -> &LoginState {
            &self.login_state
        }

        async fn login_qr_generate(&self) -> Result<String> {
            Ok("mock-qr-key".into())
        }

        async fn login_qr_check(&self, _key: &str) -> Result<Option<UserProfile>> {
            Ok(None)
        }

        async fn logout(&self) -> Result<()> {
            Ok(())
        }

        async fn user_playlists(&self, _uid: u64) -> Result<Vec<Playlist>> {
            Ok(vec![])
        }

        async fn playlist_detail(&self, _playlist_id: u64) -> Result<Vec<Song>> {
            Ok(vec![])
        }

        async fn search_songs(&self, _keyword: &str, _page: u32, _size: u32) -> Result<SearchResult> {
            Ok(SearchResult {
                songs: vec![],
                total: 0,
                has_more: false,
            })
        }

        async fn song_url(&self, _song_id: u64, _quality: QualityLevel) -> Result<String> {
            Ok("https://example.com/song.mp3".into())
        }

        async fn lyrics(&self, _song_id: u64) -> Result<Lyrics> {
            Ok(Lyrics {
                lines: vec![],
                translated: None,
            })
        }

        async fn daily_recommend(&self) -> Result<DailyRecommend> {
            Ok(DailyRecommend { songs: vec![] })
        }

        async fn personal_fm(&self) -> Result<Vec<Song>> {
            Ok(vec![])
        }
    }

    /// A minimal mock implementation of `AudioPlayer`.
    struct MockAudioPlayer {
        volume: std::sync::Mutex<f32>,
        playing: std::sync::atomic::AtomicBool,
    }

    #[async_trait]
    impl AudioPlayer for MockAudioPlayer {
        async fn play(&self, _url: &str) -> Result<()> {
            self.playing.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }

        fn pause(&self) {
            self.playing.store(false, std::sync::atomic::Ordering::SeqCst);
        }

        fn resume(&self) {
            self.playing.store(true, std::sync::atomic::Ordering::SeqCst);
        }

        fn toggle_pause(&self) {
            let current = self.playing.load(std::sync::atomic::Ordering::SeqCst);
            self.playing.store(!current, std::sync::atomic::Ordering::SeqCst);
        }

        fn stop(&self) {
            self.playing.store(false, std::sync::atomic::Ordering::SeqCst);
        }

        fn seek(&self, _seconds: f64) -> Result<()> {
            Ok(())
        }

        fn set_volume(&self, volume: f32) {
            *self.volume.lock().unwrap() = volume;
        }

        fn volume(&self) -> f32 {
            *self.volume.lock().unwrap()
        }

        fn position(&self) -> f64 {
            0.0
        }

        fn duration(&self) -> f64 {
            0.0
        }

        fn is_playing(&self) -> bool {
            self.playing.load(std::sync::atomic::Ordering::SeqCst)
        }

        fn is_paused(&self) -> bool {
            !self.playing.load(std::sync::atomic::Ordering::SeqCst)
        }
    }

    #[test]
    fn test_netease_client_trait_bounds() {
        // Verify that our mock satisfies Send + Sync.
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockNeteaseClient>();
    }

    #[test]
    fn test_audio_player_trait_bounds() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<MockAudioPlayer>();
    }

    #[tokio::test]
    async fn test_mock_netease_client_login_state() {
        let client = MockNeteaseClient {
            login_state: LoginState::LoggedOut,
        };
        match client.login_state() {
            LoginState::LoggedOut => {}
            _ => panic!("expected LoggedOut"),
        }
    }

    #[tokio::test]
    async fn test_mock_netease_client_search() {
        let client = MockNeteaseClient {
            login_state: LoginState::LoggedOut,
        };
        let result = client.search_songs("test", 1, 20).await.unwrap();
        assert_eq!(result.total, 0);
        assert!(!result.has_more);
        assert!(result.songs.is_empty());
    }

    #[tokio::test]
    async fn test_mock_audio_player_playback() {
        let player = MockAudioPlayer {
            volume: std::sync::Mutex::new(0.5),
            playing: std::sync::atomic::AtomicBool::new(false),
        };

        assert!(!player.is_playing());
        assert!(player.is_paused());

        player.play("https://example.com/song.mp3").await.unwrap();
        assert!(player.is_playing());
        assert!(!player.is_paused());

        player.pause();
        assert!(!player.is_playing());

        player.resume();
        assert!(player.is_playing());

        player.toggle_pause();
        assert!(!player.is_playing());

        player.toggle_pause();
        assert!(player.is_playing());

        player.stop();
        assert!(!player.is_playing());
    }

    #[test]
    fn test_mock_audio_player_volume() {
        let player = MockAudioPlayer {
            volume: std::sync::Mutex::new(0.5),
            playing: std::sync::atomic::AtomicBool::new(false),
        };
        assert!((player.volume() - 0.5).abs() < f32::EPSILON);
        player.set_volume(0.8);
        assert!((player.volume() - 0.8).abs() < f32::EPSILON);
    }

    /// Demonstrate using `Arc<dyn NeteaseClient>` to verify trait-object safety.
    #[test]
    fn test_netease_client_is_object_safe() {
        let client: Arc<dyn NeteaseClient> = Arc::new(MockNeteaseClient {
            login_state: LoginState::LoggedOut,
        });
        match client.login_state() {
            LoginState::LoggedOut => {}
            _ => panic!("expected LoggedOut"),
        }
    }

    /// Demonstrate using `Arc<dyn AudioPlayer>` to verify trait-object safety.
    #[test]
    fn test_audio_player_is_object_safe() {
        let player: Arc<dyn AudioPlayer> = Arc::new(MockAudioPlayer {
            volume: std::sync::Mutex::new(1.0),
            playing: std::sync::atomic::AtomicBool::new(false),
        });
        assert!(!player.is_playing());
        assert!((player.volume() - 1.0).abs() < f32::EPSILON);
    }
}
