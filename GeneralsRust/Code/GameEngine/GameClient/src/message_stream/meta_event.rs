//! Meta event translator for key and mouse remapping.

use std::collections::HashSet;
use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};
use std::time::Instant;

use game_engine::common::audio::game_audio::{
    get_global_audio_manager, initialize_global_audio_manager, AudioAffect,
};
use game_engine::common::game_engine::get_game_engine;
use game_engine::common::ini::ini_multiplayer::with_multiplayer_settings;
use game_engine::common::ini::{
    get_global_data, register_block_parser, DynamicGameLODLevel, INIError, INILoadType,
    INIResult, TimeOfDay, INI,
};
use game_engine::common::rts::science::{get_science_store, SCIENCE_INVALID};
use log::debug;

use super::game_message::{
    build_region, Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D,
    IRegion2D,
};
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::core::script_action_handler::{
    get_script_display_debug_callback, set_script_display_debug_callback,
    stop_script_display_movie, toggle_script_display_letter_box,
    toggle_script_display_movie_capture,
};
use crate::drawable::drawable_manager::with_drawable_manager_ref;
use crate::display::display::DebugDisplayCallback;
use crate::display::view::{with_tactical_view, FilterMode, FilterType};
use crate::gui::shell::get_shell;
use crate::gui::window_video_manager::with_window_video_manager;
use crate::helpers::{TheControlBar, TheInGameUI};
use crate::message_stream::player_state::{get_local_player_id, set_local_player_id};
use crate::message_stream::selection_xlat::DRAG_TOLERANCE;
use crate::system::DebugDisplay;
use gamelogic::commands::command::CommandType;
use gamelogic::commands::get_selection_manager;
use gamelogic::common::audio::TimeOfDay as LogicTimeOfDay;
use gamelogic::common::types::KindOf;
use gamelogic::common::ModelConditionFlags;
use gamelogic::helpers::{TheAudio, TheGameClient, TheGameLogic, TheThingFactory, TheVictoryConditions};
use gamelogic::object::drawable::Drawable;
use gamelogic::object::registry::OBJECT_REGISTRY;
use gamelogic::player::{PlayerType, ThePlayerList, PLAYER_INDEX_INVALID};
use gamelogic::scripting::engine::get_script_engine;

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
static OBJECTIVE_MOVIE_INDEX: OnceLock<RwLock<i32>> = OnceLock::new();
static MOTION_BLUR_ZOOM_SATURATE: OnceLock<RwLock<bool>> = OnceLock::new();
static CYCLE_LOD_LEVEL_STATE: OnceLock<RwLock<DynamicGameLODLevel>> = OnceLock::new();
static LAST_PLANE_LOCK_OBJECT_ID: OnceLock<RwLock<Option<u32>>> = OnceLock::new();
static VTUNE_ENABLED: OnceLock<RwLock<bool>> = OnceLock::new();
static SKATE_DISTANCE_OVERRIDE: OnceLock<RwLock<f32>> = OnceLock::new();

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

fn get_objective_movie_index() -> &'static RwLock<i32> {
    OBJECTIVE_MOVIE_INDEX.get_or_init(|| RwLock::new(1))
}

fn get_motion_blur_zoom_saturate_state() -> &'static RwLock<bool> {
    MOTION_BLUR_ZOOM_SATURATE.get_or_init(|| RwLock::new(false))
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

fn is_dispatch_handled_cpp_command_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    match upper.as_str() {
        "CHEAT_ADD_CASH"
        | "CHEAT_DESHROUD"
        | "CHEAT_GIVE_ALL_SCIENCES"
        | "CHEAT_GIVE_SCIENCEPURCHASEPOINTS"
        | "CHEAT_INSTANT_BUILD"
        | "CHEAT_KILL_SELECTION"
        | "CHEAT_SHOW_HEALTH"
        | "CHEAT_SWITCH_TEAMS"
        | "CHEAT_TOGGLE_MESSAGE_TEXT"
        | "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS"
        | "DEMO_ADDCASH"
        | "DEMO_BATTLE_CRY"
        | "DEBUG_DUMP_ALL_PLAYER_OBJECTS"
        | "DEBUG_DUMP_PLAYER_OBJECTS"
        | "DEBUG_DRAWABLE_ID_PERFORMANCE"
        | "DEBUG_OBJECT_ID_PERFORMANCE"
        | "DEBUG_SLEEPY_UPDATE_PERFORMANCE"
        | "DEMO_CYCLE_LOD_LEVEL"
        | "DEMO_DECR_ANIM_SKATE_SPEED"
        | "DEMO_DESHROUD"
        | "DEMO_DUMP_ASSETS"
        | "DEMO_ENSHROUD"
        | "DEMO_FREE_BUILD"
        | "DEMO_GIVE_ALL_SCIENCES"
        | "DEMO_GIVE_RANKLEVEL"
        | "DEMO_GIVE_SCIENCEPURCHASEPOINTS"
        | "DEMO_GIVE_VETERANCY"
        | "DEMO_INSTANT_BUILD"
        | "DEMO_INCR_ANIM_SKATE_SPEED"
        | "DEMO_KILL_ALL_ENEMIES"
        | "DEMO_KILL_SELECTION"
        | "DEMO_LOCK_CAMERA_TO_PLANES"
        | "DEMO_LOCK_CAMERA_TO_SELECTION"
        | "DEMO_LOD_DECREASE"
        | "DEMO_LOD_INCREASE"
        | "DEMO_MUSIC_NEXT_TRACK"
        | "DEMO_MUSIC_PREV_TRACK"
        | "DEMO_NEXT_OBJECTIVE_MOVIE"
        | "DEMO_PERFORM_STATISTICAL_DUMP"
        | "DEMO_PLAY_CAMEO_MOVIE"
        | "DEMO_REMOVE_PREREQ"
        | "DEMO_SHOW_AUDIO_LOCATIONS"
        | "DEMO_SHOW_EXTENTS"
        | "DEMO_SHOW_HEALTH"
        | "DEMO_SWITCH_TEAMS"
        | "DEMO_SWITCH_TEAMS_CHINA_USA"
        | "DEMO_SWITCH_TEAMS_BETWEEN_CHINA_USA"
        | "DEMO_TAKE_RANKLEVEL"
        | "DEMO_TAKE_VETERANCY"
        | "DEMO_TIME_OF_DAY"
        | "DEMO_TOGGLE_AI_DEBUG"
        | "DEMO_TOGGLE_AUDIODEBUG"
        | "DEMO_TOGGLE_AVI"
        | "DEMO_TOGGLE_BEHIND_BUILDINGS"
        | "DEMO_TOGGLE_CASHMAPDEBUG"
        | "DEMO_TOGGLE_CAMERA_DEBUG"
        | "DEMO_TOGGLE_DEBUG_STATS"
        | "DEMO_TOGGLE_FEATHER_WATER"
        | "DEMO_TOGGLE_FOGOFWAR"
        | "DEMO_TOGGLE_GRAPHICALFRAMERATEBAR"
        | "DEMO_TOGGLE_GREEN_VIEW"
        | "DEMO_TOGGLE_MESSAGE_TEXT"
        | "DEMO_TOGGLE_METRICS"
        | "DEMO_TOGGLE_MILITARY_SUBTITLES"
        | "DEMO_TOGGLE_MOTION_BLUR_ZOOM"
        | "DEMO_TOGGLE_MUSIC"
        | "DEMO_TOGGLE_NETWORK"
        | "DEMO_TOGGLE_NO_DRAW"
        | "DEMO_TOGGLE_PARTICLEDEBUG"
        | "DEMO_TOGGLE_PROJECTILEDEBUG"
        | "DEMO_TOGGLE_RED_VIEW"
        | "DEMO_TOGGLE_RENDER"
        | "DEMO_TOGGLE_LETTERBOX"
        | "DEMO_TOGGLE_SHADOW_VOLUMES"
        | "DEMO_TOGGLE_SOUND"
        | "DEMO_TOGGLE_SPECIAL_POWER_DELAYS"
        | "DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT"
        | "DEMO_TOGGLE_THREATDEBUG"
        | "DEMO_TOGGLE_TRACKMARKS"
        | "DEMO_TOGGLE_VISIONDEBUG"
        | "DEMO_TOGGLE_WATERPLANE"
        | "DEMO_TOGGLE_ZOOM_LOCK"
        | "DEMO_VTUNE_OFF"
        | "DEMO_VTUNE_ON"
        | "HELP"
        | "DEMO_WIN" => true,
        _ => {
            parse_runscript_alias(&upper).is_some() || parse_objective_movie_alias(&upper).is_some()
        }
    }
}

