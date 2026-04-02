//! Meta event translator for key and mouse remapping.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::ini::{
    get_global_data, register_block_parser, INIError, INILoadType, INIResult, INI,
};
use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};
use log::debug;

use super::game_message::{
    build_region, Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D,
    IRegion2D,
};
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::gui::shell::get_shell;
use crate::helpers::TheInGameUI;
use crate::message_stream::selection_xlat::DRAG_TOLERANCE;
use gamelogic::commands::command::CommandType;
use gamelogic::helpers::{TheGameLogic, TheThingFactory};
use gamelogic::player::ThePlayerList;

const MOD_CTRL: u32 = 1;
const MOD_ALT: u32 = 2;
const MOD_SHIFT: u32 = 4;

const KEY_STATE_CONTROL: u32 = 0x0004 | 0x0008;
const KEY_STATE_SHIFT: u32 = 0x0010 | 0x0020 | 0x0400;
const KEY_STATE_ALT: u32 = 0x0040 | 0x0080;
const KEY_STATE_DOWN: u32 = 0x0002;
const KEY_STATE_UP: u32 = 0x0001;
const KEY_STATE_AUTOREPEAT: u32 = 0x0100;

const COMMANDUSABLE_NONE: u32 = 0;
const COMMANDUSABLE_SHELL: u32 = 1 << 0;
const COMMANDUSABLE_GAME: u32 = 1 << 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Transition {
    Down,
    Up,
    DoubleDown,
}

#[derive(Debug, Clone)]
struct MetaMapRec {
    name: String,
    meta: Option<GameMessageType>,
    key: u32,
    transition: Transition,
    mod_state: u32,
    usable_in: u32,
    category: String,
    description: String,
    display_name: String,
}

#[derive(Debug, Clone)]
pub struct CommandMapEntry {
    pub key: u32,
    pub mod_state: u32,
    pub category: String,
    pub description: String,
    pub display_name: String,
}

#[derive(Default)]
struct MetaMap {
    records: Vec<MetaMapRec>,
}

impl MetaMap {
    fn add_record(&mut self, record: MetaMapRec) {
        let existing_index = self.records.iter().position(|existing| {
            if let (Some(existing_meta), Some(new_meta)) = (&existing.meta, &record.meta) {
                return existing_meta == new_meta;
            }
            existing.meta.is_none()
                && record.meta.is_none()
                && existing.name.eq_ignore_ascii_case(&record.name)
        });

        if let Some(index) = existing_index {
            self.records[index] = record;
        } else {
            self.records.push(record);
        }
    }

    fn iter(&self) -> impl Iterator<Item = &MetaMapRec> {
        self.records.iter()
    }
}

static META_MAP: OnceLock<RwLock<MetaMap>> = OnceLock::new();
static META_PARSER_REGISTERED: OnceLock<()> = OnceLock::new();
static LOWER_DETAIL_TOGGLE_STATE: OnceLock<RwLock<LowerDetailToggleState>> = OnceLock::new();

const DROPPED_MAX_PARTICLE_COUNT: i32 = 1000;

#[derive(Debug, Clone)]
struct LowerDetailToggleState {
    is_low_details: bool,
    old_use_shadow_volumes: bool,
    old_use_light_map: bool,
    old_use_cloud_map: bool,
    old_show_behind_building_markers: bool,
    old_max_particle_count: i32,
}

impl Default for LowerDetailToggleState {
    fn default() -> Self {
        Self {
            is_low_details: false,
            old_use_shadow_volumes: true,
            old_use_light_map: true,
            old_use_cloud_map: true,
            old_show_behind_building_markers: true,
            old_max_particle_count: 5000,
        }
    }
}

fn get_meta_map() -> &'static RwLock<MetaMap> {
    META_MAP.get_or_init(|| RwLock::new(MetaMap::default()))
}

fn get_lower_detail_toggle_state() -> &'static RwLock<LowerDetailToggleState> {
    LOWER_DETAIL_TOGGLE_STATE.get_or_init(|| RwLock::new(LowerDetailToggleState::default()))
}

fn ensure_meta_map_loaded() {
    META_PARSER_REGISTERED.get_or_init(|| {
        let _ = register_block_parser("CommandMap", parse_meta_map_definition);
    });

    if get_meta_map()
        .read()
        .map(|guard| !guard.records.is_empty())
        .unwrap_or(false)
    {
        return;
    }

    load_meta_map_files();
}

fn load_meta_map_files() {
    let mut ini = INI::new();
    let paths = discover_command_map_files();
    for (index, path) in paths.into_iter().enumerate() {
        let load_type = if index == 0 {
            INILoadType::Overwrite
        } else {
            INILoadType::MultiFile
        };
        let _ = ini.load(path, load_type);
    }
}

fn discover_command_map_files() -> Vec<PathBuf> {
    let mut roots = Vec::<PathBuf>::new();
    let mut seen_roots = HashSet::<PathBuf>::new();

    let mut push_root = |path: PathBuf| {
        if seen_roots.insert(path.clone()) {
            roots.push(path);
        }
    };

    push_root(PathBuf::from("."));
    if let Ok(current) = std::env::current_dir() {
        push_root(current.clone());
        for ancestor in current.ancestors() {
            push_root(ancestor.to_path_buf());
        }
    }

    if let Some(global) = get_global_data() {
        let mod_dir = global.read().mod_dir.clone();
        if !mod_dir.trim().is_empty() {
            push_root(PathBuf::from(mod_dir.trim()));
        }
    }

    let mut files = Vec::new();
    let mut seen = HashSet::new();

    for root in roots {
        push_command_map_file(&mut files, &mut seen, root.join("Data/INI/CommandMap.ini"));
        push_command_map_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/CommandMapDebug.ini"),
        );
        push_command_map_file(
            &mut files,
            &mut seen,
            root.join("Data/INI/CommandMapDemo.ini"),
        );

        for extracted in [
            root.join("windows_game/extracted_big_files/INIZH"),
            root.join("windows_game/extracted_big_files_v2/INIZH"),
        ] {
            push_command_map_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/CommandMap.ini"),
            );
            push_command_map_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/CommandMapDebug.ini"),
            );
            push_command_map_file(
                &mut files,
                &mut seen,
                extracted.join("Data/INI/CommandMapDemo.ini"),
            );
        }

        for localized in [
            root.join("windows_game/extracted_big_files/EnglishZH"),
            root.join("windows_game/extracted_big_files/W3DEnglishZH"),
            root.join("windows_game/extracted_big_files_v2/EnglishZH"),
            root.join("windows_game/extracted_big_files_v2/W3DEnglishZH"),
        ] {
            push_command_map_file(
                &mut files,
                &mut seen,
                localized.join("Data/English/CommandMap.ini"),
            );
        }
    }

    files
}

fn push_command_map_file(files: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.is_file() {
        let key = std::fs::canonicalize(&path).unwrap_or(path.clone());
        if seen.insert(key) {
            files.push(path);
        }
    }
}

pub fn get_command_map_entries() -> Vec<CommandMapEntry> {
    ensure_meta_map_loaded();
    let guard = get_meta_map().read().expect("MetaMap lock poisoned");
    guard
        .iter()
        .map(|record| CommandMapEntry {
            key: record.key,
            mod_state: record.mod_state,
            category: record.category.clone(),
            description: record.description.clone(),
            display_name: record.display_name.clone(),
        })
        .collect()
}

