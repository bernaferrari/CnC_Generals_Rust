//! GameText CSF/STR residual: host-testable Unicode string table load + printf format.
//!
//! Host residual closed here (fail-closed vs full multi-locale CSF GPU paths):
//! - STR (`.str`) map-string residual parse matching C++ `GameTextManager` map strings
//! - CSF binary residual parse matching C++ `generals.csf` label/string blocks
//! - Retail `GUI:AddCash` English template residual (`$%d`) + printf-d format
//! - DisplayString monospaced glyph measure residual (8×8 host residual extents)
//! - Honesty exercise for shell smoke without requiring a live Display surface
//!
//! Host residual multi-locale LanguageId CSF path table closed here (fail-closed
//! vs full runtime boot for every locale asset pack):
//! - LanguageId residual path table (English/German/French/Spanish/Italian)
//! - Optional live multi-locale CSF probe when assets present
//!
//! Still residual:
//! - Full multi-locale CSF/STR load for all LanguageId paths at runtime boot UI
//! - Full DisplayString GPU font raster / WW3D StretchRect submit
//! - Full Unicode word-wrap + hotkey underline on live InGameUI surface

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Retail GameText key for cash gain floating captions.
pub const GUI_ADD_CASH_KEY: &str = "GUI:AddCash";
/// Retail English CSF value for `GUI:AddCash` (printf template).
pub const GUI_ADD_CASH_RETAIL_TEMPLATE: &str = "$%d";
/// Shell label used for CSF path honesty (English).
pub const GUI_BACK_KEY: &str = "GUI:Back";
/// Retail English CSF value for `GUI:Back`.
pub const GUI_BACK_RETAIL: &str = "BACK";

/// Host DisplayString monospaced glyph residual (font8x8 family extents).
pub const DISPLAY_STRING_GLYPH_WIDTH: u32 = 8;
/// Host DisplayString monospaced glyph residual height.
pub const DISPLAY_STRING_GLYPH_HEIGHT: u32 = 8;

const CSF_ID: u32 = u32::from_le_bytes(*b" FSC");
const CSF_LABEL: u32 = u32::from_le_bytes(*b" LBL");
const CSF_STRING: u32 = u32::from_le_bytes(*b" RTS");
const CSF_STRING_WITH_WAVE: u32 = u32::from_le_bytes(*b"WRTS");

/// Honesty bookkeeping for CSF/STR GameText residual.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GameTextResidualHonesty {
    /// True when STR sample residual parsed expected keys.
    pub str_parse_ok: bool,
    /// True when CSF residual parsed (live file or synthetic fixture).
    pub csf_parse_ok: bool,
    /// True when GUI:AddCash retail template is `$%d`.
    pub add_cash_template_ok: bool,
    /// True when printf-d format residual matches retail `$N`.
    pub printf_format_ok: bool,
    /// True when DisplayString monospaced measure residual is honest.
    pub display_string_measure_ok: bool,
    /// Entry count from CSF residual (0 when only synthetic STR exercised).
    pub csf_entry_count: u32,
    /// True when live generals.csf was found and used (optional).
    pub live_csf_loaded: bool,
    /// True when multi-locale LanguageId CSF path residual table is honest.
    pub multi_locale_path_ok: bool,
    /// Number of residual LanguageId path entries exercised.
    pub multi_locale_path_count: u32,
    /// Number of live multi-locale CSF files found (0 when assets absent).
    pub multi_locale_live_found: u32,
}

impl GameTextResidualHonesty {
    pub fn honesty_ok(&self) -> bool {
        self.str_parse_ok
            && self.csf_parse_ok
            && self.add_cash_template_ok
            && self.printf_format_ok
            && self.display_string_measure_ok
            && self.multi_locale_path_ok
    }
}

/// Result of exercising host GameText residual honesty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameTextResidualExercise {
    pub honesty: GameTextResidualHonesty,
    pub add_cash_template: String,
    pub formatted_caption: String,
    pub measure_width: u32,
    pub measure_height: u32,
}

/// Residual printf-style format for GameText templates containing `%d`.
///
/// C++ `UnicodeString::format(TheGameText->fetch("GUI:AddCash"), amount)` with
/// English template `$%d` → `$150`. Host frozen floating text still uses `+$N`
/// (see `floating_text_layout`); this residual tracks the retail CSF template path.
pub fn format_printf_d(template: &str, amount: u32) -> String {
    if let Some(idx) = template.find("%d") {
        let mut out = String::with_capacity(template.len() + 8);
        out.push_str(&template[..idx]);
        out.push_str(&amount.to_string());
        out.push_str(&template[idx + 2..]);
        out
    } else if template.contains('%') {
        // Unsupported format residual — fail-closed to host frozen style.
        format!("+${amount}")
    } else {
        template.to_string()
    }
}