fn is_unimplemented_cpp_command_name(name: &str) -> bool {
    // C++ MetaEvent.cpp table entries that exist in CommandMap files but are not
    // represented as typed Rust messages yet. Keep these accepted/consumed so keybind
    // behavior stays aligned while the full message pipeline is still being ported.
    if is_dispatch_handled_cpp_command_name(name) {
        return false;
    }

    match name.to_ascii_uppercase().as_str() {
        "CHEAT_TOGGLE_HAND_OF_GOD_MODE" => true,
        "DEMO_BEGIN_ADJUST_FOV" => true,
        "DEMO_BEGIN_ADJUST_PITCH" => true,
        "DEMO_CYCLE_EXTENT_TYPE" => true,
        "DEMO_DEBUG_SELECTION" => true,
        "DEMO_DECR_EXTENT_HEIGHT" => true,
        "DEMO_DECR_EXTENT_HEIGHT_LARGE" => true,
        "DEMO_DECR_EXTENT_MAJOR" => true,
        "DEMO_DECR_EXTENT_MAJOR_LARGE" => true,
        "DEMO_DECR_EXTENT_MINOR" => true,
        "DEMO_DECR_EXTENT_MINOR_LARGE" => true,
        "DEMO_END_ADJUST_FOV" => true,
        "DEMO_END_ADJUST_PITCH" => true,
        "DEMO_INCR_EXTENT_HEIGHT" => true,
        "DEMO_INCR_EXTENT_HEIGHT_LARGE" => true,
        "DEMO_INCR_EXTENT_MAJOR" => true,
        "DEMO_INCR_EXTENT_MAJOR_LARGE" => true,
        "DEMO_INCR_EXTENT_MINOR" => true,
        "DEMO_INCR_EXTENT_MINOR_LARGE" => true,
        "DEMO_TEST_SURRENDER" => true,
        "DEMO_TOGGLE_BW_VIEW" => true,
        "DEMO_TOGGLE_HAND_OF_GOD_MODE" => true,
        "DEMO_TOGGLE_HURT_ME_MODE" => true,
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
        || is_dispatch_handled_cpp_command_name(name)
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

fn parse_objective_movie_alias(name: &str) -> Option<i32> {
    let upper = name.to_ascii_uppercase();
    let suffix = upper.strip_prefix("DEMO_PLAY_OBJECTIVE_MOVIE")?;
    let value = suffix.parse::<i32>().ok()?;
    if (1..=6).contains(&value) {
        Some(value)
    } else {
        None
    }
}

fn parse_runscript_alias(name: &str) -> Option<(bool, i32)> {
    let upper = name.to_ascii_uppercase();
    if let Some(suffix) = upper.strip_prefix("CHEAT_RUNSCRIPT") {
        let value = suffix.parse::<i32>().ok()?;
        return (1..=9).contains(&value).then_some((true, value));
    }

    if let Some(suffix) = upper.strip_prefix("DEMO_RUNSCRIPT") {
        let value = suffix.parse::<i32>().ok()?;
        return (1..=9).contains(&value).then_some((false, value));
    }

    None
}

fn audio_debug_display_callback(
    _display: &mut DebugDisplay,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

fn particle_system_debug_display_callback(
    _display: &mut DebugDisplay,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

fn stat_debug_display_callback(
    _display: &mut DebugDisplay,
    _user_data: Option<&mut dyn std::any::Any>,
) {
}

fn toggle_script_display_debug_callback(target: DebugDisplayCallback) {
    let active = get_script_display_debug_callback();
    let same_callback = active
        .map(|callback| callback as usize == target as usize)
        .unwrap_or(false);
    let _ = set_script_display_debug_callback(if same_callback { None } else { Some(target) });
}

fn toggle_demo_network_runtime() {
    #[cfg(not(feature = "network"))]
    {
        if let Some(network) = game_network::get_network() {
            network.toggle_network_on();
        }
    }

    #[cfg(feature = "network")]
    {
        let _ = game_network::get_network();
    }
}

fn vtune_enabled_state() -> &'static RwLock<bool> {
    VTUNE_ENABLED.get_or_init(|| RwLock::new(false))
}

fn set_vtune_enabled(enabled: bool) {
    if let Ok(mut guard) = vtune_enabled_state().write() {
        *guard = enabled;
    }
}

#[cfg(test)]
fn is_vtune_enabled_for_tests() -> bool {
    vtune_enabled_state()
        .read()
        .map(|guard| *guard)
        .unwrap_or(false)
}

fn skate_distance_override_state() -> &'static RwLock<f32> {
    SKATE_DISTANCE_OVERRIDE.get_or_init(|| RwLock::new(0.0))
}

fn adjust_skate_distance_override(delta: f32) -> f32 {
    if let Ok(mut guard) = skate_distance_override_state().write() {
        *guard += delta;
        return *guard;
    }
    0.0
}

#[cfg(test)]
fn set_skate_distance_override_for_tests(value: f32) {
    if let Ok(mut guard) = skate_distance_override_state().write() {
        *guard = value;
    }
}

fn dump_used_map_assets() -> std::io::Result<()> {
    let mut names = Vec::new();
    with_drawable_manager_ref(|manager| {
        for drawable_id in manager.get_all_drawable_ids() {
            let Some(drawable) = manager.get_drawable(drawable_id) else {
                continue;
            };
            let Some(name) = drawable.get_template_name() else {
                continue;
            };
            if !name.is_empty() {
                names.push(name.to_string());
            }
        }
    });
    names.sort();
    names.dedup();

    let mut output = String::new();
    for name in names {
        output.push_str(&name);
        output.push('\n');
    }
    fs::write("UsedMapAssets.txt", output)
}

fn cycle_lod_level_state() -> &'static RwLock<DynamicGameLODLevel> {
    CYCLE_LOD_LEVEL_STATE.get_or_init(|| RwLock::new(DynamicGameLODLevel::VeryHigh))
}

fn cycle_dynamic_lod_level() {
    let next = {
        let mut guard = cycle_lod_level_state().write().expect("LOD cycle lock poisoned");
        *guard = match *guard {
            DynamicGameLODLevel::VeryHigh => DynamicGameLODLevel::High,
            DynamicGameLODLevel::High => DynamicGameLODLevel::Medium,
            DynamicGameLODLevel::Medium => DynamicGameLODLevel::Low,
            _ => DynamicGameLODLevel::VeryHigh,
        };
        *guard
    };

    game_engine::common::game_lod::set_dynamic_lod_from_string(next.to_str());
    let message = format!("Dynamic Game Detail {}", next.to_str());
    TheInGameUI::message(&message);
}

#[cfg(test)]
fn set_cycle_lod_level_state_for_tests(level: DynamicGameLODLevel) {
    if let Ok(mut guard) = cycle_lod_level_state().write() {
        *guard = level;
    }
}

fn last_plane_lock_object_id_state() -> &'static RwLock<Option<u32>> {
    LAST_PLANE_LOCK_OBJECT_ID.get_or_init(|| RwLock::new(None))
}

fn next_plane_camera_lock_object_id() -> Option<u32> {
    let mut candidates: Vec<u32> = Vec::new();
    for object in OBJECT_REGISTRY.get_all_objects() {
        let Ok(object_guard) = object.read() else {
            continue;
        };
        if !object_guard.is_above_terrain() {
            continue;
        }
        if object_guard.is_kind_of(KindOf::Projectile) {
            continue;
        }
        candidates.push(object_guard.get_id());
    }

    if candidates.is_empty() {
        return None;
    }

    let previous = last_plane_lock_object_id_state()
        .read()
        .ok()
        .and_then(|guard| *guard);

    let next = if let Some(previous_id) = previous {
        if let Some(index) = candidates.iter().position(|id| *id == previous_id) {
            candidates[(index + 1) % candidates.len()]
        } else {
            candidates[0]
        }
    } else {
        candidates[0]
    };

    if let Ok(mut guard) = last_plane_lock_object_id_state().write() {
        *guard = Some(next);
    }

    Some(next)
}

#[cfg(test)]
fn set_last_plane_lock_object_id_for_tests(object_id: Option<u32>) {
    if let Ok(mut guard) = last_plane_lock_object_id_state().write() {
        *guard = object_id;
    }
}

