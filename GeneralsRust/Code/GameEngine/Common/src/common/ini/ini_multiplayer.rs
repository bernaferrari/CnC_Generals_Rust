////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

//! FILE: ini_multiplayer.rs
//! Author: Matthew D. Campbell, January 2002 (Converted to Rust)
//! Desc:   Parsing MultiplayerSettings and MultiplayerColor INI entries

use once_cell::sync::OnceCell;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::common::ascii_string::AsciiString;
use crate::common::ini::ini::{
    FieldParse, INIError, INILoadType as INIParseLoadType, INIResult, INI,
};
use crate::common::rts::money::Money;
use crate::debug_assert_crash;

/// Load types for INI parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IniLoadType {
    Invalid,
    Overwrite,
    CreateOverrides,
    Multifile,
}

/// Multiplayer starting money settings
#[derive(Debug, Clone)]
pub struct MultiplayerStartingMoneySettings {
    pub money: Money,
    pub is_default: bool,
}

impl Default for MultiplayerStartingMoneySettings {
    fn default() -> Self {
        Self {
            money: Money::default(),
            is_default: false,
        }
    }
}

/// Multiplayer color definition
#[derive(Debug, Clone)]
pub struct MultiplayerColorDefinition {
    pub name: AsciiString,
    pub tooltip_name: AsciiString,
    pub rgb_value: u32,
    pub rgb_night_value: u32,
    pub color: u32,
    pub night_color: u32,
}

impl MultiplayerColorDefinition {
    pub fn new(name: AsciiString) -> Self {
        Self {
            name,
            tooltip_name: AsciiString::new(),
            rgb_value: 0,
            rgb_night_value: 0,
            color: 0,
            night_color: 0,
        }
    }

    pub fn get_rgb_value(&self) -> u32 {
        self.rgb_value
    }

    pub fn get_rgb_night_value(&self) -> u32 {
        self.rgb_night_value
    }

    pub fn set_color(&mut self, color: u32) {
        self.color = color;
    }

    pub fn set_night_color(&mut self, night_color: u32) {
        self.night_color = night_color;
    }

    pub fn get_color(&self) -> u32 {
        self.color
    }

    pub fn set_tooltip_name(&mut self, name: AsciiString) {
        self.tooltip_name = name;
    }

    pub fn get_tooltip_name(&self) -> &AsciiString {
        &self.tooltip_name
    }

    fn matches_name(&self, name: &AsciiString) -> bool {
        if !self.tooltip_name.is_empty() {
            &self.tooltip_name == name
        } else {
            &self.name == name
        }
    }
}

/// Multiplayer settings container
/// Matches C++ MultiplayerSettings from MultiplayerSettings.h
/// Field parse table from MultiplayerSettings.cpp lines 32-44
#[derive(Debug)]
pub struct MultiplayerSettings {
    pub color_definitions: Vec<MultiplayerColorDefinition>,
    pub starting_money_choices: Vec<MultiplayerStartingMoneySettings>,

    // Fields from C++ MultiplayerSettings (MultiplayerSettings.h lines 102-118)
    pub start_countdown_timer_seconds: i32, // StartCountdownTimer
    pub max_beacons_per_player: i32,        // MaxBeaconsPerPlayer
    pub is_shroud_in_multiplayer: bool,     // UseShroud
    pub show_random_player_template: bool,  // ShowRandomPlayerTemplate
    pub show_random_start_pos: bool,        // ShowRandomStartPos
    pub show_random_color: bool,            // ShowRandomColor
    pub num_colors: i32,
}

impl MultiplayerSettings {
    pub fn new() -> Self {
        Self {
            color_definitions: Vec::new(),
            starting_money_choices: Vec::new(),
            // Default values from C++ MultiplayerSettings::MultiplayerSettings() lines 48-59
            start_countdown_timer_seconds: 0,
            max_beacons_per_player: 3,
            is_shroud_in_multiplayer: true,
            show_random_player_template: true,
            show_random_start_pos: true,
            show_random_color: true,
            num_colors: 0,
        }
    }

    pub fn find_multiplayer_color_definition_by_name(
        &self,
        name: &AsciiString,
    ) -> Option<&MultiplayerColorDefinition> {
        self.color_definitions
            .iter()
            .find(|def| def.matches_name(name))
    }