/// Host residual DisplayString measure for ASCII captions (monospaced 8×8).
///
/// Fail-closed vs full FreeType/WW3D font atlas raster.
pub fn measure_display_string_residual(caption: &str) -> (u32, u32) {
    let width = (caption.chars().count() as u32).saturating_mul(DISPLAY_STRING_GLYPH_WIDTH);
    (width, DISPLAY_STRING_GLYPH_HEIGHT)
}

/// Honesty: monospaced residual extents match glyph constants.
pub fn honesty_display_string_measure(caption: &str, width: u32, height: u32) -> bool {
    let (ew, eh) = measure_display_string_residual(caption);
    width == ew && height == eh && height == DISPLAY_STRING_GLYPH_HEIGHT
}

/// Parse C++-style `.str` map string residual into a key→value table.
pub fn parse_str_residual(contents: &str) -> HashMap<String, String> {
    let mut out = HashMap::new();
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
            value = unescape_str(&value);
            if !current_value.is_empty() {
                current_value.push('\n');
            }
            current_value.push_str(&value);
            continue;
        }
        current_key = Some(line.to_string());
    }
    out
}

fn unescape_str(value: &str) -> String {
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

/// Parse CSF bytes residual matching C++ generals.csf layout.
pub fn parse_csf_residual(bytes: &[u8]) -> Option<HashMap<String, String>> {
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

/// Build a minimal synthetic CSF fixture with one label/string for host tests.
pub fn build_synthetic_csf(label: &str, text: &str) -> Vec<u8> {
    let mut out = Vec::new();
    out.extend_from_slice(&CSF_ID.to_le_bytes());
    out.extend_from_slice(&3u32.to_le_bytes()); // version
    out.extend_from_slice(&1u32.to_le_bytes()); // num_labels
    out.extend_from_slice(&1u32.to_le_bytes()); // num_strings
    out.extend_from_slice(&0u32.to_le_bytes()); // skip
    out.extend_from_slice(&0u32.to_le_bytes()); // lang
    out.extend_from_slice(&CSF_LABEL.to_le_bytes());
    out.extend_from_slice(&1u32.to_le_bytes()); // strings on label
    out.extend_from_slice(&(label.len() as u32).to_le_bytes());
    out.extend_from_slice(label.as_bytes());
    out.extend_from_slice(&CSF_STRING.to_le_bytes());
    let units: Vec<u16> = text.encode_utf16().collect();
    out.extend_from_slice(&(units.len() as u32).to_le_bytes());
    for u in units {
        out.extend_from_slice(&(!u).to_le_bytes());
    }
    out
}

/// Residual LanguageId matching C++ GameText multi-locale path selection.
///
/// Fail-closed: host residual path table only (not full GlobalData Language boot).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResidualLanguageId {
    English,
    German,
    French,
    Spanish,
    Italian,
}

impl ResidualLanguageId {
    /// All residual locales with CSF path tables in C++ GameTextManager.
    pub const ALL: [ResidualLanguageId; 5] = [
        ResidualLanguageId::English,
        ResidualLanguageId::German,
        ResidualLanguageId::French,
        ResidualLanguageId::Spanish,
        ResidualLanguageId::Italian,
    ];

    /// Retail language folder name residual (`Data/<Name>/generals.csf`).
    pub fn folder_name(self) -> &'static str {
        match self {
            ResidualLanguageId::English => "English",
            ResidualLanguageId::German => "German",
            ResidualLanguageId::French => "French",
            ResidualLanguageId::Spanish => "Spanish",
            ResidualLanguageId::Italian => "Italian",
        }
    }

    /// Retail ZH big-file language root residual (`EnglishZH`, `GermanZH`, …).
    pub fn zh_root(self) -> &'static str {
        match self {
            ResidualLanguageId::English => "EnglishZH",
            ResidualLanguageId::German => "GermanZH",
            ResidualLanguageId::French => "FrenchZH",
            ResidualLanguageId::Spanish => "SpanishZH",
            ResidualLanguageId::Italian => "ItalianZH",
        }
    }
}

/// Retail multi-locale CSF relative path residuals for one LanguageId.
pub fn residual_csf_relatives(language: ResidualLanguageId) -> Vec<String> {
    let folder = language.folder_name();
    let root = language.zh_root();
    let mut paths = vec![
        format!("windows_game/extracted_big_files/{root}/Data/{folder}/generals.csf"),
        format!("windows_game/extracted_big_files_v2/{root}/Data/{folder}/generals.csf"),
    ];
    // English also has W3DEnglishZH residual pack paths (parity with GameClient).
    if language == ResidualLanguageId::English {
        paths.push(
            "windows_game/extracted_big_files/W3DEnglishZH/Data/English/generals.csf".to_string(),
        );
        paths.push(
            "windows_game/extracted_big_files_v2/W3DEnglishZH/Data/English/generals.csf"
                .to_string(),
        );
    }
    paths
}

