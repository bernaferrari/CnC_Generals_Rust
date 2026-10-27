use crate::game_logic::GameLogic;
use crate::save_load::*;
use game_engine::common::system::save_game::GameState as CommonGameState;
use game_engine::common::system::save_game::GameStateMap as CommonGameStateMap;
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "save_file/decoded.rs"]
mod decoded;
#[path = "save_file/map_admission.rs"]
mod map_admission;
pub use decoded::DecodedGameSave;

/// Same tokens as C++ `GameState::xferSaveData` (`GameState.cpp:1313-1458`)
/// and System `TheGameState` (`SAVELOAD_BLOCK_NAMES`).
///
/// Host pause-save writes the 17 named chunks then `SG_EOF`. `CHUNK_GameState`
/// is the C++ v2 header (`GameState.cpp:1539-1642`). `CHUNK_GameLogic` is still
/// host `bincode` `WorldSnapshot` on write. A C++ `GameLogic::xfer` payload
/// cannot be restored into the live host world; load fails closed instead of
/// reporting success with an empty snapshot. `CHUNK_InGameUI`,
/// `CHUNK_TacticalView`, `CHUNK_ScriptEngine`, `CHUNK_TerrainLogic`, and
/// `CHUNK_Radar` write live persist_v18 state in C++ xfer layout.
/// `CHUNK_GameClient` writes leftover `GameClient::xfer` so objectless
/// PUC / lock-on / rope drawables survive save/load.
/// `CHUNK_ParticleSystem` writes leftover `ParticleSystemManager::xfer` so
/// mid-flight explosions continue after load. `CHUNK_TerrainVisual` writes
/// C++ `W3DTerrainVisual::xfer` v3 plus the live scorch overlay.
/// `CHUNK_Players` / `CHUNK_TeamFactory` write leftover Player::xfer /
/// Team::xfer latches so science hide/disable and OnCreate do not reset.
/// Remaining registered blocks are NullSnapshot version-1 placeholders.
/// `CHUNK_GameStateMap` embeds the `.map` when the file is on disk
/// (`GameStateMap.cpp:55-156`).
const CHUNK_GAME_STATE: &str = "CHUNK_GameState";
const CHUNK_GAME_LOGIC: &str = "CHUNK_GameLogic";
const CHUNK_GAME_STATE_MAP: &str = "CHUNK_GameStateMap";
const CHUNK_CAMPAIGN: &str = "CHUNK_Campaign";
const CHUNK_INGAME_UI: &str = "CHUNK_InGameUI";
const CHUNK_TACTICAL_VIEW: &str = "CHUNK_TacticalView";
const CHUNK_SCRIPT_ENGINE: &str = "CHUNK_ScriptEngine";
const CHUNK_TERRAIN_LOGIC: &str = "CHUNK_TerrainLogic";
const CHUNK_RADAR: &str = "CHUNK_Radar";
const SAVE_FILE_EOF: &str = "SG_EOF";
const CPP_GAME_STATE_XFER_VERSION: u8 = 2;
const CPP_SAVE_FILE_TYPE_NORMAL: i32 = 0;
const CPP_SAVE_FILE_TYPE_MISSION: i32 = 1;
const CPP_INVALID_MISSION_NUMBER: i32 = -1;

const SAVELOAD_BLOCK_NAMES: &[&str] = game_engine::System::SaveGame::SAVELOAD_BLOCK_NAMES;

/// C++ `GameLogic.h` game-mode integers written by `GameStateMap::xfer` v2.
const CPP_GAME_SINGLE_PLAYER: i32 = 0;
const CPP_GAME_LAN: i32 = 1;
const CPP_GAME_SKIRMISH: i32 = 2;
const CPP_GAME_REPLAY: i32 = 3;
const CPP_GAME_SHELL: i32 = 4;
const CPP_GAME_INTERNET: i32 = 5;
const CPP_GAME_NONE: i32 = 6;

fn cpp_game_mode_from_live(mode: crate::game_logic::GameMode) -> i32 {
    use crate::game_logic::GameMode;
    match mode {
        GameMode::SinglePlayer => CPP_GAME_SINGLE_PLAYER,
        GameMode::Lan | GameMode::Multiplayer => CPP_GAME_LAN,
        GameMode::Skirmish => CPP_GAME_SKIRMISH,
        GameMode::Replay => CPP_GAME_REPLAY,
        GameMode::Shell => CPP_GAME_SHELL,
        GameMode::Internet => CPP_GAME_INTERNET,
        GameMode::None => CPP_GAME_NONE,
    }
}

pub fn live_game_mode_from_cpp(mode: i32) -> Option<crate::game_logic::GameMode> {
    use crate::game_logic::GameMode;
    match mode {
        CPP_GAME_SINGLE_PLAYER => Some(GameMode::SinglePlayer),
        CPP_GAME_LAN => Some(GameMode::Lan),
        CPP_GAME_SKIRMISH => Some(GameMode::Skirmish),
        CPP_GAME_REPLAY => Some(GameMode::Replay),
        CPP_GAME_SHELL => Some(GameMode::Shell),
        CPP_GAME_INTERNET => Some(GameMode::Internet),
        CPP_GAME_NONE => Some(GameMode::None),
        _ => None,
    }
}

/// C++ `GameState::xfer` writes `GetLocalTime` fields (`GameState.cpp:1562-1582`).
/// Leftover `SaveDate::from_local_time` already matches that calendar.
fn local_date_fields(time: SystemTime) -> [u16; 8] {
    game_engine::System::SaveDate::from_local_time(time).to_xfer_fields()
}

fn map_leaf_name(path: &str) -> String {
    path.replace('/', "\\")
        .rsplit(['\\', '/'])
        .next()
        .unwrap_or(path)
        .to_string()
}

fn cpp_save_file_type(save_type: SaveFileType) -> i32 {
    match save_type {
        SaveFileType::Mission => CPP_SAVE_FILE_TYPE_MISSION,
        _ => CPP_SAVE_FILE_TYPE_NORMAL,
    }
}

fn write_ascii<W: Write + Seek>(xfer: &mut CommonXferSave<W>, value: &str) -> SaveLoadResult<()> {
    let mut owned = value.to_string();
    xfer.xfer_ascii_string(&mut owned)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))
}

fn write_unicode<W: Write + Seek>(xfer: &mut CommonXferSave<W>, value: &str) -> SaveLoadResult<()> {
    let mut owned = value.to_string();
    xfer.xfer_unicode_string(&mut owned)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))
}

/// C++ `GameState::xfer` v2 (`GameState.cpp:1539-1642`).
fn write_cpp_game_state_header<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    save_info: &SaveGameInfo,
) -> SaveLoadResult<()> {
    let mut version = CPP_GAME_STATE_XFER_VERSION;
    xfer.xfer_version(&mut version, CPP_GAME_STATE_XFER_VERSION)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut file_type = cpp_save_file_type(save_info.save_type);
    xfer.xfer_int(&mut file_type)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mission_map = if save_info.save_type == SaveFileType::Mission {
        save_info.map_name.as_str()
    } else {
        ""
    };
    write_ascii(xfer, mission_map)?;
    let mut date = local_date_fields(save_info.save_date);
    for field in &mut date {
        xfer.xfer_unsigned_short(field)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    write_unicode(xfer, &save_info.description)?;
    write_ascii(xfer, &map_leaf_name(&save_info.map_name))?;
    write_ascii(xfer, save_info.campaign_side.as_deref().unwrap_or(""))?;
    let mut mission_number = save_info
        .mission_number
        .map(|n| n as i32)
        .unwrap_or(CPP_INVALID_MISSION_NUMBER);
    xfer.xfer_int(&mut mission_number)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    Ok(())
}

/// C++ `CampaignManager::xfer` v5 (`CampaignManager.cpp`) for CHUNK_Campaign.
fn write_campaign_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    state: &game_engine::System::CampaignManagerXferState,
) -> SaveLoadResult<()> {
    let mut version = 5u8;
    xfer.xfer_version(&mut version, 5)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut state = state.clone();
    write_ascii(xfer, &state.campaign)?;
    write_ascii(xfer, &state.mission)?;
    xfer.xfer_int(&mut state.rank_points)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_int(&mut state.difficulty)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_bool(&mut state.is_challenge)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    if state.is_challenge {
        let mut info = state.challenge_info.clone().unwrap_or_default();
        xfer_challenge_game_info(xfer, &mut info)?;
    }
    xfer.xfer_int(&mut state.generals_template)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    Ok(())
}

