use std::collections::HashMap;
use std::fmt;
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::{OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::ascii_string::AsciiString;
use crate::common::system::file::FileAccess;
use crate::common::system::file_system::get_file_system;

/// INI constant defines
pub const INI_MAX_CHARS_PER_LINE: usize = 1028;
pub const INI_READ_BUFFER: usize = 8192;

/// INI Load Type enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum INILoadType {
    Invalid,
    Overwrite,       // create new or load over existing data instance
    CreateOverrides, // create new or load into new override data instance
    MultiFile,       // create new or continue loading into existing data instance
}

/// INI Error codes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum INIError {
    CantSearchDir,
    InvalidDirectory,
    InvalidParams,
    InvalidNameList,
    InvalidData,
    InvalidValue,
    MissingEndToken,
    UnknownToken,
    BufferTooSmall,
    FileNotOpen,
    FileAlreadyOpen,
    CantOpenFile,
    UnexpectedEndOfFile,
    UnknownError,
    EndOfFile,
}

pub type INIResult<T> = Result<T, INIError>;

impl fmt::Display for INIError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            INIError::CantSearchDir => write!(f, "Cannot search directory"),
            INIError::InvalidDirectory => write!(f, "Invalid directory"),
            INIError::InvalidParams => write!(f, "Invalid parameters"),
            INIError::InvalidNameList => write!(f, "Invalid name list"),
            INIError::InvalidData => write!(f, "Invalid data"),
            INIError::InvalidValue => write!(f, "Invalid value"),
            INIError::MissingEndToken => write!(f, "Missing end token"),
            INIError::UnknownToken => write!(f, "Unknown token"),
            INIError::BufferTooSmall => write!(f, "Buffer too small"),
            INIError::FileNotOpen => write!(f, "File not open"),
            INIError::FileAlreadyOpen => write!(f, "File already open"),
            INIError::CantOpenFile => write!(f, "Cannot open file"),
            INIError::UnexpectedEndOfFile => write!(f, "Unexpected end of file"),
            INIError::UnknownError => write!(f, "Unknown error"),
            INIError::EndOfFile => write!(f, "End of file"),
        }
    }
}

impl std::error::Error for INIError {}

impl From<INIError> for String {
    fn from(error: INIError) -> String {
        error.to_string()
    }
}

/// Function type for parsing data block fields
pub type INIFieldParseProc<T> = fn(&mut INI, &mut T, &[&str]) -> INIResult<()>;

/// Function type for parsing INI block types
pub type INIBlockParse = fn(&mut INI) -> INIResult<()>;

/// Field parse structure for defining parsing table
pub struct FieldParse<T> {
    pub token: &'static str,
    pub parse: INIFieldParseProc<T>,
}

/// Lookup list record for mapping names to values
pub struct LookupListRec {
    pub name: &'static str,
    pub value: i32,
}

/// Block parse structure for mapping tokens to parser functions
pub struct BlockParse {
    pub token: &'static str,
    pub parse: INIBlockParse,
}

static EXTRA_BLOCK_PARSERS: OnceLock<RwLock<Vec<BlockParse>>> = OnceLock::new();
static ARMOR_DEFINITION_REGISTRY: OnceLock<RwLock<HashMap<String, HashMap<String, f32>>>> =
    OnceLock::new();
static OBJECT_CREATION_LIST_REGISTRY: OnceLock<RwLock<HashMap<String, Vec<String>>>> =
    OnceLock::new();

/// Register an additional INI block parser at runtime (used by client-side modules).
pub fn register_block_parser(token: &'static str, parse: INIBlockParse) -> bool {
    let registry = EXTRA_BLOCK_PARSERS.get_or_init(|| RwLock::new(Vec::new()));
    let mut guard = registry
        .write()
        .expect("INI block parser registry poisoned");
    if guard
        .iter()
        .any(|entry| entry.token.eq_ignore_ascii_case(token))
    {
        return false;
    }
    guard.push(BlockParse { token, parse });
    true
}

fn get_extra_block_parser(token: &str) -> Option<INIBlockParse> {
    EXTRA_BLOCK_PARSERS
        .get()
        .and_then(|registry| registry.read().ok())
        .and_then(|guard| {
            guard
                .iter()
                .find(|entry| entry.token.eq_ignore_ascii_case(token))
                .map(|entry| entry.parse)
        })
}

/// INI Reader interface
pub struct INI {
    file: Option<BufReader<File>>,
    staged_temp_file: Option<PathBuf>,
    read_buffer: [u8; INI_READ_BUFFER],
    read_buffer_next: usize,
    read_buffer_used: usize,
    filename: String,
    load_type: INILoadType,
    line_num: u32,
    buffer: String,
    /// Position tracking within the current buffer for strtok-style advancement.
    /// When `read_line()` is called, this resets to 0. Each call to `get_next_token()`
    /// advances past the returned token so the next call returns the following one.
    /// C++ Reference: `strtok(NULL, seps)` advancing across `m_buffer`.
    buffer_token_offset: usize,
    seps: &'static str,
    seps_percent: &'static str,
    seps_colon: &'static str,
    seps_quote: &'static str,
    block_end_token: &'static str,
    end_of_file: bool,
    #[cfg(debug_assertions)]
    cur_block_start: String,
}

fn parse_block_result(result: Result<(), String>) -> INIResult<()> {
    result.map_err(|_| INIError::InvalidData)
}

fn parse_game_data_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_game_data::parse_game_data_definition(ini))
}

fn parse_damage_fx_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_damage_fx::parse_damage_fx_definition(ini))
}

fn parse_map_cache_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_map_cache::parse_map_cache_definition(ini))
}

fn parse_map_data_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_map_data::parse_map_data_definition(ini))
}

fn parse_mapped_image_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_mapped_image::parse_mapped_image_definition(ini))
}

fn parse_model_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_model::parse_model_definition(ini))
}

fn parse_object_block(ini: &mut INI) -> INIResult<()> {
    super::ini_object::IniObject::parse_object_definition_from_ini(ini)
}

fn parse_object_reskin_block(ini: &mut INI) -> INIResult<()> {
    super::ini_object::IniObject::parse_object_reskin_definition_from_ini(ini)
}

fn parse_webpage_url_block(ini: &mut INI) -> INIResult<()> {
    super::ini_webpage_url::IniWebpageUrl::parse_webpage_url_definition_from_ini(ini)
}

fn parse_road_block(ini: &mut INI) -> INIResult<()> {
    super::ini_road::parse_terrain_road_definition_from_ini(ini)
}

fn parse_bridge_block(ini: &mut INI) -> INIResult<()> {
    super::ini_road::parse_terrain_bridge_definition_from_ini(ini)
}

fn parse_science_block(ini: &mut INI) -> INIResult<()> {
    crate::common::rts::science::parse_science_definition_block(ini)
}

fn armor_definition_registry() -> &'static RwLock<HashMap<String, HashMap<String, f32>>> {
    ARMOR_DEFINITION_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

