//! # Localization and Language Support System
//!
//! Complete internationalization system supporting:
//! - Multi-language text and audio assets
//! - Dynamic language switching
//! - Text formatting and pluralization
//! - Right-to-left (RTL) language support
//! - Cultural adaptation (dates, numbers, currencies)
//! - Hot-swappable language packs
//! - Missing translation fallbacks
//! - Translation validation and quality assurance
//! - Memory-efficient string storage

use lazy_static::lazy_static;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant, SystemTime};
use thiserror::Error;
use tokio::sync::RwLock as AsyncRwLock;

use super::{AssetError, AssetHandle};

/// Localization system errors
#[derive(Error, Debug)]
pub enum LocalizationError {
    #[error("Language pack not found: {language}")]
    LanguagePackNotFound { language: String },
    #[error("Translation key not found: {key} in language {language}")]
    TranslationNotFound { key: String, language: String },
    #[error("Language pack parsing failed: {path} - {error}")]
    ParseFailed { path: String, error: String },
    #[error("Pluralization rule invalid: {language} - {rule}")]
    InvalidPluralizationRule { language: String, rule: String },
    #[error("Text formatting error: {template} - {error}")]
    FormattingError { template: String, error: String },
    #[error("Language switching failed: from {from} to {to} - {error}")]
    LanguageSwitchFailed {
        from: String,
        to: String,
        error: String,
    },
    #[error("Encoding error: {0}")]
    EncodingError(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Supported languages with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub code: String,           // ISO 639-1 code (e.g., "en", "fr", "de")
    pub name: String,           // Native name (e.g., "English", "Français", "Deutsch")
    pub english_name: String,   // English name for UI
    pub region: Option<String>, // Region code (e.g., "US", "GB", "CA")
    pub direction: TextDirection,
    pub plural_rule: PluralizationRule,
    pub date_format: String,
    pub number_format: NumberFormat,
    pub currency: Option<CurrencyInfo>,
    pub fonts: Vec<String>, // Preferred font families
    pub completion: f32,    // Translation completion percentage
    pub is_rtl: bool,
    pub supports_complex_scripts: bool,
}

/// Text direction for rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextDirection {
    LeftToRight,
    RightToLeft,
    TopToBottom,
}

/// Pluralization rules for different languages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PluralizationRule {
    /// English-style: one, other (n != 1)
    Simple,
    /// No plural forms
    None,
    /// Slavic languages: one, few, many, other
    Slavic,
    /// Arabic: zero, one, two, few, many, other
    Arabic,
    /// Custom rule with expression
    Custom(String),
}

/// Number formatting rules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberFormat {
    pub decimal_separator: String,
    pub thousands_separator: String,
    pub negative_format: String, // e.g., "-{0}", "({0})"
}

/// Currency information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CurrencyInfo {
    pub code: String,   // ISO 4217 code
    pub symbol: String, // Currency symbol
    pub position: CurrencyPosition,
}

/// Currency symbol position
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CurrencyPosition {
    Before,
    After,
    BeforeWithSpace,
    AfterWithSpace,
}

/// Translation entry with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Translation {
    pub key: String,
    pub text: String,
    pub context: Option<String>,   // Context for translators
    pub comment: Option<String>,   // Translator notes
    pub plural_forms: Vec<String>, // For pluralization
    pub max_length: Option<u32>,   // UI constraints
    pub tags: Vec<String>,         // Categorization tags
    pub last_modified: Option<SystemTime>,
    pub translator: Option<String>,
    pub reviewed: bool,
}

/// Language pack containing all translations for a language
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePack {
    pub language: LanguageInfo,
    pub translations: HashMap<String, Translation>,
    pub metadata: LanguagePackMetadata,
}

/// Language pack metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguagePackMetadata {
    pub version: String,
    pub created: SystemTime,
    pub last_updated: SystemTime,
    pub author: String,
    pub translator_credits: Vec<String>,
    pub total_strings: u32,
    pub translated_strings: u32,
    pub reviewed_strings: u32,
    pub file_size: u64,
    pub checksum: String,
}

/// Text formatting parameters
#[derive(Debug, Clone)]
pub struct FormatParams {
    pub params: HashMap<String, FormatValue>,
}

