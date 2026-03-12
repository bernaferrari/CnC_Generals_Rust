use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Parsed `SoundEffects.ini` lookup table (`AudioEvent` -> concrete sound variants).
#[derive(Debug, Clone, Default)]
pub struct SoundEffectsTable {
    events: HashMap<String, Vec<String>>,
}

impl SoundEffectsTable {
    pub fn load_default() -> Option<Self> {
        let candidates: &[PathBuf] = &[
            PathBuf::from("windows_game/extracted_big_files/INIZH/Data/INI/SoundEffects.ini"),
            PathBuf::from("windows_game/extracted_big_files_v2/INIZH/Data/INI/SoundEffects.ini"),
            PathBuf::from("Data/INI/SoundEffects.ini"),
        ];

        for path in candidates {
            if let Ok(text) = std::fs::read_to_string(path) {
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
                if let Some(name) = rest.trim().split_whitespace().next() {
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