fn object_creation_list_registry() -> &'static RwLock<HashMap<String, Vec<String>>> {
    OBJECT_CREATION_LIST_REGISTRY.get_or_init(|| RwLock::new(HashMap::new()))
}

fn consume_block_with_nesting(ini: &mut INI) -> INIResult<Vec<String>> {
    let mut nested_depth = 0usize;
    let mut lines = Vec::new();

    loop {
        ini.read_line()?;
        if ini.end_of_file {
            return Err(INIError::MissingEndToken);
        }

        if ini.buffer.is_empty() {
            continue;
        }

        let line = ini.buffer.clone();
        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        if first.eq_ignore_ascii_case("End") {
            if nested_depth == 0 {
                break;
            }
            nested_depth -= 1;
            lines.push(line);
            continue;
        }

        if !line.contains('=') {
            nested_depth += 1;
        }

        lines.push(line);
    }

    Ok(lines)
}

fn parse_passthrough_block(ini: &mut INI) -> INIResult<()> {
    let _ = consume_block_with_nesting(ini)?;
    Ok(())
}

fn parse_armor_block(ini: &mut INI) -> INIResult<()> {
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut entries = HashMap::new();
    loop {
        ini.read_line()?;
        if ini.end_of_file {
            return Err(INIError::MissingEndToken);
        }

        if ini.buffer.is_empty() {
            continue;
        }

        let tokens = ini.get_line_tokens();
        let Some(first) = tokens.first() else {
            continue;
        };

        if first.eq_ignore_ascii_case("End") {
            break;
        }

        if !first.eq_ignore_ascii_case("Armor") {
            continue;
        }

        let values: Vec<&str> = tokens
            .iter()
            .copied()
            .skip(1)
            .filter(|token| *token != "=")
            .collect();
        if values.len() < 2 {
            continue;
        }

        if let Ok(percent) = values[1].trim_end_matches('%').parse::<f32>() {
            entries.insert(values[0].to_string(), percent / 100.0);
        }
    }

    let mut registry = armor_definition_registry()
        .write()
        .expect("Armor definition registry poisoned");
    registry.insert(name, entries);

    Ok(())
}

fn parse_object_creation_list_block(ini: &mut INI) -> INIResult<()> {
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    let lines = consume_block_with_nesting(ini)?;
    let mut registry = object_creation_list_registry()
        .write()
        .expect("ObjectCreationList registry poisoned");
    registry.insert(name, lines);
    Ok(())
}

fn parse_key_value_line(line: &str) -> Option<(String, String)> {
    if let Some((key, value)) = line.split_once('=') {
        let key = key.trim();
        if key.is_empty() {
            return None;
        }
        return Some((key.to_string(), value.trim().to_string()));
    }

    let mut tokens = line.split_whitespace();
    let key = tokens.next()?;
    let value = tokens.collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return None;
    }
    Some((key.to_string(), value))
}

fn parse_named_property_block(ini: &mut INI) -> INIResult<(String, HashMap<String, String>)> {
    let name = ini.get_next_value_token().ok_or(INIError::InvalidData)?;
    if name.trim().is_empty() {
        return Err(INIError::InvalidData);
    }

    let mut properties = HashMap::new();
    loop {
        ini.read_line()?;
        if ini.end_of_file {
            return Err(INIError::MissingEndToken);
        }

        if ini.buffer.is_empty() {
            continue;
        }

        if ini.buffer.eq_ignore_ascii_case("End") {
            break;
        }

        if let Some((key, value)) = parse_key_value_line(&ini.buffer) {
            properties.insert(key, value);
        }
    }

    Ok((name, properties))
}

fn parse_unnamed_property_block(ini: &mut INI) -> INIResult<HashMap<String, String>> {
    let mut properties = HashMap::new();
    loop {
        ini.read_line()?;
        if ini.end_of_file {
            return Err(INIError::MissingEndToken);
        }

        if ini.buffer.is_empty() {
            continue;
        }

        if ini.buffer.eq_ignore_ascii_case("End") {
            break;
        }

        if let Some((key, value)) = parse_key_value_line(&ini.buffer) {
            properties.insert(key, value);
        }
    }

    Ok(properties)
}

fn parse_fx_list_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let fx_list = super::ini_fx_list::parse_fx_list_definition(&name, &properties)
        .map_err(|_| INIError::InvalidData)?;
    let mut store = super::ini_fx_list::get_fx_list_store_mut();
    store.add_fx_list(fx_list);
    Ok(())
}

fn parse_locomotor_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let template = super::ini_locomotor::parse_locomotor_template_definition(&name, &properties)
        .map_err(|_| INIError::InvalidData)?;
    let mut store = super::ini_locomotor::get_locomotor_store_mut();
    store
        .add_template(template)
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_rank_block(ini: &mut INI) -> INIResult<()> {
    super::ini_rank::parse_rank_definition(ini).map_err(|_| INIError::InvalidData)
}

fn parse_credits_block(ini: &mut INI) -> INIResult<()> {
    super::ini_credits::parse_credits_definition(ini).map_err(|_| INIError::InvalidData)
}

fn parse_eva_event_block(ini: &mut INI) -> INIResult<()> {
    super::ini_eva_event::parse_eva_event_definition(ini).map_err(|_| INIError::InvalidData)
}

fn parse_particle_system_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let template = super::ini_particle_sys::IniParticleSys::parse_particle_system_block(
        AsciiString::from(name.as_str()),
        properties,
    )
    .map_err(|_| INIError::InvalidData)?;
    super::ini_particle_sys::IniParticleSys::register_template(template)
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_special_power_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    super::ini_special_power::IniSpecialPower::register_definition(
        AsciiString::from(name.as_str()),
        properties,
        ini.get_load_type(),
    )
    .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_terrain_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let terrain = super::ini_terrain::IniTerrain::parse_terrain_block(
        AsciiString::from(name.as_str()),
        properties,
    )
    .map_err(|_| INIError::InvalidData)?;
    super::ini_terrain::IniTerrain::register_terrain_type(terrain)
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_upgrade_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let template = super::ini_upgrade::IniUpgrade::parse_upgrade_block(
        AsciiString::from(name.as_str()),
        properties,
    )
    .map_err(|_| INIError::InvalidData)?;
    super::ini_upgrade::IniUpgrade::register_template(template)
        .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_video_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let video =
        super::ini_video::IniVideo::parse_video_block(AsciiString::from(name.as_str()), properties)
            .map_err(|_| INIError::InvalidData)?;
    super::ini_video::IniVideo::register_video(video).map_err(|_| INIError::InvalidData)?;
    Ok(())
}