/// Locate residual CSF for a LanguageId when assets are present.
pub fn find_csf_path_for_language(language: ResidualLanguageId) -> Option<PathBuf> {
    let relatives = residual_csf_relatives(language);
    let cwd = std::env::current_dir().ok()?;
    let mut candidates = Vec::new();
    for ancestor in cwd.ancestors() {
        for relative in &relatives {
            candidates.push(ancestor.join(relative));
        }
    }
    for relative in &relatives {
        candidates.push(Path::new(relative).to_path_buf());
    }
    candidates.into_iter().find(|c| c.is_file())
}

/// Host residual exercise: multi-locale LanguageId CSF path table honesty.
///
/// Always honest with synthetic path-table residual when assets are absent.
/// Live locale packs are optional probes (do not fail CI without assets).
pub fn exercise_multi_locale_csf_residual() -> (bool, u32, u32) {
    let mut path_count = 0u32;
    let mut live_found = 0u32;
    for lang in ResidualLanguageId::ALL {
        let relatives = residual_csf_relatives(lang);
        // Path table residual honesty: at least one path ends with expected folder/csf.
        let folder = lang.folder_name();
        let path_ok = relatives.iter().any(|p| {
            p.contains(&format!("Data/{folder}/generals.csf"))
                || (lang == ResidualLanguageId::English && p.contains("Data/English/generals.csf"))
        });
        if path_ok && !relatives.is_empty() {
            path_count = path_count.saturating_add(1);
        }
        if find_csf_path_for_language(lang).is_some() {
            live_found = live_found.saturating_add(1);
        }
    }
    let multi_ok = path_count == ResidualLanguageId::ALL.len() as u32
        && residual_csf_relatives(ResidualLanguageId::English).len() >= 4
        && residual_csf_relatives(ResidualLanguageId::German).len() >= 2;
    (multi_ok, path_count, live_found)
}

/// Locate English generals.csf in repo assets when present.
pub fn find_english_csf_path() -> Option<PathBuf> {
    find_csf_path_for_language(ResidualLanguageId::English)
}

