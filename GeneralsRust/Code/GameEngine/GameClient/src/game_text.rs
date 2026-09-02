//! GameText lookup helper.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};

use game_engine::common::language::{Language, LanguageId, get_current_language};

const CSF_ID: u32 = u32::from_le_bytes(*b" FSC");
const CSF_LABEL: u32 = u32::from_le_bytes(*b" LBL");
const CSF_STRING: u32 = u32::from_le_bytes(*b" RTS");
const CSF_STRING_WITH_WAVE: u32 = u32::from_le_bytes(*b"WRTS");

#[derive(Debug, Default)]
pub struct GameText {
    map_strings: HashMap<String, String>,
    csf_strings: HashMap<String, String>,
    /// Lowercase-key mirrors of the tables above: C++ `compareLUT` sorts and
    /// bsearches with `stricmp` (GameText.cpp:1373-1379), so label lookup is
    /// case-insensitive. Rebuilt whenever the primary tables are replaced.
    map_strings_lower: HashMap<String, String>,
    csf_strings_lower: HashMap<String, String>,
    no_string_list: HashMap<String, String>,
}

impl GameText {
    pub fn fetch(key: &str) -> String {
        Self::fetch_with_exists(key).0
    }

    /// C++ `GameTextManager::fetch(label, exists)` — missing labels become
    /// `MISSING: 'key'` and are cached on the no-string list.
    pub fn fetch_with_exists(key: &str) -> (String, bool) {
        let key = key.trim();
        if key.is_empty() {
            return (String::new(), false);
        }
        if let Some(text) = Self::lookup_string(key) {
            return (text, true);
        }
        let missing = format!("MISSING: '{key}'");
        let mut guard = get_game_text().write().unwrap_or_else(|e| e.into_inner());
        guard
            .no_string_list
            .entry(key.to_string())
            .or_insert_with(|| missing.clone());
        drop(guard);
        (missing, false)
    }

    /// C++ `GameTextManager::getStringsWithLabelPrefix`.
    pub fn get_strings_with_label_prefix(prefix: &str) -> Vec<String> {
        let Ok(guard) = get_game_text().read() else {
            return Vec::new();
        };
        let mut labels: Vec<String> = guard
            .map_strings
            .keys()
            .chain(guard.csf_strings.keys())
            .filter(|label| label.starts_with(prefix))
            .cloned()
            .collect();
        labels.sort();
        labels.dedup();
        labels
    }

    pub fn init_map_string_file(path: &str) -> Result<(), Box<dyn std::error::Error>> {
        let mut guard = get_game_text().write().unwrap_or_else(|e| e.into_inner());
        guard.map_strings.clear();
        let content = fs::read_to_string(path)?;
        parse_string_file(&content, &mut guard.map_strings);
        guard.map_strings_lower = guard
            .map_strings
            .iter()
            .map(|(label, value)| (label.to_ascii_lowercase(), value.clone()))
            .collect();
        Ok(())
    }

    pub fn init_runtime_strings() -> Result<usize, Box<dyn std::error::Error>> {
        // C++ `Language::init` reads `Data\<lang>\generals.csf` through the
        // archive filesystem. Resolve the loose extraction first, then the
        // virtual/BIG filesystem, then a raw ` FSC` header scan over the
        // install archives (the shipped repacked BIGs store the CSF at an
        // offset their entry table does not point at).
        let entries = find_csf_path()
            .and_then(|path| fs::read(&path).ok())
            .and_then(|bytes| parse_csf_strings(&bytes))
            .or_else(load_csf_through_engine_filesystem)
            .or_else(parse_csf_from_install_archives);
        let Some(entries) = entries else {
            return Ok(0);
        };
        Language::clear_localized_strings();
        let lower: HashMap<String, String> = entries
            .iter()
            .map(|(label, value)| (label.to_ascii_lowercase(), value.clone()))
            .collect();
        {
            let mut guard = get_game_text().write().unwrap_or_else(|e| e.into_inner());
            guard.csf_strings = entries.clone();
            guard.csf_strings_lower = lower;
            guard.no_string_list.clear();
        }
        for (label, value) in &entries {
            Language::register_localized_string(label.clone(), value.clone());
        }
        Ok(entries.len())
    }

