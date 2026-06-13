//! INI parsing for Language definitions
//!
//! This module handles parsing Language block from INI files.
//! Language settings contain global font configuration and language-specific options.
//!
//! C++ Reference: GeneralsMD/Code/GameEngine/Include/GameClient/GlobalLanguage.h
//! C++ Parser: GeneralsMD/Code/GameEngine/Source/GameClient/GlobalLanguage.cpp
//!
//! Rust port: 2025

use crate::common::ini::ini::{FieldParse, INIError, INIResult, INI};
use once_cell::sync::OnceCell;
use parking_lot::RwLock;
use std::sync::Arc;

// ============================================================================
// FontDesc Structure
// ============================================================================

/// Font description structure
///
/// Simple structure used to hold font descriptions.
/// Matches C++ FontDesc from FontDesc.h
#[derive(Debug, Clone, PartialEq)]
pub struct FontDesc {
    /// Name of font (default: "Arial Unicode MS")
    pub name: String,
    /// Point size (default: 12)
    pub size: i32,
    /// Is bold? (default: false)
    pub bold: bool,
}

impl Default for FontDesc {
    fn default() -> Self {
        Self {
            name: "Arial Unicode MS".to_string(),
            size: 12,
            bold: false,
        }
    }
}

impl FontDesc {
    /// Create a new FontDesc with the specified parameters
    pub fn new(name: &str, size: i32, bold: bool) -> Self {
        Self {
            name: name.to_string(),
            size,
            bold,
        }
    }

    /// Parse FontDesc from INI tokens
    ///
    /// Format: "FontName" Size Bold
    /// Example: "Arial Unicode MS" 12 Yes
    pub fn parse_from_tokens(_ini: &mut INI, tokens: &[&str]) -> INIResult<Self> {
        let mut font_desc = FontDesc::default();

        if tokens.is_empty() {
            return Err(INIError::InvalidData);
        }

        let name_token = tokens[0];
        let remaining_start = if name_token.starts_with('"') && name_token.ends_with('"') {
            font_desc.name = name_token[1..name_token.len() - 1].to_string();
            1
        } else if name_token.starts_with('"') {
            let mut name_parts = vec![&name_token[1..]];
            let mut idx = 1;
            while idx < tokens.len() {
                let part = tokens[idx];
                if part.ends_with('"') {
                    name_parts.push(&part[..part.len() - 1]);
                    idx += 1;
                    break;
                }
                name_parts.push(part);
                idx += 1;
            }
            if idx == tokens.len() && !tokens.last().is_some_and(|part| part.ends_with('"')) {
                return Err(INIError::InvalidData);
            }
            font_desc.name = name_parts.join(" ");
            idx
        } else {
            font_desc.name = name_token.to_string();
            1
        };

        if remaining_start + 1 >= tokens.len() {
            return Err(INIError::InvalidData);
        }

        font_desc.size = INI::parse_int(tokens[remaining_start])?;
        font_desc.bold = parse_cpp_yes_no(tokens[remaining_start + 1])?;

        Ok(font_desc)
    }
}

fn parse_cpp_yes_no(token: &str) -> INIResult<bool> {
    if token.eq_ignore_ascii_case("yes") {
        Ok(true)
    } else if token.eq_ignore_ascii_case("no") {
        Ok(false)
    } else {
        Err(INIError::InvalidData)
    }
}

fn parse_required_ascii_string_token(tokens: &[&str]) -> INIResult<String> {
    if tokens.is_empty() {
        return Err(INIError::InvalidData);
    }

    let token = tokens[0];
    if token.starts_with('"') && token.ends_with('"') {
        Ok(token[1..token.len() - 1].to_string())
    } else {
        Ok(token.to_string())
    }
}

fn parse_required_i32_token(tokens: &[&str]) -> INIResult<i32> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_int(token)
}

fn parse_required_f32_token(tokens: &[&str]) -> INIResult<f32> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    INI::parse_real(token)
}

fn parse_required_cpp_bool_token(tokens: &[&str]) -> INIResult<bool> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    parse_cpp_yes_no(token)
}

