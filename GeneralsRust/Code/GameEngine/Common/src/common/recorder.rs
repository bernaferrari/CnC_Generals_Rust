use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{Datelike, Local, TimeZone, Timelike};

use crate::common::ini::ini_game_data::get_global_data;
use crate::common::message_stream::{
    Coord3D, GameMessage, GameMessageArgumentDataType, GameMessageArgumentType, GameMessageType,
    ICoord2D, MessageSerializer, get_message_stream, is_network_command_message,
};
use crate::common::random_value::{
    get_game_logic_random_seed, init_game_logic_random, init_random_with_seed,
};
use crate::common::version::get_version;
use crate::game_network::{
    PLAYERTEMPLATE_OBSERVER,
    game_info::{GameInfo, SlotState},
};

/// Recorder operating mode
/// Matches C++ RecorderModeType from Recorder.h:23-27
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecorderMode {
    /// Recording gameplay to replay file
    Record,
    /// Playing back a recorded replay
    Playback,
    /// Inactive (shell, saved game, or no replay)
    None,
}

/// Recorder class - main replay recording/playback system
/// Matches C++ RecorderClass from Recorder.h:31-131 and Recorder.cpp
pub struct Recorder {
    /// Current mode (record/playback/none)
    mode: RecorderMode,

    /// File handle for reading or writing replay
    file: Option<File>,

    /// Filename of current replay
    filename: String,

    /// Current file position for seeking
    current_file_position: u64,

    /// Next frame number to execute command (playback only)
    next_frame: i32,

    /// Original game mode for replay metadata
    original_game_mode: i32,

    /// Current replay filename (valid during playback)
    current_replay_filename: String,

    /// CRC validation data
    crc_info: Option<CrcInfo>,

    /// Was a desync detected
    was_desync: bool,

    /// Doing offline analysis
    doing_analysis: bool,

    /// Replay game info (slot list, etc.)
    game_info: ReplayGameInfo,

    /// Current frame (if no frame provider is set)
    current_frame: u32,

    /// Optional frame provider for live frame queries
    frame_provider: Option<Arc<dyn Fn() -> u32 + Send + Sync>>,

    /// Optional game-mode provider for multiplayer parity checks.
    game_mode_provider: Option<Arc<dyn Fn() -> i32 + Send + Sync>>,

    /// Optional command source for recording
    command_source: Option<Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync>>,

    /// Optional command sink for playback
    command_sink: Option<Arc<dyn Fn(GameMessage) + Send + Sync>>,

    /// Optional command list culler during playback
    command_cull: Option<Arc<dyn Fn() + Send + Sync>>,

    /// Pending commands when no sink is configured
    pending_commands: Vec<GameMessage>,

    /// Live lobby snapshot used by `startRecording` after `reset()` (C++ TheLAN / TheSkirmishGameInfo).
    pending_lobby: Option<ReplayGameInfo>,
    pending_local_index: i32,
    /// Optional live lobby provider consulted after reset.
    lobby_info_provider: Option<Arc<dyn Fn() -> Option<(ReplayGameInfo, i32)> + Send + Sync>>,
    /// C++ `TheWindowManager->winHide` hook for ReplayControl.wnd.
    replay_control_hook: Option<Arc<dyn Fn(bool) + Send + Sync>>,
    replay_controls_hidden: bool,
    /// C++ `ThePlayerList->getNthPlayer(i)->getPlayerNameKey()` for CRC same-player test.
    player_name_key_provider: Option<Arc<dyn Fn(i32) -> Option<String> + Send + Sync>>,
}

const GAME_SINGLE_PLAYER: i32 = 0;
const GAME_SHELL: i32 = 4;
const GAME_NONE: i32 = 6;
const REPLAY_TIME_T_BYTES: u64 = 4;
const REPLAY_SYSTEM_TIME_BYTES: usize = 16;
const REPLAY_STATS_OFFSET: u64 = 6;
const REPLAY_END_TIME_OFFSET: u64 = REPLAY_STATS_OFFSET + REPLAY_TIME_T_BYTES;
const REPLAY_FRAME_DURATION_OFFSET: u64 = REPLAY_END_TIME_OFFSET + REPLAY_TIME_T_BYTES;
const REPLAY_DESYNC_OFFSET: u64 = REPLAY_FRAME_DURATION_OFFSET + 4;
const REPLAY_QUIT_EARLY_OFFSET: u64 = REPLAY_DESYNC_OFFSET + 1;
const REPLAY_PLAYER_DISCONNECTS_OFFSET: u64 = REPLAY_QUIT_EARLY_OFFSET + 1;
const _REPLAY_FIXED_HEADER_SIZE: usize = REPLAY_PLAYER_DISCONNECTS_OFFSET as usize + 8;

/// CRC information for sync validation
/// Matches C++ CRCInfo from Recorder.cpp:903-956
#[derive(Debug, Clone)]
struct CrcInfo {
    local_player: u32,
    saw_crc_mismatch: bool,
    _skipped_one: bool,
    data: Vec<u32>,
}

impl CrcInfo {
    fn new() -> Self {
        Self {
            local_player: u32::MAX,
            saw_crc_mismatch: false,
            _skipped_one: false,
            data: Vec::new(),
        }
    }

    fn add_crc(&mut self, val: u32) {
        self.data.push(val);
    }

    fn read_crc(&mut self) -> u32 {
        if self.data.is_empty() {
            return 0;
        }
        self.data.remove(0)
    }

    fn set_local_player(&mut self, index: u32) {
        self.local_player = index;
    }

    fn get_local_player(&self) -> u32 {
        self.local_player
    }

    fn set_saw_crc_mismatch(&mut self) {
        self.saw_crc_mismatch = true;
    }

    fn saw_crc_mismatch(&self) -> bool {
        self.saw_crc_mismatch
    }
}

/// Replay game information
/// Matches C++ ReplayGameInfo from Recorder.h:10-21
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayGameInfo {
    pub map: String,
    pub seed: u32,
    pub crc_interval: u32,
    pub slots: Vec<ReplaySlot>,
    #[serde(default)]
    pub local_ip: u32,
    #[serde(default)]
    pub in_game: bool,
    #[serde(default)]
    pub in_progress: bool,
    /// C++ GameInfo::getUseStats / m_useStats.
    #[serde(default)]
    pub use_stats: i32,
    /// C++ GameInfo map CRC / size / contents mask.
    #[serde(default)]
    pub map_crc: u32,
    #[serde(default)]
    pub map_size: u32,
    #[serde(default)]
    pub map_contents_mask: u32,
    /// C++ GameInfo::getSuperweaponRestriction.
    #[serde(default)]
    pub superweapon_restriction: u32,
    /// C++ GameInfo::getStartingCash().countMoney().
    #[serde(default)]
    pub starting_cash: u32,
    /// C++ GameInfo::oldFactionsOnly.
    #[serde(default)]
    pub old_factions_only: bool,
}

impl Default for ReplayGameInfo {
    fn default() -> Self {
        Self::new()
    }
}

impl ReplayGameInfo {
    pub fn new() -> Self {
        const MAX_SLOTS: usize = 8;
        Self {
            map: String::new(),
            seed: 0,
            crc_interval: 100, // REPLAY_CRC_INTERVAL from Recorder.cpp:30
            slots: vec![ReplaySlot::default(); MAX_SLOTS],
            local_ip: 0,
            in_game: false,
            in_progress: false,
            use_stats: 0,
            map_crc: 0,
            map_size: 0,
            map_contents_mask: 0,
            superweapon_restriction: 0,
            starting_cash: 0,
            old_factions_only: false,
        }
    }

    pub fn reset(&mut self) {
        self.map.clear();
        self.seed = 0;
        self.slots
            .iter_mut()
            .for_each(|s| *s = ReplaySlot::default());
        self.local_ip = 0;
        self.in_game = false;
        self.in_progress = false;
        self.use_stats = 0;
        self.map_crc = 0;
        self.map_size = 0;
        self.map_contents_mask = 0;
        self.superweapon_restriction = 0;
        self.starting_cash = 0;
        self.old_factions_only = false;
    }

    pub fn set_map(&mut self, map: String) {
        self.map = map;
    }

    pub fn get_map(&self) -> &str {
        &self.map
    }

    pub fn set_seed(&mut self, seed: u32) {
        self.seed = seed;
    }

    pub fn get_seed(&self) -> u32 {
        self.seed
    }

    pub fn set_crc_interval(&mut self, interval: u32) {
        self.crc_interval = interval;
    }

    pub fn get_crc_interval(&self) -> u32 {
        self.crc_interval
    }

    pub fn clear_slot_list(&mut self) {
        self.slots
            .iter_mut()
            .for_each(|s| *s = ReplaySlot::default());
    }

    pub fn enter_game(&mut self) {
        self.in_game = true;
    }

    pub fn start_game(&mut self, _timestamp: u64) {
        self.in_game = true;
        self.in_progress = true;
    }

    pub fn end_game(&mut self) {
        self.in_game = false;
        self.in_progress = false;
    }

    pub fn get_slot(&self, index: usize) -> Option<&ReplaySlot> {
        self.slots.get(index)
    }

    pub fn get_slot_mut(&mut self, index: usize) -> Option<&mut ReplaySlot> {
        self.slots.get_mut(index)
    }

    pub fn set_local_ip(&mut self, ip: u32) {
        self.local_ip = ip;
    }

    pub fn get_local_ip(&self) -> u32 {
        self.local_ip
    }

    pub fn apply_network_info(&mut self, info: &GameInfo) {
        self.map = info.get_map().to_string();
        self.seed = info.get_seed().max(0) as u32;
        self.crc_interval = info.get_crc_interval().max(0) as u32;

        for slot_index in 0..self.slots.len() {
            if let Some(net_slot) = info.get_slot(slot_index) {
                let replay_slot = &mut self.slots[slot_index];
                replay_slot.name = net_slot.get_name().to_string();
                replay_slot.ip = net_slot.get_ip();
                replay_slot.is_human = net_slot.is_human();
                replay_slot.is_occupied = net_slot.is_occupied();
                replay_slot.player_template = net_slot.get_player_template();
                replay_slot.is_observer = replay_slot.player_template == PLAYERTEMPLATE_OBSERVER;
            } else if let Some(slot) = self.slots.get_mut(slot_index) {
                *slot = ReplaySlot::default();
            }
        }
    }

    pub fn to_game_info(&self) -> GameInfo {
        let mut info = GameInfo::new();
        info.set_map(self.map.clone());
        info.set_seed(self.seed as i32);
        info.set_crc_interval(self.crc_interval as i32);
        info.set_local_ip(self.local_ip);

        for (slot_index, slot) in self.slots.iter().enumerate() {
            if let Some(game_slot) = info.get_slot_mut(slot_index) {
                if slot.is_occupied {
                    let state = if slot.is_human {
                        SlotState::Player
                    } else {
                        SlotState::MedAI
                    };
                    let name = if slot.is_human {
                        slot.name.clone()
                    } else {
                        String::new()
                    };
                    game_slot.set_state(state, name, slot.ip);
                    game_slot.set_player_template(if slot.is_observer {
                        PLAYERTEMPLATE_OBSERVER
                    } else {
                        slot.player_template
                    });
                } else {
                    game_slot.set_state(SlotState::Closed, String::new(), 0);
                }
            }
        }

        info
    }
}

/// Replay slot information
/// Matches C++ GameSlot data used in ReplayGameInfo
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplaySlot {
    pub name: String,
    pub ip: u32,
    pub is_human: bool,
    pub is_observer: bool,
    pub is_occupied: bool,
    /// C++ GameSlot::m_color, or -1 for random.
    #[serde(default = "replay_slot_unset")]
    pub color: i32,
    /// C++ GameSlot::m_playerTemplate.
    #[serde(default = "replay_slot_unset")]
    pub player_template: i32,
    /// C++ GameSlot::m_startPos, or -1 for random.
    #[serde(default = "replay_slot_unset")]
    pub start_pos: i32,
    /// C++ GameSlot::m_teamNumber, or -1 for none.
    #[serde(default = "replay_slot_unset")]
    pub team: i32,
    /// C++ GameSlot::m_NATBehavior.
    #[serde(default)]
    pub nat: i32,
    /// C++ GameSlot port.
    #[serde(default)]
    pub port: u32,
    #[serde(default)]
    pub accepted: bool,
    #[serde(default)]
    pub has_map: bool,
    /// 0 none, 1 easy, 2 medium, 3 hard — C++ SLOT_EASY_AI / MED / BRUTAL.
    #[serde(default)]
    pub ai_difficulty: i32,
    /// Open (not closed) when unoccupied. C++ SLOT_OPEN vs SLOT_CLOSED.
    #[serde(default)]
    pub is_open: bool,
}

fn replay_slot_unset() -> i32 {
    -1
}

impl Default for ReplaySlot {
    fn default() -> Self {
        Self {
            name: String::new(),
            ip: 0,
            is_human: false,
            is_observer: false,
            is_occupied: false,
            color: -1,
            player_template: -1,
            start_pos: -1,
            team: -1,
            nat: 0,
            port: 0,
            accepted: false,
            has_map: false,
            ai_difficulty: 0,
            is_open: false,
        }
    }
}

impl ReplaySlot {
    pub fn get_name(&self) -> &str {
        &self.name
    }

    pub fn get_ip(&self) -> u32 {
        self.ip
    }

    pub fn is_human(&self) -> bool {
        self.is_human
    }

    pub fn is_observer(&self) -> bool {
        self.is_observer
    }

    pub fn is_occupied(&self) -> bool {
        self.is_occupied
    }
}

/// C++ `MSG_CLEAR_GAME_DATA` (MessageStream.h after refined mouse messages).
const CPP_MSG_CLEAR_GAME_DATA: u32 = 27;
/// C++ `MSG_NEW_GAME`.
const CPP_MSG_NEW_GAME: u32 = 28;
/// C++ `MSG_BEGIN_NETWORK_MESSAGES`.
#[allow(dead_code)]
const CPP_MSG_BEGIN_NETWORK: u32 = 1000;
/// C++ `MSG_CREATE_SELECTED_GROUP`.
const CPP_MSG_CREATE_SELECTED_GROUP: u32 = 1001;
const CPP_MSG_CREATE_TEAM0: u32 = 1006;
const CPP_MSG_SELECT_TEAM0: u32 = 1016;
const CPP_MSG_ADD_TEAM0: u32 = 1026;
const CPP_MSG_DO_ATTACKSQUAD: u32 = 1036;
const CPP_MSG_LOGIC_CRC: u32 = 1095;

