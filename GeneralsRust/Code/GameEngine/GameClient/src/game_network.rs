//! Non-network compatibility surface for `game_network` imports in GameClient.
//!
//! This module is compiled when the `network` Cargo feature is disabled.
//! It provides enough API surface for single-player/non-network builds.

use game_engine::common::ascii_string::AsciiString;
use std::collections::{HashMap, VecDeque};
use std::net::IpAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, RwLock};

#[path = "../../GameNetwork/src/game_info/mod.rs"]
mod game_info_port;

pub use game_info_port::{
    game_info_to_ascii_string, parse_ascii_string_to_game_info, FirewallBehaviorType, GameInfo,
    GameSlot, Money, SkirmishGameInfo, SlotState, MAX_SLOTS, PLAYERTEMPLATE_MIN,
    PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM,
};

pub mod game_info {
    pub use super::game_info_port::{
        FirewallBehaviorType, GameInfo, GameSlot, Money, SkirmishGameInfo, SlotState, MAX_SLOTS,
        PLAYERTEMPLATE_MIN, PLAYERTEMPLATE_OBSERVER, PLAYERTEMPLATE_RANDOM,
    };

    pub mod serialization {
        pub use crate::{game_info_to_ascii_string, parse_ascii_string_to_game_info};
    }
}

#[derive(Default)]
pub struct NetworkFacade;

static NETWORK_ON: AtomicBool = AtomicBool::new(true);

impl NetworkFacade {
    pub fn toggle_network_on(&self) {
        NETWORK_ON.fetch_xor(true, Ordering::SeqCst);
    }

    pub fn is_network_on(&self) -> bool {
        NETWORK_ON.load(Ordering::SeqCst)
    }

    pub async fn send_disconnect_chat_message(
        &self,
        _message: String,
        _channel: i32,
    ) -> Result<(), String> {
        Ok(())
    }

    pub async fn quit_game(&self) -> Result<(), String> {
        Ok(())
    }

    pub async fn vote_for_player_disconnect(&self, _slot: i32) -> Result<(), String> {
        Ok(())
    }

    pub async fn send_chat(&self, _message: String, _player_mask: u32) -> Result<(), String> {
        Ok(())
    }
}

pub fn get_network() -> Option<Arc<NetworkFacade>> {
    static NETWORK: OnceLock<Arc<NetworkFacade>> = OnceLock::new();
    Some(NETWORK.get_or_init(|| Arc::new(NetworkFacade)).clone())
}

pub mod network {
    use super::{get_network, NetworkFacade};
    use std::sync::Arc;

    pub struct Network;

    impl Network {
        pub fn from_global() -> Option<Arc<NetworkFacade>> {
            get_network()
        }
    }
}

pub fn get_favorite_side<T>(_stats: &T) -> i32 {
    0
}

pub mod commands {
    #[derive(Debug, Clone, Default)]
    pub enum CommandParameter {
        Integer(i32),
        Float(f32),
        Text(String),
        #[default]
        None,
    }

    #[derive(Debug, Clone, Default)]
    pub struct GameCommandData {
        pub command_type: i32,
        pub parameters: Vec<CommandParameter>,
    }
}

pub mod lan_api {
    use super::*;

    #[derive(Debug, Clone, Default)]
    pub struct LanConfig {
        pub player_name: String,
        pub login_name: String,
        pub host_name: String,
    }

    #[derive(Debug, Clone, Default)]
    pub struct LanApi {
        pub config: LanConfig,
        pub local_ip: Option<IpAddr>,
    }

    impl LanApi {
        pub async fn new(config: LanConfig) -> Result<Self, String> {
            Ok(Self {
                config,
                local_ip: None,
            })
        }

        pub async fn init(&mut self) -> Result<(), String> {
            Ok(())
        }

        pub async fn request_set_name(&mut self, name: String) -> Result<(), String> {
            self.config.player_name = name;
            Ok(())
        }

        pub async fn request_lobby_leave(&mut self, _reset_state: bool) -> Result<(), String> {
            Ok(())
        }

        pub async fn set_local_ip(&mut self, ip: IpAddr) -> Result<(), String> {
            self.local_ip = Some(ip);
            Ok(())
        }