/// Values that can be formatted into text
#[derive(Debug, Clone)]
pub enum FormatValue {
    Text(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    Date(SystemTime),
    Currency(f64),
    Percentage(f64),
}

impl FormatValue {
    pub fn format(&self, language: &LanguageInfo) -> String {
        match self {
            FormatValue::Text(s) => s.clone(),
            FormatValue::Integer(i) => self.format_number(*i as f64, language, 0),
            FormatValue::Float(f) => self.format_number(*f, language, 2),
            FormatValue::Boolean(b) => if *b { "true" } else { "false" }.to_string(),
            FormatValue::Date(date) => self.format_date(*date, language),
            FormatValue::Currency(amount) => self.format_currency(*amount, language),
            FormatValue::Percentage(pct) => {
                format!("{}%", self.format_number(*pct * 100.0, language, 1))
            }
        }
    }

    fn format_number(&self, value: f64, language: &LanguageInfo, decimals: usize) -> String {
        let formatted = if decimals > 0 {
            format!("{:.1$}", value, decimals)
        } else {
            format!("{:.0}", value)
        };

        // Apply number formatting rules
        let mut result = formatted;
        result = result.replace('.', &language.number_format.decimal_separator);

        // Add thousands separators (simplified)
        if let Some(dot_pos) = result.find(&language.number_format.decimal_separator) {
            let integer_part = &result[..dot_pos];
            if integer_part.len() > 3 {
                // Insert thousands separators every 3 digits
                let mut with_separators = String::new();
                for (i, c) in integer_part.chars().rev().enumerate() {
                    if i > 0 && i % 3 == 0 {
                        with_separators.push_str(&language.number_format.thousands_separator);
                    }
                    with_separators.push(c);
                }
                result = with_separators.chars().rev().collect::<String>() + &result[dot_pos..];
            }
        }

        result
    }

    fn format_date(&self, date: SystemTime, _language: &LanguageInfo) -> String {
        // Simplified date formatting - real implementation would use chrono
        format!("{:?}", date)
    }

    fn format_currency(&self, amount: f64, language: &LanguageInfo) -> String {
        let formatted_amount = self.format_number(amount, language, 2);

        if let Some(currency) = &language.currency {
            match currency.position {
                CurrencyPosition::Before => format!("{}{}", currency.symbol, formatted_amount),
                CurrencyPosition::After => format!("{}{}", formatted_amount, currency.symbol),
                CurrencyPosition::BeforeWithSpace => {
                    format!("{} {}", currency.symbol, formatted_amount)
                }
                CurrencyPosition::AfterWithSpace => {
                    format!("{} {}", formatted_amount, currency.symbol)
                }
            }
        } else {
            formatted_amount
        }
    }
}

impl Default for FormatParams {
    fn default() -> Self {
        Self::new()
    }
}

impl FormatParams {
    pub fn new() -> Self {
        Self {
            params: HashMap::new(),
        }
    }

    pub fn add<T: Into<FormatValue>>(mut self, key: &str, value: T) -> Self {
        self.params.insert(key.to_string(), value.into());
        self
    }
}

impl From<String> for FormatValue {
    fn from(s: String) -> Self {
        FormatValue::Text(s)
    }
}

impl From<&str> for FormatValue {
    fn from(s: &str) -> Self {
        FormatValue::Text(s.to_string())
    }
}

impl From<i64> for FormatValue {
    fn from(i: i64) -> Self {
        FormatValue::Integer(i)
    }
}

impl From<f64> for FormatValue {
    fn from(f: f64) -> Self {
        FormatValue::Float(f)
    }
}

impl From<bool> for FormatValue {
    fn from(b: bool) -> Self {
        FormatValue::Boolean(b)
    }
}

impl From<SystemTime> for FormatValue {
    fn from(t: SystemTime) -> Self {
        FormatValue::Date(t)
    }
}

/// Localization statistics
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct LocalizationStats {
    pub current_language: String,
    pub available_languages: u32,
    pub total_keys: u32,
    pub translated_keys: u32,
    pub missing_keys: u32,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub format_operations: u64,
    pub language_switches: u32,
    pub memory_used_kb: u32,
}

/// Complete Localization Management System
pub struct LocalizationManager {
    // Current language
    current_language: Arc<RwLock<String>>,

    // Language packs
    language_packs: Arc<RwLock<HashMap<String, LanguagePack>>>,
    available_languages: Arc<RwLock<Vec<LanguageInfo>>>,

    // Translation cache
    translation_cache: Arc<RwLock<HashMap<String, String>>>,

    // Fallback chain
    fallback_languages: Vec<String>,

    // Configuration
    base_path: PathBuf,

    // Statistics
    stats: Arc<RwLock<LocalizationStats>>,

