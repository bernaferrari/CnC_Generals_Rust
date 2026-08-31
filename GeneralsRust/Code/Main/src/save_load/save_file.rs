use crate::game_logic::GameLogic;
use crate::save_load::*;
use game_engine::common::system::save_game::GameState as CommonGameState;
use game_engine::common::system::save_game::GameStateMap as CommonGameStateMap;
use game_engine::common::system::xfer::Xfer as CommonXfer;
use game_engine::common::system::xfer_load::XferLoad as CommonXferLoad;
use game_engine::common::system::xfer_save::XferSave as CommonXferSave;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufReader, BufWriter, Cursor, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

/// Save file format header
#[derive(Debug, Serialize, Deserialize)]
pub struct SaveFileHeader {
    pub magic: [u8; 4],         // "GZHS" (Generals Zero Hour Save)
    pub version: u32,           // Save format version
    pub flags: u32,             // Compression, encryption, etc.
    pub timestamp: u64,         // Unix timestamp
    pub checksum: u32,          // CRC32 of save data
    pub uncompressed_size: u64, // Original data size
    pub compressed_size: u64,   // Compressed data size
    pub game_version: [u8; 16], // Game version string
    pub reserved: [u8; 32],     // Reserved for future use
}

const SAVE_MAGIC: [u8; 4] = *b"GZHS";
const SAVE_HEADER_SIZE: usize = std::mem::size_of::<SaveFileHeader>();
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

