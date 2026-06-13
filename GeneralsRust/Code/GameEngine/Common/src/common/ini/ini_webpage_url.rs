////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_webpage_url.rs
//! Author: Bryan Cleveland, November 2001 (Converted to Rust)
//! Desc:   Parsing Webpage URL INI entries

use log::debug;
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::audio::launch_url_safe;
use crate::common::ini::ini::{INIError, INIResult, INI};
use crate::common::language::{get_current_language, Language, LanguageId};

const CPP_WEBPAGE_URL_FIELDS: &[&str] = &["URL"];

/// Result type for webpage URL parsing operations
pub type WebpageUrlResult<T> = Result<T, WebpageUrlError>;

/// Errors that can occur during webpage URL parsing
#[derive(Debug, Clone, PartialEq)]
pub enum WebpageUrlError {
    InvalidTag,
    InvalidUrl,
    ParseError(String),
    BrowserError(String),
    NotFound,
    AlreadyExists,
}

impl std::fmt::Display for WebpageUrlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WebpageUrlError::InvalidTag => write!(f, "Invalid URL tag"),
            WebpageUrlError::InvalidUrl => write!(f, "Invalid URL"),
            WebpageUrlError::ParseError(msg) => write!(f, "Parse error: {}", msg),
            WebpageUrlError::BrowserError(msg) => write!(f, "Web browser error: {}", msg),
            WebpageUrlError::NotFound => write!(f, "URL not found"),
            WebpageUrlError::AlreadyExists => write!(f, "URL already exists"),
        }
    }
}

impl std::error::Error for WebpageUrlError {}

/// URL categories for organization
#[derive(Debug, Clone, PartialEq)]
pub enum UrlCategory {
    Official,
    Community,
    Support,
    News,
    Download,
    Forums,
    Documentation,
    Custom(String),
}

impl UrlCategory {
    pub fn from_string(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "official" => Self::Official,
            "community" => Self::Community,
            "support" => Self::Support,
            "news" => Self::News,
            "download" => Self::Download,
            "forums" => Self::Forums,
            "documentation" | "docs" => Self::Documentation,
            _ => Self::Custom(s.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Official => "Official",
            Self::Community => "Community",
            Self::Support => "Support",
            Self::News => "News",
            Self::Download => "Download",
            Self::Forums => "Forums",
            Self::Documentation => "Documentation",
            Self::Custom(name) => name,
        }
    }
}

/// Web browser URL definition
#[derive(Debug, Clone)]
pub struct WebBrowserURL {
    pub tag: AsciiString,
    pub url: AsciiString,
    pub display_name: AsciiString,
    pub description: AsciiString,
    pub category: UrlCategory,
    pub is_local_file: bool,
    pub requires_internet: bool,
    pub language_specific: bool,
    pub target_window: AsciiString, // _blank, _self, etc.
    pub tooltip: AsciiString,
    pub icon: AsciiString,
    pub access_count: u32,
    pub last_accessed: u64, // Timestamp
    pub is_enabled: bool,
    pub properties: HashMap<String, String>,
}

impl WebBrowserURL {
    pub fn new(tag: AsciiString) -> Self {
        Self {
            tag,
            url: AsciiString::from(""),
            display_name: AsciiString::from(""),
            description: AsciiString::from(""),
            category: UrlCategory::Official,
            is_local_file: false,
            requires_internet: true,
            language_specific: false,
            target_window: AsciiString::from("_blank"),
            tooltip: AsciiString::from(""),
            icon: AsciiString::from(""),
            access_count: 0,
            last_accessed: 0,
            is_enabled: true,
            properties: HashMap::new(),
        }
    }