        pub async fn request_game_create(
            &mut self,
            _label: String,
            _direct_connect: bool,
        ) -> Result<(), String> {
            Ok(())
        }

        pub async fn request_game_join_direct_connect(
            &mut self,
            _ip: IpAddr,
        ) -> Result<(), String> {
            Ok(())
        }
    }
}

pub mod matchmaking {
    pub mod slots {
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        #[repr(i32)]
        pub enum PlayerColor {
            Red = 0,
            Blue = 1,
            Green = 2,
            Yellow = 3,
            Cyan = 4,
            Purple = 5,
            Orange = 6,
            White = 7,
        }

        impl PlayerColor {
            pub fn all() -> &'static [PlayerColor] {
                const COLORS: [PlayerColor; 8] = [
                    PlayerColor::Red,
                    PlayerColor::Blue,
                    PlayerColor::Green,
                    PlayerColor::Yellow,
                    PlayerColor::Cyan,
                    PlayerColor::Purple,
                    PlayerColor::Orange,
                    PlayerColor::White,
                ];
                &COLORS
            }

            pub fn name(self) -> &'static str {
                match self {
                    PlayerColor::Red => "Red",
                    PlayerColor::Blue => "Blue",
                    PlayerColor::Green => "Green",
                    PlayerColor::Yellow => "Yellow",
                    PlayerColor::Cyan => "Cyan",
                    PlayerColor::Purple => "Purple",
                    PlayerColor::Orange => "Orange",
                    PlayerColor::White => "White",
                }
            }
        }
    }
}

pub mod rank_point_value {
    #[derive(Debug, Clone, Default)]
    pub struct RankPoints {
        pub rank: i32,
        pub points: i32,
        pub next_rank_points: i32,
    }

    pub fn calculate_rank(points: i32) -> i32 {
        points.max(0) / 1000
    }

    pub fn get_rank_point_values() -> Vec<i32> {
        vec![0, 1000, 2000, 3000, 4000, 5000]
    }

    pub fn get_favorite_side(_profile_id: i32) -> i32 {
        0
    }
}

pub mod download_manager {
    use super::*;

    #[derive(Debug, Clone, Default)]
    pub struct DownloadProgress {
        pub bytes_read: u64,
        pub total_size: u64,
        pub time_left: i64,
    }

    #[derive(Debug, Clone, Default)]
    pub struct QueuedDownload {
        pub remote_file: String,
        pub local_file: String,
        pub server_name: String,
    }

    #[derive(Debug, Clone)]
    pub enum DownloadEvent {
        FileStarted(String),
        StatusUpdate(String),
        Progress(DownloadProgress),
        Error(String),
        End,
    }

    impl Default for DownloadEvent {
        fn default() -> Self {
            Self::StatusUpdate(String::new())
        }
    }

    #[derive(Debug, Default)]
    pub struct DownloadManager {
        error_key: String,
        status_key: String,
        last_local_file: String,
        queue: VecDeque<QueuedDownload>,
        events: Vec<DownloadEvent>,
    }

    impl DownloadManager {
        pub fn new() -> Self {
            Self {
                error_key: "FTP:UnknownError".to_string(),
                status_key: "FTP:StatusIdle".to_string(),
                last_local_file: String::new(),
                queue: VecDeque::new(),
                events: Vec::new(),
            }
        }

        pub fn error_key(&self) -> &str {
            &self.error_key
        }

        pub fn status_key(&self) -> &str {
            &self.status_key
        }

        pub fn last_local_file(&self) -> &str {
            &self.last_local_file
        }

        pub fn update(&mut self) -> Vec<DownloadEvent> {
            std::mem::take(&mut self.events)
        }

        pub fn is_done(&self) -> bool {
            self.queue.is_empty() && self.events.is_empty()
        }

        pub fn is_file_queued_for_download(&self) -> bool {
            !self.queue.is_empty()
        }

        pub fn is_active(&self) -> bool {
            self.events
                .iter()
                .any(|event| !matches!(event, DownloadEvent::End))
        }

        pub fn queue_file_for_download(&mut self, download: QueuedDownload) {
            self.queue.push_back(download);
        }

