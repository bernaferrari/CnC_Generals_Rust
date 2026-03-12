////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// FILE: language.rs /////////////////////////////////////////////////////////
//-----------------------------------------------------------------------------
//
//                       Westwood Studios Pacific.
//
//                       Confidential Information
//                Copyright (C) 2001 - All Rights Reserved
//
//-----------------------------------------------------------------------------
//
// Project:   RTS3
//
// File name: language.rs
//
// Created:   Colin Day, June 2001
//
// Desc:      For dealing with multiple languages
//
//-----------------------------------------------------------------------------
///////////////////////////////////////////////////////////////////////////////

use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::sync::{Mutex, RwLock};

/// Language identifiers
/// IMPORTANT: Make sure this enum is identical to the one in Noxstring tool
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum LanguageId {
    Us = 0,
    Uk,
    German,
    French,
    Spanish,
    Italian,
    Japanese,
    Jabber,
    Korean,
    Unknown,
}

impl Default for LanguageId {
    fn default() -> Self {
        LanguageId::Us
    }
}

impl LanguageId {
    /// Get language ID from string name
    pub fn from_name(name: &str) -> Option<Self> {
        match name.to_lowercase().as_str() {
            "us" | "english" => Some(LanguageId::Us),
            "uk" | "british" => Some(LanguageId::Uk),
            "german" | "de" => Some(LanguageId::German),
            "french" | "fr" => Some(LanguageId::French),
            "spanish" | "es" => Some(LanguageId::Spanish),
            "italian" | "it" => Some(LanguageId::Italian),
            "japanese" | "ja" => Some(LanguageId::Japanese),
            "jabber" => Some(LanguageId::Jabber),
            "korean" | "ko" => Some(LanguageId::Korean),
            _ => None,
        }
    }

    /// Get the string name for this language
    pub fn to_name(self) -> &'static str {
        match self {
            LanguageId::Us => "US",
            LanguageId::Uk => "UK",
            LanguageId::German => "German",
            LanguageId::French => "French",
            LanguageId::Spanish => "Spanish",
            LanguageId::Italian => "Italian",
            LanguageId::Japanese => "Japanese",
            LanguageId::Jabber => "Jabber",
            LanguageId::Korean => "Korean",
            LanguageId::Unknown => "Unknown",
        }
    }

    /// Get the ISO code for this language
    pub fn to_iso_code(self) -> &'static str {
        match self {
            LanguageId::Us => "en-US",
            LanguageId::Uk => "en-GB",
            LanguageId::German => "de",
            LanguageId::French => "fr",
            LanguageId::Spanish => "es",
            LanguageId::Italian => "it",
            LanguageId::Japanese => "ja",
            LanguageId::Jabber => "xx", // Special language for testing
            LanguageId::Korean => "ko",
            LanguageId::Unknown => "unknown",
        }
    }

    /// Check if this language uses right-to-left text
    pub fn is_rtl(self) -> bool {
        // None of the supported languages use RTL, but this could be extended
        false
    }

    /// Check if this language requires wide character support
    pub fn requires_wide_chars(self) -> bool {
        matches!(self, LanguageId::Japanese | LanguageId::Korean)
    }

    /// Get all available languages
    pub fn all_languages() -> &'static [LanguageId] {
        &[
            LanguageId::Us,
            LanguageId::Uk,
            LanguageId::German,
            LanguageId::French,
            LanguageId::Spanish,
            LanguageId::Italian,
            LanguageId::Japanese,
            LanguageId::Jabber,
            LanguageId::Korean,
        ]
    }
}

/// Global language setting
static CURRENT_LANGUAGE: Mutex<LanguageId> = Mutex::new(LanguageId::Us);
static LOCALIZED_STRINGS: Lazy<RwLock<HashMap<String, String>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

/// Get the current language
pub fn get_current_language() -> LanguageId {
    *CURRENT_LANGUAGE.lock().unwrap()
}

/// Set the current language
pub fn set_current_language(language: LanguageId) {
    *CURRENT_LANGUAGE.lock().unwrap() = language;
}

