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
//! - LanguageId residual path table (English/UK/German/French/Spanish/Italian)
//! - Optional live multi-locale CSF probe when assets present
//! - UK LanguageId residual maps to English CSF paths (retail UK share)
//!
//! Host residual multi-locale LanguageId STR path table closed here (fail-closed
//! vs full runtime boot for every locale STR asset pack):
//! - LanguageId residual STR path table (map.str / generals.str relatives)
//!
//! Host residual GameText fetch missing-label residual closed here:
//! - C++ `GameTextManager::fetch` missing path → `MISSING: 'label'` + exists=false
//! - Missing-string list residual de-dupes identical missing labels
//!
//! Host residual GameText `translateCopy` escape residual closed here:
//! - Backslash escape table matching C++ `GameTextManager::translateCopy`
//!   (`\\n` → newline, `\\t` tab, `\\\\` backslash, `\\'` `\\\"` `\\?`)
//! - Honesty tests for escape table residual
//!
//! Host residual English CSF pack load peel closed here (Wave 65; fail-closed
//! vs full multi-locale boot UI for all LanguageId packs):
//! - Attempt load of English CSF pack path residual when assets present under
//!   `windows_game/.../English/generals.csf`
//! - Label-count residual honesty when the live file parses
//! - Missing asset → empty table honesty (not a boot UI claim)
//!
//! Wave 68 residual closed (host-testable, fail-closed vs boot UI):
//! - Group numeral GameText key residual `NUMBER:%d` (MAX_GROUPS **10**)
//! - Formation letter key residual `LABEL:FORMATION` used by W3DDisplayStringManager
//!
//! Wave 74 residual closed (host-testable, fail-closed vs boot UI):
//! - Multi-locale CSF pack load residual for German/French/Spanish/Italian
//!   (plus English) — path resolve under windows_game when assets exist
//! - Label-count residual honesty when a locale path is present and parses
//! - Empty-table honesty when locale pack is absent (not a boot UI claim)
//!
//! Still residual:
//! - Full multi-locale CSF/STR load for all LanguageId paths at runtime boot UI
//! - Full DisplayString GPU font raster / WW3D StretchRect submit
//! - Full Unicode word-wrap + hotkey underline on live InGameUI surface
//! - Full Jabber/debug reverseWord residual (debug-only LanguageId path)

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

/// C++ W3DDisplayStringManager MAX_GROUPS residual (standard build).
pub const GAME_TEXT_MAX_GROUPS: u32 = 10;
/// C++ group numeral GameText key format residual (`NUMBER:%d`).
pub const GAME_TEXT_GROUP_NUMERAL_KEY_PREFIX: &str = "NUMBER:";
/// C++ formation letter GameText key residual.
pub const GAME_TEXT_FORMATION_LETTER_KEY: &str = "LABEL:FORMATION";

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
    /// Wave 65: English CSF pack load residual honesty (label count / empty miss).
    pub english_csf_pack_load_ok: bool,
    /// Wave 65: English CSF pack label count residual (0 when missing asset).
    pub english_csf_label_count: u32,
    /// Wave 74: multi-locale primary CSF pack load residual honesty
    /// (English/German/French/Spanish/Italian path resolve + label/empty).
    pub multi_locale_csf_pack_load_ok: bool,
    /// Wave 74: primary locales exercised (always 5 when path table residual ok).
    pub multi_locale_csf_pack_locale_count: u32,
    /// Wave 74: primary locales with live CSF packs found under windows_game.
    pub multi_locale_csf_pack_live_found: u32,
    /// Wave 74: sum of label counts across live primary locale packs (0 if none).
    pub multi_locale_csf_pack_label_total: u32,
}

impl GameTextResidualHonesty {
    pub fn honesty_ok(&self) -> bool {
        self.str_parse_ok
            && self.csf_parse_ok
            && self.add_cash_template_ok
            && self.printf_format_ok
            && self.display_string_measure_ok
            && self.multi_locale_path_ok
            && self.english_csf_pack_load_ok
            && self.multi_locale_csf_pack_load_ok
    }
}

/// Host residual result for English CSF pack path load peel (Wave 65).
///
/// Fail-closed: not full multi-locale GameTextManager boot UI for all LanguageId.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnglishCsfPackLoadResidual {
    /// True when an English generals.csf path residual was found under windows_game.
    pub path_found: bool,
    /// True when the live CSF binary residual parsed successfully.
    pub parse_ok: bool,
    /// Label count residual from the live pack (0 when missing / parse fail).
    pub label_count: u32,
    /// True when missing asset yields an empty table (honest fail-closed residual).
    pub empty_table_when_missing: bool,
    /// Optional absolute path residual when found (debug honesty only).
    pub path_display: String,
}

impl EnglishCsfPackLoadResidual {
    /// Residual honesty: live pack has labels, or missing asset is empty-table honest.
    pub fn honesty_ok(&self) -> bool {
        if self.path_found {
            self.parse_ok && self.label_count > 0
        } else {
            // Missing asset → empty table honesty (not a boot UI claim).
            self.empty_table_when_missing && self.label_count == 0 && !self.parse_ok
        }
    }
}

/// Host residual result for one LanguageId CSF pack path load peel (Wave 74).
///
/// Generalizes Wave 65 English pack load to German/French/Spanish/Italian (and
/// English). Fail-closed: not full multi-locale GameTextManager boot UI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocaleCsfPackLoadResidual {
    /// LanguageId residual this pack load was resolved for.
    pub language: ResidualLanguageId,
    /// True when a generals.csf path residual was found under windows_game.
    pub path_found: bool,
    /// True when the live CSF binary residual parsed successfully.
    pub parse_ok: bool,
    /// Label count residual from the live pack (0 when missing / parse fail).
    pub label_count: u32,
    /// True when missing asset yields an empty table (honest fail-closed residual).
    pub empty_table_when_missing: bool,
    /// Optional absolute path residual when found (debug honesty only).
    pub path_display: String,
}