        pub fn download_next_queued_file(&mut self) -> Result<(), String> {
            if let Some(next) = self.queue.pop_front() {
                self.last_local_file = next.local_file;
                self.status_key = "FTP:StatusDownloading".to_string();
                self.events
                    .push(DownloadEvent::FileStarted(next.remote_file));
                self.events.push(DownloadEvent::End);
            }
            Ok(())
        }
    }

    pub fn download_manager() -> &'static Mutex<Option<DownloadManager>> {
        static DOWNLOAD_MANAGER: OnceLock<Mutex<Option<DownloadManager>>> = OnceLock::new();
        DOWNLOAD_MANAGER.get_or_init(|| Mutex::new(None))
    }

    pub fn set_download_manager(manager: Option<DownloadManager>) {
        let mut guard = download_manager()
            .lock()
            .expect("DownloadManager compatibility lock poisoned");
        *guard = manager;
    }
}

pub mod gamespy {
    use super::*;

    pub mod config {
        #[derive(Debug, Clone, Default)]
        pub struct GameSpyConfig;

        impl GameSpyConfig {
            pub fn new_sync() -> Self {
                Self
            }

            pub fn get_ping_config(&self) -> (i32, i32, i32, i32) {
                (0, 1000, 0, 0)
            }
        }
    }

    pub mod ladder_defs {
        use super::*;

        #[derive(Debug, Clone, Default)]
        pub struct LadderInfo {
            pub index: i32,
            pub valid_qm: bool,
            pub max_wins: i32,
            pub min_wins: i32,
            pub address: String,
            pub port: i32,
            pub name: String,
            pub players_per_team: i32,
            pub random_factions: bool,
            pub random_maps: bool,
        }

        #[derive(Debug, Clone, Default)]
        pub struct LadderMapMeta {
            pub display_name: String,
            pub num_players: i32,
        }

        pub trait LadderMapProvider: Send + Sync {
            fn map_dir(&self) -> String;
            fn find_map(&self, map_path: &str) -> Option<LadderMapMeta>;
        }

        #[derive(Debug, Default)]
        pub struct LadderList {
            pub ladders: Vec<LadderInfo>,
            pub special_ladders: Vec<LadderInfo>,
            pub standard_ladders: Vec<LadderInfo>,
        }

        impl LadderList {
            pub fn find_ladder(&self, address: &AsciiString, port: i32) -> Option<&LadderInfo> {
                self.ladders.iter().find(|ladder| {
                    ladder.address.eq_ignore_ascii_case(address.as_str()) && ladder.port == port
                })
            }

            pub fn find_ladder_by_index(&self, index: i32) -> Option<&LadderInfo> {
                self.ladders.iter().find(|ladder| ladder.index == index)
            }

            pub fn get_special_ladders(&self) -> &[LadderInfo] {
                &self.special_ladders
            }

            pub fn get_standard_ladders(&self) -> &[LadderInfo] {
                &self.standard_ladders
            }
        }

        pub fn get_ladder_list() -> Option<Arc<RwLock<LadderList>>> {
            static LIST: OnceLock<Arc<RwLock<LadderList>>> = OnceLock::new();
            Some(
                LIST.get_or_init(|| Arc::new(RwLock::new(LadderList::default())))
                    .clone(),
            )
        }

        pub fn init_ladder_list() {}

        pub fn set_ladder_map_provider(_provider: Arc<dyn LadderMapProvider>) {}
    }

    pub mod peer_defs {
        use super::*;

        #[derive(Debug, Clone, Copy)]
        #[repr(usize)]
        pub enum GameSpyColor {
            Default = 0,
            Motd = 1,
            MotdHeading = 2,
            PlayerNormal = 3,
            PlayerOwner = 4,
            PlayerBuddy = 5,
            PlayerIgnored = 6,
            PlayerSelf = 7,
            Game = 8,
            GameFull = 9,
            GameCrcMismatch = 10,
            MapSelected = 11,
            MapUnselected = 12,
        }

        #[derive(Debug, Clone, Default)]
        pub struct GameSpyGroupRoom {
            pub id: i32,
            pub name: String,
        }

        #[derive(Debug, Clone, Default)]
        pub struct GPProfile {
            pub profile_id: i32,
            pub name: AsciiString,
        }