    /// Get the field parse table for this URL
    pub fn get_field_parse(
        &self,
    ) -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        CPP_WEBPAGE_URL_FIELDS
            .iter()
            .map(|field| {
                (
                    *field,
                    parse_cpp_webpage_url_field_for_table
                        as fn(&str) -> Result<Box<dyn std::any::Any>, String>,
                )
            })
            .collect()
    }

    /// Update URL from properties
    pub fn update_from_properties(
        &mut self,
        properties: &HashMap<String, String>,
    ) -> WebpageUrlResult<()> {
        for (key, value) in properties {
            match key.as_str() {
                "URL" => {
                    self.url = AsciiString::from(value);
                    self.is_local_file = value.starts_with("file://");
                    self.requires_internet = !self.is_local_file
                        && (value.starts_with("http://") || value.starts_with("https://"));
                }
                _ => {
                    return Err(WebpageUrlError::ParseError(format!(
                        "Unknown webpage URL field '{}'",
                        key
                    )));
                }
            }
        }

        Ok(())
    }

    pub fn get_tag(&self) -> &AsciiString {
        &self.tag
    }

    pub fn get_url(&self) -> &AsciiString {
        &self.url
    }

    pub fn is_valid(&self) -> bool {
        !self.tag.is_empty() && !self.url.is_empty()
    }

    pub fn is_accessible(&self) -> bool {
        self.is_enabled && !self.url.is_empty()
    }

    pub fn access(&mut self) {
        if self.is_accessible() {
            self.access_count += 1;
            // In a real implementation, you'd set the timestamp
            // self.last_accessed = current_timestamp();
        }
    }

    pub fn get_full_display_name(&self) -> String {
        if !self.display_name.is_empty() {
            self.display_name.as_str().to_string()
        } else {
            self.tag.as_str().to_string()
        }
    }

    pub fn get_protocol(&self) -> Option<String> {
        let url_str = self.url.as_str();
        if let Some(pos) = url_str.find("://") {
            Some(url_str[..pos].to_string())
        } else {
            None
        }
    }

    pub fn is_secure(&self) -> bool {
        self.get_protocol()
            .map_or(false, |protocol| protocol == "https")
    }
}

/// Web browser - manages webpage URLs
#[derive(Debug)]
pub struct WebBrowser {
    urls: HashMap<String, WebBrowserURL>,
    default_language: AsciiString,
    base_data_path: AsciiString,
}

impl WebBrowser {
    pub fn new() -> Self {
        Self {
            urls: HashMap::new(),
            default_language: AsciiString::from("English"),
            base_data_path: AsciiString::from("Data"),
        }
    }

    /// Find a URL by tag
    pub fn find_url(&self, tag: &AsciiString) -> Option<&WebBrowserURL> {
        self.urls.get(tag.as_str())
    }

    /// Find a mutable URL by tag
    pub fn find_url_mut(&mut self, tag: &AsciiString) -> Option<&mut WebBrowserURL> {
        self.urls.get_mut(tag.as_str())
    }

    /// Create a new URL entry
    pub fn make_new_url(&mut self, tag: AsciiString) -> &mut WebBrowserURL {
        let url = WebBrowserURL::new(tag.clone());
        self.urls.insert(tag.as_str().to_string(), url);
        self.urls.get_mut(tag.as_str()).unwrap()
    }

    /// Get or create a URL entry
    pub fn get_or_create_url(&mut self, tag: &AsciiString) -> &mut WebBrowserURL {
        if !self.urls.contains_key(tag.as_str()) {
            self.make_new_url(tag.clone());
        }
        self.urls.get_mut(tag.as_str()).unwrap()
    }

    /// Open a URL (simulate browser opening)
    pub fn open_url(&mut self, tag: &AsciiString) -> WebpageUrlResult<()> {
        let url = self.find_url_mut(tag).ok_or(WebpageUrlError::NotFound)?;

        if !url.is_accessible() {
            return Err(WebpageUrlError::InvalidUrl);
        }

        // Access the URL (increment counter)
        url.access();

        debug!("Opening URL: {} -> {}", tag.as_str(), url.url.as_str());
        Ok(())
    }

    /// Get all URL tags
    pub fn get_url_tags(&self) -> Vec<&String> {
        self.urls.keys().collect()
    }

    /// Get URLs by category
    pub fn get_urls_by_category(&self, category: &UrlCategory) -> Vec<&WebBrowserURL> {
        self.urls
            .values()
            .filter(|url| &url.category == category)
            .collect()
    }

    /// Get enabled URLs only
    pub fn get_enabled_urls(&self) -> Vec<&WebBrowserURL> {
        self.urls.values().filter(|url| url.is_enabled).collect()
    }

    /// Remove a URL
    pub fn remove_url(&mut self, tag: &AsciiString) -> bool {
        self.urls.remove(tag.as_str()).is_some()
    }

    /// Clear all URLs
    pub fn clear(&mut self) {
        self.urls.clear();
    }

    /// Get URL count
    pub fn get_url_count(&self) -> usize {
        self.urls.len()
    }

    /// Set default language for language-specific URLs
    pub fn set_default_language(&mut self, language: AsciiString) {
        self.default_language = language;
    }