    // Regex for format string parsing
    format_regex: Regex,
}

lazy_static! {
    static ref FORMAT_REGEX: Regex = Regex::new(r"\{(\w+)(?::([^}]+))?\}").unwrap();
}

impl LocalizationManager {
    /// Create new localization manager
    pub fn new(initial_language: String) -> Result<Self, LocalizationError> {
        let base_path = PathBuf::from("localization");

        Ok(Self {
            current_language: Arc::new(RwLock::new(initial_language.clone())),
            language_packs: Arc::new(RwLock::new(HashMap::new())),
            available_languages: Arc::new(RwLock::new(Vec::new())),
            translation_cache: Arc::new(RwLock::new(HashMap::new())),
            fallback_languages: vec!["english".to_string()],
            base_path,
            stats: Arc::new(RwLock::new(LocalizationStats {
                current_language: initial_language,
                ..Default::default()
            })),
            format_regex: FORMAT_REGEX.clone(),
        })
    }

    /// Initialize localization system
    pub async fn initialize(&self) -> Result<(), LocalizationError> {
        log::info!("Initializing localization system...");

        // Discover available language packs
        self.discover_language_packs().await?;

        // Load default language pack
        let current_lang = self
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        self.load_language_pack(&current_lang).await?;

        // Load fallback languages
        for fallback in &self.fallback_languages {
            if fallback != &current_lang {
                let _ = self.load_language_pack(fallback).await;
            }
        }

        log::info!(
            "Localization system initialized with {} languages",
            self.available_languages
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .len()
        );

        Ok(())
    }

    /// Discover available language packs
    async fn discover_language_packs(&self) -> Result<(), LocalizationError> {
        if !self.base_path.exists() {
            log::warn!(
                "Localization directory not found: {}",
                self.base_path.display()
            );
            return Ok(());
        }

        let mut available_languages = Vec::new();

        // Scan for .json language files
        let mut entries = tokio::fs::read_dir(&self.base_path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if let Some(lang_code) = path.file_stem().and_then(|s| s.to_str()) {
                    // Try to load language metadata
                    if let Ok(language_info) = self.load_language_metadata(&path).await {
                        available_languages.push(language_info);
                        log::debug!("Discovered language pack: {}", lang_code);
                    }
                }
            }
        }

        // Add built-in English if not found
        if !available_languages.iter().any(|l| l.code == "english") {
            available_languages.push(Self::create_default_english());
        }

        let count = available_languages.len();
        *self
            .available_languages
            .write()
            .unwrap_or_else(|e| e.into_inner()) = available_languages;

        // Update stats
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.available_languages = count as u32;
        }