        #[derive(Debug, Clone, Default)]
        pub struct PlayerInfo {
            pub profile_id: i32,
            pub name: AsciiString,
            pub side: i32,
            pub rank: i32,
            pub wins: i32,
            pub losses: i32,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum GameSpyBuddyStatus {
            #[default]
            Offline,
            Online,
            InLobby,
            InGame,
        }

        #[derive(Debug, Clone, Default)]
        pub struct BuddyMessage {
            pub from_profile_id: i32,
            pub from_name: AsciiString,
            pub message: String,
        }

        #[derive(Debug, Clone, Default)]
        pub struct GameSpyStagingRoom {
            pub id: i32,
            pub name: String,
            pub map_name: String,
            pub game_options: String,
        }

        #[derive(Debug, Clone, Default)]
        pub struct GameSpyInfo {
            local_profile_id: i32,
            local_name: AsciiString,
            pub group_room: GameSpyGroupRoom,
            pub staging_room: GameSpyStagingRoom,
            pub messages: Vec<(String, u32, Option<u32>)>,
        }

        impl GameSpyInfo {
            pub fn get_local_profile_id(&self) -> i32 {
                self.local_profile_id
            }

            pub fn set_local_profile_id(&mut self, id: i32) {
                self.local_profile_id = id;
            }

            pub fn get_local_name(&self) -> AsciiString {
                self.local_name.clone()
            }

            pub fn set_local_name(&mut self, name: &str) {
                self.local_name = AsciiString::from(name);
            }

            pub fn leave_group_room(&mut self) {}

            pub fn register_text_window(&mut self, _window_id: u32) {}

            pub fn unregister_text_window(&mut self, _window_id: u32) {}

            pub fn add_text(&mut self, text: String, color: u32, window_id: Option<u32>) {
                self.messages.push((text, color, window_id));
            }

            pub fn get_group_room(&self) -> &GameSpyGroupRoom {
                &self.group_room
            }

            pub fn get_staging_room(&self) -> &GameSpyStagingRoom {
                &self.staging_room
            }
        }

        pub fn default_gamespy_colors() -> [u32; 16] {
            [0xFFFF_FFFF; 16]
        }

        pub fn make_color(r: u8, g: u8, b: u8) -> u32 {
            ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
        }

        pub fn get_gamespy_info() -> Option<Arc<Mutex<GameSpyInfo>>> {
            static INFO: OnceLock<Arc<Mutex<GameSpyInfo>>> = OnceLock::new();
            Some(
                INFO.get_or_init(|| Arc::new(Mutex::new(GameSpyInfo::default())))
                    .clone(),
            )
        }

        pub fn tear_down_gamespy() {}
    }

    pub mod peer_thread {
        use super::*;