// ============================================================================
// GlobalLanguage Structure
// ============================================================================

/// Global language settings structure
///
/// Contains all global font and language configuration for the game engine.
/// This matches the C++ GlobalLanguage class from GlobalLanguage.h
#[derive(Debug, Clone)]
pub struct GlobalLanguage {
    // Unicode font settings
    pub unicode_font_name: String,
    pub unicode_font_file_name: String,
    pub use_hard_wrap: bool,

    // Caption settings
    pub military_caption_speed: i32,
    pub military_caption_delay_ms: i32,

    // Font resolution adjustment
    pub resolution_font_size_adjustment: f32,

    // Font descriptions for various UI elements
    pub copyright_font: FontDesc,
    pub message_font: FontDesc,
    pub military_caption_title_font: FontDesc,
    pub military_caption_font: FontDesc,
    pub superweapon_countdown_normal_font: FontDesc,
    pub superweapon_countdown_ready_font: FontDesc,
    pub named_timer_countdown_normal_font: FontDesc,
    pub named_timer_countdown_ready_font: FontDesc,
    pub drawable_caption_font: FontDesc,
    pub default_window_font: FontDesc,
    pub default_display_string_font: FontDesc,
    pub tooltip_font: FontDesc,
    pub native_debug_display_font: FontDesc,
    pub draw_group_info_font: FontDesc,
    pub credits_title_font: FontDesc,
    pub credits_position_font: FontDesc,
    pub credits_normal_font: FontDesc,

    // List of local font file names to load
    pub local_fonts: Vec<String>,
}

impl Default for GlobalLanguage {
    fn default() -> Self {
        Self {
            unicode_font_name: String::new(),
            unicode_font_file_name: String::new(),
            use_hard_wrap: false,
            military_caption_speed: 0,
            military_caption_delay_ms: 750,
            resolution_font_size_adjustment: 0.7,
            copyright_font: FontDesc::default(),
            message_font: FontDesc::default(),
            military_caption_title_font: FontDesc::default(),
            military_caption_font: FontDesc::default(),
            superweapon_countdown_normal_font: FontDesc::default(),
            superweapon_countdown_ready_font: FontDesc::default(),
            named_timer_countdown_normal_font: FontDesc::default(),
            named_timer_countdown_ready_font: FontDesc::default(),
            drawable_caption_font: FontDesc::default(),
            default_window_font: FontDesc::default(),
            default_display_string_font: FontDesc::default(),
            tooltip_font: FontDesc::default(),
            native_debug_display_font: FontDesc::default(),
            draw_group_info_font: FontDesc::default(),
            credits_title_font: FontDesc::default(),
            credits_position_font: FontDesc::default(),
            credits_normal_font: FontDesc::default(),
            local_fonts: Vec::new(),
        }
    }
}

impl GlobalLanguage {
    /// Create a new GlobalLanguage with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Adjust font size for resolution
    ///
    /// This matches C++ GlobalLanguage::adjustFontSize
    pub fn adjust_font_size(&self, font_size: i32, x_resolution: f32) -> i32 {
        let adjust_factor = x_resolution / 800.0;
        let adjust_factor = 1.0 + (adjust_factor - 1.0) * self.resolution_font_size_adjustment;
        let adjust_factor = adjust_factor.clamp(1.0, 2.0);
        (font_size as f32 * adjust_factor).floor() as i32
    }
}

// ============================================================================
// Global Language Instance Management
// ============================================================================

static GLOBAL_LANGUAGE: OnceCell<Arc<RwLock<GlobalLanguage>>> = OnceCell::new();

/// Initialize the global language settings
pub fn init_global_language() {
    let _ = GLOBAL_LANGUAGE.get_or_init(|| Arc::new(RwLock::new(GlobalLanguage::new())));
}

/// Get a read reference to the global language settings
pub fn get_global_language() -> Option<Arc<RwLock<GlobalLanguage>>> {
    GLOBAL_LANGUAGE.get().cloned()
}

