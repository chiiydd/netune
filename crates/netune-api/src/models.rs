//! API response models (serde deserialization targets).

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ApiLoginResponse {
    pub code: i32,
    pub profile: Option<ApiProfile>,
    pub msg: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiProfile {
    pub userId: u64,
    pub nickname: String,
    pub avatarUrl: Option<String>,
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
pub struct ApiPlaylist {
    pub id: u64,
    pub name: String,
    pub trackCount: u32,
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
pub struct ApiAlbum {
    pub id: u64,
    pub name: String,
    pub picUrl: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ApiSearchResponse {
    pub code: i32,
    pub result: Option<ApiSearchResult>,
}

#[derive(Debug, Deserialize)]
pub struct ApiSearchResult {
    #[serde(default)]
    pub songs: Vec<ApiTrack>,
    #[serde(default)]
    pub songCount: u32,
}

#[derive(Debug, Deserialize)]
pub struct ApiSongUrlResponse {
    pub code: i32,
    #[serde(default)]
    pub data: Vec<ApiSongUrl>,
}