/// Serialize lobby options like C++ `GameInfoToAsciiString` (GameInfo.cpp:875-972).
fn format_replay_game_info(info: &ReplayGameInfo) -> String {
    let map = info.map.replace('\\', "/");
    let mut options = format!(
        "US={};M={:02x}{};MC={:X};MS={};SD={};C={};SR={};SC={};O={};S=",
        info.use_stats,
        info.map_contents_mask & 0xff,
        map,
        info.map_crc,
        info.map_size,
        info.seed,
        info.crc_interval,
        info.superweapon_restriction,
        info.starting_cash,
        if info.old_factions_only { 'Y' } else { 'N' },
    );
    for slot in info.slots.iter().take(8) {
        if slot.is_occupied && slot.is_human {
            options.push_str(&format!(
                "H{},{:X},{},{}{},{},{},{},{},{}:",
                slot.name,
                slot.ip,
                slot.port,
                if slot.accepted { 'T' } else { 'F' },
                if slot.has_map { 'T' } else { 'F' },
                slot.color,
                slot.player_template,
                slot.start_pos,
                slot.team,
                slot.nat
            ));
        } else if slot.is_occupied {
            let ai = match slot.ai_difficulty {
                1 => 'E',
                3 => 'H',
                _ => 'M',
            };
            options.push_str(&format!(
                "C{},{},{},{},{}:",
                ai, slot.color, slot.player_template, slot.start_pos, slot.team
            ));
        } else if slot.is_open {
            options.push_str("O:");
        } else {
            options.push_str("X:");
        }
    }
    options.push(';');
    options
}

/// Parse C++ `GameInfoToAsciiString` output into `ReplayGameInfo`.
/// Matches `ParseAsciiStringToGameInfo` (GameInfo.cpp:982-1470) enough to restore
/// map/seed/CRC/slot color/template/start/team/NAT.
fn parse_replay_game_info(options: &str, info: &mut ReplayGameInfo) -> bool {
    if options.is_empty() {
        return false;
    }
    let mut saw_seed = false;
    let mut saw_slots = false;
    for pair in options.split(';') {
        if pair.is_empty() {
            continue;
        }
        let (key, val) = match pair.split_once('=') {
            Some(kv) => kv,
            None => continue,
        };
        match key {
            "US" => {
                info.use_stats = val.parse().unwrap_or(0);
            }
            "M" => {
                if val.len() >= 2 {
                    info.map_contents_mask = u32::from_str_radix(&val[..2], 16).unwrap_or(0);
                    info.map = val[2..].to_string();
                }
            }
            "MC" => {
                info.map_crc = u32::from_str_radix(val, 16).unwrap_or(0);
            }
            "MS" => {
                info.map_size = val.parse().unwrap_or(0);
            }
            "SD" => {
                info.seed = val.parse().unwrap_or(0);
                saw_seed = true;
            }
            "C" => {
                info.crc_interval = val.parse().unwrap_or(100);
            }
            "SR" => {
                info.superweapon_restriction = val.parse().unwrap_or(0);
            }
            "SC" => {
                info.starting_cash = val.parse().unwrap_or(0);
            }
            "O" => {
                info.old_factions_only = val.eq_ignore_ascii_case("Y");
            }
            "S" => {
                saw_slots = parse_replay_slot_list(val, info);
            }
            _ => {}
        }
    }
    saw_seed && saw_slots
}

fn parse_replay_slot_list(raw: &str, info: &mut ReplayGameInfo) -> bool {
    let mut parts: Vec<&str> = raw.split(':').collect();
    if parts.last() == Some(&"") {
        parts.pop();
    }
    if parts.len() < 8 {
        return false;
    }
    for (i, part) in parts.iter().take(8).enumerate() {
        let slot = match info.slots.get_mut(i) {
            Some(s) => s,
            None => return false,
        };
        *slot = ReplaySlot::default();
        if part.is_empty() {
            return false;
        }
        match part.as_bytes()[0] {
            b'X' => {}
            b'O' => {
                slot.is_open = true;
            }
            b'H' => {
                let fields: Vec<&str> = part[1..].split(',').collect();
                if fields.len() < 9 {
                    return false;
                }
                slot.name = fields[0].to_string();
                slot.ip = u32::from_str_radix(fields[1], 16).unwrap_or(0);
                slot.port = fields[2].parse().unwrap_or(0);
                let flags = fields[3].as_bytes();
                slot.accepted = flags.first() == Some(&b'T');
                slot.has_map = flags.get(1) == Some(&b'T');
                slot.color = fields[4].parse().unwrap_or(-1);
                slot.player_template = fields[5].parse().unwrap_or(-1);
                slot.start_pos = fields[6].parse().unwrap_or(-1);
                slot.team = fields[7].parse().unwrap_or(-1);
                slot.nat = fields[8].parse().unwrap_or(0);
                slot.is_human = true;
                slot.is_occupied = true;
                slot.is_observer = slot.player_template == PLAYERTEMPLATE_OBSERVER;
            }
            b'C' => {
                let fields: Vec<&str> = part.split(',').collect();
                if fields.len() < 5 {
                    return false;
                }
                slot.ai_difficulty = match fields[0].as_bytes().get(1) {
                    Some(&b'E') => 1,
                    Some(&b'H') => 3,
                    _ => 2,
                };
                slot.color = fields[1].parse().unwrap_or(-1);
                slot.player_template = fields[2].parse().unwrap_or(-1);
                slot.start_pos = fields[3].parse().unwrap_or(-1);
                slot.team = fields[4].parse().unwrap_or(-1);
                slot.is_human = false;
                slot.is_occupied = true;
                slot.is_observer = slot.player_template == PLAYERTEMPLATE_OBSERVER;
            }
            _ => return false,
        }
    }
    true
}

/// C++ `GameMessage::Type` stored in `.rep` files (MessageStream.h:447-570).
fn replay_type_id(msg: &GameMessageType) -> Result<u32, std::io::Error> {
    use GameMessageType::*;
    let id = match msg {
        ClearGameData => CPP_MSG_CLEAR_GAME_DATA,
        NewGame => CPP_MSG_NEW_GAME,
        CreateSelectedGroup(..) => CPP_MSG_CREATE_SELECTED_GROUP,
        CreateSelectedGroupNoSound(..) => CPP_MSG_CREATE_SELECTED_GROUP + 1,
        DestroySelectedGroup(..) => CPP_MSG_CREATE_SELECTED_GROUP + 2,
        RemoveFromSelectedGroup(..) => CPP_MSG_CREATE_SELECTED_GROUP + 3,
        SelectedGroupCommand(..) => CPP_MSG_CREATE_SELECTED_GROUP + 4,
        CreateTeamSlot(slot) => CPP_MSG_CREATE_TEAM0 + u32::from(*slot),
        SelectTeamSlot(slot) => CPP_MSG_SELECT_TEAM0 + u32::from(*slot),
        AddTeamSlot(slot) => CPP_MSG_ADD_TEAM0 + u32::from(*slot),
        other => {
            let dense = MessageSerializer::get_message_type_id(other).map_err(|err| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{:?}", err))
            })?;
            if (87..=148).contains(&dense) {
                CPP_MSG_DO_ATTACKSQUAD + u32::from(dense - 87)
            } else if dense == 25 {
                CPP_MSG_CLEAR_GAME_DATA
            } else if dense == 26 {
                CPP_MSG_NEW_GAME
            } else {
                u32::from(dense)
            }
        }
    };
    Ok(id)
}

fn decode_replay_type_id(
    id: u32,
    args: &[GameMessageArgumentType],
) -> Result<(GameMessageType, usize), std::io::Error> {
    let (dense, team_slot) = match id {
        CPP_MSG_CLEAR_GAME_DATA => (25u16, None),
        CPP_MSG_NEW_GAME => (26u16, None),
        1001..=1005 => ((79 + (id - CPP_MSG_CREATE_SELECTED_GROUP)) as u16, None),
        1006..=1015 => (84, Some(((id - CPP_MSG_CREATE_TEAM0) as u8, 0u8))),
        1016..=1025 => (85, Some(((id - CPP_MSG_SELECT_TEAM0) as u8, 1u8))),
        1026..=1035 => (86, Some(((id - CPP_MSG_ADD_TEAM0) as u8, 2u8))),
        1036..=1097 => ((87 + (id - CPP_MSG_DO_ATTACKSQUAD)) as u16, None),
        0..=148 => (id as u16, None),
        _ => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Replay message type out of range: {}", id),
            ));
        }
    };
    if let Some((slot, kind)) = team_slot {
        let ty = match kind {
            0 => GameMessageType::CreateTeamSlot(slot),
            1 => GameMessageType::SelectTeamSlot(slot),
            _ => GameMessageType::AddTeamSlot(slot),
        };
        return Ok((ty, 0));
    }
    if (79..=83).contains(&dense) {
        return decode_selected_group_type(dense, args);
    }
    if dense == 143 {
        return Ok(decode_set_replay_camera(args));
    }
    MessageSerializer::decode_message_type(dense, args)
        .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, format!("{:?}", err)))
}

fn decode_selected_group_type(
    dense: u16,
    args: &[GameMessageArgumentType],
) -> Result<(GameMessageType, usize), std::io::Error> {
    let mut consumed = 0;
    match dense {
        79 | 80 => {
            let create_new = match args.first() {
                Some(GameMessageArgumentType::Boolean(v)) => {
                    consumed = 1;
                    *v
                }
                _ => true,
            };
            let mut ids = Vec::new();
            for arg in args.iter().skip(consumed) {
                match arg {
                    GameMessageArgumentType::ObjectID(id) => {
                        ids.push(*id);
                        consumed += 1;
                    }
                    _ => break,
                }
            }
            if dense == 79 {
                Ok((
                    GameMessageType::CreateSelectedGroup(create_new, ids),
                    consumed,
                ))
            } else {
                Ok((
                    GameMessageType::CreateSelectedGroupNoSound(create_new, ids),
                    consumed,
                ))
            }
        }
        81 => {
            let team = match args.first() {
                Some(GameMessageArgumentType::TeamID(v)) => {
                    consumed = 1;
                    *v
                }
                Some(GameMessageArgumentType::Integer(v)) => {
                    consumed = 1;
                    *v as u32
                }
                _ => 0,
            };
            Ok((GameMessageType::DestroySelectedGroup(team), consumed))
        }
        82 => {
            let mut ids = Vec::new();
            for arg in args {
                match arg {
                    GameMessageArgumentType::ObjectID(id) => {
                        ids.push(*id);
                        consumed += 1;
                    }
                    _ => break,
                }
            }
            Ok((GameMessageType::RemoveFromSelectedGroup(ids), consumed))
        }
        83 => {
            let team = match args.first() {
                Some(GameMessageArgumentType::TeamID(v)) => {
                    consumed = 1;
                    *v
                }
                Some(GameMessageArgumentType::Integer(v)) => {
                    consumed = 1;
                    *v as u32
                }
                _ => 0,
            };
            Ok((GameMessageType::SelectedGroupCommand(team), consumed))
        }
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unhandled selected-group replay type {}", dense),
        )),
    }
}

/// C++ `LookAtXlat.cpp:463-469` / `GameLogicDispatch.cpp:1810-1820`:
/// loc, angle, pitch, zoom, MouseCursor int, ICoord2D pixel.
fn decode_set_replay_camera(args: &[GameMessageArgumentType]) -> (GameMessageType, usize) {
    let loc = args
        .iter()
        .find_map(|arg| match arg {
            GameMessageArgumentType::Location(coord) => Some(coord.clone()),
            _ => None,
        })
        .unwrap_or(Coord3D {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        });
    let reals: Vec<f32> = args
        .iter()
        .filter_map(|arg| match arg {
            GameMessageArgumentType::Real(value) => Some(*value),
            _ => None,
        })
        .collect();
    let angle = reals.first().copied().unwrap_or(0.0);
    let zoom = reals
        .get(2)
        .copied()
        .or_else(|| reals.get(1).copied())
        .unwrap_or(0.0);
    // Consume 0 so loc/angle/pitch/zoom/cursor/pixel stay on the message.
    (GameMessageType::SetReplayCamera(loc, angle, zoom), 0)
}

