//! API response models (serde deserialization targets).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ApiLoginResponse {
    pub code: i32,
    pub profile: Option<ApiProfile>,
    pub msg: Option<String>,
}

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