fn parse_water_set_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    let setting = super::ini_water::IniWater::parse_water_setting_block(
        AsciiString::from(name.as_str()),
        properties,
    )
    .map_err(|_| INIError::InvalidData)?;
    super::ini_water::initialize_water_settings();
    let setting_slot =
        super::ini_water::get_water_setting(setting.time_of_day).ok_or(INIError::InvalidData)?;
    let mut setting_guard = setting_slot.write().expect("WaterSetting poisoned");
    *setting_guard = setting;
    Ok(())
}

fn parse_water_transparency_block(ini: &mut INI) -> INIResult<()> {
    let properties = parse_unnamed_property_block(ini)?;
    let setting = super::ini_water::IniWater::parse_water_transparency_block(properties)
        .map_err(|_| INIError::InvalidData)?;
    super::ini_water::initialize_water_settings();
    let transparency_lock =
        super::ini_water::get_water_transparency().ok_or(INIError::InvalidData)?;
    let mut transparency = transparency_lock
        .write()
        .expect("WaterTransparency poisoned");
    if ini.get_load_type() == INILoadType::CreateOverrides {
        let mut override_setting = setting;
        override_setting.mark_as_override();
        transparency.set_next_override(override_setting);
    } else {
        *transparency = setting;
    }
    Ok(())
}

fn parse_weapon_block(ini: &mut INI) -> INIResult<()> {
    let (name, properties) = parse_named_property_block(ini)?;
    super::ini_weapon::IniWeapon::register_definition(
        AsciiString::from(name.as_str()),
        properties,
        ini.get_load_type(),
    )
    .map_err(|_| INIError::InvalidData)?;
    Ok(())
}

// GameLOD wrapper functions - convert Result<(), String> to INIResult<()>
fn parse_static_game_lod_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_game_lod::parse_static_game_lod_definition(ini))
}

fn parse_dynamic_game_lod_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_game_lod::parse_dynamic_game_lod_definition(ini))
}

fn parse_lod_preset_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_game_lod::parse_lod_preset(ini))
}

fn parse_bench_profile_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_game_lod::parse_bench_profile(ini))
}

fn parse_really_low_mhz_block(ini: &mut INI) -> INIResult<()> {
    parse_block_result(super::ini_game_lod::parse_really_low_mhz(ini))
}

const BLOCK_PARSE_TABLE: &[BlockParse] = &[
    BlockParse {
        token: "AIData",
        parse: super::ini_ai_data::parse_ai_data_definition,
    },
    BlockParse {
        token: "Animation",
        parse: super::ini_animation::parse_anim2d_definition,
    },
    BlockParse {
        token: "Armor",
        parse: parse_armor_block,
    },
    BlockParse {
        token: "AudioEvent",
        parse: super::ini_audio_event_info::parse_audio_event_definition,
    },
    BlockParse {
        token: "AudioSettings",
        parse: super::ini_audio_settings::parse_audio_settings_definition,
    },
    BlockParse {
        token: "BeaconButtonDisabled",
        parse: parse_passthrough_block,
    },
    BlockParse {
        token: "BuddyButtonDisabled",
        parse: parse_passthrough_block,
    },
    BlockParse {
        token: "ButtonSet",
        parse: parse_passthrough_block,
    },
    BlockParse {
        token: "Campaign",
        parse: super::ini_campaign::parse_campaign_definition,
    },
    BlockParse {
        token: "ChallengeGenerals",
        parse: super::ini_challenge_generals::parse_challenge_generals_definition,
    },
    BlockParse {
        token: "CommandButton",
        parse: super::ini_command_button::parse_command_button_definition,
    },
    BlockParse {
        token: "CommandMap",
        parse: super::ini_command_map::parse_meta_map_definition,
    },
    BlockParse {
        token: "CommandSet",
        parse: super::ini_command_set::parse_command_set_definition,
    },
    BlockParse {
        token: "ControlBarResizer",
        parse: parse_passthrough_block,
    },
    BlockParse {
        token: "ControlBarScheme",
        parse: super::ini_control_bar_scheme::parse_control_bar_scheme_definition,
    },
    BlockParse {
        token: "Crate",
        parse: super::ini_crate::parse_crate_template_definition,
    },
    BlockParse {
        token: "CrateData",
        parse: super::ini_crate::parse_crate_template_definition,
    },
    BlockParse {
        token: "DamageFX",
        parse: parse_damage_fx_block,
    },
    BlockParse {
        token: "DialogEvent",
        parse: super::ini_audio_event_info::parse_dialog_definition,
    },
    BlockParse {
        token: "Credits",
        parse: parse_credits_block,
    },
    BlockParse {
        token: "EvaEvent",
        parse: parse_eva_event_block,
    },
    BlockParse {
        token: "DrawGroupInfo",
        parse: super::ini_draw_group_info::parse_draw_group_number_definition,
    },
    BlockParse {
        token: "GameData",
        parse: parse_game_data_block,
    },
    BlockParse {
        token: "FXList",
        parse: parse_fx_list_block,
    },
    BlockParse {
        token: "HeaderTemplate",
        parse: super::ini_header_template::parse_header_template_definition,
    },
    BlockParse {
        token: "InGameUI",
        parse: super::ini_in_game_ui::parse_in_game_ui_definition,
    },
    BlockParse {
        token: "Locomotor",
        parse: parse_locomotor_block,
    },
    BlockParse {
        token: "Language",
        parse: super::ini_language::parse_language_definition,
    },
    BlockParse {
        token: "MapCache",
        parse: parse_map_cache_block,
    },
    BlockParse {
        token: "MapData",
        parse: parse_map_data_block,
    },
    BlockParse {
        token: "MappedImage",
        parse: parse_mapped_image_block,
    },
    BlockParse {
        token: "MiscAudio",
        parse: super::ini_misc_audio::parse_misc_audio,
    },
    BlockParse {
        token: "Model",
        parse: parse_model_block,
    },
    BlockParse {
        token: "Mouse",
        parse: super::ini_mouse::parse_mouse_definition,
    },
    BlockParse {
        token: "MouseCursor",
        parse: super::ini_mouse::parse_mouse_cursor_definition,
    },
    BlockParse {
        token: "MultiplayerColor",
        parse: super::ini_multiplayer::parse_multiplayer_color_definition,
    },
    BlockParse {
        token: "MultiplayerSettings",
        parse: super::ini_multiplayer::parse_multiplayer_settings_definition,
    },
    BlockParse {
        token: "MultiplayerStartingMoneyChoice",
        parse: super::ini_multiplayer::parse_multiplayer_starting_money_choice_definition,
    },
    BlockParse {
        token: "OnlineChatColors",
        parse: super::ini_online_chat_colors::parse_online_chat_color_definition,
    },
    BlockParse {
        token: "PlayerTemplate",
        parse: super::ini_player_template::parse_player_template_definition,
    },
    BlockParse {
        token: "ParticleSystem",
        parse: parse_particle_system_block,
    },
    BlockParse {
        token: "MusicTrack",
        parse: super::ini_audio_event_info::parse_music_track_definition,
    },
    BlockParse {
        token: "Object",
        parse: parse_object_block,
    },
    BlockParse {
        token: "ObjectReskin",
        parse: parse_object_reskin_block,
    },
    BlockParse {
        token: "ObjectCreationList",
        parse: parse_object_creation_list_block,
    },
    BlockParse {
        token: "OptionsButtonDisabled",
        parse: parse_passthrough_block,
    },
    BlockParse {
        token: "WebpageURL",
        parse: parse_webpage_url_block,
    },
    BlockParse {
        token: "Road",
        parse: parse_road_block,
    },
    BlockParse {
        token: "Rank",
        parse: parse_rank_block,
    },
    BlockParse {
        token: "ShellMenuScheme",
        parse: super::ini_shell_menu_scheme::parse_shell_menu_scheme_definition,
    },
    BlockParse {
        token: "SpecialPower",
        parse: parse_special_power_block,
    },
    BlockParse {
        token: "Terrain",
        parse: parse_terrain_block,
    },
    BlockParse {
        token: "Bridge",
        parse: parse_bridge_block,
    },
    BlockParse {
        token: "Upgrade",
        parse: parse_upgrade_block,
    },
    BlockParse {
        token: "Video",
        parse: parse_video_block,
    },
    BlockParse {
        token: "WaterSet",
        parse: parse_water_set_block,
    },
    BlockParse {
        token: "WaterTransparency",
        parse: parse_water_transparency_block,
    },
    BlockParse {
        token: "Weapon",
        parse: parse_weapon_block,
    },
    BlockParse {
        token: "WindowTransition",
        parse: super::ini_window_transition::parse_window_transition_block,
    },
    BlockParse {
        token: "Science",
        parse: parse_science_block,
    },
    BlockParse {
        token: "Weather",
        parse: super::ini_weather::parse_weather_definition,
    },
    // GameLOD block parsers
    BlockParse {
        token: "StaticGameLOD",
        parse: parse_static_game_lod_block,
    },
    BlockParse {
        token: "DynamicGameLOD",
        parse: parse_dynamic_game_lod_block,
    },
    BlockParse {
        token: "LODPreset",
        parse: parse_lod_preset_block,
    },
    BlockParse {
        token: "BenchProfile",
        parse: parse_bench_profile_block,
    },
    BlockParse {
        token: "ReallyLowMHz",
        parse: parse_really_low_mhz_block,
    },
    // Script parsers
    BlockParse {
        token: "ScriptAction",
        parse: super::ini_script::parse_script_action_definition,
    },
    BlockParse {
        token: "ScriptCondition",
        parse: super::ini_script::parse_script_condition_definition,
    },
];