fn set_replay_camera_file_args(
    msg: &GameMessage,
    coord: &Coord3D,
    type_angle: f32,
    type_zoom: f32,
) -> Vec<GameMessageArgumentType> {
    let extras: Vec<GameMessageArgumentType> = msg
        .get_arguments()
        .iter()
        .map(|arg| arg.data.clone())
        .collect();
    let loc = extras
        .iter()
        .find_map(|arg| match arg {
            GameMessageArgumentType::Location(value) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or_else(|| coord.clone());
    let reals: Vec<f32> = extras
        .iter()
        .filter_map(|arg| match arg {
            GameMessageArgumentType::Real(value) => Some(*value),
            _ => None,
        })
        .collect();
    let angle = reals.first().copied().unwrap_or(type_angle);
    let pitch = reals.get(1).copied().unwrap_or(0.0);
    let zoom = reals.get(2).copied().unwrap_or(type_zoom);
    let cursor = extras
        .iter()
        .find_map(|arg| match arg {
            GameMessageArgumentType::Integer(value) => Some(*value),
            _ => None,
        })
        .unwrap_or(0);
    let pixel = extras
        .iter()
        .find_map(|arg| match arg {
            GameMessageArgumentType::Pixel(value) => Some(value.clone()),
            _ => None,
        })
        .unwrap_or(ICoord2D { x: 0, y: 0 });
    vec![
        GameMessageArgumentType::Location(loc),
        GameMessageArgumentType::Real(angle),
        GameMessageArgumentType::Real(pitch),
        GameMessageArgumentType::Real(zoom),
        GameMessageArgumentType::Integer(cursor),
        GameMessageArgumentType::Pixel(pixel),
    ]
}

fn keep_command_during_playback(msg: &GameMessage) -> bool {
    let ty = msg.get_type();
    !(is_network_command_message(ty) && !matches!(ty, GameMessageType::LogicCRC(_)))
}

/// Replay header structure
/// Matches C++ RecorderClass::ReplayHeader from Recorder.h:61-80
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayHeader {
    pub filename: String,
    pub for_playback: bool,
    pub replay_name: String,
    pub time_val: SystemTime,
    pub version_string: String,
    pub version_time_string: String,
    pub version_number: u32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub start_time: u64,
    pub end_time: u64,
    pub frame_duration: u32,
    pub quit_early: bool,
    pub desync_game: bool,
    pub player_discons: [bool; 8], // MAX_SLOTS
    pub game_options: String,
    pub local_player_index: i32,
}

impl Default for ReplayHeader {
    fn default() -> Self {
        Self {
            filename: String::new(),
            for_playback: false,
            replay_name: String::new(),
            time_val: SystemTime::now(),
            version_string: env!("CARGO_PKG_VERSION").to_string(),
            version_time_string: String::new(),
            version_number: 1,
            exe_crc: 0,
            ini_crc: 0,
            start_time: 0,
            end_time: 0,
            frame_duration: 0,
            quit_early: false,
            desync_game: false,
            player_discons: [false; 8],
            game_options: String::new(),
            local_player_index: -1,
        }
    }
}

fn write_fixed_width_time_t(file: &mut File, value: u32) -> Result<(), std::io::Error> {
    file.write_all(&value.to_le_bytes())
}

fn read_fixed_width_time_t(file: &mut File) -> Result<u32, std::io::Error> {
    let mut buf = [0u8; REPLAY_TIME_T_BYTES as usize];
    file.read_exact(&mut buf)?;
    Ok(u32::from_le_bytes(buf))
}

fn encode_system_time_from_local(now: chrono::DateTime<Local>) -> [u8; REPLAY_SYSTEM_TIME_BYTES] {
    let fields = [
        now.year() as u16,
        now.month() as u16,
        now.weekday().num_days_from_sunday() as u16,
        now.day() as u16,
        now.hour() as u16,
        now.minute() as u16,
        now.second() as u16,
        now.timestamp_subsec_millis() as u16,
    ];

    let mut bytes = [0u8; REPLAY_SYSTEM_TIME_BYTES];
    for (index, field) in fields.iter().enumerate() {
        let offset = index * 2;
        bytes[offset..offset + 2].copy_from_slice(&field.to_le_bytes());
    }
    bytes
}

fn system_time_from_replay_bytes(bytes: [u8; REPLAY_SYSTEM_TIME_BYTES]) -> SystemTime {
    let year = u16::from_le_bytes([bytes[0], bytes[1]]) as i32;
    let month = u16::from_le_bytes([bytes[2], bytes[3]]) as u32;
    let day = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
    let hour = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
    let minute = u16::from_le_bytes([bytes[10], bytes[11]]) as u32;
    let second = u16::from_le_bytes([bytes[12], bytes[13]]) as u32;
    let milliseconds = u16::from_le_bytes([bytes[14], bytes[15]]) as u32;

    let Some(local_time) = Local
        .with_ymd_and_hms(year, month, day, hour, minute, second)
        .earliest()
    else {
        return UNIX_EPOCH;
    };

    let utc_time = (local_time + chrono::Duration::milliseconds(milliseconds as i64))
        .with_timezone(&chrono::Utc);
    let seconds = utc_time.timestamp();
    let nanos = utc_time.timestamp_subsec_nanos();

    if seconds >= 0 {
        UNIX_EPOCH + std::time::Duration::new(seconds as u64, nanos)
    } else {
        UNIX_EPOCH - std::time::Duration::new((-seconds) as u64, nanos)
    }
}

fn read_system_time_from_file(file: &mut File) -> Result<SystemTime, std::io::Error> {
    let mut buf = [0u8; REPLAY_SYSTEM_TIME_BYTES];
    file.read_exact(&mut buf)?;
    Ok(system_time_from_replay_bytes(buf))
}

fn write_system_time_to_file(
    file: &mut File,
    local_time: chrono::DateTime<Local>,
) -> Result<(), std::io::Error> {
    file.write_all(&encode_system_time_from_local(local_time))
}

/// C++ `RecorderClass::playbackFile` / `stopPlayback` append
/// `MSG_NEW_GAME` / `MSG_CLEAR_GAME_DATA` to **TheMessageStream**
/// (Recorder.cpp:447, 1123-1134), not TheCommandList.
fn append_game_message_to_the_message_stream(msg: &GameMessage) {
    let stream_lock = get_message_stream();
    let Ok(mut stream) = stream_lock.write() else {
        return;
    };
    let dest = stream.append_message(msg.get_type().clone());
    dest.set_player_index(msg.get_player_index());
    for arg in msg.get_arguments() {
        match &arg.data {
            GameMessageArgumentType::Integer(v) => dest.append_integer_argument(*v),
            GameMessageArgumentType::Real(v) => dest.append_real_argument(*v),
            GameMessageArgumentType::Boolean(v) => dest.append_boolean_argument(*v),
            GameMessageArgumentType::ObjectID(v) => dest.append_object_id_argument(*v),
            GameMessageArgumentType::DrawableID(v) => dest.append_drawable_id_argument(*v),
            GameMessageArgumentType::TeamID(v) | GameMessageArgumentType::SquadID(v) => {
                dest.append_team_id_argument(*v)
            }
            GameMessageArgumentType::Location(v) => dest.append_location_argument(v.clone()),
            GameMessageArgumentType::Pixel(v) => dest.append_pixel_argument(v.clone()),
            GameMessageArgumentType::PixelRegion(v) => dest.append_pixel_region_argument(v.clone()),
            GameMessageArgumentType::Timestamp(v) => dest.append_timestamp_argument(*v),
            GameMessageArgumentType::WideChar(v) => dest.append_wide_char_argument(*v),
            GameMessageArgumentType::String(v) => dest.append_string_argument(v.clone()),
        }
    }
}

fn send_clear_game_data(
    command_sink: &Option<Arc<dyn Fn(GameMessage) + Send + Sync>>,
    pending_commands: &mut Vec<GameMessage>,
) {
    let clear_msg = GameMessage::new(GameMessageType::ClearGameData);
    append_game_message_to_the_message_stream(&clear_msg);
    if let Some(sink) = command_sink {
        sink.as_ref()(clear_msg);
    } else {
        pending_commands.push(clear_msg);
    }
}

impl Default for Recorder {
    fn default() -> Self {
        Self::new()
    }
}

impl Recorder {
    /// Constructor
    /// Matches C++ RecorderClass::RecorderClass() from Recorder.cpp:340-355
    pub fn new() -> Self {
        let mut recorder = Self {
            mode: RecorderMode::Record,
            file: None,
            filename: String::new(),
            current_file_position: 0,
            next_frame: 0,
            original_game_mode: 0, // GAME_NONE
            current_replay_filename: String::new(),
            crc_info: None,
            was_desync: false,
            doing_analysis: false,
            game_info: ReplayGameInfo::new(),
            current_frame: 0,
            frame_provider: None,
            game_mode_provider: None,
            command_source: None,
            command_sink: None,
            command_cull: None,
            pending_commands: Vec::new(),
            pending_lobby: None,
            pending_local_index: -1,
            lobby_info_provider: None,
            replay_control_hook: None,
            replay_controls_hidden: true,
            player_name_key_provider: None,
        };

        recorder.init();
        recorder
    }

    /// Initialize the recorder
    /// Matches C++ RecorderClass::init() from Recorder.cpp:370-385
    pub fn init(&mut self) {
        self.original_game_mode = 0; // GAME_NONE
        self.mode = RecorderMode::None;
        self.file = None;
        self.filename.clear();
        self.current_file_position = 0;
        self.game_info.clear_slot_list();
        self.game_info.reset();
        self.pending_commands.clear();

        if let Some(data) = get_global_data() {
            let data = data.read();
            let map = if !data.pending_file.is_empty() {
                data.pending_file.clone()
            } else {
                data.map_name.clone()
            };
            self.game_info.set_map(map);
        } else {
            self.game_info.set_map(String::new());
        }

        self.game_info.set_seed(get_game_logic_random_seed());

        self.was_desync = false;
        self.doing_analysis = false;
    }

    /// Set the current frame (used when no frame provider is configured)
    pub fn set_current_frame(&mut self, frame: u32) {
        self.current_frame = frame;
    }

    /// Provide a frame callback for live frame queries
    pub fn set_frame_provider(&mut self, provider: Option<Arc<dyn Fn() -> u32 + Send + Sync>>) {
        self.frame_provider = provider;
    }

    /// Provide a callback for querying the active game mode.
    pub fn set_game_mode_provider(&mut self, provider: Option<Arc<dyn Fn() -> i32 + Send + Sync>>) {
        self.game_mode_provider = provider;
    }

    /// Provide a command source for recording
    pub fn set_command_source(
        &mut self,
        source: Option<Arc<dyn Fn() -> Vec<GameMessage> + Send + Sync>>,
    ) {
        self.command_source = source;
    }

    /// Provide a command sink for playback
    pub fn set_command_sink(&mut self, sink: Option<Arc<dyn Fn(GameMessage) + Send + Sync>>) {
        self.command_sink = sink;
    }

    /// Provide a command culler callback used during playback
    pub fn set_command_cull(&mut self, cull: Option<Arc<dyn Fn() + Send + Sync>>) {
        self.command_cull = cull;
    }

    /// Snapshot used by the next `start_recording` after `reset()` (C++ TheSkirmishGameInfo / TheLAN).
    pub fn set_recording_lobby(&mut self, info: ReplayGameInfo, local_index: i32) {
        self.pending_lobby = Some(info);
        self.pending_local_index = local_index;
    }

    pub fn set_lobby_info_provider(
        &mut self,
        provider: Option<Arc<dyn Fn() -> Option<(ReplayGameInfo, i32)> + Send + Sync>>,
    ) {
        self.lobby_info_provider = provider;
    }

    pub fn set_replay_control_visibility_hook(
        &mut self,
        hook: Option<Arc<dyn Fn(bool) + Send + Sync>>,
    ) {
        self.replay_control_hook = hook;
    }

    pub fn set_player_name_key_provider(
        &mut self,
        provider: Option<Arc<dyn Fn(i32) -> Option<String> + Send + Sync>>,
    ) {
        self.player_name_key_provider = provider;
    }

    /// Live `MSG_LOGIC_CRC` from GameLogic (GameLogic.cpp:3642-3651 / GameLogicDispatch.cpp:1946).
    /// File-sourced CRCs already enqueued in `append_next_command`; live values compare.
    pub fn notify_logic_crc(&mut self, crc: u32, player_index: i32) {
        self.handle_crc_message(crc, player_index, false);
    }
    pub fn replay_controls_are_hidden(&self) -> bool {
        self.replay_controls_hidden
    }

    pub fn saw_crc_mismatch(&self) -> bool {
        self.crc_info
            .as_ref()
            .map(|info| info.saw_crc_mismatch())
            .unwrap_or(false)
    }

    /// Drain any pending commands captured during playback
    pub fn drain_pending_commands(&mut self) -> Vec<GameMessage> {
        std::mem::take(&mut self.pending_commands)
    }

    fn get_current_frame(&self) -> u32 {
        self.frame_provider
            .as_ref()
            .map(|provider| provider())
            .unwrap_or(self.current_frame)
    }

    fn resolve_local_slot_index(&self) -> i32 {
        let local_ip = self.game_info.get_local_ip();
        if local_ip != 0 {
            for (index, slot) in self.game_info.slots.iter().enumerate() {
                if slot.is_occupied && slot.ip == local_ip {
                    return index as i32;
                }
            }
        }

        if self.game_info.slots.iter().any(|slot| slot.is_occupied) {
            return 0;
        }

        -1
    }

    fn is_network_message(&self, msg: &GameMessage) -> bool {
        is_network_command_message(msg.get_type())
    }

    fn is_current_game_in_game(&self) -> bool {
        // C++ GameLogic::isInGame() is `!(m_gameMode == GAME_NONE)` (GameLogic.h:375).
        // GAME_SHELL is in-game, so playback from the shell map must clear first.
        self.game_mode_provider
            .as_ref()
            .map(|provider| provider() != GAME_NONE)
            .unwrap_or(false)
    }

    /// Reset the recorder to initialized state
    /// Matches C++ RecorderClass::reset() from Recorder.cpp:390-398
    pub fn reset(&mut self) {
        if self.file.is_some() {
            // File will be closed on drop
            self.file = None;
        }
        self.filename.clear();
        self.init();
    }

    /// General update function
    /// Matches C++ RecorderClass::update() from Recorder.cpp:404-410
    pub fn update(&mut self) {
        match self.mode {
            RecorderMode::Record | RecorderMode::None => {
                self.update_record();
            }
            RecorderMode::Playback => {
                self.update_playback();
            }
        }
    }

    /// Update for recording mode
    /// Matches C++ RecorderClass::updateRecord() from Recorder.cpp:455-503
    fn update_record(&mut self) {
        let mut need_flush = false;
        if let Some(source) = &self.command_source {
            for msg in source() {
                match msg.get_type() {
                    GameMessageType::NewGame => {
                        let mode = match msg.get_argument(0) {
                            Some(GameMessageArgumentType::Integer(value)) => *value,
                            _ => GAME_NONE,
                        };

                        if mode != GAME_SHELL && mode != GAME_SINGLE_PLAYER && mode != GAME_NONE {
                            let difficulty = match msg.get_argument(1) {
                                Some(GameMessageArgumentType::Integer(value)) => *value,
                                _ => 0,
                            };
                            let rank_points = match msg.get_argument(2) {
                                Some(GameMessageArgumentType::Integer(value)) => *value,
                                _ => 0,
                            };
                            let max_fps = match msg.get_argument(3) {
                                Some(GameMessageArgumentType::Integer(value)) => *value,
                                _ => 0,
                            };

                            if let Err(err) =
                                self.start_recording(difficulty, mode, rank_points, max_fps)
                            {
                                log::error!("Failed to start replay recording: {}", err);
                            }
                        }
                    }
                    GameMessageType::ClearGameData => {
                        if self.file.is_some() {
                            if let Err(err) = self.write_to_file(&msg) {
                                log::error!("Failed to write replay command: {}", err);
                            }
                            self.stop_recording();
                        }
                        self.filename.clear();
                    }
                    _ => {
                        if self.file.is_some() && self.is_network_message(&msg) {
                            if let Err(err) = self.write_to_file(&msg) {
                                log::error!("Failed to write replay command: {}", err);
                            }
                            need_flush = true;
                        }
                    }
                }
            }
        }

        if need_flush {
            if let Some(file) = self.file.as_mut() {
                if let Err(err) = file.flush() {
                    log::error!("Failed to flush replay file: {}", err);
                }
            }
        }
    }

    /// Update for playback mode
    /// Matches C++ RecorderClass::updatePlayback() from Recorder.cpp:415-432
    fn update_playback(&mut self) {
        // Cull bad commands that user shouldn't execute during playback
        self.cull_bad_commands();

        if self.next_frame == -1 {
            // No more commands to execute
            return;
        }

        // Current frame comes from the frame provider when available
        let cur_frame = if self.doing_analysis {
            self.next_frame as u32
        } else {
            self.get_current_frame()
        };

        // While there are commands to queue for this frame, do it
        while self.next_frame == cur_frame as i32 {
            self.append_next_command();
            self.read_next_frame();
        }
    }

    /// Prevent user from giving commands during playback
    /// Matches C++ RecorderClass::cullBadCommands() from Recorder.cpp:1436-1454
    fn cull_bad_commands(&mut self) {
        if self.doing_analysis {
            return;
        }

        if let Some(cull) = &self.command_cull {
            cull();
        }
        self.pending_commands.retain(keep_command_during_playback);
    }

    /// Append next command from file to command list
    /// Matches C++ RecorderClass::appendNextCommand() from Recorder.cpp:1212-1316
    fn append_next_command(&mut self) {
        let parsed = {
            let Some(file) = self.file.as_mut() else {
                return;
            };

            let mut buf4 = [0u8; 4];
            let mut buf1 = [0u8; 1];

            if file.read_exact(&mut buf4).is_err() {
                None
            } else {
                let message_type_id = u32::from_le_bytes(buf4);
                let player_index = if file.read_exact(&mut buf4).is_err() {
                    None
                } else {
                    Some(i32::from_le_bytes(buf4))
                };
                let num_types = if file.read_exact(&mut buf1).is_err() {
                    None
                } else {
                    Some(buf1[0] as usize)
                };

                match (player_index, num_types) {
                    (Some(player_index), Some(num_types)) => {
                        let mut type_headers: Vec<(GameMessageArgumentDataType, u8)> =
                            Vec::with_capacity(num_types);
                        let mut headers_ok = true;
                        for _ in 0..num_types {
                            if file.read_exact(&mut buf1).is_err() {
                                headers_ok = false;
                                break;
                            }
                            let data_type = match arg_data_type_from_u8(buf1[0]) {
                                Ok(value) => value,
                                Err(err) => {
                                    log::error!("Invalid argument type in replay: {}", err);
                                    headers_ok = false;
                                    break;
                                }
                            };
                            if file.read_exact(&mut buf1).is_err() {
                                headers_ok = false;
                                break;
                            }
                            type_headers.push((data_type, buf1[0]));
                        }
                        if !headers_ok {
                            None
                        } else {
                            let mut args: Vec<GameMessageArgumentType> = Vec::new();
                            let mut args_ok = true;
                            for (data_type, count) in type_headers {
                                for _ in 0..count {
                                    match Self::read_argument(file, data_type) {
                                        Ok(arg) => args.push(arg),
                                        Err(err) => {
                                            log::error!("Failed to read replay argument: {}", err);
                                            args_ok = false;
                                        }
                                    }
                                    if !args_ok {
                                        break;
                                    }
                                }
                                if !args_ok {
                                    break;
                                }
                            }
                            if args_ok {
                                Some((message_type_id, player_index, args))
                            } else {
                                None
                            }
                        }
                    }
                    _ => None,
                }
            }
        };

        let Some((message_type_id, player_index, args)) = parsed else {
            self.next_frame = -1;
            self.stop_playback();
            return;
        };

        let (message_type, consumed) = match decode_replay_type_id(message_type_id, &args) {
            Ok(value) => value,
            Err(err) => {
                log::error!("Failed to decode replay message type: {}", err);
                self.next_frame = -1;
                self.stop_playback();
                return;
            }
        };

        let mut msg = GameMessage::with_player(message_type, player_index);
        for arg in args.into_iter().skip(consumed) {
            self.append_argument_to_message(&mut msg, arg);
        }

        if let GameMessageType::LogicCRC(crc) = msg.get_type() {
            let crc = *crc;
            // File-sourced MSG_LOGIC_CRC is the recorded value (Recorder.cpp:957-964 fromPlayback).
            self.handle_crc_message(crc, player_index, true);
        }
        if matches!(msg.get_type(), &GameMessageType::ClearGameData) {
            return;
        }

        if self.doing_analysis {
            return;
        }

        if let Some(sink) = &self.command_sink {
            sink(msg);
        } else {
            self.pending_commands.push(msg);
        }
    }

    /// Read next frame number from file
    /// Matches C++ RecorderClass::readNextFrame() from Recorder.cpp:1200-1207
    fn read_next_frame(&mut self) {
        let Some(ref mut file) = self.file else {
            self.next_frame = -1;
            return;
        };

        let mut buf4 = [0u8; 4];
        if file.read_exact(&mut buf4).is_err() {
            self.next_frame = -1;
            self.stop_playback();
            return;
        }

        self.next_frame = u32::from_le_bytes(buf4) as i32;
    }

    /// Start recording to file
    /// Matches C++ RecorderClass::startRecording() from Recorder.cpp:509-673
    pub fn start_recording(
        &mut self,
        difficulty: i32,
        original_game_mode: i32,
        rank_points: i32,
        max_fps: i32,
    ) -> Result<(), std::io::Error> {
        if self.file.is_some() {
            log::error!("Starting to record game while game is in progress.");
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Already recording",
            ));
        }
        let captured_lobby = if let Some(provider) = &self.lobby_info_provider {
            provider()
        } else if let Some(info) = self.pending_lobby.take() {
            Some((info, self.pending_local_index))
        } else {
            None
        };

        self.reset();
        self.mode = RecorderMode::Record;
        if let Some((mut lobby, local_idx)) = captured_lobby {
            if lobby.map.is_empty() {
                lobby.map = self.game_info.map.clone();
            }
            if lobby.seed == 0 {
                lobby.seed = self.game_info.seed;
            }
            if lobby.crc_interval == 0 {
                lobby.set_crc_interval(100);
            }
            self.game_info = lobby;
            self.pending_local_index = local_idx;
        } else {
            self.game_info.set_crc_interval(100);
            self.pending_local_index = -1;
        }

        // Get replay directory
        let mut filepath = self.get_replay_dir();

        // Create directory if it doesn't exist
        std::fs::create_dir_all(&filepath)?;

        // Build filename
        self.filename = format!(
            "{}{}",
            self.get_last_replay_filename(),
            self.get_replay_extension()
        );
        filepath.push(&self.filename);

        // Open file for writing
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&filepath)?;

        // Write "GENREP" header (matches Recorder.cpp:529)
        file.write_all(b"GENREP")?;

        // Reserve space for stats (Recorder.cpp:531-549)
        // Start time
        write_fixed_width_time_t(&mut file, 0)?;
        // End time
        write_fixed_width_time_t(&mut file, 0)?;
        // Frame duration
        file.write_all(&0u32.to_le_bytes())?;
        // Desync flag
        file.write_all(&[0u8])?;
        // Quit early flag
        file.write_all(&[0u8])?;
        // Player disconnect flags (MAX_SLOTS = 8)
        for _ in 0..8 {
            file.write_all(&[0u8])?;
        }

        // Write replay name (Recorder.cpp:552-555)
        let replay_name = "LastReplay"; // Would fetch from TheGameText->fetch("GUI:LastReplay")
        self.write_unicode_string(&mut file, replay_name)?;

        // Write system time (Recorder.cpp:557-560)
        write_system_time_to_file(&mut file, Local::now())?;

        // Write version info (Recorder.cpp:562-572)
        let version = get_version();
        let version_string = version.get_unicode_version();
        let version_time_string = version.get_unicode_build_time();
        let version_number: u32 = version.get_version_number();
        let (exe_crc, ini_crc): (u32, u32) = get_global_data()
            .map(|data| {
                let g = data.read();
                (g.exe_crc, g.ini_crc)
            })
            .unwrap_or((0, 0));

        self.write_unicode_string(&mut file, &version_string)?;
        self.write_unicode_string(&mut file, &version_time_string)?;
        file.write_all(&version_number.to_le_bytes())?;
        file.write_all(&exe_crc.to_le_bytes())?;
        file.write_all(&ini_crc.to_le_bytes())?;

        // Write slot list (Recorder.cpp:581-628) using C++ GameInfoToAsciiString layout.
        let slot_list = format_replay_game_info(&self.game_info);
        self.write_ascii_string(&mut file, &slot_list)?;

        // Write local index (LAN IP match / GameSpy slot / skirmish 0 / SP -1).
        let local_index = if self.pending_local_index >= 0 {
            self.pending_local_index
        } else {
            self.resolve_local_slot_index()
        };
        self.write_ascii_string(&mut file, &local_index.to_string())?;

        // Write game difficulty (Recorder.cpp:652-653)
        file.write_all(&difficulty.to_le_bytes())?;

        // Write original game mode (Recorder.cpp:655-656)
        file.write_all(&original_game_mode.to_le_bytes())?;

        // Write rank points (Recorder.cpp:658-659)
        file.write_all(&rank_points.to_le_bytes())?;

        // Write max FPS (Recorder.cpp:661-662)
        file.write_all(&max_fps.to_le_bytes())?;

        file.flush()?;
        self.file = Some(file);
        self.log_game_start(slot_list);
        self.original_game_mode = original_game_mode;

        log::info!("Started recording to {}", filepath.display());
        self.init_controls();
        Ok(())
    }

    /// Stop recording and close file
    /// Matches C++ RecorderClass::stopRecording() from Recorder.cpp:679-695
    pub fn stop_recording(&mut self) {
        self.log_game_end();

        // C++ Recorder.cpp:681-688 — only when TheNetwork && m_wasDesync.
        if self.is_multiplayer() {
            if self.was_desync {
                self.cleanup_replay_file();
            }
            self.was_desync = false;
        }

        if self.file.is_some() {
            // File closed on drop
            self.file = None;
        }
        self.filename.clear();
    }

    /// Write a game message to the recording file
    /// Matches C++ RecorderClass::writeToFile() from Recorder.cpp:700-760
    pub fn write_to_file(&mut self, msg: &GameMessage) -> Result<(), std::io::Error> {
        let frame = self.get_current_frame();
        let Some(ref mut file) = self.file else {
            return Ok(());
        };

        let message_type_id = replay_type_id(msg.get_type())?;

        file.write_all(&frame.to_le_bytes())?;

        file.write_all(&message_type_id.to_le_bytes())?;
        file.write_all(&msg.get_player_index().to_le_bytes())?;

        let mut args = match msg.get_type() {
            GameMessageType::SetReplayCamera(coord, angle, zoom) => {
                set_replay_camera_file_args(msg, coord, *angle, *zoom)
            }
            _ => {
                let mut args = MessageSerializer::encode_message_arguments(msg.get_type());
                for arg in msg.get_arguments() {
                    args.push(arg.data.clone());
                }
                args
            }
        };
        if matches!(msg.get_type(), GameMessageType::LogicCRC(_))
            && !args
                .iter()
                .any(|arg| matches!(arg, GameMessageArgumentType::Boolean(_)))
        {
            // C++ GameLogic.cpp:3644 — second argument is fromPlayback.
            args.push(GameMessageArgumentType::Boolean(false));
        }

        let mut type_groups: Vec<(GameMessageArgumentDataType, u8)> = Vec::new();
        let mut last_type: Option<GameMessageArgumentDataType> = None;

        for arg in &args {
            let arg_type = GameMessageArgumentDataType::from(arg);
            if let Some(last) = last_type {
                if last == arg_type {
                    if let Some((_, count)) = type_groups.last_mut() {
                        if *count == u8::MAX {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::InvalidData,
                                "Too many arguments in replay group",
                            ));
                        }
                        *count += 1;
                    }
                } else {
                    if type_groups.len() == u8::MAX as usize {
                        return Err(std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "Too many argument groups in replay",
                        ));
                    }
                    type_groups.push((arg_type, 1));
                    last_type = Some(arg_type);
                }
            } else {
                type_groups.push((arg_type, 1));
                last_type = Some(arg_type);
            }
        }

        if type_groups.len() > u8::MAX as usize {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Too many argument groups in replay",
            ));
        }
        file.write_all(&[type_groups.len() as u8])?;
        for (data_type, count) in &type_groups {
            file.write_all(&[Self::arg_data_type_to_u8(*data_type)?])?;
            file.write_all(&[*count])?;
        }

        for arg in &args {
            Self::write_argument(file, arg)?;
        }

        file.flush()?;
        Ok(())
    }

    fn write_argument<W: Write>(
        writer: &mut W,
        arg: &GameMessageArgumentType,
    ) -> Result<(), std::io::Error> {
        match arg {
            GameMessageArgumentType::Integer(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::Real(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::Boolean(v) => writer.write_all(&[*v as u8])?,
            GameMessageArgumentType::ObjectID(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::DrawableID(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::TeamID(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::SquadID(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::Location(v) => {
                writer.write_all(&v.x.to_le_bytes())?;
                writer.write_all(&v.y.to_le_bytes())?;
                writer.write_all(&v.z.to_le_bytes())?;
            }
            GameMessageArgumentType::Pixel(v) => {
                writer.write_all(&v.x.to_le_bytes())?;
                writer.write_all(&v.y.to_le_bytes())?;
            }
            GameMessageArgumentType::PixelRegion(v) => {
                // C++ IRegion2D is { ICoord2D lo, hi } (BaseType.h).
                let hi_x = v.x.saturating_add(v.width);
                let hi_y = v.y.saturating_add(v.height);
                writer.write_all(&v.x.to_le_bytes())?;
                writer.write_all(&v.y.to_le_bytes())?;
                writer.write_all(&hi_x.to_le_bytes())?;
                writer.write_all(&hi_y.to_le_bytes())?;
            }
            GameMessageArgumentType::Timestamp(v) => writer.write_all(&v.to_le_bytes())?,
            GameMessageArgumentType::WideChar(v) => {
                let raw = (*v as u32).min(u16::MAX as u32) as u16;
                writer.write_all(&raw.to_le_bytes())?;
            }
            GameMessageArgumentType::String(_) => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "String arguments are not supported in replay stream",
                ));
            }
        }

        Ok(())
    }

    fn read_argument<R: Read>(
        reader: &mut R,
        data_type: GameMessageArgumentDataType,
    ) -> Result<GameMessageArgumentType, std::io::Error> {
        let arg = match data_type {
            GameMessageArgumentDataType::Integer => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::Integer(i32::from_le_bytes(buf))
            }
            GameMessageArgumentDataType::Real => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::Real(f32::from_le_bytes(buf))
            }
            GameMessageArgumentDataType::Boolean => {
                let mut buf = [0u8; 1];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::Boolean(buf[0] != 0)
            }
            GameMessageArgumentDataType::ObjectID => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::ObjectID(u32::from_le_bytes(buf))
            }
            GameMessageArgumentDataType::DrawableID => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::DrawableID(u32::from_le_bytes(buf))
            }
            GameMessageArgumentDataType::TeamID => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::TeamID(u32::from_le_bytes(buf))
            }
            GameMessageArgumentDataType::Location => {
                let mut bx = [0u8; 4];
                let mut by = [0u8; 4];
                let mut bz = [0u8; 4];
                reader.read_exact(&mut bx)?;
                reader.read_exact(&mut by)?;
                reader.read_exact(&mut bz)?;
                GameMessageArgumentType::Location(
                    crate::common::message_stream::game_message::Coord3D {
                        x: f32::from_le_bytes(bx),
                        y: f32::from_le_bytes(by),
                        z: f32::from_le_bytes(bz),
                    },
                )
            }
            GameMessageArgumentDataType::Pixel => {
                let mut bx = [0u8; 4];
                let mut by = [0u8; 4];
                reader.read_exact(&mut bx)?;
                reader.read_exact(&mut by)?;
                GameMessageArgumentType::Pixel(
                    crate::common::message_stream::game_message::ICoord2D {
                        x: i32::from_le_bytes(bx),
                        y: i32::from_le_bytes(by),
                    },
                )
            }
            GameMessageArgumentDataType::PixelRegion => {
                let mut b0 = [0u8; 4];
                let mut b1 = [0u8; 4];
                let mut b2 = [0u8; 4];
                let mut b3 = [0u8; 4];
                reader.read_exact(&mut b0)?;
                reader.read_exact(&mut b1)?;
                reader.read_exact(&mut b2)?;
                reader.read_exact(&mut b3)?;
                GameMessageArgumentType::PixelRegion(
                    crate::common::message_stream::game_message::IRegion2D {
                        x: i32::from_le_bytes(b0),
                        y: i32::from_le_bytes(b1),
                        width: i32::from_le_bytes(b2).saturating_sub(i32::from_le_bytes(b0)),
                        height: i32::from_le_bytes(b3).saturating_sub(i32::from_le_bytes(b1)),
                    },
                )
            }
            GameMessageArgumentDataType::Timestamp => {
                let mut buf = [0u8; 4];
                reader.read_exact(&mut buf)?;
                GameMessageArgumentType::Timestamp(u32::from_le_bytes(buf))
            }
            GameMessageArgumentDataType::WideChar => {
                let mut buf = [0u8; 2];
                reader.read_exact(&mut buf)?;
                let val = u16::from_le_bytes(buf) as u32;
                let ch = std::char::from_u32(val).unwrap_or('\u{FFFD}');
                GameMessageArgumentType::WideChar(ch)
            }
            GameMessageArgumentDataType::String | GameMessageArgumentDataType::Unknown => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid argument type in replay stream",
                ));
            }
        };

        Ok(arg)
    }

    fn append_argument_to_message(&self, msg: &mut GameMessage, arg: GameMessageArgumentType) {
        match arg {
            GameMessageArgumentType::Integer(v) => msg.append_integer_argument(v),
            GameMessageArgumentType::Real(v) => msg.append_real_argument(v),
            GameMessageArgumentType::Boolean(v) => msg.append_boolean_argument(v),
            GameMessageArgumentType::ObjectID(v) => msg.append_object_id_argument(v),
            GameMessageArgumentType::DrawableID(v) => msg.append_drawable_id_argument(v),
            GameMessageArgumentType::TeamID(v) => msg.append_team_id_argument(v),
            GameMessageArgumentType::SquadID(v) => msg.append_team_id_argument(v),
            GameMessageArgumentType::Location(v) => msg.append_location_argument(v),
            GameMessageArgumentType::Pixel(v) => msg.append_pixel_argument(v),
            GameMessageArgumentType::PixelRegion(v) => msg.append_pixel_region_argument(v),
            GameMessageArgumentType::Timestamp(v) => msg.append_timestamp_argument(v),
            GameMessageArgumentType::WideChar(v) => msg.append_wide_char_argument(v),
            GameMessageArgumentType::String(v) => msg.append_string_argument(v),
        }
    }

    /// Start playback from file
    /// Matches C++ RecorderClass::playbackFile() from Recorder.cpp:1029-1138
    pub fn playback_file(&mut self, filename: String) -> Result<bool, std::io::Error> {
        // Adapter parity: stale queued commands must not survive a new playback session.
        self.pending_commands.clear();

        if !self.doing_analysis && self.is_current_game_in_game() {
            // C++ clears live game data only when playback starts from an active game.
            send_clear_game_data(&self.command_sink, &mut self.pending_commands);
        }

        self.mode = RecorderMode::Playback;

        // Read replay header
        let mut header = ReplayHeader::default();
        header.for_playback = true;
        header.filename = filename.clone();

        if !self.read_replay_header(&mut header)? {
            return Ok(false);
        }

        // Validate magic number
        // Already done in read_replay_header

        if let Some(data) = get_global_data() {
            data.write().pending_file = self.game_info.get_map().to_string();
        }

        // Initialize CRC info
        self.crc_info = Some(CrcInfo::new());
        if let Some(ref mut crc) = self.crc_info {
            crc.set_local_player(header.local_player_index as u32);
        }

        let mut difficulty = 0i32;
        let mut rank_points = 0i32;
        let mut max_fps = 0i32;

        if let Some(ref mut file) = self.file {
            let mut buf4 = [0u8; 4];
            file.read_exact(&mut buf4)?;
            difficulty = i32::from_le_bytes(buf4);

            file.read_exact(&mut buf4)?;
            self.original_game_mode = i32::from_le_bytes(buf4);

            file.read_exact(&mut buf4)?;
            rank_points = i32::from_le_bytes(buf4);

            file.read_exact(&mut buf4)?;
            max_fps = i32::from_le_bytes(buf4);
        }

        // Read next frame
        self.read_next_frame();

        // C++ parity: playback header controls replay CRC cadence.
        crate::common::crc_debug::set_replay_crc_interval(self.game_info.get_crc_interval() as i32);

        // Send MSG_NEW_GAME message
        if !self.doing_analysis {
            const GAME_REPLAY: i32 = 3;
            let mut msg = GameMessage::new(GameMessageType::NewGame);
            msg.append_integer_argument(GAME_REPLAY);
            msg.append_integer_argument(difficulty);
            msg.append_integer_argument(rank_points);
            if max_fps != 0 {
                msg.append_integer_argument(max_fps);
            }
            // C++ Recorder.cpp:1123-1134 TheMessageStream->appendMessage(MSG_NEW_GAME).
            append_game_message_to_the_message_stream(&msg);
            if let Some(sink) = &self.command_sink {
                sink(msg);
            } else {
                self.pending_commands.push(msg);
            }

            init_game_logic_random(self.game_info.get_seed());
            init_random_with_seed(self.game_info.get_seed());
        }

        self.current_replay_filename = filename;
        self.init_controls();
        Ok(true)
    }

    /// Load replay header from file without entering playback mode.
    pub fn load_replay_header(&mut self, filename: String) -> Result<ReplayHeader, std::io::Error> {
        let mut header = ReplayHeader::default();
        header.for_playback = false;
        header.filename = filename;

        if !self.read_replay_header(&mut header)? {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "Invalid replay header",
            ));
        }

        Ok(header)
    }

    /// Start replay analysis mode (C++ `analyzeReplay`).
    pub fn analyze_replay(&mut self, filename: String) -> Result<bool, std::io::Error> {
        self.doing_analysis = true;
        self.playback_file(filename)
    }

    /// Check whether replay analysis is still running (C++ `isAnalysisInProgress`).
    pub fn is_analysis_in_progress(&self) -> bool {
        self.mode == RecorderMode::Playback && self.next_frame != -1
    }

    /// Read replay header from file
    /// Matches C++ RecorderClass::readReplayHeader() from Recorder.cpp:792-878
    fn read_replay_header(&mut self, header: &mut ReplayHeader) -> Result<bool, std::io::Error> {
        let filepath = self.get_replay_dir().join(&header.filename);
        let mut file = File::open(&filepath)?;

        // Read GENREP magic (Recorder.cpp:804-812)
        let mut magic = [0u8; 6];
        file.read_exact(&mut magic)?;
        if &magic != b"GENREP" {
            log::error!("Replay file did not have GENREP at start");
            return Ok(false);
        }

        // Read stats (Recorder.cpp:814-825)
        let mut buf4 = [0u8; 4];
        let mut buf1 = [0u8; 1];

        header.start_time = read_fixed_width_time_t(&mut file)? as u64;
        header.end_time = read_fixed_width_time_t(&mut file)? as u64;

        file.read_exact(&mut buf4)?;
        header.frame_duration = u32::from_le_bytes(buf4);

        file.read_exact(&mut buf1)?;
        header.desync_game = buf1[0] != 0;

        file.read_exact(&mut buf1)?;
        header.quit_early = buf1[0] != 0;

        for i in 0..8 {
            file.read_exact(&mut buf1)?;
            header.player_discons[i] = buf1[0] != 0;
        }

        // Read replay name (Recorder.cpp:828)
        header.replay_name = self.read_unicode_string(&mut file)?;

        // Read date/time (Recorder.cpp:831)
        header.time_val = read_system_time_from_file(&mut file)?;

        // Read version info (Recorder.cpp:833-838)
        header.version_string = self.read_unicode_string(&mut file)?;
        header.version_time_string = self.read_unicode_string(&mut file)?;

        file.read_exact(&mut buf4)?;
        header.version_number = u32::from_le_bytes(buf4);

        file.read_exact(&mut buf4)?;
        header.exe_crc = u32::from_le_bytes(buf4);

        file.read_exact(&mut buf4)?;
        header.ini_crc = u32::from_le_bytes(buf4);

        // Read game info (Recorder.cpp:841-852)
        header.game_options = self.read_ascii_string(&mut file)?;
        self.game_info.reset();
        self.game_info.enter_game();

        if header.game_options.is_empty() {
            // Legacy rust files wrote an empty stub string; keep SP empty slots.
        } else if !parse_replay_game_info(&header.game_options, &mut self.game_info) {
            log::error!("Failed to parse replay game options");
            self.game_info.end_game();
            self.game_info.reset();
            return Ok(false);
        }
        self.game_info.start_game(0);

        // Read player index (Recorder.cpp:854-869)
        let player_index_str = self.read_ascii_string(&mut file)?;
        header.local_player_index = player_index_str.parse().unwrap_or(-1);

        if header.local_player_index < -1 || header.local_player_index >= 8 {
            log::error!("Invalid local slot number");
            self.game_info.end_game();
            self.game_info.reset();
            return Ok(false);
        }

        if header.local_player_index >= 0 {
            if let Some(slot) = self.game_info.get_slot(header.local_player_index as usize) {
                let local_ip = slot.get_ip();
                self.game_info.set_local_ip(local_ip);
            }
        }

        // If not for playback, cleanup and close
        if !header.for_playback {
            self.game_info.end_game();
            self.game_info.reset();
            return Ok(true);
        }

        self.current_file_position = file.seek(SeekFrom::Current(0))?;
        self.file = Some(file);
        Ok(true)
    }

    /// Stop playback
    /// Matches C++ RecorderClass::stopPlayback() from Recorder.cpp:438-449
    pub fn stop_playback(&mut self) {
        if self.file.is_some() {
            self.file = None;
        }
        self.filename.clear();

        if !self.doing_analysis {
            send_clear_game_data(&self.command_sink, &mut self.pending_commands);
        }
    }

    /// Test if replay version matches current version
    /// Matches C++ RecorderClass::testVersionPlayback() from Recorder.cpp:999-1023
    pub fn test_version_playback(&mut self, filename: String) -> Result<bool, std::io::Error> {
        let mut header = ReplayHeader::default();
        header.for_playback = true;
        header.filename = filename;

        if !self.read_replay_header(&mut header)? {
            return Ok(false);
        }

        let version = get_version();
        let version_string_diff = header.version_string != version.get_unicode_version();
        let version_time_string_diff =
            header.version_time_string != version.get_unicode_build_time();
        let version_number_diff = header.version_number != version.get_version_number();
        let (exe_crc, ini_crc): (u32, u32) = get_global_data()
            .map(|data| {
                let g = data.read();
                (g.exe_crc, g.ini_crc)
            })
            .unwrap_or((0, 0));
        let exe_different = version_string_diff
            || version_time_string_diff
            || version_number_diff
            || header.exe_crc != exe_crc;
        let ini_different = header.ini_crc != ini_crc;

        Ok(exe_different || ini_different)
    }

    /// Log game start
    /// Matches C++ RecorderClass::logGameStart() from Recorder.cpp:43-97
    pub fn log_game_start(&mut self, _options: String) {
        if self.file.is_none() {
            return;
        }

        let start_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .min(u32::MAX as u64) as u32;

        // Seek to start time offset (Recorder.cpp:49-55)
        // const startTimeOffset = 6
        if let Some(ref mut file) = self.file {
            let _ = file.seek(SeekFrom::Start(REPLAY_STATS_OFFSET));
            let _ = write_fixed_width_time_t(file, start_time);
            // Seek back to end
            let _ = file.seek(SeekFrom::End(0));
            let _ = file.flush();
        }

        // Would also write to stats file in DEBUG/INTERNAL builds
    }

    /// Log game end
    /// Matches C++ RecorderClass::logGameEnd() from Recorder.cpp:195-250
    fn log_game_end(&mut self) {
        if self.file.is_none() {
            return;
        }

        let end_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .min(u32::MAX as u64) as u32;

        let duration: u32 = self.get_current_frame();

        // Seek and write end time (offset 6 + 4 = 10)
        if let Some(ref mut file) = self.file {
            let _ = file.seek(SeekFrom::Start(REPLAY_END_TIME_OFFSET));
            let _ = write_fixed_width_time_t(file, end_time);

            // Write frame duration (offset 10 + 4 = 14)
            let _ = file.seek(SeekFrom::Start(REPLAY_FRAME_DURATION_OFFSET));
            let _ = file.write_all(&duration.to_le_bytes());

            // Seek back to end
            let _ = file.seek(SeekFrom::End(0));
            let _ = file.flush();
        }

        // Would also write to stats file in DEBUG/INTERNAL builds
    }

    /// Log player disconnect
    /// Matches C++ RecorderClass::logPlayerDisconnect() from Recorder.cpp:99-147
    pub fn log_player_disconnect(&mut self, _player: &str, slot: i32) {
        if self.file.is_none() {
            return;
        }

        if slot < 0 || slot >= 8 {
            log::error!("Attempting to disconnect invalid slot {}", slot);
            return;
        }

        // Seek to disconnect offset (6 + 4 + 4 + 4 + 1 + 1 = 20, then + slot)
        let offset = REPLAY_PLAYER_DISCONNECTS_OFFSET + slot as u64;
        if let Some(ref mut file) = self.file {
            let _ = file.seek(SeekFrom::Start(offset));
            let _ = file.write_all(&[1u8]); // true
            let _ = file.seek(SeekFrom::End(0));
            let _ = file.flush();
        }
    }

    /// Log CRC mismatch (desync)
    /// Matches C++ RecorderClass::logCRCMismatch() from Recorder.cpp:149-193
    pub fn log_crc_mismatch(&mut self) {
        if self.file.is_none() {
            return;
        }

        // Seek to desync offset (6 + 4 + 4 + 4 = 18)
        if let Some(ref mut file) = self.file {
            let _ = file.seek(SeekFrom::Start(REPLAY_DESYNC_OFFSET));
            let _ = file.write_all(&[1u8]); // true
            let _ = file.seek(SeekFrom::End(0));
            let _ = file.flush();
        }

        // C++ Recorder.cpp:169-172 — m_wasDesync only in DEBUG/INTERNAL + m_saveStats.
        #[cfg(debug_assertions)]
        {
            if get_global_data()
                .map(|data| data.read().save_stats)
                .unwrap_or(false)
            {
                self.was_desync = true;
            }
        }
        log::error!("CRC mismatch recorded");
    }

    /// Handle CRC message for validation
    /// Matches C++ RecorderClass::handleCRCMessage() from Recorder.cpp:957-994
    pub fn handle_crc_message(&mut self, new_crc: u32, player_index: i32, from_playback: bool) {
        let frame = self.get_current_frame();
        let crc_info = match &mut self.crc_info {
            Some(info) => info,
            None => return,
        };

        if from_playback {
            crc_info.add_crc(new_crc);
            return;
        }

        let local_player_index = crc_info.get_local_player() as i32;
        // C++ Recorder.cpp:968-972 — same player if !p or name key == "player{localSlot}".
        let expected_key = format!("player{}", local_player_index);
        let same_player = self
            .player_name_key_provider
            .as_ref()
            .and_then(|provider| provider(player_index))
            .map(|key| key == expected_key)
            .unwrap_or(true);
        if same_player || local_player_index < 0 {
            let playback_crc = crc_info.read_crc();

            if frame > 0 && new_crc != playback_crc && !crc_info.saw_crc_mismatch() {
                crc_info.set_saw_crc_mismatch();
                log::error!(
                    "Replay desync at frame {}: recorded 0x{:08X}, current 0x{:08X}",
                    frame,
                    playback_crc,
                    new_crc
                );
            }
        }
    }

    /// Cleanup replay file after desync
    /// Matches C++ RecorderClass::cleanUpReplayFile() from Recorder.cpp:265-330
    fn cleanup_replay_file(&mut self) {
        // C++ Recorder.cpp:265-330 — DEBUG/INTERNAL + m_saveStats copies the
        // .rep into m_baseStatsDir. It never deletes the original.
        #[cfg(debug_assertions)]
        {
            if self.filename.is_empty() {
                return;
            }
            let Some(global) = get_global_data() else {
                return;
            };
            let data = global.read();
            if !data.save_stats || data.base_stats_dir.trim().is_empty() {
                return;
            }
            let src = self.get_replay_dir().join(&self.filename);
            if !src.exists() {
                return;
            }
            let dest = PathBuf::from(data.base_stats_dir.trim()).join(&self.filename);
            if let Some(parent) = dest.parent() {
                let _ = std::fs::create_dir_all(parent);
            }
            if let Err(err) = std::fs::copy(&src, &dest) {
                log::warn!(
                    "Failed to copy replay to stats dir {}: {}",
                    dest.display(),
                    err
                );
            }
        }
    }

    /// Get current recorder mode
    /// Matches C++ RecorderClass::getMode() from Recorder.h:83
    pub fn get_mode(&self) -> RecorderMode {
        self.mode
    }

    /// Check if recorder is in playback mode
    pub fn is_playback(&self) -> bool {
        self.mode == RecorderMode::Playback
    }

    /// Check if recorder is recording
    pub fn is_recording(&self) -> bool {
        self.mode == RecorderMode::Record
    }

    /// Get replay directory path
    /// Matches C++ RecorderClass::getReplayDir() from Recorder.cpp:1459-1466
    fn get_replay_dir(&self) -> PathBuf {
        let base = get_global_data()
            .map(|data| data.read().get_path_user_data().to_string())
            .filter(|path| !path.trim().is_empty());

        let mut path = if let Some(base) = base {
            PathBuf::from(base)
        } else {
            let mut exe = std::env::current_exe().unwrap_or_default();
            exe.pop();
            exe
        };
        path.push("Replays");
        path
    }

    /// Get replay file extension
    /// Matches C++ RecorderClass::getReplayExtention() from Recorder.cpp:1471-1473
    fn get_replay_extension(&self) -> &str {
        ".rep" // From replayExtention const in Recorder.cpp:32
    }

    /// Get default replay filename
    /// Matches C++ RecorderClass::getLastReplayFileName() from Recorder.cpp:1478-1542
    fn get_last_replay_filename(&self) -> &str {
        // In DEBUG/INTERNAL with network, generates descriptive name
        // Otherwise returns "00000000" (Recorder.cpp:33)
        "00000000"
    }

    pub fn replay_dir(&self) -> PathBuf {
        self.get_replay_dir()
    }

    pub fn replay_extension(&self) -> &str {
        self.get_replay_extension()
    }

    pub fn last_replay_filename(&self) -> &str {
        self.get_last_replay_filename()
    }

    /// Initialize replay controls UI
    /// Matches C++ RecorderClass::initControls() from Recorder.cpp:1552-1562
    pub fn init_controls(&mut self) {
        // C++: winHide(getMode() != PLAYBACK) on ReplayControl.wnd:ParentReplayControl
        let hide = self.mode != RecorderMode::Playback;
        self.replay_controls_hidden = hide;
        if let Some(hook) = &self.replay_control_hook {
            hook(hide);
        }
    }

    /// Check if this is a multiplayer game
    /// Matches C++ RecorderClass::isMultiplayer() from Recorder.cpp:1565-1588
    pub fn is_multiplayer(&self) -> bool {
        if self.mode == RecorderMode::Playback {
            // Check if any slots are occupied
            for slot in &self.game_info.slots {
                if slot.is_occupied() {
                    return true;
                }
            }
        }

        let mode = self
            .game_mode_provider
            .as_ref()
            .map(|provider| provider())
            .unwrap_or(self.original_game_mode);

        if mode == GAME_SINGLE_PLAYER || mode == GAME_SHELL {
            return false;
        }

        mode != GAME_NONE
    }

    /// Get original game mode
    pub fn get_game_mode(&self) -> i32 {
        self.original_game_mode
    }

    /// Get game info for playback
    pub fn get_game_info(&self) -> &ReplayGameInfo {
        &self.game_info
    }

    /// Get current replay filename (playback only)
    pub fn get_current_replay_filename(&self) -> &str {
        &self.current_replay_filename
    }

    // Helper methods for string I/O
    fn arg_data_type_to_u8(data_type: GameMessageArgumentDataType) -> Result<u8, std::io::Error> {
        let value = match data_type {
            GameMessageArgumentDataType::Integer => 0,
            GameMessageArgumentDataType::Real => 1,
            GameMessageArgumentDataType::Boolean => 2,
            GameMessageArgumentDataType::ObjectID => 3,
            GameMessageArgumentDataType::DrawableID => 4,
            GameMessageArgumentDataType::TeamID => 5,
            GameMessageArgumentDataType::Location => 6,
            GameMessageArgumentDataType::Pixel => 7,
            GameMessageArgumentDataType::PixelRegion => 8,
            GameMessageArgumentDataType::Timestamp => 9,
            GameMessageArgumentDataType::WideChar => 10,
            GameMessageArgumentDataType::String | GameMessageArgumentDataType::Unknown => {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "Invalid argument type for replay stream",
                ));
            }
        };

        Ok(value)
    }

    fn write_unicode_string<W: Write>(
        &self,
        writer: &mut W,
        s: &str,
    ) -> Result<(), std::io::Error> {
        // Write UTF-16 encoded string with null terminator
        for ch in s.encode_utf16().chain(std::iter::once(0)) {
            writer.write_all(&ch.to_le_bytes())?;
        }
        Ok(())
    }

    fn write_ascii_string<W: Write>(&self, writer: &mut W, s: &str) -> Result<(), std::io::Error> {
        writer.write_all(s.as_bytes())?;
        writer.write_all(&[0])?; // null terminator
        Ok(())
    }

    fn read_unicode_string<R: Read>(&self, reader: &mut R) -> Result<String, std::io::Error> {
        let mut buf = [0u8; 2];
        let mut chars = Vec::new();

        loop {
            reader.read_exact(&mut buf)?;
            let ch = u16::from_le_bytes(buf);
            if ch == 0 {
                break;
            }
            chars.push(ch);
        }

        String::from_utf16(&chars)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }

    fn read_ascii_string<R: Read>(&self, reader: &mut R) -> Result<String, std::io::Error> {
        let mut bytes = Vec::new();
        let mut buf = [0u8; 1];

        loop {
            reader.read_exact(&mut buf)?;
            if buf[0] == 0 {
                break;
            }
            bytes.push(buf[0]);
        }

        String::from_utf8(bytes)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))
    }
}

