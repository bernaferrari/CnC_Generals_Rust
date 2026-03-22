//! INI parser for InGameUI settings
//!
//! Corresponds to C++ INI::parseInGameUIDefinition in InGameUI.cpp
//! Parses UI configuration for in-game elements like messages, captions, and superweapon countdowns.

use crate::common::ini::{ini, FieldParse, INIError, INIResult, INI};
use std::sync::{OnceLock, RwLock};

/// In-game UI settings singleton
static INGAME_UI_SETTINGS: OnceLock<RwLock<InGameUISettings>> = OnceLock::new();

/// 2D integer coordinate
#[derive(Debug, Clone, Copy, Default)]
pub struct ICoord2D {
    pub x: i32,
    pub y: i32,
}

/// 2D floating-point coordinate
#[derive(Debug, Clone, Copy, Default)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

/// RGBA color as integers (0-255)
#[derive(Debug, Clone, Copy, Default)]
pub struct RGBAColorInt {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
    pub alpha: u8,
}

impl RGBAColorInt {
    pub fn new(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self {
            red,
            green,
            blue,
            alpha,
        }
    }
}

/// In-game UI settings structure
/// Matches C++ InGameUI class members parsed from INI
#[derive(Debug, Clone)]
pub struct InGameUISettings {
    // Selection settings
    pub max_selection_size: i32,

    // Message display settings
    pub message_color1: u32,
    pub message_color2: u32,
    pub message_position: ICoord2D,
    pub message_font: String,
    pub message_point_size: i32,
    pub message_bold: bool,
    pub message_delay_ms: i32,

    // Military caption settings
    pub military_caption_color: RGBAColorInt,
    pub military_caption_position: ICoord2D,
    pub military_caption_title_font: String,
    pub military_caption_title_point_size: i32,
    pub military_caption_title_bold: bool,
    pub military_caption_font: String,
    pub military_caption_point_size: i32,
    pub military_caption_bold: bool,
    pub military_caption_randomize_typing: bool,
    pub military_caption_speed: i32,

    // Superweapon countdown settings
    pub superweapon_position: Coord2D,
    pub superweapon_flash_duration: f32,
    pub superweapon_flash_color: u32,
    pub superweapon_normal_font: String,
    pub superweapon_normal_point_size: i32,
    pub superweapon_normal_bold: bool,
    pub superweapon_ready_font: String,
    pub superweapon_ready_point_size: i32,
    pub superweapon_ready_bold: bool,

    // Named timer countdown settings
    pub named_timer_position: Coord2D,
    pub named_timer_flash_duration: f32,
    pub named_timer_flash_color: u32,
    pub named_timer_normal_font: String,
    pub named_timer_normal_point_size: i32,
    pub named_timer_normal_bold: bool,
    pub named_timer_normal_color: u32,
    pub named_timer_ready_font: String,
    pub named_timer_ready_point_size: i32,
    pub named_timer_ready_bold: bool,
    pub named_timer_ready_color: u32,
}

impl Default for InGameUISettings {
    fn default() -> Self {
        Self {
            // Selection defaults
            max_selection_size: -1, // -1 means no limit

            // Message defaults (matching C++ constructor)
            message_color1: make_color(255, 255, 255, 255),
            message_color2: make_color(180, 180, 180, 255),
            message_position: ICoord2D { x: 10, y: 10 },
            message_font: "Arial".to_string(),
            message_point_size: 10,
            message_bold: false,
            message_delay_ms: 5000,

            // Military caption defaults
            military_caption_color: RGBAColorInt::new(200, 200, 30, 255),
            military_caption_position: ICoord2D { x: 10, y: 380 },
            military_caption_title_font: "Courier".to_string(),
            military_caption_title_point_size: 12,
            military_caption_title_bold: true,
            military_caption_font: "Courier".to_string(),
            military_caption_point_size: 12,
            military_caption_bold: false,
            military_caption_randomize_typing: false,
            military_caption_speed: 1,

            // Superweapon countdown defaults
            superweapon_position: Coord2D::default(),
            superweapon_flash_duration: 0.0,
            superweapon_flash_color: 0,
            superweapon_normal_font: String::new(),
            superweapon_normal_point_size: 0,
            superweapon_normal_bold: false,
            superweapon_ready_font: String::new(),
            superweapon_ready_point_size: 0,
            superweapon_ready_bold: false,

            // Named timer defaults
            named_timer_position: Coord2D::default(),
            named_timer_flash_duration: 0.0,
            named_timer_flash_color: 0,
            named_timer_normal_font: String::new(),
            named_timer_normal_point_size: 0,
            named_timer_normal_bold: false,
            named_timer_normal_color: 0,
            named_timer_ready_font: String::new(),
            named_timer_ready_point_size: 0,
            named_timer_ready_bold: false,
            named_timer_ready_color: 0,
        }
    }
}