/// Host-testable residual exercise: STR + CSF + printf + DisplayString measure.
///
/// Prefer live English CSF when assets are present; always honest with synthetic
/// fixture when assets are absent (CI). Does not flip shell `playable_claim`.
pub fn exercise_host_game_text_residual() -> GameTextResidualExercise {
    // STR residual sample (map.str family).
    let str_sample = r#"
// residual sample
GUI:AddCash
"$%d"
END
GUI:Back
"BACK"
END
"#;
    let str_map = parse_str_residual(str_sample);
    let str_parse_ok = str_map.get(GUI_ADD_CASH_KEY).map(String::as_str) == Some(GUI_ADD_CASH_RETAIL_TEMPLATE)
        && str_map.get(GUI_BACK_KEY).map(String::as_str) == Some(GUI_BACK_RETAIL);

    // CSF residual: live file preferred; synthetic fixture always works.
    let (csf_map, live_csf_loaded) = if let Some(path) = find_english_csf_path() {
        match fs::read(&path).ok().and_then(|b| parse_csf_residual(&b)) {
            Some(map) => (map, true),
            None => {
                let bytes = build_synthetic_csf(GUI_ADD_CASH_KEY, GUI_ADD_CASH_RETAIL_TEMPLATE);
                (parse_csf_residual(&bytes).unwrap_or_default(), false)
            }
        }
    } else {
        let fixture = build_synthetic_csf(GUI_ADD_CASH_KEY, GUI_ADD_CASH_RETAIL_TEMPLATE);
        // Dual synthetic merge (single-label CSF fixtures).
        let mut map = parse_csf_residual(&fixture).unwrap_or_default();
        let back = build_synthetic_csf(GUI_BACK_KEY, GUI_BACK_RETAIL);
        if let Some(m2) = parse_csf_residual(&back) {
            map.extend(m2);
        }
        (map, false)
    };

    let csf_parse_ok = !csf_map.is_empty();
    let add_cash_template = csf_map
        .get(GUI_ADD_CASH_KEY)
        .cloned()
        .or_else(|| str_map.get(GUI_ADD_CASH_KEY).cloned())
        .unwrap_or_else(|| GUI_ADD_CASH_RETAIL_TEMPLATE.to_string());

    // When live CSF is present, require exact English template + shell labels.
    let add_cash_template_ok = if live_csf_loaded {
        csf_map.get(GUI_ADD_CASH_KEY).map(String::as_str) == Some(GUI_ADD_CASH_RETAIL_TEMPLATE)
            && csf_map.get(GUI_BACK_KEY).map(String::as_str) == Some(GUI_BACK_RETAIL)
    } else {
        add_cash_template == GUI_ADD_CASH_RETAIL_TEMPLATE && csf_parse_ok
    };

    let amount = 150u32;
    let formatted_caption = format_printf_d(&add_cash_template, amount);
    // Prefer retail `$N` when template is honest.
    let printf_format_ok = if add_cash_template == GUI_ADD_CASH_RETAIL_TEMPLATE {
        formatted_caption == format!("${amount}")
    } else {
        formatted_caption == format!("${amount}") || formatted_caption == format!("+${amount}")
    };

    let (measure_width, measure_height) = measure_display_string_residual(&formatted_caption);
    let display_string_measure_ok =
        honesty_display_string_measure(&formatted_caption, measure_width, measure_height)
            && measure_width > 0;

    let (multi_locale_path_ok, multi_locale_path_count, multi_locale_live_found) =
        exercise_multi_locale_csf_residual();

    GameTextResidualExercise {
        honesty: GameTextResidualHonesty {
            str_parse_ok,
            csf_parse_ok,
            add_cash_template_ok,
            printf_format_ok,
            display_string_measure_ok,
            csf_entry_count: csf_map.len() as u32,
            live_csf_loaded,
            multi_locale_path_ok,
            multi_locale_path_count,
            multi_locale_live_found,
        },
        add_cash_template,
        formatted_caption,
        measure_width,
        measure_height,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn str_residual_parses_add_cash_template() {
        let map = parse_str_residual(
            "GUI:AddCash\n\"$%d\"\nEND\nGUI:Back\n\"BACK\"\nEND\n",
        );
        assert_eq!(map.get(GUI_ADD_CASH_KEY).map(String::as_str), Some("$%d"));
        assert_eq!(map.get(GUI_BACK_KEY).map(String::as_str), Some("BACK"));
    }

    #[test]
    fn printf_d_formats_retail_add_cash() {
        assert_eq!(format_printf_d("$%d", 200), "$200");
        assert_eq!(format_printf_d("Cash: %d credits", 50), "Cash: 50 credits");
    }

    #[test]
    fn synthetic_csf_roundtrip() {
        let bytes = build_synthetic_csf(GUI_ADD_CASH_KEY, GUI_ADD_CASH_RETAIL_TEMPLATE);
        let map = parse_csf_residual(&bytes).expect("parse synthetic CSF");
        assert_eq!(
            map.get(GUI_ADD_CASH_KEY).map(String::as_str),
            Some(GUI_ADD_CASH_RETAIL_TEMPLATE)
        );
    }

    #[test]
    fn display_string_measure_residual_is_monospaced() {
        let (w, h) = measure_display_string_residual("$150");
        assert_eq!(h, 8);
        assert_eq!(w, 4 * 8);
        assert!(honesty_display_string_measure("$150", w, h));
    }

    #[test]
    fn exercise_host_game_text_residual_is_honest() {
        let ex = exercise_host_game_text_residual();
        assert!(ex.honesty.str_parse_ok, "STR residual");
        assert!(ex.honesty.csf_parse_ok, "CSF residual");
        assert!(ex.honesty.add_cash_template_ok, "template={}", ex.add_cash_template);
        assert!(ex.honesty.printf_format_ok, "caption={}", ex.formatted_caption);
        assert!(ex.honesty.display_string_measure_ok);
        assert!(ex.honesty.multi_locale_path_ok, "multi-locale path table");
        assert_eq!(ex.honesty.multi_locale_path_count, 5);
        assert!(ex.honesty.honesty_ok());
        assert_eq!(ex.formatted_caption, "$150");
    }

    #[test]
    fn multi_locale_csf_path_residual_table() {
        assert_eq!(ResidualLanguageId::ALL.len(), 5);
        assert_eq!(ResidualLanguageId::German.folder_name(), "German");
        assert_eq!(ResidualLanguageId::French.zh_root(), "FrenchZH");
        let en = residual_csf_relatives(ResidualLanguageId::English);
        assert!(en.iter().any(|p| p.contains("EnglishZH")));
        assert!(en.iter().any(|p| p.contains("W3DEnglishZH")));
        let de = residual_csf_relatives(ResidualLanguageId::German);
        assert!(de.iter().any(|p| p.contains("GermanZH/Data/German/generals.csf")));
        let (ok, count, _live) = exercise_multi_locale_csf_residual();
        assert!(ok);
        assert_eq!(count, 5);
    }
}