fn arg_data_type_from_u8(value: u8) -> Result<GameMessageArgumentDataType, std::io::Error> {
    match value {
        0 => Ok(GameMessageArgumentDataType::Integer),
        1 => Ok(GameMessageArgumentDataType::Real),
        2 => Ok(GameMessageArgumentDataType::Boolean),
        3 => Ok(GameMessageArgumentDataType::ObjectID),
        4 => Ok(GameMessageArgumentDataType::DrawableID),
        5 => Ok(GameMessageArgumentDataType::TeamID),
        6 => Ok(GameMessageArgumentDataType::Location),
        7 => Ok(GameMessageArgumentDataType::Pixel),
        8 => Ok(GameMessageArgumentDataType::PixelRegion),
        9 => Ok(GameMessageArgumentDataType::Timestamp),
        10 => Ok(GameMessageArgumentDataType::WideChar),
        _ => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Invalid argument type code",
        )),
    }
}

/// Global recorder instance
static GLOBAL_RECORDER: OnceCell<Mutex<Recorder>> = OnceCell::new();

/// Initialize and obtain the global recorder instance
/// Matches C++ TheRecorder and createRecorder() from Recorder.cpp:335-336, 1593-1595
pub fn init_recorder() -> &'static Mutex<Recorder> {
    GLOBAL_RECORDER.get_or_init(|| {
        log::info!("Recorder initialized");
        Mutex::new(Recorder::new())
    })
}

