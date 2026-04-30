//! Central error types for netune.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum NetuneError {
    #[error("API error: {0}")]
    Api(String),

    #[error("Authentication failed: {0}")]
    Auth(String),

    #[error("Network error: {0}")]
    Network(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Config error: {0}")]
    Config(String),

    #[error("Player error: {0}")]
    Player(String),

    #[error("Crypto error: {0}")]
    Crypto(String),

    #[error("Not logged in")]
    NotLoggedIn,

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, NetuneError>;
