//! Netease Cloud Music API client implementation.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use tokio::sync::RwLock;

use netune_core::Result;
use netune_core::models::*;
use netune_core::traits::NeteaseClient;

#[cfg(test)]
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
                .no_proxy()
                .build()
                .expect("Failed to build HTTP client"),
            cookie_jar,
            login_state: Arc::new(RwLock::new(LoginState::LoggedOut)),
            base_url: "https://music.163.com".to_string(),
        }
    }

    /// Get the actual current login state (async, requires locking).
    pub async fn current_login_state(&self) -> LoginState {
        self.login_state.read().await.clone()
    }

    /// Fetch the current logged-in user profile via /api/user/account.
    /// This uses the session cookies (MUSIC_U) set during QR login.
    async fn fetch_current_user_profile(&self) -> std::result::Result<UserProfile, ApiError> {
        let path = "/api/user/account";
        let params = serde_json::json!({});
        let resp: ApiUserAccountResponse = self.inner_request(path, &params).await?;
        if resp.code != 200 {
            return Err(ApiError::Code(resp.code));
        }
        match resp.profile {
            Some(p) => Ok(UserProfile {
                uid: p.user_id,
                nickname: p.nickname,
                avatar_url: p.avatar_url,
            }),
            None => Err(ApiError::Message(
                "no profile in /api/user/account response".into(),
            )),
        }
    }

    /// Send a GET request and return the raw deserialized response.
    async fn inner_request<T: serde::de::DeserializeOwned>(
        &self,
        path: &str,
        params: &serde_json::Value,
    ) -> std::result::Result<T, ApiError> {
        // Convert /weapi/ path to /api/ path
        let api_path = path.replace("/weapi/", "/api/");

        // Build query string from JSON params
        let mut url = format!("{}{api_path}", self.base_url);
        if let Some(obj) = params.as_object() {
            if !obj.is_empty() {
                let qs: String = obj
                    .iter()
                    .map(|(k, v)| {
                        let val = match v {
                            serde_json::Value::String(s) => s.clone(),
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            _ => v.to_string(),
                        };
                        format!(
                            "{}={}",
                            urlencoding::encode(k),
                            urlencoding::encode(&val)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("&");
                url = format!("{url}?{qs}");
            }
        }

        let resp = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| ApiError::Message(e.to_string()))?;
        let status = resp.status();
        let body = resp
            .text()
            .await
            .map_err(|e| ApiError::Message(e.to_string()))?;
        tracing::debug!(status = %status, path = %api_path, body_len = body.len(), "API response");
        serde_json::from_str(&body)
            .map_err(|e| ApiError::Message(format!("{e}: {}", &body[..body.len().min(200)])))
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
        // NOTE: We return a static reference. Callers should use `self.login_state`
        // Arc<RwLock<LoginState>> directly for real-time access. The trait signature
        // requires `&LoginState` so we cannot return a guard here.
        static LOGGED_OUT: LoginState = LoginState::LoggedOut;
        &LOGGED_OUT
    }

    async fn login_qr_generate(&self) -> Result<String> {
        let url = format!("{}/api/login/qrcode/unikey", self.base_url);
        tracing::debug!(url = %url, "QR generate request");
        let resp = self
            .http
            .post(&url)
            .form(&[("type", "1")])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let body = resp
            .text()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        tracing::debug!(body = %body, "QR generate response");
        let result: ApiQrKeyResponse = serde_json::from_str(&body)
            .map_err(|e| netune_core::NetuneError::Network(format!("parse qrkey: {e}, body={body}")))?;
        if result.code != 200 || result.unikey.is_empty() {
            return Err(netune_core::NetuneError::Network(format!(
                "qr generate failed: code {} body={}",
                result.code, body
            )));
        }
        tracing::info!(unikey = %result.unikey, "QR key generated");
        Ok(result.unikey)
    }

    async fn login_qr_check(&self, key: &str) -> Result<Option<UserProfile>> {
        let url_str = format!("{}/api/login/qrcode/client/login", self.base_url);
        tracing::debug!(key = %key, url = %url_str, "QR check request");

        // Inject cookies before checking: os=pc and random NMTID
        let base_url: reqwest::Url = self
            .base_url
            .parse()
            .map_err(|e| netune_core::NetuneError::Network(format!("url parse: {e}")))?;
        self.cookie_jar.add_cookie_str("os=pc", &base_url);
        let mut nmtid_buf = [0u8; 16];
        getrandom::getrandom(&mut nmtid_buf)
            .map_err(|e| netune_core::NetuneError::Crypto(e.to_string()))?;
        let nmtid = hex::encode(nmtid_buf);
        self.cookie_jar
            .add_cookie_str(&format!("NMTID={nmtid}"), &base_url);

        let resp = self
            .http
            .post(&url_str)
            .form(&[("key", key), ("type", "1")])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let body = resp
            .text()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        tracing::debug!(body = %body, "QR check response");
        let result: ApiQrCheckResponse = serde_json::from_str(&body)
            .map_err(|e| netune_core::NetuneError::Network(format!("parse qrcheck: {e}, body={body}")))?;
        match result.code {
            803 => {
                // The QR login endpoint often doesn't include profile data.
                // If missing, fetch from /api/user/account using the fresh cookies.
                let profile = match result.profile {
                    Some(p) => UserProfile {
                        uid: p.user_id,
                        nickname: p.nickname,
                        avatar_url: p.avatar_url,
                    },
                    None => {
                        tracing::debug!("QR login response has no profile, fetching from /api/user/account");
                        self.fetch_current_user_profile()
                            .await
                            .map_err(|e| netune_core::NetuneError::Network(format!(
                                "login succeeded but failed to fetch user profile: {e}"
                            )))?
                    }
                };
                tracing::info!(uid = profile.uid, nickname = %profile.nickname, "Login profile resolved");
                // Update stored login state
                *self.login_state.write().await = LoginState::LoggedIn(profile.clone());
                Ok(Some(profile))
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
        *self.login_state.write().await = LoginState::LoggedOut;
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
            "lv": 1,
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
        let path = "/weapi/v3/discovery/recommend/songs";
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



#[cfg(test)]
mod api_qr_test {
    use super::*;

    #[tokio::test]
    async fn test_api_qr_full_flow() {
        let client = NeteaseApiClient::new();
        
        // Step 1: Generate
        let key = client.login_qr_generate().await.unwrap();
        eprintln!("1. Key: {key}");
        
        // Step 2: Check (should be 801 = waiting)
        let result = client.login_qr_check(&key).await;
        eprintln!("2. Check result: {result:?}");
        assert!(result.is_ok(), "Check should return Ok, got: {result:?}");
        assert!(result.unwrap().is_none(), "Should be waiting (None)");
        
        eprintln!("Full flow OK!");
    }
}

#[cfg(test)]
mod eapi_test {
    use super::*;

    #[tokio::test]
    async fn test_eapi_user_playlists() {
        let client = NeteaseApiClient::new();
        let params = serde_json::json!({"uid": 0, "limit": 30, "offset": 0});
        let encrypted = crypto::encrypt_eapi(&params.to_string(), "/weapi/user/playlist").unwrap();
        let resp = client.http
            .post("https://music.163.com/weapi/user/playlist")
            .form(&[("params", &encrypted)])
            .send().await.unwrap();
        eprintln!("Status: {}", resp.status());
        let body = resp.text().await.unwrap();
        eprintln!("Body len: {} content: {}", body.len(), &body[..200.min(body.len())]);
    }
}