/// Get the global recorder (creates if needed)
pub fn get_recorder() -> &'static Mutex<Recorder> {
    init_recorder()
}

/// Execute a closure with mutable access to the global recorder
pub fn with_recorder_mut<R>(f: impl FnOnce(&mut Recorder) -> R) -> Option<R> {
    GLOBAL_RECORDER
        .get()
        .and_then(|recorder| recorder.lock().ok().map(|mut guard| f(&mut *guard)))
}

/// Execute a closure with shared access to the global recorder
pub fn with_recorder<R>(f: impl FnOnce(&Recorder) -> R) -> Option<R> {
    GLOBAL_RECORDER
        .get()
        .and_then(|recorder| recorder.lock().ok().map(|guard| f(&*guard)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn read_utf16_z_end(bytes: &[u8], mut offset: usize) -> usize {
        loop {
            assert!(
                offset + 1 < bytes.len(),
                "Malformed replay header while reading UTF-16 string"
            );
            let code_unit = u16::from_le_bytes([bytes[offset], bytes[offset + 1]]);
            offset += 2;
            if code_unit == 0 {
                return offset;
            }
        }
    }

    fn replay_version_offsets(bytes: &[u8]) -> (usize, usize, usize, usize, usize) {
        // Magic + fixed replay stats block
        let mut offset = _REPLAY_FIXED_HEADER_SIZE;
        // Replay name
        offset = read_utf16_z_end(bytes, offset);
        // Timestamp
        offset += REPLAY_SYSTEM_TIME_BYTES;
        let version_string_start = offset;
        let version_string_end = read_utf16_z_end(bytes, offset);
        let version_time_start = version_string_end;
        let version_time_end = read_utf16_z_end(bytes, version_time_start);
        let version_number_offset = version_time_end;
        (
            version_string_start,
            version_string_end,
            version_time_start,
            version_number_offset,
            version_number_offset + 8, // ini CRC offset
        )
    }

    fn mutate_utf16_first_code_unit(bytes: &mut [u8], start: usize, end: usize, field_name: &str) {
        // Non-empty UTF-16 z-string has at least one code unit plus terminator.
        assert!(
            end >= start + 4,
            "Replay {field_name} field is unexpectedly empty"
        );
        let current = u16::from_le_bytes([bytes[start], bytes[start + 1]]);
        let next = current.wrapping_add(1).max(1);
        bytes[start..start + 2].copy_from_slice(&next.to_le_bytes());
    }

    fn write_variant(
        base_path: &Path,
        replays_dir: &Path,
        variant_name: &str,
        mutate: impl FnOnce(&mut Vec<u8>),
    ) -> PathBuf {
        let mut bytes = std::fs::read(base_path).expect("base replay should be readable");
        mutate(&mut bytes);
        let variant_path = replays_dir.join(variant_name);
        std::fs::write(&variant_path, bytes).expect("variant replay should be writable");
        variant_path
    }

    #[test]
    fn test_recorder_init() {
        let recorder = Recorder::new();
        assert_eq!(recorder.get_mode(), RecorderMode::None);
        assert!(!recorder.is_recording());
        assert!(!recorder.is_playback());
    }

    #[test]
    fn test_recorder_modes() {
        let mut recorder = Recorder::new();

        // Test mode changes
        recorder.mode = RecorderMode::Record;
        assert!(recorder.is_recording());
        assert!(!recorder.is_playback());

        recorder.mode = RecorderMode::Playback;
        assert!(!recorder.is_recording());
        assert!(recorder.is_playback());

        recorder.mode = RecorderMode::None;
        assert!(!recorder.is_recording());
        assert!(!recorder.is_playback());
    }

    #[test]
    fn test_crc_info() {
        let mut crc = CrcInfo::new();
        assert_eq!(crc.get_local_player(), u32::MAX);
        assert!(!crc.saw_crc_mismatch());

        crc.set_local_player(2);
        assert_eq!(crc.get_local_player(), 2);

        crc.add_crc(0x12345678);
        crc.add_crc(0xABCDEF00);

        assert_eq!(crc.read_crc(), 0x12345678);
        assert_eq!(crc.read_crc(), 0xABCDEF00);
        assert_eq!(crc.read_crc(), 0); // Empty returns 0
    }

    #[test]
    fn test_replay_game_info() {
        let mut info = ReplayGameInfo::new();

        assert_eq!(info.get_map(), "");
        assert_eq!(info.get_seed(), 0);
        assert_eq!(info.get_crc_interval(), 100);

        info.set_map("Maps/Tournament Desert".to_string());
        info.set_seed(12345);
        info.set_crc_interval(200);

        assert_eq!(info.get_map(), "Maps/Tournament Desert");
        assert_eq!(info.get_seed(), 12345);
        assert_eq!(info.get_crc_interval(), 200);
    }

    #[test]
    fn test_is_multiplayer_uses_game_mode_provider() {
        let mut recorder = Recorder::new();

        recorder.set_game_mode_provider(Some(Arc::new(|| GAME_SINGLE_PLAYER)));
        assert!(!recorder.is_multiplayer());

        recorder.set_game_mode_provider(Some(Arc::new(|| GAME_SHELL)));
        assert!(!recorder.is_multiplayer());

        recorder.set_game_mode_provider(Some(Arc::new(|| 2)));
        assert!(recorder.is_multiplayer());

        recorder.set_game_mode_provider(Some(Arc::new(|| GAME_NONE)));
        assert!(!recorder.is_multiplayer());
    }

    #[test]
    fn test_local_slot_index_resolution_matches_replay_expectations() {
        let mut recorder = Recorder::new();

        recorder.game_info.slots[3].is_occupied = true;
        recorder.game_info.slots[3].ip = 0x0102_0304;
        recorder.game_info.set_local_ip(0x0102_0304);
        assert_eq!(recorder.resolve_local_slot_index(), 3);

        recorder.game_info.set_local_ip(0);
        assert_eq!(recorder.resolve_local_slot_index(), 0);

        recorder.game_info.clear_slot_list();
        assert_eq!(recorder.resolve_local_slot_index(), -1);
    }

    #[test]
    fn test_replay_dir_and_playback_updates_pending_map_from_header() {
        let temp = tempfile::tempdir().unwrap();
        let map_name = "Maps/TestPlayback.map".to_string();
        let expected_seed = 0x1357_9BDF;
        let captured_types = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = map_name.clone();
            data.pending_file.clear();
        }

        init_game_logic_random(expected_seed);

        let mut writer = Recorder::new();
        assert_eq!(writer.replay_dir(), temp.path().join("Replays"));

        writer.start_recording(1, 2, 3, 60).unwrap();

        writer.set_current_frame(3);
        let crc_message = GameMessage::new(GameMessageType::LogicCRC(0xDEADBEEF));
        writer.write_to_file(&crc_message).unwrap();

        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        assert!(temp.path().join("Replays").join(&replay_name).exists());

        if let Some(global) = get_global_data() {
            global.write().pending_file = "OldPending.map".to_string();
        }

        let mut reader = Recorder::new();
        reader.set_game_mode_provider(Some(Arc::new(|| 2)));
        let sink_types = captured_types.clone();
        reader.set_command_sink(Some(std::sync::Arc::new(move |msg| {
            sink_types.lock().unwrap().push(msg.get_type().clone());
        })));
        assert!(reader.playback_file(replay_name).unwrap());
        assert_eq!(get_game_logic_random_seed(), expected_seed);

        let pending = get_global_data()
            .map(|global| global.read().pending_file.clone())
            .unwrap_or_default();
        assert_eq!(pending, map_name);
        assert_eq!(
            captured_types.lock().unwrap().as_slice(),
            &[GameMessageType::ClearGameData, GameMessageType::NewGame,]
        );

        reader.stop_playback();
        assert_eq!(
            captured_types.lock().unwrap().as_slice(),
            &[
                GameMessageType::ClearGameData,
                GameMessageType::NewGame,
                GameMessageType::ClearGameData,
            ]
        );
    }

    #[test]
    fn test_playback_file_clears_shell_map_like_cpp_is_in_game() {
        // C++ GameLogic.h:375 isInGame() is true for GAME_SHELL.
        let temp = tempfile::tempdir().unwrap();

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/PlaybackShellClear.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(2);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x0BAD_F00D)))
            .unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let captured_types = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut reader = Recorder::new();
        reader.set_game_mode_provider(Some(Arc::new(|| GAME_SHELL)));
        let sink_types = captured_types.clone();
        reader.set_command_sink(Some(std::sync::Arc::new(move |msg| {
            sink_types.lock().unwrap().push(msg.get_type().clone());
        })));

        assert!(reader.playback_file(replay_name).unwrap());
        assert_eq!(
            captured_types.lock().unwrap().as_slice(),
            &[GameMessageType::ClearGameData, GameMessageType::NewGame,]
        );
    }

    #[test]
    fn test_playback_file_skips_clear_game_data_when_not_in_game() {
        let temp = tempfile::tempdir().unwrap();

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/PlaybackNoClear.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(2);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x0BAD_F00D)))
            .unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let captured_types = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut reader = Recorder::new();
        reader.set_game_mode_provider(Some(Arc::new(|| GAME_NONE)));
        let sink_types = captured_types.clone();
        reader.set_command_sink(Some(std::sync::Arc::new(move |msg| {
            sink_types.lock().unwrap().push(msg.get_type().clone());
        })));

        assert!(reader.playback_file(replay_name).unwrap());
        assert_eq!(
            captured_types.lock().unwrap().as_slice(),
            &[GameMessageType::NewGame,]
        );
    }

    #[test]
    fn test_replay_header_round_trips_fixed_width_time_layout() {
        let temp = tempfile::tempdir().unwrap();
        let map_name = "Maps/HeaderParity.map".to_string();

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = map_name.clone();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(11);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x1234_5678)))
            .unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let bytes = std::fs::read(temp.path().join("Replays").join(&replay_name))
            .expect("replay should be readable");

        assert_eq!(&bytes[..6], b"GENREP");

        let start_time = u32::from_le_bytes(
            bytes[6..10]
                .try_into()
                .expect("start time should be 4 bytes"),
        );
        let end_time = u32::from_le_bytes(
            bytes[10..14]
                .try_into()
                .expect("end time should be 4 bytes"),
        );
        let frame_duration = u32::from_le_bytes(
            bytes[14..18]
                .try_into()
                .expect("frame duration should be 4 bytes"),
        );
        assert!(start_time > 0);
        assert!(end_time >= start_time);
        assert_eq!(frame_duration, 11);

        let mut name_offset = _REPLAY_FIXED_HEADER_SIZE;
        name_offset = read_utf16_z_end(&bytes, name_offset);
        let time_bytes = &bytes[name_offset..name_offset + REPLAY_SYSTEM_TIME_BYTES];
        let year = u16::from_le_bytes([time_bytes[0], time_bytes[1]]);
        let month = u16::from_le_bytes([time_bytes[2], time_bytes[3]]);
        let day_of_week = u16::from_le_bytes([time_bytes[4], time_bytes[5]]);
        let day = u16::from_le_bytes([time_bytes[6], time_bytes[7]]);
        let hour = u16::from_le_bytes([time_bytes[8], time_bytes[9]]);
        let minute = u16::from_le_bytes([time_bytes[10], time_bytes[11]]);
        let second = u16::from_le_bytes([time_bytes[12], time_bytes[13]]);
        let milliseconds = u16::from_le_bytes([time_bytes[14], time_bytes[15]]);

        assert!((1980..=2100).contains(&year));
        assert!((1..=12).contains(&month));
        assert!((0..=6).contains(&day_of_week));
        assert!((1..=31).contains(&day));
        assert!(hour <= 23);
        assert!(minute <= 59);
        assert!(second <= 59);
        assert!(milliseconds <= 999);

        let mut reader = Recorder::new();
        let header = reader
            .load_replay_header(replay_name)
            .expect("header should parse");
        let version = get_version();
        assert_eq!(header.replay_name, "LastReplay");
        assert_eq!(header.start_time, start_time as u64);
        assert_eq!(header.end_time, end_time as u64);
        assert_eq!(header.version_string, version.get_unicode_version());
        assert_eq!(header.version_time_string, version.get_unicode_build_time());
        assert!(header.time_val > UNIX_EPOCH);
    }

    #[test]
    fn test_recorder_init_prefers_pending_file_over_map_name() {
        let snapshot = get_global_data().map(|global| global.read().clone());
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.pending_file = "Maps/PendingOverride.map".to_string();
            data.map_name = "Maps/MapNameFallback.map".to_string();
        }

        let mut recorder = Recorder::new();
        recorder.init();

        assert_eq!(
            recorder.get_game_info().get_map(),
            "Maps/PendingOverride.map"
        );

        if let (Some(global), Some(snapshot)) = (get_global_data(), snapshot) {
            *global.write() = snapshot;
        }
    }

    #[test]
    fn test_analyze_replay_sets_analysis_mode_and_progress_state() {
        let temp = tempfile::tempdir().unwrap();

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/AnalysisReplay.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(7);
        let crc_message = GameMessage::new(GameMessageType::LogicCRC(0xABCD1234));
        writer.write_to_file(&crc_message).unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let mut analyzer = Recorder::new();
        assert!(!analyzer.is_analysis_in_progress());
        assert!(analyzer.analyze_replay(replay_name).unwrap());
        assert!(analyzer.doing_analysis);
        assert!(analyzer.is_analysis_in_progress());

        analyzer.update();
        assert!(!analyzer.is_analysis_in_progress());
    }

    #[test]
    fn test_analyze_replay_suppresses_clear_game_data_messages() {
        let temp = tempfile::tempdir().unwrap();

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/AnalysisClearReplay.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(4);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0xFACE_B00C)))
            .unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        let captured_types = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let mut analyzer = Recorder::new();
        let sink_types = captured_types.clone();
        analyzer.set_command_sink(Some(std::sync::Arc::new(move |msg| {
            sink_types.lock().unwrap().push(msg.get_type().clone());
        })));

        assert!(analyzer.analyze_replay(replay_name).unwrap());
        analyzer.stop_playback();

        assert!(captured_types.lock().unwrap().is_empty());
    }

    #[test]
    fn test_version_playback_detects_header_mismatch_matrix() {
        let temp = tempfile::tempdir().unwrap();

        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/VersionMatrixReplay.map".to_string();
            data.pending_file.clear();
            data.exe_crc = 0x1122_3344;
            data.ini_crc = 0x5566_7788;
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(1);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0xA1B2C3D4)))
            .unwrap();
        writer.stop_recording();

        let base_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let replays_dir = temp.path().join("Replays");
        let base_path = replays_dir.join(&base_name);
        assert!(base_path.exists());

        let (
            version_string_start,
            version_string_end,
            version_time_start,
            version_number_offset,
            ini_crc_offset,
        ) = replay_version_offsets(
            &std::fs::read(&base_path).expect("base replay should be readable for offset parsing"),
        );
        let exe_crc_offset = version_number_offset + 4;

        // Baseline: no differences => FALSE (matches C++).
        assert!(
            !Recorder::new()
                .test_version_playback(base_name.clone())
                .unwrap()
        );

        let ext = writer.replay_extension();
        let version_string_name = format!("version_string_diff{ext}");
        write_variant(&base_path, &replays_dir, &version_string_name, |bytes| {
            mutate_utf16_first_code_unit(
                bytes,
                version_string_start,
                version_string_end,
                "version string",
            );
        });
        assert!(
            Recorder::new()
                .test_version_playback(version_string_name)
                .unwrap()
        );

        let version_time_name = format!("version_time_diff{ext}");
        write_variant(&base_path, &replays_dir, &version_time_name, |bytes| {
            mutate_utf16_first_code_unit(
                bytes,
                version_time_start,
                version_number_offset,
                "version build-time string",
            );
        });
        assert!(
            Recorder::new()
                .test_version_playback(version_time_name)
                .unwrap()
        );

        let version_number_name = format!("version_number_diff{ext}");
        write_variant(&base_path, &replays_dir, &version_number_name, |bytes| {
            let current = u32::from_le_bytes(
                bytes[version_number_offset..version_number_offset + 4]
                    .try_into()
                    .expect("version number slice should be 4 bytes"),
            );
            bytes[version_number_offset..version_number_offset + 4]
                .copy_from_slice(&current.wrapping_add(1).to_le_bytes());
        });
        assert!(
            Recorder::new()
                .test_version_playback(version_number_name)
                .unwrap()
        );

        let exe_crc_name = format!("exe_crc_diff{ext}");
        write_variant(&base_path, &replays_dir, &exe_crc_name, |bytes| {
            let current = u32::from_le_bytes(
                bytes[exe_crc_offset..exe_crc_offset + 4]
                    .try_into()
                    .expect("exe CRC slice should be 4 bytes"),
            );
            bytes[exe_crc_offset..exe_crc_offset + 4]
                .copy_from_slice(&current.wrapping_add(1).to_le_bytes());
        });
        assert!(Recorder::new().test_version_playback(exe_crc_name).unwrap());

        let ini_crc_name = format!("ini_crc_diff{ext}");
        write_variant(&base_path, &replays_dir, &ini_crc_name, |bytes| {
            let current = u32::from_le_bytes(
                bytes[ini_crc_offset..ini_crc_offset + 4]
                    .try_into()
                    .expect("ini CRC slice should be 4 bytes"),
            );
            bytes[ini_crc_offset..ini_crc_offset + 4]
                .copy_from_slice(&current.wrapping_add(1).to_le_bytes());
        });
        assert!(Recorder::new().test_version_playback(ini_crc_name).unwrap());
    }

    #[test]
    fn test_global_recorder() {
        let recorder = init_recorder();
        assert!(recorder.lock().is_ok());

        // Test with_recorder
        let mode = with_recorder(|r| r.get_mode());
        assert_eq!(mode, Some(RecorderMode::None));

        // Test with_recorder_mut
        let result = with_recorder_mut(|r| {
            r.mode = RecorderMode::Record;
            r.get_mode()
        });
        assert_eq!(result, Some(RecorderMode::Record));
    }

    #[test]
    fn test_replay_type_ids_match_cpp_1000_base() {
        // C++ MessageStream.h:462-570 — network IDs start at 1000.
        assert_eq!(
            replay_type_id(&GameMessageType::ClearGameData).unwrap(),
            CPP_MSG_CLEAR_GAME_DATA
        );
        assert_eq!(
            replay_type_id(&GameMessageType::NewGame).unwrap(),
            CPP_MSG_NEW_GAME
        );
        assert_eq!(
            replay_type_id(&GameMessageType::CreateSelectedGroup(true, vec![1])).unwrap(),
            1001
        );
        assert_eq!(
            replay_type_id(&GameMessageType::DoMoveTo(
                crate::common::message_stream::game_message::Coord3D {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0
                }
            ))
            .unwrap(),
            1068
        );
        assert_eq!(
            replay_type_id(&GameMessageType::LogicCRC(0x11)).unwrap(),
            CPP_MSG_LOGIC_CRC
        );
        assert_eq!(
            replay_type_id(&GameMessageType::CreateTeamSlot(3)).unwrap(),
            CPP_MSG_CREATE_TEAM0 + 3
        );
        let (ty, _) = decode_replay_type_id(
            1068,
            &[GameMessageArgumentType::Location(
                crate::common::message_stream::game_message::Coord3D {
                    x: 1.0,
                    y: 2.0,
                    z: 3.0,
                },
            )],
        )
        .unwrap();
        assert!(matches!(ty, GameMessageType::DoMoveTo(_)));
        let (ty, _) = decode_replay_type_id(1009, &[]).unwrap();
        assert_eq!(ty, GameMessageType::CreateTeamSlot(3));
    }

    #[test]
    fn test_pixel_region_writes_lo_hi_not_xywh() {
        // C++ BaseType.h IRegion2D { lo, hi }; Recorder.cpp writeArgument PIXELREGION.
        let region = crate::common::message_stream::game_message::IRegion2D {
            x: 100,
            y: 100,
            width: 100,
            height: 100,
        };
        let mut buf = Vec::new();
        Recorder::write_argument(
            &mut buf,
            &GameMessageArgumentType::PixelRegion(region.clone()),
        )
        .unwrap();
        assert_eq!(
            buf,
            [
                100i32.to_le_bytes(),
                100i32.to_le_bytes(),
                200i32.to_le_bytes(),
                200i32.to_le_bytes()
            ]
            .concat()
        );
        let decoded = Recorder::read_argument(
            &mut buf.as_slice(),
            GameMessageArgumentDataType::PixelRegion,
        )
        .unwrap();
        match decoded {
            GameMessageArgumentType::PixelRegion(got) => {
                assert_eq!(got.x, 100);
                assert_eq!(got.y, 100);
                assert_eq!(got.width, 100);
                assert_eq!(got.height, 100);
            }
            other => panic!("unexpected {other:?}"),
        }
    }

    #[test]
    fn test_start_recording_preserves_lobby_slots() {
        let temp = tempfile::tempdir().unwrap();
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/LobbySlots.map".to_string();
            data.pending_file.clear();
        }

        let mut lobby = ReplayGameInfo::new();
        lobby.set_map("Maps/LobbySlots.map".to_string());
        lobby.set_seed(42);
        lobby.set_crc_interval(100);
        lobby.slots[0] = ReplaySlot {
            name: "Host".to_string(),
            ip: 0x0A00_0001,
            is_human: true,
            is_observer: false,
            is_occupied: true,
            color: 3,
            player_template: 4,
            start_pos: 1,
            team: 0,
            nat: 2,
            port: 8086,
            accepted: true,
            has_map: true,
            ai_difficulty: 0,
            is_open: false,
        };
        lobby.slots[1] = ReplaySlot {
            name: String::new(),
            ip: 0,
            is_human: false,
            is_observer: false,
            is_occupied: true,
            color: 1,
            player_template: 2,
            start_pos: 2,
            team: 1,
            nat: 0,
            port: 0,
            accepted: false,
            has_map: false,
            ai_difficulty: 2,
            is_open: false,
        };

        let mut writer = Recorder::new();
        writer.set_recording_lobby(lobby, 0);
        writer.start_recording(1, 2, 0, 30).unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let mut reader = Recorder::new();
        assert!(reader.playback_file(replay_name).unwrap());
        let slot0 = reader.get_game_info().get_slot(0).unwrap();
        assert_eq!(slot0.name, "Host");
        assert_eq!(slot0.color, 3);
        assert_eq!(slot0.player_template, 4);
        assert_eq!(slot0.start_pos, 1);
        assert_eq!(slot0.team, 0);
        assert_eq!(slot0.nat, 2);
        let slot1 = reader.get_game_info().get_slot(1).unwrap();
        assert!(slot1.is_occupied);
        assert!(!slot1.is_human);
        assert_eq!(slot1.color, 1);
        assert_eq!(slot1.start_pos, 2);
        assert_eq!(slot1.team, 1);
    }

    #[test]
    fn test_init_controls_hides_except_during_playback() {
        // C++ Recorder.cpp:1557-1560 winHide(mode != PLAYBACK)
        let hidden = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = hidden.clone();
        let mut recorder = Recorder::new();
        recorder.set_replay_control_visibility_hook(Some(Arc::new(move |hide| {
            flag.store(hide, std::sync::atomic::Ordering::SeqCst);
        })));
        recorder.init_controls();
        assert!(recorder.replay_controls_are_hidden());
        assert!(hidden.load(std::sync::atomic::Ordering::SeqCst));

        recorder.mode = RecorderMode::Playback;
        recorder.init_controls();
        assert!(!recorder.replay_controls_are_hidden());
        assert!(!hidden.load(std::sync::atomic::Ordering::SeqCst));
    }

    #[test]
    fn test_logic_crc_from_file_reaches_handle_crc_message() {
        let temp = tempfile::tempdir().unwrap();
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/CrcPath.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 0, 30).unwrap();
        writer.set_current_frame(5);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0xCAFE_BABE)))
            .unwrap();
        writer.stop_recording();

        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let mut reader = Recorder::new();
        reader.set_current_frame(5);
        assert!(reader.playback_file(replay_name).unwrap());
        reader.update();
        assert!(!reader.saw_crc_mismatch());
        reader.notify_logic_crc(0xCAFE_BABE, 0);
        assert!(!reader.saw_crc_mismatch());
        reader.set_current_frame(6);
        reader.notify_logic_crc(0xDEAD_BEEF, 0);
        assert!(reader.saw_crc_mismatch());
    }

    #[test]
    fn set_replay_camera_file_args_match_cpp_six_layout() {
        let coord = Coord3D {
            x: 8.0,
            y: 1.0,
            z: 3.0,
        };
        let mut msg = GameMessage::new(GameMessageType::SetReplayCamera(coord.clone(), 0.25, 1.5));
        msg.append_location_argument(coord.clone());
        msg.append_real_argument(0.25);
        msg.append_real_argument(0.5);
        msg.append_real_argument(1.5);
        msg.append_integer_argument(2);
        msg.append_pixel_argument(ICoord2D { x: 10, y: 20 });

        let args = set_replay_camera_file_args(&msg, &coord, 0.25, 1.5);
        assert_eq!(args.len(), 6);
        match &args[0] {
            GameMessageArgumentType::Location(loc) => assert!((loc.x - 8.0).abs() < f32::EPSILON),
            other => panic!("expected location, got {other:?}"),
        }
        match &args[1] {
            GameMessageArgumentType::Real(angle) => assert!((*angle - 0.25).abs() < f32::EPSILON),
            other => panic!("expected angle, got {other:?}"),
        }
        match &args[2] {
            GameMessageArgumentType::Real(pitch) => assert!((*pitch - 0.5).abs() < f32::EPSILON),
            other => panic!("expected pitch, got {other:?}"),
        }
        match &args[3] {
            GameMessageArgumentType::Real(zoom) => assert!((*zoom - 1.5).abs() < f32::EPSILON),
            other => panic!("expected zoom, got {other:?}"),
        }
        match &args[4] {
            GameMessageArgumentType::Integer(cursor) => assert_eq!(*cursor, 2),
            other => panic!("expected cursor int, got {other:?}"),
        }
        match &args[5] {
            GameMessageArgumentType::Pixel(pixel) => {
                assert_eq!(pixel.x, 10);
                assert_eq!(pixel.y, 20);
            }
            other => panic!("expected pixel, got {other:?}"),
        }
    }

    #[test]
    fn cleanup_replay_file_preserves_original_rep() {
        let temp = tempfile::tempdir().unwrap();
        let stats = temp.path().join("stats");
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.save_stats = true;
            data.base_stats_dir = stats.to_string_lossy().to_string();
            data.map_name = "Maps/Cleanup.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 0, 30).unwrap();
        writer.set_current_frame(4);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x1111_2222)))
            .unwrap();
        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );
        let src = writer.replay_dir().join(&replay_name);
        writer.log_crc_mismatch();
        writer.stop_recording();

        assert!(
            src.exists(),
            "C++ cleanUpReplayFile copies the .rep and never deletes it"
        );
        #[cfg(debug_assertions)]
        assert!(
            stats.join(&replay_name).exists(),
            "DEBUG + m_saveStats copies the replay into m_baseStatsDir"
        );
    }

    #[test]
    fn playback_file_appends_new_game_to_the_message_stream() {
        // C++ Recorder.cpp:1123-1134 appends MSG_NEW_GAME to TheMessageStream
        // (not TheCommandList). stopPlayback :447 appends MSG_CLEAR_GAME_DATA.
        let temp = tempfile::tempdir().unwrap();
        if let Some(global) = get_global_data() {
            let mut data = global.write();
            data.set_path_user_data(temp.path().to_string_lossy().to_string());
            data.map_name = "Maps/PlaybackStream.map".to_string();
            data.pending_file.clear();
        }

        let mut writer = Recorder::new();
        writer.start_recording(1, 2, 3, 60).unwrap();
        writer.set_current_frame(2);
        writer
            .write_to_file(&GameMessage::new(GameMessageType::LogicCRC(0x0BAD_F00D)))
            .unwrap();
        writer.stop_recording();
        let replay_name = format!(
            "{}{}",
            writer.last_replay_filename(),
            writer.replay_extension()
        );

        {
            let stream = get_message_stream();
            stream
                .write()
                .unwrap_or_else(|e| e.into_inner())
                .clear_messages();
        }

        let mut reader = Recorder::new();
        reader.set_game_mode_provider(Some(Arc::new(|| GAME_NONE)));
        assert!(reader.playback_file(replay_name).unwrap());

        {
            let stream = get_message_stream();
            let guard = stream.read().unwrap_or_else(|e| e.into_inner());
            let types: Vec<_> = guard
                .get_messages()
                .iter()
                .map(|msg| msg.get_type().clone())
                .collect();
            assert!(
                types
                    .iter()
                    .any(|ty| matches!(ty, GameMessageType::NewGame)),
                "playbackFile must append MSG_NEW_GAME to TheMessageStream, got {types:?}"
            );
        }

        reader.stop_playback();
        {
            let stream = get_message_stream();
            let guard = stream.read().unwrap_or_else(|e| e.into_inner());
            let types: Vec<_> = guard
                .get_messages()
                .iter()
                .map(|msg| msg.get_type().clone())
                .collect();
            assert!(
                types
                    .iter()
                    .any(|ty| matches!(ty, GameMessageType::ClearGameData)),
                "stopPlayback must append MSG_CLEAR_GAME_DATA to TheMessageStream, got {types:?}"
            );
        }
    }
}
