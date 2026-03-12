//! Cross-platform configuration management
//!
//! Replaces Windows registry functionality with JSON-based configuration files
//! stored in appropriate platform-specific directories.

use crate::error::{DownloadError, DownloadResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

const CONFIG_FILE: &str = "download_config.json";
const APP_NAME: &str = "CnC_Generals_Zero_Hour";

/// Configuration storage for download settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub base_url: String,
    pub language: String,
    pub version: u32,
    pub map_pack_version: u32,
    pub use_non_blocking_ftp: bool,
    /// Additional key-value pairs for extensibility
    pub extra: HashMap<String, String>,
}

impl Default for DownloadConfig {
    fn default() -> Self {
        Self {
            base_url: "http://servserv.generals.ea.com/servserv/GeneralsZH/".to_string(),
            language: "english".to_string(),
            version: 1,
            map_pack_version: 1,
            use_non_blocking_ftp: true,
            extra: HashMap::new(),
        }
    }
}

/// Cross-platform configuration manager
pub struct ConfigManager {
    config_path: PathBuf,
}

impl ConfigManager {
    /// Create a new configuration manager
    pub fn new() -> DownloadResult<Self> {
        let config_dir = dirs::config_dir()
            .ok_or_else(|| {
                DownloadError::ConfigError("Cannot determine config directory".to_string())
            })?
            .join(APP_NAME);

        // Create config directory if it doesn't exist
        fs::create_dir_all(&config_dir).map_err(|e| {
            DownloadError::ConfigError(format!("Failed to create config directory: {}", e))
        })?;

        let config_path = config_dir.join(CONFIG_FILE);

        Ok(Self { config_path })
    }

    /// Load configuration from file, creating default if it doesn't exist
    pub fn load(&self) -> DownloadResult<DownloadConfig> {
        if !self.config_path.exists() {
            let config = DownloadConfig::default();
            self.save(&config)?;
            return Ok(config);
        }

        let content = fs::read_to_string(&self.config_path).map_err(|e| {
            DownloadError::ConfigError(format!("Failed to read config file: {}", e))
        })?;

        serde_json::from_str(&content)
            .map_err(|e| DownloadError::ConfigError(format!("Failed to parse config file: {}", e)))
    }

    /// Save configuration to file
    pub fn save(&self, config: &DownloadConfig) -> DownloadResult<()> {
        let content = serde_json::to_string_pretty(config).map_err(|e| {
            DownloadError::ConfigError(format!("Failed to serialize config: {}", e))
        })?;

        fs::write(&self.config_path, content)
            .map_err(|e| DownloadError::ConfigError(format!("Failed to write config file: {}", e)))
    }

    /// Get a string value from configuration
    pub fn get_string(&self, path: &str, key: &str) -> DownloadResult<Option<String>> {
        let config = self.load()?;

        match key {
            "BaseURL" => Ok(Some(config.base_url)),
            "Language" => Ok(Some(config.language)),
            _ => {
                let full_key = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}_{}", path, key)
                };
                Ok(config.extra.get(&full_key).cloned())
            }
        }
    }

    /// Get an unsigned integer value from configuration
    pub fn get_unsigned_int(&self, path: &str, key: &str) -> DownloadResult<Option<u32>> {
        let config = self.load()?;

        match key {
            "Version" => Ok(Some(config.version)),
            "MapPackVersion" => Ok(Some(config.map_pack_version)),
            _ => {
                let full_key = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}_{}", path, key)
                };
                if let Some(value_str) = config.extra.get(&full_key) {
                    value_str.parse().map(Some).map_err(|e| {
                        DownloadError::ConfigError(format!(
                            "Invalid integer value for key {}: {}",
                            full_key, e
                        ))
                    })
                } else {
                    Ok(None)
                }
            }
        }
    }

    /// Set a string value in configuration
    pub fn set_string(&self, path: &str, key: &str, value: String) -> DownloadResult<()> {
        let mut config = self.load()?;

        match key {
            "BaseURL" => config.base_url = value,
            "Language" => config.language = value,
            _ => {
                let full_key = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}_{}", path, key)
                };
                config.extra.insert(full_key, value);
            }
        }

        self.save(&config)
    }

    /// Set an unsigned integer value in configuration
    pub fn set_unsigned_int(&self, path: &str, key: &str, value: u32) -> DownloadResult<()> {
        let mut config = self.load()?;

        match key {
            "Version" => config.version = value,
            "MapPackVersion" => config.map_pack_version = value,
            _ => {
                let full_key = if path.is_empty() {
                    key.to_string()
                } else {
                    format!("{}_{}", path, key)
                };
                config.extra.insert(full_key, value.to_string());
            }
        }

        self.save(&config)
    }

    /// Check if non-blocking FTP should be used
    pub fn use_non_blocking_ftp(&self) -> bool {
        self.load()
            .map(|config| config.use_non_blocking_ftp)
            .unwrap_or(true) // Default to true
    }
}

// Convenience functions that mirror the original C++ API
/// Get a string from the configuration system
pub fn get_string_from_registry(path: &str, key: &str, default_val: &mut String) -> bool {
    match ConfigManager::new() {
        Ok(manager) => match manager.get_string(path, key) {
            Ok(Some(value)) => {
                *default_val = value;
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// Get an unsigned integer from the configuration system
pub fn get_unsigned_int_from_registry(path: &str, key: &str, default_val: &mut u32) -> bool {
    match ConfigManager::new() {
        Ok(manager) => match manager.get_unsigned_int(path, key) {
            Ok(Some(value)) => {
                *default_val = value;
                true
            }
            _ => false,
        },
        _ => false,
    }
}

/// Set a string in the configuration system
pub fn set_string_in_registry(path: &str, key: &str, value: String) -> bool {
    match ConfigManager::new() {
        Ok(manager) => manager.set_string(path, key, value).is_ok(),
        _ => false,
    }
}

/// Set an unsigned integer in the configuration system
pub fn set_unsigned_int_in_registry(path: &str, key: &str, value: u32) -> bool {
    match ConfigManager::new() {
        Ok(manager) => manager.set_unsigned_int(path, key, value).is_ok(),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_config_roundtrip() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.json");

        let manager = ConfigManager { config_path };

        let mut config = DownloadConfig::default();
        config.version = 42;
        config.language = "test".to_string();

        manager.save(&config).unwrap();
        let loaded_config = manager.load().unwrap();

        assert_eq!(loaded_config.version, 42);
        assert_eq!(loaded_config.language, "test");
    }
}
