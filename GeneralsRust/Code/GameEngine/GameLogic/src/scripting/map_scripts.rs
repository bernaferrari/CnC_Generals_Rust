//! Map Script Loading and Integration
//!
//! Loads scripts embedded in map files and integrates them with the script engine.
//! Generals Zero Hour maps contain embedded scripts in an INI-like format that
//! define victory conditions, objectives, and scripted events.
//!
//! # Map File Format
//!
//! Maps (`.map` files) contain a `[Scripts]` or `[ScriptEngine]` section with
//! script definitions in INI format:
//!
//! ```ini
//! [Scripts]
//! Script InitialScript
//!     ScriptOrCondition OR
//!     Condition ConditionTrue
//!     ScriptAction DisplayText 'Mission objectives loaded'
//!     ScriptActionFalse NoOp
//! End
//!
//! Script VictoryCheck
//!     ScriptOrCondition OR
//!     Condition PlayerAllDestroyed ThePlayer's Enemy
//!     ScriptAction Victory
//! End
//! ```

use super::core::*;
use super::ini_parser::parse_script_from_ini;
use crate::{GameLogicError, GameLogicResult};
use std::path::Path;

/// Map script loader
///
/// Manages loading and parsing of scripts from map files, maintaining
/// references to loaded scripts for execution by the script engine.
pub struct MapScriptLoader {
    /// All loaded script lists from maps
    loaded_scripts: Vec<Box<ScriptList>>,

    /// Current map name
    map_name: String,

    /// Map metadata
    map_metadata: MapMetadata,
}

/// Metadata about the loaded map
#[derive(Debug, Clone, Default)]
pub struct MapMetadata {
    /// Map name
    pub name: String,

    /// Map description
    pub description: String,

    /// Map author
    pub author: String,

    /// Player count
    pub player_count: usize,

    /// Map dimensions
    pub width: u32,
    pub height: u32,
}

impl MapScriptLoader {
    /// Create a new map script loader
    pub fn new() -> Self {
        Self {
            loaded_scripts: Vec::new(),
            map_name: String::new(),
            map_metadata: MapMetadata::default(),
        }
    }

    /// Load scripts from map file
    ///
    /// # Arguments
    /// * `map_path` - Path to the .map file
    ///
    /// # Returns
    /// Parsed script list on success
    ///
    /// # Errors
    /// Returns error if file cannot be read or parsed
    ///
    /// # Example
    /// ```rust
    /// use gamelogic::scripting::map_scripts::MapScriptLoader;
    /// use std::path::Path;
    ///
    /// let mut loader = MapScriptLoader::new();
    /// // let scripts = loader.load_from_map(Path::new("maps/test.map"))?;
    /// ```
    pub fn load_from_map(&mut self, map_path: &Path) -> GameLogicResult<Box<ScriptList>> {
        log::info!("Loading scripts from map: {}", map_path.display());

        // Read map file
        let content = std::fs::read_to_string(map_path)
            .map_err(|e| GameLogicError::IO(format!("Failed to read map file: {}", e)))?;

        // Parse map metadata
        self.parse_map_metadata(&content);

        // Extract script section from map file
        let script_section = self.extract_script_section(&content)?;

        // Parse scripts from INI format
        let script_list = parse_script_from_ini(&script_section)?;

        self.map_name = map_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Unknown")
            .to_string();

        log::info!(
            "Loaded scripts from map '{}': {} script groups",
            self.map_name,
            self.count_script_groups(&script_list)
        );

        self.loaded_scripts.push(script_list.clone());

        Ok(script_list)
    }

