use super::{ConfigError, ConfigSection, ConfigValue, Configuration, LoadMode, LoadResult};
use anyhow::Result;
use log::{debug, warn};
use std::collections::HashMap;
use std::fs::File;
use std::path::Path;

/// INI file parser
pub struct IniParser {
    /// Current configuration data
    config: Configuration,
    /// Case sensitive keys
    case_sensitive: bool,
    /// Allow duplicate sections
    allow_duplicate_sections: bool,
}

impl IniParser {
    /// Create new INI parser
    pub fn new() -> Self {
        Self {
            config: HashMap::new(),
            case_sensitive: false, // C&C INI files are typically case-insensitive
            allow_duplicate_sections: true, // C&C allows duplicate sections
        }
    }

    pub fn set_allow_duplicate_sections(&mut self, allow: bool) {
        self.allow_duplicate_sections = allow;
    }

    /// Set case sensitivity
    pub fn set_case_sensitive(&mut self, case_sensitive: bool) {
        self.case_sensitive = case_sensitive;
    }

    /// Load INI file
    pub fn load_file<P: AsRef<Path>>(&mut self, path: P, mode: LoadMode) -> Result<LoadResult> {
        let path = path.as_ref();
        debug!("Loading INI file: {:?} (mode: {:?})", path, mode);

        if !path.exists() {
            return Err(ConfigError::FileNotFound(path.to_string_lossy().to_string()).into());
        }

        let _file = File::open(path)?;
        let content = std::fs::read_to_string(path)?;

        self.parse_content(&content, mode)
    }

    /// Load INI from string content
    pub fn load_from_string(&mut self, content: &str, mode: LoadMode) -> Result<LoadResult> {
        self.parse_content(content, mode)
    }

    fn normalize_section_name(&self, section: &str) -> String {
        if self.case_sensitive {
            section.to_string()
        } else {
            section.to_lowercase()
        }
    }

    fn looks_like_block_section_header(line: &str) -> bool {
        if line.is_empty() {
            return false;
        }
        line.chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
    }

    fn strip_inline_comment(value: &str) -> &str {
        let mut in_single = false;
        let mut in_double = false;
        for (index, ch) in value.char_indices() {
            match ch {
                '\'' if !in_double => in_single = !in_single,
                '"' if !in_single => in_double = !in_double,
                ';' | '#' if !in_single && !in_double => return &value[..index],
                _ => {}
            }
        }
        value
    }

    /// Parse INI content
    fn parse_content(&mut self, content: &str, mode: LoadMode) -> Result<LoadResult> {
        let mut result = LoadResult {
            sections_loaded: 0,
            keys_loaded: 0,
            warnings: Vec::new(),
            errors: Vec::new(),
        };

        let mut current_section = String::new();
        let mut section_data = ConfigSection::new();
        let mut line_number = 0;

        for line in content.lines() {
            line_number += 1;
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with(';') || line.starts_with('#') {
                continue;
            }

            // Handle section headers
            if line.starts_with('[') && line.ends_with(']') {
                // Save previous section if it exists
                if !current_section.is_empty() {
                    self.merge_section(&current_section, section_data, mode, &mut result);
                    section_data = ConfigSection::new();
                }

                // Parse new section name
                current_section = self.normalize_section_name(line[1..line.len() - 1].trim());

                debug!("Found section: [{}]", current_section);
                continue;
            }

            // Handle classic C&C-style block sections:
            //   GameData
            //     Key = Value
            //   End
            if line.eq_ignore_ascii_case("End") {
                if !current_section.is_empty() {
                    self.merge_section(&current_section, section_data, mode, &mut result);
                    section_data = ConfigSection::new();
                    current_section.clear();
                }
                continue;
            }

            if !line.contains('=') && Self::looks_like_block_section_header(line) {
                if !current_section.is_empty() {
                    self.merge_section(&current_section, section_data, mode, &mut result);
                    section_data = ConfigSection::new();
                }
                current_section = self.normalize_section_name(line);
                debug!("Found block section: [{}]", current_section);
                continue;
            }

            // Handle key-value pairs
            if let Some(eq_pos) = line.find('=') {
                let key = line[..eq_pos].trim().to_string();
                let value_str = Self::strip_inline_comment(&line[eq_pos + 1..]).trim();

                let key_normalized = if self.case_sensitive {
                    key.clone()
                } else {
                    key.to_lowercase()
                };

                match self.parse_value(value_str) {
                    Ok(value) => {
                        section_data.insert(key_normalized, value);
                        result.keys_loaded += 1;
                        debug!("  {} = {:?}", key, value_str);
                    }
                    Err(e) => {
                        let error_msg = format!(
                            "Line {}: Failed to parse value for key '{}': {}",
                            line_number, key, e
                        );
                        result.errors.push(error_msg.clone());
                        warn!("{}", error_msg);
                    }
                }
            } else {
                let warning_msg = format!(
                    "Line {}: Invalid line format (no '=' found): {}",
                    line_number, line
                );
                result.warnings.push(warning_msg.clone());
                warn!("{}", warning_msg);
            }
        }

        // Save final section
        if !current_section.is_empty() {
            self.merge_section(&current_section, section_data, mode, &mut result);
        }

        debug!(
            "INI parsing complete: {} sections, {} keys loaded",
            result.sections_loaded, result.keys_loaded
        );
        Ok(result)
    }