    /// Get language-specific URL path
    pub fn get_language_path(&self, base_path: &str) -> String {
        format!(
            "{}/{}/{}",
            self.base_data_path.as_str(),
            self.default_language.as_str(),
            base_path
        )
    }
}

impl Default for WebBrowser {
    fn default() -> Self {
        Self::new()
    }
}

/// Global web browser instance
static WEB_BROWSER: OnceCell<RwLock<WebBrowser>> = OnceCell::new();

fn web_browser_cell() -> &'static RwLock<WebBrowser> {
    WEB_BROWSER.get_or_init(|| RwLock::new(WebBrowser::new()))
}

fn web_browser_mut() -> RwLockWriteGuard<'static, WebBrowser> {
    web_browser_cell().write().expect("WebBrowser poisoned")
}

fn web_browser() -> RwLockReadGuard<'static, WebBrowser> {
    web_browser_cell().read().expect("WebBrowser poisoned")
}

/// Initialize the global web browser
pub fn initialize_web_browser() {
    let _ = web_browser_cell();
}

/// Get a reference to the global web browser
pub fn get_web_browser() -> Option<RwLockReadGuard<'static, WebBrowser>> {
    Some(web_browser())
}

pub fn get_web_browser_mut() -> Option<RwLockWriteGuard<'static, WebBrowser>> {
    Some(web_browser_mut())
}

/// URL encoding function - encodes URLs for safe transmission
pub fn encode_url(source: AsciiString) -> AsciiString {
    if source.is_empty() {
        return AsciiString::from("");
    }

    let mut target = String::new();
    let allowed_chars = "$-_.+!*'(),\\";

    for ch in source.as_str().chars() {
        if ch.is_alphanumeric() || allowed_chars.contains(ch) {
            target.push(ch);
        } else {
            target.push_str(&format!("%{:02x}", ch as u8));
        }
    }

    AsciiString::from(&target)
}

/// Get current working directory.
pub fn get_current_directory() -> String {
    std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("\\"))
        .to_string_lossy()
        .to_string()
}