pub fn update_command_map_entry(
    category: &str,
    display_name: &str,
    key: u32,
    mod_state: u32,
) -> bool {
    ensure_meta_map_loaded();
    let Ok(mut guard) = get_meta_map().write() else {
        return false;
    };

    let Some(record) = guard.records.iter_mut().find(|record| {
        record.category.eq_ignore_ascii_case(category)
            && record.display_name.eq_ignore_ascii_case(display_name)
    }) else {
        return false;
    };

    record.key = key;
    record.mod_state = mod_state;
    true
}

pub fn reset_command_map_entries() {
    META_PARSER_REGISTERED.get_or_init(|| {
        let _ = register_block_parser("CommandMap", parse_meta_map_definition);
    });
    if let Ok(mut guard) = get_meta_map().write() {
        guard.records.clear();
    }
    load_meta_map_files();
}

fn parse_meta_map_definition(ini: &mut INI) -> INIResult<()> {
    let tokens = ini.get_line_tokens();
    let name = tokens
        .iter()
        .skip(1)
        .find(|token| **token != "=")
        .ok_or(INIError::InvalidData)?
        .to_string();

    if !is_supported_command_map_name(&name) {
        return Err(INIError::InvalidData);
    }

    let meta = lookup_meta_message_type(&name);
    let mut record = MetaMapRec {
        name: name.clone(),
        meta,
        key: 0,
        transition: Transition::Down,
        mod_state: 0,
        usable_in: COMMANDUSABLE_NONE,
        category: String::new(),
        description: String::new(),
        display_name: String::new(),
    };

    loop {
        let Some((key, values)) = parse_block_field(ini)? else {
            continue;
        };
        if key.eq_ignore_ascii_case("End") {
            break;
        }

        match key.to_ascii_uppercase().as_str() {
            "KEY" => {
                if let Some(value) = values.first() {
                    if let Some(code) = lookup_key_code(value) {
                        record.key = code;
                    }
                }
            }
            "TRANSITION" => {
                if let Some(value) = values.first() {
                    record.transition = parse_transition(value);
                }
            }
            "MODIFIERS" => {
                if let Some(value) = values.first() {
                    record.mod_state = parse_mod_state(value);
                }
            }
            "USEABLEIN" => {
                record.usable_in = parse_usable_in(&values);
            }
            "CATEGORY" => {
                if let Some(value) = values.first() {
                    record.category = value.to_string();
                }
            }
            "DESCRIPTION" => {
                if let Some(value) = values.first() {
                    record.description = value.to_string();
                }
            }
            "DISPLAYNAME" => {
                if let Some(value) = values.first() {
                    record.display_name = value.to_string();
                }
            }
            _ => {}
        }
    }

    get_meta_map()
        .write()
        .expect("MetaMap lock poisoned")
        .add_record(record);
    Ok(())
}

fn is_unimplemented_cpp_command_name(name: &str) -> bool {
    // C++ MetaEvent.cpp table entries that exist in CommandMap files but are not
    // represented as typed Rust messages yet. Keep these accepted/consumed so keybind
    // behavior stays aligned while the full message pipeline is still being ported.
    match name.to_ascii_uppercase().as_str() {
        "CHEAT_ADD_CASH" => true,
        "CHEAT_DESHROUD" => true,
        "CHEAT_GIVE_ALL_SCIENCES" => true,
        "CHEAT_GIVE_SCIENCEPURCHASEPOINTS" => true,
        "CHEAT_INSTANT_BUILD" => true,
        "CHEAT_KILL_SELECTION" => true,
        "CHEAT_RUNSCRIPT1" => true,
        "CHEAT_RUNSCRIPT2" => true,
        "CHEAT_RUNSCRIPT3" => true,
        "CHEAT_RUNSCRIPT4" => true,
        "CHEAT_RUNSCRIPT5" => true,
        "CHEAT_RUNSCRIPT6" => true,
        "CHEAT_RUNSCRIPT7" => true,
        "CHEAT_RUNSCRIPT8" => true,
        "CHEAT_RUNSCRIPT9" => true,
        "CHEAT_SHOW_HEALTH" => true,
        "CHEAT_SWITCH_TEAMS" => true,
        "CHEAT_TOGGLE_HAND_OF_GOD_MODE" => true,
        "CHEAT_TOGGLE_MESSAGE_TEXT" => true,
        "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS" => true,
        "DEBUG_DRAWABLE_ID_PERFORMANCE" => true,
        "DEBUG_DUMP_ALL_PLAYER_OBJECTS" => true,
        "DEBUG_DUMP_PLAYER_OBJECTS" => true,
        "DEBUG_OBJECT_ID_PERFORMANCE" => true,
        "DEBUG_SLEEPY_UPDATE_PERFORMANCE" => true,
        "DEMO_ADDCASH" => true,
        "DEMO_BATTLE_CRY" => true,
        "DEMO_BEGIN_ADJUST_FOV" => true,
        "DEMO_BEGIN_ADJUST_PITCH" => true,
        "DEMO_CYCLE_EXTENT_TYPE" => true,
        "DEMO_CYCLE_LOD_LEVEL" => true,
        "DEMO_DEBUG_SELECTION" => true,
        "DEMO_DECR_ANIM_SKATE_SPEED" => true,
        "DEMO_DECR_EXTENT_HEIGHT" => true,
        "DEMO_DECR_EXTENT_HEIGHT_LARGE" => true,
        "DEMO_DECR_EXTENT_MAJOR" => true,
        "DEMO_DECR_EXTENT_MAJOR_LARGE" => true,
        "DEMO_DECR_EXTENT_MINOR" => true,
        "DEMO_DECR_EXTENT_MINOR_LARGE" => true,
        "DEMO_DESHROUD" => true,
        "DEMO_DUMP_ASSETS" => true,
        "DEMO_END_ADJUST_FOV" => true,
        "DEMO_END_ADJUST_PITCH" => true,
        "DEMO_ENSHROUD" => true,
        "DEMO_GIVE_ALL_SCIENCES" => true,
        "DEMO_GIVE_RANKLEVEL" => true,
        "DEMO_GIVE_SCIENCEPURCHASEPOINTS" => true,
        "DEMO_GIVE_VETERANCY" => true,
        "DEMO_INCR_ANIM_SKATE_SPEED" => true,
        "DEMO_INCR_EXTENT_HEIGHT" => true,
        "DEMO_INCR_EXTENT_HEIGHT_LARGE" => true,
        "DEMO_INCR_EXTENT_MAJOR" => true,
        "DEMO_INCR_EXTENT_MAJOR_LARGE" => true,
        "DEMO_INCR_EXTENT_MINOR" => true,
        "DEMO_INCR_EXTENT_MINOR_LARGE" => true,
        "DEMO_INSTANT_BUILD" => true,
        "DEMO_KILL_ALL_ENEMIES" => true,
        "DEMO_KILL_SELECTION" => true,
        "DEMO_LOCK_CAMERA_TO_PLANES" => true,
        "DEMO_LOCK_CAMERA_TO_SELECTION" => true,
        "DEMO_LOD_DECREASE" => true,
        "DEMO_LOD_INCREASE" => true,
        "DEMO_MUSIC_NEXT_TRACK" => true,
        "DEMO_MUSIC_PREV_TRACK" => true,
        "DEMO_NEXT_OBJECTIVE_MOVIE" => true,
        "DEMO_PERFORM_STATISTICAL_DUMP" => true,
        "DEMO_PLAY_CAMEO_MOVIE" => true,
        "DEMO_PLAY_OBJECTIVE_MOVIE1" => true,
        "DEMO_PLAY_OBJECTIVE_MOVIE2" => true,
        "DEMO_PLAY_OBJECTIVE_MOVIE3" => true,
        "DEMO_PLAY_OBJECTIVE_MOVIE4" => true,
        "DEMO_PLAY_OBJECTIVE_MOVIE5" => true,
        "DEMO_PLAY_OBJECTIVE_MOVIE6" => true,
        "DEMO_REMOVE_PREREQ" => true,
        "DEMO_RUNSCRIPT1" => true,
        "DEMO_RUNSCRIPT2" => true,
        "DEMO_RUNSCRIPT3" => true,
        "DEMO_RUNSCRIPT4" => true,
        "DEMO_RUNSCRIPT5" => true,
        "DEMO_RUNSCRIPT6" => true,
        "DEMO_RUNSCRIPT7" => true,
        "DEMO_RUNSCRIPT8" => true,
        "DEMO_RUNSCRIPT9" => true,
        "DEMO_SHOW_AUDIO_LOCATIONS" => true,
        "DEMO_SHOW_EXTENTS" => true,
        "DEMO_SHOW_HEALTH" => true,
        "DEMO_SWITCH_TEAMS" => true,
        "DEMO_SWITCH_TEAMS_CHINA_USA" => true,
        "DEMO_TAKE_RANKLEVEL" => true,
        "DEMO_TAKE_VETERANCY" => true,
        "DEMO_TEST_SURRENDER" => true,
        "DEMO_TIME_OF_DAY" => true,
        "DEMO_TOGGLE_AI_DEBUG" => true,
        "DEMO_TOGGLE_AUDIODEBUG" => true,
        "DEMO_TOGGLE_AVI" => true,
        "DEMO_TOGGLE_BEHIND_BUILDINGS" => true,
        "DEMO_TOGGLE_BW_VIEW" => true,
        "DEMO_TOGGLE_CAMERA_DEBUG" => true,
        "DEMO_TOGGLE_CASHMAPDEBUG" => true,
        "DEMO_TOGGLE_DEBUG_STATS" => true,
        "DEMO_TOGGLE_FEATHER_WATER" => true,
        "DEMO_TOGGLE_FOGOFWAR" => true,
        "DEMO_TOGGLE_GRAPHICALFRAMERATEBAR" => true,
        "DEMO_TOGGLE_GREEN_VIEW" => true,
        "DEMO_TOGGLE_HAND_OF_GOD_MODE" => true,
        "DEMO_TOGGLE_HURT_ME_MODE" => true,
        "DEMO_TOGGLE_LETTERBOX" => true,
        "DEMO_TOGGLE_MESSAGE_TEXT" => true,
        "DEMO_TOGGLE_METRICS" => true,
        "DEMO_TOGGLE_MILITARY_SUBTITLES" => true,
        "DEMO_TOGGLE_MOTION_BLUR_ZOOM" => true,
        "DEMO_TOGGLE_MUSIC" => true,
        "DEMO_TOGGLE_NETWORK" => true,
        "DEMO_TOGGLE_NO_DRAW" => true,
        "DEMO_TOGGLE_PARTICLEDEBUG" => true,
        "DEMO_TOGGLE_PROJECTILEDEBUG" => true,
        "DEMO_TOGGLE_RED_VIEW" => true,
        "DEMO_TOGGLE_RENDER" => true,
        "DEMO_TOGGLE_SHADOW_VOLUMES" => true,
        "DEMO_TOGGLE_SOUND" => true,
        "DEMO_TOGGLE_SPECIAL_POWER_DELAYS" => true,
        "DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT" => true,
        "DEMO_TOGGLE_THREATDEBUG" => true,
        "DEMO_TOGGLE_TRACKMARKS" => true,
        "DEMO_TOGGLE_VISIONDEBUG" => true,
        "DEMO_TOGGLE_WATERPLANE" => true,
        "DEMO_TOGGLE_ZOOM_LOCK" => true,
        "DEMO_VTUNE_OFF" => true,
        "DEMO_VTUNE_ON" => true,
        "DEMO_WIN" => true,
        "HELP" => true,
        _ => false,
    }
}