    pub fn find_multiplayer_color_definition_by_name_mut(
        &mut self,
        name: &AsciiString,
    ) -> Option<&mut MultiplayerColorDefinition> {
        self.color_definitions
            .iter_mut()
            .find(|def| def.matches_name(name))
    }

    pub fn new_multiplayer_color_definition(
        &mut self,
        name: AsciiString,
    ) -> &mut MultiplayerColorDefinition {
        let definition = MultiplayerColorDefinition::new(name);
        self.color_definitions.push(definition);
        self.color_definitions.last_mut().unwrap()
    }

    pub fn add_starting_money_choice(&mut self, money: Money, is_default: bool) {
        let settings = MultiplayerStartingMoneySettings { money, is_default };
        self.starting_money_choices.push(settings);
    }

    pub fn get_num_colors(&self) -> i32 {
        self.color_definitions.len() as i32
    }

    pub fn get_color_value(&self, index: i32) -> Option<u32> {
        if index < 0 {
            return None;
        }

        let index = index as usize;
        self.color_definitions.get(index).map(|def| def.get_color())
    }

    pub fn get_color_value_by_name(&self, name: &str) -> Option<u32> {
        let key = AsciiString::from(name);
        self.find_multiplayer_color_definition_by_name(&key)
            .map(|def| def.get_color())
    }
}

impl Default for MultiplayerSettings {
    fn default() -> Self {
        Self::new()
    }
}

const MULTIPLAYER_SETTINGS_FIELDS: &[FieldParse<MultiplayerSettings>] = &[
    FieldParse {
        token: "StartCountdownTimer",
        parse: parse_start_countdown_timer,
    },
    FieldParse {
        token: "MaxBeaconsPerPlayer",
        parse: parse_max_beacons_per_player,
    },
    FieldParse {
        token: "UseShroud",
        parse: parse_use_shroud,
    },
    FieldParse {
        token: "ShowRandomPlayerTemplate",
        parse: parse_show_random_player_template,
    },
    FieldParse {
        token: "ShowRandomStartPos",
        parse: parse_show_random_start_pos,
    },
    FieldParse {
        token: "ShowRandomColor",
        parse: parse_show_random_color,
    },
];

const MULTIPLAYER_COLOR_FIELDS: &[FieldParse<MultiplayerColorDefinition>] = &[
    FieldParse {
        token: "TooltipName",
        parse: parse_multiplayer_color_tooltip_name,
    },
    FieldParse {
        token: "RGBColor",
        parse: parse_multiplayer_color_rgb,
    },
    FieldParse {
        token: "RGBNightColor",
        parse: parse_multiplayer_color_rgb_night,
    },
];

const STARTING_MONEY_FIELDS: &[FieldParse<MultiplayerStartingMoneySettings>] = &[
    FieldParse {
        token: "Value",
        parse: parse_starting_money_value,
    },
    FieldParse {
        token: "Default",
        parse: parse_starting_money_default,
    },
];

fn parse_start_countdown_timer(
    _ini: &mut INI,
    settings: &mut MultiplayerSettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.start_countdown_timer_seconds = INI::parse_int(value)?;
    Ok(())
}

fn parse_max_beacons_per_player(
    _ini: &mut INI,
    settings: &mut MultiplayerSettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.max_beacons_per_player = INI::parse_int(value)?;
    Ok(())
}

fn parse_use_shroud(
    _ini: &mut INI,
    settings: &mut MultiplayerSettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.is_shroud_in_multiplayer = INI::parse_bool(value)?;
    Ok(())
}

fn parse_show_random_player_template(
    _ini: &mut INI,
    settings: &mut MultiplayerSettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.show_random_player_template = INI::parse_bool(value)?;
    Ok(())
}

fn parse_show_random_start_pos(
    _ini: &mut INI,
    settings: &mut MultiplayerSettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.show_random_start_pos = INI::parse_bool(value)?;
    Ok(())
}

fn parse_show_random_color(
    _ini: &mut INI,
    settings: &mut MultiplayerSettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.show_random_color = INI::parse_bool(value)?;
    Ok(())
}

fn parse_multiplayer_color_tooltip_name(
    _ini: &mut INI,
    color: &mut MultiplayerColorDefinition,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    let parsed = INI::parse_ascii_string(value)?;
    color.set_tooltip_name(AsciiString::from(&parsed));
    Ok(())
}