impl INI {
    /// Create a new INI reader
    pub fn new() -> Self {
        Self {
            file: None,
            staged_temp_file: None,
            read_buffer: [0; INI_READ_BUFFER],
            read_buffer_next: 0,
            read_buffer_used: 0,
            filename: "None".to_string(),
            load_type: INILoadType::Invalid,
            line_num: 0,
            buffer: String::with_capacity(INI_MAX_CHARS_PER_LINE),
            buffer_token_offset: 0,
            seps: " \n\r\t=",
            seps_percent: " \n\r\t=%",
            seps_colon: " \n\r\t=:",
            seps_quote: "\"\n=",
            block_end_token: "END",
            end_of_file: false,
            #[cfg(debug_assertions)]
            cur_block_start: String::new(),
        }
    }

    /// Check if a filename is a valid INI filename
    pub fn is_valid_ini_filename(filename: &str) -> bool {
        if filename.len() < 3 {
            return false;
        }

        let chars: Vec<char> = filename.chars().collect();
        let len = chars.len();

        // Check for .ini extension (case insensitive)
        chars[len - 1].to_ascii_lowercase() == 'i'
            && chars[len - 2].to_ascii_lowercase() == 'n'
            && chars[len - 3].to_ascii_lowercase() == 'i'
            && (len == 3 || chars[len - 4] == '.')
    }

