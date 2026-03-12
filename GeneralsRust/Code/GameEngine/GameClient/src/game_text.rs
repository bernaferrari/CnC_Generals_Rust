//! GameText lookup helper.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use game_engine::common::language::{get_current_language, Language, LanguageId};

const CSF_ID: u32 = u32::from_le_bytes(*b" FSC");
const CSF_LABEL: u32 = u32::from_le_bytes(*b" LBL");
const CSF_STRING: u32 = u32::from_le_bytes(*b" RTS");
const CSF_STRING_WITH_WAVE: u32 = u32::from_le_bytes(*b"WRTS");

#[derive(Debug, Default)]
pub struct GameText {
    map_strings: HashMap<String, String>,
}

impl GameText {
    pub fn fetch(key: &str) -> String {
        let key = key.trim();
        if key.is_empty() {
            return String::new();
        }
        if let Some(text) = Self::lookup_map_string(key) {
            return text;
        }
        Language::get_localized_string(key)
    }

    pub fn init_map_string_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = get_game_text().write().unwrap();
        guard.map_strings.clear();
        let content = fs::read_to_string(path)?;
        parse_string_file(&content, &mut guard.map_strings);
        Ok(())
    }

    pub fn init_runtime_strings() -> Result<usize, Box<dyn std::error::Error>> {
        let Some(path) = find_csf_path() else {
            return Ok(0);
        };
        let bytes = fs::read(&path)?;
        let entries = parse_csf_strings(&bytes)
            .ok_or_else(|| format!("failed to parse CSF string table at {}", path.display()))?;
        Language::clear_localized_strings();
        for (label, value) in &entries {
            Language::register_localized_string(label.clone(), value.clone());
        }
        Ok(entries.len())
    }

    pub fn reset() {
        let mut guard = get_game_text().write().unwrap();
        guard.map_strings.clear();
    }

    fn lookup_map_string(key: &str) -> Option<String> {
        get_game_text()
            .read()
            .ok()
            .and_then(|guard| guard.map_strings.get(key).cloned())
    }
}

static THE_GAME_TEXT: OnceLock<RwLock<GameText>> = OnceLock::new();

pub fn get_game_text() -> &'static RwLock<GameText> {
    THE_GAME_TEXT.get_or_init(|| RwLock::new(GameText::default()))
}

fn parse_string_file(contents: &str, out: &mut HashMap<String, String>) {
    let mut current_key: Option<String> = None;
    let mut current_value = String::new();

    for raw_line in contents.lines() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.eq_ignore_ascii_case("END") {
            if let Some(key) = current_key.take() {
                out.insert(key, current_value.clone());
            }
            current_value.clear();
            continue;
        }
        if line.starts_with('"') {
            let mut value = line.trim_matches('"').to_string();
            value = unescape_string(&value);
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(&value);
            continue;
        }
        current_key = Some(line.to_string());
    }
}

fn unescape_string(value: &str) -> String {
    let mut out = String::new();
    let mut chars = value.chars();
    while let Some(ch) = chars.next() {
        if ch == '\\' {
            match chars.next() {
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('t') => out.push('\t'),
                Some('"') => out.push('"'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(ch);
        }
    }
    out
}

#[derive(Debug)]
struct CsfCursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> CsfCursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn read_u32(&mut self) -> Option<u32> {
        let end = self.offset.checked_add(4)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(u32::from_le_bytes(value.try_into().ok()?))
    }

    fn read_u16(&mut self) -> Option<u16> {
        let end = self.offset.checked_add(2)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(u16::from_le_bytes(value.try_into().ok()?))
    }

    fn read_bytes(&mut self, len: usize) -> Option<&'a [u8]> {
        let end = self.offset.checked_add(len)?;
        let value = self.bytes.get(self.offset..end)?;
        self.offset = end;
        Some(value)
    }
}