fn parse_campaign_block(
    payload: &[u8],
) -> SaveLoadResult<game_engine::System::CampaignManagerXferState> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), SAVE_FILE_VERSION);
    let mut version = 0u8;
    xfer.xfer_version(&mut version, 5)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut state = game_engine::System::CampaignManagerXferState::default();
    state.campaign = read_ascii(&mut xfer)?;
    state.mission = read_ascii(&mut xfer)?;
    if version >= 2 {
        xfer.xfer_int(&mut state.rank_points)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    if version >= 3 {
        xfer.xfer_int(&mut state.difficulty)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    if version >= 4 {
        xfer.xfer_bool(&mut state.is_challenge)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        if state.is_challenge {
            let mut info = game_engine::System::ChallengeGameInfoXfer::default();
            xfer_challenge_game_info(&mut xfer, &mut info)?;
            state.challenge_info = Some(info);
        }
    }
    if version >= 5 {
        xfer.xfer_int(&mut state.generals_template)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    Ok(state)
}

pub(crate) fn campaign_state_requires_owner(
    state: &game_engine::System::CampaignManagerXferState,
) -> bool {
    !state.campaign.trim().is_empty() || !state.mission.trim().is_empty() || state.is_challenge
}

fn campaign_difficulty(state: &game_engine::System::CampaignManagerXferState) -> GameDifficulty {
    match state.difficulty {
        0 => GameDifficulty::Easy,
        2 => GameDifficulty::Hard,
        _ => GameDifficulty::Medium,
    }
}

fn xfer_challenge_game_info<X: CommonXfer>(
    xfer: &mut X,
    info: &mut game_engine::System::ChallengeGameInfoXfer,
) -> SaveLoadResult<()> {
    const VERSION: u8 = 4;
    let mut version = VERSION;
    xfer.xfer_version(&mut version, VERSION)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_int(&mut info.preorder_mask)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_int(&mut info.crc_interval)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_bool(&mut info.in_game)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_bool(&mut info.in_progress)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_bool(&mut info.surrendered)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_int(&mut info.game_id)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut slot_count = game_engine::System::CHALLENGE_MAX_SLOTS as i32;
    xfer.xfer_int(&mut slot_count)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let slots = slot_count.clamp(0, game_engine::System::CHALLENGE_MAX_SLOTS as i32) as usize;
    for slot in info.slots.iter_mut().take(slots) {
        xfer.xfer_int(&mut slot.state)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        if version >= 2 {
            xfer.xfer_unicode_string(&mut slot.name)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }
        xfer.xfer_bool(&mut slot.is_accepted)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_bool(&mut slot.is_muted)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.color)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.start_pos)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.player_template)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.team_number)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.orig_color)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.orig_start_pos)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_int(&mut slot.orig_player_template)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    xfer.xfer_unsigned_int(&mut info.local_ip)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_ascii_string(&mut info.map_name)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_unsigned_int(&mut info.map_crc)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_unsigned_int(&mut info.map_size)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_int(&mut info.map_mask)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_int(&mut info.seed)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    if version >= 3 {
        xfer.xfer_unsigned_short(&mut info.superweapon_restriction)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        if version == 3 {
            let mut obsolete = false;
            xfer.xfer_bool(&mut obsolete)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }
        let mut money_version = 1u8;
        xfer.xfer_version(&mut money_version, 1)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        xfer.xfer_unsigned_int(&mut info.starting_cash)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    Ok(())
}

pub(crate) fn parse_named_chunk_save_info(data: &[u8]) -> SaveLoadResult<SaveGameInfo> {
    SaveFileManager::read_named_chunk_save_info(data)
}

fn read_ascii(xfer: &mut CommonXferLoad<Cursor<&[u8]>>) -> SaveLoadResult<String> {
    let mut value = String::new();
    xfer.xfer_ascii_string(&mut value)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    Ok(value)
}

fn read_unicode(xfer: &mut CommonXferLoad<Cursor<&[u8]>>) -> SaveLoadResult<String> {
    let mut value = String::new();
    xfer.xfer_unicode_string(&mut value)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    Ok(value)
}

/// Parse C++ `GameState::xfer` enough to list a save. Version 2 is accepted.
fn parse_cpp_game_state_header(payload: &[u8]) -> SaveLoadResult<SaveGameInfo> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), SAVE_FILE_VERSION);
    let mut version = 0u8;
    // Accept C++ currentVersion=2; do not treat it as > host SAVE_FILE_VERSION.
    xfer.xfer_version(&mut version, CPP_GAME_STATE_XFER_VERSION)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut file_type = CPP_SAVE_FILE_TYPE_NORMAL;
    let mut mission_map_name = String::new();
    if version >= 2 {
        xfer.xfer_int(&mut file_type)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        mission_map_name = read_ascii(&mut xfer)?;
    }
    let mut date = [0u16; 8];
    for field in &mut date {
        xfer.xfer_unsigned_short(field)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    let description = read_unicode(&mut xfer)?;
    let map_label = read_ascii(&mut xfer)?;
    let campaign_side = read_ascii(&mut xfer)?;
    let mut mission_number = CPP_INVALID_MISSION_NUMBER;
    xfer.xfer_int(&mut mission_number)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

    let save_type = if file_type == CPP_SAVE_FILE_TYPE_MISSION {
        SaveFileType::Mission
    } else {
        SaveFileType::Normal
    };
    let map_name = if save_type == SaveFileType::Mission && !mission_map_name.is_empty() {
        mission_map_name
    } else {
        map_label
    };
    let (year, month, day, hour, minute, second, milliseconds) = (
        date[0] as i32,
        date[1].clamp(1, 12) as u32,
        date[2].clamp(1, 31) as u32,
        date[4].min(23) as u32,
        date[5].min(59) as u32,
        date[6].min(59) as u32,
        date[7] as u32,
    );
    let save_date = civil_utc_to_system_time(year, month, day, hour, minute, second, milliseconds);
    Ok(SaveGameInfo {
        pristine_map_name: None,
        filename: String::new(),
        display_name: description.clone(),
        description,
        map_name,
        campaign_side: if campaign_side.is_empty() {
            None
        } else {
            Some(campaign_side)
        },
        mission_number: if mission_number >= 0 {
            Some(mission_number as u32)
        } else {
            None
        },
        save_date,
        game_version: String::new(),
        play_time: std::time::Duration::from_secs(0),
        difficulty: GameDifficulty::Medium,
        save_type,
    })
}

fn civil_utc_to_system_time(
    year: i32,
    month: u32,
    day: u32,
    hour: u32,
    minute: u32,
    second: u32,
    milliseconds: u32,
) -> SystemTime {
    // Inverse of local civil fields treated as naive civil (C++ stores GetLocalTime).
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = (year - era * 400) as u32;
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = (era as i64) * 146_097 + doe as i64 - 719_468;
    let secs = days
        .saturating_mul(86_400)
        .saturating_add((hour * 3600 + minute * 60 + second) as i64);
    if secs >= 0 {
        UNIX_EPOCH + std::time::Duration::new(secs as u64, milliseconds.saturating_mul(1_000_000))
    } else {
        UNIX_EPOCH
    }
}

fn write_named_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    name: &str,
    payload: impl FnOnce(&mut CommonXferSave<W>) -> SaveLoadResult<()>,
) -> SaveLoadResult<()> {
    write_ascii(xfer, name)?;
    xfer.begin_block()
        .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;
    payload(xfer)?;
    xfer.end_block()
        .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;
    Ok(())
}

fn write_null_snapshot_version<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
) -> SaveLoadResult<()> {
    let mut version = 1u8;
    xfer.xfer_version(&mut version, 1)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))
}