fn is_runtime_command_map_alias(name: &str) -> bool {
    name.eq_ignore_ascii_case("PLACE_BEACON")
        || name.eq_ignore_ascii_case("DELETE_BEACON")
        || name.eq_ignore_ascii_case("TOGGLE_LOWER_DETAILS")
}

fn is_supported_command_map_name(name: &str) -> bool {
    lookup_meta_message_type(name).is_some()
        || is_runtime_command_map_alias(name)
        || is_unimplemented_cpp_command_name(name)
}

fn with_local_player_mut<F>(f: F) -> bool
where
    F: FnOnce(&mut gamelogic::player::Player),
{
    let Some(local_player) = ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
    else {
        return false;
    };

    let Ok(mut local_guard) = local_player.write() else {
        return false;
    };
    f(&mut local_guard);
    true
}

fn parse_block_field(ini: &mut INI) -> INIResult<Option<(String, Vec<String>)>> {
    ini.read_line()?;
    if ini.is_eof() {
        return Err(INIError::EndOfFile);
    }
    let tokens = ini.get_line_tokens();
    let Some(key) = tokens.first() else {
        return Ok(None);
    };
    if key.eq_ignore_ascii_case("End") {
        return Ok(Some((String::from("End"), Vec::new())));
    }
    let mut values = Vec::new();
    for token in tokens.iter().skip(1) {
        if *token != "=" {
            values.push((*token).to_string());
        }
    }
    Ok(Some((key.to_string(), values)))
}

fn parse_transition(value: &str) -> Transition {
    match value.to_ascii_uppercase().as_str() {
        "UP" => Transition::Up,
        "DOUBLEDOWN" => Transition::DoubleDown,
        _ => Transition::Down,
    }
}

fn parse_mod_state(value: &str) -> u32 {
    match value.to_ascii_uppercase().as_str() {
        "CTRL" => MOD_CTRL,
        "ALT" => MOD_ALT,
        "SHIFT" => MOD_SHIFT,
        "CTRL_ALT" => MOD_CTRL | MOD_ALT,
        "SHIFT_CTRL" => MOD_SHIFT | MOD_CTRL,
        "SHIFT_ALT" => MOD_SHIFT | MOD_ALT,
        "SHIFT_ALT_CTRL" => MOD_SHIFT | MOD_ALT | MOD_CTRL,
        _ => 0,
    }
}

fn parse_usable_in(values: &[String]) -> u32 {
    let mut flags = 0;
    for value in values {
        match value.to_ascii_uppercase().as_str() {
            "SHELL" => flags |= COMMANDUSABLE_SHELL,
            "GAME" => flags |= COMMANDUSABLE_GAME,
            _ => {}
        }
    }
    flags
}

