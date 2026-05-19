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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert!(NetuneError::Api("bad request".into()).to_string().contains("API error: bad request"));
        assert!(NetuneError::Auth("wrong pwd".into()).to_string().contains("Authentication failed"));
        assert!(NetuneError::Network("timeout".into()).to_string().contains("Network error"));
        assert!(NetuneError::Io(std::io::Error::new(std::io::ErrorKind::Other, "disk full")).to_string().contains("IO error"));
        assert!(NetuneError::Json(serde_json::from_str::<serde_json::Value>("invalid").unwrap_err()).to_string().contains("JSON error"));
        assert!(NetuneError::Config("missing".into()).to_string().contains("Config error"));
        assert!(NetuneError::Player("no device".into()).to_string().contains("Player error"));
        assert!(NetuneError::Crypto("decrypt failed".into()).to_string().contains("Crypto error"));
        assert!(NetuneError::NotLoggedIn.to_string().contains("Not logged in"));
        assert!(NetuneError::Other("misc".into()).to_string().contains("misc"));
    }

    #[test]
    fn test_error_from_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let netune_err: NetuneError = io_err.into();
        assert!(matches!(netune_err, NetuneError::Io(_)));
    }

    #[test]
    fn test_error_from_json() {
        let json_err = serde_json::from_str::<serde_json::Value>("invalid").unwrap_err();
        let netune_err: NetuneError = json_err.into();
        assert!(matches!(netune_err, NetuneError::Json(_)));
    }

    #[test]
    fn test_result_type() {
        let ok: Result<i32> = Ok(42);
        assert_eq!(ok.unwrap(), 42);

        let err: Result<i32> = Err(NetuneError::Other("fail".into()));
        assert!(err.is_err());
    }
}
