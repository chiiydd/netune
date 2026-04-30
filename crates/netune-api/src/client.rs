//! Netease Cloud Music API client implementation.

use std::sync::Arc;

use async_trait::async_trait;
use reqwest::Client;
use tokio::sync::RwLock;

use netune_core::models::*;
use netune_core::traits::NeteaseClient;
use netune_core::Result;

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

    async fn login_email(&self, _email: &str, _password: &str) -> Result<UserProfile> {
        todo!("A组(Codex): 实现邮箱登录")
    }

    async fn login_qr_generate(&self) -> Result<String> {
        todo!("A组(Codex): 实现二维码登录 - 生成")
    }

    async fn login_qr_check(&self, _key: &str) -> Result<Option<UserProfile>> {
        todo!("A组(Codex): 实现二维码登录 - 检查")
    }

    async fn logout(&self) -> Result<()> {
        todo!("A组(Codex): 实现登出")
    }

    async fn user_playlists(&self, _uid: u64) -> Result<Vec<Playlist>> {
        todo!("A组(Codex): 实现获取用户歌单")
    }

    async fn playlist_detail(&self, _playlist_id: u64) -> Result<Vec<Song>> {
        todo!("A组(Codex): 实现获取歌单详情")
    }

    async fn search_songs(&self, _keyword: &str, _page: u32, _size: u32) -> Result<SearchResult> {
        todo!("A组(Codex): 实现歌曲搜索")
    }

    async fn song_url(&self, _song_id: u64, _quality: QualityLevel) -> Result<String> {
        todo!("A组(Codex): 实现获取歌曲URL")
    }

    async fn lyrics(&self, _song_id: u64) -> Result<Lyrics> {
        todo!("A组(Codex): 实现获取歌词")
    }

    async fn daily_recommend(&self) -> Result<DailyRecommend> {
        todo!("A组(Codex): 实现每日推荐")
    }

    async fn personal_fm(&self) -> Result<Vec<Song>> {
        todo!("A组(Codex): 实现私人FM")
    }
}