impl SaveFileHeader {
    pub fn new() -> Self {
        Self {
            magic: SAVE_MAGIC,
            version: SAVE_FILE_VERSION,
            flags: 0,
            timestamp: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs(),
            checksum: 0,
            uncompressed_size: 0,
            compressed_size: 0,
            game_version: [0; 16],
            reserved: [0; 32],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.magic == SAVE_MAGIC && self.version <= SAVE_FILE_VERSION
    }

    pub fn is_compressed(&self) -> bool {
        (self.flags & 0x01) != 0
    }

    pub fn set_compressed(&mut self, compressed: bool) {
        if compressed {
            self.flags |= 0x01;
        } else {
            self.flags &= !0x01;
        }
    }
}

impl Default for SaveFileHeader {
    fn default() -> Self {
        Self::new()
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
        let save_path = self.get_save_path(filename);
        let temp_path = self.get_temp_path(&format!("{}_temp", filename));

        // Create snapshot of current game state
        let snapshot_builder = SnapshotBuilder::new();
        let mut world_snapshot = snapshot_builder.create_world_snapshot(game_logic)?;
        world_snapshot.client_drawables = client_drawables;
        set_pending_save_game_mode(Some(cpp_game_mode_from_live(game_logic.game_mode())));
        crate::save_load::stamp_player_team_chunks(game_logic);

        // Save to temporary file first
        let write_result = self.save_to_file(&temp_path, &world_snapshot, save_info);
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
        let mut save_path = self.get_save_path(filename);

        if !save_path.exists() {
            let mut legacy = self.save_directory.clone();
            legacy.push(format!("{}.{}", filename, LEGACY_SAVE_EXTENSION));
            if legacy.exists() {
                save_path = legacy;
            } else {
                return Err(SaveLoadError::FileNotFound(filename.to_string()));
            }
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
        if !save_path.exists() {
            let mut legacy = self.save_directory.clone();
            legacy.push(format!("{}.{}", filename, LEGACY_SAVE_EXTENSION));
            if legacy.exists() {
                return self.get_save_info_from_path(&legacy);
            }
        }
        self.get_save_info_from_path(&save_path)
    }

    fn get_save_info_from_path(&self, save_path: &Path) -> SaveLoadResult<SaveGameInfo> {
        let mut file = File::open(save_path)?;
        let mut all = Vec::new();
        file.read_to_end(&mut all)?;
        if Self::looks_like_common_sav_chunks(&all) {
            return Self::read_named_chunk_save_info(&all);
        }
        let mut reader = Cursor::new(all);
        let header = self.read_header(&mut reader)?;
        if !header.is_valid() {
            return Err(SaveLoadError::InvalidFormat);
        }
        self.read_save_info(&mut reader, &header)
    }

    /// List all available save files
    pub fn list_saves(&self) -> SaveLoadResult<Vec<AvailableGameInfo>> {
        let mut saves = Vec::new();

        let entries = std::fs::read_dir(&self.save_directory)?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();

            if let Some(extension) = path.extension() {
                if extension == SAVE_EXTENSION || extension == LEGACY_SAVE_EXTENSION {
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
    ) -> SaveLoadResult<()> {
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)?;
        let mut writer = BufWriter::new(file);
        let encoded = Self::write_common_sav_chunks(world_snapshot, save_info)?;
        writer.write_all(&encoded)?;
        writer.flush()?;
        Ok(())
    }

    /// Load data from file. Prefers Common `.sav` chunks; falls back to GZHS `.gen`.
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

        let mut reader = Cursor::new(all);
        let header = self.read_header(&mut reader)?;
        if !header.is_valid() {
            return Err(SaveLoadError::InvalidFormat);
        }

        if header.version > SAVE_FILE_VERSION {
            return Err(SaveLoadError::VersionMismatch {
                expected: SAVE_FILE_VERSION,
                actual: header.version,
            });
        }

        let save_info = self.read_save_info(&mut reader, &header)?;
        let mut world_data = Vec::with_capacity(header.compressed_size as usize);
        reader.read_to_end(&mut world_data)?;

        let actual_checksum = crc32fast::hash(&world_data);
        if actual_checksum != header.checksum {
            return Err(SaveLoadError::Corrupted(format!(
                "Checksum mismatch: expected {}, got {}",
                header.checksum, actual_checksum
            )));
        }

        let decompressed = if header.is_compressed() {
            compression::decompress(&world_data)?
        } else {
            world_data
        };

        let world_snapshot = match Self::decode_common_game_state(&decompressed) {
            Ok(common_state) => match Self::decode_world_snapshot_payload(&common_state.data) {
                Ok(snapshot) => snapshot,
                Err(common_payload_err) => {
                    // Raw legacy `.gen` bincode can happen to satisfy enough
                    // CommonXfer framing to produce an empty/invalid
                    // GameState.  Only commit to the wrapper route after its
                    // nested WorldSnapshot has decoded; otherwise retry the
                    // original raw payload.
                    log::warn!(
                        "GZHS Common SaveGame payload did not contain a valid WorldSnapshot ({}); falling back to raw legacy snapshot payload",
                        common_payload_err
                    );
                    Self::decode_world_snapshot_payload(&decompressed)?
                }
            },
            Err(common_err) => {
                log::warn!(
                    "Common SaveGame payload decode failed ({}), falling back to legacy snapshot payload",
                    common_err
                );
                Self::decode_world_snapshot_payload(&decompressed)?
            }
        };

        Ok((world_snapshot, save_info))
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
        let logic_payload = bincode::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Self::write_common_sav_chunks_with_payload(world_snapshot, save_info, logic_payload)
    }

    /// Shared 17-block container writer. Logic payload is kept separate so
    /// the outer C++ chunk table is independent of the positional
    /// WorldSnapshot schema and historical fixtures still migrate.
    fn write_common_sav_chunks_with_payload(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
    ) -> SaveLoadResult<Vec<u8>> {
        let ghost_bytes = capture_w3d_ghost_xfer_bytes().unwrap_or_default();
        let game_client_bytes = capture_game_client_xfer_bytes().unwrap_or_default();
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
                            let mut bytes = game_client_bytes.clone();
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
        let path = if save_path.exists() {
            save_path
        } else {
            let mut legacy = self.save_directory.clone();
            legacy.push(format!("{}.{}", filename, LEGACY_SAVE_EXTENSION));
            legacy
        };
        let data = std::fs::read(&path)?;
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
        match Self::decode_world_snapshot_payload(payload) {
            Ok(snapshot) => Ok(snapshot),
            Err(host_err) => {
                let mut wrapped = CommonGameState::default();
                let mut xfer = CommonXferLoad::new(Cursor::new(payload), SAVE_FILE_VERSION);
                if wrapped.xfer(&mut xfer).is_ok() && !wrapped.data.is_empty() {
                    if let Ok(snapshot) = Self::decode_world_snapshot_payload(&wrapped.data) {
                        return Ok(snapshot);
                    }
                }
                Err(SaveLoadError::Corrupted(format!(
                    "CHUNK_GameLogic is not a host WorldSnapshot; C++ GameLogic::xfer (GameLogic.cpp:4666) was not restored ({host_err})"
                )))
            }
        }
    }

    /// Decode the positional bincode payload shared by Common `.sav` chunks,
    /// GZHS-wrapped Common state, and the original raw `.gen` fallback.
    ///
    /// Production snapshot fields were appended inside nested records, so this
    /// must go through the exact v1 mirror instead of relying on serde defaults
    /// at each outer container call site.
    fn decode_world_snapshot_payload(payload: &[u8]) -> SaveLoadResult<WorldSnapshot> {
        let (snapshot, path) = decode_bincode_world_snapshot(payload)?;
        match path {
            BincodeWorldSnapshotDecodePath::Current => {}
            BincodeWorldSnapshotDecodePath::LegacyPreV20V19
            | BincodeWorldSnapshotDecodePath::LegacyPreV19V18
            | BincodeWorldSnapshotDecodePath::LegacyPreV18V17
            | BincodeWorldSnapshotDecodePath::LegacyPreV17V16
            | BincodeWorldSnapshotDecodePath::LegacyPreV16V15
            | BincodeWorldSnapshotDecodePath::LegacyPreV15V14
            | BincodeWorldSnapshotDecodePath::LegacyPreV14V13
            | BincodeWorldSnapshotDecodePath::LegacyPreV13V12
            | BincodeWorldSnapshotDecodePath::LegacyPreV12V11
            | BincodeWorldSnapshotDecodePath::LegacyPreV11V10
            | BincodeWorldSnapshotDecodePath::LegacyPreV10V9
            | BincodeWorldSnapshotDecodePath::LegacyPreV9V8
            | BincodeWorldSnapshotDecodePath::LegacyPreV8V7
            | BincodeWorldSnapshotDecodePath::LegacyPreV7V6
            | BincodeWorldSnapshotDecodePath::LegacyProductionV1
            | BincodeWorldSnapshotDecodePath::LegacyPreHackerDisableV2
            | BincodeWorldSnapshotDecodePath::LegacyPreV4V3
            | BincodeWorldSnapshotDecodePath::LegacyPreV5V4
            | BincodeWorldSnapshotDecodePath::LegacyPreV6V5 => {
                log::info!(
                    "Migrated legacy bincode WorldSnapshot ({path:?}) into schema v{}",
                    WORLD_SNAPSHOT_BINCODE_VERSION
                );
            }
        }
        Ok(snapshot)
    }

    fn common_state_from_save_info(
        save_info: &SaveGameInfo,
        world_snapshot: &WorldSnapshot,
    ) -> CommonGameState {
        let mut state = CommonGameState::new(SAVE_FILE_VERSION);
        state.timestamp = save_info
            .save_date
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        state.map_name = save_info.map_name.clone();
        state.game_mode = format!("{:?}", save_info.save_type);
        state.player_count = world_snapshot.players.len() as u32;
        state.current_frame = u32::try_from(world_snapshot.frame_number).unwrap_or(u32::MAX);
        state.elapsed_time = save_info.play_time.as_secs_f32();
        state.set_metadata("display_name".to_string(), save_info.display_name.clone());
        state.set_metadata("description".to_string(), save_info.description.clone());
        state.set_metadata("game_version".to_string(), save_info.game_version.clone());
        state.set_metadata(
            "difficulty".to_string(),
            format!("{:?}", save_info.difficulty),
        );
        if let Some(side) = &save_info.campaign_side {
            state.set_metadata("campaign_side".to_string(), side.clone());
        }
        if let Some(mission_number) = save_info.mission_number {
            state.set_metadata("mission_number".to_string(), mission_number.to_string());
        }
        state
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

    fn encode_common_game_state(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
    ) -> SaveLoadResult<Vec<u8>> {
        let logic_payload = bincode::serialize(world_snapshot)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Self::encode_common_game_state_with_payload(world_snapshot, save_info, logic_payload)
    }

    /// Encode the older GZHS wrapper's Common GameState body around an already
    /// serialized WorldSnapshot payload.
    fn encode_common_game_state_with_payload(
        world_snapshot: &WorldSnapshot,
        save_info: &SaveGameInfo,
        logic_payload: Vec<u8>,
    ) -> SaveLoadResult<Vec<u8>> {
        let mut state = CommonGameState::new(SAVE_FILE_VERSION);
        state.timestamp = save_info
            .save_date
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        state.map_name = save_info.map_name.clone();
        state.game_mode = format!("{:?}", save_info.save_type);
        state.player_count = world_snapshot.players.len() as u32;
        state.current_frame = u32::try_from(world_snapshot.frame_number).unwrap_or(u32::MAX);
        state.elapsed_time = save_info.play_time.as_secs_f32();
        state.set_metadata("display_name".to_string(), save_info.display_name.clone());
        state.set_metadata("description".to_string(), save_info.description.clone());
        state.set_metadata("game_version".to_string(), save_info.game_version.clone());
        state.set_metadata(
            "difficulty".to_string(),
            format!("{:?}", save_info.difficulty),
        );
        if let Some(side) = &save_info.campaign_side {
            state.set_metadata("campaign_side".to_string(), side.clone());
        }
        if let Some(mission_number) = save_info.mission_number {
            state.set_metadata("mission_number".to_string(), mission_number.to_string());
        }

        state.data = logic_payload;

        let mut cursor = Cursor::new(Vec::<u8>::new());
        {
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            state
                .xfer(&mut xfer)
                .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        }
        Ok(cursor.into_inner())
    }

    fn decode_common_game_state(data: &[u8]) -> SaveLoadResult<CommonGameState> {
        let mut state = CommonGameState::default();
        let mut xfer = CommonXferLoad::new(Cursor::new(data), SAVE_FILE_VERSION);
        state
            .xfer(&mut xfer)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;
        Ok(state)
    }

    /// Read file header
    fn read_header<R: Read>(&self, reader: &mut R) -> SaveLoadResult<SaveFileHeader> {
        let mut header_bytes = vec![0u8; SAVE_HEADER_SIZE];
        reader.read_exact(&mut header_bytes)?;

        let header: SaveFileHeader = bincode::deserialize(&header_bytes)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(header)
    }

    /// Read save info section
    fn read_save_info<R: Read>(
        &self,
        reader: &mut R,
        _header: &SaveFileHeader,
    ) -> SaveLoadResult<SaveGameInfo> {
        let mut size_bytes = [0u8; 4];
        reader.read_exact(&mut size_bytes)?;
        let size = u32::from_le_bytes(size_bytes) as usize;

        let mut info_bytes = vec![0u8; size];
        reader.read_exact(&mut info_bytes)?;

        let save_info: SaveGameInfo = bincode::deserialize(&info_bytes)
            .map_err(|e| SaveLoadError::Serialization(e.to_string()))?;

        Ok(save_info)
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
mod tests {
    use super::*;
    use crate::game_logic::{
        HackerDisableChannelPhase, HackerDisableChannelState, KindOf, ObjectId, Player,
        SupplyTruckState, Team, ThingTemplate,
    };
    use crate::save_load::snapshot::CollectorRuntimeSnapshot;
    use glam::Vec3;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn legacy_production_payload_fixture() -> (
        Vec<u8>,
        ObjectId,
        std::collections::HashMap<String, ThingTemplate>,
    ) {
        let mut source = GameLogic::new();
        source.add_player(Player::new(1, Team::USA, "Legacy Player", true));

        let mut barracks = ThingTemplate::new("LegacyBarracks");
        barracks
            .add_kind_of(KindOf::Structure)
            .add_kind_of(KindOf::Selectable);
        source
            .templates
            .insert("LegacyBarracks".to_string(), barracks);

        let mut ranger = ThingTemplate::new("LegacyRanger");
        ranger
            .add_kind_of(KindOf::Infantry)
            .add_kind_of(KindOf::Selectable)
            .set_cost(225, 0);
        ranger.build_time = 12.0;
        source.templates.insert("LegacyRanger".to_string(), ranger);

        let barracks_id = source
            .create_object("LegacyBarracks", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("legacy fixture barracks");
        assert!(source.enqueue_production(barracks_id, "LegacyRanger".to_string()));
        {
            let building = source
                .host_object_mut(barracks_id)
                .expect("legacy fixture barracks must remain live");
            let building_data = building
                .building_data
                .as_mut()
                .expect("legacy fixture needs production data");
            // These post-v1 values make an accidental current bincode decode
            // observably wrong while the historical serializer below omits
            // them exactly as an old saved game did.
            building_data.production_queue[0].progress = 4.5;
            building_data.production_queue[0].construction_frames = 135;
            building_data.production_queue[0].quantity_total = 2;
            building_data.production_queue[0].quantity_produced = 1;
            building_data.exit_delay_remaining = 0.3;
            building_data.exit_delay_remaining_frames = 9;
            building_data.exit_burst_remaining = 1;
            building_data.queue_exit_state_initialized = true;
        }

        let snapshot = SnapshotBuilder::new()
            .create_world_snapshot(&source)
            .expect("legacy fixture snapshot");
        let templates = source.templates.clone();
        (
            serialize_legacy_production_v1_fixture(snapshot)
                .expect("serialize exact v1 production fixture"),
            barracks_id,
            templates,
        )
    }

    fn fixture_save_info() -> SaveGameInfo {
        SaveGameInfo {
            filename: "legacy_fixture".to_string(),
            display_name: "Legacy production fixture".to_string(),
            description: "v1 bincode production payload".to_string(),
            map_name: "LegacyMap".to_string(),
            campaign_side: None,
            mission_number: None,
            save_date: UNIX_EPOCH,
            game_version: "test".to_string(),
            play_time: std::time::Duration::from_secs(0),
            difficulty: GameDifficulty::Medium,
            save_type: SaveFileType::Normal,
        }
    }

    fn assert_legacy_production_migrated(snapshot: &WorldSnapshot, barracks_id: ObjectId) {
        assert_eq!(snapshot.version, WORLD_SNAPSHOT_BINCODE_VERSION);
        assert_eq!(
            snapshot.next_weapon_discharge_sequence,
            default_next_weapon_discharge_sequence(),
            "v1/v2 records predate the v4 world tail"
        );
        assert!(snapshot.client_drawables.drawables.is_empty());
        let object = snapshot
            .objects
            .get(&barracks_id)
            .expect("migrated snapshot must retain its producer");
        let ModuleSnapshot::Production(production) = object
            .modules
            .get("Production")
            .expect("migrated snapshot must retain production module")
        else {
            panic!("migrated producer must use Production module");
        };
        let entry = production
            .production_queue
            .first()
            .expect("migrated producer must retain queue entry");
        assert_eq!(entry.template_name, "LegacyRanger");
        assert!((entry.progress - 4.5).abs() < f32::EPSILON);
        assert_eq!(entry.cost, 225);
        // These values never existed in the historical bincode record.  The
        // first live production tick reconstructs the integer frame counter
        // from `progress`; batch/exit state receives C++ legacy defaults.
        assert_eq!(entry.construction_frames, 0);
        assert_eq!(entry.quantity_total, 1);
        assert_eq!(entry.quantity_produced, 0);
        assert!(!entry.is_upgrade);
        assert_eq!(production.exit_delay_remaining, 0.0);
        assert_eq!(production.exit_delay_remaining_frames, 0);
        assert_eq!(production.exit_burst_remaining, 0);
        assert!(!production.queue_exit_state_initialized);
        assert!(object.hacker_disable_channel.is_none());
        assert_eq!(
            object.weapon_barrel_states,
            default_weapon_barrel_state_snapshots()
        );
        assert_eq!(object.last_weapon_discharge_sequence, 0);
        assert_eq!(object.last_weapon_discharge_slot, 0);
        assert_eq!(object.last_weapon_discharge_barrel, 0);
        assert_eq!(object.last_weapon_discharge_frame, 0);
    }

    fn assert_pre_v4_v3_migrated(snapshot: &WorldSnapshot, barracks_id: ObjectId) {
        assert_eq!(snapshot.version, WORLD_SNAPSHOT_BINCODE_VERSION);
        assert_eq!(
            snapshot.next_weapon_discharge_sequence,
            default_next_weapon_discharge_sequence(),
            "v3 ended before the v4 world tail"
        );
        assert!(
            snapshot.client_drawables.drawables.is_empty(),
            "v3 must default the renderer companion rather than replay stale visuals"
        );
        let object = snapshot
            .objects
            .get(&barracks_id)
            .expect("v3 migration must retain its producer");
        assert_eq!(
            object.hacker_disable_channel,
            Some(HackerDisableChannelState::new(
                ObjectId(77),
                HackerDisableChannelPhase::Preparing,
                1_500,
            )),
            "v3 must retain its final HDB object tail"
        );
        assert_eq!(
            object.weapon_barrel_states,
            default_weapon_barrel_state_snapshots(),
            "v3 must not reinterpret pre-v4 bytes as barrel cursors"
        );
        assert_eq!(object.last_weapon_discharge_sequence, 0);
        assert_eq!(object.last_weapon_discharge_slot, 0);
        assert_eq!(object.last_weapon_discharge_barrel, 0);
        assert_eq!(object.last_weapon_discharge_frame, 0);
    }

    fn gzhs_fixture_bytes(save_info: &SaveGameInfo, decompressed_payload: &[u8]) -> Vec<u8> {
        let mut header = SaveFileHeader::new();
        header.set_compressed(false);
        header.checksum = crc32fast::hash(decompressed_payload);
        header.uncompressed_size = decompressed_payload.len() as u64;
        header.compressed_size = decompressed_payload.len() as u64;

        let mut bytes = bincode::serialize(&header).expect("serialize GZHS header");
        assert!(bytes.len() <= SAVE_HEADER_SIZE);
        // `read_header` reserves the native header footprint before bincode
        // consumes its compact fields; mirror the legacy writer's padding.
        bytes.resize(SAVE_HEADER_SIZE, 0);

        let save_info = bincode::serialize(save_info).expect("serialize GZHS save info");
        bytes.extend_from_slice(&(save_info.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&save_info);
        bytes.extend_from_slice(decompressed_payload);
        bytes
    }

    fn unique_fixture_directory() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "generalsrust-legacy-production-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn test_save_header_serialization() {
        let mut header = SaveFileHeader::new();
        header.set_compressed(true);
        header.uncompressed_size = 12345;
        header.compressed_size = 6789;

        let serialized = bincode::serialize(&header).unwrap();
        let deserialized: SaveFileHeader = bincode::deserialize(&serialized).unwrap();

        assert_eq!(header.magic, deserialized.magic);
        assert_eq!(header.version, deserialized.version);
        assert_eq!(header.uncompressed_size, deserialized.uncompressed_size);
        assert_eq!(header.compressed_size, deserialized.compressed_size);
        assert!(deserialized.is_compressed());
        assert!(deserialized.is_valid());
    }

    #[test]
    fn test_save_file_paths() {
        let manager = SaveFileManager::new();

        let save_path = manager.get_save_path("test_save");
        assert!(save_path.to_string_lossy().contains("test_save"));
        assert!(
            save_path
                .to_string_lossy()
                .ends_with(&format!(".{}", SAVE_EXTENSION))
        );

        let temp_path = manager.get_temp_path("test_temp");
        assert!(temp_path.to_string_lossy().contains("test_temp"));
        assert!(temp_path.to_string_lossy().ends_with(".tmp"));
    }

    #[test]
    fn default_save_directory_is_user_data_save_like_popup() {
        let host = SaveLoadManager::default_save_directory();
        let popup = crate::subsystem_manager::resolve_save_directory();
        assert_eq!(
            host, popup,
            "host SaveFileManager and Popup TheGameState must share UserData/Save"
        );
        assert_eq!(host.file_name().and_then(|s| s.to_str()), Some("Save"));
        let manager = SaveFileManager::new();
        assert_eq!(manager.save_directory(), host.as_path());
    }

    #[test]
    fn legacy_bincode_production_payload_migrates_through_every_save_container() {
        let (legacy_payload, barracks_id, templates) = legacy_production_payload_fixture();

        // This is the regression: bincode's positional reader cannot use the
        // current nested serde defaults to safely consume an actual v1 record.
        assert!(
            bincode::deserialize::<WorldSnapshot>(&legacy_payload).is_err(),
            "the current positional record must not be trusted for a v1 production payload"
        );

        let (mut migrated, path) = decode_bincode_world_snapshot(&legacy_payload)
            .expect("exact legacy production payload should migrate");
        assert_eq!(path, BincodeWorldSnapshotDecodePath::LegacyProductionV1);
        assert_legacy_production_migrated(&migrated, barracks_id);

        // Schema v2 already had production frames and Queue exit state, but
        // its ObjectSnapshot predates the appended Hacker Disable channel.
        // Decode it via its exact mirror instead of allowing the current v4
        // object decoder to consume the following world fields as an Option.
        let (v2_source, v2_source_path) =
            decode_bincode_world_snapshot(&legacy_payload).expect("rebuild source for v2 fixture");
        assert_eq!(
            v2_source_path,
            BincodeWorldSnapshotDecodePath::LegacyProductionV1
        );
        let v2_payload = serialize_pre_hacker_disable_v2_fixture(v2_source)
            .expect("serialize exact pre-HDB v2 fixture");
        assert!(
            bincode::deserialize::<WorldSnapshot>(&v2_payload).is_err(),
            "the current v3 object record must not consume a pre-HDB v2 payload"
        );
        let (v2_migrated, v2_path) = decode_bincode_world_snapshot(&v2_payload)
            .expect("pre-HDB v2 payload should migrate through its exact mirror");
        assert_eq!(
            v2_path,
            BincodeWorldSnapshotDecodePath::LegacyPreHackerDisableV2
        );
        assert_legacy_production_migrated(&v2_migrated, barracks_id);

        // Version 3 was current before the v4 logical barrel/discharge and
        // renderer companion tails. It did include HDB, so this genuine
        // historical-shape fixture proves we choose its exact outer/Object
        // mirrors rather than trusting positional serde defaults.
        let mut v3_source = v2_migrated;
        let v3_object = v3_source
            .objects
            .get_mut(&barracks_id)
            .expect("v3 fixture producer");
        v3_object.hacker_disable_channel = Some(HackerDisableChannelState::new(
            ObjectId(77),
            HackerDisableChannelPhase::Preparing,
            1_500,
        ));
        v3_object.weapon_barrel_states[1] = WeaponBarrelStateSnapshot {
            current_barrel: 2,
            shots_left_on_barrel: 7,
        };
        v3_object.last_weapon_discharge_sequence = 91;
        v3_object.last_weapon_discharge_slot = 1;
        v3_object.last_weapon_discharge_barrel = 2;
        v3_object.last_weapon_discharge_frame = 4_200;
        v3_source.next_weapon_discharge_sequence = 92;
        let v3_payload =
            serialize_pre_v4_v3_fixture(v3_source).expect("serialize exact predecessor v3 fixture");
        assert!(
            bincode::deserialize::<WorldSnapshot>(&v3_payload).is_err(),
            "the current v4 record must not consume a v3 positional payload"
        );
        let (v3_migrated, v3_path) = decode_bincode_world_snapshot(&v3_payload)
            .expect("pre-v4 v3 payload should migrate through its exact mirror");
        assert_eq!(v3_path, BincodeWorldSnapshotDecodePath::LegacyPreV4V3);
        assert_pre_v4_v3_migrated(&v3_migrated, barracks_id);

        // The old float is converted at the first real production update,
        // where the restored template and live power factor are available.
        let mut restored = GameLogic::new();
        restored.templates = templates;
        SnapshotBuilder::new()
            .restore_from_snapshot(&migrated, &mut restored)
            .expect("restore migrated production snapshot");
        let restored_production = restored
            .host_object_mut(barracks_id)
            .expect("restored legacy producer")
            .building_data
            .as_mut()
            .expect("restored legacy production data");
        restored_production.production_queue[0].progress = 4.5;
        // 12 seconds at 30 FPS: floor(4.5 / 12 * 360) = 135, then this
        // update advances exactly one C++ logic frame.
        restored_production.production_queue[0].construction_frames = 0;
        restored_production.advance_production_progress(1.0, 1.0);
        assert_eq!(
            restored_production.production_queue[0].construction_frames,
            136
        );

        // A re-save is tagged v5 and uses the current record directly. Its
        // HDB channel must survive independently of older production layouts.
        migrated
            .objects
            .get_mut(&barracks_id)
            .expect("migrated producer for current HDB channel")
            .hacker_disable_channel = Some(HackerDisableChannelState::new(
            ObjectId(77),
            HackerDisableChannelPhase::Preparing,
            1_500,
        ));
        let current_object = migrated
            .objects
            .get_mut(&barracks_id)
            .expect("migrated producer for current v4 tails");
        current_object.weapon_barrel_states[0] = WeaponBarrelStateSnapshot {
            current_barrel: 2,
            shots_left_on_barrel: 3,
        };
        current_object.last_weapon_discharge_sequence = 88;
        current_object.last_weapon_discharge_slot = 0;
        current_object.last_weapon_discharge_barrel = 2;
        current_object.last_weapon_discharge_frame = 7_777;
        // A v5-only collector tail must be deliberately omitted from the
        // predecessor fixture. The decoder must still consume every v4 object
        // and world field without treating the next byte as an Option tag.
        current_object.collector_runtime = Some(CollectorRuntimeSnapshot {
            owner_player_id: Some(1),
            producer_id: Some(ObjectId(91)),
            preferred_dock_id: Some(ObjectId(92)),
            target: Some(ObjectId(93)),
            supply_center_spawn_behavior_fired: true,
            supply_truck_state: SupplyTruckState::DockingCenter,
            supply_truck_force_pending: true,
            supply_truck_next_dock_action_frame: 7_800,
            stored_supply_boxes: 4,
        });
        migrated.next_weapon_discharge_sequence = 89;
        migrated
            .client_drawables
            .drawables
            .push(ClientDrawableStateSnapshot {
                object_id: barracks_id.0,
                draw_module_index: 1,
                source_template_name: "LegacyBarracks".to_string(),
                model_key: "UVLegacyBarracks".to_string(),
                selected_condition_state_index: 2,
                animation: None,
                last_seen_weapon_discharge_sequence: 88,
                recoil_slots: std::array::from_fn(|_| Vec::new()),
            });
        let v4_payload =
            serialize_pre_v5_v4_fixture(migrated).expect("serialize exact predecessor v4 fixture");
        assert!(
            bincode::deserialize::<WorldSnapshot>(&v4_payload).is_err(),
            "the current v5 record must not consume a v4 positional payload"
        );
        let (mut migrated, v4_path) = decode_bincode_world_snapshot(&v4_payload)
            .expect("pre-v5 v4 payload should migrate through its exact mirror");
        assert_eq!(v4_path, BincodeWorldSnapshotDecodePath::LegacyPreV5V4);
        assert!(migrated.player_template_bindings.is_empty());
        assert!(
            migrated
                .objects
                .get(&barracks_id)
                .and_then(|object| object.collector_runtime.as_ref())
                .is_none(),
            "v4 predecessor records must default the v5 collector tail"
        );
        let current_payload = bincode::serialize(&migrated).expect("serialize current snapshot");
        let (current_round_trip, current_path) = decode_bincode_world_snapshot(&current_payload)
            .expect("current production snapshot should remain readable");
        assert_eq!(current_path, BincodeWorldSnapshotDecodePath::Current);
        assert_eq!(current_round_trip.version, WORLD_SNAPSHOT_BINCODE_VERSION);
        assert_eq!(
            current_round_trip
                .objects
                .get(&barracks_id)
                .and_then(|object| object.hacker_disable_channel),
            Some(HackerDisableChannelState::new(
                ObjectId(77),
                HackerDisableChannelPhase::Preparing,
                1_500,
            ))
        );
        let current_object = current_round_trip
            .objects
            .get(&barracks_id)
            .expect("current object tails");
        assert_eq!(
            current_object.weapon_barrel_states[0],
            WeaponBarrelStateSnapshot {
                current_barrel: 2,
                shots_left_on_barrel: 3,
            }
        );
        assert_eq!(current_object.last_weapon_discharge_sequence, 88);
        assert_eq!(current_object.last_weapon_discharge_slot, 0);
        assert_eq!(current_object.last_weapon_discharge_barrel, 2);
        assert_eq!(current_object.last_weapon_discharge_frame, 7_777);
        assert_eq!(current_round_trip.next_weapon_discharge_sequence, 89);
        assert_eq!(current_round_trip.client_drawables.drawables.len(), 1);
        assert_eq!(
            current_round_trip.client_drawables.drawables[0].last_seen_weapon_discharge_sequence,
            88
        );

        let mut future_payload = current_payload.clone();
        future_payload[..std::mem::size_of::<u32>()]
            .copy_from_slice(&(WORLD_SNAPSHOT_BINCODE_VERSION + 1).to_le_bytes());
        assert!(matches!(
            decode_bincode_world_snapshot(&future_payload),
            Err(SaveLoadError::VersionMismatch {
                expected: WORLD_SNAPSHOT_BINCODE_VERSION,
                actual
            }) if actual == WORLD_SNAPSHOT_BINCODE_VERSION + 1
        ));

        let save_info = fixture_save_info();

        // Native Common `.sav` CHUNK_GameLogic route.
        let common_chunks = SaveFileManager::write_common_sav_chunks_with_payload(
            &migrated,
            &save_info,
            legacy_payload.clone(),
        )
        .expect("encode Common fixture");
        let (common_snapshot, _) =
            SaveFileManager::read_common_sav_chunks(&common_chunks, Path::new(""))
                .expect("Common fixture should migrate legacy payload");
        assert_legacy_production_migrated(&common_snapshot, barracks_id);

        // V3 must choose the same exact migration path through the native
        // Common container, not only when decoding a raw test payload.
        let v3_common_chunks = SaveFileManager::write_common_sav_chunks_with_payload(
            &migrated,
            &save_info,
            v3_payload.clone(),
        )
        .expect("encode Common v3 fixture");
        let (v3_common_snapshot, _) =
            SaveFileManager::read_common_sav_chunks(&v3_common_chunks, Path::new(""))
                .expect("Common fixture should migrate v3 payload");
        assert_pre_v4_v3_migrated(&v3_common_snapshot, barracks_id);

        // The GZHS wrapper can contain a Common GameState body or the original
        // raw WorldSnapshot body; both call the same migration seam.
        let wrapped_common = SaveFileManager::encode_common_game_state_with_payload(
            &migrated,
            &save_info,
            legacy_payload.clone(),
        )
        .expect("encode GZHS Common payload");
        let fixture_directory = unique_fixture_directory();
        std::fs::create_dir_all(&fixture_directory).expect("create fixture directory");
        let manager = SaveFileManager::with_save_directory(&fixture_directory);

        let wrapped_path = fixture_directory.join("legacy_common.gen");
        std::fs::write(
            &wrapped_path,
            gzhs_fixture_bytes(&save_info, &wrapped_common),
        )
        .expect("write GZHS Common fixture");
        let (wrapped_snapshot, _) = manager
            .load_from_file(&wrapped_path)
            .expect("GZHS Common fixture should migrate legacy payload");
        assert_legacy_production_migrated(&wrapped_snapshot, barracks_id);

        let raw_path = fixture_directory.join("legacy_raw.gen");
        std::fs::write(&raw_path, gzhs_fixture_bytes(&save_info, &legacy_payload))
            .expect("write raw GZHS fixture");
        let (raw_snapshot, _) = manager
            .load_from_file(&raw_path)
            .expect("raw GZHS fixture should migrate legacy payload");
        assert_legacy_production_migrated(&raw_snapshot, barracks_id);

        let v3_wrapped_common = SaveFileManager::encode_common_game_state_with_payload(
            &migrated,
            &save_info,
            v3_payload.clone(),
        )
        .expect("encode GZHS Common v3 payload");
        let v3_wrapped_path = fixture_directory.join("v3_common.gen");
        std::fs::write(
            &v3_wrapped_path,
            gzhs_fixture_bytes(&save_info, &v3_wrapped_common),
        )
        .expect("write GZHS Common v3 fixture");
        let (v3_wrapped_snapshot, _) = manager
            .load_from_file(&v3_wrapped_path)
            .expect("GZHS Common fixture should migrate v3 payload");
        assert_pre_v4_v3_migrated(&v3_wrapped_snapshot, barracks_id);

        let v3_raw_path = fixture_directory.join("v3_raw.gen");
        std::fs::write(&v3_raw_path, gzhs_fixture_bytes(&save_info, &v3_payload))
            .expect("write raw GZHS v3 fixture");
        let (v3_raw_snapshot, _) = manager
            .load_from_file(&v3_raw_path)
            .expect("raw GZHS fixture should migrate v3 payload");
        assert_pre_v4_v3_migrated(&v3_raw_snapshot, barracks_id);

        let _ = std::fs::remove_file(wrapped_path);
        let _ = std::fs::remove_file(raw_path);
        let _ = std::fs::remove_file(v3_wrapped_path);
        let _ = std::fs::remove_file(v3_raw_path);
        let _ = std::fs::remove_dir(fixture_directory);
    }

    #[test]
    fn host_pause_save_writes_cpp_17_named_chunks_and_v2_header() {
        // C++ GameState::init (GameState.cpp:289-305) + xferSaveData
        // (GameState.cpp:1313-1381) writes 17 CHUNK_* tokens then SG_EOF.
        // Pre-fix host wrote only GameState/GameLogic/GhostObject with a
        // Rust-invented CommonGameState schema (version 1).
        let snapshot = WorldSnapshot::default();
        let save_info = fixture_save_info();
        let bytes = SaveFileManager::write_common_sav_chunks(&snapshot, &save_info)
            .expect("write 17-block sav");
        let text = String::from_utf8_lossy(&bytes);
        for name in SAVELOAD_BLOCK_NAMES {
            assert!(
                text.contains(name),
                "host writer must emit C++ block token {name}"
            );
        }
        assert!(text.contains(SAVE_FILE_EOF));

        let blocks = walk_named_chunks(&bytes).expect("walk host chunks");
        assert_eq!(blocks.len(), SAVELOAD_BLOCK_NAMES.len());
        assert_eq!(
            blocks
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            SAVELOAD_BLOCK_NAMES.to_vec()
        );

        let header = parse_cpp_game_state_header(&blocks[0].1).expect("C++ v2 header");
        assert_eq!(header.description, save_info.description);
        assert_eq!(header.map_name, "LegacyMap");
        assert_eq!(header.save_type, SaveFileType::Normal);

        let listed = SaveFileManager::read_named_chunk_save_info(&bytes).expect("list");
        assert_eq!(listed.description, save_info.description);
        assert_eq!(listed.map_name, "LegacyMap");
    }

    #[test]
    fn host_pause_save_persists_terrain_scorches_and_particle_systems() {
        game_client::terrain::clear_terrain_scorches();
        assert!(game_client::terrain::add_terrain_scorch(
            [88.0, 16.0, 4.0],
            22.0,
            1
        ));

        let snapshot = WorldSnapshot::default();
        let save_info = fixture_save_info();
        let bytes = SaveFileManager::write_common_sav_chunks(&snapshot, &save_info)
            .expect("write sav with FX chunks");
        let blocks = walk_named_chunks(&bytes).expect("walk host chunks");

        let terrain = blocks
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(CHUNK_TERRAIN_VISUAL))
            .expect("CHUNK_TerrainVisual");
        assert_eq!(terrain.1.first().copied(), Some(3));
        assert!(
            terrain.1.len() > 1,
            "CHUNK_TerrainVisual must not be NullSnapshot v1"
        );

        let particles = blocks
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(CHUNK_PARTICLE_SYSTEM))
            .expect("CHUNK_ParticleSystem");
        assert!(
            !particles.1.is_empty(),
            "CHUNK_ParticleSystem must write manager xfer"
        );

        game_client::terrain::clear_terrain_scorches();
        assert!(game_client::terrain::terrain_scorch_marks().is_empty());
        restore_terrain_visual_from_xfer_bytes(&terrain.1).expect("restore scorches from chunk");
        let marks = game_client::terrain::terrain_scorch_marks();
        assert_eq!(marks.len(), 1);
        assert_eq!(marks[0].location, [88.0, 16.0, 4.0]);
        assert_eq!(marks[0].radius, 22.0);
        assert_eq!(marks[0].scorch_type, 1);
        game_client::terrain::clear_terrain_scorches();
    }

    #[test]
    fn host_lists_cpp_game_state_version_2_without_rejecting() {
        // C++ GameState::xfer (GameState.cpp:1543-1559) writes version=2.
        // Pre-fix CommonGameState::xfer currentVersion=1 rejected it.
        let mut payload = Vec::new();
        {
            let mut cursor = Cursor::new(&mut payload);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            let info = SaveGameInfo {
                filename: "retail".into(),
                display_name: "Retail Save".into(),
                description: "C++ listed".into(),
                map_name: "Maps\\Alpine Assault.map".into(),
                campaign_side: Some("America".into()),
                mission_number: Some(3),
                save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
                game_version: "1.04".into(),
                play_time: std::time::Duration::from_secs(0),
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Mission,
            };
            write_cpp_game_state_header(&mut xfer, &info).expect("encode v2");
        }
        assert_eq!(payload.first().copied(), Some(2), "C++ currentVersion is 2");

        let mut bytes = Vec::new();
        bytes.push(CHUNK_GAME_STATE.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
        bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        bytes.push(SAVE_FILE_EOF.len() as u8);
        bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

        let info = SaveFileManager::read_named_chunk_save_info(&bytes)
            .expect("version 2 header must list");
        assert_eq!(info.description, "C++ listed");
        assert_eq!(info.save_type, SaveFileType::Mission);
        assert_eq!(info.campaign_side.as_deref(), Some("America"));
        assert_eq!(info.mission_number, Some(3));
        assert_eq!(info.map_name, "Maps\\Alpine Assault.map");
        // C++ mission files have no world restore payload. Host lists them
        // and loadGame restarts the mission instead of decoding GameLogic.
        let (snapshot, listed) = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
            .expect("mission header-only is a thin restart file");
        assert_eq!(listed.save_type, SaveFileType::Mission);
        assert!(
            snapshot.objects.is_empty(),
            "mission save must not invent a mid-world snapshot"
        );
    }

    #[test]
    fn game_state_header_writes_empty_campaign_and_invalid_mission() {
        // C++ GameState.cpp:1632-1638: no current campaign → empty side + -1.
        let mut payload = Vec::new();
        {
            let mut cursor = Cursor::new(&mut payload);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            write_cpp_game_state_header(
                &mut xfer,
                &SaveGameInfo {
                    filename: "skirmish".into(),
                    display_name: "Skirmish".into(),
                    description: "Skirmish".into(),
                    map_name: "Maps\\Alpine Assault.map".into(),
                    campaign_side: None,
                    mission_number: None,
                    save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
                    game_version: "1.04".into(),
                    play_time: std::time::Duration::from_secs(0),
                    difficulty: GameDifficulty::Medium,
                    save_type: SaveFileType::Normal,
                },
            )
            .expect("encode empty campaign header");
        }

        let mut bytes = Vec::new();
        bytes.push(CHUNK_GAME_STATE.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
        bytes.extend_from_slice(&(payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&payload);
        bytes.push(SAVE_FILE_EOF.len() as u8);
        bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

        let info = SaveFileManager::read_named_chunk_save_info(&bytes)
            .expect("empty campaign header must list");
        assert_eq!(info.campaign_side.as_deref(), None);
        assert_eq!(info.mission_number, None);
    }

    fn cpp_game_logic_xfer_with_objects() -> Vec<u8> {
        // Minimal C++ GameLogic::xfer (GameLogic.cpp:4666-4696): version 10,
        // frame, object TOC with one template, objectCount=1. Host bincode
        // cannot consume this as WorldSnapshot.
        let mut payload = Vec::new();
        {
            let mut cursor = Cursor::new(&mut payload);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            let mut version = 10u8;
            xfer.xfer_version(&mut version, 10)
                .expect("C++ GameLogic version");
            let mut frame = 42u32;
            xfer.xfer_unsigned_int(&mut frame).expect("frame");
            let mut toc_version = 1u8;
            xfer.xfer_version(&mut toc_version, 1).expect("TOC version");
            let mut toc_count = 1u32;
            xfer.xfer_unsigned_int(&mut toc_count).expect("TOC count");
            write_ascii(&mut xfer, "AmericaRanger").expect("TOC name");
            let mut toc_id = 1u16;
            xfer.xfer_unsigned_short(&mut toc_id).expect("TOC id");
            let mut object_count = 1u32;
            xfer.xfer_unsigned_int(&mut object_count)
                .expect("objectCount");
            xfer.xfer_unsigned_short(&mut toc_id)
                .expect("object TOC id");
            xfer.begin_block().expect("object block");
            let mut dummy = [0u8, 1, 2, 3];
            // SAFETY: dummy is a stack array of exactly its own length;
            // test-only save round-trip.
            unsafe {
                xfer.xfer_user(dummy.as_mut_ptr(), dummy.len())
                    .expect("object bytes");
            }
            xfer.end_block().expect("end object block");
        }
        payload
    }

    #[test]
    fn cpp_chunk_game_logic_does_not_report_successful_empty_world() {
        // C++ GameState::xferSaveData (GameState.cpp:1313-1381) writes
        // CHUNK_GameLogic via GameLogic::xfer (GameLogic.cpp:4666). Pre-fix
        // host decoded that as WorldSnapshot::default() and load reported
        // success with objects stripped.
        let mut header = Vec::new();
        {
            let mut cursor = Cursor::new(&mut header);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            write_cpp_game_state_header(
                &mut xfer,
                &SaveGameInfo {
                    filename: "retail".into(),
                    display_name: "Retail Save".into(),
                    description: "C++ listed".into(),
                    map_name: "Maps\\Alpine Assault.map".into(),
                    campaign_side: Some("America".into()),
                    mission_number: Some(3),
                    save_date: UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
                    game_version: "1.04".into(),
                    play_time: std::time::Duration::from_secs(0),
                    difficulty: GameDifficulty::Medium,
                    save_type: SaveFileType::Mission,
                },
            )
            .expect("encode v2 header");
        }
        let logic = cpp_game_logic_xfer_with_objects();
        assert_eq!(
            logic.first().copied(),
            Some(10),
            "C++ GameLogic currentVersion is 10"
        );

        let mut bytes = Vec::new();
        bytes.push(CHUNK_GAME_STATE.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
        bytes.extend_from_slice(&(header.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.push(CHUNK_GAME_LOGIC.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_LOGIC.as_bytes());
        bytes.extend_from_slice(&(logic.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&logic);
        bytes.push(SAVE_FILE_EOF.len() as u8);
        bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

        let listed = SaveFileManager::read_named_chunk_save_info(&bytes)
            .expect("C++ header must still list");
        assert_eq!(listed.description, "C++ listed");

        let err = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
            .expect_err("C++ GameLogic::xfer must not succeed as an empty host world");
        let err = err.to_string();
        assert!(
            err.contains("GameLogic::xfer") || err.contains("not a host WorldSnapshot"),
            "error must say C++ objects were not restored, got {err}"
        );

        let fixture_directory = unique_fixture_directory();
        std::fs::create_dir_all(&fixture_directory).expect("create fixture directory");
        let path = fixture_directory.join("retail_cpp.sav");
        std::fs::write(&path, &bytes).expect("write C++-shaped save");
        let mut manager = SaveFileManager::with_save_directory(&fixture_directory);
        let mut world = GameLogic::new();
        let load_err = manager
            .load_game("retail_cpp", &mut world)
            .expect_err("live load must refuse unrestored C++ CHUNK_GameLogic");
        assert!(
            world.host_objects().is_empty(),
            "fail-closed load must not populate a stripped world"
        );
        let load_err = load_err.to_string();
        assert!(
            load_err.contains("GameLogic::xfer") || load_err.contains("not a host WorldSnapshot"),
            "live load error must name the unrestored C++ stream, got {load_err}"
        );
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(fixture_directory);
    }

    #[test]
    fn mission_save_writes_only_game_state_and_campaign_chunks() {
        let snapshot = WorldSnapshot::default();
        let mut save_info = fixture_save_info();
        save_info.save_type = SaveFileType::Mission;
        save_info.map_name = "Maps\\Alpine Assault.map".into();
        save_info.description = "MissionSave".into();
        let bytes = SaveFileManager::write_common_sav_chunks(&snapshot, &save_info)
            .expect("write mission sav");
        let blocks = walk_named_chunks(&bytes).expect("walk mission chunks");
        let names: Vec<&str> = blocks.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, vec![CHUNK_GAME_STATE, CHUNK_CAMPAIGN]);
        let listed = SaveFileManager::read_named_chunk_save_info(&bytes).expect("list mission");
        assert_eq!(listed.save_type, SaveFileType::Mission);
        assert_eq!(listed.map_name, "Maps\\Alpine Assault.map");
        let (world, info) =
            SaveFileManager::read_common_sav_chunks(&bytes, Path::new("")).expect("read mission");
        assert_eq!(info.save_type, SaveFileType::Mission);
        assert!(world.objects.is_empty());
    }

    #[test]
    fn campaign_block_writes_runtime_difficulty_and_challenge() {
        use std::sync::Arc;
        game_engine::System::register_campaign_manager_runtime_hooks(
            Some(Arc::new(|| game_engine::System::CampaignManagerXferState {
                campaign: "GLA".into(),
                mission: "GLA02".into(),
                rank_points: 0,
                difficulty: 2,
                is_challenge: true,
                challenge_info: Some(game_engine::System::ChallengeGameInfoXfer::default()),
                generals_template: 4,
            })),
            None,
        );
        let snapshot = WorldSnapshot::default();
        let mut save_info = fixture_save_info();
        save_info.save_type = SaveFileType::Mission;
        save_info.map_name = "Maps\\GLA02.map".into();
        let bytes =
            SaveFileManager::write_common_sav_chunks(&snapshot, &save_info).expect("write mission");
        let listed = SaveFileManager::read_named_chunk_save_info(&bytes).expect("list");
        assert_eq!(listed.difficulty, GameDifficulty::Hard);
        let blocks = walk_named_chunks(&bytes).expect("walk");
        let campaign = blocks
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(CHUNK_CAMPAIGN))
            .map(|(_, payload)| parse_campaign_block(payload).expect("parse campaign"));
        let state = campaign.expect("CHUNK_Campaign");
        assert_eq!(state.campaign, "GLA");
        assert_eq!(state.mission, "GLA02");
        assert_eq!(state.difficulty, 2);
        assert!(state.is_challenge);
        assert_eq!(state.generals_template, 4);
        // Listing must surface CHUNK_Campaign difficulty (C++ loadGame then
        // MSG_NEW_GAME uses TheCampaignManager->getGameDifficulty()).
        assert_eq!(campaign_difficulty(&state), GameDifficulty::Hard);
    }

    #[test]
    fn load_prefers_embedded_scratch_over_installed_same_named_map() {
        // C++ GameStateMap::xfer always plays the extracted Save-dir copy.
        // Pre-fix live kept the header map when find_map_file hit retail.
        let root = unique_fixture_directory();
        let installed_dir = root.join("installed");
        let save_dir = root.join("Save");
        std::fs::create_dir_all(&installed_dir).expect("create installed dir");
        std::fs::create_dir_all(&save_dir).expect("create save dir");
        let leaf = "Hq6q2b5ScratchPrefer.map";
        let installed = installed_dir.join(leaf);
        std::fs::write(&installed, b"RETAIL-INSTALLED").expect("write installed map");

        let mut save_info = fixture_save_info();
        save_info.save_type = SaveFileType::Mission;
        save_info.map_name = installed.to_string_lossy().into_owned();
        assert!(
            crate::game_logic::script_loader::find_map_file(&save_info.map_name).is_some(),
            "fixture must make find_map_file hit the installed same-named map"
        );

        let mut header = Vec::new();
        {
            let mut cursor = Cursor::new(&mut header);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            write_cpp_game_state_header(&mut xfer, &save_info).expect("encode header");
        }

        let mut map_payload = Vec::new();
        {
            let mut cursor = Cursor::new(&mut map_payload);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            let mut version = 2u8;
            xfer.xfer_version(&mut version, 2).expect("map version");
            write_ascii(&mut xfer, &format!("Save\\{leaf}")).expect("save path");
            write_ascii(&mut xfer, &format!("Maps\\{leaf}")).expect("pristine path");
            let mut game_mode = 0i32;
            xfer.xfer_int(&mut game_mode).expect("game mode");
            xfer.begin_block().expect("begin embed");
            let mut map_bytes = b"SCRATCH-CUSTOM".to_vec();
            // SAFETY: map_bytes is an owned Vec; xfer_user reads exactly
            // its len for this embedded-map fixture.
            unsafe {
                xfer.xfer_user(map_bytes.as_mut_ptr(), map_bytes.len())
                    .expect("embed scratch");
            }
            xfer.end_block().expect("end embed");
            let mut object_id = 1u32;
            let mut drawable_id = 1u32;
            xfer.xfer_unsigned_int(&mut object_id).expect("object id");
            xfer.xfer_unsigned_int(&mut drawable_id)
                .expect("drawable id");
        }

        let mut bytes = Vec::new();
        bytes.push(CHUNK_GAME_STATE.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
        bytes.extend_from_slice(&(header.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.push(CHUNK_GAME_STATE_MAP.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_STATE_MAP.as_bytes());
        bytes.extend_from_slice(&(map_payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&map_payload);
        bytes.push(SAVE_FILE_EOF.len() as u8);
        bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

        let (_, listed) = SaveFileManager::read_common_sav_chunks(&bytes, &save_dir)
            .expect("mission + GameStateMap must load");
        let extracted = save_dir.join(leaf);
        assert_eq!(
            listed.map_name,
            extracted.to_string_lossy().into_owned(),
            "load must play the extracted Save-dir scratch, not the installed map"
        );
        assert_eq!(
            std::fs::read(&extracted).expect("read extracted scratch"),
            b"SCRATCH-CUSTOM"
        );

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn game_state_map_round_trips_live_game_mode() {
        set_pending_save_game_mode(Some(CPP_GAME_SKIRMISH));
        let mut payload = Vec::new();
        {
            let mut cursor = Cursor::new(&mut payload);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            let info = SaveGameInfo {
                filename: "mode".into(),
                display_name: "Mode".into(),
                description: "mode".into(),
                map_name: String::new(),
                campaign_side: None,
                mission_number: None,
                save_date: UNIX_EPOCH,
                game_version: "test".into(),
                play_time: std::time::Duration::from_secs(0),
                difficulty: GameDifficulty::Medium,
                save_type: SaveFileType::Normal,
            };
            write_game_state_map_block(&mut xfer, &info).expect("write map");
        }
        set_pending_save_game_mode(None);
        store_loaded_game_state_map_mode(None);
        let dest = unique_fixture_directory();
        let _ = extract_embedded_map(&payload, &dest);
        assert_eq!(
            take_loaded_game_state_map_mode(),
            Some(CPP_GAME_SKIRMISH),
            "CHUNK_GameStateMap v2 must persist TheGameLogic game mode"
        );
        let _ = std::fs::remove_dir_all(dest);
    }

    #[test]
    fn failed_load_does_not_apply_chunk_campaign_to_live_match() {
        // C++ GameState::loadGame only keeps CHUNK_Campaign after the whole
        // xfer succeeds; failure calls clearGameData. Live decode must stash
        // campaign and leave the still-playable match's identity/rank/difficulty.
        use std::sync::Arc;

        let prior = capture_live_campaign_state();
        let live = game_engine::System::CampaignManagerXferState {
            campaign: "USA".into(),
            mission: "USA01".into(),
            rank_points: 11,
            difficulty: 0,
            is_challenge: false,
            challenge_info: None,
            generals_template: 0,
        };
        apply_campaign_manager_state(live.clone());

        game_engine::System::register_campaign_manager_runtime_hooks(
            Some(Arc::new(|| game_engine::System::CampaignManagerXferState {
                campaign: "GLA".into(),
                mission: "GLA02".into(),
                rank_points: 99,
                difficulty: 2,
                is_challenge: false,
                challenge_info: None,
                generals_template: 4,
            })),
            None,
        );

        let mut campaign_payload = Vec::new();
        {
            let mut cursor = Cursor::new(&mut campaign_payload);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            write_campaign_block(&mut xfer).expect("write campaign");
        }

        let mut header = Vec::new();
        {
            let mut cursor = Cursor::new(&mut header);
            let mut xfer = CommonXferSave::new(&mut cursor, SAVE_FILE_VERSION);
            write_cpp_game_state_header(
                &mut xfer,
                &SaveGameInfo {
                    filename: "bad_campaign".into(),
                    display_name: "Bad Campaign".into(),
                    description: "failed load".into(),
                    map_name: "Maps\\Alpine Assault.map".into(),
                    campaign_side: Some("GLA".into()),
                    mission_number: Some(2),
                    save_date: UNIX_EPOCH,
                    game_version: "test".into(),
                    play_time: std::time::Duration::from_secs(0),
                    difficulty: GameDifficulty::Hard,
                    save_type: SaveFileType::Normal,
                },
            )
            .expect("encode header");
        }

        let logic = cpp_game_logic_xfer_with_objects();
        let mut bytes = Vec::new();
        bytes.push(CHUNK_GAME_STATE.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_STATE.as_bytes());
        bytes.extend_from_slice(&(header.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&header);
        bytes.push(CHUNK_CAMPAIGN.len() as u8);
        bytes.extend_from_slice(CHUNK_CAMPAIGN.as_bytes());
        bytes.extend_from_slice(&(campaign_payload.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&campaign_payload);
        bytes.push(CHUNK_GAME_LOGIC.len() as u8);
        bytes.extend_from_slice(CHUNK_GAME_LOGIC.as_bytes());
        bytes.extend_from_slice(&(logic.len() as i32).to_le_bytes());
        bytes.extend_from_slice(&logic);
        bytes.push(SAVE_FILE_EOF.len() as u8);
        bytes.extend_from_slice(SAVE_FILE_EOF.as_bytes());

        let decode_err = SaveFileManager::read_common_sav_chunks(&bytes, Path::new(""))
            .expect_err("C++ GameLogic must fail closed");
        let decode_err = decode_err.to_string();
        assert!(
            decode_err.contains("GameLogic::xfer")
                || decode_err.contains("not a host WorldSnapshot"),
            "unexpected decode error: {decode_err}"
        );
        let after_decode = capture_live_campaign_state();
        assert_eq!(after_decode.rank_points, 11);
        assert_eq!(after_decode.difficulty, 0);

        let fixture_directory = unique_fixture_directory();
        std::fs::create_dir_all(&fixture_directory).expect("create fixture directory");
        let path = fixture_directory.join("bad_campaign.sav");
        std::fs::write(&path, &bytes).expect("write failed-load save");
        let mut manager = SaveFileManager::with_save_directory(&fixture_directory);
        let mut world = GameLogic::new();
        manager
            .load_game("bad_campaign", &mut world)
            .expect_err("failed load must not succeed");
        let after_load = capture_live_campaign_state();
        assert_eq!(
            after_load.rank_points, 11,
            "failed load must not keep save rank"
        );
        assert_eq!(
            after_load.difficulty, 0,
            "failed load must not keep save difficulty"
        );

        let snapshot = WorldSnapshot::default();
        let mut save_info = fixture_save_info();
        save_info.save_type = SaveFileType::Mission;
        let mission_bytes = SaveFileManager::write_common_sav_chunks(&snapshot, &save_info)
            .expect("write mission sav");
        let _ = SaveFileManager::read_common_sav_chunks(&mission_bytes, Path::new(""))
            .expect("mission decode");
        let after_stash = capture_live_campaign_state();
        assert_eq!(
            after_stash.rank_points, 11,
            "successful decode must stash CHUNK_Campaign, not apply it"
        );
        commit_stashed_campaign_state();
        let after_commit = capture_live_campaign_state();
        assert_eq!(after_commit.rank_points, 99);
        assert_eq!(after_commit.difficulty, 2);

        apply_campaign_manager_state(prior);
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_dir(fixture_directory);
    }
}