impl LocaleCsfPackLoadResidual {
    /// Residual honesty: live pack has labels, or missing asset is empty-table honest.
    pub fn honesty_ok(&self) -> bool {
        if self.path_found {
            self.parse_ok && self.label_count > 0
        } else {
            self.empty_table_when_missing && self.label_count == 0 && !self.parse_ok
        }
    }
}

/// Primary retail locale packs residual (Wave 74 multi-locale CSF peel).
///
/// English + German + French + Spanish + Italian. UK/Jabber/Japanese/Korean/
/// Unknown are path-table residual only (not pack-load peel targets here).
pub const PRIMARY_LOCALE_CSF_PACKS: [ResidualLanguageId; 5] = [
    ResidualLanguageId::English,
    ResidualLanguageId::German,
    ResidualLanguageId::French,
    ResidualLanguageId::Spanish,
    ResidualLanguageId::Italian,
];

/// Wave 102: expanded locale CSF pack-load residual targets.
///
/// All residual LanguageId discriminants that ship path tables (including UK /
/// Japanese / Korean / Jabber / Unknown). Missing assets remain empty-table
/// honest (fail-closed; CI without locale packs still passes).
pub const EXPANDED_LOCALE_CSF_PACKS: [ResidualLanguageId; 10] = [
    ResidualLanguageId::English,
    ResidualLanguageId::Uk,
    ResidualLanguageId::German,
    ResidualLanguageId::French,
    ResidualLanguageId::Spanish,
    ResidualLanguageId::Italian,
    ResidualLanguageId::Japanese,
    ResidualLanguageId::Jabber,
    ResidualLanguageId::Korean,
    ResidualLanguageId::Unknown,
];

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

/// Format C++ missing-string residual: `MISSING: 'label'`.
///
/// C++ `GameTextManager::fetch` when LUT miss formats
/// `UnicodeString missingString; missingString.format(L"MISSING: '%hs'", label)`.
#[inline]
pub fn game_text_missing_string(label: &str) -> String {
    format!("MISSING: '{label}'")
}

/// Residual GameText fetch result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameTextFetchResidual {
    pub text: String,
    /// C++ `*exists` out-param residual.
    pub exists: bool,
    /// True when this fetch registered a new missing-string list entry.
    pub registered_missing: bool,
}

/// Host residual for C++ `GameTextManager::fetch`.
///
/// Hit → (value, exists=true). Miss → (`MISSING: 'label'`, exists=false).
/// Optional `seen_missing` de-dupes the C++ `m_noStringList` residual.
/// Fail-closed: not full multi-locale CSF boot UI / live DisplayString draw.
pub fn game_text_fetch_residual(
    table: &HashMap<String, String>,
    label: &str,
    seen_missing: &mut Vec<String>,
) -> GameTextFetchResidual {
    if let Some(value) = table.get(label) {
        return GameTextFetchResidual {
            text: value.clone(),
            exists: true,
            registered_missing: false,
        };
    }
    let missing = game_text_missing_string(label);
    let registered_missing = if seen_missing.iter().any(|s| s == &missing) {
        false
    } else {
        seen_missing.push(missing.clone());
        true
    };
    GameTextFetchResidual {
        text: missing,
        exists: false,
        registered_missing,
    }
}

/// Honesty: fetch residual matches hit / MISSING path + de-dupe.
pub fn honesty_game_text_fetch_missing() -> bool {
    let mut table = HashMap::new();
    table.insert(
        GUI_ADD_CASH_KEY.to_string(),
        GUI_ADD_CASH_RETAIL_TEMPLATE.to_string(),
    );
    let mut seen = Vec::new();
    let hit = game_text_fetch_residual(&table, GUI_ADD_CASH_KEY, &mut seen);
    if !hit.exists || hit.text != GUI_ADD_CASH_RETAIL_TEMPLATE || hit.registered_missing {
        return false;
    }
    if !seen.is_empty() {
        return false;
    }
    let miss = game_text_fetch_residual(&table, "GUI:NoSuchLabel", &mut seen);
    if miss.exists
        || miss.text != "MISSING: 'GUI:NoSuchLabel'"
        || !miss.registered_missing
        || seen.len() != 1
    {
        return false;
    }
    let miss2 = game_text_fetch_residual(&table, "GUI:NoSuchLabel", &mut seen);
    !miss2.exists
        && miss2.text == "MISSING: 'GUI:NoSuchLabel'"
        && !miss2.registered_missing
        && seen.len() == 1
        && game_text_missing_string("X") == "MISSING: 'X'"
}

// ---------------------------------------------------------------------------
// GameText translateCopy residual (GameText.cpp)
// ---------------------------------------------------------------------------

/// C++ `GameTextManager::translateCopy` residual escape table.
///
/// Converts STR/CSF backslash escape sequences used by retail string files:
/// - `\\` → `\`
/// - `\'` → `'`
/// - `\"` → `"`
/// - `\?` → `?`
/// - `\t` → tab
/// - `\n` → newline
/// - other `\X` → `X` (default branch residual)
///
/// Fail-closed: not full Jabber reverseWord debug residual / multi-locale boot UI.
pub fn translate_copy_residual(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars();
    let mut slash = false;
    while let Some(ch) = chars.next() {
        if slash {
            slash = false;
            match ch {
                '\\' => out.push('\\'),
                '\'' => out.push('\''),
                '"' => out.push('"'),
                '?' => out.push('?'),
                't' => out.push('\t'),
                'n' => out.push('\n'),
                // C++ default: *outbuf++ = *inbuf & 0x00FF
                other => out.push(other),
            }
        } else if ch == '\\' {
            slash = true;
        } else {
            out.push(ch);
        }
    }
    // Trailing lone backslash residual: C++ would leave slash=TRUE with no char;
    // host residual drops the trailing incomplete escape (fail-closed honesty).
    out
}