    /// Merge section into configuration
    fn merge_section(
        &mut self,
        section_name: &str,
        section_data: ConfigSection,
        mode: LoadMode,
        result: &mut LoadResult,
    ) {
        let section_name = section_name.to_string();

        if !self.allow_duplicate_sections && self.config.contains_key(&section_name) {
            let warning = format!("Duplicate section '{}' ignored", section_name);
            result.warnings.push(warning.clone());
            warn!("{}", warning);
            return;
        }

        match mode {
            LoadMode::Overwrite => {
                // Replace entire section
                if !section_data.is_empty() {
                    self.config.insert(section_name, section_data);
                    result.sections_loaded += 1;
                }
            }
            LoadMode::NoOverwrite => {
                // Only add if section doesn't exist
                if !self.config.contains_key(&section_name) && !section_data.is_empty() {
                    self.config.insert(section_name, section_data);
                    result.sections_loaded += 1;
                }
            }
            LoadMode::MultiFile => {
                // Merge with existing section
                if section_data.is_empty() {
                    return;
                }

                if let Some(existing_section) = self.config.get_mut(&section_name) {
                    // Merge keys
                    for (key, value) in section_data {
                        existing_section.insert(key, value);
                    }
                } else {
                    // Create new section
                    self.config.insert(section_name, section_data);
                    result.sections_loaded += 1;
                }
            }
        }
    }

    /// Parse value string into ConfigValue
    fn parse_value(&self, value_str: &str) -> Result<ConfigValue> {
        let value_str = value_str.trim();

        // Remove quotes if present
        let value_str = if (value_str.starts_with('"') && value_str.ends_with('"'))
            || (value_str.starts_with('\'') && value_str.ends_with('\''))
        {
            &value_str[1..value_str.len() - 1]
        } else {
            value_str
        };

        // Try parsing as different types

        // Boolean
        match value_str.to_lowercase().as_str() {
            "true" | "yes" | "on" => return Ok(ConfigValue::Boolean(true)),
            "false" | "no" | "off" => return Ok(ConfigValue::Boolean(false)),
            _ => {}
        }

        // Integer
        if let Ok(int_val) = value_str.parse::<i32>() {
            return Ok(ConfigValue::Integer(int_val));
        }

        // Float
        if let Ok(float_val) = value_str.parse::<f32>() {
            return Ok(ConfigValue::Float(float_val));
        }

        // Color (R:G:B:A format)
        if let Some(color) = self.parse_color(value_str) {
            return Ok(color);
        }

        // Vector3 (X Y Z format)
        if let Some(vector) = self.parse_vector3(value_str) {
            return Ok(vector);
        }

        // List (space or comma separated)
        if value_str.contains(' ') || value_str.contains(',') {
            let list = self.parse_list(value_str);
            if list.len() > 1 {
                return Ok(ConfigValue::List(list));
            }
        }

        // Default to string
        Ok(ConfigValue::String(value_str.to_string()))
    }

    /// Parse color value (R:G:B:A or R G B A format)
    fn parse_color(&self, value_str: &str) -> Option<ConfigValue> {
        let parts: Vec<&str> = if value_str.contains(':') {
            value_str.split(':').collect()
        } else {
            value_str.split_whitespace().collect()
        };

        if parts.len() == 3 || parts.len() == 4 {
            if let (Ok(r), Ok(g), Ok(b)) = (
                parts[0].parse::<u8>(),
                parts[1].parse::<u8>(),
                parts[2].parse::<u8>(),
            ) {
                let a = if parts.len() == 4 {
                    parts[3].parse::<u8>().unwrap_or(255)
                } else {
                    255
                };
                return Some(ConfigValue::Color(r, g, b, a));
            }
        }

        None
    }

