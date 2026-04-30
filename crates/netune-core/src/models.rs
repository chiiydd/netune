//! Core data models.

use serde::{Deserialize, Serialize};

/// A song/track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    pub id: u64,
    pub name: String,
    pub artists: Vec<Artist>,
    pub album: Album,
    /// Duration in milliseconds.
    pub duration: u64,
    /// Available quality levels.
    pub quality: QualityLevel,
}

/// An artist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Artist {
    pub id: u64,
    pub name: String,
}

/// An album.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Album {
    pub id: u64,
    pub name: String,
    pub cover_url: Option<String>,
}

/// Audio quality level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QualityLevel {
    /// 128kbps
    Standard,
    /// 192kbps
    Higher,
    /// 320kbps
    ExHigh,
    /// Lossless FLAC
    Lossless,
    /// Hi-Res
    HiRes,
}

impl QualityLevel {
    pub fn bitrate(&self) -> u32 {
        match self {
            Self::Standard => 128_000,
            Self::Higher => 192_000,
            Self::ExHigh => 320_000,
            Self::Lossless => 999_000,
            Self::HiRes => 999_000,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Standard => "Standard",
            Self::Higher => "Higher",
            Self::ExHigh => "ExHigh",
            Self::Lossless => "Lossless",
            Self::HiRes => "HiRes",
        }
    }
}

/// A playlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Playlist {
    pub id: u64,
    pub name: String,
    pub cover_url: Option<String>,
    pub track_count: u32,
    pub creator: Option<UserProfile>,
}

/// User profile (minimal).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserProfile {
    pub uid: u64,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

/// Lyric line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LyricLine {
    /// Offset from start in milliseconds.
    pub timestamp: u64,
    pub text: String,
}

/// Full lyrics (original + translated).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lyrics {
    pub lines: Vec<LyricLine>,
    pub translated: Option<Vec<LyricLine>>,
}

/// Search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResult {
    pub songs: Vec<Song>,
    pub total: u32,
    pub has_more: bool,
}

/// Login state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoginState {
    LoggedOut,
    LoggedIn(UserProfile),
}

/// Daily recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DailyRecommend {
    pub songs: Vec<Song>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to build a minimal Song for tests.
    fn make_song() -> Song {
        Song {
            id: 123456,
            name: "Test Song".into(),
            artists: vec![Artist {
                id: 1,
                name: "Test Artist".into(),
            }],
            album: Album {
                id: 10,
                name: "Test Album".into(),
                cover_url: Some("https://example.com/cover.jpg".into()),
            },
            duration: 210_000,
            quality: QualityLevel::ExHigh,
        }
    }

    #[test]
    fn test_song_creation() {
        let song = make_song();
        assert_eq!(song.id, 123456);
        assert_eq!(song.name, "Test Song");
        assert_eq!(song.artists.len(), 1);
        assert_eq!(song.artists[0].name, "Test Artist");
        assert_eq!(song.album.name, "Test Album");
        assert_eq!(song.album.cover_url.as_deref(), Some("https://example.com/cover.jpg"));
        assert_eq!(song.duration, 210_000);
        assert_eq!(song.quality, QualityLevel::ExHigh);
    }

    #[test]
    fn test_song_serialization_roundtrip() {
        let song = make_song();
        let json = serde_json::to_string(&song).expect("serialize");
        let back: Song = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.id, song.id);
        assert_eq!(back.name, song.name);
        assert_eq!(back.duration, song.duration);
        assert_eq!(back.quality, song.quality);
    }

    #[test]
    fn test_search_result_has_more() {
        // When total > songs.len(), has_more should logically be true.
        // The struct stores the flag directly; verify it survives serde.
        let result = SearchResult {
            songs: vec![make_song()],
            total: 100,
            has_more: true,
        };
        assert!(result.has_more);
        assert_eq!(result.total, 100);

        let result_no_more = SearchResult {
            songs: vec![],
            total: 0,
            has_more: false,
        };
        assert!(!result_no_more.has_more);

        // Round-trip via JSON to ensure the flag is preserved.
        let json = serde_json::to_string(&result).unwrap();
        let back: SearchResult = serde_json::from_str(&json).unwrap();
        assert!(back.has_more);
    }

    #[test]
    fn test_quality_level_serialization() {
        // Verify enum variants survive serde round-trips.
        for variant in [
            QualityLevel::Standard,
            QualityLevel::Higher,
            QualityLevel::ExHigh,
            QualityLevel::Lossless,
            QualityLevel::HiRes,
        ] {
            let json = serde_json::to_string(&variant).unwrap();
            let back: QualityLevel = serde_json::from_str(&json).unwrap();
            assert_eq!(back, variant);
        }
    }

    #[test]
    fn test_login_state_serialization() {
        let logged_out = LoginState::LoggedOut;
        let json = serde_json::to_string(&logged_out).unwrap();
        let back: LoginState = serde_json::from_str(&json).unwrap();
        match back {
            LoginState::LoggedOut => {}
            _ => panic!("expected LoggedOut"),
        }

        let logged_in = LoginState::LoggedIn(UserProfile {
            uid: 42,
            nickname: "user".into(),
            avatar_url: None,
        });
        let json = serde_json::to_string(&logged_in).unwrap();
        let back: LoginState = serde_json::from_str(&json).unwrap();
        match back {
            LoginState::LoggedIn(profile) => {
                assert_eq!(profile.uid, 42);
                assert_eq!(profile.nickname, "user");
            }
            _ => panic!("expected LoggedIn"),
        }
    }

    #[test]
    fn test_lyric_line_creation() {
        let line = LyricLine {
            timestamp: 12_500,
            text: "Hello world".into(),
        };
        assert_eq!(line.timestamp, 12_500);
        assert_eq!(line.text, "Hello world");

        // Round-trip through JSON.
        let json = serde_json::to_string(&line).unwrap();
        let back: LyricLine = serde_json::from_str(&json).unwrap();
        assert_eq!(back.timestamp, 12_500);
        assert_eq!(back.text, "Hello world");
    }

    #[test]
    fn test_lyrics_with_translation() {
        let lyrics = Lyrics {
            lines: vec![
                LyricLine { timestamp: 0, text: "Line 1".into() },
                LyricLine { timestamp: 5000, text: "Line 2".into() },
            ],
            translated: Some(vec![
                LyricLine { timestamp: 0, text: "第一行".into() },
                LyricLine { timestamp: 5000, text: "第二行".into() },
            ]),
        };
        assert_eq!(lyrics.lines.len(), 2);
        assert!(lyrics.translated.is_some());
        assert_eq!(lyrics.translated.as_ref().unwrap()[0].text, "第一行");
    }

    #[test]
    fn test_quality_level_bitrate() {
        assert_eq!(QualityLevel::Standard.bitrate(), 128_000);
        assert_eq!(QualityLevel::Higher.bitrate(), 192_000);
        assert_eq!(QualityLevel::ExHigh.bitrate(), 320_000);
        assert_eq!(QualityLevel::Lossless.bitrate(), 999_000);
        assert_eq!(QualityLevel::HiRes.bitrate(), 999_000);
    }

    #[test]
    fn test_quality_level_label() {
        assert_eq!(QualityLevel::Standard.label(), "Standard");
        assert_eq!(QualityLevel::Higher.label(), "Higher");
        assert_eq!(QualityLevel::ExHigh.label(), "ExHigh");
        assert_eq!(QualityLevel::Lossless.label(), "Lossless");
        assert_eq!(QualityLevel::HiRes.label(), "HiRes");
    }
}