/// Honesty: translateCopy residual escape table matches C++ slash handling subset.
pub fn honesty_translate_copy_escape_table() -> bool {
    translate_copy_residual(r"Hello\nWorld") == "Hello\nWorld"
        && translate_copy_residual(r"Tab\tHere") == "Tab\tHere"
        && translate_copy_residual(r"Back\\Slash") == "Back\\Slash"
        && translate_copy_residual("Quote\\\"Mark") == "Quote\"Mark"
        && translate_copy_residual(r"Apos\'s") == "Apos's"
        && translate_copy_residual(r"Q\?M") == "Q?M"
        // Default branch residual: unknown escape keeps the escaped char.
        && translate_copy_residual(r"X\yZ") == "XyZ"
        // No escapes residual is identity.
        && translate_copy_residual("BACK") == "BACK"
        && translate_copy_residual("$%d") == "$%d"
        // Multi-escape residual.
        && translate_copy_residual(r"Line1\nLine2\n") == "Line1\nLine2\n"
        // Empty residual.
        && translate_copy_residual("").is_empty()
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
    /// LANGUAGE_ID_US residual.
    English,
    /// LANGUAGE_ID_UK residual (shares English CSF pack paths in retail ZH).
    Uk,
    German,
    French,
    Spanish,
    Italian,
    /// LANGUAGE_ID_JAPANESE residual (path table honesty; may lack live ZH packs).
    Japanese,
    /// LANGUAGE_ID_JABBER residual (debug locale; shares English CSF pack paths).
    Jabber,
    /// LANGUAGE_ID_KOREAN residual (path table honesty; may lack live ZH packs).
    Korean,
    /// LANGUAGE_ID_UNKNOWN residual (fail-closed English CSF pack path residual).
    Unknown,
}

impl ResidualLanguageId {
    /// All residual LanguageId discriminants with CSF path tables (Language.h order).
    ///
    /// Includes LANGUAGE_ID_UK residual which maps to English folder/pack paths
    /// (UK does not ship a separate EnglishUK generals.csf in ZH extracts),
    /// plus Japanese/Jabber/Korean/Unknown residual discriminants.
    pub const ALL: [ResidualLanguageId; 10] = [
        ResidualLanguageId::English,
        ResidualLanguageId::Uk,
        ResidualLanguageId::German,
        ResidualLanguageId::French,
        ResidualLanguageId::Spanish,
        ResidualLanguageId::Italian,
        ResidualLanguageId::Japanese,
        ResidualLanguageId::Jabber,
        ResidualLanguageId::Korean,
        ResidualLanguageId::Unknown,
    ];

