//! Configuration management

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("RON parse error: {0}")]
    Ron(#[from] ron::error::SpannedError),
    #[error("RON error: {0}")]
    RonError(#[from] ron::Error),
    #[error("Configuration file not found: {0}")]
    NotFound(String),
}

/// UHPM configuration
#[derive(Serialize, Deserialize, Debug, Default)]
pub struct Config {
    pub update_source: String,
}

impl Config {
    pub fn new() -> Self {
        Self {
            update_source: String::new(),
        }
    }

    /// Loads configuration from default location
    pub fn load() -> Result<Self, ConfigError> {
        let config_path = Self::get_config_path()?;
        Self::load_from_path(&config_path)
    }

    /// Loads configuration from specific path
    pub fn load_from_path<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        if !path_ref.exists() {
            return Err(ConfigError::NotFound(
                path_ref.to_string_lossy().to_string(),
            ));
        }

        let content = fs::read_to_string(path_ref)?;
        let config: Config = ron::from_str(&content)?;
        Ok(config)
    }

    /// Saves configuration to default location
    pub fn save(&self) -> Result<(), ConfigError> {
        let config_path = Self::get_config_path()?;
        self.save_to_path(&config_path)
    }

    /// Saves configuration to specific path
    pub fn save_to_path<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        let path_ref = path.as_ref();

        if let Some(parent) = path_ref.parent() {
            fs::create_dir_all(parent)?;
        }

        let pretty = ron::ser::PrettyConfig::new();
        let ron_str = ron::ser::to_string_pretty(self, pretty)?;
        fs::write(path_ref, ron_str)?;

        Ok(())
    }

    /// Returns default configuration path
    pub fn get_config_path() -> Result<PathBuf, ConfigError> {
        let home_dir = dirs::home_dir()
            .ok_or_else(|| ConfigError::NotFound("Home directory not found".to_string()))?;

        let mut config_path = home_dir;
        config_path.push(".uhpm");
        config_path.push("config.ron");

        Ok(config_path)
    }

    /// Creates default configuration file if missing
    pub fn ensure_default() -> Result<(), ConfigError> {
        let config_path = Self::get_config_path()?;

        if !config_path.exists() {
            let default_config = Config::new();
            default_config.save()?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_config_creation() {
        let config = Config::new();
        assert!(config.update_source.is_empty());
    }

    #[test]
    fn test_config_serialization() {
        let mut config = Config::new();
        config.update_source = "https://example.com/updates".to_string();

        let tmp_dir = tempdir().unwrap();
        let config_path = tmp_dir.path().join("config.ron");

        config.save_to_path(&config_path).unwrap();

        let loaded_config = Config::load_from_path(&config_path).unwrap();
        assert_eq!(loaded_config.update_source, "https://example.com/updates");
    }

    #[test]
    fn test_config_not_found() {
        let tmp_dir = tempdir().unwrap();
        let non_existent_path = tmp_dir.path().join("nonexistent.ron");

        let result = Config::load_from_path(&non_existent_path);
        assert!(matches!(result, Err(ConfigError::NotFound(_))));
    }
}