fn apply_persist_chunks(
    snapshot: &mut WorldSnapshot,
    ingame_ui: Option<&[u8]>,
    tactical_view: Option<&[u8]>,
    script_engine: Option<&[u8]>,
    terrain_logic: Option<&[u8]>,
    radar: Option<&[u8]>,
) {
    use crate::save_load::snapshot::persist_v18;
    if let Some(payload) = ingame_ui {
        if payload.len() > 1 {
            if let Ok(chunk) = persist_v18::parse_ingame_ui_block(payload) {
                persist_v18::merge_chunk_persist(&mut snapshot.persist_v18, chunk);
            }
        }
    }
    if let Some(payload) = tactical_view {
        if payload.len() > 1 {
            if let Ok(camera) = persist_v18::parse_tactical_view_block(payload) {
                // Original View xfer is not the native full eye/zoom record.
                // Keep both decoded values detached; the receiving host selects
                // its complete native record when present.
                snapshot.persist_v18.tactical_view_camera = Some(camera);
            }
        }
    }
    if let Some(payload) = script_engine {
        if payload.len() > 1 {
            if let Ok((sequential, counters, flags, actives, named_reveals, tail)) =
                persist_v18::parse_script_engine_block(payload)
            {
                if !sequential.is_empty() {
                    snapshot.persist_v18.script_sequential = sequential;
                }
                if !counters.is_empty() {
                    snapshot.persist_v18.script_counters = counters;
                }
                if !flags.is_empty() {
                    snapshot.persist_v18.script_flags = flags;
                }
                if !actives.is_empty() {
                    snapshot.persist_v18.script_actives = actives;
                }
                if !named_reveals.is_empty() {
                    snapshot.persist_v18.script_named_reveals = named_reveals;
                }
                snapshot.persist_v18.script_engine_tail = tail;
            }
        }
    }
    if let Some(payload) = terrain_logic {
        if payload.len() > 1 {
            if let Ok((boundary, water)) = persist_v18::parse_terrain_logic_block(payload) {
                snapshot.persist_v18.terrain_active_boundary = boundary;
                if !water.is_empty() {
                    snapshot.persist_v18.water_updates = water;
                }
            }
        }
    }
    if let Some(payload) = radar {
        if payload.len() > 1 {
            if let Ok((hidden, forced, events, next, last)) =
                persist_v18::parse_radar_block(payload)
            {
                snapshot.persist_v18.radar_hidden = hidden;
                snapshot.persist_v18.radar_forced = forced;
                if !events.is_empty() {
                    snapshot.persist_v18.radar_events = events;
                }
                snapshot.persist_v18.radar_next_event = next;
                snapshot.persist_v18.radar_last_event = last;
            }
        }
    }
}

/// C++ `GameStateMap::xfer` v2 map embed (`GameStateMap.cpp:224-394`).
fn write_game_state_map_block<W: Write + Seek>(
    xfer: &mut CommonXferSave<W>,
    save_info: &SaveGameInfo,
    save_directory: &Path,
    source: Option<&game_engine::common::system::skirmish_game_info::SkirmishGameInfo>,
    mut game_mode: i32,
) -> SaveLoadResult<()> {
    let mut version = 2u8;
    xfer.xfer_version(&mut version, 2)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let leaf = map_leaf_name(&save_info.map_name);
    write_ascii(xfer, &format!("Save\\{leaf}"))?;
    let pristine_source = save_info
        .pristine_map_name
        .as_deref()
        .unwrap_or(&save_info.map_name);
    let user_data = game_engine::common::global_data::read()
        .get_user_data_dir()
        .to_string();
    let pristine = super::map_paths::to_portable(pristine_source, save_directory, &user_data);
    write_ascii(xfer, &pristine)?;
    xfer.xfer_int(&mut game_mode)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

    let mut map_bytes = Vec::new();
    if !save_info.map_name.is_empty() {
        if let Some(path) = crate::game_logic::script_loader::find_map_file(&save_info.map_name) {
            map_bytes = std::fs::read(path).unwrap_or_default();
        } else if Path::new(&save_info.map_name).is_file() {
            map_bytes = std::fs::read(&save_info.map_name).unwrap_or_default();
        }
    }
    xfer.begin_block()
        .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;
    if !map_bytes.is_empty() {
        // SAFETY: buffer lives for the xfer_user call.
        unsafe {
            xfer.xfer_user(map_bytes.as_mut_ptr(), map_bytes.len())
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }
    }
    xfer.end_block()
        .map_err(|e| SaveLoadError::Serialization(format!("{e:?}")))?;

    let mut object_id = 1u32;
    let mut drawable_id = 1u32;
    xfer.xfer_unsigned_int(&mut object_id)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    xfer.xfer_unsigned_int(&mut drawable_id)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    if let Some(source) = source {
        // Xfer::xferMapName wraps this one field in the same explicit map-root
        // projection as the enclosing GameStateMap (Xfer.cpp151–163).
        let mut source = source.clone();
        source.map_name =
            super::map_paths::to_portable(&source.map_name, save_directory, &user_data);
        source
            .write_to_common(xfer)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    }
    Ok(())
}

fn extract_embedded_map(
    payload: &[u8],
    save_dir: &Path,
    user_data: &str,
) -> Option<(PathBuf, String)> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), SAVE_FILE_VERSION);
    let mut version = 0u8;
    xfer.xfer_version(&mut version, 2).ok()?;
    let save_game_map = read_ascii(&mut xfer).ok()?;
    let portable_pristine = read_ascii(&mut xfer).ok()?;
    let pristine = super::map_paths::from_portable(&portable_pristine, save_dir, user_data);
    if version >= 2 {
        let mut game_mode = 0i32;
        xfer.xfer_int(&mut game_mode).ok()?;
    }
    let data_size = xfer.begin_block().ok()?;
    if data_size <= 0 {
        return None;
    }
    let mut buffer = vec![0u8; data_size as usize];
    // SAFETY: buffer is an owned Vec sized to the map block's data_size;
    // xfer_user fills exactly buffer.len() bytes.
    unsafe {
        xfer.xfer_user(buffer.as_mut_ptr(), buffer.len()).ok()?;
    }
    let _ = xfer.end_block();
    let leaf = map_leaf_name(&save_game_map);
    if leaf.is_empty() {
        return None;
    }
    let _ = std::fs::create_dir_all(save_dir);
    let dest = save_dir.join(leaf);
    std::fs::write(&dest, buffer).ok()?;
    Some((dest, pristine))
}

fn walk_named_chunks(data: &[u8]) -> SaveLoadResult<Vec<(String, Vec<u8>)>> {
    let mut pos = 0usize;
    let mut blocks = Vec::new();
    while pos < data.len() {
        let token_len = data[pos] as usize;
        pos += 1;
        if pos + token_len > data.len() {
            return Err(SaveLoadError::Corrupted(
                "truncated named-chunk token".to_string(),
            ));
        }
        let token = std::str::from_utf8(&data[pos..pos + token_len])
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?
            .to_string();
        pos += token_len;
        if token.eq_ignore_ascii_case(SAVE_FILE_EOF) {
            break;
        }
        if pos + 4 > data.len() {
            return Err(SaveLoadError::Corrupted(
                "truncated named-chunk size".to_string(),
            ));
        }
        let block_size =
            i32::from_le_bytes([data[pos], data[pos + 1], data[pos + 2], data[pos + 3]]);
        pos += 4;
        if block_size < 0 {
            return Err(SaveLoadError::Corrupted(
                "negative named-chunk size".to_string(),
            ));
        }
        let end = pos
            .checked_add(block_size as usize)
            .ok_or_else(|| SaveLoadError::Corrupted("named-chunk size overflow".to_string()))?;
        if end > data.len() {
            return Err(SaveLoadError::Corrupted(
                "named-chunk payload overruns file".to_string(),
            ));
        }
        blocks.push((token, data[pos..end].to_vec()));
        pos = end;
    }
    Ok(blocks)
}