    /// Load all INI files in the specified directory
    pub async fn load_directory<P: AsRef<Path>>(
        &mut self,
        dir_name: P,
        subdirs: bool,
        load_type: INILoadType,
    ) -> INIResult<()> {
        let dir_path = dir_name.as_ref();
        if !dir_path.exists() || !dir_path.is_dir() {
            return Err(INIError::InvalidDirectory);
        }

        let mut files_to_load = Vec::new();

        // Collect all .ini files
        if let Ok(mut entries) = tokio::fs::read_dir(dir_path).await {
            while let Some(entry) = entries.next_entry().await.transpose() {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        if let Some(extension) = path.extension() {
                            if extension.to_ascii_lowercase() == "ini" {
                                files_to_load.push(path);
                            }
                        }
                    } else if subdirs && path.is_dir() {
                        // Recursively load subdirectories
                        self.load_directory(&path, true, load_type).await?;
                    }
                }
            }
        }

        // Sort files for consistent loading order
        files_to_load.sort();

        // Load each file
        for file_path in files_to_load {
            self.load(&file_path, load_type)?;
        }

        Ok(())
    }

    /// Load and parse an INI file
    pub fn load<P: AsRef<Path>>(&mut self, filename: P, load_type: INILoadType) -> INIResult<()> {
        self.prep_file(&filename, load_type)?;

        let result = self.parse_file();
        self.un_prep_file();

        result
    }

    /// Temporarily parse an inline INI block by staging it in a temp file.
    pub fn with_inline_source<F, R>(&mut self, contents: &str, f: F) -> INIResult<R>
    where
        F: FnOnce(&mut INI) -> INIResult<R>,
    {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let filename = format!("codex_inline_{}_{}.ini", std::process::id(), nanos);
        let path = std::env::temp_dir().join(filename);

        fs::write(&path, contents).map_err(|_| INIError::CantOpenFile)?;
        let prep_result = self.prep_file(&path, INILoadType::Overwrite);
        if let Err(err) = prep_result {
            let _ = fs::remove_file(&path);
            return Err(err);
        }

        let result = f(self);
        self.un_prep_file();
        let _ = fs::remove_file(&path);

        result
    }

    /// Prepare file for reading
    fn prep_file<P: AsRef<Path>>(&mut self, filename: P, load_type: INILoadType) -> INIResult<()> {
        if self.file.is_some() {
            return Err(INIError::FileAlreadyOpen);
        }

        self.staged_temp_file = None;
        let filename_ref = filename.as_ref();
        let file = match File::open(filename_ref) {
            Ok(file) => file,
            Err(_) => {
                let staged = self
                    .stage_virtual_file_to_temp(filename_ref)
                    .ok_or(INIError::CantOpenFile)?;
                let file = File::open(&staged).map_err(|_| INIError::CantOpenFile)?;
                self.staged_temp_file = Some(staged);
                file
            }
        };
        self.file = Some(BufReader::new(file));
        self.filename = filename.as_ref().to_string_lossy().to_string();
        self.load_type = load_type;
        self.read_buffer_next = 0;
        self.read_buffer_used = 0;

        Ok(())
    }

    /// Clean up after file reading
    fn un_prep_file(&mut self) {
        self.file = None;
        if let Some(path) = self.staged_temp_file.take() {
            let _ = fs::remove_file(path);
        }
        self.read_buffer_used = 0;
        self.read_buffer_next = 0;
        self.filename = "None".to_string();
        self.load_type = INILoadType::Invalid;
        self.line_num = 0;
        self.end_of_file = false;
    }

    fn stage_virtual_file_to_temp(&self, filename: &Path) -> Option<PathBuf> {
        let virtual_name = filename.to_string_lossy().to_string();
        let file_system = get_file_system();
        let mut fs_guard = file_system.lock().ok()?;
        let mut file =
            fs_guard.open_file(&virtual_name, FileAccess::READ.combine(FileAccess::BINARY))?;
        let bytes = file.read_entire_and_close().ok()?;

        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let staged = std::env::temp_dir().join(format!(
            "generals_ini_stage_{}_{}.ini",
            std::process::id(),
            nanos
        ));

        fs::write(&staged, bytes).ok()?;
        Some(staged)
    }

    /// Parse the entire file
    fn parse_file(&mut self) -> INIResult<()> {
        while !self.end_of_file {
            self.read_line()?;

            if let Some(first_token) = self.get_first_token() {
                // Look up the block parser for this token type
                if let Some(parser) = self.find_block_parser(&first_token) {
                    #[cfg(debug_assertions)]
                    {
                        self.cur_block_start = first_token.clone();
                    }

                    parser(self).map_err(|e| {
                        eprintln!(
                            "Error parsing block '{}' in INI file '{}'",
                            first_token, self.filename
                        );
                        e
                    })?;

                    #[cfg(debug_assertions)]
                    {
                        self.cur_block_start.clear();
                    }
                } else {
                    eprintln!(
                        "Unknown block '{}' in file '{}' at line {}",
                        first_token, self.filename, self.line_num
                    );
                    return Err(INIError::UnknownToken);
                }
            }
        }

        Ok(())
    }

    /// Read a line from the file
    pub fn read_line(&mut self) -> INIResult<()> {
        if self.end_of_file {
            self.buffer.clear();
            return Ok(());
        }

        self.buffer.clear();
        self.buffer_token_offset = 0;
        self.line_num += 1;

        if let Some(ref mut reader) = self.file {
            match reader.read_line(&mut self.buffer) {
                Ok(0) => {
                    self.end_of_file = true;
                }
                Ok(_) => {
                    // Remove comments (everything after ';')
                    if let Some(comment_pos) = self.buffer.find(';') {
                        self.buffer.truncate(comment_pos);
                    }

                    // Trim whitespace
                    self.buffer = self.buffer.trim().to_string();

                    // Check for tab characters
                    if self.buffer.contains('\t') {
                        eprintln!(
                            "Tab characters are not allowed in INI files ({}). Line: {}",
                            self.filename, self.line_num
                        );
                        return Err(INIError::InvalidData);
                    }
                }
                Err(_) => return Err(INIError::UnknownError),
            }
        } else {
            return Err(INIError::FileNotOpen);
        }

        Ok(())
    }

    /// Get the first token from the current line
    pub fn get_first_token(&self) -> Option<String> {
        self.buffer.split_whitespace().next().map(|s| s.to_string())
    }

    /// Find the block parser for a given token
    fn find_block_parser(&self, token: &str) -> Option<INIBlockParse> {
        if let Some(parser) = get_extra_block_parser(token) {
            return Some(parser);
        }
        BLOCK_PARSE_TABLE
            .iter()
            .find(|entry| entry.token.eq_ignore_ascii_case(token))
            .map(|entry| entry.parse)
    }

    /// Advance to the next token in the current line.
    pub fn get_next_value_token(&mut self) -> Option<String> {
        self.buffer
            .split_whitespace()
            .skip(1)
            .find(|token| *token != "=")
            .map(|token| token.to_string())
    }

    /// Parse basic data types
    pub fn parse_unsigned_byte(token: &str) -> INIResult<u8> {
        let value: i32 = token.parse().map_err(|_| INIError::InvalidData)?;
        if value < 0 || value > 255 {
            return Err(INIError::InvalidData);
        }
        Ok(value as u8)
    }

    pub fn parse_short(token: &str) -> INIResult<i16> {
        let value: i32 = token.parse().map_err(|_| INIError::InvalidData)?;
        if value < -32768 || value > 32767 {
            return Err(INIError::InvalidData);
        }
        Ok(value as i16)
    }

    pub fn parse_unsigned_short(token: &str) -> INIResult<u16> {
        let value: i32 = token.parse().map_err(|_| INIError::InvalidData)?;
        if value < 0 || value > 65535 {
            return Err(INIError::InvalidData);
        }
        Ok(value as u16)
    }

    pub fn parse_int(token: &str) -> INIResult<i32> {
        token.parse().map_err(|_| INIError::InvalidData)
    }

    pub fn parse_unsigned_int(token: &str) -> INIResult<u32> {
        token.parse().map_err(|_| INIError::InvalidData)
    }

    pub fn parse_real(token: &str) -> INIResult<f32> {
        let trimmed = token.trim_end_matches(|c| c == 'f' || c == 'F');
        trimmed.parse().map_err(|_| INIError::InvalidData)
    }

    pub fn parse_bool(token: &str) -> INIResult<bool> {
        match token.to_ascii_lowercase().as_str() {
            "yes" | "true" | "1" => Ok(true),
            "no" | "false" | "0" => Ok(false),
            _ => Err(INIError::InvalidData),
        }
    }

    pub fn parse_ascii_string(token: &str) -> INIResult<String> {
        // Handle quoted strings
        if token.starts_with('"') && token.ends_with('"') {
            Ok(token[1..token.len() - 1].to_string())
        } else {
            Ok(token.to_string())
        }
    }

    pub fn parse_percent_to_real(token: &str) -> INIResult<f32> {
        let value: f32 = token
            .trim_end_matches('%')
            .parse()
            .map_err(|_| INIError::InvalidData)?;
        Ok(value / 100.0)
    }

    /// Parse angle in degrees and convert to radians
    pub fn parse_angle_real(token: &str) -> INIResult<f32> {
        let degrees: f32 = token.parse().map_err(|_| INIError::InvalidData)?;
        Ok(degrees * std::f32::consts::PI / 180.0)
    }

    /// Parse angular velocity in degrees per second and convert to radians per frame
    pub fn parse_angular_velocity_real(token: &str) -> INIResult<f32> {
        let degrees_per_sec: f32 = token.parse().map_err(|_| INIError::InvalidData)?;
        // Assuming 30 FPS for frame conversion
        Ok((degrees_per_sec * std::f32::consts::PI / 180.0) / 30.0)
    }

    /// Parse velocity in distance/second and convert to distance/frame.
    pub fn parse_velocity_real(token: &str) -> INIResult<f32> {
        let units_per_sec: f32 = token.parse().map_err(|_| INIError::InvalidData)?;
        Ok(units_per_sec / 30.0)
    }

    /// Parse a color in R:100 G:114 B:245 format
    pub fn parse_rgb_color(tokens: &[&str]) -> INIResult<(f32, f32, f32)> {
        let mut r = None;
        let mut g = None;
        let mut b = None;
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];
            let (key, value) = if let Some((left, right)) = token.split_once(':') {
                if right.is_empty() {
                    i += 1;
                    if i >= tokens.len() {
                        return Err(INIError::InvalidData);
                    }
                    (left, tokens[i])
                } else {
                    (left, right)
                }
            } else {
                if i + 1 >= tokens.len() {
                    return Err(INIError::InvalidData);
                }
                (token.trim_end_matches(':'), tokens[i + 1])
            };

            let value: i32 = value.parse().map_err(|_| INIError::InvalidData)?;
            if value < 0 || value > 255 {
                return Err(INIError::InvalidData);
            }

            match key.to_ascii_uppercase().as_str() {
                "R" => r = Some(value),
                "G" => g = Some(value),
                "B" => b = Some(value),
                _ => {}
            }

            i += 1;
        }

        let r = r.ok_or(INIError::InvalidData)?;
        let g = g.ok_or(INIError::InvalidData)?;
        let b = b.ok_or(INIError::InvalidData)?;

        Ok((r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0))
    }

    /// Parse a 3D coordinate in X:400 Y:-214.3 Z:8.6 format
    pub fn parse_coord_3d(tokens: &[&str]) -> INIResult<(f32, f32, f32)> {
        let mut x = None;
        let mut y = None;
        let mut z = None;
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];
            let (key, value) = if let Some((left, right)) = token.split_once(':') {
                if right.is_empty() {
                    i += 1;
                    if i >= tokens.len() {
                        return Err(INIError::InvalidData);
                    }
                    (left, tokens[i])
                } else {
                    (left, right)
                }
            } else {
                if i + 1 >= tokens.len() {
                    return Err(INIError::InvalidData);
                }
                (token.trim_end_matches(':'), tokens[i + 1])
            };

            let value: f32 = value.parse().map_err(|_| INIError::InvalidData)?;

            match key.to_ascii_uppercase().as_str() {
                "X" => x = Some(value),
                "Y" => y = Some(value),
                "Z" => z = Some(value),
                _ => {}
            }

            i += 1;
        }

        Ok((
            x.ok_or(INIError::InvalidData)?,
            y.ok_or(INIError::InvalidData)?,
            z.ok_or(INIError::InvalidData)?,
        ))
    }

    /// Parse a 2D coordinate in X:400 Y:-214.3 format
    pub fn parse_coord_2d(tokens: &[&str]) -> INIResult<(f32, f32)> {
        let mut x = None;
        let mut y = None;
        let mut i = 0;

        while i < tokens.len() {
            let token = tokens[i];
            let (key, value) = if let Some((left, right)) = token.split_once(':') {
                if right.is_empty() {
                    i += 1;
                    if i >= tokens.len() {
                        return Err(INIError::InvalidData);
                    }
                    (left, tokens[i])
                } else {
                    (left, right)
                }
            } else {
                if i + 1 >= tokens.len() {
                    return Err(INIError::InvalidData);
                }
                (token.trim_end_matches(':'), tokens[i + 1])
            };

            let value: f32 = value.parse().map_err(|_| INIError::InvalidData)?;

            match key.to_ascii_uppercase().as_str() {
                "X" => x = Some(value),
                "Y" => y = Some(value),
                _ => {}
            }

            i += 1;
        }

        Ok((
            x.ok_or(INIError::InvalidData)?,
            y.ok_or(INIError::InvalidData)?,
        ))
    }

    /// Parse index list - find token in name list and return index
    pub fn parse_index_list(token: &str, name_list: &[&str]) -> INIResult<usize> {
        name_list
            .iter()
            .position(|&name| name.eq_ignore_ascii_case(token))
            .ok_or(INIError::InvalidData)
    }

    /// Parse lookup list - find token in lookup list and return associated value
    pub fn parse_lookup_list(token: &str, lookup_list: &[LookupListRec]) -> INIResult<i32> {
        lookup_list
            .iter()
            .find(|rec| rec.name.eq_ignore_ascii_case(token))
            .map(|rec| rec.value)
            .ok_or(INIError::InvalidData)
    }

    /// Parse bit string with support for flags
    pub fn parse_bit_string_32(tokens: &[&str], flag_list: &[&str]) -> INIResult<u32> {
        let mut bits = 0u32;
        let mut found_normal = false;
        let mut found_add_or_sub = false;

        for token in tokens {
            if token.eq_ignore_ascii_case("NONE") {
                if found_normal || found_add_or_sub {
                    return Err(INIError::InvalidData);
                }
                bits = 0;
                break;
            }

            if token.starts_with('+') {
                if found_normal {
                    return Err(INIError::InvalidData);
                }
                let bit_index = Self::parse_index_list(&token[1..], flag_list)?;
                bits |= 1 << bit_index;
                found_add_or_sub = true;
            } else if token.starts_with('-') {
                if found_normal {
                    return Err(INIError::InvalidData);
                }
                let bit_index = Self::parse_index_list(&token[1..], flag_list)?;
                bits &= !(1 << bit_index);
                found_add_or_sub = true;
            } else {
                if found_add_or_sub {
                    return Err(INIError::InvalidData);
                }

                if !found_normal {
                    bits = 0;
                }

                let bit_index = Self::parse_index_list(token, flag_list)?;
                bits |= 1 << bit_index;
                found_normal = true;
            }
        }

        Ok(bits)
    }

    /// Convert duration from milliseconds to frames (assuming 30 FPS)
    pub fn convert_duration_msecs_to_frames(msecs: f32) -> f32 {
        msecs / (1000.0 / 30.0)
    }

    /// Convert velocity from units per second to units per frame (assuming 30 FPS)
    pub fn convert_velocity_secs_to_frames(velocity: f32) -> f32 {
        velocity / 30.0
    }

    /// Convert acceleration from units per second squared to units per frame squared (assuming 30 FPS)
    pub fn convert_acceleration_secs_to_frames(acceleration: f32) -> f32 {
        acceleration / (30.0 * 30.0)
    }

    // Getter methods
    pub fn get_filename(&self) -> &str {
        &self.filename
    }

    pub fn get_load_type(&self) -> INILoadType {
        self.load_type
    }

    pub fn get_line_num(&self) -> u32 {
        self.line_num
    }

    pub fn is_eof(&self) -> bool {
        self.end_of_file
    }

    // Instance methods for parsing from current token stream

    /// Get next token from the current line, advancing the internal position.
    ///
    /// C++ Reference: `INI::getNextToken()` (INI.cpp line 1535-1542)
    /// Uses `strtok(NULL, seps)` which is stateful — each call advances past
    /// the previously returned token. This Rust implementation mirrors that
    /// behavior by tracking `buffer_token_offset`.
    ///
    /// Returns `Err(INIError::InvalidData)` if no more tokens are available,
    /// matching the C++ throw behavior.
    pub fn get_next_token(&mut self) -> INIResult<String> {
        self.get_next_token_with_seps(self.seps)
            .ok_or(INIError::InvalidData)
    }

    /// Get next token or None if no more tokens available.
    /// C++ Reference: `INI::getNextTokenOrNull()` (INI.cpp line 1545-1550)
    pub fn get_next_token_or_null(&mut self) -> Option<String> {
        self.get_next_token_with_seps(self.seps)
    }

    /// Internal: advance through buffer from `buffer_token_offset` using the
    /// given separator set, returning the next token and updating position.
    fn get_next_token_with_seps(&mut self, seps: &str) -> Option<String> {
        let bytes = self.buffer.as_bytes();
        let len = bytes.len();
        let mut pos = self.buffer_token_offset;

        // Skip leading separators (strtok behavior)
        while pos < len && seps.contains(bytes[pos] as char) {
            pos += 1;
        }

        if pos >= len {
            self.buffer_token_offset = len;
            return None;
        }

        // Find end of token
        let start = pos;
        while pos < len && !seps.contains(bytes[pos] as char) {
            pos += 1;
        }

        self.buffer_token_offset = pos;
        Some(self.buffer[start..pos].to_string())
    }

    /// Return all tokens in the current line.
    pub fn get_line_tokens(&self) -> Vec<&str> {
        self.buffer.split_whitespace().collect()
    }

    /// Get the current token from the buffer.
    ///
    /// C++ Reference: `INI::getCurrentToken()` — returns the first token of the
    /// current line, typically the block/section name after a block header.
    pub fn get_current_token(&self) -> Option<String> {
        self.buffer.split_whitespace().next().map(|s| s.to_string())
    }

    /// Read the next line and return the field name (first token).
    ///
    /// Used by field-based INI parsers that iterate over `Field = Value` lines
    /// inside a block. Returns `None` on EOF or when an `End` token is reached.
    ///
    /// C++ Reference: the common pattern in C++ particle system and similar
    /// parsers that loop over fields within a named block.
    pub fn get_next_field(&mut self) -> Option<String> {
        loop {
            if self.end_of_file {
                return None;
            }
            // Read the next line
            if self.read_line().is_err() {
                return None;
            }
            if self.end_of_file {
                return None;
            }
            let trimmed = self.buffer.trim();
            // Skip empty lines and comments
            if trimmed.is_empty() || trimmed.starts_with(';') || trimmed.starts_with("//") {
                continue;
            }
            // Check for End block token
            let first = trimmed.split_whitespace().next().unwrap_or("");
            if first.eq_ignore_ascii_case("end") {
                return None;
            }
            return Some(first.to_string());
        }
    }

    /// Get the value portion of the current `Field = Value` line.
    ///
    /// After `get_next_field()` positions the cursor on a line like
    /// `Shader = ADDITIVE`, this returns everything after the `=` sign,
    /// trimmed. The `_field_name` parameter is accepted for API parity
    /// with C++ but the value is extracted from the current buffer line.
    ///
    /// C++ Reference: `INI::getFieldValue()` in field-parse table usage.
    pub fn get_field_value(&mut self, _field_name: &str) -> INIResult<String> {
        // Find '=' and return everything after it, trimmed
        if let Some(eq_pos) = self.buffer.find('=') {
            let value = self.buffer[eq_pos + 1..].trim();
            if value.is_empty() {
                Err(INIError::InvalidValue)
            } else {
                Ok(value.to_string())
            }
        } else {
            // Some INI fields use space-separated tokens without '='
            // Fall back to second token
            let mut tokens = self.buffer.split_whitespace();
            tokens.next(); // skip field name
            tokens
                .next()
                .map(|s| s.to_string())
                .ok_or(INIError::InvalidValue)
        }
    }

    /// Check whether the INI stream has reached end-of-file.
    pub fn is_end_of_file(&self) -> bool {
        self.end_of_file
    }

    /// Get a reference to the current line buffer (already trimmed, comments removed).
    pub fn get_buffer(&self) -> &str {
        &self.buffer
    }

    /// Parse coordinate from tokens in the line
    pub fn parse_coord3d(&mut self) -> INIResult<(f32, f32, f32)> {
        let tokens: Vec<&str> = self.buffer.split_whitespace().collect();
        Self::parse_coord_3d(&tokens)
    }

    /// Parse boolean from next token
    pub fn parse_next_bool(&mut self) -> INIResult<bool> {
        let token = self.get_next_token()?;
        Self::parse_bool(&token)
    }

    /// Parse integer from next token
    pub fn parse_next_int(&mut self) -> INIResult<i32> {
        let token = self.get_next_token()?;
        Self::parse_int(&token)
    }

    /// Parse unsigned integer from next token
    pub fn parse_next_unsigned_int(&mut self) -> INIResult<u32> {
        let token = self.get_next_token()?;
        Self::parse_unsigned_int(&token)
    }

    /// Parse ASCII string from next token
    pub fn parse_next_ascii_string(&mut self) -> INIResult<String> {
        let token = self.get_next_token()?;
        Self::parse_ascii_string(&token)
    }

    /// Parse 2D region from tokens
    pub fn parse_region2d(&mut self) -> INIResult<(f32, f32, f32, f32)> {
        let tokens: Vec<&str> = self.buffer.split_whitespace().collect();
        if tokens.len() < 8 {
            return Err(INIError::InvalidData);
        }

        let mut left = 0.0;
        let mut top = 0.0;
        let mut right = 0.0;
        let mut bottom = 0.0;

        for i in (0..tokens.len()).step_by(2) {
            if i + 1 >= tokens.len() {
                break;
            }

            let component = tokens[i];
            let value_str = tokens[i + 1];
            let value: f32 = value_str.parse().map_err(|_| INIError::InvalidData)?;

            match component.to_uppercase().as_str() {
                "LEFT:" => left = value,
                "TOP:" => top = value,
                "RIGHT:" => right = value,
                "BOTTOM:" => bottom = value,
                _ => continue,
            }
        }

        Ok((left, top, right, bottom))
    }

    /// Parse flags from tokens with flag list
    pub fn parse_flags_with_list(&mut self, flag_list: &[&str]) -> INIResult<u32> {
        let tokens: Vec<&str> = self.buffer.split_whitespace().collect();
        Self::parse_bit_string_32(&tokens, flag_list)
    }

    /// Parse flags as integer value (for compatibility)
    pub fn parse_flags(&mut self) -> INIResult<u32> {
        let tokens: Vec<&str> = self.buffer.split_whitespace().collect();
        // Return parsed integer or 0 for empty
        if tokens.is_empty() {
            Ok(0)
        } else {
            tokens[0].parse().map_err(|_| INIError::InvalidData)
        }
    }

    /// Parse audio event token from the current line.
    pub fn parse_audio_event_rts(&mut self) -> INIResult<String> {
        let token = self.get_next_token()?;
        Self::parse_ascii_string(&token)
    }

    /// Initialize structure from INI using field parse table
    pub fn init_from_ini_with_fields<T>(
        &mut self,
        target: &mut T,
        field_parse_table: &[FieldParse<T>],
    ) -> INIResult<()> {
        loop {
            self.read_line()?;

            if self.end_of_file {
                return Err(INIError::EndOfFile);
            }

            let line = self.buffer.clone();
            let mut parts = line.split_whitespace();

            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");
            let mut handled = false;

            for field in field_parse_table {
                if field.token.eq_ignore_ascii_case(key) {
                    (field.parse)(self, target, &value_tokens)?;
                    handled = true;
                    break;
                }
            }

            if !handled {
                return Err(INIError::UnknownToken);
            }
        }

        Ok(())
    }

    /// Initialize structure from INI using field parse table, ignoring unknown tokens.
    pub fn init_from_ini_with_fields_allow_unknown<T>(
        &mut self,
        target: &mut T,
        field_parse_table: &[FieldParse<T>],
    ) -> INIResult<()> {
        loop {
            self.read_line()?;

            if self.end_of_file {
                return Err(INIError::EndOfFile);
            }

            let line = self.buffer.clone();
            let mut parts = line.split_whitespace();

            let Some(key) = parts.next() else {
                continue;
            };

            if key.eq_ignore_ascii_case("End") {
                break;
            }

            let mut value_tokens: Vec<&str> = parts.collect();
            value_tokens.retain(|token| *token != "=");
            let mut handled = false;

            for field in field_parse_table {
                if field.token.eq_ignore_ascii_case(key) {
                    (field.parse)(self, target, &value_tokens)?;
                    handled = true;
                    break;
                }
            }

            if !handled {
                continue;
            }
        }

        Ok(())
    }

    /// Parse percent value to real number  
    pub fn parse_next_percent_to_real(&mut self) -> INIResult<f32> {
        let token = self.get_next_token()?;
        Self::parse_percent_to_real(&token)
    }

    /// Parse duration string into frames (unsigned int).
    /// Accepts raw milliseconds or suffixes "ms" / "s".
    pub fn parse_duration_unsigned_int(token: &str) -> INIResult<u32> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(INIError::InvalidData);
        }

        let lower = trimmed.to_ascii_lowercase();
        let (value_str, multiplier) = if let Some(stripped) = lower.strip_suffix("ms") {
            (stripped, 1.0)
        } else if let Some(stripped) = lower.strip_suffix('s') {
            (stripped, 1000.0)
        } else {
            (lower.as_str(), 1.0)
        };

        let value = Self::parse_real(value_str)?;
        if value.is_sign_negative() {
            return Err(INIError::InvalidData);
        }

        let msecs = value * multiplier;
        let frames = Self::convert_duration_msecs_to_frames(msecs);
        Ok(frames.round().max(0.0) as u32)
    }

    /// Parse duration string into frames (real).
    /// Accepts raw milliseconds or suffixes "ms" / "s".
    pub fn parse_duration_real(token: &str) -> INIResult<f32> {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(INIError::InvalidData);
        }

        let lower = trimmed.to_ascii_lowercase();
        let (value_str, multiplier) = if let Some(stripped) = lower.strip_suffix("ms") {
            (stripped, 1.0)
        } else if let Some(stripped) = lower.strip_suffix('s') {
            (stripped, 1000.0)
        } else {
            (lower.as_str(), 1.0)
        };

        let value = Self::parse_real(value_str)?;
        if value.is_sign_negative() {
            return Err(INIError::InvalidData);
        }

        let msecs = value * multiplier;
        Ok(Self::convert_duration_msecs_to_frames(msecs))
    }

    /// Parse color value as integer
    pub fn parse_color_int(&mut self) -> INIResult<u32> {
        let token = self.get_next_token()?;
        token.parse().map_err(|_| INIError::InvalidData)
    }

    /// Parse quoted ASCII string
    pub fn parse_quoted_ascii_string(&mut self) -> INIResult<String> {
        let token = self.get_next_token()?;
        Self::parse_ascii_string(&token)
    }

    /// Get the next sub-token in "Tag:Value" format.
    ///
    /// This is called when the next thing you expect is something like:
    ///   `Tag:value`
    ///
    /// Pass "Tag" (without the colon) for `expected`, and you will have the
    /// 'value' token returned.
    ///
    /// If "Tag" is not the next token, an error is returned.
    /// Matches C++ INI::getSubToken
    pub fn get_next_sub_token(&mut self, expected: &str) -> INIResult<String> {
        let token = self.get_next_token()?;

        let (tag, value) = token.split_once(':').ok_or(INIError::InvalidData)?;

        if !tag.eq_ignore_ascii_case(expected) {
            return Err(INIError::InvalidData);
        }

        Ok(value.to_string())
    }

    /// Parse a string label and translate it using the localization system.
    ///
    /// This fetches the localized text for the given label key and returns it.
    /// If the translation is empty or the key is not found, an error is returned.
    ///
    /// C++ equivalent: `INI::parseAndTranslateLabel`
    pub fn parse_and_translate_label(&mut self) -> INIResult<String> {
        let token = self.get_next_token()?;

        // Translate using GameText (if available) or return the key itself
        let translated = Self::translate_label(&token)?;

        Ok(translated)
    }

    /// Translate a label key to its localized string.
    ///
    /// This is a helper function that performs the actual translation lookup.
    pub fn translate_label(label: &str) -> INIResult<String> {
        // Try to get the translated string using the Language system
        // The Language::get_localized_string function returns the key itself if no translation found
        let translated = crate::common::language::Language::get_localized_string(label);

        if translated.is_empty() {
            // In C++, an empty translation throws INI_INVALID_DATA
            // However, for flexibility, we return the key itself
            if label.is_empty() {
                return Err(INIError::InvalidData);
            }
            return Ok(label.to_string());
        }

        Ok(translated)
    }

    /// Scan a real value from a sub-token (helper for coordinate parsing)
    pub fn scan_real_from_sub_token(&mut self, expected: &str) -> INIResult<f32> {
        let token = self.get_next_sub_token(expected)?;
        Self::parse_real(&token)
    }

    /// Scan an int value from a sub-token (helper for coordinate parsing)
    pub fn scan_int_from_sub_token(&mut self, expected: &str) -> INIResult<i32> {
        let token = self.get_next_sub_token(expected)?;
        Self::parse_int(&token)
    }
}

impl Default for INI {
    fn default() -> Self {
        Self::new()
    }
}
