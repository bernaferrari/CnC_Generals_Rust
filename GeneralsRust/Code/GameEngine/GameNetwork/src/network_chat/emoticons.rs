//! Emoticons Module
//!
//! Provides custom emoticon support for chat messages

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Emoticon definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Emoticon {
    /// Emoticon name
    pub name: String,
    /// Shortcut/text trigger (e.g., ":)")
    pub shortcut: String,
    /// Image data (optional, for custom emoticons)
    pub image_data: Option<Vec<u8>>,
    /// Image URL (optional, for remote emoticons)
    pub image_url: Option<String>,
    /// Category
    pub category: EmoticonCategory,
    /// Is custom (user-defined)
    pub is_custom: bool,
}

/// Emoticon categories
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EmoticonCategory {
    /// Standard emoticons (smile, sad, etc.)
    Standard,
    /// Action emoticons (wave, dance, etc.)
    Action,
    /// Game-specific emoticons
    Game,
    /// Custom user emoticons
    Custom,
}

/// Emoticon manager
///
/// Single-owner state: only the owning struct's methods touch the tables, so
/// they are plain fields (the old shared-handle ceremony bought nothing --
/// the only "shared" use was a one-shot spawn that populated the defaults
/// asynchronously, which is now done synchronously in `new`).
#[derive(Clone)]
pub struct EmoticonManager {
    /// Available emoticons
    emoticons: HashMap<String, Emoticon>,
    /// Shortcut to emoticon mapping
    shortcuts: HashMap<String, String>,
}

impl EmoticonManager {
    /// Create new emoticon manager
    pub fn new() -> Self {
        let mut manager = Self {
            emoticons: HashMap::new(),
            shortcuts: HashMap::new(),
        };

        // Initialize default emoticons
        manager.initialize_default_emoticons();

        manager
    }

    /// Initialize default emoticon set
    fn initialize_default_emoticons(&mut self) {
        let default_emoticons = vec![
            Emoticon {
                name: "smile".to_string(),
                shortcut: ":)".to_string(),
                image_data: None,
                image_url: Some("emoticons/smile.png".to_string()),
                category: EmoticonCategory::Standard,
                is_custom: false,
            },
            Emoticon {
                name: "sad".to_string(),
                shortcut: ":(".to_string(),
                image_data: None,
                image_url: Some("emoticons/sad.png".to_string()),
                category: EmoticonCategory::Standard,
                is_custom: false,
            },
            Emoticon {
                name: "laugh".to_string(),
                shortcut: ":D".to_string(),
                image_data: None,
                image_url: Some("emoticons/laugh.png".to_string()),
                category: EmoticonCategory::Standard,
                is_custom: false,
            },
            Emoticon {
                name: "wink".to_string(),
                shortcut: ";)".to_string(),
                image_data: None,
                image_url: Some("emoticons/wink.png".to_string()),
                category: EmoticonCategory::Standard,
                is_custom: false,
            },
            Emoticon {
                name: "wave".to_string(),
                shortcut: "/wave".to_string(),
                image_data: None,
                image_url: Some("emoticons/wave.png".to_string()),
                category: EmoticonCategory::Action,
                is_custom: false,
            },
            Emoticon {
                name: "dance".to_string(),
                shortcut: "/dance".to_string(),
                image_data: None,
                image_url: Some("emoticons/dance.png".to_string()),
                category: EmoticonCategory::Action,
                is_custom: false,
            },
        ];

        for emoticon in default_emoticons {
            let shortcut = emoticon.shortcut.clone();
            let name = emoticon.name.clone();

            self.emoticons.insert(name.clone(), emoticon);
            self.shortcuts.insert(shortcut, name);
        }
    }

    /// Add custom emoticon
    pub async fn add_emoticon(&mut self, emoticon: Emoticon) -> Result<(), String> {
        let name = emoticon.name.clone();
        let shortcut = emoticon.shortcut.clone();

        if self.emoticons.contains_key(&name) {
            return Err(format!("Emoticon '{}' already exists", name));
        }

        self.emoticons.insert(name.clone(), emoticon);
        self.shortcuts.insert(shortcut, name);

        Ok(())
    }

    /// Remove emoticon
    pub async fn remove_emoticon(&mut self, name: &str) -> bool {
        if let Some(emoticon) = self.emoticons.remove(name) {
            self.shortcuts.remove(&emoticon.shortcut);
            return true;
        }

        false
    }

    /// Get emoticon by name
    pub async fn get_emoticon(&self, name: &str) -> Option<Emoticon> {
        self.emoticons.get(name).cloned()
    }

    /// Get emoticon by shortcut
    pub async fn get_by_shortcut(&self, shortcut: &str) -> Option<Emoticon> {
        let name = self.shortcuts.get(shortcut).cloned();
        if let Some(name) = name {
            return self.get_emoticon(&name).await;
        }
        None
    }

