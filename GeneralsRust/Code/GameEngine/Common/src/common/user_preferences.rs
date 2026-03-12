// user_preferences.rs - User preferences system (C++-faithful key/value store)

use crate::common::ini::ini_game_data::get_global_data;
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// User preference types
#[derive(Debug, Clone)]
pub enum PreferenceValue {
    Bool(bool),
    Int(i32),
    Float(f32),
    String(String),
}

/// User preferences manager
#[derive(Debug, Default, Clone)]
pub struct UserPreferences {
    preferences: BTreeMap<String, String>,
    filename: Option<PathBuf>,
}

impl UserPreferences {
    pub fn new() -> Self {
        Self::default()
    }

    fn resolve_path(filename: &str) -> PathBuf {
        if Path::new(filename).is_absolute() {
            return PathBuf::from(filename);
        }
        let base = get_global_data()
            .map(|data| data.read().get_path_user_data().to_string())
            .unwrap_or_default();
        let mut base = if base.is_empty() {
            "UserData/".to_string()
        } else {
            base
        };
        if !base.ends_with('/') && !base.ends_with('\\') {
            base.push('/');
        }
        Path::new(&base).join(filename)
    }

    pub fn load(&mut self, filename: &str) -> bool {
        let path = Self::resolve_path(filename);
        self.filename = Some(path.clone());
        self.preferences.clear();

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(_) => return false,
        };

        for line in BufReader::new(file).lines().flatten() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if key.is_empty() || value.is_empty() {
                    continue;
                }
                self.preferences.insert(key.to_string(), value.to_string());
            }
        }

        true
    }

    pub fn write(&self) -> bool {
        let Some(path) = self.filename.as_ref() else {
            return false;
        };

        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        let mut file = match File::create(path) {
            Ok(file) => file,
            Err(_) => return false,
        };

        for (key, value) in &self.preferences {
            let _ = writeln!(file, "{} = {}", key, value);
        }

        true
    }

    pub fn load_from_file(&mut self, file_path: &str) -> Result<(), std::io::Error> {
        let path = Self::resolve_path(file_path);
        self.filename = Some(path.clone());
        self.preferences.clear();

        let file = match File::open(&path) {
            Ok(file) => file,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(err),
        };

        for line in BufReader::new(file).lines() {
            let line = line?;
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();
                if key.is_empty() || value.is_empty() {
                    continue;
                }
                self.preferences.insert(key.to_string(), value.to_string());
            }
        }
        Ok(())
    }

    pub fn save_to_file(&self) -> Result<(), std::io::Error> {
        let Some(path) = self.filename.as_ref() else {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "No filename configured",
            ));
        };
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = File::create(path)?;
        for (key, value) in &self.preferences {
            writeln!(file, "{} = {}", key, value)?;
        }
        Ok(())
    }

    pub fn set_bool(&mut self, key: &str, value: bool) {
        self.preferences
            .insert(key.to_string(), if value { "1" } else { "0" }.to_string());
    }

    pub fn set_int(&mut self, key: &str, value: i32) {
        self.preferences.insert(key.to_string(), value.to_string());
    }

    pub fn set_float(&mut self, key: &str, value: f32) {
        self.preferences
            .insert(key.to_string(), format!("{}", value));
    }

    pub fn set_string(&mut self, key: &str, value: String) {
        self.preferences.insert(key.to_string(), value);
    }

    pub fn get_bool(&self, key: &str) -> Option<bool> {
        let value = self.get_string(key)?;
        let value = value.to_ascii_lowercase();
        Some(matches!(
            value.as_str(),
            "1" | "t" | "true" | "y" | "yes" | "ok"
        ))
    }

    pub fn get_int(&self, key: &str) -> Option<i32> {
        self.get_string(key)?.parse::<i32>().ok()
    }

    pub fn get_float(&self, key: &str) -> Option<f32> {
        self.get_string(key)?.parse::<f32>().ok()
    }

    pub fn get_string(&self, key: &str) -> Option<&String> {
        self.preferences.get(key)
    }

    pub fn get_string_or(&self, key: &str, default: &str) -> String {
        self.preferences
            .get(key)
            .cloned()
            .unwrap_or_else(|| default.to_string())
    }

    pub fn get_int_or(&self, key: &str, default: i32) -> i32 {
        self.get_int(key).unwrap_or(default)
    }

    pub fn get_bool_or(&self, key: &str, default: bool) -> bool {
        self.get_bool(key).unwrap_or(default)
    }

    pub fn entries(&self) -> impl Iterator<Item = (&String, &String)> {
        self.preferences.iter()
    }

    pub fn clear(&mut self) {
        self.preferences.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_set_values() {
        let mut prefs = UserPreferences::new();
        prefs.set_bool("fullscreen", true);
        prefs.set_int("width", 1920);
        prefs.set_string("player", "Nova".to_string());

        assert_eq!(prefs.get_bool("fullscreen"), Some(true));
        assert_eq!(prefs.get_int("width"), Some(1920));
        assert_eq!(
            prefs.get_string("player").cloned(),
            Some("Nova".to_string())
        );
    }
}