fn lookup_key_code(name: &str) -> Option<u32> {
    match name.to_ascii_uppercase().as_str() {
        "KEY_ESC" => Some(0x1B),
        "KEY_BACKSPACE" => Some(0x08),
        "KEY_ENTER" => Some(0x0D),
        "KEY_SPACE" => Some(0x20),
        "KEY_TAB" => Some(0x09),
        "KEY_F1" => Some(0x70),
        "KEY_F2" => Some(0x71),
        "KEY_F3" => Some(0x72),
        "KEY_F4" => Some(0x73),
        "KEY_F5" => Some(0x74),
        "KEY_F6" => Some(0x75),
        "KEY_F7" => Some(0x76),
        "KEY_F8" => Some(0x77),
        "KEY_F9" => Some(0x78),
        "KEY_F10" => Some(0x79),
        "KEY_F11" => Some(0x7A),
        "KEY_F12" => Some(0x7B),
        "KEY_A" => Some(0x41),
        "KEY_B" => Some(0x42),
        "KEY_C" => Some(0x43),
        "KEY_D" => Some(0x44),
        "KEY_E" => Some(0x45),
        "KEY_F" => Some(0x46),
        "KEY_G" => Some(0x47),
        "KEY_H" => Some(0x48),
        "KEY_I" => Some(0x49),
        "KEY_J" => Some(0x4A),
        "KEY_K" => Some(0x4B),
        "KEY_L" => Some(0x4C),
        "KEY_M" => Some(0x4D),
        "KEY_N" => Some(0x4E),
        "KEY_O" => Some(0x4F),
        "KEY_P" => Some(0x50),
        "KEY_Q" => Some(0x51),
        "KEY_R" => Some(0x52),
        "KEY_S" => Some(0x53),
        "KEY_T" => Some(0x54),
        "KEY_U" => Some(0x55),
        "KEY_V" => Some(0x56),
        "KEY_W" => Some(0x57),
        "KEY_X" => Some(0x58),
        "KEY_Y" => Some(0x59),
        "KEY_Z" => Some(0x5A),
        "KEY_1" => Some(0x31),
        "KEY_2" => Some(0x32),
        "KEY_3" => Some(0x33),
        "KEY_4" => Some(0x34),
        "KEY_5" => Some(0x35),
        "KEY_6" => Some(0x36),
        "KEY_7" => Some(0x37),
        "KEY_8" => Some(0x38),
        "KEY_9" => Some(0x39),
        "KEY_0" => Some(0x30),
        "KEY_KP1" => Some(0x61),
        "KEY_KP2" => Some(0x62),
        "KEY_KP3" => Some(0x63),
        "KEY_KP4" => Some(0x64),
        "KEY_KP5" => Some(0x65),
        "KEY_KP6" => Some(0x66),
        "KEY_KP7" => Some(0x67),
        "KEY_KP8" => Some(0x68),
        "KEY_KP9" => Some(0x69),
        "KEY_KP0" => Some(0x60),
        "KEY_KPDEL" => Some(0x6E),
        "KEY_KPSTAR" => Some(0x6A),
        "KEY_KPMINUS" => Some(0x6D),
        "KEY_KPPLUS" => Some(0x6B),
        "KEY_UP" => Some(0x26),
        "KEY_DOWN" => Some(0x28),
        "KEY_LEFT" => Some(0x25),
        "KEY_RIGHT" => Some(0x27),
        "KEY_HOME" => Some(0x24),
        "KEY_END" => Some(0x23),
        "KEY_PGUP" => Some(0x21),
        "KEY_PGDN" => Some(0x22),
        "KEY_INS" => Some(0x2D),
        "KEY_DEL" => Some(0x2E),
        "KEY_MINUS" => Some(0xBD),
        "KEY_EQUAL" => Some(0xBB),
        "KEY_LBRACKET" => Some(0xDB),
        "KEY_RBRACKET" => Some(0xDD),
        "KEY_SEMICOLON" => Some(0xBA),
        "KEY_APOSTROPHE" => Some(0xDE),
        "KEY_TICK" => Some(0xC0),
        "KEY_BACKSLASH" => Some(0xDC),
        "KEY_COMMA" => Some(0xBC),
        "KEY_PERIOD" => Some(0xBE),
        "KEY_SLASH" => Some(0xBF),
        "KEY_KPENTER" => Some(0x0D),
        "KEY_KPSLASH" => Some(0x6F),
        "KEY_NONE" => Some(0),
        _ => None,
    }
}

