//! Configuration management.

use camino::Utf8PathBuf;
use serde::{Deserialize, Serialize};

use crate::models::QualityLevel;
use crate::{NetuneError, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    /// Preferred audio quality.
    pub quality: QualityLevel,
    /// Volume (0.0 - 1.0).
    pub volume: f32,
    /// Show translated lyrics.
    pub show_translation: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            quality: QualityLevel::ExHigh,
            volume: 0.7,
            show_translation: true,
        }
    }
}

impl Config {
    /// Config file path: ~/.config/netune/config.toml
    pub fn config_path() -> Result<Utf8PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| NetuneError::Config("Cannot find config directory".into()))?;
        Ok(Utf8PathBuf::try_from(dir)
            .map_err(|e| NetuneError::Config(e.to_string()))?
            .join("netune")
            .join("config.toml"))
    }

    /// Load config from disk, or return default.
    pub fn load() -> Result<Self> {
        let path = Self::config_path()?;
        if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            let config: Self = toml::from_str(&content)
                .map_err(|e| NetuneError::Config(e.to_string()))?;
            Ok(config)
        } else {
            Ok(Self::default())
        }
    }

    /// Save config to disk.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)
            .map_err(|e| NetuneError::Config(e.to_string()))?;
        std::fs::write(&path, content)?;
        Ok(())
    }
}
