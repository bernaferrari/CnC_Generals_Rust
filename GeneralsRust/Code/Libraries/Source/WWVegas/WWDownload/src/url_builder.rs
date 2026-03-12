//! URL building functionality for patch and content downloads

use crate::config::ConfigManager;
use crate::error::{DownloadError, DownloadResult};
use url::Url;

/// URL configuration for different types of downloads
#[derive(Debug, Clone)]
pub struct UrlConfig {
    pub game_patch_url: String,
    pub map_patch_url: String,
    pub config_url: String,
    pub motd_url: String,
}

/// Build URLs from configuration registry/config
pub fn format_urls_from_config() -> DownloadResult<UrlConfig> {
    let config_manager = ConfigManager::new()?;
    let config = config_manager.load()?;

    let sku = "GeneralsZH";
    let base_url = format!("http://servserv.generals.ea.com/servserv/{}/", sku);

    // Use configured values or defaults
    let base_url = config.base_url;
    let language = config.language;
    let version = config.version;
    let map_version = config.map_pack_version;

    // Build URLs using the modern Url crate for proper URL construction
    let base = Url::parse(&base_url).map_err(|e| {
        DownloadError::UrlParseError(format!("Invalid base URL '{}': {}", base_url, e))
    })?;

    let game_patch_url = format!("{}{}-{}.txt", base.as_str(), language, version);
    let map_patch_url = format!("{}maps-{}.txt", base.as_str(), map_version);
    let config_url = format!("{}config.txt", base.as_str());
    let motd_url = format!("{}MOTD-{}.txt", base.as_str(), language);

    // Validate all constructed URLs
    Url::parse(&game_patch_url)?;
    Url::parse(&map_patch_url)?;
    Url::parse(&config_url)?;
    Url::parse(&motd_url)?;

    Ok(UrlConfig {
        game_patch_url,
        map_patch_url,
        config_url,
        motd_url,
    })
}

/// Enhanced URL builder with better error handling and validation
pub struct UrlBuilder {
    base_url: Url,
}

impl UrlBuilder {
    /// Create a new URL builder with a base URL
    pub fn new(base_url: &str) -> DownloadResult<Self> {
        let base_url = Url::parse(base_url)?;
        Ok(Self { base_url })
    }

    /// Build a game patch URL
    pub fn game_patch_url(&self, language: &str, version: u32) -> DownloadResult<String> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("{}-{}.txt", language, version));
        Ok(url.to_string())
    }

    /// Build a map patch URL
    pub fn map_patch_url(&self, version: u32) -> DownloadResult<String> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("maps-{}.txt", version));
        Ok(url.to_string())
    }

    /// Build a config URL
    pub fn config_url(&self) -> DownloadResult<String> {
        let mut url = self.base_url.clone();
        url.set_path("config.txt");
        Ok(url.to_string())
    }

    /// Build a MOTD URL
    pub fn motd_url(&self, language: &str) -> DownloadResult<String> {
        let mut url = self.base_url.clone();
        url.set_path(&format!("MOTD-{}.txt", language));
        Ok(url.to_string())
    }

    /// Build a custom file URL
    pub fn custom_file_url(&self, path: &str) -> DownloadResult<String> {
        let mut url = self.base_url.clone();
        url.set_path(path);
        Ok(url.to_string())
    }

    /// Get the base URL
    pub fn base_url(&self) -> &str {
        self.base_url.as_str()
    }
}

/// Extract file information from URL
pub fn parse_url_info(url: &str) -> DownloadResult<(String, String, Option<u16>)> {
    let parsed_url = Url::parse(url)?;

    let host = parsed_url
        .host_str()
        .ok_or_else(|| DownloadError::UrlParseError("URL has no host".to_string()))?
        .to_string();

    let path = parsed_url.path().to_string();
    let port = parsed_url.port();

    Ok((host, path, port))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_builder() {
        let builder = UrlBuilder::new("http://example.com/base/").unwrap();

        let game_url = builder.game_patch_url("english", 1).unwrap();
        assert_eq!(game_url, "http://example.com/english-1.txt");

        let map_url = builder.map_patch_url(2).unwrap();
        assert_eq!(map_url, "http://example.com/maps-2.txt");

        let config_url = builder.config_url().unwrap();
        assert_eq!(config_url, "http://example.com/config.txt");

        let motd_url = builder.motd_url("french").unwrap();
        assert_eq!(motd_url, "http://example.com/MOTD-french.txt");
    }

    #[test]
    fn test_parse_url_info() {
        let (host, path, port) = parse_url_info("http://example.com:8080/path/file.txt").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(path, "/path/file.txt");
        assert_eq!(port, Some(8080));

        let (host, path, port) = parse_url_info("https://example.com/file.txt").unwrap();
        assert_eq!(host, "example.com");
        assert_eq!(path, "/file.txt");
        assert_eq!(port, None);
    }

    #[test]
    fn test_invalid_url() {
        assert!(UrlBuilder::new("not-a-url").is_err());
        assert!(parse_url_info("invalid-url").is_err());
    }
}