        Ok(())
    }

    /// Load language metadata from file
    async fn load_language_metadata(&self, path: &Path) -> Result<LanguageInfo, LocalizationError> {
        let content = tokio::fs::read_to_string(path).await?;
        let pack: LanguagePack =
            serde_json::from_str(&content).map_err(|e| LocalizationError::ParseFailed {
                path: path.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        Ok(pack.language)
    }

    /// Create default English language info
    fn create_default_english() -> LanguageInfo {
        LanguageInfo {
            code: "english".to_string(),
            name: "English".to_string(),
            english_name: "English".to_string(),
            region: Some("US".to_string()),
            direction: TextDirection::LeftToRight,
            plural_rule: PluralizationRule::Simple,
            date_format: "%m/%d/%Y".to_string(),
            number_format: NumberFormat {
                decimal_separator: ".".to_string(),
                thousands_separator: ",".to_string(),
                negative_format: "-{0}".to_string(),
            },
            currency: Some(CurrencyInfo {
                code: "USD".to_string(),
                symbol: "$".to_string(),
                position: CurrencyPosition::Before,
            }),
            fonts: vec!["Arial".to_string(), "Helvetica".to_string()],
            completion: 100.0,
            is_rtl: false,
            supports_complex_scripts: false,
        }
    }

    /// Load language pack from file
    pub async fn load_language_pack(&self, language_code: &str) -> Result<(), LocalizationError> {
        let file_path = self.base_path.join(format!("{}.json", language_code));

        if !file_path.exists() {
            return Err(LocalizationError::LanguagePackNotFound {
                language: language_code.to_string(),
            });
        }

        log::info!("Loading language pack: {}", language_code);

        let content = tokio::fs::read_to_string(&file_path).await?;
        let language_pack: LanguagePack =
            serde_json::from_str(&content).map_err(|e| LocalizationError::ParseFailed {
                path: file_path.to_string_lossy().to_string(),
                error: e.to_string(),
            })?;

        // Validate language pack
        self.validate_language_pack(&language_pack)?;

        // Store language pack
        self.language_packs
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .insert(language_code.to_string(), language_pack.clone());

        // Update stats
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.total_keys = stats
                .total_keys
                .max(language_pack.translations.len() as u32);
            stats.translated_keys = language_pack.metadata.translated_strings;
            stats.memory_used_kb += (content.len() / 1024) as u32;
        }

        log::info!(
            "Loaded language pack: {} ({} translations)",
            language_code,
            language_pack.translations.len()
        );

        Ok(())
    }

    /// Validate language pack structure and content
    fn validate_language_pack(
        &self,
        language_pack: &LanguagePack,
    ) -> Result<(), LocalizationError> {
        // Check for required keys
        let required_keys = [
            "menu.main.title",
            "menu.main.new_game",
            "menu.main.exit",
            "game.loading",
            "game.paused",
        ];

        for &key in &required_keys {
            if !language_pack.translations.contains_key(key) {
                log::warn!("Missing required translation key: {}", key);
            }
        }

        // Validate format strings
        for (key, translation) in &language_pack.translations {
            if let Err(e) = self.validate_format_string(&translation.text) {
                log::error!("Invalid format string in {}: {}", key, e);
            }
        }

        Ok(())
    }

    /// Validate format string syntax
    fn validate_format_string(&self, text: &str) -> Result<(), String> {
        for captures in self.format_regex.captures_iter(text) {
            let param_name = captures.get(1).unwrap().as_str();
            if param_name.is_empty() {
                return Err("Empty parameter name".to_string());
            }

            // Validate format specifier if present
            if let Some(format_spec) = captures.get(2) {
                let spec = format_spec.as_str();
                if !self.is_valid_format_specifier(spec) {
                    return Err(format!("Invalid format specifier: {}", spec));
                }
            }
        }

        Ok(())
    }

    /// Check if format specifier is valid
    fn is_valid_format_specifier(&self, spec: &str) -> bool {
        matches!(spec, "d" | "f" | "c" | "p" | "date" | "time" | "currency")
    }

    /// Switch to different language
    pub async fn switch_language(&self, language_code: &str) -> Result<(), LocalizationError> {
        let old_language = self
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();

        if old_language == language_code {
            return Ok(()); // Already using this language
        }

        log::info!(
            "Switching language from {} to {}",
            old_language,
            language_code
        );

        // Load language pack if not already loaded
        if !self
            .language_packs
            .read()
            .unwrap()
            .contains_key(language_code)
        {
            self.load_language_pack(language_code).await?;
        }

        // Verify language pack is available
        if !self
            .language_packs
            .read()
            .unwrap()
            .contains_key(language_code)
        {
            return Err(LocalizationError::LanguageSwitchFailed {
                from: old_language,
                to: language_code.to_string(),
                error: "Language pack not available".to_string(),
            });
        }

        // Update current language
        *self
            .current_language
            .write()
            .unwrap_or_else(|e| e.into_inner()) = language_code.to_string();

        // Clear translation cache
        self.translation_cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();

        // Update stats
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.current_language = language_code.to_string();
            stats.language_switches += 1;
        }

        log::info!("Language switched successfully to {}", language_code);
        Ok(())
    }

    /// Get localized text for a key
    pub fn get_text(&self, key: &str) -> String {
        self.get_text_with_params(key, None)
    }

    /// Get localized text with formatting parameters
    pub fn get_text_with_params(&self, key: &str, params: Option<FormatParams>) -> String {
        // Check cache first
        let cache_key = if params.is_some() {
            format!("{}:formatted", key)
        } else {
            key.to_string()
        };

        {
            let cache = self
                .translation_cache
                .read()
                .unwrap_or_else(|e| e.into_inner());
            if let Some(cached_text) = cache.get(&cache_key) {
                let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
                stats.cache_hits += 1;
                return cached_text.clone();
            }
        }

        // Get translation
        let translation = self.get_translation_internal(key);
        let formatted_text = match params.as_ref() {
            Some(params_ref) => self.format_text(&translation, params_ref),
            None => translation,
        };

        // Cache result
        {
            let mut cache = self
                .translation_cache
                .write()
                .unwrap_or_else(|e| e.into_inner());
            cache.insert(cache_key, formatted_text.clone());
        }

        // Update stats
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.cache_misses += 1;
            if params.is_some() {
                stats.format_operations += 1;
            }
        }

        formatted_text
    }

    /// Get pluralized text based on count
    pub fn get_plural_text(&self, key: &str, count: i64, params: Option<FormatParams>) -> String {
        let current_lang = self
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());

        if let Some(pack) = language_packs.get(&current_lang) {
            if let Some(translation) = pack.translations.get(key) {
                let plural_form = self.get_plural_form(count, &pack.language.plural_rule);

                if plural_form < translation.plural_forms.len() {
                    let text = &translation.plural_forms[plural_form];
                    return if let Some(params) = params {
                        self.format_text(text, &params)
                    } else {
                        text.clone()
                    };
                }
            }
        }

        // Fallback to regular translation
        self.get_text_with_params(key, params)
    }

    /// Get translation with fallback logic
    fn get_translation_internal(&self, key: &str) -> String {
        let current_lang = self
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());

        // Try current language first
        if let Some(pack) = language_packs.get(&current_lang) {
            if let Some(translation) = pack.translations.get(key) {
                return translation.text.clone();
            }
        }

        // Try fallback languages
        for fallback_lang in &self.fallback_languages {
            if fallback_lang == &current_lang {
                continue; // Already tried
            }

            if let Some(pack) = language_packs.get(fallback_lang) {
                if let Some(translation) = pack.translations.get(key) {
                    log::debug!(
                        "Using fallback translation for '{}' from {}",
                        key,
                        fallback_lang
                    );
                    return translation.text.clone();
                }
            }
        }

        // No translation found
        {
            let mut stats = self.stats.write().unwrap_or_else(|e| e.into_inner());
            stats.missing_keys += 1;
        }

        log::warn!("Translation not found: {}", key);
        format!("[MISSING: {}]", key)
    }

    /// Format text with parameters
    fn format_text(&self, template: &str, params: &FormatParams) -> String {
        let current_lang = self
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let language_info = language_packs
            .get(&current_lang)
            .map(|pack| pack.language.clone())
            .unwrap_or_else(Self::create_default_english);

        let mut result = template.to_string();

        for captures in self.format_regex.captures_iter(template) {
            let full_match = captures.get(0).unwrap().as_str();
            let param_name = captures.get(1).unwrap().as_str();
            let format_spec = captures.get(2).map(|m| m.as_str());

            if let Some(value) = params.params.get(param_name) {
                let formatted_value = match format_spec {
                    Some("d") => match value {
                        FormatValue::Integer(i) => i.to_string(),
                        FormatValue::Float(f) => (*f as i64).to_string(),
                        _ => value.format(&language_info),
                    },
                    Some("f") => match value {
                        FormatValue::Float(f) => format!("{:.2}", f),
                        FormatValue::Integer(i) => format!("{:.2}", *i as f64),
                        _ => value.format(&language_info),
                    },
                    Some("c") => match value {
                        FormatValue::Float(f) => FormatValue::Currency(*f).format(&language_info),
                        FormatValue::Integer(i) => {
                            FormatValue::Currency(*i as f64).format(&language_info)
                        }
                        _ => value.format(&language_info),
                    },
                    Some("p") => match value {
                        FormatValue::Float(f) => FormatValue::Percentage(*f).format(&language_info),
                        _ => value.format(&language_info),
                    },
                    _ => value.format(&language_info),
                };

                result = result.replace(full_match, &formatted_value);
            } else {
                log::warn!(
                    "Format parameter '{}' not provided for template: {}",
                    param_name,
                    template
                );
            }
        }

        result
    }

    /// Get appropriate plural form index based on count and language rules
    fn get_plural_form(&self, count: i64, rule: &PluralizationRule) -> usize {
        match rule {
            PluralizationRule::None => 0,
            PluralizationRule::Simple => {
                if count == 1 {
                    0
                } else {
                    1
                }
            }
            PluralizationRule::Slavic => {
                if count % 10 == 1 && count % 100 != 11 {
                    0 // one
                } else if (2..=4).contains(&(count % 10)) && !(12..=14).contains(&(count % 100)) {
                    1 // few
                } else {
                    2 // many
                }
            }
            PluralizationRule::Arabic => {
                match count {
                    0 => 0,                                   // zero
                    1 => 1,                                   // one
                    2 => 2,                                   // two
                    n if (3..=10).contains(&(n % 100)) => 3,  // few
                    n if (11..=99).contains(&(n % 100)) => 4, // many
                    _ => 5,                                   // other
                }
            }
            PluralizationRule::Custom(_expr) => Self::evaluate_plural_expression(_expr, count)
                .unwrap_or(if count == 1 { 0 } else { 1 }),
        }
    }

    fn evaluate_plural_expression(expr: &str, count: i64) -> Option<usize> {
        let mut parser = PluralExpressionParser::new(expr, count);
        let value = parser.parse_expression().ok()?;
        if parser.has_remaining_tokens() {
            return None;
        }
        let value = if value < 0 { 0 } else { value as usize };
        Some(value)
    }
    /// Get list of available languages
    pub fn get_available_languages(&self) -> Vec<LanguageInfo> {
        self.available_languages
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// Get current language info
    pub fn get_current_language(&self) -> Option<LanguageInfo> {
        let current_lang = self
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());
        language_packs
            .get(&current_lang)
            .map(|pack| pack.language.clone())
    }

    /// Check if language is RTL (right-to-left)
    pub fn is_rtl(&self) -> bool {
        self.get_current_language()
            .is_some_and(|lang| lang.is_rtl)
    }

    /// Get language completion percentage
    pub fn get_completion_percentage(&self, language_code: &str) -> f32 {
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());
        language_packs
            .get(language_code)
            .map_or(0.0, |pack| pack.language.completion)
    }

    /// Get localization statistics
    pub fn get_stats(&self) -> LocalizationStats {
        self.stats.read().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// Preload commonly used translations into cache
    pub fn preload_common_translations(&self) {
        let common_keys = [
            "menu.main.title",
            "menu.main.new_game",
            "menu.main.load_game",
            "menu.main.options",
            "menu.main.exit",
            "game.loading",
            "game.paused",
            "game.victory",
            "game.defeat",
            "ui.ok",
            "ui.cancel",
            "ui.yes",
            "ui.no",
        ];

        for &key in &common_keys {
            let _ = self.get_text(key);
        }

        log::debug!("Preloaded {} common translations", common_keys.len());
    }

    /// Clear translation cache
    pub fn clear_cache(&self) {
        self.translation_cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
        log::debug!("Translation cache cleared");
    }

    /// Export translations to file (for translators)
    pub async fn export_translations(
        &self,
        language_code: &str,
        output_path: &Path,
    ) -> Result<(), LocalizationError> {
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(pack) = language_packs.get(language_code) {
            let json = serde_json::to_string_pretty(pack)
                .map_err(|e| LocalizationError::EncodingError(e.to_string()))?;

            tokio::fs::write(output_path, json).await?;
            log::info!(
                "Exported translations for {} to {}",
                language_code,
                output_path.display()
            );
            Ok(())
        } else {
            Err(LocalizationError::LanguagePackNotFound {
                language: language_code.to_string(),
            })
        }
    }

    /// Validate all loaded language packs
    pub fn validate_all_languages(&self) -> HashMap<String, Vec<String>> {
        let mut validation_results = HashMap::new();
        let language_packs = self
            .language_packs
            .read()
            .unwrap_or_else(|e| e.into_inner());

        for (lang_code, pack) in language_packs.iter() {
            let mut issues = Vec::new();

            // Check translation coverage
            if pack.language.completion < 90.0 {
                issues.push(format!("Low completion: {:.1}%", pack.language.completion));
            }

            // Check for format string issues
            for (key, translation) in &pack.translations {
                if let Err(error) = self.validate_format_string(&translation.text) {
                    issues.push(format!("Format error in '{}': {}", key, error));
                }
            }

            if !issues.is_empty() {
                validation_results.insert(lang_code.clone(), issues);
            }
        }

        validation_results
    }
}