/// Make a color value from RGBA components (matching C++ GameMakeColor)
fn make_color(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Field parse table for InGameUI settings
/// Matches C++ s_fieldParseTable in InGameUI.cpp
const INGAME_UI_FIELD_PARSE_TABLE: &[FieldParse<InGameUISettings>] = &[
    FieldParse {
        token: "MaxSelectionSize",
        parse: parse_max_selection_size,
    },
    FieldParse {
        token: "MessageColor1",
        parse: parse_message_color1,
    },
    FieldParse {
        token: "MessageColor2",
        parse: parse_message_color2,
    },
    FieldParse {
        token: "MessagePosition",
        parse: parse_message_position,
    },
    FieldParse {
        token: "MessageFont",
        parse: parse_message_font,
    },
    FieldParse {
        token: "MessagePointSize",
        parse: parse_message_point_size,
    },
    FieldParse {
        token: "MessageBold",
        parse: parse_message_bold,
    },
    FieldParse {
        token: "MessageDelayMS",
        parse: parse_message_delay_ms,
    },
    FieldParse {
        token: "MilitaryCaptionColor",
        parse: parse_military_caption_color,
    },
    FieldParse {
        token: "MilitaryCaptionPosition",
        parse: parse_military_caption_position,
    },
    FieldParse {
        token: "MilitaryCaptionTitleFont",
        parse: parse_military_caption_title_font,
    },
    FieldParse {
        token: "MilitaryCaptionTitlePointSize",
        parse: parse_military_caption_title_point_size,
    },
    FieldParse {
        token: "MilitaryCaptionTitleBold",
        parse: parse_military_caption_title_bold,
    },
    FieldParse {
        token: "MilitaryCaptionFont",
        parse: parse_military_caption_font,
    },
    FieldParse {
        token: "MilitaryCaptionPointSize",
        parse: parse_military_caption_point_size,
    },
    FieldParse {
        token: "MilitaryCaptionBold",
        parse: parse_military_caption_bold,
    },
    FieldParse {
        token: "MilitaryCaptionRandomizeTyping",
        parse: parse_military_caption_randomize_typing,
    },
    FieldParse {
        token: "MilitaryCaptionSpeed",
        parse: parse_military_caption_speed,
    },
    FieldParse {
        token: "SuperweaponCountdownPosition",
        parse: parse_superweapon_position,
    },
    FieldParse {
        token: "SuperweaponCountdownFlashDuration",
        parse: parse_superweapon_flash_duration,
    },
    FieldParse {
        token: "SuperweaponCountdownFlashColor",
        parse: parse_superweapon_flash_color,
    },
    FieldParse {
        token: "SuperweaponCountdownNormalFont",
        parse: parse_superweapon_normal_font,
    },
    FieldParse {
        token: "SuperweaponCountdownNormalPointSize",
        parse: parse_superweapon_normal_point_size,
    },
    FieldParse {
        token: "SuperweaponCountdownNormalBold",
        parse: parse_superweapon_normal_bold,
    },
    FieldParse {
        token: "SuperweaponCountdownReadyFont",
        parse: parse_superweapon_ready_font,
    },
    FieldParse {
        token: "SuperweaponCountdownReadyPointSize",
        parse: parse_superweapon_ready_point_size,
    },
    FieldParse {
        token: "SuperweaponCountdownReadyBold",
        parse: parse_superweapon_ready_bold,
    },
    FieldParse {
        token: "NamedTimerCountdownPosition",
        parse: parse_named_timer_position,
    },
    FieldParse {
        token: "NamedTimerCountdownFlashDuration",
        parse: parse_named_timer_flash_duration,
    },
    FieldParse {
        token: "NamedTimerCountdownFlashColor",
        parse: parse_named_timer_flash_color,
    },
    FieldParse {
        token: "NamedTimerCountdownNormalFont",
        parse: parse_named_timer_normal_font,
    },
    FieldParse {
        token: "NamedTimerCountdownNormalPointSize",
        parse: parse_named_timer_normal_point_size,
    },
    FieldParse {
        token: "NamedTimerCountdownNormalBold",
        parse: parse_named_timer_normal_bold,
    },
    FieldParse {
        token: "NamedTimerCountdownNormalColor",
        parse: parse_named_timer_normal_color,
    },
    FieldParse {
        token: "NamedTimerCountdownReadyFont",
        parse: parse_named_timer_ready_font,
    },
    FieldParse {
        token: "NamedTimerCountdownReadyPointSize",
        parse: parse_named_timer_ready_point_size,
    },
    FieldParse {
        token: "NamedTimerCountdownReadyBold",
        parse: parse_named_timer_ready_bold,
    },
    FieldParse {
        token: "NamedTimerCountdownReadyColor",
        parse: parse_named_timer_ready_color,
    },
];

// Field parser functions

fn parse_max_selection_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.max_selection_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_message_color1(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.message_color1 = ini.parse_color_int()?;
    Ok(())
}

fn parse_message_color2(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.message_color2 = ini.parse_color_int()?;
    Ok(())
}

fn parse_message_position(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    let x = ini.scan_int_from_sub_token("X")?;
    let y = ini.scan_int_from_sub_token("Y")?;
    target.message_position = ICoord2D { x, y };
    Ok(())
}

fn parse_message_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.message_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_message_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.message_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_message_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.message_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_message_delay_ms(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.message_delay_ms = ini.parse_next_int()?;
    Ok(())
}

fn parse_military_caption_color(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    // Parse R: G: B: A: format
    let r = ini.scan_int_from_sub_token("R")? as u8;
    let g = ini.scan_int_from_sub_token("G")? as u8;
    let b = ini.scan_int_from_sub_token("B")? as u8;
    let a = ini.scan_int_from_sub_token("A")? as u8;
    target.military_caption_color = RGBAColorInt::new(r, g, b, a);
    Ok(())
}

fn parse_military_caption_position(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    let x = ini.scan_int_from_sub_token("X")?;
    let y = ini.scan_int_from_sub_token("Y")?;
    target.military_caption_position = ICoord2D { x, y };
    Ok(())
}

fn parse_military_caption_title_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_title_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_military_caption_title_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_title_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_military_caption_title_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_title_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_military_caption_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_military_caption_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_military_caption_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_military_caption_randomize_typing(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_randomize_typing = ini.parse_next_bool()?;
    Ok(())
}

fn parse_military_caption_speed(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.military_caption_speed = ini.parse_next_int()?;
    Ok(())
}

fn parse_superweapon_position(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    let x = ini.scan_real_from_sub_token("X")?;
    let y = ini.scan_real_from_sub_token("Y")?;
    target.superweapon_position = Coord2D { x, y };
    Ok(())
}

fn parse_superweapon_flash_duration(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    let token = ini.get_next_token().ok_or(INIError::InvalidData)?;
    target.superweapon_flash_duration = INI::parse_duration_real(&token)?;
    Ok(())
}

fn parse_superweapon_flash_color(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_flash_color = ini.parse_color_int()?;
    Ok(())
}

fn parse_superweapon_normal_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_normal_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_superweapon_normal_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_normal_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_superweapon_normal_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_normal_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_superweapon_ready_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_ready_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_superweapon_ready_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_ready_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_superweapon_ready_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.superweapon_ready_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_named_timer_position(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    let x = ini.scan_real_from_sub_token("X")?;
    let y = ini.scan_real_from_sub_token("Y")?;
    target.named_timer_position = Coord2D { x, y };
    Ok(())
}

fn parse_named_timer_flash_duration(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    let token = ini.get_next_token().ok_or(INIError::InvalidData)?;
    target.named_timer_flash_duration = INI::parse_duration_real(&token)?;
    Ok(())
}

fn parse_named_timer_flash_color(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_flash_color = ini.parse_color_int()?;
    Ok(())
}

fn parse_named_timer_normal_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_normal_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_named_timer_normal_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_normal_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_named_timer_normal_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_normal_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_named_timer_normal_color(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_normal_color = ini.parse_color_int()?;
    Ok(())
}

fn parse_named_timer_ready_font(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_ready_font = ini.parse_quoted_ascii_string()?;
    Ok(())
}

fn parse_named_timer_ready_point_size(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_ready_point_size = ini.parse_next_int()?;
    Ok(())
}

fn parse_named_timer_ready_bold(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_ready_bold = ini.parse_next_bool()?;
    Ok(())
}

fn parse_named_timer_ready_color(
    ini: &mut INI,
    target: &mut InGameUISettings,
    _tokens: &[&str],
) -> INIResult<()> {
    target.named_timer_ready_color = ini.parse_color_int()?;
    Ok(())
}

/// Initialize the InGameUI settings singleton
pub fn init_in_game_ui_settings() {
    INGAME_UI_SETTINGS.get_or_init(|| RwLock::new(InGameUISettings::default()));
}

/// Get a read reference to the InGameUI settings
pub fn get_in_game_ui_settings() -> Option<std::sync::RwLockReadGuard<'static, InGameUISettings>> {
    INGAME_UI_SETTINGS.get()?.read().ok()
}