fn parse_rgb_color_value(args: &[&str]) -> INIResult<u32> {
    let (r, g, b) = INI::parse_rgb_color(args)?;
    let r = (r * 255.0).round().clamp(0.0, 255.0) as u32;
    let g = (g * 255.0).round().clamp(0.0, 255.0) as u32;
    let b = (b * 255.0).round().clamp(0.0, 255.0) as u32;
    Ok((r << 16) | (g << 8) | b)
}

fn parse_multiplayer_color_rgb(
    _ini: &mut INI,
    color: &mut MultiplayerColorDefinition,
    args: &[&str],
) -> INIResult<()> {
    color.rgb_value = parse_rgb_color_value(args)?;
    Ok(())
}

fn parse_multiplayer_color_rgb_night(
    _ini: &mut INI,
    color: &mut MultiplayerColorDefinition,
    args: &[&str],
) -> INIResult<()> {
    color.rgb_night_value = parse_rgb_color_value(args)?;
    Ok(())
}

fn parse_starting_money_value(
    _ini: &mut INI,
    settings: &mut MultiplayerStartingMoneySettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    let amount = INI::parse_unsigned_int(value)?;
    settings.money = Money::new_with_amount(amount);
    Ok(())
}

fn parse_starting_money_default(
    _ini: &mut INI,
    settings: &mut MultiplayerStartingMoneySettings,
    args: &[&str],
) -> INIResult<()> {
    let value = args.first().ok_or(INIError::InvalidData)?;
    settings.is_default = INI::parse_bool(value)?;
    Ok(())
}

/// Global multiplayer settings instance (would be managed by a global state manager in practice)
static MULTIPLAYER_SETTINGS: OnceCell<RwLock<MultiplayerSettings>> = OnceCell::new();

fn multiplayer_settings_cell() -> &'static RwLock<MultiplayerSettings> {
    MULTIPLAYER_SETTINGS.get_or_init(|| RwLock::new(MultiplayerSettings::new()))
}

fn multiplayer_settings_mut() -> RwLockWriteGuard<'static, MultiplayerSettings> {
    multiplayer_settings_cell()
        .write()
        .expect("MultiplayerSettings poisoned")
}

fn multiplayer_settings() -> RwLockReadGuard<'static, MultiplayerSettings> {
    multiplayer_settings_cell()
        .read()
        .expect("MultiplayerSettings poisoned")
}

pub fn with_multiplayer_settings<R>(f: impl FnOnce(&MultiplayerSettings) -> R) -> R {
    let settings = multiplayer_settings();
    f(&settings)
}

/// Field parse table for starting money settings
pub struct StartingMoneyFieldParseTable;

impl StartingMoneyFieldParseTable {
    pub fn get_table() -> Vec<(
        &'static str,
        fn(&str) -> Result<Box<dyn std::any::Any>, String>,
    )> {
        vec![
            ("Value", |value| {
                Money::parse_money_amount(value)
                    .map(|m| Box::new(m) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse money value: {}", e))
            }),
            ("Default", |value| {
                parse_bool(value)
                    .map(|b| Box::new(b) as Box<dyn std::any::Any>)
                    .map_err(|e| format!("Failed to parse bool value: {}", e))
            }),
        ]
    }
}

/// Parse a boolean value from string
pub fn parse_bool(value: &str) -> Result<bool, String> {
    match value.trim().to_lowercase().as_str() {
        "true" | "yes" | "1" => Ok(true),
        "false" | "no" | "0" => Ok(false),
        _ => Err(format!("Invalid boolean value: {}", value)),
    }
}

pub fn parse_multiplayer_settings_definition(ini: &mut INI) -> INIResult<()> {
    if ini.get_load_type() == INIParseLoadType::CreateOverrides {
        return Err(INIError::InvalidData);
    }

    let mut settings = multiplayer_settings_mut();
    ini.init_from_ini_with_fields(&mut *settings, MULTIPLAYER_SETTINGS_FIELDS)?;
    Ok(())
}

pub fn parse_multiplayer_color_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?;

    let mut settings = multiplayer_settings_mut();
    let color_definition = if settings
        .find_multiplayer_color_definition_by_name(&AsciiString::from(name))
        .is_some()
    {
        settings
            .find_multiplayer_color_definition_by_name_mut(&AsciiString::from(name))
            .unwrap()
    } else {
        settings.new_multiplayer_color_definition(AsciiString::from(name))
    };