/// Get a read guard to the global language settings
pub fn get_global_language_read() -> Option<parking_lot::RwLockReadGuard<'static, GlobalLanguage>> {
    GLOBAL_LANGUAGE.get().map(|g| g.read())
}

/// Get a write guard to the global language settings
pub fn get_global_language_write() -> Option<parking_lot::RwLockWriteGuard<'static, GlobalLanguage>>
{
    GLOBAL_LANGUAGE.get().map(|g| g.write())
}

// ============================================================================
// Field Parse Functions
// ============================================================================

/// Parse UnicodeFontName field
fn parse_unicode_font_name(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.unicode_font_name = parse_required_ascii_string_token(tokens)?;
    Ok(())
}

/// Parse LocalFontFile field (adds to list)
fn parse_local_font_file(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    let font_file = parse_required_ascii_string_token(tokens)?;
    target.local_fonts.insert(0, font_file);
    Ok(())
}

/// Parse MilitaryCaptionSpeed field
fn parse_military_caption_speed(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_speed = parse_required_i32_token(tokens)?;
    Ok(())
}

/// Parse UseHardWordWrap field
fn parse_use_hard_wrap(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.use_hard_wrap = parse_required_cpp_bool_token(tokens)?;
    Ok(())
}

/// Parse ResolutionFontAdjustment field
fn parse_resolution_font_adjustment(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.resolution_font_size_adjustment = parse_required_f32_token(tokens)?;
    Ok(())
}

/// Parse MilitaryCaptionDelayMS field
fn parse_military_caption_delay_ms(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_delay_ms = parse_required_i32_token(tokens)?;
    Ok(())
}

/// Generic FontDesc parser factory
fn make_font_parser<F>(
    field_setter: F,
) -> impl Fn(&mut INI, &mut GlobalLanguage, &[&str]) -> INIResult<()>
where
    F: Fn(&mut GlobalLanguage, FontDesc) + 'static,
{
    move |ini: &mut INI, target: &mut GlobalLanguage, tokens: &[&str]| {
        let font_desc = FontDesc::parse_from_tokens(ini, tokens)?;
        field_setter(target, font_desc);
        Ok(())
    }
}

/// Parse CopyrightFont field
fn parse_copyright_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.copyright_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse MessageFont field
fn parse_message_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.message_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse MilitaryCaptionTitleFont field
fn parse_military_caption_title_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_title_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse MilitaryCaptionFont field
fn parse_military_caption_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse SuperweaponCountdownNormalFont field
fn parse_superweapon_countdown_normal_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_countdown_normal_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse SuperweaponCountdownReadyFont field
fn parse_superweapon_countdown_ready_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_countdown_ready_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse NamedTimerCountdownNormalFont field
fn parse_named_timer_countdown_normal_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_countdown_normal_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse NamedTimerCountdownReadyFont field
fn parse_named_timer_countdown_ready_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_countdown_ready_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse DrawableCaptionFont field
fn parse_drawable_caption_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.drawable_caption_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse DefaultWindowFont field
fn parse_default_window_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.default_window_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse DefaultDisplayStringFont field
fn parse_default_display_string_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.default_display_string_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse TooltipFontName field
fn parse_tooltip_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.tooltip_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse NativeDebugDisplay field
fn parse_native_debug_display_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.native_debug_display_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse DrawGroupInfoFont field
fn parse_draw_group_info_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.draw_group_info_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse CreditsTitleFont field
fn parse_credits_title_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.credits_title_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse CreditsMinorTitleFont (maps to credits_position_font) field
fn parse_credits_position_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.credits_position_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

/// Parse CreditsNormalFont field
fn parse_credits_normal_font(
    ini: &mut INI,
    target: &mut GlobalLanguage,
    tokens: &[&str],
) -> INIResult<()> {
    target.credits_normal_font = FontDesc::parse_from_tokens(ini, tokens)?;
    Ok(())
}

// ============================================================================
// Field Parse Table
// ============================================================================