fn toggle_bw_color_view(mode: FilterMode) {
    with_tactical_view(|view| {
        if view.get_view_filter_type() == FilterType::BlackAndWhite {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            view.set_fade_parameters(30, -1);
            return;
        }

        view.set_view_filter_mode(mode);
        view.set_view_filter(FilterType::BlackAndWhite);
        view.set_fade_parameters(30, 1);
    });
}

fn toggle_motion_blur_zoom_filter() {
    with_tactical_view(|view| {
        if view.get_view_filter_type() == FilterType::MotionBlur {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            return;
        }

        let saturate = if let Ok(mut state) = get_motion_blur_zoom_saturate_state().write() {
            let current = *state;
            *state = !*state;
            current
        } else {
            false
        };

        let mut mode = if saturate {
            FilterMode::MBInAndOutSaturate
        } else {
            FilterMode::MBInAndOutAlpha
        };
        if view.camera_lock_id().is_some() {
            mode = FilterMode::MBPanAlpha;
        }

        let mut filter_pos = *view.position();
        filter_pos.x += 200.0;
        filter_pos.y += 200.0;
        view.set_view_filter_pos(&filter_pos);
        view.set_view_filter_mode(mode);
        view.set_view_filter(FilterType::MotionBlur);
    });
}

fn run_key_script_alias(script_index: i32) {
    let script_name = format!("KEY_F{script_index}");
    let script_engine = gamelogic::scripting::engine::get_script_engine();
    let Ok(mut engine_guard) = script_engine.write() else {
        return;
    };
    let Some(engine) = engine_guard.as_mut() else {
        return;
    };
    let _ = engine.execute_subroutine_by_name(&script_name);
}

fn local_selection_object_ids() -> Vec<u32> {
    let selection_manager = get_selection_manager();
    selection_manager
        .read()
        .ok()
        .and_then(|manager| {
            manager
                .get_player_selection_ref(get_local_player_id())
                .map(|selection| selection.get_selected_objects())
        })
        .unwrap_or_default()
}

fn dump_player_object_counts(include_all_objects: bool) {
    let Ok(player_list) = ThePlayerList().read() else {
        return;
    };

    TheInGameUI::message("*******************************");
    TheInGameUI::message("Dumping player object counts");

    for i in 0..player_list.get_player_count() {
        let Some(player_arc) = player_list.get_player(i as i32).cloned() else {
            continue;
        };
        let Ok(player_guard) = player_arc.read() else {
            continue;
        };
        if !player_guard.is_playable_side() {
            continue;
        }

        let mut object_count = 0;
        let mut object_lines: Vec<String> = Vec::new();
        let _ = player_guard.iterate_objects(|object_arc| {
            let Ok(object_guard) = object_arc.read() else {
                return Ok(());
            };
            if object_guard.is_effectively_dead() {
                return Ok(());
            }

            object_count += 1;
            if include_all_objects || object_count <= 5 {
                object_lines.push(format!(
                    "Object {} ({})",
                    object_guard.get_id(),
                    object_guard.get_template().get_name().to_string()
                ));
            }
            Ok(())
        });

        TheInGameUI::message(&format!(
            "Player {i} ({}) has {object_count} non-dead objects",
            player_guard.get_player_display_name()
        ));

        if object_count > 0 && (include_all_objects || object_count <= 5) {
            TheInGameUI::message("Objects are:");
            for line in object_lines {
                TheInGameUI::message(&line);
            }
        }
    }
}

fn report_object_id_lookup_performance() {
    for number_lookups in [10_000_u32, 100_000_u32, 1_000_000_u32] {
        let start = Instant::now();
        for test_index in 1..number_lookups {
            black_box(TheGameLogic::find_object_by_id(test_index));
        }
        let elapsed = start.elapsed().as_secs_f64();
        let next_index = TheGameLogic::get_object_id_counter();
        TheInGameUI::message(&format!(
            "Time to run {number_lookups} ObjectID lookups is {elapsed:.6}. Next index is {next_index}."
        ));
    }
}

fn report_drawable_id_lookup_performance() {
    let maybe_client = TheGameClient::get();
    for number_lookups in [10_000_u32, 100_000_u32, 1_000_000_u32] {
        let start = Instant::now();
        for test_index in 1..number_lookups {
            let value = maybe_client.and_then(|client| client.find_drawable_by_id(test_index));
            black_box(value);
        }
        let elapsed = start.elapsed().as_secs_f64();
        let next_index = Drawable::get_drawable_id_counter();
        TheInGameUI::message(&format!(
            "Time to run {number_lookups} DrawableID lookups is {elapsed:.6}. Next index is {next_index}."
        ));
    }
}

fn kill_local_player_selection() {
    let selected_ids = local_selection_object_ids();

    for object_id in selected_ids {
        if let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) {
            if let Ok(mut object) = object_arc.write() {
                object.kill(None, None);
            }
        }
    }
}

fn kill_all_enemy_objects_for_local_player() {
    let Some(local_team) = ThePlayerList().read().ok().and_then(|list| {
        list.get_local_player().and_then(|player| {
            player
                .read()
                .ok()
                .and_then(|guard| guard.get_default_team())
        })
    }) else {
        return;
    };
    let Ok(local_team_guard) = local_team.read() else {
        return;
    };

    for object in OBJECT_REGISTRY.get_all_objects() {
        let Ok(mut object_guard) = object.write() else {
            continue;
        };
        let is_enemy = object_guard
            .get_controlling_player()
            .and_then(|player| {
                player
                    .read()
                    .ok()
                    .map(|guard| guard.is_enemy_with_team(&local_team_guard))
            })
            .unwrap_or(false);
        if is_enemy {
            object_guard.kill(None, None);
        }
    }
}

fn first_selected_object_id_for_local_player() -> Option<u32> {
    local_selection_object_ids().into_iter().next()
}

fn adjust_local_selection_veterancy(delta: i32) {
    for object_id in local_selection_object_ids() {
        let Some(object_arc) = TheGameLogic::find_object_by_id(object_id) else {
            continue;
        };
        let Ok(mut object) = object_arc.write() else {
            continue;
        };
        let Some(tracker_arc) = object.get_experience_tracker() else {
            continue;
        };
        let Ok(mut tracker) = tracker_arc.lock() else {
            continue;
        };
        if !tracker.is_trainable() {
            continue;
        }

        let old_level = tracker.get_veterancy_level();
        let new_level = old_level.saturating_add_levels(delta);
        if tracker.set_veterancy_level(new_level).is_some() {
            drop(tracker);
            object.on_veterancy_level_changed(old_level, new_level, true);
        }
    }
}

fn clear_local_player_selection() {
    let local_player_id = get_local_player_id();
    if let Ok(mut manager) = get_selection_manager().write() {
        if manager.get_player_selection_ref(local_player_id).is_none() {
            manager.initialize_player(local_player_id);
        }
        if let Some(selection) = manager.get_player_selection(local_player_id) {
            selection.clear_selection();
        }
    }
}

fn local_player_side_name() -> Option<String> {
    let list = ThePlayerList().read().ok()?;
    let index = list.get_local_player_index();
    if index == PLAYER_INDEX_INVALID || index < 0 {
        return None;
    }
    let player = list.get_player(index)?;
    let guard = player.read().ok()?;
    Some(guard.get_side().to_string())
}

fn local_player_index_u32() -> Option<u32> {
    let list = ThePlayerList().read().ok()?;
    let index = list.get_local_player_index();
    if index == PLAYER_INDEX_INVALID || index < 0 {
        return None;
    }
    Some(index as u32)
}

fn adjust_texture_reduction_factor(delta: i32) {
    let Some(global_data) = get_global_data() else {
        return;
    };
    let mut global = global_data.write();
    global.texture_reduction_factor = (global.texture_reduction_factor + delta).clamp(0, 4);
}

fn reveal_local_player_map_permanently() {
    let Some(player_id) = local_player_index_u32() else {
        return;
    };
    if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        let _ = shroud.reveal_map_for_player_permanently(player_id);
    }
}

fn shroud_local_player_map() {
    let Some(player_id) = local_player_index_u32() else {
        return;
    };
    if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        let _ = shroud.undo_reveal_map_for_player_permanently(player_id);
        let _ = shroud.shroud_map_for_player(player_id);
    }
}

