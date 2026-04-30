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
