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
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

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

static PENDING_SAVE_GAME_MODE: Mutex<Option<i32>> = Mutex::new(None);
static LOADED_GAME_STATE_MAP_MODE: Mutex<Option<i32>> = Mutex::new(None);

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

fn set_pending_save_game_mode(mode: Option<i32>) {
    if let Ok(mut slot) = PENDING_SAVE_GAME_MODE.lock() {
        *slot = mode;
    }
}

fn pending_save_game_mode() -> i32 {
    PENDING_SAVE_GAME_MODE
        .lock()
        .ok()
        .and_then(|slot| *slot)
        .unwrap_or(0)
}

fn store_loaded_game_state_map_mode(mode: Option<i32>) {
    if let Ok(mut slot) = LOADED_GAME_STATE_MAP_MODE.lock() {
        *slot = mode;
    }
}

/// Take the `GameStateMap` v2 game-mode last decoded from a save.
pub fn take_loaded_game_state_map_mode() -> Option<i32> {
    LOADED_GAME_STATE_MAP_MODE
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

#[cfg(test)]
pub fn store_loaded_game_state_map_mode_for_test(mode: Option<i32>) {
    store_loaded_game_state_map_mode(mode);
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
fn write_campaign_block<W: Write + Seek>(xfer: &mut CommonXferSave<W>) -> SaveLoadResult<()> {
    let mut version = 5u8;
    xfer.xfer_version(&mut version, 5)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let mut state = game_engine::System::capture_campaign_manager_runtime();
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

fn apply_campaign_manager_state(state: game_engine::System::CampaignManagerXferState) {
    game_engine::System::apply_campaign_manager_runtime(state.clone());
    game_client::gui::campaign_manager::get_campaign_manager().apply_logic_chunk_state(state);
}

/// Stash CHUNK_Campaign until the staged restore commits. Applying during
/// `read_common_sav_chunks` mutates live campaign globals before map/snapshot
/// success — C++ `GameState::loadGame` only keeps campaign after the whole
/// xfer succeeds, and failed loads call `clearGameData`.
static PENDING_CAMPAIGN: Mutex<Option<game_engine::System::CampaignManagerXferState>> =
    Mutex::new(None);

fn stash_loaded_campaign_state(state: game_engine::System::CampaignManagerXferState) {
    if let Ok(mut slot) = PENDING_CAMPAIGN.lock() {
        *slot = Some(state);
    }
}

pub(crate) fn take_stashed_campaign_state() -> Option<game_engine::System::CampaignManagerXferState>
{
    PENDING_CAMPAIGN
        .lock()
        .ok()
        .and_then(|mut slot| slot.take())
}

pub(crate) fn discard_stashed_campaign_state() {
    if let Ok(mut slot) = PENDING_CAMPAIGN.lock() {
        *slot = None;
    }
}

pub(crate) fn capture_live_campaign_state() -> game_engine::System::CampaignManagerXferState {
    game_client::gui::campaign_manager::get_campaign_manager().capture_logic_chunk_state()
}

pub(crate) fn commit_stashed_campaign_state() {
    if let Some(state) = take_stashed_campaign_state() {
        apply_campaign_manager_state(state);
    }
}

/// C++ `GameState::loadGame` (`GameState.cpp:695-712`) `clearGameData` on
/// xfer/loadPostProcess failure. Restore the pre-load campaign so a failed
/// CHUNK_Campaign decode cannot stick on the still-playable match.
pub(crate) fn rollback_campaign_after_failed_load(
    prior: game_engine::System::CampaignManagerXferState,
) {
    discard_stashed_campaign_state();
    apply_campaign_manager_state(prior);
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
                snapshot.persist_v18.camera_valid = true;
                snapshot.persist_v18.camera_angle = camera.angle;
                snapshot.persist_v18.camera_position = camera.position;
                snapshot.persist_v18.camera_target = camera.target;
                snapshot.persist_v18.camera_zoom = camera.zoom;
                persist_v18::set_pending_camera(camera);
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
) -> SaveLoadResult<()> {
    let mut version = 2u8;
    xfer.xfer_version(&mut version, 2)
        .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
    let leaf = map_leaf_name(&save_info.map_name);
    write_ascii(xfer, &format!("Save\\{leaf}"))?;
    let pristine = if save_info.map_name.is_empty() {
        String::new()
    } else {
        format!("Maps\\{}", leaf)
    };
    write_ascii(xfer, &pristine)?;
    let mut game_mode = pending_save_game_mode();
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
    Ok(())
}

fn extract_embedded_map(payload: &[u8], save_dir: &Path) -> Option<PathBuf> {
    let mut xfer = CommonXferLoad::new(Cursor::new(payload), SAVE_FILE_VERSION);
    let mut version = 0u8;
    xfer.xfer_version(&mut version, 2).ok()?;
    let save_game_map = read_ascii(&mut xfer).ok()?;
    let _pristine = read_ascii(&mut xfer).ok()?;
    if version >= 2 {
        let mut game_mode = 0i32;
        xfer.xfer_int(&mut game_mode).ok()?;
        store_loaded_game_state_map_mode(Some(game_mode));
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
    Some(dest)
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
}

impl SaveFileManager {
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
        )
    }

    fn save_game_with_client_state_impl(
        &mut self,
        filename: &str,
        game_logic: &GameLogic,
        client_drawables: ClientDrawableWorldSnapshot,
        mut client: Option<&mut game_client::core::game_client::GameClient>,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<()> {
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
        // C++ GameState writes the logic chunk before GameClient::xfer.
        let client_xfer_bytes = if let Some(client) = client {
            capture_game_client_xfer_bytes(client)?
        } else {
            Vec::new()
        };
        set_pending_save_game_mode(Some(cpp_game_mode_from_live(game_logic.game_mode())));
        crate::save_load::stamp_player_team_chunks(game_logic);

        // C++ GameState::saveGame (GameState.cpp:534) makes sure the save
        // directory exists at save time; our atomic flow writes into
        // save_directory/temp first, so that directory must exist too.
        std::fs::create_dir_all(&self.temp_directory)?;

        // Save to temporary file first
        let write_result =
            self.save_to_file(&temp_path, &world_snapshot, save_info, &client_xfer_bytes);
        set_pending_save_game_mode(None);
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
    pub fn load_game(
        &mut self,
        filename: &str,
        game_logic: &mut GameLogic,
    ) -> SaveLoadResult<SaveGameInfo> {
        let prior_campaign = capture_live_campaign_state();
        let (world_snapshot, save_info) = match self.load_game_snapshot(filename) {
            Ok(decoded) => decoded,
            Err(err) => {
                rollback_campaign_after_failed_load(prior_campaign);
                return Err(err);
            }
        };
        if save_info.save_type != SaveFileType::Mission {
            if let Err(err) = self.restore_game_snapshot(&world_snapshot, game_logic) {
                rollback_campaign_after_failed_load(prior_campaign);
                return Err(err);
            }
        }
        commit_stashed_campaign_state();

        log::info!("Game loaded successfully from save slot: {}", filename);
        Ok(save_info)
    }

    /// Decode a save without mutating a live `GameLogic` instance.
    ///
    /// The runtime host uses this to load the saved map into a staging world
    /// before it restores the snapshot.  Keeping the decode separate from the
    /// restore means a bad/missing map or corrupted snapshot cannot partially
    /// overwrite the currently playable match.
    pub fn load_game_snapshot(
        &self,
        filename: &str,
    ) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
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
    ) -> SaveLoadResult<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        let encoded = Self::write_common_sav_chunks_with_client_bytes(
            world_snapshot,
            save_info,
            client_xfer_bytes,
        )?;
        writer.write_all(&encoded)?;
        writer.flush()?;
        Ok(())
    }

    /// Load the current named-chunk `.sav` container.
    fn load_from_file(&self, path: &Path) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
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
    ) -> SaveLoadResult<Vec<u8>> {
        Self::write_common_sav_chunks_with_client_bytes(world_snapshot, save_info, &[])
    }

    fn write_common_sav_chunks_with_client_bytes(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        client_xfer_bytes: &[u8],
    ) -> SaveLoadResult<Vec<u8>> {
        validate_direct_world_snapshot_version(world_snapshot.version)?;
        let logic_payload = bincode_legacy::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Self::write_common_sav_chunks_with_payload_and_client(
            world_snapshot,
            save_info,
            logic_payload,
            client_xfer_bytes,
        )
    }

    /// Shared 17-block container writer. Logic payload is kept separate so
    /// the outer C++ chunk table is independent of the positional
    /// WorldSnapshot schema and historical fixtures still migrate.
    fn write_common_sav_chunks_with_payload(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
    ) -> SaveLoadResult<Vec<u8>> {
        Self::write_common_sav_chunks_with_payload_and_client(
            world_snapshot,
            save_info,
            logic_payload,
            &[],
        )
    }

    fn write_common_sav_chunks_with_payload_and_client(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
        game_client_bytes: &[u8],
    ) -> SaveLoadResult<Vec<u8>> {
        let ghost_bytes = capture_w3d_ghost_xfer_bytes().unwrap_or_default();
        let particle_system_bytes = capture_particle_system_xfer_bytes().unwrap_or_default();
        let terrain_visual_bytes = capture_terrain_visual_xfer_bytes().unwrap_or_default();
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
                    CHUNK_GAME_STATE_MAP => write_game_state_map_block(xfer, save_info),
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
                            let mut bytes = particle_system_bytes.clone();
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
                    CHUNK_CAMPAIGN => write_campaign_block(xfer),
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
                    CHUNK_PLAYERS => write_players_block(xfer),
                    CHUNK_TEAM_FACTORY => write_team_factory_block(xfer),

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

    fn read_common_sav_chunks(
        data: &[u8],
        save_dir: &Path,
    ) -> SaveLoadResult<(WorldSnapshot, SaveGameInfo)> {
        discard_stashed_campaign_state();
        store_loaded_game_state_map_mode(None);
        let blocks = walk_named_chunks(data)?;
        let mut save_info = SaveGameInfo {
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
        let mut logic_data: Option<Vec<u8>> = None;
        let mut saw_game_state = false;
        let mut ingame_ui_payload: Option<Vec<u8>> = None;
        let mut tactical_view_payload: Option<Vec<u8>> = None;
        let mut script_engine_payload: Option<Vec<u8>> = None;
        let mut terrain_logic_payload: Option<Vec<u8>> = None;
        let mut radar_payload: Option<Vec<u8>> = None;
        let mut players_payload: Option<Vec<u8>> = None;
        let mut team_factory_payload: Option<Vec<u8>> = None;

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
                    stash_loaded_campaign_state(state);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_GHOST_OBJECT) {
                if payload.first().copied() == Some(1) && payload.len() > 1 {
                    stash_loaded_w3d_ghost_xfer(payload[1..].to_vec());
                } else {
                    let mut block = CommonGameState::default();
                    let mut xfer = CommonXferLoad::new(Cursor::new(&payload), SAVE_FILE_VERSION);
                    if block.xfer(&mut xfer).is_ok() {
                        stash_loaded_w3d_ghost_xfer(block.data);
                    }
                }
            } else if token.eq_ignore_ascii_case(CHUNK_GAME_CLIENT) {
                // NullSnapshot is a lone version-1 byte. Leftover GameClient::xfer
                // starts at version 3 and recreates objectless drawables.
                if payload.first().copied() != Some(1) || payload.len() > 1 {
                    stash_loaded_game_client_xfer(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_PARTICLE_SYSTEM) {
                // NullSnapshot is a lone version-1 byte. Manager xfer is
                // version 1 plus uniqueSystemID / systemCount / systems.
                if payload.len() > 1 {
                    stash_loaded_particle_system_xfer(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_TERRAIN_VISUAL) {
                // NullSnapshot is a lone version-1 byte. W3DTerrainVisual::xfer
                // starts at version 3 and carries scorches after the tree/prop
                // snapshot.
                if payload.first().copied() != Some(1) || payload.len() > 1 {
                    stash_loaded_terrain_visual_xfer(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_GAME_STATE_MAP) {
                // C++ extractAndSaveMap (GameStateMap.cpp:308-368) parks the
                // embedded .map in the Save directory and always sets
                // TheWritableGlobalData->m_mapName to that scratch path.
                // An installed same-named retail map must not override it.
                if let Some(extracted) = extract_embedded_map(&payload, save_dir) {
                    save_info.map_name = extracted.to_string_lossy().into_owned();
                }
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
                if payload.len() > 1 {
                    players_payload = Some(payload);
                }
            } else if token.eq_ignore_ascii_case(CHUNK_TEAM_FACTORY) {
                if payload.len() > 1 {
                    team_factory_payload = Some(payload);
                }
            }
        }
        stash_loaded_player_team_chunks(
            players_payload.as_deref(),
            team_factory_payload.as_deref(),
        );
        if !saw_game_state {
            return Err(SaveLoadError::Corrupted(
                "CHUNK_GameState missing from named-chunk save".to_string(),
            ));
        }
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
        apply_persist_chunks(
            &mut world_snapshot,
            ingame_ui_payload.as_deref(),
            tactical_view_payload.as_deref(),
            script_engine_payload.as_deref(),
            terrain_logic_payload.as_deref(),
            radar_payload.as_deref(),
        );
        Ok((world_snapshot, save_info))
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

// Global save file manager instance
lazy_static::lazy_static! {
    pub static ref SAVE_FILE_MANAGER: std::sync::Mutex<SaveFileManager> =
        std::sync::Mutex::new(SaveFileManager::new());
}

/// Initialize the global save file system
pub fn init_save_file_system() -> SaveLoadResult<()> {
    let mut manager = SAVE_FILE_MANAGER.lock().unwrap_or_else(|e| e.into_inner());
    manager.init()
}

#[cfg(test)]
mod tests;