fn parse_chunk_game_state(payload: &[u8]) -> SaveLoadResult<SaveGameInfo> {
    if payload.first().copied().unwrap_or(0) >= 2 {
        return parse_cpp_game_state_header(payload);
    }
    let mut header = CommonGameState::default();
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), SAVE_FILE_VERSION);
    match header.xfer(&mut xfer) {
        Ok(()) => Ok(SaveFileManager::save_info_from_common_state(
            &header,
            &WorldSnapshot::default(),
        )),
        Err(err) => Err(SaveLoadError::Serialization(err.to_string())),
    }
}

/// Save file section types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SaveFileSection {
    Header,
    GameInfo,
    WorldState,
    PlayerStates,
    AIStates,
    MapState,
    Scripts,
    EndMarker,
}

/// Save file manager
pub struct SaveFileManager {
    save_directory: PathBuf,
    temp_directory: PathBuf,
    auto_save_interval: std::time::Duration,
    max_save_files: usize,
    last_auto_save: SystemTime,
    rejected_logical_worlds: Vec<Box<GameLogic>>,
}

impl SaveFileManager {
    pub(crate) fn retain_rejected_logical_world(&mut self, world: Box<GameLogic>) {
        self.rejected_logical_worlds.push(world);
    }
    pub(crate) fn transfer_rejected_logical_worlds_to(&mut self, receiving: &mut Self) {
        receiving
            .rejected_logical_worlds
            .append(&mut self.rejected_logical_worlds);
    }
    pub(crate) fn validate_logical_restore_admission(&self) -> Result<(), String> {
        if self.rejected_logical_worlds.is_empty() {
            Ok(())
        } else {
            Err("terminal logical restore candidate retirement".into())
        }
    }

    pub fn new() -> Self {
        let save_dir = SaveLoadManager::default_save_directory();
        Self::with_save_directory(save_dir)
    }

    pub fn with_save_directory(save_directory: impl Into<PathBuf>) -> Self {
        let save_dir = save_directory.into();
        let mut temp_dir = save_dir.clone();
        temp_dir.push("temp");

        Self {
            save_directory: save_dir,
            temp_directory: temp_dir,
            auto_save_interval: std::time::Duration::from_secs(300), // 5 minutes
            max_save_files: MAX_SAVE_SLOTS,
            last_auto_save: SystemTime::now(),
            rejected_logical_worlds: Vec::new(),
        }
    }

    /// Directory used for list/save/load. Default is `UserData/Save` (Popup + host).
    pub fn save_directory(&self) -> &Path {
        &self.save_directory
    }

    pub fn init(&mut self) -> SaveLoadResult<()> {
        // Create directories if they don't exist
        std::fs::create_dir_all(&self.save_directory)?;
        std::fs::create_dir_all(&self.temp_directory)?;

        // Clean up old temporary files
        self.cleanup_temp_files()?;

        Ok(())
    }

    /// Save game to file
    pub fn save_game(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        // Non-host callers deliberately remain logic-only.  The authoritative
        // CnCGameEngine path captures its renderer-owned companion explicitly
        // through the companion-aware API below.
        self.save_game_with_client_drawable_snapshot(
            filename,
            game_logic,
            ClientDrawableWorldSnapshot::default(),
            save_info,
        )
    }