    pub fn reset() {
        let mut guard = get_game_text().write().unwrap_or_else(|e| e.into_inner());
        guard.map_strings.clear();
        guard.map_strings_lower.clear();
        guard.no_string_list.clear();
    }

    fn lookup_string(key: &str) -> Option<String> {
        let lookup = key.strip_prefix("LOC:").unwrap_or(key);
        if let Ok(guard) = get_game_text().read() {
            if let Some(text) = guard
                .map_strings
                .get(key)
                .or_else(|| guard.map_strings.get(lookup))
            {
                return Some(text.clone());
            }
            if let Some(text) = guard
                .csf_strings
                .get(key)
                .or_else(|| guard.csf_strings.get(lookup))
            {
                return Some(text.clone());
            }
            // C++ bsearch + stricmp: case-insensitive label match.
            let lower = lookup.to_ascii_lowercase();
            if let Some(text) = guard
                .map_strings_lower
                .get(&lower)
                .or_else(|| guard.csf_strings_lower.get(&lower))
            {
                return Some(text.clone());
            }
        }
        let localized = Language::get_localized_string(key);
        if localized != lookup && localized != key {
            Some(localized)
        } else {
            None
        }
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

/// C++-parity attempt: read `Data/<lang>/generals.csf` through the engine
/// virtual filesystem (loose dirs, then BIG archives).
fn load_csf_through_engine_filesystem() -> Option<HashMap<String, String>> {
    let language = match get_current_language() {
        LanguageId::German => "german",
        LanguageId::French => "french",
        LanguageId::Spanish => "spanish",
        LanguageId::Italian => "italian",
        _ => "english",
    };
    let virtual_name = format!("Data/{language}/generals.csf");
    let file_system = game_engine::common::system::file_system::get_file_system();
    let mut fs_guard = file_system.lock().ok()?;
    let mut file = fs_guard.open_file(
        &virtual_name,
        game_engine::common::system::file::FileAccess::READ
            .combine(game_engine::common::system::file::FileAccess::BINARY),
    )?;
    let bytes = file.read_entire_and_close().ok()?;
    parse_csf_strings(&bytes)
}

/// The shipped repacked `EnglishZH.big` / `W3DEnglishZH.big` carry an intact
/// `generals.csf` whose entry-table offsets point elsewhere; the real image
/// still starts with the ` FSC` magic, so scan candidates and keep the
/// richest parse.
fn parse_csf_from_install_archives() -> Option<HashMap<String, String>> {
    let mut archives: Vec<PathBuf> = Vec::new();
    for root in game_engine::common::system::install_layout::zh_install_roots() {
        let Ok(entries) = fs::read_dir(&root) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|ext| ext.to_str())
                .is_some_and(|ext| ext.eq_ignore_ascii_case("big"))
            {
                archives.push(path);
            }
        }
    }

    // Language/patch archives first — the audio archives are huge and never
    // carry the string table.
    archives.sort_by_key(|path| {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        (
            !(name.contains("English") || name.contains("Patch")),
            name.len(),
        )
    });

    let mut best: Option<HashMap<String, String>> = None;
    for path in archives {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        let mut start = 0usize;
        let mut attempts = 0usize;
        while attempts < 32
            && let Some(offset) = bytes[start..]
                .windows(4)
                .position(|window| window == CSF_ID.to_le_bytes())
        {
            let absolute = start + offset;
            start = absolute + 1;
            attempts += 1;
            if let Some(entries) = parse_csf_strings(&bytes[absolute..])
                && best.as_ref().is_none_or(|best| entries.len() > best.len())
            {
                log::info!(
                    "GameText: recovered {} CSF labels from {} at offset {}",
                    entries.len(),
                    path.display(),
                    absolute
                );
                best = Some(entries);
            }
        }
        if best.as_ref().is_some_and(|entries| entries.len() > 512) {
            break;
        }
    }
    best
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

    #[test]
    fn missing_labels_use_cpp_missing_prefix() {
        GameText::reset();
        let (text, exists) = GameText::fetch_with_exists("DefinitelyNotARealLabel");
        assert!(!exists);
        assert_eq!(text, "MISSING: 'DefinitelyNotARealLabel'");
        assert!(GameText::get_strings_with_label_prefix("Definitely").is_empty());
    }
}