    /// Process message and replace emoticon shortcuts with formatted text
    pub async fn process_message(&self, message: &str) -> String {
        let mut processed = message.to_string();

        // Sort shortcuts by length (longest first) to avoid partial matches
        let mut sorted_shortcuts: Vec<_> = self.shortcuts.iter().collect();
        sorted_shortcuts.sort_by_key(|(k, _)| std::cmp::Reverse(k.len()));

        for (shortcut, name) in sorted_shortcuts {
            // Simple text replacement
            // In production, you'd use rich text formatting or HTML
            let replacement = format!("[emoticon:{}]", name);
            processed = processed.replace(shortcut, &replacement);
        }

        processed
    }

    /// Get all emoticons
    pub async fn get_all_emoticons(&self) -> Vec<Emoticon> {
        self.emoticons.values().cloned().collect()
    }

    /// Get emoticons by category
    pub async fn get_by_category(&self, category: EmoticonCategory) -> Vec<Emoticon> {
        self.emoticons
            .values()
            .filter(|e| e.category == category)
            .cloned()
            .collect()
    }

    /// Get custom emoticons
    pub async fn get_custom_emoticons(&self) -> Vec<Emoticon> {
        self.emoticons
            .values()
            .filter(|e| e.is_custom)
            .cloned()
            .collect()
    }

    /// Search emoticons by name
    pub async fn search(&self, query: &str) -> Vec<Emoticon> {
        let query_lower = query.to_lowercase();

        self.emoticons
            .values()
            .filter(|e| e.name.to_lowercase().contains(&query_lower) ||
                       e.shortcut.to_lowercase().contains(&query_lower))
            .cloned()
            .collect()
    }

    /// Export emoticons to JSON
    pub async fn export_emoticons(&self) -> Result<String, String> {
        let custom: Vec<_> = self.emoticons
            .values()
            .filter(|e| e.is_custom)
            .collect();

        serde_json::to_string_pretty(&custom)
            .map_err(|e| format!("Failed to export: {}", e))
    }

    /// Import emoticons from JSON
    pub async fn import_emoticons(&mut self, json: &str) -> Result<usize, String> {
        let imported: Vec<Emoticon> = serde_json::from_str(json)
            .map_err(|e| format!("Failed to import: {}", e))?;

        let mut count = 0;
        for emoticon in imported {
            if self.add_emoticon(emoticon).await.is_ok() {
                count += 1;
            }
        }

        Ok(count)
    }

    /// Clear all custom emoticons
    pub async fn clear_custom(&mut self) {
        self.emoticons.retain(|_, e| !e.is_custom);
        self.shortcuts.retain(|_, name| {
            self.emoticons.get(name)
                .map(|e| !e.is_custom)
                .unwrap_or(false)
        });
    }
}

impl Default for EmoticonManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_emoticon_manager_creation() {
        let mut manager = EmoticonManager::new();
        let all = manager.get_all_emoticons().await;
        assert!(!all.is_empty()); // Should have default emoticons
    }

    #[tokio::test]
    async fn test_get_by_shortcut() {
        let mut manager = EmoticonManager::new();
        let emoticon = manager.get_by_shortcut(":)").await;
        assert!(emoticon.is_some());
        assert_eq!(emoticon.unwrap().name, "smile");
    }

    #[tokio::test]
    async fn test_add_custom_emoticon() {
        let mut manager = EmoticonManager::new();

        let custom = Emoticon {
            name: "test".to_string(),
            shortcut: ":test:".to_string(),
            image_data: None,
            image_url: None,
            category: EmoticonCategory::Custom,
            is_custom: true,
        };

        assert!(manager.add_emoticon(custom).await.is_ok());
        assert!(manager.get_emoticon("test").await.is_some());
    }

    #[tokio::test]
    async fn test_remove_emoticon() {
        let mut manager = EmoticonManager::new();

        let custom = Emoticon {
            name: "toremove".to_string(),
            shortcut: ":remove:".to_string(),
            image_data: None,
            image_url: None,
            category: EmoticonCategory::Custom,
            is_custom: true,
        };

        manager.add_emoticon(custom).await.unwrap();
        assert!(manager.remove_emoticon("toremove").await);
        assert!(!manager.remove_emoticon("nonexistent").await);
    }

    #[tokio::test]
    async fn test_process_message() {
        let mut manager = EmoticonManager::new();

        let processed = manager.process_message("Hello :) world").await;
        assert!(processed.contains("[emoticon:smile]"));
    }

    #[tokio::test]
    async fn test_get_by_category() {
        let mut manager = EmoticonManager::new();

        let standard = manager.get_by_category(EmoticonCategory::Standard).await;
        assert!(!standard.is_empty());

        let actions = manager.get_by_category(EmoticonCategory::Action).await;
        assert!(!actions.is_empty());
    }

    #[tokio::test]
    async fn test_search() {
        let mut manager = EmoticonManager::new();

        let results = manager.search("smile").await;
        assert!(!results.is_empty());

        let results = manager.search(":)").await;
        assert!(!results.is_empty());
    }
}