    /// Save a WorldSnapshot with an explicitly captured renderer companion.
    ///
    /// SnapshotBuilder owns only GameLogic, so it must not reach into live
    /// renderer/global state.  The host captures the DTO at its authority
    /// boundary, passes it here, and this method attaches it before the normal
    /// Common `.sav` writer serializes the exact v4 positional record.
    pub fn save_game_with_client_drawable_snapshot(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            None,
            save_info,
            crate::save_load::snapshot::ClientSaveState::default(),
            None,
            None,
        )
    }

    /// Save the committed host world's visual companion from its owning client.
    pub fn save_game_with_client_state(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            crate::save_load::snapshot::ClientSaveState::default(),
            None,
            None,
        )
    }

    /// Save a view captured by the same engine that owns this world/client.
    pub fn save_game_with_client_state_and_camera(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        camera: crate::save_load::snapshot::CameraPersist,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            crate::save_load::snapshot::ClientSaveState {
                camera: Some(camera),
                camera_continuation: None,
                control_groups: None,
            },
            None,
            None,
        )
    }

    /// Capture camera and control groups from the same driving engine.
    pub fn save_game_with_client_save_state(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            state,
            None,
            None,
        )
    }

    /// Native caller supplies its actual scorch observation explicitly.
    pub fn save_game_with_client_save_state_and_terrain_scorches(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
        scorches: Option<&[game_client::terrain::scorch_mesh::ScorchMark]>,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            state,
            scorches,
            None,
        )
    }

    /// Native source owner: water-render state and scorch marks are observed
    /// together from the receiving pipeline, never from ambient selection.
    pub fn save_game_with_client_save_state_and_native_terrain(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        client: &mut game_client::core::game_client::GameClient,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
        visual: Option<&game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) -> SaveLoadResult<()> {
        self.save_game_with_client_state_impl(
            filename,
            game_logic,
            client_drawables,
            Some(client),
            save_info,
            state,
            visual.map(|v| v.scorch_marks()),
            visual,
        )
    }

    fn save_game_with_client_state_impl(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        mut client: Option<&mut game_client::core::game_client::GameClient>,
        save_info: &SaveGameInfo,
        state: crate::save_load::snapshot::ClientSaveState,
        terrain_scorches: Option<&[game_client::terrain::scorch_mesh::ScorchMark]>,
        native_terrain: Option<&game_client::terrain::terrain_visual::TerrainVisualImpl>,
    ) -> SaveLoadResult<()> {
        if let Some(camera) = state.camera {
            camera.validate()?;
        }
        if let Some(groups) = &state.control_groups {
            groups.validate()?;
            if game_logic.get_player(groups.player_index as u32).is_none() {
                return Err(SaveLoadError::Corrupted(
                    "control-group owner absent from receiving roster".into(),
                ));
            }
        }
        let native_camera = state
            .camera_continuation
            .as_deref()
            .map(crate::cnc_game_engine::CnCGameEngine::decode_camera_continuation)
            .transpose()
            .map_err(SaveLoadError::Corrupted)?;
        // Full native continuation is canonical when supplied. Derive the
        // historical View/v18 pose from that same owner, never a second input.
        let camera = native_camera
            .as_ref()
            .map(|camera| camera.camera_persist())
            .or(state.camera);
        let save_path = self.get_save_path(filename);
        let temp_path = self.get_temp_path(&format!("{}_temp", filename));

        // Create snapshot of current game state
        let snapshot_builder = SnapshotBuilder::new();
        let mut world_snapshot = if let Some(client) = client.as_deref() {
            snapshot_builder.create_world_snapshot_with_client(game_logic, client)?
        } else {
            snapshot_builder.create_world_snapshot(game_logic)?
        };
        world_snapshot.client_drawables = client_drawables;
        if let Some(camera) = camera {
            camera.write_to(&mut world_snapshot.persist_v18);
        }
        if let Some(groups) = &state.control_groups {
            crate::save_load::snapshot::write_control_groups(
                &mut world_snapshot.lifecycle_tail,
                groups,
            )?;
        }
        if let Some(bytes) = state.camera_continuation.as_deref() {
            crate::save_load::snapshot::write_native_camera_continuation(
                &mut world_snapshot.lifecycle_tail,
                bytes,
            )?;
        }
        // C++ GameState writes the logic chunk before GameClient::xfer.
        let campaign_state = if let Some(client) = client.as_deref() {
            client.campaign_manager().capture_logic_chunk_state()
        } else if game_logic.has_campaign_session() {
            return Err(SaveLoadError::Serialization(
                "active campaign save has no receiving campaign owner".into(),
            ));
        } else {
            game_engine::System::CampaignManagerXferState::default()
        };
        if client.is_some()
            && campaign_state_requires_owner(&campaign_state) != game_logic.has_campaign_session()
        {
            return Err(SaveLoadError::Serialization(
                "receiving campaign owner and admitted session kind disagree".into(),
            ));
        }
        let client_xfer_bytes = if let Some(client) = client.as_deref_mut() {
            #[cfg(feature = "game_client")]
            {
                crate::save_load::snapshot::capture_game_client_xfer_bytes_excluding_owned(
                    client,
                    &game_logic.owned_jet_lockon_receipts(),
                )?
            }
            #[cfg(not(feature = "game_client"))]
            {
                capture_game_client_xfer_bytes(client)?
            }
        } else {
            Vec::new()
        };
        // C++ named phases: GameLogic, GameClient, then ParticleSystem. Capture
        // once on this new save candidate; the writer never consults a later
        // live manager for a decoded or absent companion.
        world_snapshot.particle_system_companion = if save_info.save_type == SaveFileType::Mission {
            // C++ mission saves contain only GameState and Campaign blocks.
            None
        } else {
            client
                .as_deref_mut()
                .map(|client| {
                    let mut bytes =
                        capture_particle_system_xfer_bytes(client.particle_systems_mut())?;
                    game_logic.append_fx_random_capsule(&mut bytes);
                    Ok::<_, SaveLoadError>(bytes)
                })
                .transpose()?
        };

        // C++ GameState::saveGame (GameState.cpp:534) makes sure the save
        // directory exists at save time; our atomic flow writes into
        // save_directory/temp first, so that directory must exist too.
        std::fs::create_dir_all(&self.temp_directory)?;

        // Save to temporary file first
        let mut effective_info = save_info.clone();
        if effective_info.pristine_map_name.is_none() {
            effective_info.pristine_map_name = game_logic.map_definition_source.clone();
        }
        let native_water = native_terrain.map(|visual| visual.capture_water_state());
        let terrain_visual_bytes =
            crate::save_load::snapshot::capture_receiving_terrain_visual_with_water(
                game_logic,
                native_water.as_ref(),
                terrain_scorches,
            )?;
        let write_result = self.save_to_file(
            &temp_path,
            &world_snapshot,
            &effective_info,
            &client_xfer_bytes,
            Some(&terrain_visual_bytes),
            cpp_game_mode_from_live(game_logic.game_mode()),
            &campaign_state,
        );
        write_result?;

        // Atomically move temp file to final location
        std::fs::rename(&temp_path, &save_path).map_err(|e| {
            let _ = std::fs::remove_file(&temp_path);
            SaveLoadError::Io(e)
        })?;

        // C++ GameState::saveGame never deletes existing saves.
        log::info!("Game saved successfully to: {}", save_path.display());
        Ok(())
    }

    /// Load game from file
    /// Logical-only API. Native applications use the companion-aware route.
    pub fn load_game(
        &mut self,
        filename: &str,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<SaveGameInfo> {
        let (info, _native_effects) = self.load_game_with_terrain_visual(filename, game_logic)?;
        // Explicit absence of a native receiver: logical state only, matching
        // this API's existing omission of the GameClient companion.
        Ok(info)
    }

    pub fn load_game_with_terrain_visual(
        &mut self,
        filename: &str,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<(SaveGameInfo, Option<PreparedTerrainVisual>)> {
        self.validate_logical_restore_admission()
            .map_err(SaveLoadError::Serialization)?;
        let mut decoded = self.load_game_snapshot(filename)?;
        if decoded
            .campaign_state
            .as_ref()
            .is_some_and(campaign_state_requires_owner)
        {
            return Err(SaveLoadError::Serialization(
                "campaign save requires a receiving campaign owner".into(),
            ));
        }
        // Mission metadata launches a later mission-start operation, as before.
        if decoded.save_info.save_type == SaveFileType::Mission {
            // Logical-only API has no active campaign receiver. Its archive
            // payload remains detached and cannot mutate another application.
            decoded.campaign_state.take();
            return Ok((decoded.save_info, None));
        }
        match crate::cnc_game_engine::CnCGameEngine::restore_decoded_logical_save(
            self, filename, decoded, game_logic,
        )
        .map_err(SaveLoadError::Serialization)
        {
            Ok(restored) => Ok(restored),
            Err(error) => Err(error),
        }
    }

    /// Decode a save without mutating a live `GameLogic` instance.
    ///
    /// The runtime host uses this to load the saved map into a staging world
    /// before it restores the snapshot.  Keeping the decode separate from the
    /// restore means a bad/missing map or corrupted snapshot cannot partially
    /// overwrite the currently playable match.
    pub fn load_game_snapshot(&self, filename: &str) -> SaveLoadResult<DecodedGameSave> {
        let save_path = self.get_save_path(filename);
        if !save_path.exists() {
            return Err(SaveLoadError::FileNotFound(filename.to_string()));
        }

        self.load_from_file(&save_path)
    }

    /// Restore a previously decoded snapshot into the supplied world.
    ///
    /// Callers that need transactional map identity handling should restore
    /// into a fresh staging world and install it only after this succeeds.
    pub fn restore_game_snapshot(
        &self,
        world_snapshot: &WorldSnapshot,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<()> {
        let snapshot_builder = SnapshotBuilder::new();
        snapshot_builder.restore_from_snapshot(world_snapshot, game_logic)
    }

    /// Quick save to slot 0
    pub fn quick_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<()> {
        let save_info = SaveGameInfo {
            pristine_map_name: None,
            filename: "quicksave".to_string(),
            display_name: "Quick Save".to_string(),
            description: "Quick save".to_string(),
            map_name: "Unknown".to_string(), // Would get from game state
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs(0), // Would track actual play time
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::QuickSave,
        };

        self.save_game("quicksave", game_logic, &save_info)
    }

    /// Auto save if enough time has passed
    pub fn try_auto_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<bool> {
        let now = SystemTime::now();
        if now.duration_since(self.last_auto_save).unwrap_or_default() >= self.auto_save_interval {
            self.auto_save(game_logic)?;
            self.last_auto_save = now;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Auto save
    pub fn auto_save(&mut self, game_logic: &GameLogic) -> SaveLoadResult<()> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let save_info = SaveGameInfo {
            pristine_map_name: None,
            filename: format!("autosave_{}", timestamp),
            display_name: "Auto Save".to_string(),
            description: format!("Automatic save at {}", timestamp),
            map_name: "Unknown".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: SystemTime::now(),
            game_version: env!("CARGO_PKG_VERSION").to_string(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::AutoSave,
        };

        let filename = &save_info.filename;
        self.save_game(filename, game_logic, &save_info)?;

        Ok(())
    }

    /// Delete save file
    pub fn delete_save(&self, filename: &str) -> SaveLoadResult<()> {
        let save_path = self.get_save_path(filename);

        if save_path.exists() {
            std::fs::remove_file(&save_path)?;
            log::info!("Deleted save file: {}", save_path.display());
        }

        Ok(())
    }

    /// Check if save file exists
    pub fn save_exists(&self, filename: &str) -> bool {
        self.get_save_path(filename).exists()
    }

    /// Get save file info without loading the entire file
    pub fn get_save_info(&self, filename: &str) -> SaveLoadResult<SaveGameInfo> {
        let save_path = self.get_save_path(filename);
        self.get_save_info_from_path(&save_path)
    }

    fn get_save_info_from_path(&self, save_path: &Path) -> SaveLoadResult<SaveGameInfo> {
        let mut file = File::open(save_path)?;
        let mut all = Vec::new();
        file.read_to_end(&mut all)?;
        if Self::looks_like_common_sav_chunks(&all) {
            return Self::read_named_chunk_save_info(&all);
        }
        Err(SaveLoadError::InvalidFormat)
    }

    /// List all available save files
    pub fn list_saves(&self) -> SaveLoadResult<Vec<AvailableGameInfo>> {
        let mut saves = Vec::new();

        let entries = std::fs::read_dir(&self.save_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == SAVE_EXTENSION {
                    if let Some(filename) = path.file_stem().and_then(|s| s.to_str()) {
                        match self.get_save_info(filename) {
                            Ok(save_info) => {
                                saves.push(AvailableGameInfo {
                                    filename: filename.to_string(),
                                    save_info,
                                });
                            }
                            Err(e) => {
                                log::warn!(
                                    "Failed to read save info from {}: {}",
                                    path.display(),
                                    e
                                );
                            }
                        }
                    }
                }
            }
        }

        // Sort by save date, newest first
        saves.sort_by(|a, b| b.save_info.save_date.cmp(&a.save_info.save_date));

        Ok(saves)
    }

    /// Save data to file as Common `.sav` chunks (same tokens as Popup).
    fn save_to_file(
        &self,
        path: &Path,
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        client_xfer_bytes: &[u8],
        terrain_visual_bytes: Option<&[u8]>,
        supplied_game_mode: i32,
        campaign_state: &game_engine::System::CampaignManagerXferState,
    ) -> SaveLoadResult<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        let encoded = Self::write_common_sav_chunks_with_client_and_terrain_bytes(
            world_snapshot,
            save_info,
            client_xfer_bytes,
            terrain_visual_bytes,
            &self.save_directory,
            supplied_game_mode,
            campaign_state,
        )?;
        writer.write_all(&encoded)?;
        writer.flush()?;
        Ok(())
    }

    /// Load the current named-chunk `.sav` container.
    fn load_from_file(&self, path: &Path) -> SaveLoadResult<DecodedGameSave> {
        // C++ GameState::loadGame (GameState.cpp:648) clears scratch-pad maps
        // before opening the save. Leftover GameStateMap already matches
        // clearScratchPadMaps (delete every `.map` in the Save directory).
        // Drop it before extract so leftover Drop cannot delete the new file.
        {
            let scratch = CommonGameStateMap::new(self.save_directory.clone());
            if let Err(err) = scratch.clear_scratch_pad_maps() {
                log::warn!("Error clearing scratch-pad maps before load: {err}");
            }
        }

        let mut file = File::open(path)?;
        let mut all = Vec::new();
        file.read_to_end(&mut all)?;

        if Self::looks_like_common_sav_chunks(&all) {
            return Self::read_common_sav_chunks(&all, &self.save_directory);
        }

        Err(SaveLoadError::InvalidFormat)
    }

    fn looks_like_common_sav_chunks(data: &[u8]) -> bool {
        data.windows(CHUNK_GAME_STATE.len())
            .any(|w| w == CHUNK_GAME_STATE.as_bytes())
            || data
                .windows(SAVE_FILE_EOF.len())
                .any(|w| w == SAVE_FILE_EOF.as_bytes())
    }

    /// C++ `GameState::xferSaveData` 17 named chunks + `SG_EOF` (`GameState.cpp:1313-1458`).
    ///
    /// `CHUNK_GameState` is the C++ v2 header. Host `CHUNK_GameLogic` payload is
    /// still `bincode` `WorldSnapshot` (not crate/`C++` `GameLogic::xfer`).
    fn write_common_sav_chunks(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        supplied_game_mode: i32,
    ) -> SaveLoadResult<Vec<u8>> {
        Self::write_common_sav_chunks_with_client_bytes(
            world_snapshot,
            save_info,
            &[],
            Path::new("Save"),
            supplied_game_mode,
        )
    }

    fn write_common_sav_chunks_with_client_bytes(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        client_xfer_bytes: &[u8],
        save_directory: &Path,
        supplied_game_mode: i32,
    ) -> SaveLoadResult<Vec<u8>> {
        Self::write_common_sav_chunks_with_client_and_terrain_bytes(
            world_snapshot,
            save_info,
            client_xfer_bytes,
            None,
            save_directory,
            supplied_game_mode,
            &game_engine::System::CampaignManagerXferState::default(),
        )
    }

    fn write_common_sav_chunks_with_client_and_terrain_bytes(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        client_xfer_bytes: &[u8],
        terrain_visual_bytes: Option<&[u8]>,
        save_directory: &Path,
        supplied_game_mode: i32,
        campaign_state: &game_engine::System::CampaignManagerXferState,
    ) -> SaveLoadResult<Vec<u8>> {
        validate_direct_world_snapshot_version(world_snapshot.version)?;
        let logic_payload = bincode_legacy::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Self::write_common_sav_chunks_with_payload_and_client(
            world_snapshot,
            save_info,
            logic_payload,
            client_xfer_bytes,
            terrain_visual_bytes,
            save_directory,
            supplied_game_mode,
            campaign_state,
        )
    }

    /// Shared 17-block container writer. Logic payload is kept separate so
    /// the outer C++ chunk table is independent of the positional
    /// WorldSnapshot schema and historical fixtures still migrate.
    fn write_common_sav_chunks_with_payload(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
        save_directory: &Path,
        supplied_game_mode: i32,
    ) -> SaveLoadResult<Vec<u8>> {
        Self::write_common_sav_chunks_with_payload_and_client(
            world_snapshot,
            save_info,
            logic_payload,
            &[],
            None,
            save_directory,
            supplied_game_mode,
            &game_engine::System::CampaignManagerXferState::default(),
        )
    }

    fn write_common_sav_chunks_with_payload_and_client(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
        game_client_bytes: &[u8],
        supplied_terrain_visual_bytes: Option<&[u8]>,
        save_directory: &Path,
        supplied_game_mode: i32,
        campaign_state: &game_engine::System::CampaignManagerXferState,
    ) -> SaveLoadResult<Vec<u8>> {
        let initial = crate::save_load::snapshot::initial_skirmish::read(world_snapshot)?;
        let source_game_info =
            crate::save_load::snapshot::initial_skirmish::source(world_snapshot)?;
        let mode = if initial.is_some() || source_game_info.is_some() {
            CPP_GAME_SKIRMISH
        } else {
            supplied_game_mode
        };
        if save_info.save_type != SaveFileType::Mission {
            map_admission::validate_native(
                world_snapshot,
                mode,
                source_game_info.as_ref(),
                str::to_owned,
            )?;
        }
        let player_team_chunks = player_team_chunks_from_world(world_snapshot)?;
        if save_info.save_type != SaveFileType::Mission {
            validate_host_alliances(world_snapshot, &player_team_chunks)?;
        }
        let ghost_bytes = match world_snapshot.w3d_ghost_companion.as_deref() {
            Some(bytes) => bytes.to_vec(),
            None => capture_w3d_ghost_xfer_bytes(&Default::default())?,
        };
        let particle_system_bytes = world_snapshot
            .particle_system_companion
            .as_deref()
            .unwrap_or_default();
        let terrain_visual_bytes = supplied_terrain_visual_bytes.unwrap_or_default().to_vec();
        let block_names: &[&str] = if save_info.save_type == SaveFileType::Mission {
            // C++ `xferSaveData` (`GameState.cpp:1339-1346`) writes only
            // CHUNK_GameState + CHUNK_Campaign for SAVE_FILE_TYPE_MISSION.
            &[CHUNK_GAME_STATE, CHUNK_CAMPAIGN]
        } else {
            SAVELOAD_BLOCK_NAMES
        };
        let mut cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            for &name in block_names {
                write_named_block(&mut xfer, name, |xfer| match name {
                    CHUNK_GAME_STATE => write_cpp_game_state_header(xfer, save_info),
                    CHUNK_GAME_LOGIC => {
                        if !logic_payload.is_empty() {
                            let mut bytes = logic_payload.clone();
                            // SAFETY: buffer lives for this block write.
                            unsafe {
                                xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())
                                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
                            }
                        }
                        Ok(())
                    }
                    CHUNK_GAME_STATE_MAP => write_game_state_map_block(
                        xfer,
                        save_info,
                        save_directory,
                        source_game_info.as_ref(),
                        mode,
                    ),
                    CHUNK_GHOST_OBJECT => {
                        write_null_snapshot_version(xfer)?;
                        if !ghost_bytes.is_empty() {
                            let mut bytes = ghost_bytes.clone();
                            // SAFETY: bytes is an owned clone; xfer_user
                            // writes exactly its length during save.
                            unsafe {
                                xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())
                                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
                            }
                        }
                        Ok(())
                    }
                    CHUNK_GAME_CLIENT => {
                        if game_client_bytes.is_empty() {
                            write_null_snapshot_version(xfer)
                        } else {
                            let mut bytes = game_client_bytes.to_vec();
                            // SAFETY: owned byte vector of exact
                            // length handed to the save writer.
                            unsafe {
                                xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())
                                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))
                            }
                        }
                    }
                    CHUNK_PARTICLE_SYSTEM => {
                        if particle_system_bytes.is_empty() {
                            write_null_snapshot_version(xfer)
                        } else {
                            let mut bytes = particle_system_bytes.to_vec();
                            // SAFETY: owned byte vector of exact length
                            // handed to the save writer.
                            unsafe {
                                xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())
                                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))
                            }
                        }
                    }
                    CHUNK_TERRAIN_VISUAL => {
                        if terrain_visual_bytes.is_empty() {
                            write_null_snapshot_version(xfer)
                        } else {
                            let mut bytes = terrain_visual_bytes.clone();
                            // SAFETY: owned byte vector of exact length
                            // handed to the save writer.
                            unsafe {
                                xfer.xfer_user(bytes.as_mut_ptr(), bytes.len())
                                    .map_err(|e| SaveLoadError::Serialization(e.to_string()))
                            }
                        }
                    }
                    CHUNK_CAMPAIGN => write_campaign_block(xfer, campaign_state),
                    CHUNK_INGAME_UI => {
                        crate::save_load::snapshot::persist_v18::write_ingame_ui_block(
                            xfer,
                            &world_snapshot.persist_v18,
                        )
                    }
                    CHUNK_TACTICAL_VIEW => {
                        crate::save_load::snapshot::persist_v18::write_tactical_view_block(
                            xfer,
                            &world_snapshot.persist_v18,
                        )
                    }
                    CHUNK_SCRIPT_ENGINE => {
                        crate::save_load::snapshot::persist_v18::write_script_engine_block(
                            xfer,
                            &world_snapshot.persist_v18,
                        )
                    }
                    CHUNK_TERRAIN_LOGIC => {
                        crate::save_load::snapshot::persist_v18::write_terrain_logic_block(
                            xfer,
                            &world_snapshot.persist_v18,
                        )
                    }
                    CHUNK_RADAR => crate::save_load::snapshot::persist_v18::write_radar_block(
                        xfer,
                        &world_snapshot.persist_v18,
                    ),
                    CHUNK_PLAYERS => write_players_block(xfer, &player_team_chunks),
                    CHUNK_TEAM_FACTORY => write_team_factory_block(xfer, &player_team_chunks),

                    _ => write_null_snapshot_version(xfer),
                })?;
            }
            write_ascii(&mut xfer, SAVE_FILE_EOF)?;
        }
        Ok(cursor.into_inner())
    }

    fn read_named_chunk_save_info(data: &[u8]) -> SaveLoadResult<SaveGameInfo> {
        let blocks = walk_named_chunks(data)?;
        let mut info = None;
        let mut campaign = None;
        for (token, payload) in &blocks {
            if token.eq_ignore_ascii_case(CHUNK_GAME_STATE) {
                info = Some(parse_chunk_game_state(payload)?);
            } else if token.eq_ignore_ascii_case(CHUNK_CAMPAIGN) {
                campaign = parse_campaign_block(payload).ok();
            }
        }
        let mut info = info.ok_or_else(|| {
            SaveLoadError::Corrupted("CHUNK_GameState not found in named-chunk save".to_string())
        })?;
        if let Some(state) = campaign {
            info.difficulty = campaign_difficulty(&state);
        }
        Ok(info)
    }

    /// C++ `GameState::loadGame` walks CHUNK_Campaign before MSG_NEW_GAME.
    pub fn read_campaign_state(
        &self,
        filename: &str,
    ) -> SaveLoadResult<game_engine::System::CampaignManagerXferState> {
        let save_path = self.get_save_path(filename);
        let data = std::fs::read(&save_path)?;
        let blocks = walk_named_chunks(&data)?;
        for (token, payload) in blocks {
            if token.eq_ignore_ascii_case(CHUNK_CAMPAIGN) {
                return parse_campaign_block(&payload);
            }
        }
        Err(SaveLoadError::Corrupted(
            "CHUNK_Campaign not found in named-chunk save".to_string(),
        ))
    }

    fn read_common_sav_chunks(data: &[u8], save_dir: &Path) -> SaveLoadResult<DecodedGameSave> {
        let blocks = walk_named_chunks(data)?;
        let mut save_info = SaveGameInfo {
            pristine_map_name: None,
            filename: String::new(),
            display_name: String::new(),
            description: String::new(),
            map_name: String::new(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH,
            game_version: String::new(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        };
        let mut campaign_state = None;
        let mut logic_data: Option<Vec<u8>> = None;
        let mut game_client_xfer = None;
        let mut terrain_visual_xfer = None;
        let mut saw_game_state = false;
        let mut ingame_ui_payload: Option<Vec<u8>> = None;
        let mut tactical_view_payload: Option<Vec<u8>> = None;
        let mut script_engine_payload: Option<Vec<u8>> = None;
        let mut terrain_logic_payload: Option<Vec<u8>> = None;
        let mut radar_payload: Option<Vec<u8>> = None;
        let mut players_payload: Option<Vec<u8>> = None;
        let mut team_factory_payload: Option<Vec<u8>> = None;
        let mut map_payload: Option<Vec<u8>> = None;
        let mut map_admission = None;
        let mut ghost_companion: Option<Vec<u8>> = None;
        let mut particle_companion: Option<Vec<u8>> = None;

        for (token, payload) in blocks {
            if token.eq_ignore_ascii_case(CHUNK_GAME_STATE) {
                save_info = parse_chunk_game_state(&payload)?;
                saw_game_state = true;
            } else if token.eq_ignore_ascii_case(CHUNK_GAME_LOGIC) {
                logic_data = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_CAMPAIGN) {
                if let Ok(state) = parse_campaign_block(&payload) {
                    save_info.difficulty = campaign_difficulty(&state);
                    if !state.campaign.is_empty() {
                        save_info.campaign_side = Some(state.campaign.clone());
                    }
                    campaign_state = Some(state);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_GHOST_OBJECT) {
                let bytes = if payload.first().copied() == Some(1) {
                    if payload.len() == 1 {
                        None // Historical empty Snapshotable placeholder.
                    } else {
                        Some(payload[1..].to_vec())
                    }
                } else {
                    let mut block = CommonGameState::default();
                    let mut xfer = CommonXferLoad::new(Cursor::new(&payload), SAVE_FILE_VERSION);
                    block
                        .xfer(&mut xfer)
                        .map_err(|error| SaveLoadError::Serialization(error.to_string()))?;
                    Some(block.data)
                };
                if let Some(bytes) = bytes {
                    decode_w3d_ghost_xfer_bytes(&bytes)?;
                    ghost_companion = Some(bytes);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_GAME_CLIENT) {
                // NullSnapshot is a lone version-1 byte. Leftover GameClient::xfer
                // starts at version 3 and recreates objectless drawables.
                if payload.first().copied() != Some(1) || payload.len() > 1 {
                    game_client_xfer = Some(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_PARTICLE_SYSTEM) {
                // NullSnapshot is a lone version-1 byte. Manager xfer is
                // version 1 plus uniqueSystemID / systemCount / systems.
                if payload.len() > 1 {
                    particle_companion = Some(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_TERRAIN_VISUAL) {
                // NullSnapshot is a lone version-1 byte. W3DTerrainVisual::xfer
                // starts at version 3 and carries scorches after the tree/prop
                // snapshot.
                if payload.first().copied() != Some(1) || payload.len() > 1 {
                    terrain_visual_xfer = Some(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_GAME_STATE_MAP) {
                map_admission = Some(map_admission::read_source(&payload)?);
                map_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_INGAME_UI) {
                ingame_ui_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_TACTICAL_VIEW) {
                tactical_view_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_SCRIPT_ENGINE) {
                script_engine_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_TERRAIN_LOGIC) {
                terrain_logic_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_RADAR) {
                radar_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_PLAYERS) {
                players_payload = Some(payload);
            } else if token.eq_ignore_ascii_case(CHUNK_TEAM_FACTORY) {
                team_factory_payload = Some(payload);
            }
        }
        if !saw_game_state {
            return Err(SaveLoadError::Corrupted(
                "CHUNK_GameState missing from named-chunk save".to_string(),
            ));
        }
        let has_game_logic = logic_data.is_some();
        let mut world_snapshot = match logic_data {
            Some(payload) => Self::decode_chunk_game_logic_for_host(&payload)?,
            None if save_info.save_type == SaveFileType::Mission => WorldSnapshot::default(),
            None => {
                return Err(SaveLoadError::Corrupted(
                    "CHUNK_GameLogic missing; refusing to report a successful empty world"
                        .to_string(),
                ));
            }
        };
        let player_team_chunks = stash_loaded_player_team_chunks(
            players_payload.as_deref(),
            team_factory_payload.as_deref(),
        )?;
        // Mission saves carry only GameState/Campaign (GameState.cpp1339).
        // Their empty placeholder is never restored by load_game; capability
        // metadata is required whenever an actual GameLogic body was supplied.
        if has_game_logic {
            validate_host_alliances(&world_snapshot, &player_team_chunks)?;
        }
        bind_player_team_chunks(&mut world_snapshot, &player_team_chunks)?;
        // Reuse the existing map-root observation for both the original
        // GameInfo field and embedded-map extraction; no receiving definitions
        // or lobby/runtime state are polled while decoding either candidate.
        let user_data = game_engine::common::global_data::read()
            .get_user_data_dir()
            .to_string();
        if let Some((mode, source)) = &map_admission {
            map_admission::bind_original_if_needed(&mut world_snapshot, source.as_ref())?;
            map_admission::validate_native(&world_snapshot, *mode, source.as_ref(), |map| {
                super::map_paths::to_portable(map, save_dir, &user_data)
            })?;
        }
        // C++ GameStateMap.cpp324–329 changes mode only for version 2.
        // Retain the already-decoded header with this archive even when its
        // map block is empty. Extraction must not publish match selection.
        let saved_game_mode = match (map_payload.as_deref(), &map_admission) {
            (Some([2, ..]), Some((mode, _))) => Some(*mode),
            _ => None,
        };
        if let Some(payload) = map_payload {
            if let Some((extracted, pristine)) =
                extract_embedded_map(&payload, save_dir, &user_data)
            {
                save_info.map_name = extracted.to_string_lossy().into_owned();
                save_info.pristine_map_name = (!pristine.is_empty()).then_some(pristine);
            }
        }
        world_snapshot.w3d_ghost_companion = ghost_companion;
        world_snapshot.particle_system_companion = particle_companion;
        apply_persist_chunks(
            &mut world_snapshot,
            ingame_ui_payload.as_deref(),
            tactical_view_payload.as_deref(),
            script_engine_payload.as_deref(),
            terrain_logic_payload.as_deref(),
            radar_payload.as_deref(),
        );
        Ok(DecodedGameSave {
            snapshot: world_snapshot,
            campaign_state,
            save_info,
            game_client_xfer,
            terrain_visual_xfer,
            saved_game_mode,
        })
    }

    /// Host `CHUNK_GameLogic` is positional `WorldSnapshot` bincode, optionally
    /// wrapped in the older CommonGameState envelope. C++ `GameLogic::xfer`
    /// (`GameLogic.cpp:4666`) is a different stream: refuse to report success
    /// when those objects were not actually restored.
    fn decode_chunk_game_logic_for_host(payload: &[u8]) -> SaveLoadResult<WorldSnapshot> {
        decode_bincode_world_snapshot(payload).map_err(|host_err| {
            SaveLoadError::Corrupted(format!(
                "CHUNK_GameLogic is not a current host WorldSnapshot; C++ GameLogic::xfer (GameLogic.cpp:4666) was not restored ({host_err})"
            ))
        })
    }

    fn save_info_from_common_state(
        state: &CommonGameState,
        _world_snapshot: &WorldSnapshot,
    ) -> SaveGameInfo {
        let difficulty = match state
            .get_metadata("difficulty")
            .map(|s| s.as_str())
            .unwrap_or("Medium")
        {
            "Easy" => GameDifficulty::Easy,
            "Hard" => GameDifficulty::Hard,
            _ => GameDifficulty::Medium,
        };
        let save_type = match state.game_mode.as_str() {
            "Mission" => SaveFileType::Mission,
            "QuickSave" => SaveFileType::QuickSave,
            "AutoSave" => SaveFileType::AutoSave,
            _ => SaveFileType::Normal,
        };
        SaveGameInfo {
            pristine_map_name: None,
            filename: String::new(),
            display_name: state
                .get_metadata("display_name")
                .cloned()
                .unwrap_or_default(),
            description: state
                .get_metadata("description")
                .cloned()
                .unwrap_or_default(),
            map_name: state.map_name.clone(),
            campaign_side: state.get_metadata("campaign_side").cloned(),
            mission_number: state
                .get_metadata("mission_number")
                .and_then(|s| s.parse().ok()),
            save_date: UNIX_EPOCH + std::time::Duration::from_secs(state.timestamp),
            game_version: state
                .get_metadata("game_version")
                .cloned()
                .unwrap_or_default(),
            play_time: std::time::Duration::from_secs_f32(state.elapsed_time.max(0.0)),
            difficulty,
            save_type,
        }
    }

    /// Get full path for save file
    pub fn get_save_path(&self, filename: &str) -> PathBuf {
        let mut path = self.save_directory.clone();
        path.push(format!("{}.{}", filename, SAVE_EXTENSION));
        path
    }

    /// Get temporary file path
    fn get_temp_path(&self, filename: &str) -> PathBuf {
        let mut path = self.temp_directory.clone();
        path.push(format!("{}.tmp", filename));
        path
    }

    /// Clean up temporary files
    fn cleanup_temp_files(&self) -> SaveLoadResult<()> {
        if !self.temp_directory.exists() {
            return Ok(());
        }

        let entries = std::fs::read_dir(&self.temp_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == "tmp" {
                    if let Err(e) = std::fs::remove_file(&path) {
                        log::warn!("Failed to remove temp file {}: {}", path.display(), e);
                    }
                }
            }
        }

        Ok(())
    }

    /// Clean up old auto save files
    fn cleanup_old_auto_saves(&self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        let auto_saves: Vec<_> = saves
            .into_iter()
            .filter(|s| s.save_info.save_type == SaveFileType::AutoSave)
            .collect();

        // Keep only the 5 most recent auto saves
        if auto_saves.len() > 5 {
            for old_save in &auto_saves[5..] {
                if let Err(e) = self.delete_save(&old_save.filename) {
                    log::warn!(
                        "Failed to delete old auto save {}: {}",
                        old_save.filename,
                        e
                    );
                }
            }
        }

        Ok(())
    }

    fn enforce_save_limit(&self) -> SaveLoadResult<()> {
        let saves = self.list_saves()?;
        if saves.len() <= self.max_save_files {
            return Ok(());
        }

        for old_save in saves.iter().skip(self.max_save_files) {
            if let Err(e) = self.delete_save(&old_save.filename) {
                log::warn!(
                    "Failed to delete excess save {} while enforcing limit: {}",
                    old_save.filename,
                    e
                );
            }
        }

        Ok(())
    }
}

impl Default for SaveFileManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod rejected_logical_candidate_tests {
    use super::*;
    use crate::game_logic::{Object, ObjectId, Team, ThingTemplate};
    #[test]
    fn terminal_logical_candidate_is_retained_and_blocks_load_before_decode() {
        let mut manager = SaveFileManager::with_save_directory("unused-terminal-candidate");
        let mut failed = GameLogic::new();
        failed.add_object(Object::new(
            ThingTemplate::new("FailedOwner"),
            ObjectId(7),
            Team::USA,
        ));
        failed.frame = 91;
        let mut peer = GameLogic::new();
        peer.add_object(Object::new(
            ThingTemplate::new("Peer"),
            ObjectId(7),
            Team::USA,
        ));
        manager.retain_rejected_logical_world(Box::new(failed));
        let error = manager.load_game("not-decoded", &mut peer).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("terminal logical restore candidate")
        );
        assert_eq!(manager.rejected_logical_worlds.len(), 1);
        assert_eq!(manager.rejected_logical_worlds[0].frame, 91);
        assert!(
            manager.rejected_logical_worlds[0]
                .host_object(ObjectId(7))
                .is_some()
        );
        assert!(peer.host_object(ObjectId(7)).is_some());
        assert_eq!(peer.frame, 0);
    }
}
