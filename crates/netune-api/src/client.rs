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
    login_state: Arc<RwLock<LoginState>>,
    base_url: String,
}

impl NeteaseApiClient {
    pub fn new() -> Self {
        Self {
            http: Client::builder()
                .cookie_store(true)
                .build()
                .expect("Failed to build HTTP client"),
            login_state: Arc::new(RwLock::new(LoginState::LoggedOut)),
            base_url: "https://music.163.com".to_string(),
        }
    }

    /// Helper: convert an `ApiTrack` into a core `Song`.
    fn track_to_song(t: ApiTrack) -> Song {
        let (album_id, album_name, cover_url) = match t.al {
            Some(al) => (al.id, al.name, al.picUrl),
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

    async fn login_phone(&self, phone: &str, password: &str) -> Result<UserProfile> {
        let body = serde_json::json!({
            "phone": phone,
            "password": password,
            "countrycode": "86"
        });
        let params = crypto::encrypt_linuxapi(&body.to_string())
            .map_err(|e| netune_core::NetuneError::Auth(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}/weapi/login/cellphone", self.base_url))
            .form(&[("params", &params)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiLoginResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Auth(
                result.msg.unwrap_or_default(),
            ));
        }
        let profile = result
            .profile
            .ok_or_else(|| netune_core::NetuneError::Auth("No profile".into()))?;
        Ok(UserProfile {
            uid: profile.userId,
            nickname: profile.nickname,
            avatar_url: profile.avatarUrl,
        })
    }

    async fn login_email(&self, email: &str, password: &str) -> Result<UserProfile> {
        let body = serde_json::json!({
            "username": email,
            "password": password,
            "rememberLogin": "true"
        });
        let params = crypto::encrypt_linuxapi(&body.to_string())
            .map_err(|e| netune_core::NetuneError::Auth(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}/weapi/login", self.base_url))
            .form(&[("params", &params)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiLoginResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Auth(
                result.msg.unwrap_or_default(),
            ));
        }
        let profile = result
            .profile
            .ok_or_else(|| netune_core::NetuneError::Auth("No profile".into()))?;
        Ok(UserProfile {
            uid: profile.userId,
            nickname: profile.nickname,
            avatar_url: profile.avatarUrl,
        })
    }

    async fn login_qr_generate(&self) -> Result<String> {
        let path = "/weapi/login/qrcode/unikey";
        let params = serde_json::json!({ "type": 1 });
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiQrKeyResponse = resp
            .json()
            .await
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
        let params = serde_json::json!({ "key": key, "type": 1 });
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiQrCheckResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        match result.code {
            803 => {
                // Success — profile should be present
                let profile = result
                    .profile
                    .ok_or_else(|| netune_core::NetuneError::Auth("No profile".into()))?;
                Ok(Some(UserProfile {
                    uid: profile.userId,
                    nickname: profile.nickname,
                    avatar_url: profile.avatarUrl,
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
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        self.http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
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
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiUserPlaylistsResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Network(format!(
                "user_playlists failed: code {}",
                result.code
            )));
        }
        let playlists = result
            .playlist
            .into_iter()
            .map(|p| Playlist {
                id: p.id,
                name: p.name,
                cover_url: p.coverImgUrl,
                track_count: p.trackCount,
                creator: p.creator.map(|c| UserProfile {
                    uid: c.userId,
                    nickname: c.nickname,
                    avatar_url: c.avatarUrl,
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
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiPlaylistResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Network(format!(
                "playlist_detail failed: code {}",
                result.code
            )));
        }
        let playlist = result
            .playlist
            .ok_or_else(|| netune_core::NetuneError::Network("no playlist data".into()))?;
        let songs = playlist
            .tracks
            .into_iter()
            .map(Self::track_to_song)
            .collect();
        Ok(songs)
    }

    async fn search_songs(&self, keyword: &str, page: u32, size: u32) -> Result<SearchResult> {
        let path = "/weapi/cloudsearch/get/web";
        let params = serde_json::json!({
            "s": keyword,
            "type": 1,
            "offset": page * size,
            "limit": size,
            "total": true
        });
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiSearchResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Network(format!(
                "search failed: code {}",
                result.code
            )));
        }
        let search_result = result.result.unwrap_or(ApiSearchResult {
            songs: vec![],
            songCount: 0,
        });
        let songs: Vec<Song> = search_result
            .songs
            .into_iter()
            .map(|t| {
                let (album_id, album_name, cover_url) = match t.al {
                    Some(al) => (al.id, al.name, al.picUrl),
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
            })
            .collect();
        let total = search_result.songCount;
        Ok(SearchResult {
            songs,
            total,
            has_more: ((page + 1) * size) < total,
        })
    }

    async fn song_url(&self, song_id: u64, quality: QualityLevel) -> Result<String> {
        let path = "/weapi/song/enhance/player/url";
        let params = serde_json::json!({
            "ids": [song_id],
            "br": quality.bitrate()
        });
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiSongUrlResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Network(format!(
                "song_url failed: code {}",
                result.code
            )));
        }
        let song_url = result
            .data
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
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiLyricResponse = resp
            .json()
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
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiDailyRecommendResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Network(format!(
                "daily_recommend failed: code {}",
                result.code
            )));
        }
        let songs = result
            .data
            .map(|d| d.dailySongs.into_iter().map(Self::track_to_song).collect())
            .unwrap_or_default();
        Ok(DailyRecommend { songs })
    }

    async fn personal_fm(&self) -> Result<Vec<Song>> {
        let path = "/weapi/v6/personal/fm";
        let params = serde_json::json!({ "limit": 30 });
        let encrypted = crypto::encrypt_eapi(&params.to_string(), path)
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let resp = self
            .http
            .post(format!("{}{path}", self.base_url))
            .form(&[("params", &encrypted)])
            .send()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        let result: ApiPersonalFmResponse = resp
            .json()
            .await
            .map_err(|e| netune_core::NetuneError::Network(e.to_string()))?;
        if result.code != 200 {
            return Err(netune_core::NetuneError::Network(format!(
                "personal_fm failed: code {}",
                result.code
            )));
        }
        let songs = result.data.into_iter().map(Self::track_to_song).collect();
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