    /// Parse Vector3 value (X Y Z format)
    fn parse_vector3(&self, value_str: &str) -> Option<ConfigValue> {
        let parts: Vec<&str> = value_str.split_whitespace().collect();

        if parts.len() == 3 {
            if let (Ok(x), Ok(y), Ok(z)) = (
                parts[0].parse::<f32>(),
                parts[1].parse::<f32>(),
                parts[2].parse::<f32>(),
            ) {
                return Some(ConfigValue::Vector3(x, y, z));
            }
        }

        None
    }

    /// Parse list value
    fn parse_list(&self, value_str: &str) -> Vec<String> {
        let mut items = Vec::new();

        // Try comma-separated first
        if value_str.contains(',') {
            for item in value_str.split(',') {
                let item = item.trim();
                if !item.is_empty() {
                    items.push(item.to_string());
                }
            }
        } else {
            // Space-separated
            for item in value_str.split_whitespace() {
                items.push(item.to_string());
            }
        }

        items
    }

    /// Get configuration data
    pub fn get_config(&self) -> &Configuration {
        &self.config
    }

    /// Get mutable configuration data
    pub fn get_config_mut(&mut self) -> &mut Configuration {
        &mut self.config
    }

    /// Get value from configuration
    pub fn get_value(&self, section: &str, key: &str) -> Option<&ConfigValue> {
        let section = if self.case_sensitive {
            section.to_string()
        } else {
            section.to_lowercase()
        };

        let key = if self.case_sensitive {
            key.to_string()
        } else {
            key.to_lowercase()
        };

        self.config.get(&section)?.get(&key)
    }

    /// Get string value
    pub fn get_string(&self, section: &str, key: &str, default: Option<&str>) -> String {
        match self.get_value(section, key) {
            Some(ConfigValue::String(s)) => s.clone(),
            Some(ConfigValue::List(items)) => items.join(" "),
            Some(ConfigValue::Integer(value)) => value.to_string(),
            Some(ConfigValue::Float(value)) => {
                if value.is_finite() && value.fract() == 0.0 {
                    format!("{value:.1}")
                } else {
                    value.to_string()
                }
            }
            Some(ConfigValue::Boolean(value)) => value.to_string(),
            Some(ConfigValue::Vector3(x, y, z)) => format!("{x} {y} {z}"),
            Some(ConfigValue::Color(r, g, b, a)) => format!("{r} {g} {b} {a}"),
            None => default.unwrap_or("").to_string(),
        }
    }

    /// Get integer value
    pub fn get_int(&self, section: &str, key: &str, default: i32) -> i32 {
        match self.get_value(section, key) {
            Some(ConfigValue::Integer(i)) => *i,
            Some(ConfigValue::String(s)) => s.parse().unwrap_or(default),
            _ => default,
        }
    }

    /// Get float value
    pub fn get_float(&self, section: &str, key: &str, default: f32) -> f32 {
        match self.get_value(section, key) {
            Some(ConfigValue::Float(f)) => *f,
            Some(ConfigValue::String(s)) => s.parse().unwrap_or(default),
            _ => default,
        }
    }

    /// Get boolean value
    pub fn get_bool(&self, section: &str, key: &str, default: bool) -> bool {
        match self.get_value(section, key) {
            Some(ConfigValue::Boolean(b)) => *b,
            Some(ConfigValue::Integer(value)) => match *value {
                0 => false,
                1 => true,
                _ => default,
            },
            Some(ConfigValue::Float(value)) => match value {
                v if v.fract() == 0.0 && *v >= 0.0 => match *v as i32 {
                    0 => false,
                    1 => true,
                    _ => default,
                },
                _ => default,
            },
            Some(ConfigValue::String(s)) => match s.to_lowercase().as_str() {
                "true" | "yes" | "on" | "1" => true,
                "false" | "no" | "off" | "0" => false,
                _ => default,
            },
            _ => default,
        }
    }

    /// Check if section exists
    pub fn has_section(&self, section: &str) -> bool {
        let section = if self.case_sensitive {
            section.to_string()
        } else {
            section.to_lowercase()
        };
        self.config.contains_key(&section)
    }

    /// Check if key exists in section
    pub fn has_key(&self, section: &str, key: &str) -> bool {
        self.get_value(section, key).is_some()
    }

    /// Get all section names
    pub fn get_sections(&self) -> Vec<String> {
        self.config.keys().cloned().collect()
    }

    /// Get all keys in a section
    pub fn get_keys(&self, section: &str) -> Vec<String> {
        let section = if self.case_sensitive {
            section.to_string()
        } else {
            section.to_lowercase()
        };

        match self.config.get(&section) {
            Some(section_data) => section_data.keys().cloned().collect(),
            None => Vec::new(),
        }
    }