/// Field parse table for Language block
///
/// Matches C++ TheGlobalLanguageDataFieldParseTable from GlobalLanguage.cpp
pub const LANGUAGE_FIELD_PARSE_TABLE: &[FieldParse<GlobalLanguage>] = &[
    FieldParse {
        token: "UnicodeFontName",
        parse: parse_unicode_font_name,
    },
    FieldParse {
        token: "LocalFontFile",
        parse: parse_local_font_file,
    },
    FieldParse {
        token: "MilitaryCaptionSpeed",
        parse: parse_military_caption_speed,
    },
    FieldParse {
        token: "UseHardWordWrap",
        parse: parse_use_hard_wrap,
    },
    FieldParse {
        token: "ResolutionFontAdjustment",
        parse: parse_resolution_font_adjustment,
    },
    FieldParse {
        token: "CopyrightFont",
        parse: parse_copyright_font,
    },
    FieldParse {
        token: "MessageFont",
        parse: parse_message_font,
    },
    FieldParse {
        token: "MilitaryCaptionTitleFont",
        parse: parse_military_caption_title_font,
    },
    FieldParse {
        token: "MilitaryCaptionDelayMS",
        parse: parse_military_caption_delay_ms,
    },
    FieldParse {
        token: "MilitaryCaptionFont",
        parse: parse_military_caption_font,
    },
    FieldParse {
        token: "SuperweaponCountdownNormalFont",
        parse: parse_superweapon_countdown_normal_font,
    },
    FieldParse {
        token: "SuperweaponCountdownReadyFont",
        parse: parse_superweapon_countdown_ready_font,
    },
    FieldParse {
        token: "NamedTimerCountdownNormalFont",
        parse: parse_named_timer_countdown_normal_font,
    },
    FieldParse {
        token: "NamedTimerCountdownReadyFont",
        parse: parse_named_timer_countdown_ready_font,
    },
    FieldParse {
        token: "DrawableCaptionFont",
        parse: parse_drawable_caption_font,
    },
    FieldParse {
        token: "DefaultWindowFont",
        parse: parse_default_window_font,
    },
    FieldParse {
        token: "DefaultDisplayStringFont",
        parse: parse_default_display_string_font,
    },
    FieldParse {
        token: "TooltipFontName",
        parse: parse_tooltip_font,
    },
    FieldParse {
        token: "NativeDebugDisplay",
        parse: parse_native_debug_display_font,
    },
    FieldParse {
        token: "DrawGroupInfoFont",
        parse: parse_draw_group_info_font,
    },
    FieldParse {
        token: "CreditsTitleFont",
        parse: parse_credits_title_font,
    },
    FieldParse {
        token: "CreditsMinorTitleFont",
        parse: parse_credits_position_font,
    },
    FieldParse {
        token: "CreditsNormalFont",
        parse: parse_credits_normal_font,
    },
];

// ============================================================================
// Block Parser
// ============================================================================

