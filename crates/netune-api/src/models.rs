//! API response models (serde deserialization targets).

use serde::Deserialize;

// ─── Error & helpers ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum ApiError {
    Code(i32),
    Message(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ApiError::Code(c) => write!(f, "API error code {c}"),
            ApiError::Message(m) => write!(f, "{m}"),
        }
    }
}

impl std::error::Error for ApiError {}

/// Extracts the inner data payload from a response envelope.
pub trait InnerData {
    type Output;
    fn into_data(self) -> Result<Self::Output, ApiError>;
}

/// Pagination metadata extracted from a response.
pub trait PaginationInfo {
    fn total(&self) -> u32;
    fn has_more(&self, offset: u32, limit: u32) -> bool {
        (offset + limit) < self.total()
    }
}

/// Result of a paginated API request.
#[derive(Debug)]
pub struct PaginationResult<T> {
    pub items: T,
    pub offset: u32,
    pub limit: u32,
    pub total: u32,
}

impl<T> PaginationResult<T> {
    pub fn has_more(&self) -> bool {
        (self.offset + self.limit) < self.total
    }
}

// ─── Response types ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiProfile {
    pub user_id: u64,
    pub nickname: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiSongUrl {
    pub id: u64,
    pub url: Option<String>,
    pub br: Option<u32>,
    #[serde(default)]
    pub fee: i32,
}

#[derive(Debug, Deserialize)]
pub struct ApiLyricResponse {
    pub code: i32,
    pub lrc: Option<ApiLyricBody>,
    pub tlyric: Option<ApiLyricBody>,
}