fn apply_local_player_switch_side_effects(initialize_shortcut_bar: bool) {
    clear_local_player_selection();
    if let Some(side) = local_player_side_name() {
        if initialize_shortcut_bar {
            TheControlBar::init_special_power_shortcut_bar_for_player(&side);
        }
        TheControlBar::set_control_bar_scheme_by_player(&side);
    }
}

fn set_local_player_index_with_refresh(index: i32, initialize_shortcut_bar: bool) {
    {
        let Ok(mut list) = ThePlayerList().write() else {
            return;
        };
        list.set_local_player_index(index);
    }
    set_local_player_id(index);
    if let Ok(mut shroud) = gamelogic::system::shroud_manager::get_shroud_manager().lock() {
        shroud.refresh_shroud_for_local_player();
    }
    apply_local_player_switch_side_effects(initialize_shortcut_bar);
}

fn switch_to_next_non_neutral_player() -> bool {
    let Ok(mut list) = ThePlayerList().write() else {
        return false;
    };

    let player_count = list.get_player_count() as i32;
    if player_count <= 0 {
        return false;
    }

    let current = list.get_local_player_index();
    if current == PLAYER_INDEX_INVALID || current < 0 || current >= player_count {
        return false;
    }

    let neutral_index = list.iter().enumerate().find_map(|(idx, player)| {
        let guard = player.read().ok()?;
        if guard.get_player_type() == PlayerType::Neutral {
            Some(idx as i32)
        } else {
            None
        }
    });

    let mut target = current;
    if player_count > 1 {
        let mut idx = current;
        loop {
            idx += 1;
            if idx >= player_count {
                idx = 0;
            }

            if idx == current {
                break;
            }
            if neutral_index == Some(idx) {
                continue;
            }

            target = idx;
            break;
        }
    }

    drop(list);
    set_local_player_index_with_refresh(target, true);
    target != current
}

fn switch_local_player_between_sides(side_a: &str, side_b: &str) -> bool {
    let Ok(list) = ThePlayerList().read() else {
        return false;
    };

    let current = list.get_local_player_index();
    if current == PLAYER_INDEX_INVALID || current < 0 {
        return false;
    }

    let Some(current_player) = list.get_player(current) else {
        return false;
    };
    let Ok(current_guard) = current_player.read() else {
        return false;
    };
    let target_side = if current_guard.get_side().eq_ignore_ascii_case(side_a) {
        side_b
    } else if current_guard.get_side().eq_ignore_ascii_case(side_b) {
        side_a
    } else {
        return false;
    };
    drop(current_guard);

    let target_index = list.iter().enumerate().find_map(|(idx, player)| {
        let guard = player.read().ok()?;
        if guard.get_side().eq_ignore_ascii_case(target_side) {
            Some(idx as i32)
        } else {
            None
        }
    });
    drop(list);

    if let Some(index) = target_index {
        set_local_player_index_with_refresh(index, false);
        true
    } else {
        false
    }
}

fn stop_movies_for_sound_toggle() {
    let _ = stop_script_display_movie();
    with_window_video_manager(|manager| manager.stop_all_movies());
}

fn cycle_music_track(next: bool) -> Option<String> {
    let manager = get_global_audio_manager()?;
    let mut audio = manager.lock().ok()?;

    let script_engine = get_script_engine();
    let mut script_guard = script_engine.write().ok()?;
    let engine = script_guard.as_mut()?;
    let current = engine.get_current_track_name().to_string();
    let next_track = if next {
        audio.next_track_name(&current)
    } else {
        audio.prev_track_name(&current)
    };

    if next_track.is_empty() {
        return None;
    }

    if let Some(action_handler) = engine.action_handler() {
        let _ = action_handler.music_set_track(&next_track, false, false);
    }
    engine.set_current_track_name(next_track.clone());
    Some(next_track)
}

fn map_meta_time_of_day_to_logic_time_of_day(time_of_day: TimeOfDay) -> LogicTimeOfDay {
    match time_of_day {
        TimeOfDay::Morning => LogicTimeOfDay::Morning,
        TimeOfDay::Afternoon => LogicTimeOfDay::Day,
        TimeOfDay::Evening => LogicTimeOfDay::Evening,
        TimeOfDay::Night => LogicTimeOfDay::Night,
        TimeOfDay::Invalid => LogicTimeOfDay::Day,
    }
}

fn refresh_drawable_time_of_day(time_of_day: TimeOfDay) {
    let mapped = map_meta_time_of_day_to_logic_time_of_day(time_of_day);
    for object in OBJECT_REGISTRY.get_all_objects() {
        let drawable = object.read().ok().and_then(|guard| guard.get_drawable());
        let Some(drawable) = drawable else {
            continue;
        };
        let mut drawable_guard = match drawable.write() {
            Ok(guard) => guard,
            Err(_) => continue,
        };
        drawable_guard.set_time_of_day(mapped);
    }
}