fn find_csf_path() -> Option<PathBuf> {
    let language_relatives = match get_current_language() {
        LanguageId::German => vec![
            "windows_game/extracted_big_files/GermanZH/Data/German/generals.csf",
            "windows_game/extracted_big_files_v2/GermanZH/Data/German/generals.csf",
        ],
        LanguageId::French => vec![
            "windows_game/extracted_big_files/FrenchZH/Data/French/generals.csf",
            "windows_game/extracted_big_files_v2/FrenchZH/Data/French/generals.csf",
        ],
        LanguageId::Spanish => vec![
            "windows_game/extracted_big_files/SpanishZH/Data/Spanish/generals.csf",
            "windows_game/extracted_big_files_v2/SpanishZH/Data/Spanish/generals.csf",
        ],
        LanguageId::Italian => vec![
            "windows_game/extracted_big_files/ItalianZH/Data/Italian/generals.csf",
            "windows_game/extracted_big_files_v2/ItalianZH/Data/Italian/generals.csf",
        ],
        _ => vec![
            "windows_game/extracted_big_files/EnglishZH/Data/English/generals.csf",
            "windows_game/extracted_big_files/W3DEnglishZH/Data/English/generals.csf",
            "windows_game/extracted_big_files_v2/EnglishZH/Data/English/generals.csf",
            "windows_game/extracted_big_files_v2/W3DEnglishZH/Data/English/generals.csf",
        ],
    };

    let cwd = std::env::current_dir().ok()?;
    let mut candidates = Vec::new();
    for ancestor in cwd.ancestors() {
        for relative in &language_relatives {
            candidates.push(ancestor.join(relative));
        }
    }
    for relative in &language_relatives {
        candidates.push(Path::new(relative).to_path_buf());
    }

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn parse_csf_strings(bytes: &[u8]) -> Option<HashMap<String, String>> {
    let mut cursor = CsfCursor::new(bytes);
    if cursor.read_u32()? != CSF_ID {
        return None;
    }
    let _version = cursor.read_u32()?;
    let num_labels = cursor.read_u32()? as usize;
    let _num_strings = cursor.read_u32()?;
    let _skip = cursor.read_u32()?;
    let _lang_id = cursor.read_u32()?;

    let mut entries = HashMap::with_capacity(num_labels);
    for _ in 0..num_labels {
        if cursor.read_u32()? != CSF_LABEL {
            return None;
        }
        let num_strings = cursor.read_u32()? as usize;
        let label_len = cursor.read_u32()? as usize;
        let label = String::from_utf8_lossy(cursor.read_bytes(label_len)?).into_owned();

        let mut first_text = None;
        for _ in 0..num_strings {
            let string_id = cursor.read_u32()?;
            if string_id != CSF_STRING && string_id != CSF_STRING_WITH_WAVE {
                return None;
            }

            let text_len = cursor.read_u32()? as usize;
            let mut code_units = Vec::with_capacity(text_len);
            for _ in 0..text_len {
                code_units.push(!cursor.read_u16()?);
            }

            if first_text.is_none() {
                first_text = Some(String::from_utf16_lossy(&code_units).trim().to_string());
            }

            if string_id == CSF_STRING_WITH_WAVE {
                let wave_len = cursor.read_u32()? as usize;
                cursor.read_bytes(wave_len)?;
            }
        }

        if let Some(text) = first_text {
            entries.insert(label, text);
        }
    }

    Some(entries)
}

#[cfg(test)]
mod csf_tests {
    use super::*;

    #[test]
    fn csf_runtime_strings_include_shell_labels() {
        let path = find_csf_path().expect("expected generals.csf in repo assets");
        let bytes = fs::read(path).expect("read generals.csf");
        let entries = parse_csf_strings(&bytes).expect("parse generals.csf");
        assert_eq!(entries.get("GUI:Back").map(String::as_str), Some("BACK"));
        assert_eq!(
            entries.get("GUI:SinglePlayer").map(String::as_str),
            Some("SOLO PLAY")
        );
    }
}