/// Language utility functions
pub struct Language;

impl Language {
    /// Initialize language system
    pub fn init() {
        let detected = Self::detect_system_language();
        if Self::is_language_supported(detected) {
            set_current_language(detected);
        }
    }

    /// Detect system language
    pub fn detect_system_language() -> LanguageId {
        fn parse_locale_tag(raw: &str) -> Option<LanguageId> {
            let locale = raw.trim();
            if locale.is_empty() {
                return None;
            }

            let normalized = locale
                .split(['.', '@'])
                .next()
                .unwrap_or(locale)
                .replace('_', "-")
                .to_ascii_lowercase();
            let base = normalized.split('-').next().unwrap_or("").trim();

            match base {
                "en" => {
                    if normalized.contains("gb") || normalized.contains("uk") {
                        Some(LanguageId::Uk)
                    } else {
                        Some(LanguageId::Us)
                    }
                }
                "de" => Some(LanguageId::German),
                "fr" => Some(LanguageId::French),
                "es" => Some(LanguageId::Spanish),
                "it" => Some(LanguageId::Italian),
                "ja" => Some(LanguageId::Japanese),
                "ko" => Some(LanguageId::Korean),
                _ => None,
            }
        }

        for key in ["LC_ALL", "LC_MESSAGES", "LANG", "LANGUAGE"] {
            if let Ok(value) = std::env::var(key) {
                if let Some(language) = parse_locale_tag(&value) {
                    return language;
                }
            }
        }

        LanguageId::Us
    }

    /// Check if a language is supported
    pub fn is_language_supported(language: LanguageId) -> bool {
        !matches!(language, LanguageId::Unknown)
    }

    /// Register or replace a localized string in the runtime table.
    pub fn register_localized_string<K: Into<String>, V: Into<String>>(key: K, value: V) {
        if let Ok(mut table) = LOCALIZED_STRINGS.write() {
            table.insert(key.into(), value.into());
        }
    }

    /// Clear all runtime localized strings.
    pub fn clear_localized_strings() {
        if let Ok(mut table) = LOCALIZED_STRINGS.write() {
            table.clear();
        }
    }

    /// Get localized string from runtime table, falling back to original key.
    pub fn get_localized_string(key: &str) -> String {
        let trimmed = key.trim();
        if trimmed.is_empty() {
            return String::new();
        }

        let lookup = trimmed.strip_prefix("LOC:").unwrap_or(trimmed);
        if let Ok(table) = LOCALIZED_STRINGS.read() {
            if let Some(value) = table.get(lookup) {
                return value.clone();
            }
            if let Some(value) = table.get(trimmed) {
                return value.clone();
            }
        }

        lookup.to_string()
    }

    /// Format localized string with indexed replacements (`{0}`, `{1}`, ...).
    pub fn format_localized_string(key: &str, args: &[&str]) -> String {
        let mut result = Self::get_localized_string(key);
        for (i, arg) in args.iter().enumerate() {
            result = result.replace(&format!("{{{}}}", i), arg);
        }
        result
    }
}

/// Game-specific string utility macros/functions
/// These correspond to the C++ macros in the original header

/// String operations (using standard Rust string operations)
pub trait GameString {
    fn game_strlen(&self) -> usize;
    fn game_strcmp(&self, other: &str) -> std::cmp::Ordering;
    fn game_stricmp(&self, other: &str) -> std::cmp::Ordering;
    fn game_contains(&self, pattern: &str) -> bool;
    fn game_starts_with(&self, prefix: &str) -> bool;
    fn game_to_i32(&self) -> Option<i32>;
    fn game_to_f64(&self) -> Option<f64>;
}

impl GameString for str {
    fn game_strlen(&self) -> usize {
        self.len()
    }

    fn game_strcmp(&self, other: &str) -> std::cmp::Ordering {
        self.cmp(other)
    }

    fn game_stricmp(&self, other: &str) -> std::cmp::Ordering {
        self.to_lowercase().cmp(&other.to_lowercase())
    }

    fn game_contains(&self, pattern: &str) -> bool {
        self.contains(pattern)
    }