    ini.init_from_ini_with_fields(color_definition, MULTIPLAYER_COLOR_FIELDS)?;
    color_definition.set_color(color_definition.get_rgb_value());
    color_definition.set_night_color(color_definition.get_rgb_night_value());
    Ok(())
}

pub fn parse_multiplayer_starting_money_choice_definition(ini: &mut INI) -> INIResult<()> {
    if ini.get_load_type() == INIParseLoadType::CreateOverrides {
        return Err(INIError::InvalidData);
    }

    let mut settings = MultiplayerStartingMoneySettings::default();
    ini.init_from_ini_with_fields(&mut settings, STARTING_MONEY_FIELDS)?;

    let mut multiplayer_settings = multiplayer_settings_mut();
    multiplayer_settings.add_starting_money_choice(settings.money, settings.is_default);
    Ok(())
}

/// INI parsing functions
pub struct IniMultiplayer;

impl IniMultiplayer {
    /// Parse MultiplayerSettings definition
    pub fn parse_multiplayer_settings_definition(load_type: IniLoadType) -> Result<(), String> {
        if load_type == IniLoadType::CreateOverrides {
            debug_assert_crash!(false, "Creating an override of MultiplayerSettings!");
            return Err("Override creation not supported for MultiplayerSettings".to_string());
        }

        let _guard = multiplayer_settings_mut();
        // Original implementation would populate the structure via INI parsing.
        Ok(())
    }

    /// Parse MultiplayerColor definition
    pub fn parse_multiplayer_color_definition(name: AsciiString) -> Result<(), String> {
        let mut settings = multiplayer_settings_mut();

        let color_definition = if settings
            .find_multiplayer_color_definition_by_name(&name)
            .is_some()
        {
            settings
                .find_multiplayer_color_definition_by_name_mut(&name)
                .unwrap()
        } else {
            settings.new_multiplayer_color_definition(name)
        };

        // In the original implementation this would parse INI data into the structure.
        color_definition.set_color(color_definition.get_rgb_value());
        color_definition.set_night_color(color_definition.get_rgb_night_value());

        Ok(())
    }

    /// Parse MultiplayerStartingMoneyChoice definition
    pub fn parse_multiplayer_starting_money_choice_definition(
        load_type: IniLoadType,
    ) -> Result<(), String> {
        if load_type == IniLoadType::CreateOverrides {
            debug_assert_crash!(
                false,
                "Overrides not supported for MultiplayerStartingMoneyChoice"
            );
            return Err("Overrides not supported for MultiplayerStartingMoneyChoice".to_string());
        }

        // Temporary data store
        let settings = MultiplayerStartingMoneySettings::default();

        // Parse the ini definition would happen here
        // In the original C++, this calls ini->initFromINI(&settings, startingMoneyFieldParseTable)

        let mut multiplayer_settings = multiplayer_settings_mut();
        multiplayer_settings.add_starting_money_choice(settings.money, settings.is_default);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_bool() {
        assert_eq!(parse_bool("true"), Ok(true));
        assert_eq!(parse_bool("True"), Ok(true));
        assert_eq!(parse_bool("YES"), Ok(true));
        assert_eq!(parse_bool("1"), Ok(true));

        assert_eq!(parse_bool("false"), Ok(false));
        assert_eq!(parse_bool("False"), Ok(false));
        assert_eq!(parse_bool("NO"), Ok(false));
        assert_eq!(parse_bool("0"), Ok(false));

        assert!(parse_bool("invalid").is_err());
    }

    #[test]
    fn test_multiplayer_settings() {
        let mut settings = MultiplayerSettings::new();
        let name = AsciiString::from("TestColor");

        let color_def = settings.new_multiplayer_color_definition(name.clone());
        color_def.rgb_value = 0xFF0000;
        color_def.rgb_night_value = 0x800000;
        color_def.set_color(color_def.get_rgb_value());
        color_def.set_night_color(color_def.get_rgb_night_value());

        assert!(settings
            .find_multiplayer_color_definition_by_name(&name)
            .is_some());

        let money = Money::new_with_amount(1000);
        settings.add_starting_money_choice(money, true);
        assert_eq!(settings.starting_money_choices.len(), 1);
        assert!(settings.starting_money_choices[0].is_default);
    }
}