fn lookup_meta_message_type(name: &str) -> Option<GameMessageType> {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "SAVE_VIEW1" => Some(GameMessageType::MetaSaveView(1)),
        "SAVE_VIEW2" => Some(GameMessageType::MetaSaveView(2)),
        "SAVE_VIEW3" => Some(GameMessageType::MetaSaveView(3)),
        "SAVE_VIEW4" => Some(GameMessageType::MetaSaveView(4)),
        "SAVE_VIEW5" => Some(GameMessageType::MetaSaveView(5)),
        "SAVE_VIEW6" => Some(GameMessageType::MetaSaveView(6)),
        "SAVE_VIEW7" => Some(GameMessageType::MetaSaveView(7)),
        "SAVE_VIEW8" => Some(GameMessageType::MetaSaveView(8)),
        "VIEW_VIEW1" => Some(GameMessageType::MetaViewView(1)),
        "VIEW_VIEW2" => Some(GameMessageType::MetaViewView(2)),
        "VIEW_VIEW3" => Some(GameMessageType::MetaViewView(3)),
        "VIEW_VIEW4" => Some(GameMessageType::MetaViewView(4)),
        "VIEW_VIEW5" => Some(GameMessageType::MetaViewView(5)),
        "VIEW_VIEW6" => Some(GameMessageType::MetaViewView(6)),
        "VIEW_VIEW7" => Some(GameMessageType::MetaViewView(7)),
        "VIEW_VIEW8" => Some(GameMessageType::MetaViewView(8)),
        "CREATE_TEAM0" => Some(GameMessageType::MetaCreateTeam(0)),
        "CREATE_TEAM1" => Some(GameMessageType::MetaCreateTeam(1)),
        "CREATE_TEAM2" => Some(GameMessageType::MetaCreateTeam(2)),
        "CREATE_TEAM3" => Some(GameMessageType::MetaCreateTeam(3)),
        "CREATE_TEAM4" => Some(GameMessageType::MetaCreateTeam(4)),
        "CREATE_TEAM5" => Some(GameMessageType::MetaCreateTeam(5)),
        "CREATE_TEAM6" => Some(GameMessageType::MetaCreateTeam(6)),
        "CREATE_TEAM7" => Some(GameMessageType::MetaCreateTeam(7)),
        "CREATE_TEAM8" => Some(GameMessageType::MetaCreateTeam(8)),
        "CREATE_TEAM9" => Some(GameMessageType::MetaCreateTeam(9)),
        "SELECT_TEAM0" => Some(GameMessageType::MetaSelectTeam(0)),
        "SELECT_TEAM1" => Some(GameMessageType::MetaSelectTeam(1)),
        "SELECT_TEAM2" => Some(GameMessageType::MetaSelectTeam(2)),
        "SELECT_TEAM3" => Some(GameMessageType::MetaSelectTeam(3)),
        "SELECT_TEAM4" => Some(GameMessageType::MetaSelectTeam(4)),
        "SELECT_TEAM5" => Some(GameMessageType::MetaSelectTeam(5)),
        "SELECT_TEAM6" => Some(GameMessageType::MetaSelectTeam(6)),
        "SELECT_TEAM7" => Some(GameMessageType::MetaSelectTeam(7)),
        "SELECT_TEAM8" => Some(GameMessageType::MetaSelectTeam(8)),
        "SELECT_TEAM9" => Some(GameMessageType::MetaSelectTeam(9)),
        "ADD_TEAM0" => Some(GameMessageType::MetaAddTeam(0)),
        "ADD_TEAM1" => Some(GameMessageType::MetaAddTeam(1)),
        "ADD_TEAM2" => Some(GameMessageType::MetaAddTeam(2)),
        "ADD_TEAM3" => Some(GameMessageType::MetaAddTeam(3)),
        "ADD_TEAM4" => Some(GameMessageType::MetaAddTeam(4)),
        "ADD_TEAM5" => Some(GameMessageType::MetaAddTeam(5)),
        "ADD_TEAM6" => Some(GameMessageType::MetaAddTeam(6)),
        "ADD_TEAM7" => Some(GameMessageType::MetaAddTeam(7)),
        "ADD_TEAM8" => Some(GameMessageType::MetaAddTeam(8)),
        "ADD_TEAM9" => Some(GameMessageType::MetaAddTeam(9)),
        "VIEW_TEAM0" => Some(GameMessageType::MetaViewTeam(0)),
        "VIEW_TEAM1" => Some(GameMessageType::MetaViewTeam(1)),
        "VIEW_TEAM2" => Some(GameMessageType::MetaViewTeam(2)),
        "VIEW_TEAM3" => Some(GameMessageType::MetaViewTeam(3)),
        "VIEW_TEAM4" => Some(GameMessageType::MetaViewTeam(4)),
        "VIEW_TEAM5" => Some(GameMessageType::MetaViewTeam(5)),
        "VIEW_TEAM6" => Some(GameMessageType::MetaViewTeam(6)),
        "VIEW_TEAM7" => Some(GameMessageType::MetaViewTeam(7)),
        "VIEW_TEAM8" => Some(GameMessageType::MetaViewTeam(8)),
        "VIEW_TEAM9" => Some(GameMessageType::MetaViewTeam(9)),
        "SELECT_MATCHING_UNITS" => Some(GameMessageType::MetaSelectMatchingUnits),
        "SELECT_NEXT_UNIT" => Some(GameMessageType::MetaSelectNextUnit),
        "SELECT_PREV_UNIT" => Some(GameMessageType::MetaSelectPrevUnit),
        "SELECT_NEXT_WORKER" => Some(GameMessageType::MetaSelectNextWorker),
        "SELECT_PREV_WORKER" => Some(GameMessageType::MetaSelectPrevWorker),
        "SELECT_HERO" => Some(GameMessageType::MetaSelectHero),
        "SELECT_ALL" => Some(GameMessageType::MetaSelectAll),
        "SELECT_ALL_AIRCRAFT" => Some(GameMessageType::MetaSelectAllAircraft),
        "VIEW_COMMAND_CENTER" => Some(GameMessageType::MetaViewCommandCenter),
        "VIEW_LAST_RADAR_EVENT" => Some(GameMessageType::MetaViewLastRadarEvent),
        "SCATTER" => Some(GameMessageType::MetaScatter),
        "STOP" => Some(GameMessageType::MetaStop),
        "DEPLOY" => Some(GameMessageType::MetaDeploy),
        "CREATE_FORMATION" => Some(GameMessageType::MetaCreateFormation),
        "FOLLOW" => Some(GameMessageType::MetaFollow),
        "CHAT_PLAYERS" => Some(GameMessageType::MetaChatPlayers),
        "CHAT_ALLIES" => Some(GameMessageType::MetaChatAllies),
        "CHAT_EVERYONE" => Some(GameMessageType::MetaChatEveryone),
        "DIPLOMACY" => Some(GameMessageType::MetaDiplomacy),
        "OPTIONS" => Some(GameMessageType::MetaOptions),
        "TOGGLE_CONTROL_BAR" => Some(GameMessageType::MetaToggleControlBar),
        "BEGIN_PATH_BUILD" => Some(GameMessageType::MetaBeginPathBuild),
        "END_PATH_BUILD" => Some(GameMessageType::MetaEndPathBuild),
        "BEGIN_FORCEATTACK" => Some(GameMessageType::MetaBeginForceAttack),
        "END_FORCEATTACK" => Some(GameMessageType::MetaEndForceAttack),
        "BEGIN_FORCEMOVE" => Some(GameMessageType::MetaBeginForceMove),
        "END_FORCEMOVE" => Some(GameMessageType::MetaEndForceMove),
        "BEGIN_WAYPOINTS" => Some(GameMessageType::MetaBeginWaypoints),
        "END_WAYPOINTS" => Some(GameMessageType::MetaEndWaypoints),
        "BEGIN_PREFER_SELECTION" => Some(GameMessageType::MetaBeginPreferSelection),
        "END_PREFER_SELECTION" => Some(GameMessageType::MetaEndPreferSelection),
        "TAKE_SCREENSHOT" => Some(GameMessageType::MetaTakeScreenshot),
        "ALL_CHEER" => Some(GameMessageType::MetaAllCheer),
        "BEGIN_CAMERA_ROTATE_LEFT" => Some(GameMessageType::MetaBeginCameraRotateLeft),
        "END_CAMERA_ROTATE_LEFT" => Some(GameMessageType::MetaEndCameraRotateLeft),
        "BEGIN_CAMERA_ROTATE_RIGHT" => Some(GameMessageType::MetaBeginCameraRotateRight),
        "END_CAMERA_ROTATE_RIGHT" => Some(GameMessageType::MetaEndCameraRotateRight),
        "BEGIN_CAMERA_ZOOM_IN" => Some(GameMessageType::MetaBeginCameraZoomIn),
        "END_CAMERA_ZOOM_IN" => Some(GameMessageType::MetaEndCameraZoomIn),
        "BEGIN_CAMERA_ZOOM_OUT" => Some(GameMessageType::MetaBeginCameraZoomOut),
        "END_CAMERA_ZOOM_OUT" => Some(GameMessageType::MetaEndCameraZoomOut),
        "CAMERA_RESET" => Some(GameMessageType::MetaCameraReset),
        "TOGGLE_CAMERA_TRACKING_DRAWABLE" => Some(GameMessageType::MetaToggleCameraTracking),
        "TOGGLE_FAST_FORWARD_REPLAY" => Some(GameMessageType::MetaToggleFastForwardReplay),
        "DEMO_INSTANT_QUIT" => Some(GameMessageType::MetaDemoInstantQuit),
        _ => None,
    }
}

