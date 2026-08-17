use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Parsed `SoundEffects.ini` lookup table (`AudioEvent` -> concrete sound variants).
#[derive(Debug, Clone, Default)]
pub struct SoundEffectsTable {
    events: HashMap<String, Vec<String>>,
}

impl SoundEffectsTable {
    pub fn load_default() -> Option<Self> {
        // C++ AudioManager::init (GameAudio.cpp:192-193) loads Default then
        // Data\\INI\\SoundEffects.ini. Search cwd plus extracted INIZH trees so
        // cargo-test / live-boot cwd still finds the retail file.
        let mut paths = Vec::new();
        if let Some(path) = game_engine::common::system::install_layout::resolve_data_ini_file(
            "Data/INI/SoundEffects.ini",
        ) {
            paths.push(path);
        }
        if let Some(path) = game_engine::common::system::install_layout::resolve_data_ini_file(
            "Data/INI/Default/SoundEffects.ini",
        ) {
            paths.push(path);
        }

        for path in paths {
            if let Ok(text) = std::fs::read_to_string(&path) {
                let table = Self::from_text(&text);
                if !table.events.is_empty() {
                    return Some(table);
                }
            }
        }
        None
    }

    pub fn from_text(text: &str) -> Self {
        let mut table = Self::default();

        let mut current_event: Option<String> = None;
        let mut sounds: Vec<String> = Vec::new();

        for raw_line in text.lines() {
            let line = raw_line.split(';').next().unwrap_or("").trim();
            if line.is_empty() {
                continue;
            }

            if let Some(rest) = line.strip_prefix("AudioEvent") {
                if let Some(name) = rest.split_whitespace().next() {
                    current_event = Some(name.to_string());
                    sounds.clear();
                }
                continue;
            }

            if line.eq_ignore_ascii_case("End") {
                if let Some(event) = current_event.take() {
                    if !sounds.is_empty() {
                        table.events.insert(event, sounds.clone());
                    }
                }
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim();

            match key {
                "Sounds" | "Attack" | "Decay" => {
                    for token in value.split_whitespace() {
                        let token = token.trim();
                        if !token.is_empty() {
                            sounds.push(token.to_string());
                        }
                    }
                }
                _ => {}
            }
        }

        table
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn resolve_sound_path(&self, event_type: &str) -> Option<String> {
        let variants = self.events.get(event_type)?;
        if variants.is_empty() {
            return None;
        }
        let pick = variants[fastrand::usize(..variants.len())].as_str();
        Some(format!("Data/Audio/Sounds/{pick}.wav"))
    }

    pub fn resolve_sound_path_from_ini_path(
        &self,
        event_type: &str,
        ini_path: &Path,
    ) -> Option<String> {
        let _ = ini_path;
        self.resolve_sound_path(event_type)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_default_resolves_retail_guiclick_from_extracted_ini() {
        // C++ AudioManager::init (GameAudio.cpp:192-193) loads SoundEffects.ini.
        // Pre-fix load_default only checked cwd-relative windows_game paths, so
        // cargo-test cwd missed the extracted INIZH file and the gameplay queue
        // dropped every event.
        let table = SoundEffectsTable::load_default()
            .expect("SoundEffects.ini must resolve via install_layout");
        assert!(
            table.resolve_sound_path("GUIClick").is_some(),
            "retail GUIClick must resolve after boot-equivalent load"
        );
    }
}
