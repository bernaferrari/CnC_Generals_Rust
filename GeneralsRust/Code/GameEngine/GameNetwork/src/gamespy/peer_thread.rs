//! GameSpy peer thread definitions (C++ PeerThread.cpp parity).

use std::collections::VecDeque;
use std::sync::{Arc, Mutex, OnceLock};

use crate::config::MAX_SLOTS;
use crate::error::NetworkResult;
use crate::gamespy::peer_defs::GPProfile;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SerialAuthResult {
    Nonexistent,
    AuthFailed,
    Banned,
    Ok,
}

impl Default for SerialAuthResult {
    fn default() -> Self {
        SerialAuthResult::Nonexistent
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisconnectReason {
    NickTaken = 1,
    BadNick,
    LostConnection,
    CouldNotConnect,
    GpLoginTimeout,
    GpLoginBadNick,
    GpLoginBadEmail,
    GpLoginBadPassword,
    GpLoginBadProfile,
    GpLoginProfileDeleted,
    GpLoginConnectionFailed,
    GpLoginServerAuthFailed,
    SerialInvalid,
    SerialNotPresent,
    SerialBanned,
    GpNewUserBadNick,
    GpNewUserBadPassword,
    GpNewProfileBadNick,
    GpNewProfileBadOldNick,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QMStatus {
    Idle,
    JoiningQmChannel,
    LookingForBot,
    SentInfo,
    Working,
    PoolSize,
    WideningSearch,
    Matched,
    InChannel,
    NegotiatingFirewalls,
    StartingGame,
    CouldNotFindBot,
    CouldNotFindChannel,
    CouldNotNegotiateFirewalls,
    Stopped,
}

#[derive(Debug, Clone)]
pub enum PeerRequestType {
    Login,
    Logout,
    MessagePlayer,
    MessageRoom,
    JoinGroupRoom,
    LeaveGroupRoom,
    StartGameList,
    StopGameList,
    CreateStagingRoom,
    SetGameOptions,
    JoinStagingRoom,
    LeaveStagingRoom,
    UtmPlayer,
    UtmRoom,
    StartGame,
    StartQuickMatch,
    WidenQuickMatchSearch,
    StopQuickMatch,
    PushStats,
    GetExtendedStagingRoomInfo,
}

#[derive(Debug, Clone)]
pub struct PeerRequest {
    pub request_type: PeerRequestType,
    pub nick: String,
    pub text: String,
    pub password: String,
    pub email: String,
    pub id: String,
    pub options: String,
    pub ladder_ip: String,
    pub host_ping_str: String,
    pub game_opts_map_name: String,
    pub game_opts_player_names: [String; MAX_SLOTS],
    pub qm_maps: Vec<bool>,
    pub profile_id: i32,
    pub group_id: i32,
    pub restrict_game_list: bool,
    pub is_action: bool,
    pub staging_room_id: i32,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub game_version: u32,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub lad_port: u16,
    pub lad_pass_crc: u32,
    pub wins: [i32; MAX_SLOTS],
    pub losses: [i32; MAX_SLOTS],
    pub profiles: [i32; MAX_SLOTS],
    pub faction: [i32; MAX_SLOTS],
    pub color: [i32; MAX_SLOTS],
    pub num_players: i32,
    pub max_players: i32,
    pub num_observers: i32,
    pub qm_min_point_percentage: i32,
    pub qm_max_point_percentage: i32,
    pub qm_points: i32,
    pub qm_widen_time: i32,
    pub qm_ladder_id: i32,
    pub qm_ladder_pass_crc: u32,
    pub qm_max_ping: i32,
    pub qm_max_discons: i32,
    pub qm_discons: i32,
    pub qm_pings: [u8; 8],
    pub qm_num_players: i32,
    pub qm_bot_id: i32,
    pub qm_room_id: i32,
    pub qm_side: i32,
    pub qm_color: i32,
    pub qm_nat: i32,
    pub stats_locale: i32,
    pub stats_wins: i32,
    pub stats_losses: i32,
    pub stats_rank_points: i32,
    pub stats_side: i32,
    pub stats_preorder: bool,
}

impl Default for PeerRequest {
    fn default() -> Self {
        Self {
            request_type: PeerRequestType::Login,
            nick: String::new(),
            text: String::new(),
            password: String::new(),
            email: String::new(),
            id: String::new(),
            options: String::new(),
            ladder_ip: String::new(),
            host_ping_str: String::new(),
            game_opts_map_name: String::new(),
            game_opts_player_names: std::array::from_fn(|_| String::new()),
            qm_maps: Vec::new(),
            profile_id: 0,
            group_id: 0,
            restrict_game_list: false,
            is_action: false,
            staging_room_id: 0,
            exe_crc: 0,
            ini_crc: 0,
            game_version: 0,
            allow_observers: false,
            use_stats: false,
            lad_port: 0,
            lad_pass_crc: 0,
            wins: [0; MAX_SLOTS],
            losses: [0; MAX_SLOTS],
            profiles: [0; MAX_SLOTS],
            faction: [0; MAX_SLOTS],
            color: [0; MAX_SLOTS],
            num_players: 0,
            max_players: 0,
            num_observers: 0,
            qm_min_point_percentage: 0,
            qm_max_point_percentage: 0,
            qm_points: 0,
            qm_widen_time: 0,
            qm_ladder_id: 0,
            qm_ladder_pass_crc: 0,
            qm_max_ping: 0,
            qm_max_discons: 0,
            qm_discons: 0,
            qm_pings: [0; 8],
            qm_num_players: 0,
            qm_bot_id: 0,
            qm_room_id: 0,
            qm_side: 0,
            qm_color: 0,
            qm_nat: 0,
            stats_locale: 0,
            stats_wins: 0,
            stats_losses: 0,
            stats_rank_points: 0,
            stats_side: 0,
            stats_preorder: false,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PeerResponseType {
    Login,
    Disconnect,
    Message,
    GroupRoom,
    StagingRoom,
    StagingRoomListComplete,
    StagingRoomPlayerInfo,
    JoinGroupRoom,
    CreateStagingRoom,
    JoinStagingRoom,
    PlayerJoin,
    PlayerLeft,
    PlayerChangedNick,
    PlayerInfo,
    PlayerChangedFlags,
    RoomUtm,
    PlayerUtm,
    QuickMatchStatus,
    GameStart,
    FailedToHost,
}

#[derive(Debug, Clone)]
pub struct PeerResponse {
    pub response_type: PeerResponseType,
    pub group_room_name: String,
    pub nick: String,
    pub old_nick: String,
    pub text: String,
    pub locale: String,
    pub staging_server_game_options: String,
    pub staging_server_name: String,
    pub staging_server_ping_string: String,
    pub staging_server_ladder_ip: String,
    pub staging_room_map_name: String,
    pub staging_room_player_names: [String; MAX_SLOTS],
    pub command: String,
    pub command_options: String,
    pub discon_reason: DisconnectReason,
    pub group_room_id: i32,
    pub group_room_num_waiting: i32,
    pub group_room_max_waiting: i32,
    pub group_room_num_games: i32,
    pub group_room_num_playing: i32,
    pub join_group_ok: bool,
    pub create_staging_result: i32,
    pub join_staging_id: i32,
    pub join_staging_ok: bool,
    pub join_staging_host_present: bool,
    pub join_staging_result: i32,
    pub message_is_private: bool,
    pub message_is_action: bool,
    pub message_profile_id: i32,
    pub player_profile_id: i32,
    pub player_wins: i32,
    pub player_losses: i32,
    pub player_room_type: i32,
    pub player_flags: i32,
    pub player_ip: u32,
    pub player_rank_points: i32,
    pub player_side: i32,
    pub player_preorder: i32,
    pub player_internal_ip: u32,
    pub player_external_ip: u32,
    pub staging_id: i32,
    pub staging_action: i32,
    pub staging_is_staging: bool,
    pub staging_requires_password: bool,
    pub staging_allow_observers: bool,
    pub staging_use_stats: bool,
    pub staging_version: u32,
    pub staging_exe_crc: u32,
    pub staging_ini_crc: u32,
    pub staging_ladder_port: u16,
    pub staging_wins: [i32; MAX_SLOTS],
    pub staging_losses: [i32; MAX_SLOTS],
    pub staging_profiles: [i32; MAX_SLOTS],
    pub staging_faction: [i32; MAX_SLOTS],
    pub staging_color: [i32; MAX_SLOTS],
    pub staging_num_players: i32,
    pub staging_num_observers: i32,
    pub staging_max_players: i32,
    pub staging_percent_complete: i32,
    pub qm_status: QMStatus,
    pub qm_pool_size: i32,
    pub qm_map_idx: i32,
    pub qm_seed: i32,
    pub qm_ip: [u32; MAX_SLOTS],
    pub qm_side: [i32; MAX_SLOTS],
    pub qm_color: [i32; MAX_SLOTS],
    pub qm_nat: [i32; MAX_SLOTS],
}

impl Default for PeerResponse {
    fn default() -> Self {
        Self {
            response_type: PeerResponseType::Login,
            group_room_name: String::new(),
            nick: String::new(),
            old_nick: String::new(),
            text: String::new(),
            locale: String::new(),
            staging_server_game_options: String::new(),
            staging_server_name: String::new(),
            staging_server_ping_string: String::new(),
            staging_server_ladder_ip: String::new(),
            staging_room_map_name: String::new(),
            staging_room_player_names: std::array::from_fn(|_| String::new()),
            command: String::new(),
            command_options: String::new(),
            discon_reason: DisconnectReason::LostConnection,
            group_room_id: 0,
            group_room_num_waiting: 0,
            group_room_max_waiting: 0,
            group_room_num_games: 0,
            group_room_num_playing: 0,
            join_group_ok: false,
            create_staging_result: 0,
            join_staging_id: 0,
            join_staging_ok: false,
            join_staging_host_present: false,
            join_staging_result: 0,
            message_is_private: false,
            message_is_action: false,
            message_profile_id: 0,
            player_profile_id: 0,
            player_wins: 0,
            player_losses: 0,
            player_room_type: 0,
            player_flags: 0,
            player_ip: 0,
            player_rank_points: 0,
            player_side: 0,
            player_preorder: 0,
            player_internal_ip: 0,
            player_external_ip: 0,
            staging_id: 0,
            staging_action: 0,
            staging_is_staging: false,
            staging_requires_password: false,
            staging_allow_observers: false,
            staging_use_stats: false,
            staging_version: 0,
            staging_exe_crc: 0,
            staging_ini_crc: 0,
            staging_ladder_port: 0,
            staging_wins: [0; MAX_SLOTS],
            staging_losses: [0; MAX_SLOTS],
            staging_profiles: [0; MAX_SLOTS],
            staging_faction: [0; MAX_SLOTS],
            staging_color: [0; MAX_SLOTS],
            staging_num_players: 0,
            staging_num_observers: 0,
            staging_max_players: 0,
            staging_percent_complete: 0,
            qm_status: QMStatus::Idle,
            qm_pool_size: 0,
            qm_map_idx: 0,
            qm_seed: 0,
            qm_ip: [0; MAX_SLOTS],
            qm_side: [0; MAX_SLOTS],
            qm_color: [0; MAX_SLOTS],
            qm_nat: [0; MAX_SLOTS],
        }
    }
}

#[derive(Default)]
pub struct GameSpyPeerMessageQueue {
    requests: VecDeque<PeerRequest>,
    responses: VecDeque<PeerResponse>,
    running: bool,
    connected: bool,
    connecting: bool,
    serial_auth_result: SerialAuthResult,
}

impl GameSpyPeerMessageQueue {
    pub fn start_thread(&mut self) {
        self.running = true;
    }

    pub fn end_thread(&mut self) {
        self.running = false;
        self.connected = false;
        self.connecting = false;
    }

    pub fn is_thread_running(&self) -> bool {
        self.running
    }

    pub fn is_connected(&self) -> bool {
        self.connected
    }

    pub fn is_connecting(&self) -> bool {
        self.connecting
    }

    pub fn set_connected(&mut self, connected: bool) {
        self.connected = connected;
    }

    pub fn set_connecting(&mut self, connecting: bool) {
        self.connecting = connecting;
    }

    pub fn add_request(&mut self, req: PeerRequest) {
        self.requests.push_back(req);
    }

    pub fn get_request(&mut self) -> Option<PeerRequest> {
        self.requests.pop_front()
    }

    pub fn add_response(&mut self, resp: PeerResponse) {
        self.responses.push_back(resp);
    }

    pub fn get_response(&mut self) -> Option<PeerResponse> {
        self.responses.pop_front()
    }

    pub fn set_serial_auth_result(&mut self, result: SerialAuthResult) {
        self.serial_auth_result = result;
    }

    pub fn get_serial_auth_result(&self) -> SerialAuthResult {
        self.serial_auth_result
    }
}

static THE_GAMESPY_PEER_QUEUE: OnceLock<Arc<Mutex<GameSpyPeerMessageQueue>>> = OnceLock::new();

pub fn init_peer_message_queue() -> Arc<Mutex<GameSpyPeerMessageQueue>> {
    THE_GAMESPY_PEER_QUEUE
        .get_or_init(|| Arc::new(Mutex::new(GameSpyPeerMessageQueue::default())))
        .clone()
}

pub fn get_peer_message_queue() -> Option<Arc<Mutex<GameSpyPeerMessageQueue>>> {
    THE_GAMESPY_PEER_QUEUE.get().cloned()
}

pub fn teardown_peer_message_queue() {
    if let Some(queue) = THE_GAMESPY_PEER_QUEUE.get() {
        if let Ok(mut guard) = queue.lock() {
            guard.requests.clear();
            guard.responses.clear();
        }
    }
}
