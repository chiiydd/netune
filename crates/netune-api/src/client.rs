//! Netease Cloud Music API client implementation.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use tokio::sync::RwLock;

use netune_core::Result;
use netune_core::models::*;
use netune_core::traits::NeteaseClient;

use crate::crypto;
use crate::models::*;

/// HTTP-based Netease Cloud Music API client.
pub struct NeteaseApiClient {
    http: Client,
    cookie_jar: Arc<reqwest::cookie::Jar>,
    login_state: Arc<RwLock<LoginState>>,
    base_url: String,
}

impl NeteaseApiClient {
    pub fn new() -> Self {
        let cookie_jar = Arc::new(reqwest::cookie::Jar::default());
        Self {
            http: Client::builder()
                .cookie_provider(Arc::clone(&cookie_jar))
                .build()
                .expect("Failed to build HTTP client"),
            cookie_jar,
            login_state: Arc::new(RwLock::new(LoginState::LoggedOut)),
            base_url: "https://music.163.com".to_string(),
        }
    }

    /// Send a request and return the raw deserialized response.
    async fn inner_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &serde_json::Value,
    ) -> std::result::Result<T, ApiError> {
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| ApiError::Message(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| ApiError::Message(e.to_string()))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ApiError::Message(e.to_string()))?;
        eprintln!("[{}] {} -> {}...", status, path, &body[..body.len().min(200)]);
        serde_json::from_str(&body).map_err(|e| ApiError::Message(e.to_string()))
    }

    /// Send a request, check the code, and extract the inner data.
    async fn request<D: InnerData + serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &serde_json::Value,
    ) -> std::result::Result<D::Output, ApiError> {
        let resp: D = self.inner_request(path, params).await?;
        resp.into_data()
    }

    /// Send a paginated request and return items with pagination metadata.
    async fn pagination<T, D: InnerData<Output = T> + PaginationInfo + serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &serde_json::Value,
        offset: u32,
        limit: u32,
    ) -> std::result::Result<PaginationResult<T>, ApiError> {
        let resp: D = self.inner_request(path, params).await?;
        let total = resp.total();
        let items = resp.into_data()?;
        Ok(PaginationResult {
            items,
            offset,
            limit,
            total,
        })
    }

    /// Helper: convert an `ApiTrack` into a core `Song`.
    fn track_to_song(t: ApiTrack) -> Song {
        let (album_id, album_name, cover_url) = match t.al {
            Some(al) => (al.id, al.name, al.pic_url),
            None => (0, String::new(), None),
        };
        Song {
            id: t.id,
            name: t.name,
            artists: t
                .ar
                .into_iter()
                .map(|a| Artist {
                    id: a.id,
                    name: a.name,
                })
                .collect(),
            album: Album {
                id: album_id,
                name: album_name,
                cover_url,
            },
            duration: t.dt,
            quality: QualityLevel::ExHigh,
        }
    }
}

#[async_trait]
impl NeteaseClient for NeteaseApiClient {
    fn login_state(&self) -> &LoginState {
        // NOTE: This returns a static reference placeholder.
        // Real implementation will use Arc<RwLock<LoginState>> properly.
        static LOGGED_OUT: LoginState = LoginState::LoggedOut;
        &LOGGED_OUT
    }