    /// Clear all configuration data
    pub fn clear(&mut self) {
        self.config.clear();
    }

    /// Get statistics
    pub fn get_stats(&self) -> IniStats {
        let total_keys: usize = self.config.values().map(|section| section.len()).sum();

        IniStats {
            sections: self.config.len(),
            total_keys,
        }
    }
}

/// INI statistics
#[derive(Debug)]
pub struct IniStats {
    pub sections: usize,
    pub total_keys: usize,
}

/// INI section (alias for easier usage)
pub type IniSection = ConfigSection;

/// INI value (alias for easier usage)
pub type IniValue = ConfigValue;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ini_parsing() {
        let ini_content = r#"
; Test INI file
[General]
Name = Test Game
Version = 1.0
Debug = true
MaxPlayers = 8

[Graphics]
Width = 1920
Height = 1080
Fullscreen = yes
AntiAliasing = 4x

[Audio]
MasterVolume = 0.8
MusicVolume = 0.6
SfxVolume = 0.7

[Colors]
BackgroundColor = 64:128:255:255
TextColor = 255 255 255

[Positions]
StartPos = 100.0 200.0 0.0
"#;

        let mut parser = IniParser::new();
        let result = parser
            .load_from_string(ini_content, LoadMode::Overwrite)
            .unwrap();

        assert!(result.sections_loaded > 0);
        assert!(result.keys_loaded > 0);
        assert_eq!(result.errors.len(), 0);

        // Test string values
        assert_eq!(parser.get_string("General", "Name", None), "Test Game");
        assert_eq!(parser.get_string("General", "Version", None), "1.0");

        // Test integer values
        assert_eq!(parser.get_int("General", "MaxPlayers", 0), 8);
        assert_eq!(parser.get_int("Graphics", "Width", 0), 1920);

        // Test boolean values
        assert!(parser.get_bool("General", "Debug", false));
        assert!(parser.get_bool("Graphics", "Fullscreen", false));

        // Test float values
        assert_eq!(parser.get_float("Audio", "MasterVolume", 0.0), 0.8);

        // Test color parsing
        if let Some(ConfigValue::Color(r, g, b, a)) = parser.get_value("Colors", "BackgroundColor")
        {
            assert_eq!(*r, 64);
            assert_eq!(*g, 128);
            assert_eq!(*b, 255);
            assert_eq!(*a, 255);
        } else {
            panic!("Color not parsed correctly");
        }

        // Test vector3 parsing
        if let Some(ConfigValue::Vector3(x, y, z)) = parser.get_value("Positions", "StartPos") {
            assert_eq!(*x, 100.0);
            assert_eq!(*y, 200.0);
            assert_eq!(*z, 0.0);
        } else {
            panic!("Vector3 not parsed correctly");
        }
    }

    #[test]
    fn test_case_sensitivity() {
        let ini_content = r#"
[Section]
Key = Value
"#;

        let mut parser = IniParser::new();
        parser.set_case_sensitive(false);
        parser
            .load_from_string(ini_content, LoadMode::Overwrite)
            .unwrap();

        assert_eq!(parser.get_string("section", "key", None), "Value");
        assert_eq!(parser.get_string("SECTION", "KEY", None), "Value");

        parser.set_case_sensitive(true);
        parser.clear();
        parser
            .load_from_string(ini_content, LoadMode::Overwrite)
            .unwrap();

        assert_eq!(parser.get_string("Section", "Key", None), "Value");
        assert_eq!(parser.get_string("section", "key", None), ""); // Case sensitive, should not match
    }

    #[test]
    fn test_cnc_block_sections_and_inline_comments() {
        let ini_content = r#"
GameData
  UseFPSLimit = Yes
  FramesPerSecondLimit = 30 ; target logic FPS
  TextureReductionFactor = 0; 1 is half res
  ShellMapName = "Maps\ShellMapMD\ShellMapMD.map"
End
"#;

        let mut parser = IniParser::new();
        let result = parser
            .load_from_string(ini_content, LoadMode::Overwrite)
            .unwrap();

        assert_eq!(result.errors.len(), 0);
        assert!(parser.has_section("GameData"));
        assert!(parser.get_bool("GameData", "UseFPSLimit", false));
        assert_eq!(parser.get_int("GameData", "FramesPerSecondLimit", 0), 30);
        assert_eq!(
            parser.get_string("GameData", "TextureReductionFactor", None),
            "0"
        );
        assert_eq!(
            parser.get_string("GameData", "ShellMapName", None),
            "Maps\\ShellMapMD\\ShellMapMD.map"
        );
    }
}