fn refresh_drawable_model_conditions() {
    let clear = ModelConditionFlags::empty();
    let set = ModelConditionFlags::empty();
    for object in OBJECT_REGISTRY.get_all_objects() {
        if let Ok(mut object_guard) = object.write() {
            let _ = object_guard.clear_and_set_model_condition_flags(clear, set);
        }
    }
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
                    TheInGameUI::message(if enabled {
                        "m_TiVOFastMode: ON"
                    } else {
                        "m_TiVOFastMode: OFF"
                    });
                }
            }
            return Some(GameMessageDisposition::DestroyMessage);
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

    if record.name.eq_ignore_ascii_case("HELP") {
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_LOD_DECREASE") {
        adjust_texture_reduction_factor(-1);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_LOD_INCREASE") {
        adjust_texture_reduction_factor(1);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_DESHROUD") {
        reveal_local_player_map_permanently();
        return None;
    }

    if record.name.eq_ignore_ascii_case("CHEAT_DESHROUD") {
        if !TheGameLogic::is_in_multiplayer_game() {
            reveal_local_player_map_permanently();
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_ENSHROUD") {
        shroud_local_player_map();
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_DUMP_ASSETS") {
        let _ = dump_used_map_assets();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_VTUNE_ON") {
        set_vtune_enabled(true);
        TheInGameUI::message("VTune Gathering is ON");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_VTUNE_OFF") {
        set_vtune_enabled(false);
        TheInGameUI::message("VTune Gathering is OFF");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_INCR_ANIM_SKATE_SPEED") {
        let value = adjust_skate_distance_override(0.25);
        TheInGameUI::message(&format!("Skate Distance Override is now {value:.6}"));
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_DECR_ANIM_SKATE_SPEED") {
        let value = adjust_skate_distance_override(-0.25);
        TheInGameUI::message(&format!("Skate Distance Override is now {value:.6}"));
        return None;
    }

    if record.name.eq_ignore_ascii_case("CHEAT_ADD_CASH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.get_money_mut().deposit_money(10_000);
            });
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_ADDCASH") {
        if !TheGameLogic::is_in_multiplayer_game() {
            let _ = with_local_player_mut(|player| {
                player.get_money_mut().deposit_money(10_000);
            });
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("CHEAT_INSTANT_BUILD") {
        if !TheGameLogic::is_in_multiplayer_game() {
            #[cfg(any(debug_assertions, feature = "internal"))]
            {
                let _ = with_local_player_mut(|player| {
                    player.toggle_instant_build();
                });
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
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
            TheInGameUI::message("Adding a SciencePurchasePoint");
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_GIVE_SCIENCEPURCHASEPOINTS")
    {
        let _ = with_local_player_mut(|player| {
            player.add_science_purchase_points(1);
        });
        TheInGameUI::message("Adding a SciencePurchasePoint");
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
            TheInGameUI::message("Granting all sciences!");
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("CHEAT_SWITCH_TEAMS") {
        if !TheGameLogic::is_in_multiplayer_game() {
            if TheGameLogic::is_in_game() {
                let _ = switch_to_next_non_neutral_player();
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
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
        TheInGameUI::message("Granting all sciences!");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_GIVE_RANKLEVEL") {
        let _ = with_local_player_mut(|player| {
            let _ = player.set_rank_level(player.get_rank_level() + 1);
        });
        TheInGameUI::message("Adding a RankLevel");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TAKE_RANKLEVEL") {
        let _ = with_local_player_mut(|player| {
            let _ = player.set_rank_level(player.get_rank_level() - 1);
        });
        TheInGameUI::message("Subtracting a RankLevel");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_SWITCH_TEAMS") {
        if TheGameLogic::is_in_game() {
            let _ = switch_to_next_non_neutral_player();
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_SWITCH_TEAMS_CHINA_USA")
        || record
            .name
            .eq_ignore_ascii_case("DEMO_SWITCH_TEAMS_BETWEEN_CHINA_USA")
    {
        let _ = switch_local_player_between_sides("America", "China");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("CHEAT_KILL_SELECTION") {
        if !TheGameLogic::is_in_multiplayer_game() {
            kill_local_player_selection();
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_KILL_SELECTION") {
        kill_local_player_selection();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_KILL_ALL_ENEMIES") {
        kill_all_enemy_objects_for_local_player();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_BATTLE_CRY") {
        if get_global_audio_manager().is_some() {
            let Some(audio) = TheAudio::get() else {
                return Some(GameMessageDisposition::DestroyMessage);
            };
            let misc = TheAudio::get_misc_audio();
            let _ = audio.add_misc_audio_event(&misc.battle_cry_sound);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_GIVE_VETERANCY")
        || record.name.eq_ignore_ascii_case("DEMO_TAKE_VETERANCY")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            let delta = if record.name.eq_ignore_ascii_case("DEMO_GIVE_VETERANCY") {
                1
            } else {
                -1
            };
            adjust_local_selection_veterancy(delta);
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_LOCK_CAMERA_TO_PLANES")
    {
        if let Some(object_id) = next_plane_camera_lock_object_id() {
            with_tactical_view(|view| {
                view.set_camera_lock(Some(object_id));
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_LOCK_CAMERA_TO_SELECTION")
    {
        let selected_id = first_selected_object_id_for_local_player();
        with_tactical_view(|view| {
            let mut next_camera_lock = selected_id;
            if next_camera_lock.is_some() && view.camera_lock_id() == next_camera_lock {
                next_camera_lock = None;
                view.force_redraw();
            }
            view.set_camera_lock(next_camera_lock);
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if let Some((is_cheat_alias, script_index)) = parse_runscript_alias(&record.name) {
        if is_cheat_alias && TheGameLogic::is_in_multiplayer_game() {
            return None;
        }
        run_key_script_alias(script_index);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_SOUND") {
        if let Some(manager) = get_global_audio_manager() {
            if let Ok(mut audio) = manager.lock() {
                if audio.is_on(AudioAffect::Sound) {
                    stop_movies_for_sound_toggle();
                    audio.set_on(false, AudioAffect::All);
                } else {
                    audio.set_on(true, AudioAffect::All);
                }
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_MILITARY_SUBTITLES")
    {
        TheInGameUI::military_subtitle("MSG:Testing", 10_000);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_NEXT_OBJECTIVE_MOVIE")
    {
        if TheGameLogic::is_in_game() {
            let mut next = 1;
            if let Ok(mut objective) = get_objective_movie_index().write() {
                *objective += 1;
                if *objective > 6 {
                    *objective = 1;
                }
                next = *objective;
            }
            let _ = TheInGameUI::play_movie(&format!("DemoObjective{next:02}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if let Some(movie_index) = parse_objective_movie_alias(&record.name) {
        if TheGameLogic::is_in_game() {
            if let Ok(mut objective) = get_objective_movie_index().write() {
                *objective = movie_index;
            }
            let _ = TheInGameUI::play_movie(&format!("DemoObjective{movie_index:02}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_PLAY_CAMEO_MOVIE") {
        if TheGameLogic::is_in_game() {
            const CAMEO_MOVIE: &str = "CameoMovie";
            if !TheInGameUI::is_movie_playing(CAMEO_MOVIE) {
                let _ = TheInGameUI::play_movie(CAMEO_MOVIE);
            } else {
                let target_window = [
                    "ControlBar.wnd:CameoMovieWindow",
                    "ControlBar.wnd:RightHUD",
                ]
                .into_iter()
                .find_map(|window_name| {
                    let window_id =
                        game_engine::common::name_key_generator::NameKeyGenerator::name_to_key(
                            window_name,
                        ) as i32;
                    crate::gui::with_window_manager_ref(|manager| manager.get_window_by_id(window_id))
                });
                if let Some(window) = target_window {
                    with_window_video_manager(|manager| manager.stop_movie(&window));
                }
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TIME_OF_DAY") {
        if let Some(global_data) = get_global_data() {
            let (next_time_of_day, changed_time_of_day, force_model_refresh) = {
                let mut global = global_data.write();
                let tod = match global.time_of_day {
                    TimeOfDay::Morning => TimeOfDay::Afternoon,
                    TimeOfDay::Afternoon => TimeOfDay::Evening,
                    TimeOfDay::Evening => TimeOfDay::Night,
                    TimeOfDay::Night | TimeOfDay::Invalid => TimeOfDay::Morning,
                };
                let changed = global.set_time_of_day(tod);
                (tod, changed, global.force_models_to_follow_time_of_day)
            };
            if changed_time_of_day {
                refresh_drawable_time_of_day(next_time_of_day);
                if force_model_refresh {
                    refresh_drawable_model_conditions();
                }
            }
        }
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
        return None;
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

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_LETTERBOX") {
        if get_shell().is_shell_active() {
            let mut shell = get_shell();
            if let Some(layout) = shell.top() {
                let hide = !layout.is_hidden();
                layout.hide(hide);
            }
        } else {
            let _ = toggle_script_display_letter_box();
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_MOTION_BLUR_ZOOM")
    {
        toggle_motion_blur_zoom_filter();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_RED_VIEW") {
        toggle_bw_color_view(FilterMode::BWRedAndWhite);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_GREEN_VIEW") {
        toggle_bw_color_view(FilterMode::BWGreenAndWhite);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SUPPLY_CENTER_PLACEMENT")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_supply_center_placement = !global.debug_supply_center_placement;
            TheInGameUI::message(if global.debug_supply_center_placement {
                "Log SupplyCenter Placement is ON"
            } else {
                "Log SupplyCenter Placement is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_AI_DEBUG") {
        if let Some(global_data) = get_global_data() {
            let debug_level = {
                let mut global = global_data.write();
                global.debug_ai.value = global.debug_ai.value.saturating_add(1);
                if global.debug_ai.value >= 6 {
                    global.debug_ai.value = 0;
                }
                global.debug_ai.value
            };

            if debug_level == 0 {
                TheInGameUI::message("Debug AI Mode is OFF");
            } else {
                TheInGameUI::message(&format!("Debug AI Mode is Level {}", debug_level));
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_CAMERA_DEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_camera = !global.debug_camera;
            TheInGameUI::message(if global.debug_camera {
                "Debug Camera Mode is On"
            } else {
                "Debug Camera Mode is OFF"
            });
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_VISIONDEBUG") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.debug_visibility = !global.debug_visibility;
            TheInGameUI::message(if global.debug_visibility {
                "Debug Vision Mode is On"
            } else {
                "Debug Vision Mode is OFF"
            });
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
            TheInGameUI::message(if global.debug_projectile_path {
                "Debug Projectile Path Mode is On"
            } else {
                "Debug Projectile Path Mode is OFF"
            });
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
            TheInGameUI::message(if global.debug_threat_map {
                "Debug Threat Map is On"
            } else {
                "Debug Threat Map is OFF"
            });
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
            TheInGameUI::message(if global.debug_cash_value_map {
                "Debug Cash Value Map is On"
            } else {
                "Debug Cash Value Map is OFF"
            });
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
            TheInGameUI::message(if global.show_collision_extents {
                "Show Object Extents ON"
            } else {
                "Show Object Extents OFF"
            });
        }
        return None;
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_SHOW_AUDIO_LOCATIONS")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_audio_locations = !global.show_audio_locations;
            TheInGameUI::message(if global.show_audio_locations {
                "Show AudioLocations ON"
            } else {
                "Show AudioLocations OFF"
            });
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_SHOW_HEALTH") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_object_health = !global.show_object_health;
            TheInGameUI::message(if global.show_object_health {
                "Object Health ON"
            } else {
                "Object Health OFF"
            });
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_METRICS") {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.show_metrics = !global.show_metrics;
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_DEBUG_STATS") {
        toggle_script_display_debug_callback(stat_debug_display_callback);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_DUMP_PLAYER_OBJECTS")
    {
        dump_player_object_counts(false);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_DUMP_ALL_PLAYER_OBJECTS")
    {
        dump_player_object_counts(true);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_OBJECT_ID_PERFORMANCE")
    {
        report_object_id_lookup_performance();
        return None;
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_DRAWABLE_ID_PERFORMANCE")
    {
        report_drawable_id_lookup_performance();
        return None;
    }

    if record
        .name
        .eq_ignore_ascii_case("DEBUG_SLEEPY_UPDATE_PERFORMANCE")
    {
        let count = TheGameLogic::get_number_sleepy_updates();
        TheInGameUI::message(&format!("Number of Sleepy Modules: {count}."));
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_NETWORK") {
        toggle_demo_network_runtime();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_CYCLE_LOD_LEVEL") {
        cycle_dynamic_lod_level();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_PARTICLEDEBUG")
    {
        toggle_script_display_debug_callback(particle_system_debug_display_callback);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_AUDIODEBUG") {
        toggle_script_display_debug_callback(audio_debug_display_callback);
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_AVI") {
        let _ = toggle_script_display_movie_capture();
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_MUSIC") {
        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        if let Ok(mut audio) = manager.lock() {
            if audio.is_on(AudioAffect::Music) {
                audio.set_on(false, AudioAffect::Music);
                TheInGameUI::message("Stopping Music");
            } else {
                audio.set_on(true, AudioAffect::Music);
                TheInGameUI::message("Resuming Music");
            }
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_MUSIC_NEXT_TRACK") {
        if let Some(track_name) = cycle_music_track(true) {
            TheInGameUI::message(&format!("Playing Track: {track_name}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_MUSIC_PREV_TRACK") {
        if let Some(track_name) = cycle_music_track(false) {
            TheInGameUI::message(&format!("Playing Track: {track_name}"));
        }
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_PERFORM_STATISTICAL_DUMP")
    {
        if let Some(global_data) = get_global_data() {
            global_data.write().dump_performance_statistics = true;
        }
        TheInGameUI::message(&format!(
            "Statistics dump made on frame: {}",
            TheGameLogic::get_frame()
        ));
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_WIN") {
        TheVictoryConditions::set_local_allied_victory(true);
        if let Ok(list) = ThePlayerList().read() {
            if let Some(local_player) = list.get_local_player() {
                if let Ok(mut guard) = local_player.write() {
                    guard.set_defeated(false);
                }
            }
        }
        let script_engine = get_script_engine();
        if let Ok(mut guard) = script_engine.write() {
            if let Some(engine) = guard.as_mut() {
                engine.start_end_game_timer();
            }
        }
        TheInGameUI::message("Instant Win");
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_ZOOM_LOCK") {
        let zoom_limited = with_tactical_view(|view| {
            let next = !view.is_zoom_limited();
            view.set_zoom_limited(next);
            next
        });
        TheInGameUI::message(if zoom_limited {
            "Camera Zoom Limit: ON"
        } else {
            "Camera Zoom Limit: OFF"
        });
        return Some(GameMessageDisposition::DestroyMessage);
    }

    if record
        .name
        .eq_ignore_ascii_case("DEMO_TOGGLE_SPECIAL_POWER_DELAYS")
    {
        if let Some(global_data) = get_global_data() {
            let mut global = global_data.write();
            global.special_power_uses_delay = !global.special_power_uses_delay;
            TheInGameUI::message(if global.special_power_uses_delay {
                "Special Power (Superweapon) Delay: ON"
            } else {
                "Special Power (Superweapon) Delay: OFF"
            });
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
                TheInGameUI::message(if global.show_object_health {
                    "Object Health ON"
                } else {
                    "Object Health OFF"
                });
            }
        }
        return None;
    }

    if record
        .name
        .eq_ignore_ascii_case("CHEAT_TOGGLE_MESSAGE_TEXT")
    {
        if !TheGameLogic::is_in_multiplayer_game() {
            TheInGameUI::toggle_messages();
            if TheInGameUI::is_messages_on() {
                TheInGameUI::message("GUI:MessagesOn");
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
    }

    if record.name.eq_ignore_ascii_case("DEMO_TOGGLE_MESSAGE_TEXT") {
        TheInGameUI::toggle_messages();
        if TheInGameUI::is_messages_on() {
            TheInGameUI::message("GUI:MessagesOn");
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
                TheInGameUI::message(if global.special_power_uses_delay {
                    "Special Power (Superweapon) Delay: ON"
                } else {
                    "Special Power (Superweapon) Delay: OFF"
                });
            }
            return Some(GameMessageDisposition::DestroyMessage);
        }
        return None;
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
    use crate::message_stream::player_state::get_local_player_id;
    use game_engine::common::ini::TimeOfDay as GlobalTimeOfDay;
    use gamelogic::player::Player;
    use gamelogic::system::game_logic::{get_game_logic, GAME_LAN, GAME_NONE, GAME_SINGLE_PLAYER};
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
    fn test_fast_forward_replay_meta_record_is_destroyed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let mut record = alias_record("TOGGLE_FAST_FORWARD_REPLAY");
        record.meta = Some(GameMessageType::MetaToggleFastForwardReplay);
        assert_eq!(
            dispatch_map_entry(&record),
            Some(GameMessageDisposition::DestroyMessage)
        );
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
            dispatch_map_entry(&alias_record("DEMO_DECR_EXTENT_MAJOR")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_BW_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BEGIN_ADJUST_FOV")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TEST_SURRENDER")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_dispatch_handled_cpp_command_entries_are_supported_and_not_unimplemented() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        for alias in [
            "CHEAT_ADD_CASH",
            "CHEAT_DESHROUD",
            "CHEAT_RUNSCRIPT3",
            "DEBUG_DUMP_ALL_PLAYER_OBJECTS",
            "DEBUG_DUMP_PLAYER_OBJECTS",
            "DEBUG_DRAWABLE_ID_PERFORMANCE",
            "DEBUG_OBJECT_ID_PERFORMANCE",
            "DEBUG_SLEEPY_UPDATE_PERFORMANCE",
            "DEMO_CYCLE_LOD_LEVEL",
            "DEMO_DECR_ANIM_SKATE_SPEED",
            "DEMO_DESHROUD",
            "DEMO_DUMP_ASSETS",
            "DEMO_INCR_ANIM_SKATE_SPEED",
            "DEMO_KILL_ALL_ENEMIES",
            "DEMO_LOCK_CAMERA_TO_PLANES",
            "DEMO_LOD_DECREASE",
            "DEMO_LOD_INCREASE",
            "DEMO_MUSIC_NEXT_TRACK",
            "DEMO_PLAY_CAMEO_MOVIE",
            "DEMO_PLAY_OBJECTIVE_MOVIE2",
            "DEMO_TOGGLE_AUDIODEBUG",
            "DEMO_TOGGLE_AVI",
            "DEMO_TOGGLE_DEBUG_STATS",
            "DEMO_TOGGLE_GREEN_VIEW",
            "DEMO_TOGGLE_LETTERBOX",
            "DEMO_TOGGLE_MOTION_BLUR_ZOOM",
            "DEMO_TOGGLE_NETWORK",
            "DEMO_TOGGLE_PARTICLEDEBUG",
            "DEMO_TOGGLE_RED_VIEW",
            "DEMO_ENSHROUD",
            "DEMO_VTUNE_OFF",
            "DEMO_VTUNE_ON",
            "HELP",
            "DEMO_WIN",
        ] {
            assert!(is_dispatch_handled_cpp_command_name(alias));
            assert!(!is_unimplemented_cpp_command_name(alias));
            assert!(is_supported_command_map_name(alias));
        }
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
    fn test_demo_lod_aliases_adjust_texture_reduction_factor_with_cpp_clamp() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        global_data.write().texture_reduction_factor = 0;

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_LOD_DECREASE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(global_data.read().texture_reduction_factor, 0);

        for _ in 0..6 {
            assert_eq!(
                dispatch_map_entry(&alias_record("DEMO_LOD_INCREASE")),
                Some(GameMessageDisposition::DestroyMessage)
            );
        }
        assert_eq!(global_data.read().texture_reduction_factor, 4);

        for _ in 0..6 {
            assert_eq!(
                dispatch_map_entry(&alias_record("DEMO_LOD_DECREASE")),
                Some(GameMessageDisposition::DestroyMessage)
            );
        }
        assert_eq!(global_data.read().texture_reduction_factor, 0);
    }

    #[test]
    fn test_deshroud_aliases_follow_cpp_keep_message_semantics() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        assert_eq!(dispatch_map_entry(&alias_record("CHEAT_DESHROUD")), None);
        assert_eq!(dispatch_map_entry(&alias_record("DEMO_DESHROUD")), None);
        assert_eq!(dispatch_map_entry(&alias_record("DEMO_ENSHROUD")), None);
    }

    #[test]
    fn test_help_alias_is_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        assert_eq!(
            dispatch_map_entry(&alias_record("HELP")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_demo_vtune_aliases_toggle_compat_state() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        set_vtune_enabled(false);
        assert!(!is_vtune_enabled_for_tests());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_VTUNE_ON")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(is_vtune_enabled_for_tests());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_VTUNE_OFF")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!is_vtune_enabled_for_tests());
    }

    #[test]
    fn test_demo_skate_speed_aliases_keep_message_and_adjust_value() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        set_skate_distance_override_for_tests(0.0);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_INCR_ANIM_SKATE_SPEED")),
            None
        );
        assert!((adjust_skate_distance_override(0.0) - 0.25).abs() < f32::EPSILON);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_DECR_ANIM_SKATE_SPEED")),
            None
        );
        assert!((adjust_skate_distance_override(0.0) - 0.0).abs() < f32::EPSILON);
    }

    #[test]
    fn test_demo_dump_assets_alias_is_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        let output_path = PathBuf::from("UsedMapAssets.txt");
        let _ = fs::remove_file(&output_path);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_DUMP_ASSETS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(output_path.exists());
        let _ = fs::remove_file(&output_path);
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
            global.debug_ai.value = 0;
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
            "DEMO_TOGGLE_AI_DEBUG",
            "CHEAT_SHOW_HEALTH",
            "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS",
        ];
        for alias in aliases {
            let expected = match alias {
                "DEMO_TOGGLE_RENDER"
                | "DEMO_SHOW_EXTENTS"
                | "DEMO_SHOW_AUDIO_LOCATIONS"
                | "DEMO_SHOW_HEALTH"
                | "DEMO_TOGGLE_METRICS"
                | "CHEAT_SHOW_HEALTH" => None,
                _ => Some(GameMessageDisposition::DestroyMessage),
            };
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                expected,
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
        assert_eq!(global.debug_ai.value, 1);
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

        assert_eq!(dispatch_map_entry(&alias_record("DEMO_ADDCASH")), None);
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_SCIENCEPURCHASEPOINTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().expect("player lock");
            assert_eq!(local_guard.get_money().get_money(), 10_000);
            assert_eq!(local_guard.get_science_purchase_points(), 1);
        }

        assert_eq!(dispatch_map_entry(&alias_record("CHEAT_ADD_CASH")), None);
        assert_eq!(
            local_player
                .read()
                .expect("player lock")
                .get_money()
                .get_money(),
            20_000
        );

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

    #[test]
    fn test_demo_rank_level_aliases_adjust_local_player_rank() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut local_guard = local_player.write().expect("player lock");
            let _ = local_guard.set_rank_level(1);
        }

        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_RANKLEVEL")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_RANKLEVEL")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TAKE_RANKLEVEL")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        {
            let local_guard = local_player.read().expect("player lock");
            assert_eq!(local_guard.get_rank_level(), 2);
        }

        ThePlayerList().write().expect("player list lock").clear();
    }

    #[test]
    fn test_message_text_aliases_toggle_ingame_ui_message_state() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        if !TheInGameUI::is_messages_on() {
            TheInGameUI::toggle_messages();
        }
        assert!(TheInGameUI::is_messages_on());

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MESSAGE_TEXT")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!TheInGameUI::is_messages_on());

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_TOGGLE_MESSAGE_TEXT")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(TheInGameUI::is_messages_on());
    }

    #[test]
    fn test_demo_zoom_lock_alias_toggles_view_zoom_limit() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        with_tactical_view(|view| view.set_zoom_limited(false));
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_ZOOM_LOCK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(crate::display::view::with_tactical_view_ref(
            |view| view.is_zoom_limited()
        ));

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_ZOOM_LOCK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(!crate::display::view::with_tactical_view_ref(
            |view| view.is_zoom_limited()
        ));
    }

    #[test]
    fn test_demo_objective_movie_aliases_update_index_when_in_game() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_SINGLE_PLAYER);
        }
        if let Ok(mut index) = get_objective_movie_index().write() {
            *index = 1;
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_PLAY_OBJECTIVE_MOVIE4")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            *get_objective_movie_index().read().expect("objective lock"),
            4
        );

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_NEXT_OBJECTIVE_MOVIE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            *get_objective_movie_index().read().expect("objective lock"),
            5
        );

        if let Ok(mut index) = get_objective_movie_index().write() {
            *index = 6;
        }
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_NEXT_OBJECTIVE_MOVIE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            *get_objective_movie_index().read().expect("objective lock"),
            1
        );

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
    }

    #[test]
    fn test_demo_military_subtitles_and_time_of_day_aliases_are_wired() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();

        {
            let mut global = global_data.write();
            global.time_of_day = GlobalTimeOfDay::Afternoon;
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MILITARY_SUBTITLES")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TIME_OF_DAY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(global_data.read().time_of_day, GlobalTimeOfDay::Evening);
    }

    #[test]
    fn test_demo_play_cameo_movie_alias_is_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_SINGLE_PLAYER);
        }
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_PLAY_CAMEO_MOVIE")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
    }

    #[test]
    fn test_switch_team_aliases_cycle_or_swap_local_player_index() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let player_usa = Arc::new(RwLock::new(Player::new(0)));
        let player_china = Arc::new(RwLock::new(Player::new(1)));
        let player_neutral = Arc::new(RwLock::new(Player::new(2)));
        {
            let mut usa = player_usa.write().expect("player lock");
            usa.set_side("America");
            usa.set_player_type(PlayerType::Human, false);
        }
        {
            let mut china = player_china.write().expect("player lock");
            china.set_side("China");
            china.set_player_type(PlayerType::Human, false);
        }
        {
            let mut neutral = player_neutral.write().expect("player lock");
            neutral.set_side("Neutral");
            neutral.set_player_type(PlayerType::Neutral, false);
        }

        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&player_usa));
            list.add_player(Arc::clone(&player_china));
            list.add_player(Arc::clone(&player_neutral));
            list.set_local_player_index(0);
        }
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_SINGLE_PLAYER);
        }
        set_local_player_id(0);

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_SWITCH_TEAMS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            ThePlayerList()
                .read()
                .expect("player list lock")
                .get_local_player_index(),
            1
        );
        assert_eq!(get_local_player_id(), 1);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_SWITCH_TEAMS_CHINA_USA")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            ThePlayerList()
                .read()
                .expect("player list lock")
                .get_local_player_index(),
            0
        );
        assert_eq!(get_local_player_id(), 0);

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
        ThePlayerList().write().expect("player list lock").clear();
    }

    #[test]
    fn test_cheat_switch_teams_keeps_message_in_multiplayer() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let player_usa = Arc::new(RwLock::new(Player::new(0)));
        let player_china = Arc::new(RwLock::new(Player::new(1)));
        {
            let mut usa = player_usa.write().expect("player lock");
            usa.set_side("America");
            usa.set_player_type(PlayerType::Human, false);
        }
        {
            let mut china = player_china.write().expect("player lock");
            china.set_side("China");
            china.set_player_type(PlayerType::Human, false);
        }

        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&player_usa));
            list.add_player(Arc::clone(&player_china));
            list.set_local_player_index(0);
        }
        set_local_player_id(0);
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_LAN);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_SWITCH_TEAMS")),
            None
        );
        assert_eq!(
            ThePlayerList()
                .read()
                .expect("player list lock")
                .get_local_player_index(),
            0
        );
        assert_eq!(get_local_player_id(), 0);

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
        ThePlayerList().write().expect("player list lock").clear();
    }

    #[test]
    fn test_multiplayer_gated_cheat_aliases_keep_message() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }
        set_local_player_id(0);
        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_LAN);
        }

        let aliases = [
            "CHEAT_ADD_CASH",
            "CHEAT_GIVE_ALL_SCIENCES",
            "CHEAT_GIVE_SCIENCEPURCHASEPOINTS",
            "CHEAT_INSTANT_BUILD",
            "CHEAT_KILL_SELECTION",
            "CHEAT_RUNSCRIPT3",
            "CHEAT_SHOW_HEALTH",
            "CHEAT_TOGGLE_MESSAGE_TEXT",
            "CHEAT_TOGGLE_SPECIAL_POWER_DELAYS",
        ];
        for alias in aliases {
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                None,
                "{alias} should keep message in multiplayer"
            );
        }

        if let Ok(mut logic) = get_game_logic().lock() {
            logic.set_game_mode(GAME_NONE);
        }
        ThePlayerList().write().expect("player list lock").clear();
    }

    #[test]
    fn test_demo_toggle_sound_and_music_aliases_update_audio_flags() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let manager = get_global_audio_manager().unwrap_or_else(initialize_global_audio_manager);
        {
            let mut audio = manager.lock().expect("audio lock");
            audio.set_on(true, AudioAffect::All);
            audio.set_on(true, AudioAffect::Music);
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_SOUND")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().expect("audio lock");
            assert!(!audio.is_on(AudioAffect::Sound));
            assert!(!audio.is_on(AudioAffect::Music));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_SOUND")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().expect("audio lock");
            assert!(audio.is_on(AudioAffect::Sound));
            assert!(audio.is_on(AudioAffect::Music));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MUSIC")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().expect("audio lock");
            assert!(!audio.is_on(AudioAffect::Music));
            assert!(audio.is_on(AudioAffect::Sound));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MUSIC")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        {
            let audio = manager.lock().expect("audio lock");
            assert!(audio.is_on(AudioAffect::Music));
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_MUSIC_NEXT_TRACK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_MUSIC_PREV_TRACK")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_demo_debug_display_and_movie_capture_aliases_are_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        for alias in [
            "DEMO_TOGGLE_DEBUG_STATS",
            "DEMO_TOGGLE_PARTICLEDEBUG",
            "DEMO_TOGGLE_AUDIODEBUG",
            "DEMO_TOGGLE_AVI",
        ] {
            assert_eq!(
                dispatch_map_entry(&alias_record(alias)),
                Some(GameMessageDisposition::DestroyMessage),
                "alias {alias} should be consumed"
            );
        }
    }

    #[test]
    fn test_demo_view_filter_aliases_toggle_expected_filter_modes() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        with_tactical_view(|view| {
            view.set_view_filter_mode(FilterMode::Null);
            view.set_view_filter(FilterType::Null);
            view.set_fade_parameters(0, -1);
            view.set_camera_lock(None);
            view.set_position(&crate::display::view::Point3::new(0.0, 0.0, 0.0));
        });
        if let Ok(mut saturate) = get_motion_blur_zoom_saturate_state().write() {
            *saturate = false;
        }

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_RED_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::BlackAndWhite);
            assert_eq!(view.get_view_filter_mode(), FilterMode::BWRedAndWhite);
        });
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_RED_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::Null);
            assert_eq!(view.get_view_filter_mode(), FilterMode::Null);
        });

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_GREEN_VIEW")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::BlackAndWhite);
            assert_eq!(view.get_view_filter_mode(), FilterMode::BWGreenAndWhite);
        });

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MOTION_BLUR_ZOOM")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::MotionBlur);
            assert_eq!(view.get_view_filter_mode(), FilterMode::MBInAndOutAlpha);
        });
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_MOTION_BLUR_ZOOM")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        with_tactical_view(|view| {
            assert_eq!(view.get_view_filter_type(), FilterType::Null);
            assert_eq!(view.get_view_filter_mode(), FilterMode::Null);
        });
    }

    #[test]
    fn test_demo_toggle_network_alias_is_consumed_and_toggles_compat_state() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TOGGLE_NETWORK")),
            Some(GameMessageDisposition::DestroyMessage)
        );

        #[cfg(not(feature = "network"))]
        {
            if let Some(network) = game_network::get_network() {
                let current = network.is_network_on();
                assert_eq!(
                    dispatch_map_entry(&alias_record("DEMO_TOGGLE_NETWORK")),
                    Some(GameMessageDisposition::DestroyMessage)
                );
                assert_eq!(network.is_network_on(), !current);
            }
        }
    }

    #[test]
    fn test_demo_cycle_lod_level_alias_matches_cpp_decrement_wrap_order() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        set_cycle_lod_level_state_for_tests(DynamicGameLODLevel::VeryHigh);

        for expected in ["High", "Medium", "Low", "VeryHigh"] {
            assert_eq!(
                dispatch_map_entry(&alias_record("DEMO_CYCLE_LOD_LEVEL")),
                Some(GameMessageDisposition::DestroyMessage)
            );
            assert_eq!(game_engine::common::game_lod::get_dynamic_lod(), expected);
        }
    }

    #[test]
    fn test_debug_dump_player_object_aliases_are_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_DUMP_PLAYER_OBJECTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_DUMP_ALL_PLAYER_OBJECTS")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_debug_sleepy_update_performance_alias_keeps_message() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_SLEEPY_UPDATE_PERFORMANCE")),
            None
        );
    }

    #[test]
    fn test_debug_id_performance_aliases_keep_message() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_OBJECT_ID_PERFORMANCE")),
            None
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEBUG_DRAWABLE_ID_PERFORMANCE")),
            None
        );
    }

    #[test]
    fn test_demo_perform_statistical_dump_sets_dump_flag() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        let global_data = game_engine::common::ini::ini_game_data::ensure_global_data();
        global_data.write().dump_performance_statistics = false;

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_PERFORM_STATISTICAL_DUMP")),
            None
        );
        assert!(global_data.read().dump_performance_statistics);
    }

    #[test]
    fn test_demo_win_alias_sets_local_victory_state() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        let local_player = Arc::new(RwLock::new(Player::new(0)));
        {
            let mut guard = local_player.write().expect("player lock");
            guard.set_defeated(true);
        }
        {
            let mut list = ThePlayerList().write().expect("player list lock");
            list.clear();
            list.add_player(Arc::clone(&local_player));
            list.set_local_player_index(0);
        }
        TheVictoryConditions::set_local_allied_victory(false);

        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_WIN")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert!(TheVictoryConditions::is_local_allied_victory());
        assert!(!local_player.read().expect("player lock").is_defeated());

        ThePlayerList().write().expect("player list lock").clear();
    }

    #[test]
    fn test_runscript_alias_parsing_accepts_cpp_ranges() {
        assert_eq!(parse_runscript_alias("CHEAT_RUNSCRIPT1"), Some((true, 1)));
        assert_eq!(parse_runscript_alias("CHEAT_RUNSCRIPT9"), Some((true, 9)));
        assert_eq!(parse_runscript_alias("DEMO_RUNSCRIPT2"), Some((false, 2)));
        assert_eq!(parse_runscript_alias("DEMO_RUNSCRIPT9"), Some((false, 9)));
        assert_eq!(parse_runscript_alias("CHEAT_RUNSCRIPT0"), None);
        assert_eq!(parse_runscript_alias("DEMO_RUNSCRIPT10"), None);
    }

    #[test]
    fn test_demo_battle_cry_alias_is_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_BATTLE_CRY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_kill_selection_and_runscript_aliases_are_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        if let Ok(mut manager) = get_selection_manager().write() {
            manager.initialize_player(0);
            if let Some(selection) = manager.get_player_selection(0) {
                selection.clear_selection();
            }
        }
        set_local_player_id(0);

        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_KILL_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_KILL_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_KILL_ALL_ENEMIES")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("CHEAT_RUNSCRIPT3")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_RUNSCRIPT7")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_GIVE_VETERANCY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_TAKE_VETERANCY")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }

    #[test]
    fn test_demo_lock_camera_to_selection_alias_clears_lock_when_no_selection() {
        let _guard = test_state_lock().lock().expect("lock poisoned");

        if let Ok(mut manager) = get_selection_manager().write() {
            manager.initialize_player(0);
            if let Some(selection) = manager.get_player_selection(0) {
                selection.clear_selection();
            }
        }
        set_local_player_id(0);

        with_tactical_view(|view| view.set_camera_lock(Some(42)));
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_LOCK_CAMERA_TO_SELECTION")),
            Some(GameMessageDisposition::DestroyMessage)
        );
        assert_eq!(
            crate::display::view::with_tactical_view_ref(|view| view.camera_lock_id()),
            None
        );
    }

    #[test]
    fn test_demo_lock_camera_to_planes_alias_is_consumed() {
        let _guard = test_state_lock().lock().expect("lock poisoned");
        set_last_plane_lock_object_id_for_tests(None);
        assert_eq!(
            dispatch_map_entry(&alias_record("DEMO_LOCK_CAMERA_TO_PLANES")),
            Some(GameMessageDisposition::DestroyMessage)
        );
    }
}