    /// Retail language folder name residual (`Data/<Name>/generals.csf`).
    pub fn folder_name(self) -> &'static str {
        match self {
            ResidualLanguageId::English
            | ResidualLanguageId::Uk
            | ResidualLanguageId::Jabber
            | ResidualLanguageId::Unknown => "English",
            ResidualLanguageId::German => "German",
            ResidualLanguageId::French => "French",
            ResidualLanguageId::Spanish => "Spanish",
            ResidualLanguageId::Italian => "Italian",
            ResidualLanguageId::Japanese => "Japanese",
            ResidualLanguageId::Korean => "Korean",
        }
    }

    /// Retail ZH big-file language root residual (`EnglishZH`, `GermanZH`, …).
    pub fn zh_root(self) -> &'static str {
        match self {
            ResidualLanguageId::English
            | ResidualLanguageId::Uk
            | ResidualLanguageId::Jabber
            | ResidualLanguageId::Unknown => "EnglishZH",
            ResidualLanguageId::German => "GermanZH",
            ResidualLanguageId::French => "FrenchZH",
            ResidualLanguageId::Spanish => "SpanishZH",
            ResidualLanguageId::Italian => "ItalianZH",
            ResidualLanguageId::Japanese => "JapaneseZH",
            ResidualLanguageId::Korean => "KoreanZH",
        }
    }

    /// C++ LanguageID discriminant residual (Language.h order).
    pub fn language_id(self) -> u32 {
        match self {
            ResidualLanguageId::English => 0, // LANGUAGE_ID_US
            ResidualLanguageId::Uk => 1,      // LANGUAGE_ID_UK
            ResidualLanguageId::German => 2,
            ResidualLanguageId::French => 3,
            ResidualLanguageId::Spanish => 4,
            ResidualLanguageId::Italian => 5,
            ResidualLanguageId::Japanese => 6,
            ResidualLanguageId::Jabber => 7,
            ResidualLanguageId::Korean => 8,
            ResidualLanguageId::Unknown => 9,
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
    // English-family residual packs also have W3DEnglishZH paths (parity with GameClient).
    if matches!(
        language,
        ResidualLanguageId::English
            | ResidualLanguageId::Uk
            | ResidualLanguageId::Jabber
            | ResidualLanguageId::Unknown
    ) {
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

/// Retail multi-locale STR relative path residuals for one LanguageId.
///
/// Fail-closed path table residual (not full GlobalData Language STR boot).
/// English-family residual also includes W3DEnglishZH map.str relatives.
pub fn residual_str_relatives(language: ResidualLanguageId) -> Vec<String> {
    let folder = language.folder_name();
    let root = language.zh_root();
    let mut paths = vec![
        format!("windows_game/extracted_big_files/{root}/Data/{folder}/generals.str"),
        format!("windows_game/extracted_big_files_v2/{root}/Data/{folder}/generals.str"),
        format!("windows_game/extracted_big_files/{root}/Data/{folder}/map.str"),
        format!("windows_game/extracted_big_files_v2/{root}/Data/{folder}/map.str"),
    ];
    if matches!(
        language,
        ResidualLanguageId::English
            | ResidualLanguageId::Uk
            | ResidualLanguageId::Jabber
            | ResidualLanguageId::Unknown
    ) {
        paths.push(
            "windows_game/extracted_big_files/W3DEnglishZH/Data/English/generals.str".to_string(),
        );
        paths.push(
            "windows_game/extracted_big_files_v2/W3DEnglishZH/Data/English/generals.str"
                .to_string(),
        );
    }
    paths
}

/// Host residual exercise: multi-locale LanguageId STR path table honesty.
///
/// Always honest with synthetic path-table residual when assets are absent.
pub fn exercise_multi_locale_str_residual() -> (bool, u32) {
    let mut path_count = 0u32;
    for lang in ResidualLanguageId::ALL {
        let relatives = residual_str_relatives(lang);
        let folder = lang.folder_name();
        let path_ok = relatives.iter().any(|p| {
            p.contains(&format!("Data/{folder}/generals.str"))
                || p.contains(&format!("Data/{folder}/map.str"))
                || (matches!(
                    lang,
                    ResidualLanguageId::English
                        | ResidualLanguageId::Uk
                        | ResidualLanguageId::Jabber
                        | ResidualLanguageId::Unknown
                ) && p.contains("Data/English/generals.str"))
        });
        if path_ok && !relatives.is_empty() {
            path_count = path_count.saturating_add(1);
        }
    }
    let multi_ok = path_count == ResidualLanguageId::ALL.len() as u32
        && residual_str_relatives(ResidualLanguageId::English).len() >= 4
        && residual_str_relatives(ResidualLanguageId::German).len() >= 4
        && residual_str_relatives(ResidualLanguageId::Japanese).len() >= 4
        && residual_str_relatives(ResidualLanguageId::Korean).len() >= 4;
    (multi_ok, path_count)
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
                || (matches!(
                    lang,
                    ResidualLanguageId::English
                        | ResidualLanguageId::Uk
                        | ResidualLanguageId::Jabber
                        | ResidualLanguageId::Unknown
                ) && p.contains("Data/English/generals.csf"))
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
        && residual_csf_relatives(ResidualLanguageId::German).len() >= 2
        && residual_csf_relatives(ResidualLanguageId::Japanese).len() >= 2
        && residual_csf_relatives(ResidualLanguageId::Korean).len() >= 2
        && ResidualLanguageId::Japanese.language_id() == 6
        && ResidualLanguageId::Jabber.language_id() == 7
        && ResidualLanguageId::Korean.language_id() == 8
        && ResidualLanguageId::Unknown.language_id() == 9;
    (multi_ok, path_count, live_found)
}

/// Locate English generals.csf in repo assets when present.
pub fn find_english_csf_path() -> Option<PathBuf> {
    find_csf_path_for_language(ResidualLanguageId::English)
}

/// Host residual: attempt load of one LanguageId CSF pack path under windows_game.
///
/// When the asset is present, parse and count labels. When missing, return an
/// empty table honesty residual (label_count **0**, empty_table_when_missing).
/// Fail-closed: not full multi-locale CSF boot UI / LanguageId runtime switch.
pub fn load_locale_csf_pack_residual(language: ResidualLanguageId) -> LocaleCsfPackLoadResidual {
    match find_csf_path_for_language(language) {
        Some(path) => {
            let path_display = path.display().to_string();
            match fs::read(&path).ok().and_then(|b| parse_csf_residual(&b)) {
                Some(map) => LocaleCsfPackLoadResidual {
                    language,
                    path_found: true,
                    parse_ok: !map.is_empty(),
                    label_count: map.len() as u32,
                    empty_table_when_missing: false,
                    path_display,
                },
                None => LocaleCsfPackLoadResidual {
                    // Path exists but parse failed — fail-closed empty residual.
                    language,
                    path_found: true,
                    parse_ok: false,
                    label_count: 0,
                    empty_table_when_missing: true,
                    path_display,
                },
            }
        }
        None => LocaleCsfPackLoadResidual {
            language,
            path_found: false,
            parse_ok: false,
            label_count: 0,
            empty_table_when_missing: true,
            path_display: String::new(),
        },
    }
}

/// Host residual: attempt load of English CSF pack path under windows_game.
///
/// When the asset is present, parse and count labels. When missing, return an
/// empty table honesty residual (label_count **0**, empty_table_when_missing).
/// Fail-closed: not full multi-locale CSF boot UI / LanguageId runtime switch.
pub fn load_english_csf_pack_residual() -> EnglishCsfPackLoadResidual {
    let locale = load_locale_csf_pack_residual(ResidualLanguageId::English);
    EnglishCsfPackLoadResidual {
        path_found: locale.path_found,
        parse_ok: locale.parse_ok,
        label_count: locale.label_count,
        empty_table_when_missing: locale.empty_table_when_missing,
        path_display: locale.path_display,
    }
}

/// Wave 74: multi-locale primary CSF pack load residual exercise.
///
/// Path-resolve + parse for English/German/French/Spanish/Italian. Live packs
/// report label counts; missing packs report empty-table honesty. Always
/// honest without assets (CI). Fail-closed: not full multi-locale boot UI.
pub fn exercise_multi_locale_csf_pack_load_residual(
) -> (bool, u32, u32, u32, Vec<LocaleCsfPackLoadResidual>) {
    let mut packs = Vec::with_capacity(PRIMARY_LOCALE_CSF_PACKS.len());
    let mut live_found = 0u32;
    let mut label_total = 0u32;
    let mut all_ok = true;
    for &lang in &PRIMARY_LOCALE_CSF_PACKS {
        let pack = load_locale_csf_pack_residual(lang);
        if !pack.honesty_ok() {
            all_ok = false;
        }
        // Path table residual must list expected locale folder/csf relatives.
        let relatives = residual_csf_relatives(lang);
        let folder = lang.folder_name();
        let path_table_ok = !relatives.is_empty()
            && relatives
                .iter()
                .any(|p| p.contains(&format!("Data/{folder}/generals.csf")));
        if !path_table_ok {
            all_ok = false;
        }
        if pack.path_found && pack.parse_ok {
            live_found = live_found.saturating_add(1);
            label_total = label_total.saturating_add(pack.label_count);
        }
        packs.push(pack);
    }
    let locale_count = PRIMARY_LOCALE_CSF_PACKS.len() as u32;
    let multi_ok = all_ok
        && locale_count == 5
        && packs.len() as u32 == locale_count
        // German/French/Spanish/Italian residual path tables always present.
        && residual_csf_relatives(ResidualLanguageId::German)
            .iter()
            .any(|p| p.contains("GermanZH/Data/German/generals.csf"))
        && residual_csf_relatives(ResidualLanguageId::French)
            .iter()
            .any(|p| p.contains("FrenchZH/Data/French/generals.csf"))
        && residual_csf_relatives(ResidualLanguageId::Spanish)
            .iter()
            .any(|p| p.contains("SpanishZH/Data/Spanish/generals.csf"))
        && residual_csf_relatives(ResidualLanguageId::Italian)
            .iter()
            .any(|p| p.contains("ItalianZH/Data/Italian/generals.csf"));
    (multi_ok, locale_count, live_found, label_total, packs)
}

/// Honesty: English CSF pack load residual (Wave 65).
///
/// Live pack → label_count > 0. Missing asset → empty table honesty.
/// Also checks that residual_csf_relatives(English) path table is non-empty.
pub fn honesty_english_csf_pack_load() -> bool {
    let load = load_english_csf_pack_residual();
    load.honesty_ok()
        && !residual_csf_relatives(ResidualLanguageId::English).is_empty()
        && residual_csf_relatives(ResidualLanguageId::English)
            .iter()
            .any(|p| p.contains("English") && p.ends_with("generals.csf"))
}

/// Honesty: multi-locale primary CSF pack load residual (Wave 74).
///
/// Path resolve for German/French/Spanish/Italian (and English). Live packs
/// count labels; absent packs are empty-table honest. Fail-closed vs boot UI.
pub fn honesty_multi_locale_csf_pack_load() -> bool {
    let (ok, locale_count, _live, _labels, packs) = exercise_multi_locale_csf_pack_load_residual();
    ok && locale_count == PRIMARY_LOCALE_CSF_PACKS.len() as u32
        && packs.iter().all(|p| p.honesty_ok())
        && packs
            .iter()
            .any(|p| p.language == ResidualLanguageId::German)
        && packs
            .iter()
            .any(|p| p.language == ResidualLanguageId::French)
        && packs
            .iter()
            .any(|p| p.language == ResidualLanguageId::Spanish)
        && packs
            .iter()
            .any(|p| p.language == ResidualLanguageId::Italian)
}

/// Wave 102: expanded multi-locale CSF pack load residual exercise.
///
/// Path-resolve + parse for all **10** residual LanguageId values (primary 5
/// plus UK/Japanese/Jabber/Korean/Unknown). Live packs report label counts;
/// missing packs report empty-table honesty. Always honest without assets (CI).
/// Fail-closed: not full multi-locale GameTextManager boot UI.
pub fn exercise_expanded_locale_csf_pack_load_residual(
) -> (bool, u32, u32, u32, Vec<LocaleCsfPackLoadResidual>) {
    let mut packs = Vec::with_capacity(EXPANDED_LOCALE_CSF_PACKS.len());
    let mut live_found = 0u32;
    let mut label_total = 0u32;
    let mut all_ok = true;
    for &lang in &EXPANDED_LOCALE_CSF_PACKS {
        let pack = load_locale_csf_pack_residual(lang);
        if !pack.honesty_ok() {
            all_ok = false;
        }
        // Path table residual must list expected locale folder/csf relatives.
        let relatives = residual_csf_relatives(lang);
        let folder = lang.folder_name();
        let path_table_ok = !relatives.is_empty()
            && relatives
                .iter()
                .any(|p| p.contains(&format!("Data/{folder}/generals.csf")));
        if !path_table_ok {
            all_ok = false;
        }
        // Empty honesty when missing: path_found=false → empty_table_when_missing.
        if !pack.path_found {
            if !pack.empty_table_when_missing || pack.label_count != 0 || pack.parse_ok {
                all_ok = false;
            }
        }
        if pack.path_found && pack.parse_ok {
            live_found = live_found.saturating_add(1);
            label_total = label_total.saturating_add(pack.label_count);
        }
        packs.push(pack);
    }
    let locale_count = EXPANDED_LOCALE_CSF_PACKS.len() as u32;
    let expanded_ok = all_ok
        && locale_count == 10
        && packs.len() as u32 == locale_count
        // Path tables for JA/KO residual always present (even if assets absent).
        && residual_csf_relatives(ResidualLanguageId::Japanese)
            .iter()
            .any(|p| p.contains("JapaneseZH/Data/Japanese/generals.csf"))
        && residual_csf_relatives(ResidualLanguageId::Korean)
            .iter()
            .any(|p| p.contains("KoreanZH/Data/Korean/generals.csf"))
        // UK/Jabber/Unknown share English folder residual paths.
        && residual_csf_relatives(ResidualLanguageId::Uk)
            .iter()
            .any(|p| p.contains("English") && p.ends_with("generals.csf"))
        && residual_csf_relatives(ResidualLanguageId::Jabber)
            .iter()
            .any(|p| p.contains("English") && p.ends_with("generals.csf"));
    (expanded_ok, locale_count, live_found, label_total, packs)
}

/// Honesty: expanded multi-locale CSF pack load residual (Wave 102).
///
/// All 10 LanguageId residual packs: live → labels; absent → empty honesty.
/// Fail-closed vs full multi-locale boot UI / DisplayString Unicode draw.
pub fn honesty_expanded_locale_csf_pack_load_wave102() -> bool {
    let (ok, locale_count, _live, _labels, packs) =
        exercise_expanded_locale_csf_pack_load_residual();
    ok && locale_count == EXPANDED_LOCALE_CSF_PACKS.len() as u32
        && packs.iter().all(|p| p.honesty_ok())
        && packs.len() == 10
        && packs.iter().any(|p| p.language == ResidualLanguageId::Japanese)
        && packs.iter().any(|p| p.language == ResidualLanguageId::Korean)
        && packs.iter().any(|p| p.language == ResidualLanguageId::Uk)
        && packs.iter().any(|p| p.language == ResidualLanguageId::Jabber)
        // Primary pack residual still honest.
        && honesty_multi_locale_csf_pack_load()
        && honesty_english_csf_pack_load()
}

/// Combined Wave 102 multi-locale CSF residual honesty pack.
pub fn honesty_csf_multi_locale_residual_deepen_pack_wave102() -> bool {
    honesty_expanded_locale_csf_pack_load_wave102()
}

/// Format C++ group numeral GameText key residual: `NUMBER:N`.
pub fn game_text_group_numeral_key(numeral: u32) -> String {
    format!("{GAME_TEXT_GROUP_NUMERAL_KEY_PREFIX}{numeral}")
}

/// Whether a group numeral is in residual MAX_GROUPS range [0, MAX_GROUPS).
pub fn game_text_group_numeral_in_range(numeral: i32) -> bool {
    numeral >= 0 && (numeral as u32) < GAME_TEXT_MAX_GROUPS
}

/// Wave 68 residual honesty: group numeral + formation letter GameText keys.
pub fn honesty_game_text_group_numeral_keys() -> bool {
    if GAME_TEXT_MAX_GROUPS != 10 {
        return false;
    }
    if GAME_TEXT_FORMATION_LETTER_KEY != "LABEL:FORMATION" {
        return false;
    }
    if GAME_TEXT_GROUP_NUMERAL_KEY_PREFIX != "NUMBER:" {
        return false;
    }
    for i in 0..GAME_TEXT_MAX_GROUPS {
        let key = game_text_group_numeral_key(i);
        if key != format!("NUMBER:{i}") {
            return false;
        }
        if !game_text_group_numeral_in_range(i as i32) {
            return false;
        }
    }
    if game_text_group_numeral_in_range(-1) || game_text_group_numeral_in_range(10) {
        return false;
    }
    true
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
    let str_parse_ok = str_map.get(GUI_ADD_CASH_KEY).map(String::as_str)
        == Some(GUI_ADD_CASH_RETAIL_TEMPLATE)
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

    // Wave 65: English CSF pack load residual peel (label count / empty miss).
    let english_pack = load_english_csf_pack_residual();
    let english_csf_pack_load_ok = english_pack.honesty_ok()
        && !residual_csf_relatives(ResidualLanguageId::English).is_empty();
    // Prefer live pack label count; fall back to exercise map count when synthetic.
    let english_csf_label_count = if english_pack.path_found && english_pack.parse_ok {
        english_pack.label_count
    } else {
        0
    };

    // Wave 74: multi-locale primary CSF pack load residual (DE/FR/ES/IT + EN).
    let (
        multi_locale_csf_pack_load_ok,
        multi_locale_csf_pack_locale_count,
        multi_locale_csf_pack_live_found,
        multi_locale_csf_pack_label_total,
        _packs,
    ) = exercise_multi_locale_csf_pack_load_residual();

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
            english_csf_pack_load_ok,
            english_csf_label_count,
            multi_locale_csf_pack_load_ok,
            multi_locale_csf_pack_locale_count,
            multi_locale_csf_pack_live_found,
            multi_locale_csf_pack_label_total,
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
        let map = parse_str_residual("GUI:AddCash\n\"$%d\"\nEND\nGUI:Back\n\"BACK\"\nEND\n");
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
        assert!(
            ex.honesty.add_cash_template_ok,
            "template={}",
            ex.add_cash_template
        );
        assert!(
            ex.honesty.printf_format_ok,
            "caption={}",
            ex.formatted_caption
        );
        assert!(ex.honesty.display_string_measure_ok);
        assert!(ex.honesty.multi_locale_path_ok, "multi-locale path table");
        assert_eq!(ex.honesty.multi_locale_path_count, 10);
        assert!(
            ex.honesty.english_csf_pack_load_ok,
            "english CSF pack load residual"
        );
        // Live English CSF under windows_game → label_count > 0; missing → 0.
        let pack = load_english_csf_pack_residual();
        if pack.path_found && pack.parse_ok {
            assert!(ex.honesty.english_csf_label_count > 0);
            assert_eq!(ex.honesty.english_csf_label_count, pack.label_count);
        } else {
            assert_eq!(ex.honesty.english_csf_label_count, 0);
            assert!(pack.empty_table_when_missing);
        }
        // Wave 74: multi-locale primary pack load residual.
        assert!(
            ex.honesty.multi_locale_csf_pack_load_ok,
            "multi-locale CSF pack load residual"
        );
        assert_eq!(ex.honesty.multi_locale_csf_pack_locale_count, 5);
        assert!(ex.honesty.honesty_ok());
        assert_eq!(ex.formatted_caption, "$150");
    }

    #[test]
    fn game_text_group_numeral_keys_residual_honesty() {
        assert!(honesty_game_text_group_numeral_keys());
        assert_eq!(GAME_TEXT_MAX_GROUPS, 10);
        assert_eq!(game_text_group_numeral_key(0), "NUMBER:0");
        assert_eq!(game_text_group_numeral_key(9), "NUMBER:9");
        assert_eq!(GAME_TEXT_FORMATION_LETTER_KEY, "LABEL:FORMATION");
        assert!(game_text_group_numeral_in_range(0));
        assert!(game_text_group_numeral_in_range(9));
        assert!(!game_text_group_numeral_in_range(10));
        assert!(!game_text_group_numeral_in_range(-1));
    }

    #[test]
    fn english_csf_pack_load_residual_wave65_honesty() {
        assert!(honesty_english_csf_pack_load());
        let pack = load_english_csf_pack_residual();
        assert!(
            pack.honesty_ok(),
            "path_found={} parse_ok={} labels={}",
            pack.path_found,
            pack.parse_ok,
            pack.label_count
        );
        // English residual path table always lists windows_game English pack relatives.
        let relatives = residual_csf_relatives(ResidualLanguageId::English);
        assert!(relatives.iter().any(|p| p.contains("EnglishZH")));
        assert!(relatives.iter().any(|p| p.ends_with("generals.csf")));
        if pack.path_found {
            // Live asset residual: must count labels honestly (retail generals.csf is large).
            assert!(pack.parse_ok, "live English CSF must parse");
            assert!(
                pack.label_count > 100,
                "retail English CSF label_count residual, got {}",
                pack.label_count
            );
            // Shell label residual when live pack present.
            if let Some(path) = find_english_csf_path() {
                let map = fs::read(&path)
                    .ok()
                    .and_then(|b| parse_csf_residual(&b))
                    .expect("parse live English CSF");
                assert_eq!(map.len() as u32, pack.label_count);
                assert_eq!(
                    map.get(GUI_ADD_CASH_KEY).map(String::as_str),
                    Some(GUI_ADD_CASH_RETAIL_TEMPLATE)
                );
            }
        } else {
            // Fail-closed missing-asset residual: empty table honesty.
            assert_eq!(pack.label_count, 0);
            assert!(!pack.parse_ok);
            assert!(pack.empty_table_when_missing);
        }
    }

    #[test]
    fn multi_locale_csf_pack_load_residual_wave74_honesty() {
        assert!(honesty_multi_locale_csf_pack_load());
        assert_eq!(PRIMARY_LOCALE_CSF_PACKS.len(), 5);
        let (ok, locale_count, live_found, label_total, packs) =
            exercise_multi_locale_csf_pack_load_residual();
        assert!(ok, "multi-locale CSF pack load residual");
        assert_eq!(locale_count, 5);
        assert_eq!(packs.len(), 5);
        // Path resolve residual for DE/FR/ES/IT always present in relatives table.
        for lang in [
            ResidualLanguageId::German,
            ResidualLanguageId::French,
            ResidualLanguageId::Spanish,
            ResidualLanguageId::Italian,
        ] {
            let pack = load_locale_csf_pack_residual(lang);
            assert!(pack.honesty_ok(), "locale={lang:?}");
            assert_eq!(pack.language, lang);
            if pack.path_found {
                assert!(pack.parse_ok);
                assert!(pack.label_count > 0, "live {} labels", lang.folder_name());
            } else {
                // Empty honesty when absent (typical CI / English-only extract).
                assert_eq!(pack.label_count, 0);
                assert!(!pack.parse_ok);
                assert!(pack.empty_table_when_missing);
            }
            let relatives = residual_csf_relatives(lang);
            assert!(
                relatives.iter().any(|p| p.contains(&format!(
                    "{}/Data/{}/generals.csf",
                    lang.zh_root(),
                    lang.folder_name()
                ))),
                "path table for {:?}",
                lang
            );
        }
        // Live-found count matches packs that resolved files.
        let counted_live = packs.iter().filter(|p| p.path_found && p.parse_ok).count() as u32;
        assert_eq!(live_found, counted_live);
        if live_found > 0 {
            assert!(label_total > 0);
        } else {
            assert_eq!(label_total, 0);
        }
        // English pack load residual must stay consistent with Wave 65 peel.
        let en = load_locale_csf_pack_residual(ResidualLanguageId::English);
        let en_legacy = load_english_csf_pack_residual();
        assert_eq!(en.path_found, en_legacy.path_found);
        assert_eq!(en.label_count, en_legacy.label_count);
        assert_eq!(en.parse_ok, en_legacy.parse_ok);
    }

    /// Wave 102 residual: expanded locale CSF pack load (JA/KO/UK/Jabber + empty honesty).
    #[test]
    fn expanded_locale_csf_pack_load_residual_wave102_honesty() {
        assert!(honesty_expanded_locale_csf_pack_load_wave102());
        assert!(honesty_csf_multi_locale_residual_deepen_pack_wave102());
        assert_eq!(EXPANDED_LOCALE_CSF_PACKS.len(), 10);
        assert_eq!(ResidualLanguageId::ALL.len(), 10);
        let (ok, locale_count, live_found, label_total, packs) =
            exercise_expanded_locale_csf_pack_load_residual();
        assert!(ok, "expanded multi-locale CSF pack load residual");
        assert_eq!(locale_count, 10);
        assert_eq!(packs.len(), 10);
        for lang in [
            ResidualLanguageId::Japanese,
            ResidualLanguageId::Korean,
            ResidualLanguageId::Uk,
            ResidualLanguageId::Jabber,
            ResidualLanguageId::Unknown,
        ] {
            let pack = load_locale_csf_pack_residual(lang);
            assert!(pack.honesty_ok(), "locale={lang:?}");
            assert_eq!(pack.language, lang);
            if pack.path_found {
                assert!(pack.parse_ok);
                assert!(pack.label_count > 0, "live {} labels", lang.folder_name());
            } else {
                // Empty honesty when absent (typical CI / English-only extract).
                assert_eq!(pack.label_count, 0);
                assert!(!pack.parse_ok);
                assert!(pack.empty_table_when_missing);
            }
        }
        let counted_live = packs.iter().filter(|p| p.path_found && p.parse_ok).count() as u32;
        assert_eq!(live_found, counted_live);
        if live_found > 0 {
            assert!(label_total > 0);
        } else {
            assert_eq!(label_total, 0);
        }
    }

    #[test]
    fn multi_locale_csf_path_residual_table() {
        assert_eq!(ResidualLanguageId::ALL.len(), 10);
        assert_eq!(ResidualLanguageId::German.folder_name(), "German");
        assert_eq!(ResidualLanguageId::French.zh_root(), "FrenchZH");
        assert_eq!(ResidualLanguageId::Uk.folder_name(), "English");
        assert_eq!(ResidualLanguageId::Uk.zh_root(), "EnglishZH");
        assert_eq!(ResidualLanguageId::English.language_id(), 0);
        assert_eq!(ResidualLanguageId::Uk.language_id(), 1);
        assert_eq!(ResidualLanguageId::German.language_id(), 2);
        assert_eq!(ResidualLanguageId::Japanese.language_id(), 6);
        assert_eq!(ResidualLanguageId::Jabber.language_id(), 7);
        assert_eq!(ResidualLanguageId::Korean.language_id(), 8);
        assert_eq!(ResidualLanguageId::Unknown.language_id(), 9);
        assert_eq!(ResidualLanguageId::Japanese.folder_name(), "Japanese");
        assert_eq!(ResidualLanguageId::Japanese.zh_root(), "JapaneseZH");
        assert_eq!(ResidualLanguageId::Korean.folder_name(), "Korean");
        assert_eq!(ResidualLanguageId::Korean.zh_root(), "KoreanZH");
        // Jabber/Unknown fail-closed share English CSF pack residual paths.
        assert_eq!(ResidualLanguageId::Jabber.folder_name(), "English");
        assert_eq!(ResidualLanguageId::Unknown.zh_root(), "EnglishZH");
        let en = residual_csf_relatives(ResidualLanguageId::English);
        assert!(en.iter().any(|p| p.contains("EnglishZH")));
        assert!(en.iter().any(|p| p.contains("W3DEnglishZH")));
        let uk = residual_csf_relatives(ResidualLanguageId::Uk);
        assert!(uk.iter().any(|p| p.contains("EnglishZH")));
        assert!(uk.iter().any(|p| p.contains("W3DEnglishZH")));
        let de = residual_csf_relatives(ResidualLanguageId::German);
        assert!(de
            .iter()
            .any(|p| p.contains("GermanZH/Data/German/generals.csf")));
        let ja = residual_csf_relatives(ResidualLanguageId::Japanese);
        assert!(ja
            .iter()
            .any(|p| p.contains("JapaneseZH/Data/Japanese/generals.csf")));
        let ko = residual_csf_relatives(ResidualLanguageId::Korean);
        assert!(ko
            .iter()
            .any(|p| p.contains("KoreanZH/Data/Korean/generals.csf")));
        let (ok, count, _live) = exercise_multi_locale_csf_residual();
        assert!(ok);
        assert_eq!(count, 10);
    }

    #[test]
    fn multi_locale_str_path_residual_table() {
        assert_eq!(ResidualLanguageId::ALL.len(), 10);
        let en = residual_str_relatives(ResidualLanguageId::English);
        assert!(en
            .iter()
            .any(|p| p.contains("EnglishZH") && p.ends_with("generals.str")));
        assert!(en.iter().any(|p| p.contains("W3DEnglishZH")));
        assert!(en.iter().any(|p| p.contains("map.str")));
        let uk = residual_str_relatives(ResidualLanguageId::Uk);
        assert!(uk.iter().any(|p| p.contains("EnglishZH")));
        let de = residual_str_relatives(ResidualLanguageId::German);
        assert!(de
            .iter()
            .any(|p| p.contains("GermanZH/Data/German/generals.str")));
        let ja = residual_str_relatives(ResidualLanguageId::Japanese);
        assert!(ja
            .iter()
            .any(|p| p.contains("JapaneseZH/Data/Japanese/generals.str")));
        let ko = residual_str_relatives(ResidualLanguageId::Korean);
        assert!(ko
            .iter()
            .any(|p| p.contains("KoreanZH/Data/Korean/generals.str")));
        // Jabber/Unknown fail-closed share English STR pack residual paths.
        assert!(residual_str_relatives(ResidualLanguageId::Jabber)
            .iter()
            .any(|p| p.contains("EnglishZH")));
        assert!(residual_str_relatives(ResidualLanguageId::Unknown)
            .iter()
            .any(|p| p.contains("EnglishZH")));
        let (ok, count) = exercise_multi_locale_str_residual();
        assert!(ok);
        assert_eq!(count, 10);
    }

    #[test]
    fn game_text_fetch_missing_residual_honesty() {
        assert!(honesty_game_text_fetch_missing());
        assert_eq!(game_text_missing_string("GUI:Back"), "MISSING: 'GUI:Back'");
        let mut table = HashMap::new();
        table.insert("GUI:Back".to_string(), "BACK".to_string());
        let mut seen = Vec::new();
        let hit = game_text_fetch_residual(&table, "GUI:Back", &mut seen);
        assert!(hit.exists);
        assert_eq!(hit.text, "BACK");
        let miss = game_text_fetch_residual(&table, "GUI:Nope", &mut seen);
        assert!(!miss.exists);
        assert_eq!(miss.text, "MISSING: 'GUI:Nope'");
        assert!(miss.registered_missing);
        let miss2 = game_text_fetch_residual(&table, "GUI:Nope", &mut seen);
        assert!(!miss2.registered_missing);
        assert_eq!(seen.len(), 1);
    }

    #[test]
    fn translate_copy_escape_table_residual_honesty() {
        assert!(honesty_translate_copy_escape_table());
        assert_eq!(translate_copy_residual(r"Hello\nWorld"), "Hello\nWorld");
        assert_eq!(translate_copy_residual(r"Tab\tX"), "Tab\tX");
        assert_eq!(translate_copy_residual(r"A\\B"), "A\\B");
        assert_eq!(translate_copy_residual("Q\\\"M"), "Q\"M");
        assert_eq!(translate_copy_residual(r"A\'B"), "A'B");
        assert_eq!(translate_copy_residual(r"\?"), "?");
        // Default residual: keep escaped char.
        assert_eq!(translate_copy_residual(r"\x"), "x");
        // Identity residual.
        assert_eq!(translate_copy_residual("BACK"), "BACK");
        assert_eq!(translate_copy_residual("$%d"), "$%d");
        // Multi-line residual used by CSF/STR caption paths.
        assert_eq!(translate_copy_residual(r"Line1\nLine2"), "Line1\nLine2");
        assert!(translate_copy_residual("").is_empty());
    }
}