        #[derive(Debug, Clone, Default)]
        pub struct PeerRequest {
            pub request_type: PeerRequestType,
            pub payload: String,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum PeerRequestType {
            #[default]
            None,
            StopQuickMatch,
            WidenQuickMatchSearch,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum DisconnectReason {
            #[default]
            Unknown,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum PeerResponseType {
            #[default]
            None,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum QMStatus {
            #[default]
            None,
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

        #[derive(Debug, Clone, Default)]
        pub struct PeerResponse {
            pub response_type: PeerResponseType,
        }

        #[derive(Debug, Default)]
        pub struct PeerMessageQueue {
            pub requests: VecDeque<PeerRequest>,
            pub responses: VecDeque<PeerResponse>,
        }

        impl PeerMessageQueue {
            pub fn add_request(&mut self, request: PeerRequest) {
                self.requests.push_back(request);
            }

            pub fn pop_response(&mut self) -> Option<PeerResponse> {
                self.responses.pop_front()
            }

            pub fn is_connected(&self) -> bool {
                false
            }
        }

        pub fn get_peer_message_queue() -> Option<Arc<Mutex<PeerMessageQueue>>> {
            static QUEUE: OnceLock<Arc<Mutex<PeerMessageQueue>>> = OnceLock::new();
            Some(
                QUEUE
                    .get_or_init(|| Arc::new(Mutex::new(PeerMessageQueue::default())))
                    .clone(),
            )
        }

        pub fn teardown_peer_message_queue() {}

        pub fn init_peer_message_queue() {}
    }

    pub mod persistent_storage_thread {
        use super::*;

        #[derive(Debug, Clone, Default)]
        pub struct PlayerStats {
            pub profile_id: i32,
            pub wins: HashMap<i32, u32>,
            pub losses: HashMap<i32, u32>,
            pub points: i32,
        }

        #[derive(Debug, Clone, Default)]
        pub struct PSRequest {
            pub request_type: PSRequestType,
        }

        #[derive(Debug, Clone, Default)]
        pub struct PSResponse {
            pub response_type: PSResponseType,
        }

        pub type PSPlayerStats = PlayerStats;

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum PSRequestType {
            #[default]
            None,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum PSResponseType {
            #[default]
            None,
        }

        pub const LOC_MIN: i32 = 0;
        pub const LOC_MAX: i32 = 255;

        #[derive(Debug, Default)]
        pub struct GameSpyPSMessageQueue {
            pub requests: VecDeque<PSRequest>,
            pub responses: VecDeque<PSResponse>,
            pub stats_by_profile: HashMap<i32, PlayerStats>,
        }

        impl GameSpyPSMessageQueue {
            pub fn add_request(&mut self, request: PSRequest) {
                self.requests.push_back(request);
            }

            pub fn pop_response(&mut self) -> Option<PSResponse> {
                self.responses.pop_front()
            }

            pub fn find_player_stats_by_id(&self, profile_id: i32) -> PlayerStats {
                self.stats_by_profile
                    .get(&profile_id)
                    .cloned()
                    .unwrap_or_default()
            }
        }

        pub fn get_ps_message_queue() -> Option<Arc<Mutex<GameSpyPSMessageQueue>>> {
            static QUEUE: OnceLock<Arc<Mutex<GameSpyPSMessageQueue>>> = OnceLock::new();
            Some(
                QUEUE
                    .get_or_init(|| Arc::new(Mutex::new(GameSpyPSMessageQueue::default())))
                    .clone(),
            )
        }

        pub fn teardown_ps_message_queue() {}

        pub fn init_ps_message_queue() {}
    }

    pub mod buddy_thread {
        use super::*;

        pub const MAX_BUDDY_CHAT_LEN: usize = 127;

        #[derive(Debug, Clone, Default)]
        pub struct BuddyRequest {
            pub request_type: BuddyRequestType,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum BuddyRequestType {
            #[default]
            None,
        }

        #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
        pub enum BuddyResponseType {
            #[default]
            None,
        }

        #[derive(Debug, Clone, Default)]
        pub struct BuddyResponse {
            pub response_type: BuddyResponseType,
        }

        #[derive(Debug, Default)]
        pub struct BuddyMessageQueue {
            pub requests: VecDeque<BuddyRequest>,
            pub responses: VecDeque<BuddyResponse>,
        }

        impl BuddyMessageQueue {
            pub fn add_request(&mut self, request: BuddyRequest) {
                self.requests.push_back(request);
            }

            pub fn pop_response(&mut self) -> Option<BuddyResponse> {
                self.responses.pop_front()
            }
        }

        pub fn get_buddy_message_queue() -> Option<Arc<Mutex<BuddyMessageQueue>>> {
            static QUEUE: OnceLock<Arc<Mutex<BuddyMessageQueue>>> = OnceLock::new();
            Some(
                QUEUE
                    .get_or_init(|| Arc::new(Mutex::new(BuddyMessageQueue::default())))
                    .clone(),
            )
        }

        pub fn teardown_buddy_message_queue() {}

        pub fn init_buddy_message_queue() {}
    }

    pub mod ping_thread {
        use super::*;

        #[derive(Debug, Clone, Default)]
        pub struct PingRequest {
            pub host: String,
            pub port: u16,
        }

        #[derive(Debug, Default)]
        pub struct PingQueue {
            pub requests: VecDeque<PingRequest>,
        }

        impl PingQueue {
            pub fn add_request(&mut self, request: PingRequest) {
                self.requests.push_back(request);
            }
        }

        pub fn init_ping_queue() {}

        pub fn get_ping_queue() -> Option<Arc<Mutex<PingQueue>>> {
            static QUEUE: OnceLock<Arc<Mutex<PingQueue>>> = OnceLock::new();
            Some(
                QUEUE
                    .get_or_init(|| Arc::new(Mutex::new(PingQueue::default())))
                    .clone(),
            )
        }

        pub fn teardown_ping_queue() {}
    }
}