    /// Parse map metadata from file content
    ///
    /// Extracts information like map name, description, author, etc.
    fn parse_map_metadata(&mut self, content: &str) {
        // Look for [MapProperties] or [General] section
        let mut in_properties = false;

        for line in content.lines() {
            let trimmed = line.trim();

            if trimmed == "[MapProperties]" || trimmed == "[General]" {
                in_properties = true;
                continue;
            }

            if trimmed.starts_with('[') && in_properties {
                break;
            }

            if in_properties {
                if let Some((key, value)) = trimmed.split_once('=') {
                    let key = key.trim();
                    let value = value.trim();

                    match key {
                        "MapName" | "Name" => self.map_metadata.name = value.to_string(),
                        "Description" => self.map_metadata.description = value.to_string(),
                        "Author" => self.map_metadata.author = value.to_string(),
                        "PlayerCount" | "NumPlayers" => {
                            if let Ok(count) = value.parse() {
                                self.map_metadata.player_count = count;
                            }
                        }
                        "Width" => {
                            if let Ok(width) = value.parse() {
                                self.map_metadata.width = width;
                            }
                        }
                        "Height" => {
                            if let Ok(height) = value.parse() {
                                self.map_metadata.height = height;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        log::debug!("Parsed map metadata: {:?}", self.map_metadata);
    }

    /// Extract script section from map file content
    ///
    /// Looks for [Scripts] or [ScriptEngine] section and extracts all content
    /// until the next section begins.
    fn extract_script_section(&self, content: &str) -> GameLogicResult<String> {
        let mut in_script_section = false;
        let mut script_content = String::new();
        let mut section_found = false;

        for line in content.lines() {
            let trimmed = line.trim();

            // Start of script section
            if trimmed == "[Scripts]" || trimmed == "[ScriptEngine]" {
                in_script_section = true;
                section_found = true;
                script_content.push_str(line);
                script_content.push('\n');
                continue;
            }

            // Start of different section - stop
            if trimmed.starts_with('[') && in_script_section {
                break;
            }

            // Inside script section - collect content
            if in_script_section {
                script_content.push_str(line);
                script_content.push('\n');
            }
        }

        if !section_found {
            log::warn!("No script section found in map file");
            // Return empty script section - some maps may not have scripts
            return Ok("[Scripts]\nEnd\n".to_string());
        }

        if script_content.trim().is_empty() {
            return Err(GameLogicError::Configuration(
                "Script section found but empty".to_string(),
            ));
        }

        log::debug!(
            "Extracted script section: {} bytes, {} lines",
            script_content.len(),
            script_content.lines().count()
        );

        Ok(script_content)
    }

    /// Count script groups in a script list
    fn count_script_groups(&self, script_list: &ScriptList) -> usize {
        let mut count = 0;
        let mut current_group = script_list.get_script_group();

        while let Some(group) = current_group {
            count += 1;
            current_group = group.get_next();
        }

        count
    }

    /// Get all loaded scripts
    pub fn get_scripts(&self) -> &[Box<ScriptList>] {
        &self.loaded_scripts
    }

    /// Get most recently loaded scripts
    pub fn get_current_scripts(&self) -> Option<&Box<ScriptList>> {
        self.loaded_scripts.last()
    }

    /// Get map name
    pub fn get_map_name(&self) -> &str {
        &self.map_name
    }

    /// Get map metadata
    pub fn get_map_metadata(&self) -> &MapMetadata {
        &self.map_metadata
    }

    /// Clear all loaded scripts
    ///
    /// Should be called when changing maps or resetting the game.
    pub fn clear(&mut self) {
        log::debug!(
            "Clearing map script loader ({} script lists loaded)",
            self.loaded_scripts.len()
        );
        self.loaded_scripts.clear();
        self.map_name.clear();
        self.map_metadata = MapMetadata::default();
    }

    /// Reload scripts from current map
    ///
    /// Useful for hot-reloading during development.
    pub fn reload(&mut self) -> GameLogicResult<()> {
        if self.map_name.is_empty() {
            return Err(GameLogicError::Configuration(
                "No map loaded to reload".to_string(),
            ));
        }

        let map_path_string = format!("maps/{}.map", self.map_name);
        let map_path = Path::new(&map_path_string);
        self.clear();
        self.load_from_map(map_path)?;

        Ok(())
    }
}

impl Default for MapScriptLoader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::File;
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn create_test_map_file(content: &str) -> PathBuf {
        let mut path = std::env::temp_dir();
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        path.push(format!(
            "map_scripts_test_{}_{}.map",
            std::process::id(),
            unique
        ));
        let mut file = File::create(&path).unwrap();
        write!(file, "{}", content).unwrap();
        path
    }

    #[test]
    fn test_extract_script_section_basic() {
        let loader = MapScriptLoader::new();

        let content = r#"
[General]
MapName = TestMap

[Scripts]
Script TestScript
    ScriptOrCondition OR
    Condition ConditionTrue
    ScriptAction Victory
End

[Other]
Data = Value
"#;

        let result = loader.extract_script_section(content).unwrap();
        assert!(result.contains("[Scripts]"));
        assert!(result.contains("Script TestScript"));
        assert!(!result.contains("[Other]"));
    }

    #[test]
    fn test_extract_script_section_alternative_name() {
        let loader = MapScriptLoader::new();

        let content = r#"
[ScriptEngine]
Script TestScript
    ScriptOrCondition OR
    Condition ConditionTrue
    ScriptAction Victory
End
"#;

        let result = loader.extract_script_section(content).unwrap();
        assert!(result.contains("[ScriptEngine]"));
        assert!(result.contains("Script TestScript"));
    }

    #[test]
    fn test_extract_script_section_empty() {
        let loader = MapScriptLoader::new();

        let content = r#"
[General]
MapName = TestMap

[Other]
Data = Value
"#;

        let result = loader.extract_script_section(content).unwrap();
        // Should return minimal script section when none found
        assert!(result.contains("[Scripts]"));
    }

    #[test]
    fn test_parse_map_metadata() {
        let mut loader = MapScriptLoader::new();

        let content = r#"
[MapProperties]
MapName = Test Mission
Description = A test map
Author = TestAuthor
PlayerCount = 4
Width = 1024
Height = 768

[Scripts]
End
"#;

        loader.parse_map_metadata(content);

        assert_eq!(loader.map_metadata.name, "Test Mission");
        assert_eq!(loader.map_metadata.description, "A test map");
        assert_eq!(loader.map_metadata.author, "TestAuthor");
        assert_eq!(loader.map_metadata.player_count, 4);
        assert_eq!(loader.map_metadata.width, 1024);
        assert_eq!(loader.map_metadata.height, 768);
    }

    #[test]
    fn test_load_from_map_file() {
        let content = r#"
[MapProperties]
MapName = TestMap

[Scripts]
Script InitScript
    ScriptOrCondition OR
    Condition ConditionTrue
    ScriptAction NoOp
End
"#;

        let map_file = create_test_map_file(content);
        let mut loader = MapScriptLoader::new();

        let result = loader.load_from_map(map_file.as_path());
        assert!(result.is_ok());

        assert_eq!(loader.get_scripts().len(), 1);
        assert!(!loader.get_map_name().is_empty());
    }

    #[test]
    fn test_clear() {
        let content = r#"
[Scripts]
Script TestScript
    ScriptOrCondition OR
    Condition ConditionTrue
    ScriptAction NoOp
End
"#;

        let map_file = create_test_map_file(content);
        let mut loader = MapScriptLoader::new();

        loader.load_from_map(map_file.as_path()).unwrap();
        assert_eq!(loader.get_scripts().len(), 1);

        loader.clear();
        assert_eq!(loader.get_scripts().len(), 0);
        assert!(loader.get_map_name().is_empty());
    }

    #[test]
    fn test_get_current_scripts() {
        let content = r#"
[Scripts]
Script TestScript
    ScriptOrCondition OR
    Condition ConditionTrue
    ScriptAction NoOp
End
"#;

        let map_file = create_test_map_file(content);
        let mut loader = MapScriptLoader::new();

        assert!(loader.get_current_scripts().is_none());

        loader.load_from_map(map_file.as_path()).unwrap();
        assert!(loader.get_current_scripts().is_some());
    }
}