/// Get registry language.
pub fn get_registry_language() -> AsciiString {
    for key in ["CNC_REGISTRY_LANGUAGE", "GENERALS_REGISTRY_LANGUAGE"] {
        if let Ok(value) = std::env::var(key) {
            let trimmed = value.trim();
            if !trimmed.is_empty() {
                return AsciiString::from(trimmed);
            }
        }
    }

    let mut language = get_current_language();
    if matches!(language, LanguageId::Unknown) {
        language = Language::detect_system_language();
    }

    match language {
        LanguageId::German => AsciiString::from("German"),
        LanguageId::French => AsciiString::from("French"),
        LanguageId::Spanish => AsciiString::from("Spanish"),
        LanguageId::Italian => AsciiString::from("Italian"),
        LanguageId::Japanese => AsciiString::from("Japanese"),
        LanguageId::Korean => AsciiString::from("Korean"),
        LanguageId::Us | LanguageId::Uk | LanguageId::Jabber | LanguageId::Unknown => {
            AsciiString::from("English")
        }
    }
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

fn parse_cpp_webpage_url_field_for_table(value: &str) -> Result<Box<dyn std::any::Any>, String> {
    Ok(Box::new(AsciiString::from(value)) as Box<dyn std::any::Any>)
}

/// INI parsing functions for webpage URLs
pub struct IniWebpageUrl;

impl IniWebpageUrl {
    /// Parse WebpageURL entry from an INI line and consume its block.
    pub fn parse_webpage_url_definition_from_ini(ini: &mut INI) -> INIResult<()> {
        let tokens = ini.get_line_tokens();
        let tag = tokens
            .iter()
            .skip(1)
            .find(|token| **token != "=")
            .ok_or(INIError::InvalidData)?
            .to_string();

        let mut properties: HashMap<String, String> = HashMap::new();
        loop {
            ini.read_line()?;
            if ini.is_eof() {
                return Err(INIError::MissingEndToken);
            }

            let line_tokens = ini.get_line_tokens();
            let Some(first) = line_tokens.first().copied() else {
                continue;
            };
            if first.eq_ignore_ascii_case("End") {
                break;
            }

            let mut key: Option<String> = None;
            let mut value_tokens: Vec<&str> = Vec::new();
            if let Some(eq_idx) = line_tokens.iter().position(|token| *token == "=") {
                if eq_idx > 0 {
                    key = Some(line_tokens[0..eq_idx].join(" "));
                    value_tokens.extend_from_slice(&line_tokens[eq_idx + 1..]);
                }
            } else if line_tokens.len() >= 2 {
                key = Some(line_tokens[0].to_string());
                value_tokens.extend_from_slice(&line_tokens[1..]);
            }

            if let Some(key) = key {
                let value = value_tokens.join(" ").trim().to_string();
                if !key.trim().is_empty() {
                    properties.insert(key.trim().to_string(), value);
                }
            }
        }

        let tag_ascii = AsciiString::from(tag.as_str());
        let url = Self::parse_webpage_url_block(tag_ascii.clone(), properties)
            .map_err(|_| INIError::InvalidData)?;
        Self::register_webpage_url(url).map_err(|_| INIError::InvalidData)?;
        Self::parse_webpage_url_definition(tag_ascii).map_err(|_| INIError::InvalidData)?;
        Ok(())
    }

    /// Parse webpage URL definition - equivalent to INI::parseWebpageURLDefinition
    pub fn parse_webpage_url_definition(tag: AsciiString) -> WebpageUrlResult<()> {
        // Validate tag
        if tag.is_empty() {
            return Err(WebpageUrlError::InvalidTag);
        }

        // Initialize web browser if needed
        initialize_web_browser();

        // Get web browser
        let mut browser = get_web_browser_mut()
            .ok_or_else(|| WebpageUrlError::BrowserError("Browser not initialized".to_string()))?;

        // Find existing URL or create new one
        let url = if browser.find_url(&tag).is_some() {
            browser.find_url_mut(&tag).unwrap()
        } else {
            browser.make_new_url(tag.clone())
        };

        // In the original C++, this would call:
        // ini->initFromINI(url, url->getFieldParse());

        // Handle file:// URL conversion
        if url.url.as_str().starts_with("file://") {
            let cwd = get_current_directory();
            let language = get_registry_language();
            let file_path = &url.url.as_str()[7..]; // Remove "file://" prefix

            let new_url = format!(
                "file://{}\\Data\\{}\\{}",
                encode_url(AsciiString::from(&cwd)).as_str(),
                language.as_str(),
                file_path
            );

            url.url = AsciiString::from(&new_url);
            debug!("Converted URL to: {}", url.url.as_str());
        }

        debug!("Parsing webpage URL definition for: {}", tag.as_str());
        Ok(())
    }

    /// Parse a complete webpage URL block from INI data
    pub fn parse_webpage_url_block(
        tag: AsciiString,
        properties: HashMap<String, String>,
    ) -> WebpageUrlResult<WebBrowserURL> {
        // Validate tag
        if tag.is_empty() {
            return Err(WebpageUrlError::InvalidTag);
        }

        // Create URL
        let mut url = WebBrowserURL::new(tag);

        // Update URL from properties
        url.update_from_properties(&properties)?;

        // Validate URL
        if !url.is_valid() {
            return Err(WebpageUrlError::ParseError(
                "Invalid webpage URL configuration".to_string(),
            ));
        }

        Ok(url)
    }

    /// Register a webpage URL
    pub fn register_webpage_url(url: WebBrowserURL) -> WebpageUrlResult<()> {
        initialize_web_browser();

        let mut browser = get_web_browser_mut()
            .ok_or_else(|| WebpageUrlError::BrowserError("Browser not initialized".to_string()))?;

        let tag = url.tag.as_str().to_string();
        browser.urls.insert(tag, url);
        Ok(())
    }

    /// Find a webpage URL by tag
    pub fn find_webpage_url_by_tag(tag: &AsciiString) -> Option<WebBrowserURL> {
        if let Some(browser) = get_web_browser() {
            browser.find_url(tag).cloned()
        } else {
            None
        }
    }

    /// Open a webpage URL
    pub fn open_webpage_url(tag: &AsciiString) -> WebpageUrlResult<()> {
        initialize_web_browser();

        let mut browser = get_web_browser_mut()
            .ok_or_else(|| WebpageUrlError::BrowserError("Browser not initialized".to_string()))?;

        browser.open_url(tag)
    }

    pub fn load_webpage_urls_from_file(path: &Path) -> WebpageUrlResult<usize> {
        let contents = fs::read_to_string(path).map_err(|err| {
            WebpageUrlError::ParseError(format!("Failed to read {}: {}", path.display(), err))
        })?;

        let mut current_tag: Option<AsciiString> = None;
        let mut properties: HashMap<String, String> = HashMap::new();
        let mut loaded = 0usize;

        for raw_line in contents.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            if line.eq_ignore_ascii_case("End") {
                if let Some(tag) = current_tag.take() {
                    let url =
                        IniWebpageUrl::parse_webpage_url_block(tag.clone(), properties.clone())?;
                    IniWebpageUrl::register_webpage_url(url)?;
                    IniWebpageUrl::parse_webpage_url_definition(tag)?;
                    loaded += 1;
                    properties.clear();
                }
                continue;
            }

            if current_tag.is_none() {
                if let Some(rest) = line
                    .strip_prefix("WebpageURL")
                    .or_else(|| line.strip_prefix("WEBPAGEURL"))
                {
                    let tag = rest.trim();
                    if !tag.is_empty() {
                        current_tag = Some(AsciiString::from(tag));
                        properties.clear();
                    }
                }
                continue;
            }

            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if !key.is_empty() {
                    properties.insert(key.to_string(), value.to_string());
                }
            }
        }

        Ok(loaded)
    }

    pub fn open_webpage_url_external(tag: &AsciiString) -> WebpageUrlResult<()> {
        let url = IniWebpageUrl::find_webpage_url_by_tag(tag).ok_or(WebpageUrlError::NotFound)?;
        let url_str = url.url.as_str();
        if url_str.is_empty() {
            return Err(WebpageUrlError::InvalidUrl);
        }
        launch_url_safe(url_str).map_err(|err| WebpageUrlError::BrowserError(err))?;
        if let Some(mut browser) = get_web_browser_mut() {
            if let Some(entry) = browser.find_url_mut(tag) {
                entry.access();
            }
        }
        Ok(())
    }

    /// Validate URL tag format
    pub fn validate_tag(tag: &AsciiString) -> bool {
        !tag.is_empty() && tag.len() < 128 // Reasonable length limit
    }

    /// Validate URL format
    pub fn validate_url(url: &AsciiString) -> bool {
        let url_str = url.as_str();
        url_str.starts_with("http://")
            || url_str.starts_with("https://")
            || url_str.starts_with("file://")
            || url_str.starts_with("ftp://")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_category_parsing() {
        assert_eq!(UrlCategory::from_string("official"), UrlCategory::Official);
        assert_eq!(UrlCategory::from_string("FORUMS"), UrlCategory::Forums);
        assert_eq!(
            UrlCategory::from_string("CustomCategory"),
            UrlCategory::Custom("CustomCategory".to_string())
        );
    }

    #[test]
    fn test_web_browser_url_creation() {
        let tag = AsciiString::from("TestURL");
        let url = WebBrowserURL::new(tag.clone());

        assert_eq!(url.tag, tag);
        assert!(url.url.is_empty());
        assert!(url.requires_internet);
        assert!(!url.is_local_file);
        assert!(url.is_enabled);
    }

    #[test]
    fn test_web_browser() {
        let mut browser = WebBrowser::new();
        let tag = AsciiString::from("TestURL");

        // Create new URL
        let url = browser.make_new_url(tag.clone());
        url.url = AsciiString::from("https://example.com");
        url.display_name = AsciiString::from("Example Site");
        url.category = UrlCategory::Official;

        // Find URL
        let found = browser
            .find_url(&tag)
            .expect("URL should be present after creation");
        assert_eq!(found.url.as_str(), "https://example.com");
        assert!(matches!(found.category, UrlCategory::Official));

        // Open URL and verify access count incremented
        browser
            .open_url(&tag)
            .expect("Opening the URL should succeed");
        let found_after = browser
            .find_url(&tag)
            .expect("URL should remain accessible after opening");
        assert_eq!(found_after.access_count, 1);

        // Count URLs
        assert_eq!(browser.get_url_count(), 1);
    }

    #[test]
    fn test_url_properties_update() {
        let mut url = WebBrowserURL::new(AsciiString::from("Test"));
        let mut properties = HashMap::new();
        properties.insert("URL".to_string(), "https://secure-site.com".to_string());

        url.update_from_properties(&properties).unwrap();

        assert_eq!(url.url.as_str(), "https://secure-site.com");
        assert!(url.requires_internet);
        assert!(url.is_secure());
        assert!(url.properties.is_empty());
    }

    #[test]
    fn webpage_url_rejects_fields_outside_cpp_parse_table() {
        for field in [
            "DisplayName",
            "Description",
            "Category",
            "IsLocalFile",
            "RequiresInternet",
            "LanguageSpecific",
            "TargetWindow",
            "Tooltip",
            "Icon",
            "IsEnabled",
            "UnknownField",
        ] {
            let mut properties = HashMap::new();
            properties.insert("URL".to_string(), "https://example.com".to_string());
            properties.insert(field.to_string(), "value".to_string());
            assert!(
                IniWebpageUrl::parse_webpage_url_block(AsciiString::from("StrictUrl"), properties)
                    .is_err(),
                "{} should be rejected because C++ WebBrowserURL does not parse it",
                field
            );
        }
    }

    #[test]
    fn test_url_encoding() {
        let source = AsciiString::from("Hello World!");
        let encoded = encode_url(source);
        assert!(encoded.as_str().contains("Hello%20World!"));

        let simple = AsciiString::from("simple-text_123");
        let encoded_simple = encode_url(simple);
        assert_eq!(encoded_simple.as_str(), "simple-text_123");
    }

    #[test]
    fn test_url_protocol_detection() {
        let mut url = WebBrowserURL::new(AsciiString::from("TestHTTPS"));
        url.url = AsciiString::from("https://secure.example.com");

        assert_eq!(url.get_protocol(), Some("https".to_string()));
        assert!(url.is_secure());

        let mut url2 = WebBrowserURL::new(AsciiString::from("TestHTTP"));
        url2.url = AsciiString::from("http://example.com");

        assert_eq!(url2.get_protocol(), Some("http".to_string()));
        assert!(!url2.is_secure());
    }

    #[test]
    fn test_file_url_detection() {
        let mut url = WebBrowserURL::new(AsciiString::from("LocalFile"));
        let mut properties = HashMap::new();
        properties.insert("URL".to_string(), "file://local/path/file.html".to_string());

        url.update_from_properties(&properties).unwrap();

        assert!(url.is_local_file);
        assert!(!url.requires_internet);
    }

    #[test]
    fn test_url_accessibility() {
        let mut url = WebBrowserURL::new(AsciiString::from("TestURL"));
        url.url = AsciiString::from("https://example.com");
        url.is_enabled = true;

        assert!(url.is_accessible());

        url.is_enabled = false;
        assert!(!url.is_accessible());

        url.is_enabled = true;
        url.url = AsciiString::from("");
        assert!(!url.is_accessible());
    }

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("TRUE"), Ok(true));
        assert_eq!(parse_bool("yes"), Ok(true));
        assert_eq!(parse_bool("1"), Ok(true));

        assert_eq!(parse_bool("false"), Ok(false));
        assert_eq!(parse_bool("FALSE"), Ok(false));
        assert_eq!(parse_bool("no"), Ok(false));
        assert_eq!(parse_bool("0"), Ok(false));

        assert!(parse_bool("invalid").is_err());
    }

    #[test]
    fn test_parse_webpage_url_definition_converts_file_url() {
        let tag = AsciiString::from("TestFileUrlConversion");
        let language = get_registry_language();

        initialize_web_browser();
        {
            let mut browser = get_web_browser_mut().expect("browser should initialize");
            let entry = browser.get_or_create_url(&tag);
            entry.url = AsciiString::from("file://maps/test.html");
        }

        IniWebpageUrl::parse_webpage_url_definition(tag.clone())
            .expect("file URL conversion should succeed");

        let browser = get_web_browser().expect("browser should remain available");
        let converted = browser
            .find_url(&tag)
            .expect("converted URL entry should exist")
            .url
            .as_str()
            .to_string();
        assert!(converted.starts_with("file://"));
        assert!(converted.contains(&format!("\\Data\\{}\\maps/test.html", language.as_str())));
    }

    #[test]
    fn test_validate_tag_and_url() {
        assert!(IniWebpageUrl::validate_tag(&AsciiString::from("ValidTag")));
        assert!(!IniWebpageUrl::validate_tag(&AsciiString::from("")));

        assert!(IniWebpageUrl::validate_url(&AsciiString::from(
            "https://example.com"
        )));
        assert!(IniWebpageUrl::validate_url(&AsciiString::from(
            "file://local.html"
        )));
        assert!(!IniWebpageUrl::validate_url(&AsciiString::from(
            "invalid-url"
        )));
    }
}