    fn game_starts_with(&self, prefix: &str) -> bool {
        self.starts_with(prefix)
    }

    fn game_to_i32(&self) -> Option<i32> {
        self.trim().parse().ok()
    }

    fn game_to_f64(&self) -> Option<f64> {
        self.trim().parse().ok()
    }
}

impl GameString for String {
    fn game_strlen(&self) -> usize {
        self.as_str().game_strlen()
    }

    fn game_strcmp(&self, other: &str) -> std::cmp::Ordering {
        self.as_str().game_strcmp(other)
    }

    fn game_stricmp(&self, other: &str) -> std::cmp::Ordering {
        self.as_str().game_stricmp(other)
    }

    fn game_contains(&self, pattern: &str) -> bool {
        self.as_str().game_contains(pattern)
    }

    fn game_starts_with(&self, prefix: &str) -> bool {
        self.as_str().game_starts_with(prefix)
    }

    fn game_to_i32(&self) -> Option<i32> {
        self.as_str().game_to_i32()
    }

    fn game_to_f64(&self) -> Option<f64> {
        self.as_str().game_to_f64()
    }
}

/// Character classification functions
pub fn game_is_digit(ch: char) -> bool {
    ch.is_ascii_digit()
}

pub fn game_is_ascii(ch: char) -> bool {
    ch.is_ascii()
}

pub fn game_is_alphanumeric(ch: char) -> bool {
    ch.is_alphanumeric()
}

pub fn game_is_alpha(ch: char) -> bool {
    ch.is_alphabetic()
}

/// String formatting utilities
pub fn game_sprintf(format: &str, args: &[&dyn std::fmt::Display]) -> String {
    // Simple implementation - in reality would need more sophisticated formatting
    let mut result = format.to_string();
    for (i, arg) in args.iter().enumerate() {
        result = result.replace(&format!("%{}", i), &format!("{}", arg));
    }
    result
}

/// Initialize global language (equivalent to C++ global variable)
pub fn initialize_language_system() {
    let detected = Language::detect_system_language();
    set_current_language(detected);
    Language::init();
}

/// Check if current language is valid
pub fn is_current_language_valid() -> bool {
    Language::is_language_supported(get_current_language())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_language_creation() {
        let lang = LanguageId::Us;
        assert_eq!(lang.to_name(), "US");
        assert_eq!(lang.to_iso_code(), "en-US");
        assert!(!lang.is_rtl());
        assert!(!lang.requires_wide_chars());
    }

    #[test]
    fn test_language_from_name() {
        assert_eq!(LanguageId::from_name("german"), Some(LanguageId::German));
        assert_eq!(LanguageId::from_name("fr"), Some(LanguageId::French));
        assert_eq!(LanguageId::from_name("invalid"), None);
    }

    #[test]
    fn test_wide_char_languages() {
        assert!(LanguageId::Japanese.requires_wide_chars());
        assert!(LanguageId::Korean.requires_wide_chars());
        assert!(!LanguageId::Us.requires_wide_chars());
    }

    #[test]
    fn test_current_language() {
        set_current_language(LanguageId::German);
        assert_eq!(get_current_language(), LanguageId::German);
    }

    #[test]
    fn test_string_operations() {
        use GameString;

        let test_str = "Hello World";
        assert_eq!(test_str.game_strlen(), 11);
        assert!(test_str.game_contains("World"));
        assert!(test_str.game_starts_with("Hello"));

        assert_eq!("123".game_to_i32(), Some(123));
        assert_eq!("3.14".game_to_f64(), Some(3.14));
    }

    #[test]
    fn test_character_classification() {
        assert!(game_is_digit('5'));
        assert!(!game_is_digit('a'));
        assert!(game_is_ascii('a'));
        assert!(game_is_alpha('z'));
        assert!(game_is_alphanumeric('5'));
    }

    #[test]
    fn test_language_support() {
        assert!(Language::is_language_supported(LanguageId::Us));
        assert!(Language::is_language_supported(LanguageId::Japanese));
        assert!(!Language::is_language_supported(LanguageId::Unknown));
    }
}