/// Get a write reference to the InGameUI settings
pub fn get_in_game_ui_settings_mut(
) -> Option<std::sync::RwLockWriteGuard<'static, InGameUISettings>> {
    INGAME_UI_SETTINGS.get()?.write().ok()
}

/// Parse the InGameUI definition block
/// C++ equivalent: INI::parseInGameUIDefinition
pub fn parse_in_game_ui_definition(ini: &mut INI) -> INIResult<()> {
    init_in_game_ui_settings();

    let mut settings = if let Some(mut guard) = get_in_game_ui_settings_mut() {
        std::mem::take(&mut *guard)
    } else {
        InGameUISettings::default()
    };

    ini.init_from_ini_with_fields_allow_unknown(&mut settings, INGAME_UI_FIELD_PARSE_TABLE)?;

    if let Some(mut guard) = get_in_game_ui_settings_mut() {
        *guard = settings;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_settings() {
        let settings = InGameUISettings::default();
        assert_eq!(settings.max_selection_size, -1);
        assert_eq!(settings.message_position.x, 10);
        assert_eq!(settings.message_position.y, 10);
        assert_eq!(settings.message_font, "Arial");
        assert!(!settings.message_bold);
        assert_eq!(settings.message_delay_ms, 5000);
    }

    #[test]
    fn test_make_color() {
        let color = make_color(255, 128, 64, 32);
        assert_eq!(color, 0x20FF8040);
    }
}