/// Parse Language block from INI file
///
/// This matches C++ INI::parseLanguageDefinition from GlobalLanguage.cpp
///
/// Example INI block:
/// ```ini
/// Language
///     UnicodeFontName = "Arial Unicode MS"
///     LocalFontFile = "custom_font.ttf"
///     MilitaryCaptionSpeed = 5
///     UseHardWordWrap = Yes
///     ResolutionFontAdjustment = 0.7
///     CopyrightFont = "Arial Unicode MS" 12 No
///     MessageFont = "Arial Unicode MS" 10 No
/// End
/// ```
pub fn parse_language_definition(ini: &mut INI) -> INIResult<()> {
    // Ensure global language is initialized
    init_global_language();

    // Get write access to global language
    let mut language = get_global_language_write().ok_or(INIError::InvalidData)?;

    // Parse fields from INI using the field parse table
    ini.init_from_ini_with_fields_allow_unknown(&mut *language, LANGUAGE_FIELD_PARSE_TABLE)?;

    Ok(())
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_font_desc_default() {
        let font = FontDesc::default();
        assert_eq!(font.name, "Arial Unicode MS");
        assert_eq!(font.size, 12);
        assert_eq!(font.bold, false);
    }

    #[test]
    fn test_font_desc_new() {
        let font = FontDesc::new("Custom Font", 14, true);
        assert_eq!(font.name, "Custom Font");
        assert_eq!(font.size, 14);
        assert_eq!(font.bold, true);
    }

    #[test]
    fn font_desc_parses_cpp_quoted_name_size_and_bold() {
        let mut ini = INI::new();
        let font =
            FontDesc::parse_from_tokens(&mut ini, &["\"Arial", "Unicode", "MS\"", "14", "Yes"])
                .expect("valid C++ font descriptor");

        assert_eq!(font, FontDesc::new("Arial Unicode MS", 14, true));
    }

    #[test]
    fn font_desc_rejects_missing_cpp_fields() {
        let mut ini = INI::new();

        assert!(FontDesc::parse_from_tokens(&mut ini, &[]).is_err());
        assert!(FontDesc::parse_from_tokens(&mut ini, &["\"Arial Unicode MS\""]).is_err());
        assert!(FontDesc::parse_from_tokens(&mut ini, &["\"Arial Unicode MS\"", "12"]).is_err());
    }

    #[test]
    fn font_desc_rejects_invalid_cpp_numeric_and_bool_fields() {
        let mut ini = INI::new();

        assert!(
            FontDesc::parse_from_tokens(&mut ini, &["\"Arial Unicode MS\"", "large", "No"])
                .is_err()
        );
        assert!(
            FontDesc::parse_from_tokens(&mut ini, &["\"Arial Unicode MS\"", "12", "false"])
                .is_err()
        );
        assert!(
            FontDesc::parse_from_tokens(&mut ini, &["\"Arial Unicode MS\"", "12", "1"]).is_err()
        );
    }

    #[test]
    fn language_bool_fields_use_cpp_yes_no_tokens() {
        let mut ini = INI::new();
        let mut lang = GlobalLanguage::default();

        parse_use_hard_wrap(&mut ini, &mut lang, &["Yes"]).expect("Yes is valid C++ bool");
        assert!(lang.use_hard_wrap);
        parse_use_hard_wrap(&mut ini, &mut lang, &["No"]).expect("No is valid C++ bool");
        assert!(!lang.use_hard_wrap);

        assert!(parse_use_hard_wrap(&mut ini, &mut lang, &["true"]).is_err());
        assert!(parse_use_hard_wrap(&mut ini, &mut lang, &["1"]).is_err());
    }

    #[test]
    fn local_font_file_matches_cpp_push_front_order() {
        let mut ini = INI::new();
        let mut lang = GlobalLanguage::default();

        parse_local_font_file(&mut ini, &mut lang, &["first.ttf"]).expect("first font");
        parse_local_font_file(&mut ini, &mut lang, &["second.ttf"]).expect("second font");

        assert_eq!(lang.local_fonts, vec!["second.ttf", "first.ttf"]);
    }

    #[test]
    fn test_global_language_default() {
        let lang = GlobalLanguage::default();
        assert_eq!(lang.unicode_font_name, "");
        assert_eq!(lang.use_hard_wrap, false);
        assert_eq!(lang.military_caption_speed, 0);
        assert_eq!(lang.military_caption_delay_ms, 750);
        assert!((lang.resolution_font_size_adjustment - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_adjust_font_size() {
        let lang = GlobalLanguage::default();

        // Test at 800 resolution (no adjustment)
        let size = lang.adjust_font_size(12, 800.0);
        assert_eq!(size, 12);

        // Test at 1600 resolution (should increase)
        let size = lang.adjust_font_size(12, 1600.0);
        assert!(size > 12);
        assert!(size <= 24); // max 2x

        // Test at 400 resolution (should not decrease below original)
        let size = lang.adjust_font_size(12, 400.0);
        assert_eq!(size, 12);
    }

    #[test]
    fn test_global_language_init() {
        init_global_language();
        assert!(get_global_language().is_some());
    }
}