#[derive(Debug, Deserialize)]
pub struct ApiLyricBody {
    pub lyric: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiPlaylistResponse {
    pub code: i32,
    pub playlist: Option<ApiPlaylist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiPlaylist {
    pub id: u64,
    pub name: String,
    pub track_count: u32,
    #[serde(default)]
    pub tracks: Vec<ApiTrack>,
}

#[derive(Debug, Deserialize)]
pub struct ApiTrack {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub ar: Vec<ApiArtist>,
    #[serde(default)]
    pub al: Option<ApiAlbum>,
    #[serde(default)]
    pub dt: u64,
}

#[derive(Debug, Deserialize)]
pub struct ApiArtist {
    pub id: u64,
    pub name: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAlbum {
    pub id: u64,
    pub name: String,
    pub pic_url: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiSearchResponse {
    pub code: i32,
    pub result: Option<ApiSearchResult>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiSearchResult {
    #[serde(default)]
    pub songs: Vec<ApiTrack>,
    #[serde(default)]
    pub song_count: u32,
}

#[derive(Debug, Deserialize)]
pub struct ApiSongUrlResponse {
    pub code: i32,
    #[serde(default)]
    pub data: Vec<ApiSongUrl>,
}

// ─── QR code login ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApiQrKeyResponse {
    pub code: i32,
    #[serde(default)]
    pub unikey: String,
}

#[derive(Debug, Deserialize)]
pub struct ApiQrCheckResponse {
    /// 801 = waiting, 802 = scanned/confirming, 803 = success (cookie set)
    pub code: i32,
    pub message: Option<String>,
    pub profile: Option<ApiProfile>,
}

// ─── User account ──────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApiUserAccountResponse {
    pub code: i32,
    pub account: Option<ApiAccount>,
    pub profile: Option<ApiProfile>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiAccount {
    pub id: u64,
    pub user_name: Option<String>,
}

// ─── User playlists ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApiUserPlaylistsResponse {
    pub code: i32,
    #[serde(default)]
    pub playlist: Vec<ApiUserPlaylist>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiUserPlaylist {
    pub id: u64,
    pub name: String,
    #[serde(default)]
    pub cover_img_url: Option<String>,
    #[serde(default)]
    pub track_count: u32,
    pub creator: Option<ApiProfile>,
}

// ─── Daily recommend ───────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApiDailyRecommendResponse {
    pub code: i32,
    #[serde(default)]
    pub data: Option<ApiDailyRecommendData>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiDailyRecommendData {
    #[serde(default)]
    pub daily_songs: Vec<ApiTrack>,
}

// ─── Personal FM ───────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ApiPersonalFmResponse {
    pub code: i32,
    #[serde(default)]
    pub data: Vec<ApiTrack>,
}

// ─── InnerData impls ─────────────────────────────────────────────────────────

impl InnerData for ApiSearchResponse {
    type Output = ApiSearchResult;
    fn into_data(self) -> Result<Self::Output, ApiError> {
        if self.code != 200 {
            return Err(ApiError::Code(self.code));
        }
        self.result.ok_or_else(|| ApiError::Message("no search result".into()))
    }
}

impl PaginationInfo for ApiSearchResponse {
    fn total(&self) -> u32 {
        self.result.as_ref().map_or(0, |r| r.song_count)
    }
}

impl InnerData for ApiSongUrlResponse {
    type Output = Vec<ApiSongUrl>;
    fn into_data(self) -> Result<Self::Output, ApiError> {
        if self.code != 200 {
            return Err(ApiError::Code(self.code));
        }
        Ok(self.data)
    }
}

impl InnerData for ApiPlaylistResponse {
    type Output = ApiPlaylist;
    fn into_data(self) -> Result<Self::Output, ApiError> {
        if self.code != 200 {
            return Err(ApiError::Code(self.code));
        }
        self.playlist.ok_or_else(|| ApiError::Message("no playlist data".into()))
    }
}

impl InnerData for ApiUserPlaylistsResponse {
    type Output = Vec<ApiUserPlaylist>;
    fn into_data(self) -> Result<Self::Output, ApiError> {
        if self.code != 200 {
            return Err(ApiError::Code(self.code));
        }
        Ok(self.playlist)
    }
}

impl PaginationInfo for ApiUserPlaylistsResponse {
    fn total(&self) -> u32 {
        self.playlist.len() as u32
    }
    fn has_more(&self, _offset: u32, _limit: u32) -> bool {
        false
    }
}

impl InnerData for ApiDailyRecommendResponse {
    type Output = ApiDailyRecommendData;
    fn into_data(self) -> Result<Self::Output, ApiError> {
        if self.code != 200 {
            return Err(ApiError::Code(self.code));
        }
        self.data.ok_or_else(|| ApiError::Message("no daily recommend data".into()))
    }
}

impl InnerData for ApiPersonalFmResponse {
    type Output = Vec<ApiTrack>;
    fn into_data(self) -> Result<Self::Output, ApiError> {
        if self.code != 200 {
            return Err(ApiError::Code(self.code));
        }
        Ok(self.data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_profile_deserialization() {
        let json = r#"{
            "userId": 12345,
            "nickname": "test_user",
            "avatarUrl": "https://example.com/avatar.jpg"
        }"#;
        let profile: ApiProfile = serde_json::from_str(json).unwrap();
        assert_eq!(profile.user_id, 12345);
        assert_eq!(profile.nickname, "test_user");
        assert_eq!(
            profile.avatar_url.as_deref(),
            Some("https://example.com/avatar.jpg")
        );
    }

    #[test]
    fn test_api_track_to_song() {
        let json = r#"{
            "id": 456,
            "name": "Test Song",
            "ar": [
                {"id": 1, "name": "Artist A"},
                {"id": 2, "name": "Artist B"}
            ],
            "al": {
                "id": 10,
                "name": "Test Album",
                "picUrl": "https://example.com/cover.jpg"
            },
            "dt": 240000
        }"#;
        let track: ApiTrack = serde_json::from_str(json).unwrap();
        assert_eq!(track.id, 456);
        assert_eq!(track.name, "Test Song");
        assert_eq!(track.ar.len(), 2);
        assert_eq!(track.ar[0].name, "Artist A");
        let album = track.al.as_ref().unwrap();
        assert_eq!(album.name, "Test Album");
        assert_eq!(track.dt, 240000);
    }

    #[test]
    fn test_api_search_response() {
        let json = r#"{
            "code": 200,
            "result": {
                "songs": [
                    {
                        "id": 789,
                        "name": "Found Song",
                        "ar": [{"id": 3, "name": "Singer"}],
                        "al": {"id": 20, "name": "Hits", "picUrl": null},
                        "dt": 180000
                    }
                ],
                "songCount": 1
            }
        }"#;
        let resp: ApiSearchResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.code, 200);
        let result = resp.result.unwrap();
        assert_eq!(result.song_count, 1);
        assert_eq!(result.songs.len(), 1);
        assert_eq!(result.songs[0].name, "Found Song");
    }

    #[test]
    fn test_api_song_url_response() {
        let json = r#"{
            "code": 200,
            "data": [
                {
                    "id": 123,
                    "url": "https://m10.music.126.net/song.mp3",
                    "br": 320000,
                    "fee": 0
                },
                {
                    "id": 456,
                    "url": null,
                    "br": null,
                    "fee": 1
                }
            ]
        }"#;
        let resp: ApiSongUrlResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.code, 200);
        assert_eq!(resp.data.len(), 2);
        assert_eq!(resp.data[0].id, 123);
        assert!(resp.data[0].url.is_some());
        assert_eq!(resp.data[0].br, Some(320000));
        assert_eq!(resp.data[1].fee, 1);
        assert!(resp.data[1].url.is_none());
    }

    #[test]
    fn test_api_lyric_response() {
        let json = r#"{
            "code": 200,
            "lrc": {
                "lyric": "[00:00.00]Hello World\n[00:05.30]Second line"
            },
            "tlyric": {
                "lyric": "[00:00.00]你好世界\n[00:05.30]第二行"
            }
        }"#;
        let resp: ApiLyricResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.code, 200);
        let lrc = resp.lrc.unwrap();
        assert!(lrc.lyric.contains("Hello World"));
        let tlyric = resp.tlyric.unwrap();
        assert!(tlyric.lyric.contains("你好世界"));
    }
}