    async fn login_qr_generate(&self) -> Result<String> {
        let path = "/weapi/login/qrcode/unikey";
        let params = serde_json::json!({"type": 1, "noCheckToken": true});
        let (enc_params, enc_sec_key) = crypto::weapi_encrypt(&params)?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &enc_params), ("encSecKey", &enc_sec_key)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let body = resp
            .text()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiQrKeyResponse = serde_json::from_str(&body)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 || result.unikey.is_empty() {
            return Err(netune_core::NetuneError::Network(format!(
                "qr generate failed: code {}",
                result.code
            )));
        }
        Ok(result.unikey)
    }

    async fn login_qr_check(&self, key: &str) -> Result<Option<UserProfile>> {
        let path = "/weapi/login/qrcode/client/login";

        // Inject cookies before checking: os=pc and random NMTID
        let url: reqwest::Url = self
            .base_url
            .parse()
            .map_err(|e| netune_core::NetuneError::Network(format!("url parse: {e}")))?;
        self.cookie_jar.add_cookie_str("os=pc", &url);
        let mut nmtid_buf = [0u8; 16];
        getrandom::getrandom(&mut nmtid_buf)
            .map_err(|e| netune_core::NetuneError::Crypto(e.to_string()))?;
        let nmtid = hex::encode(nmtid_buf);
        self.cookie_jar
            .add_cookie_str(&format!("NMTID={nmtid}"), &url);

        let params = serde_json::json!({
            "type": 1,
            "noCheckToken": true,
            "key": key,
        });
        let (enc_params, enc_sec_key) = crypto::weapi_encrypt(&params)?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &enc_params), ("encSecKey", &enc_sec_key)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let body = resp
            .text()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiQrCheckResponse = serde_json::from_str(&body)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        match result.code {
            803 => {
                let profile = result
                    .profile
                    .ok_or_else(|| netune_core::NetuneError::Auth("No profile".into()))?;
                Ok(Some(UserProfile {
                    uid: profile.user_id,
                    nickname: profile.nickname,
                    avatar_url: profile.avatar_url,
                }))
            }
            800 => Err(netune_core::NetuneError::Auth(
                result.message.unwrap_or_else(|| "QR code expired".into()),
            )),
            // 801 = waiting for scan, 802 = scanned/confirming
            _ => Ok(None),
        }
    }

    async fn logout(&self) -> Result<()> {
        let path = "/weapi/logout";
        let params = serde_json::json!({});
        let _: serde_json::Value = self
            .inner_request(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        Ok(())
    }

    async fn user_playlists(&self, uid: u64) -> Result<Vec<Playlist>> {
        let path = "/weapi/user/playlist";
        let params = serde_json::json!({
            "uid": uid,
            "limit": 30,
            "offset": 0
        });
        let playlists = self
            .request::<ApiUserPlaylistsResponse>(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?
            .into_iter()
            .map(|p| Playlist {
                id: p.id,
                name: p.name,
                cover_url: p.cover_img_url,
                track_count: p.track_count,
                creator: p.creator.map(|c| UserProfile {
                    uid: c.user_id,
                    nickname: c.nickname,
                    avatar_url: c.avatar_url,
                }),
            })
            .collect();
        Ok(playlists)
    }

    async fn playlist_detail(&self, playlist_id: u64) -> Result<Vec<Song>> {
        let path = "/weapi/v6/playlist/detail";
        let params = serde_json::json!({
            "id": playlist_id,
            "n": 100000
        });
        let playlist = self
            .request::<ApiPlaylistResponse>(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let songs = playlist
            .tracks
            .into_iter()
            .map(Self::track_to_song)
            .collect();
        Ok(songs)
    }

    async fn search_songs(&self, keyword: &str, page: u32, size: u32) -> Result<SearchResult> {
        let path = "/weapi/cloudsearch/get/web";
        let offset = page * size;
        let params = serde_json::json!({
            "s": keyword,
            "type": 1,
            "offset": offset,
            "limit": size,
            "total": true
        });
        let result = self
            .pagination::<ApiSearchResult, ApiSearchResponse>(path, &params, offset, size)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let has_more = result.has_more();
        let songs: Vec<Song> = result
            .items
            .songs
            .into_iter()
            .map(Self::track_to_song)
            .collect();
        Ok(SearchResult {
            songs,
            total: result.total,
            has_more,
        })
    }

    async fn song_url(&self, song_id: u64, quality: QualityLevel) -> Result<String> {
        let path = "/weapi/song/enhance/player/url";
        let params = serde_json::json!({
            "ids": [song_id],
            "br": quality.bitrate()
        });
        let data = self
            .request::<ApiSongUrlResponse>(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let song_url = data
            .into_iter()
            .find(|d| d.id == song_id)
            .and_then(|d| d.url)
            .ok_or_else(|| netune_core::NetuneError::Network("no url available".into()))?;
        Ok(song_url)
    }

    async fn lyrics(&self, song_id: u64) -> Result<Lyrics> {
        let path = "/weapi/song/lyric";
        let params = serde_json::json!({
            "id": song_id,
            "lv": -1,
            "tv": -1
        });
        let result: ApiLyricResponse = self
            .inner_request(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let lines = result
            .lrc
            .map(|lrc| parse_lrc(&lrc.lyric))
            .unwrap_or_default();
        let translated = result.tlyric.map(|t| parse_lrc(&t.lyric));
        Ok(Lyrics { lines, translated })
    }

    async fn daily_recommend(&self) -> Result<DailyRecommend> {
        let path = "/weapi/v2/discovery/recommend/songs";
        let params = serde_json::json!({ "total": true, "limit": 30 });
        let data = self
            .request::<ApiDailyRecommendResponse>(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let songs = data.daily_songs.into_iter().map(Self::track_to_song).collect();
        Ok(DailyRecommend { songs })
    }

    async fn personal_fm(&self) -> Result<Vec<Song>> {
        let path = "/weapi/v6/personal/fm";
        let params = serde_json::json!({ "limit": 30 });
        let tracks = self
            .request::<ApiPersonalFmResponse>(path, &params)
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let songs = tracks.into_iter().map(Self::track_to_song).collect();
        Ok(songs)
    }
}

/// Parse an LRC-format lyric string into `Vec<LyricLine>`.
///
/// Each line is expected to be `[mm:ss.xx]text`. Lines without a valid
/// timestamp or text are silently skipped.
fn parse_lrc(lrc: &str) -> Vec<LyricLine> {
    let mut lines = Vec::new();
    for line in lrc.lines() {
        let line = line.trim();
        // Expect at least "[mm:ss.xx]" prefix
        if !line.starts_with('[') {
            continue;
        }
        let close = match line.find(']') {
            Some(c) => c,
            None => continue,
        };
        let ts_str = &line[1..close];
        let text = line[close + 1..].trim().to_string();

        // Parse mm:ss.xx
        let parts: Vec<&str> = ts_str.splitn(3, ':').collect();
        if parts.len() != 2 {
            continue;
        }
        let minutes: u64 = match parts[0].parse() {
            Ok(m) => m,
            Err(_) => continue,
        };
        let sec_parts: Vec<&str> = parts[1].splitn(2, '.').collect();
        let seconds: u64 = match sec_parts[0].parse() {
            Ok(s) => s,
            Err(_) => continue,
        };
        let millis: u64 = if sec_parts.len() > 1 {
            // "xx" can be 2 or 3 digits; normalise to ms
            let frac = sec_parts[1];
            match frac.len() {
                1 => frac.parse::<u64>().unwrap_or(0) * 100,
                2 => frac.parse::<u64>().unwrap_or(0) * 10,
                _ => frac[..3].parse::<u64>().unwrap_or(0),
            }
        } else {
            0
        };

        let timestamp = minutes * 60_000 + seconds * 1_000 + millis;
        // Skip metadata tags like [ti:], [ar:], or empty text lines
        if text.is_empty() {
            continue;
        }
        lines.push(LyricLine { timestamp, text });
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_client_creation() {
        let client = NeteaseApiClient::new();
        // Verify the client is created with default state
        assert!(matches!(client.login_state(), LoginState::LoggedOut));
    }

    #[test]
    fn test_track_to_song() {
        let track = ApiTrack {
            id: 42,
            name: "My Song".to_string(),
            ar: vec![
                ApiArtist { id: 1, name: "Singer A".to_string() },
                ApiArtist { id: 2, name: "Singer B".to_string() },
            ],
            al: Some(ApiAlbum {
                id: 99,
                name: "Great Album".to_string(),
                pic_url: Some("https://example.com/pic.jpg".to_string()),
            }),
            dt: 300_000,
        };
        let song = NeteaseApiClient::track_to_song(track);
        assert_eq!(song.id, 42);
        assert_eq!(song.name, "My Song");
        assert_eq!(song.artists.len(), 2);
        assert_eq!(song.artists[0].name, "Singer A");
        assert_eq!(song.album.id, 99);
        assert_eq!(song.album.name, "Great Album");
        assert_eq!(song.album.cover_url.as_deref(), Some("https://example.com/pic.jpg"));
        assert_eq!(song.duration, 300_000);
    }

    #[test]
    fn test_track_to_song_no_album() {
        let track = ApiTrack {
            id: 1,
            name: "No Album Track".to_string(),
            ar: vec![],
            al: None,
            dt: 0,
        };
        let song = NeteaseApiClient::track_to_song(track);
        assert_eq!(song.album.id, 0);
        assert_eq!(song.album.name, "");
        assert!(song.album.cover_url.is_none());
        assert!(song.artists.is_empty());
    }
}

#[cfg(test)]
mod qr_debug_test {
    use super::*;

    #[tokio::test]
    async fn test_qr_generate_debug() {
        let client = NeteaseApiClient::new();
        let params = serde_json::json!({ "type": 1 });
        let path = "/weapi/login/qrcode/unikey";
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path).unwrap();
        
        let resp = client.http
            .post(format!("{}{}", client.base_url, path))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .unwrap();
        
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body: {}", &body[..500.min(body.len())]);
        
        match serde_json::from_str::<serde_json::Value>(&body) {
            Ok(val) => eprintln!("Parsed: {:#?}", val),
            Err(e) => eprintln!("Parse error: {}", e),
        }
    }
}

#[cfg(test)]
mod qr_encrypt_test {
    use super::*;

    #[tokio::test]
    async fn test_different_encryptions() {
        let client = reqwest::Client::new();
        let base_url = "https://music.163.com";
        let params = serde_json::json!({ "type": 1 });
        let path = "/weapi/login/qrcode/unikey";
        
        // Test 1: eapi
        eprintln!("=== Test 1: eapi ===");
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path).unwrap();
        let resp = client.post(format!("{}{}", base_url, path))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .unwrap();
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body len: {}, content: {}", body.len(), &body[..200.min(body.len())]);
        
        // Test 2: linuxapi
        eprintln!("\n=== Test 2: linuxapi ===");
        let encrypted = crypto::encrypt_linuxapi(&params.to_string()).unwrap();
        let resp = client.post(format!("{}{}", base_url, path))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .unwrap();
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body len: {}, content: {}", body.len(), &body[..200.min(body.len())]);
        
        // Test 3: weapi (if available)
        eprintln!("\n=== Test 3: raw (no encryption) ===");
        let resp = client.post(format!("{}{}", base_url, path))
            .form(&[("type", "1")])
            .send()
            .await
            .unwrap();
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body len: {}, content: {}", body.len(), &body[..200.min(body.len())]);
    }
}

#[cfg(test)]
mod qr_headers_test {
    use super::*;

    #[tokio::test]
    async fn test_with_headers() {
        let client = reqwest::Client::new();
        let base_url = "https://music.163.com";
        let params = serde_json::json!({ "type": 1 });
        let path = "/weapi/login/qrcode/unikey";
        
        // Test with proper headers
        eprintln!("=== Test with headers ===");
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path).unwrap();
        let resp = client.post(format!("{}{}", base_url, path))
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .header("Referer", "https://music.163.com")
            .header("Content-Type", "application/x-www-form-urlencoded")
            .form(&[("params", &encrypted)])
            .send()
            .await
            .unwrap();
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body len: {}, content: {}", body.len(), &body[..500.min(body.len())]);
        
        // Try with different endpoint
        eprintln!("\n=== Test /api/login/qr/key ===");
        let resp = client.post(format!("{}/api/login/qr/key", base_url))
            .header("User-Agent", "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36")
            .header("Referer", "https://music.163.com")
            .form(&[("timestamp", chrono::Utc::now().timestamp_millis().to_string())])
            .send()
            .await
            .unwrap();
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body len: {}, content: {}", body.len(), &body[..500.min(body.len())]);
    }
}