impl From<LocalizationError> for AssetError {
    fn from(err: LocalizationError) -> Self {
        match err {
            LocalizationError::Io(io_err) => AssetError::Io(io_err),
            _ => AssetError::LoadingFailed {
                path: "localization".to_string(),
                error: err.to_string(),
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluralToken {
    Number(i64),
    Ident,
    Op(PluralOp),
    LParen,
    RParen,
    Question,
    Colon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PluralOp {
    Or,
    And,
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Not,
}

struct PluralExpressionParser {
    tokens: Vec<PluralToken>,
    pos: usize,
    count: i64,
}

impl PluralExpressionParser {
    fn new(expr: &str, count: i64) -> Self {
        Self {
            tokens: Self::tokenize(expr),
            pos: 0,
            count,
        }
    }

    fn has_remaining_tokens(&self) -> bool {
        self.pos < self.tokens.len()
    }

    fn tokenize(expr: &str) -> Vec<PluralToken> {
        let mut tokens = Vec::new();
        let mut iter = expr.chars().peekable();
        while let Some(ch) = iter.peek().copied() {
            if ch.is_whitespace() {
                iter.next();
                continue;
            }

            if ch.is_ascii_digit() {
                let mut number = String::new();
                while let Some(digit) = iter.peek().copied() {
                    if digit.is_ascii_digit() {
                        number.push(digit);
                        iter.next();
                    } else {
                        break;
                    }
                }
                if let Ok(value) = number.parse::<i64>() {
                    tokens.push(PluralToken::Number(value));
                }
                continue;
            }

            match ch {
                'n' | 'N' => {
                    tokens.push(PluralToken::Ident);
                    iter.next();
                }
                '(' => {
                    tokens.push(PluralToken::LParen);
                    iter.next();
                }
                ')' => {
                    tokens.push(PluralToken::RParen);
                    iter.next();
                }
                '?' => {
                    tokens.push(PluralToken::Question);
                    iter.next();
                }
                ':' => {
                    tokens.push(PluralToken::Colon);
                    iter.next();
                }
                '&' => {
                    iter.next();
                    if iter.peek() == Some(&'&') {
                        iter.next();
                        tokens.push(PluralToken::Op(PluralOp::And));
                    }
                }
                '|' => {
                    iter.next();
                    if iter.peek() == Some(&'|') {
                        iter.next();
                        tokens.push(PluralToken::Op(PluralOp::Or));
                    }
                }
                '=' => {
                    iter.next();
                    if iter.peek() == Some(&'=') {
                        iter.next();
                        tokens.push(PluralToken::Op(PluralOp::Eq));
                    }
                }
                '!' => {
                    iter.next();
                    if iter.peek() == Some(&'=') {
                        iter.next();
                        tokens.push(PluralToken::Op(PluralOp::Ne));
                    } else {
                        tokens.push(PluralToken::Op(PluralOp::Not));
                    }
                }
                '<' => {
                    iter.next();
                    if iter.peek() == Some(&'=') {
                        iter.next();
                        tokens.push(PluralToken::Op(PluralOp::Lte));
                    } else {
                        tokens.push(PluralToken::Op(PluralOp::Lt));
                    }
                }
                '>' => {
                    iter.next();
                    if iter.peek() == Some(&'=') {
                        iter.next();
                        tokens.push(PluralToken::Op(PluralOp::Gte));
                    } else {
                        tokens.push(PluralToken::Op(PluralOp::Gt));
                    }
                }
                '+' => {
                    tokens.push(PluralToken::Op(PluralOp::Add));
                    iter.next();
                }
                '-' => {
                    tokens.push(PluralToken::Op(PluralOp::Sub));
                    iter.next();
                }
                '*' => {
                    tokens.push(PluralToken::Op(PluralOp::Mul));
                    iter.next();
                }
                '/' => {
                    tokens.push(PluralToken::Op(PluralOp::Div));
                    iter.next();
                }
                '%' => {
                    tokens.push(PluralToken::Op(PluralOp::Mod));
                    iter.next();
                }
                _ => {
                    iter.next();
                }
            }
        }
        tokens
    }

    fn parse_expression(&mut self) -> Result<i64, String> {
        self.parse_ternary()
    }

    fn parse_ternary(&mut self) -> Result<i64, String> {
        let condition = self.parse_or()?;
        if self.consume(PluralToken::Question) {
            let true_expr = self.parse_expression()?;
            if !self.consume(PluralToken::Colon) {
                return Err("Missing ':' in ternary expression".to_string());
            }
            let false_expr = self.parse_expression()?;
            Ok(if condition != 0 {
                true_expr
            } else {
                false_expr
            })
        } else {
            Ok(condition)
        }
    }

    fn parse_or(&mut self) -> Result<i64, String> {
        let mut value = self.parse_and()?;
        while self.consume_op(PluralOp::Or) {
            let rhs = self.parse_and()?;
            value = if value != 0 || rhs != 0 { 1 } else { 0 };
        }
        Ok(value)
    }

    fn parse_and(&mut self) -> Result<i64, String> {
        let mut value = self.parse_equality()?;
        while self.consume_op(PluralOp::And) {
            let rhs = self.parse_equality()?;
            value = if value != 0 && rhs != 0 { 1 } else { 0 };
        }
        Ok(value)
    }

    fn parse_equality(&mut self) -> Result<i64, String> {
        let mut value = self.parse_relational()?;
        loop {
            if self.consume_op(PluralOp::Eq) {
                let rhs = self.parse_relational()?;
                value = if value == rhs { 1 } else { 0 };
            } else if self.consume_op(PluralOp::Ne) {
                let rhs = self.parse_relational()?;
                value = if value != rhs { 1 } else { 0 };
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_relational(&mut self) -> Result<i64, String> {
        let mut value = self.parse_add()?;
        loop {
            if self.consume_op(PluralOp::Lt) {
                let rhs = self.parse_add()?;
                value = if value < rhs { 1 } else { 0 };
            } else if self.consume_op(PluralOp::Lte) {
                let rhs = self.parse_add()?;
                value = if value <= rhs { 1 } else { 0 };
            } else if self.consume_op(PluralOp::Gt) {
                let rhs = self.parse_add()?;
                value = if value > rhs { 1 } else { 0 };
            } else if self.consume_op(PluralOp::Gte) {
                let rhs = self.parse_add()?;
                value = if value >= rhs { 1 } else { 0 };
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_add(&mut self) -> Result<i64, String> {
        let mut value = self.parse_mul()?;
        loop {
            if self.consume_op(PluralOp::Add) {
                let rhs = self.parse_mul()?;
                value = value.saturating_add(rhs);
            } else if self.consume_op(PluralOp::Sub) {
                let rhs = self.parse_mul()?;
                value = value.saturating_sub(rhs);
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_mul(&mut self) -> Result<i64, String> {
        let mut value = self.parse_unary()?;
        loop {
            if self.consume_op(PluralOp::Mul) {
                let rhs = self.parse_unary()?;
                value = value.saturating_mul(rhs);
            } else if self.consume_op(PluralOp::Div) {
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err("Division by zero".to_string());
                }
                value /= rhs;
            } else if self.consume_op(PluralOp::Mod) {
                let rhs = self.parse_unary()?;
                if rhs == 0 {
                    return Err("Modulo by zero".to_string());
                }
                value %= rhs;
            } else {
                break;
            }
        }
        Ok(value)
    }

    fn parse_unary(&mut self) -> Result<i64, String> {
        if self.consume_op(PluralOp::Not) {
            let value = self.parse_unary()?;
            return Ok(if value == 0 { 1 } else { 0 });
        }
        if self.consume_op(PluralOp::Sub) {
            let value = self.parse_unary()?;
            return Ok(value.saturating_mul(-1));
        }
        if self.consume_op(PluralOp::Add) {
            return self.parse_unary();
        }
        self.parse_primary()
    }

    fn parse_primary(&mut self) -> Result<i64, String> {
        if let Some(token) = self.peek() {
            match token {
                PluralToken::Number(value) => {
                    self.pos += 1;
                    return Ok(value);
                }
                PluralToken::Ident => {
                    self.pos += 1;
                    return Ok(self.count);
                }
                PluralToken::LParen => {
                    self.pos += 1;
                    let value = self.parse_expression()?;
                    if !self.consume(PluralToken::RParen) {
                        return Err("Missing ')'".to_string());
                    }
                    return Ok(value);
                }
                _ => {}
            }
        }
        Err("Unexpected token in expression".to_string())
    }

    fn consume(&mut self, token: PluralToken) -> bool {
        if self.peek() == Some(token) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn consume_op(&mut self, op: PluralOp) -> bool {
        if self.peek() == Some(PluralToken::Op(op)) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn peek(&self) -> Option<PluralToken> {
        self.tokens.get(self.pos).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_localization_manager_creation() {
        let manager = LocalizationManager::new("english".to_string()).unwrap();
        let current = manager
            .current_language
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        assert_eq!(current, "english");
    }

    #[test]
    fn test_format_value_conversion() {
        let value: FormatValue = "test".into();
        match value {
            FormatValue::Text(s) => assert_eq!(s, "test"),
            _ => panic!("Wrong format value type"),
        }
    }

    #[test]
    fn test_plural_form_calculation() {
        let manager = LocalizationManager::new("english".to_string()).unwrap();

        // English simple pluralization
        assert_eq!(manager.get_plural_form(1, &PluralizationRule::Simple), 0);
        assert_eq!(manager.get_plural_form(2, &PluralizationRule::Simple), 1);
        assert_eq!(manager.get_plural_form(0, &PluralizationRule::Simple), 1);
    }

    #[test]
    fn test_custom_plural_expression() {
        let manager = LocalizationManager::new("english".to_string()).unwrap();
        let rule = PluralizationRule::Custom("n==1?0:1".to_string());

        assert_eq!(manager.get_plural_form(1, &rule), 0);
        assert_eq!(manager.get_plural_form(2, &rule), 1);
    }

    #[test]
    fn test_format_string_validation() {
        let manager = LocalizationManager::new("english".to_string()).unwrap();

        assert!(manager.validate_format_string("Hello {name}!").is_ok());
        assert!(manager.validate_format_string("Score: {points:d}").is_ok());
        assert!(manager.validate_format_string("Price: {amount:c}").is_ok());

        // These should be invalid
        assert!(manager.validate_format_string("Hello {}!").is_err());
    }

    #[test]
    fn test_format_params_builder() {
        let params = FormatParams::new()
            .add("name", "Player")
            .add("score", 1000i64)
            .add("accuracy", 0.95f64);

        assert_eq!(params.params.len(), 3);
    }
}