fn dispatch_map_entry(record: &MetaMapRec) -> Option<GameMessageDisposition> {
    if let Some(meta) = &record.meta {
        if matches!(meta, GameMessageType::MetaToggleFastForwardReplay) {
            if TheGameLogic::is_in_replay_game() {
                if let Some(global_data) = get_global_data() {
                    let enabled = {
                        let mut guard = global_data.write();
                        guard.tivo_fast_mode = !guard.tivo_fast_mode;
                        guard.tivo_fast_mode
                    };
                    TheInGameUI::message(if enabled { "GUI:FF_ON" } else { "GUI:FF_OFF" });
                }
            }
            return Some(GameMessageDisposition::KeepMessage);
        }
        emit_message(GameMessage::new(meta.clone()));
        return Some(GameMessageDisposition::DestroyMessage);
    }

    // Runtime CommandMap currently relies on these aliases. Keep behavior close to C++:
    // consume the key regardless of whether runtime game-state allows the command.
    if record.name.eq_ignore_ascii_case("PLACE_BEACON") {
        if can_enter_place_beacon_mode() {
            const CMD_NEED_TARGET_POS: u32 = 0x0000_0020;
            TheInGameUI::clear_pending_special_power();
            TheInGameUI::set_pending_command(CommandType::PlaceBeacon, CMD_NEED_TARGET_POS, 0);
            TheInGameUI::set_force_attack_mode(false);
            TheInGameUI::set_force_move_to_mode(false);
            TheInGameUI::set_prefer_selection_mode(false);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DELETE_BEACON") {
        if TheGameLogic::is_in_multiplayer_game() && !TheGameLogic::is_in_replay_game() {
            emit_message(GameMessage::new(GameMessageType::RemoveBeacon(
                Coord3D::default(),
            )));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("TOGGLE_LOWER_DETAILS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            if let Ok(mut state) = get_lower_detail_toggle_state().write() {
                if state.is_low_details {
                    global.use_shadow_volumes = state.old_use_shadow_volumes;
                    global.use_light_map = state.old_use_light_map;
                    global.use_cloud_map = state.old_use_cloud_map;
                    global.max_particle_count = state.old_max_particle_count;
                    TheGameLogic::set_show_behind_building_markers(
                        state.old_show_behind_building_markers,
                    );
                    TheInGameUI::message("GUI:ReturnGraphicsToPreviousSettings");
                } else {
                    state.old_use_shadow_volumes = global.use_shadow_volumes;
                    global.use_shadow_volumes = false;

                    state.old_use_light_map = global.use_light_map;
                    global.use_light_map = false;

                    state.old_use_cloud_map = global.use_cloud_map;
                    global.use_cloud_map = false;

                    state.old_show_behind_building_markers =
                        TheGameLogic::get_show_behind_building_markers();
                    TheGameLogic::set_show_behind_building_markers(false);

                    state.old_max_particle_count = global.max_particle_count;
                    global.max_particle_count = DROPPED_MAX_PARTICLE_COUNT;

                    TheInGameUI::message("GUI:DetailsSetToLowest");
                }

                state.is_low_details = !state.is_low_details;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_NO_DRAW") {
        // C++ CommandXlat.cpp handles MSG_NO_DRAW by setting m_noDraw = 2^32 - 1.
        // This keeps CommandMap demo/debug parity without requiring MSG_NO_DRAW typing yet.
        if let Some(global_data) = get_global_data() {
            global_data.write().no_draw = u32::MAX;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_ADD_CASH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.get_money_mut().deposit_money(10_000);
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_ADDCASH") {
        let _ = with_local_player_mut(|player| {
            player.get_money_mut().deposit_money(10_000);
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_INSTANT_BUILD") {
        if !TheGameLogic::is_in_multiplayer_game() {
            #[cfg(any(debug_assertions, feature = "internal"))]
            {
                let _ = with_local_player_mut(|player| {
                    player.toggle_instant_build();
                });
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_INSTANT_BUILD") {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = with_local_player_mut(|player| {
                player.toggle_instant_build();
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_FREE_BUILD") {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = with_local_player_mut(|player| {
                player.toggle_free_build();
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_REMOVE_PREREQ") {
        #[cfg(any(debug_assertions, feature = "internal"))]
        {
            let _ = with_local_player_mut(|player| {
                player.toggle_ignore_prereqs();
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_GIVE_SCIENCEPURCHASEPOINTS")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.add_science_purchase_points(1);
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_GIVE_SCIENCEPURCHASEPOINTS")
    {
        let _ = with_local_player_mut(|player| {
            player.add_science_purchase_points(1);
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_GIVE_ALL_SCIENCES") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                if let Some(science_store) = get_science_store() {
                    for (&science, _) in science_store.iter() {
                        if science != SCIENCE_INVALID && science_store.is_science_grantable(science)
                        {
                            let _ = player.grant_science(science);
                        }
                    }
                }
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_GIVE_ALL_SCIENCES") {
        let _ = with_local_player_mut(|player| {
            if let Some(science_store) = get_science_store() {
                for (&science, _) in science_store.iter() {
                    if science != SCIENCE_INVALID && science_store.is_science_grantable(science) {
                        let _ = player.grant_science(science);
                    }
                }
            }
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SHADOW_VOLUMES")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.use_shadow_volumes = !global.use_shadow_volumes;
            global.use_shadow_decals = !global.use_shadow_decals;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_FOGOFWAR") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.fog_of_war_on = !global.fog_of_war_on;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_TRACKMARKS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.make_track_marks = !global.make_track_marks;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_WATERPLANE") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.use_water_plane = !global.use_water_plane;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_RENDER") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.disable_render = !global.disable_render;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_BEHIND_BUILDINGS")
    {
        let show_markers = TheGameLogic::get_show_behind_building_markers();
        if show_markers {
            TheGameLogic::set_show_behind_building_markers(false);
            TheInGameUI::message("GUI:ShowBehindBuildings");
        } else {
            TheGameLogic::set_show_behind_building_markers(true);
            TheInGameUI::message("GUI:HideBehindBuildings");
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_supply_center_placement = !global.debug_supply_center_placement;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_CAMERA_DEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_camera = !global.debug_camera;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_VISIONDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_visibility = !global.debug_visibility;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_PROJECTILEDEBUG")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_projectile_path = !global.debug_projectile_path;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_THREATDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_threat_map = !global.debug_threat_map;
            if global.debug_threat_map {
                global.debug_cash_value_map = false;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_CASHMAPDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_cash_value_map = !global.debug_cash_value_map;
            if global.debug_cash_value_map {
                global.debug_threat_map = false;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_GRAPHICALFRAMERATEBAR")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_show_graphical_framerate = !global.debug_show_graphical_framerate;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_SHOW_EXTENTS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_collision_extents = !global.show_collision_extents;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_SHOW_AUDIO_LOCATIONS")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_audio_locations = !global.show_audio_locations;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_SHOW_HEALTH") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_object_health = !global.show_object_health;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_METRICS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_metrics = !global.show_metrics;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SPECIAL_POWER_DELAYS")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.special_power_uses_delay = !global.special_power_uses_delay;
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_FEATHER_WATER")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.feather_water -= 1;
            if global.feather_water < 0 {
                global.feather_water = 5;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_SHOW_HEALTH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            if let Some(global_data) = get_global_data() {
                let mut global = global_data.write();
                global.show_object_health = !global.show_object_health;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_TOGGLE_SPECIAL_POWER_DELAYS")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            if let Some(global_data) = get_global_data() {
                let mut global = global_data.write();
                global.special_power_uses_delay = !global.special_power_uses_delay;
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    // C++ consumes these command-map keybinds by appending the corresponding
    // message type. Rust keeps input parity by consuming even when full message
    // handlers are not ported yet.
    if is_unimplemented_cpp_command_name(&record.name) {
        return Some(GameMessageDisposition::DestroyMessage);
    }

    None
}

fn can_enter_place_beacon_mode() -> bool {
    if !TheGameLogic::is_in_multiplayer_game() || TheGameLogic::is_in_replay_game() {
        return false;
    }

    let Some(local_player) = ThePlayerList()
        .read()
        .ok()
        .and_then(|list| list.get_local_player().cloned())
    else {
        return false;
    };

    let Ok(local_guard) = local_player.read() else {
        return false;
    };
    if !local_guard.is_player_active() {
        return false;
    }

    let net_min_players = get_global_data()
        .map(|data| data.read().net_min_players)
        .unwrap_or(0);
    let is_multiplayer_session = get_game_engine()
        .map(|engine| engine.lock().is_multiplayer_session())
        .unwrap_or(false);
    if net_min_players != 0 && !is_multiplayer_session {
        return false;
    }

    let Some(template_name) = local_guard
        .get_player_template()
        .map(|template| template.beacon_name.clone())
    else {
        return false;
    };
    if template_name.is_empty() {
        return false;
    }

    let Some(beacon_template) = TheThingFactory::find_template(&template_name) else {
        return false;
    };
    let mut count = [0];
    local_guard.count_objects_by_thing_template(
        std::slice::from_ref(&beacon_template),
        false,
        false,
        &mut count,
    );
    debug!(
        "MSG_META_PLACE_BEACON - Player already has {} beacons active",
        count[0]
    );

    let max_beacons = with_multiplayer_settings(|settings| settings.max_beacons_per_player);
    count[0] < max_beacons
}

#[derive(Default)]
pub struct MetaEventTranslator {
    last_key_down: u32,
    last_mod_state: u32,
    mouse_down_position: [ICoord2D; 3],
    next_up_creates_double: [bool; 3],
}

impl MetaEventTranslator {
    pub fn new() -> Self {
        ensure_meta_map_loaded();
        Self::default()
    }
}

impl GameMessageTranslator for MetaEventTranslator {
    fn translate_game_message(&mut self, msg: &GameMessage) -> GameMessageDisposition {
        let mut disp = GameMessageDisposition::KeepMessage;
        let msg_type = msg.get_type();

        if matches!(
            msg_type,
            GameMessageType::RawKeyDown(_) | GameMessageType::RawKeyUp(_)
        ) {
            let key = match msg.get_argument(0) {
                Some(GameMessageArgumentType::Integer(value)) => *value as u32,
                _ => match msg_type {
                    GameMessageType::RawKeyDown(code) | GameMessageType::RawKeyUp(code) => *code,
                    _ => 0,
                },
            };
            let key_state = match msg.get_argument(1) {
                Some(GameMessageArgumentType::Integer(value)) => *value as u32,
                _ => 0,
            };

            let mut new_mod_state = 0;
            if (key_state & KEY_STATE_CONTROL) != 0 {
                new_mod_state |= MOD_CTRL;
            }
            if (key_state & KEY_STATE_SHIFT) != 0 {
                new_mod_state |= MOD_SHIFT;
            }
            if (key_state & KEY_STATE_ALT) != 0 {
                new_mod_state |= MOD_ALT;
            }

            let shell_active = get_shell().is_shell_active();
            let client_frame =
                crate::core::game_client::with_live_game_client_mut(|client| client.get_frame())
                    .unwrap_or(0);

            let map_guard = get_meta_map().read().expect("MetaMap lock poisoned");
            for map in map_guard.iter() {
                // C++ parity: ignore game-only keybinds before the GameClient reaches frame 1.
                // This prevents load-screen input from getting stuck in frame-0 menu transitions.
                if map.usable_in == COMMANDUSABLE_GAME && client_frame < 1 {
                    continue;
                }
                if shell_active && (map.usable_in & COMMANDUSABLE_SHELL) == 0 {
                    continue;
                }
                if !shell_active && (map.usable_in & COMMANDUSABLE_GAME) == 0 {
                    continue;
                }

                if map.key == 0
                    && new_mod_state != self.last_mod_state
                    && ((map.transition == Transition::Up && map.mod_state == self.last_mod_state)
                        || (map.transition == Transition::Down && map.mod_state == new_mod_state))
                {
                    if let Some(new_disp) = dispatch_map_entry(map) {
                        disp = new_disp;
                        break;
                    }
                }

                let transition_matches = match map.transition {
                    Transition::Up => (key_state & KEY_STATE_UP) != 0,
                    Transition::Down => (key_state & KEY_STATE_DOWN) != 0,
                    // C++ currently disables DOUBLEDOWN generation in MetaEvent.cpp.
                    Transition::DoubleDown => false,
                };

                if map.key == key && map.mod_state == new_mod_state && transition_matches {
                    // C++ eats autorepeat for known keys but does not emit the meta-message.
                    if (key_state & KEY_STATE_AUTOREPEAT) != 0 {
                        disp = GameMessageDisposition::DestroyMessage;
                        break;
                    }

                    if let Some(new_disp) = dispatch_map_entry(map) {
                        disp = new_disp;
                        break;
                    }
                }
            }

            if matches!(msg_type, GameMessageType::RawKeyDown(_)) {
                self.last_key_down = key;
            }
            self.last_mod_state = new_mod_state;
        }

        match msg_type {
            GameMessageType::RawMouseLeftButtonDown(pos, ..)
            | GameMessageType::RawMouseMiddleButtonDown(pos, ..)
            | GameMessageType::RawMouseRightButtonDown(pos, ..) => {
                let index = match msg_type {
                    GameMessageType::RawMouseMiddleButtonDown(..) => 1,
                    GameMessageType::RawMouseRightButtonDown(..) => 2,
                    _ => 0,
                };
                self.mouse_down_position[index] = pos.clone();
                self.next_up_creates_double[index] = false;
            }
            GameMessageType::RawMouseLeftDoubleClick(..)
            | GameMessageType::RawMouseMiddleDoubleClick(..)
            | GameMessageType::RawMouseRightDoubleClick(..) => {
                let index = match msg_type {
                    GameMessageType::RawMouseMiddleDoubleClick(..) => 1,
                    GameMessageType::RawMouseRightDoubleClick(..) => 2,
                    _ => 0,
                };
                self.next_up_creates_double[index] = true;
            }
            GameMessageType::RawMouseLeftButtonUp(pos, modifiers, ..)
            | GameMessageType::RawMouseMiddleButtonUp(pos, modifiers, ..)
            | GameMessageType::RawMouseRightButtonUp(pos, modifiers, ..) => {
                let index = match msg_type {
                    GameMessageType::RawMouseMiddleButtonUp(..) => 1,
                    GameMessageType::RawMouseRightButtonUp(..) => 2,
                    _ => 0,
                };

                let region = build_region(&self.mouse_down_position[index], pos);
                let mut region = IRegion2D {
                    x: region.x,
                    y: region.y,
                    width: region.width,
                    height: region.height,
                };

                if region.width.abs() < DRAG_TOLERANCE && region.height.abs() < DRAG_TOLERANCE {
                    region.width = 0;
                    region.height = 0;
                }

                let click_message = if self.next_up_creates_double[index] {
                    self.next_up_creates_double[index] = false;
                    match msg_type {
                        GameMessageType::RawMouseMiddleButtonUp(..) => {
                            GameMessageType::MouseMiddleDoubleClick(region, *modifiers)
                        }
                        GameMessageType::RawMouseRightButtonUp(..) => {
                            GameMessageType::MouseRightDoubleClick(region, *modifiers)
                        }
                        _ => GameMessageType::MouseLeftDoubleClick(region, *modifiers),
                    }
                } else {
                    match msg_type {
                        GameMessageType::RawMouseMiddleButtonUp(..) => {
                            GameMessageType::MouseMiddleClick(region, *modifiers)
                        }
                        GameMessageType::RawMouseRightButtonUp(..) => {
                            GameMessageType::MouseRightClick(region, *modifiers)
                        }
                        _ => GameMessageType::MouseLeftClick(region, *modifiers),
                    }
                };
                emit_message(GameMessage::new(click_message));
            }
            _ => {}
        }

        disp
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gamelogic::player::Player;
    use std::sync::{Arc, RwLock};
    use std::sync::{Mutex, OnceLock};

    fn test_state_lock() -> &'static Mutex<()> {
        static TEST_STATE_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        TEST_STATE_LOCK.get_or_init(|| Mutex::new(()))
    }
    use std::fs;

    fn repo_root() -> PathBuf {
        let mut dir = std::env::current_dir().expect("current_dir");
        loop {
            if dir.join("GeneralsMD").is_dir() && dir.join("windows_game").is_dir() {
                return dir;
            }
            if !dir.pop() {
                panic!("failed to locate repository root");
            }
        }
    }

    fn active_command_map_names() -> Vec<String> {
        let root = repo_root();
        let paths = [
            root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/CommandMap.ini"),
            root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/CommandMapDebug.ini"),
            root.join("windows_game/extracted_big_files_v2/INIZH/Data/INI/CommandMapDemo.ini"),
            root.join("windows_game/extracted_big_files_v2/EnglishZH/Data/English/CommandMap.ini"),
            root.join(
                "windows_game/extracted_big_files_v2/W3DEnglishZH/Data/English/CommandMap.ini",
            ),
        ];

        let mut names = Vec::new();
        for path in paths {
            let Ok(contents) = fs::read_to_string(&path) else {
                continue;
            };
            for line in contents.lines() {
                let line = line.trim_start();
                if line.starts_with(';') {
                    continue;
                }
                let Some(rest) = line.strip_prefix("CommandMap ") else {
                    continue;
                };
                let Some(name) = rest.split_whitespace().find(|token| *token != "=") else {
                    continue;
                };
                names.push(name.to_string());
            }
        }
        names
    }

    fn alias_record(name: &str) -> MetaMapRec {
        MetaMapRec {
            name: name.to_string(),
            meta: None,
            key: 0,
            transition: Transition::Down,
            mod_state: 0,
            usable_in: COMMANDUSABLE_NONE,
            category: String::new(),
            description: String::new(),
            display_name: String::new(),
        }
    }

    #[test]
    fn test_lookup_meta_message_type_uses_cpp_attack_move_spelling() {
        assert_eq!(lookup_meta_message_type("TOGGLE_ATTACKMOVE"), None);
        assert_eq!(lookup_meta_message_type("TOGGLE_ATTACK_MOVE"), None);
        assert!(!is_supported_command_map_name("TOGGLE_ATTACKMOVE"));
        assert!(is_supported_command_map_name("PLACE_BEACON"));
        assert!(is_supported_command_map_name("DELETE_BEACON"));
        assert!(is_supported_command_map_name("TOGGLE_LOWER_DETAILS"));
        assert!(is_supported_command_map_name("DEMO_TOGGLE_SOUND"));
        assert!(is_supported_command_map_name("CHEAT_ADD_CASH"));
        assert!(is_supported_command_map_name("DEBUG_OBJECT_ID_PERFORMANCE"));
        assert!(is_supported_command_map_name("HELP"));
        assert!(!is_supported_command_map_name("DEMO_NOT_A_REAL_COMMAND"));
        assert!(!is_supported_command_map_name("CHEAT_NOT_A_REAL_COMMAND"));
        assert!(!is_supported_command_map_name("DEBUG_NOT_A_REAL_COMMAND"));
        assert!(!is_supported_command_map_name("UNKNOWN_WIDGET"));
    }

    #[test]
    fn test_lookup_key_code_covers_cpp_keypad_entries() {
        assert_eq!(lookup_key_code("KEY_KP0"), Some(0x60));
        assert_eq!(lookup_key_code("KEY_KP9"), Some(0x69));
        assert_eq!(lookup_key_code("KEY_KPDEL"), Some(0x6E));
        assert_eq!(lookup_key_code("KEY_KPSTAR"), Some(0x6A));
        assert_eq!(lookup_key_code("KEY_KPMINUS"), Some(0x6D));
        assert_eq!(lookup_key_code("KEY_KPPLUS"), Some(0x6B));
        assert_eq!(lookup_key_code("KEY_KPSLASH"), Some(0x6F));
        assert_eq!(lookup_key_code("KEY_KPENTER"), Some(0x0D));
        assert_eq!(lookup_key_code("KEY_NONE"), Some(0));
    }

    #[test]
    fn test_discovered_command_map_names_are_either_mapped_or_intentionally_unresolved() {
        let names = active_command_map_names();
        assert!(!names.is_empty());

        for name in names {
            assert!(
                is_supported_command_map_name(&name),
                "unhandled CommandMap entry: {name}"
            );
        }
    }

    #[test]
    fn test_alias_command_map_entries_use_runtime_dispatch_paths() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        assert_eq!(
            dispatch_map_entry(&alias_record("PLACE_BEACON")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DELETE_BEACON")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("TOGGLE_LOWER_DETAILS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_unimplemented_cpp_command_entries_are_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_ADD_CASH")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_NO_DRAW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("HELP")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_demo_toggle_no_draw_sets_cpp_equivalent_runtime_value() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        global_data.write().no_draw = 0;

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_NO_DRAW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(global_data.read().no_draw, u32::MAX);
    }

    #[test]
    fn test_demo_toggle_aliases_apply_cpp_global_data_side_effects() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        {
            let mut global = global_data.write();
            global.use_shadow_volumes = true;
            global.use_shadow_decals = true;
            global.fog_of_war_on = true;
            global.make_track_marks = true;
            global.use_water_plane = true;
            global.disable_render = false;
            global.debug_supply_center_placement = false;
            global.debug_camera = false;
            global.debug_visibility = false;
            global.debug_projectile_path = false;
            global.debug_threat_map = false;
            global.debug_cash_value_map = true;
            global.debug_show_graphical_framerate = false;
            global.show_collision_extents = false;
            global.show_audio_locations = false;
            global.show_object_health = false;
            global.show_metrics = false;
            global.special_power_uses_delay = true;
            global.feather_water = 0;
        }
        TheGameLogic::set_show_behind_building_markers(false);

        let aliases = [
            "DEMO_TOGGLE_SHADOW_VOLUMES",
            "DEMO_TOGGLE_FOGOFWAR",
            "DEMO_TOGGLE_TRACKMARKS",
            "DEMO_TOGGLE_WATERPLANE",
            "DEMO_TOGGLE_RENDER",
            "DEMO_TOGGLE_BEHIND_BUILDINGS",
            "DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT",
            "DEMO_TOGGLE_CAMERA_DEBUG",
            "DEMO_TOGGLE_VISIONDEBUG",
            "DEMO_TOGGLE_PROJECTILEDEBUG",
            "DEMO_TOGGLE_THREATDEBUG",
            "DEMO_TOGGLE_GRAPHICALFRAMERATEBAR",
            "DEMO_SHOW_EXTENTS",
            "DEMO_SHOW_AUDIO_LOCATIONS",
            "DEMO_SHOW_HEALTH",
            "DEMO_TOGGLE_METRICS",
            "DEMO_TOGGLE_SPECIAL_POWER_DELAYS",
            "DEMO_TOGGLE_FEATHER_WATER",
            "DEMO_TOGGLE_CASHMAPDEBUG",
            "CHEAT_SHOW_HEALTH",
            "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS",
        ];
        for alias in aliases {
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                Some(GameMessageDisposition::DestroyMessage),
                "alias {alias} should be consumed"
            );
        }

        let global = global_data.read();
        assert!(!global.use_shadow_volumes);
        assert!(!global.use_shadow_decals);
        assert!(!global.fog_of_war_on);
        assert!(!global.make_track_marks);
        assert!(!global.use_water_plane);
        assert!(global.disable_render);
        assert!(global.debug_supply_center_placement);
        assert!(global.debug_camera);
        assert!(global.debug_visibility);
        assert!(global.debug_projectile_path);
        assert!(!global.debug_threat_map);
        assert!(global.debug_cash_value_map);
        assert!(global.debug_show_graphical_framerate);
        assert!(global.show_collision_extents);
        assert!(global.show_audio_locations);
        assert!(!global.show_object_health);
        assert!(global.show_metrics);
        assert!(global.special_power_uses_delay);
        assert_eq!(global.feather_water, 5);
        assert!(TheGameLogic::get_show_behind_building_markers());
    }

    #[test]
    fn test_demo_cash_and_science_point_aliases_apply_local_player_effects() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut local_guard = local_player.write().expect("player lock");
            local_guard.get_money_mut().set_money(0);
            let spp = local_guard.get_science_purchase_points();
            if spp != 0 {
                local_guard.add_science_purchase_points(-spp);
            }
        }

        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_ADDCASH")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_SCIENCEPURCHASEPOINTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().expect("player lock");
            assert_eq!(local_guard.get_money().get_money(), 10_000);
            assert_eq!(local_guard.get_science_purchase_points(), 1);
        }

        ThePlayerList().write().expect("player list lock").clear();
    }

    #[test]
    fn test_demo_build_mode_aliases_toggle_local_player_debug_flags() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_INSTANT_BUILD")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_FREE_BUILD")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_REMOVE_PREREQ")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().expect("player lock");
            assert!(local_guard.builds_instantly());
            assert!(local_guard.builds_for_free());
            assert!(local_guard.ignores_prereqs());
        }

        ThePlayerList().write().expect("player list lock").clear();
    }
}
