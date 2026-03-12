/*
** Command & Conquer Generals Zero Hour(tm)
** Lightweight localization bridge for the Rust port.
**
** This module mirrors the behavior of the original SAGE GlobalLanguage
** system by loading language-specific key/value pairs from disk based on
** the currently selected language (typically dictated by the command line
** or game configuration).  The format is intentionally simple: each
** language is stored as a JSON object containing string pairs.
*/

use log::{debug, error, info};
use parking_lot::RwLock;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::OnceLock;

#[derive(Debug, Default)]
pub struct LocalizationManager {
    current_language: String,
    translations: HashMap<String, String>,
    fallback_translations: HashMap<String, String>,
    base_paths: Vec<PathBuf>,
}

#[derive(Debug, Deserialize)]
struct TranslationFile(HashMap<String, String>);

impl LocalizationManager {
    pub fn new(language: &str) -> Self {
        let mut manager = Self {
            current_language: String::new(),
            translations: HashMap::new(),
            fallback_translations: HashMap::new(),
            base_paths: vec![
                PathBuf::from("Data/Localization"),
                PathBuf::from("Localization"),
            ],
        };
        manager.load_fallback_language();
        manager.set_language(language);
        manager
    }

    pub fn set_language(&mut self, language: &str) {
        let normalized = if language.is_empty() {
            "English".to_string()
        } else {
            language.to_string()
        };

        if self
            .current_language
            .eq_ignore_ascii_case(normalized.as_str())
        {
            return;
        }

        match self.load_language_map(&normalized) {
            Some(map) => {
                info!("Localization: loaded language '{}'", normalized);
                self.translations = map;
                self.current_language = normalized;
            }
            None => {
                debug!(
                    "Localization pack not found for '{}'; falling back to English",
                    normalized
                );
                self.translations = self.fallback_translations.clone();
                self.current_language = "English".to_string();
            }
        }
    }

    pub fn translate(&self, key: &str) -> Option<String> {
        self.translations
            .get(key)
            .cloned()
            .or_else(|| self.fallback_translations.get(key).cloned())
    }

    pub fn set_base_paths(&mut self, paths: &[PathBuf]) {
        let mut merged = Vec::new();
        for path in paths {
            if !merged.iter().any(|p: &PathBuf| p == path) {
                merged.push(path.clone());
            }
        }
        if merged.is_empty() {
            merged.push(PathBuf::from("Data/Localization"));
            merged.push(PathBuf::from("Localization"));
        }
        self.base_paths = merged;
    }

    fn load_fallback_language(&mut self) {
        if let Some(map) = self.load_language_map("English") {
            self.fallback_translations = map;
        } else {
            debug!("Fallback English localization missing; using empty dictionary");
            self.fallback_translations.clear();
        }
    }

    fn load_language_map(&self, language: &str) -> Option<HashMap<String, String>> {
        for path in self.language_paths(language) {
            if path.exists() {
                match fs::read_to_string(&path) {
                    Ok(data) => match serde_json::from_str::<TranslationFile>(&data) {
                        Ok(TranslationFile(map)) => {
                            debug!("Loaded {} entries from {}", map.len(), path.display());
                            return Some(map);
                        }
                        Err(err) => {
                            error!("Failed to parse localization {}: {}", path.display(), err)
                        }
                    },
                    Err(err) => error!(
                        "Failed to read localization file {}: {}",
                        path.display(),
                        err
                    ),
                }
            }
        }
        None
    }

    fn language_paths(&self, language: &str) -> Vec<PathBuf> {
        let mut paths = Vec::new();
        let normalized = language.replace('\\', "/");
        let lower = normalized.to_lowercase();

        for base in &self.base_paths {
            paths.push(base.join(format!("{}.json", normalized)));
            paths.push(base.join(format!("{}.json", lower)));
        }

        paths
    }
}

static GLOBAL_LOCALIZATION: OnceLock<Arc<RwLock<LocalizationManager>>> = OnceLock::new();

fn manager() -> Arc<RwLock<LocalizationManager>> {
    GLOBAL_LOCALIZATION
        .get_or_init(|| Arc::new(RwLock::new(LocalizationManager::new("English"))))
        .clone()
}

/// Initialize the localization system (idempotent).
pub fn init(language: &str) {
    let mgr = manager();
    mgr.write().set_language(language);
}

/// Change the active language at runtime.
pub fn set_language(language: &str) {
    let mgr = manager();
    mgr.write().set_language(language);
}

/// Override the search paths used to find localization packs.
pub fn set_search_paths(paths: &[PathBuf]) {
    let mgr = manager();
    mgr.write().set_base_paths(paths);
}

/// Translate a key, returning `None` if no translation exists.
pub fn translate(key: &str) -> Option<String> {
    manager().read().translate(key)
}

/// Translate a key, returning the fallback if missing.
pub fn localize(key: &str, fallback: &str) -> String {
    translate(key).unwrap_or_else(|| fallback.to_string())
}

/// Translate a key and substitute `{placeholder}` tokens with the provided values.
pub fn localize_with_args(key: &str, fallback: &str, replacements: &[(&str, &str)]) -> String {
    let mut value = localize(key, fallback);
    for (placeholder, replacement) in replacements {
        let token = format!("{{{}}}", placeholder);
        value = value.replace(&token, replacement);
    }
    value
}
