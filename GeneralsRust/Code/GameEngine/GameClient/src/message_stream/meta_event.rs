//! Meta event translator for key and mouse remapping.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{OnceLock, RwLock};

use game_engine::common::ini::{
    get_global_data, register_block_parser, INIError, INILoadType, INIResult, INI,
};
use log::debug;

use super::game_message::{
    build_region, Coord3D, GameMessage, GameMessageArgumentType, GameMessageType, ICoord2D,
    IRegion2D,
};
use super::message_stream::{emit_message, GameMessageDisposition, GameMessageTranslator};
use crate::helpers::TheInGameUI;
use crate::gui::shell::get_shell;
use crate::message_stream::selection_xlat::DRAG_TOLERANCE;
use gamelogic::commands::command::CommandType;
use gamelogic::helpers::TheGameLogic;

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
    let mut roots = HashSet::<PathBuf>::new();
    roots.insert(PathBuf::from("."));
    if let Ok(current) = std::env::current_dir() {
        roots.insert(current.clone());
        for ancestor in current.ancestors() {
            roots.insert(ancestor.to_path_buf());
        }
    }

    if let Some(global) = get_global_data() {
        let mod_dir = global.read().mod_dir.clone();
        if !mod_dir.trim().is_empty() {
            roots.insert(PathBuf::from(mod_dir.trim()));
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

    let meta = lookup_meta_message_type(&name);
    let has_custom_handler =
        name.eq_ignore_ascii_case("PLACE_BEACON") || name.eq_ignore_ascii_case("DELETE_BEACON");
    // Temporary until the full command-table parity work lands: let debug/demo-only
    // entries load even when GameMessageType coverage is still incomplete.
    if meta.is_none() && !has_custom_handler && !is_unresolved_command_name_allowed(&name) {
        return Err(INIError::InvalidData);
    }

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

fn is_unresolved_command_name_allowed(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    upper.starts_with("DEMO_")
        || upper.starts_with("CHEAT_")
        || upper.starts_with("DEBUG_")
        || upper == "HELP"
        || upper == "TOGGLE_LOWER_DETAILS"
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
                        "GUI:FF_ON"
                    } else {
                        "GUI:FF_OFF"
                    });
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
        if TheGameLogic::is_in_multiplayer_game() && !TheGameLogic::is_in_replay_game() {
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
            emit_message(GameMessage::new(GameMessageType::RemoveBeacon(Coord3D::default())));
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

    None
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

            let map_guard = get_meta_map().read().expect("MetaMap lock poisoned");
            for map in map_guard.iter() {
                // C++ parity: ignore game-only keybinds before frame 1 to avoid load-screen input bugs.
                if map.usable_in == COMMANDUSABLE_GAME && TheGameLogic::get_frame() < 1 {
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
