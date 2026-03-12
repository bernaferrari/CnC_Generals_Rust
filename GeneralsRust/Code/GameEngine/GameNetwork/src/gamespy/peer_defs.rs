//! GameSpy peer definitions and shared runtime state (C++ PeerDefs.cpp parity).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::sync::{Arc, Mutex, OnceLock, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::MAX_SLOTS;
use game_engine::common::ascii_string::AsciiString;

pub type GPProfile = i32;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSpyColor {
    Default = 0,
    CurrentRoom,
    Room,
    Game,
    GameFull,
    GameCrcMismatch,
    PlayerNormal,
    PlayerOwner,
    PlayerBuddy,
    PlayerSelf,
    PlayerIgnored,
    ChatNormal,
    ChatEmote,
    ChatOwner,
    ChatOwnerEmote,
    ChatPrivate,
    ChatPrivateEmote,
    ChatPrivateOwner,
    ChatPrivateOwnerEmote,
    ChatBuddy,
    ChatSelf,
    AcceptTrue,
    AcceptFalse,
    MapSelected,
    MapUnselected,
    Motd,
    MotdHeading,
    Max,
}

pub type Color = u32;

pub fn make_color(r: u8, g: u8, b: u8, a: u8) -> Color {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

pub fn default_gamespy_colors() -> [Color; GameSpyColor::Max as usize] {
    let mut colors = [0u32; GameSpyColor::Max as usize];
    colors[GameSpyColor::Default as usize] = make_color(255, 255, 255, 255);
    colors[GameSpyColor::CurrentRoom as usize] = make_color(255, 255, 0, 255);
    colors[GameSpyColor::Room as usize] = make_color(255, 255, 255, 255);
    colors[GameSpyColor::Game as usize] = make_color(128, 128, 0, 255);
    colors[GameSpyColor::GameFull as usize] = make_color(128, 128, 128, 255);
    colors[GameSpyColor::GameCrcMismatch as usize] = make_color(128, 128, 128, 255);
    colors[GameSpyColor::PlayerNormal as usize] = make_color(255, 255, 255, 255);
    colors[GameSpyColor::PlayerOwner as usize] = make_color(255, 0, 255, 255);
    colors[GameSpyColor::PlayerBuddy as usize] = make_color(255, 0, 128, 255);
    colors[GameSpyColor::PlayerSelf as usize] = make_color(255, 0, 0, 255);
    colors[GameSpyColor::PlayerIgnored as usize] = make_color(128, 128, 128, 255);
    colors[GameSpyColor::ChatNormal as usize] = make_color(255, 255, 255, 255);
    colors[GameSpyColor::ChatEmote as usize] = make_color(255, 128, 0, 255);
    colors[GameSpyColor::ChatOwner as usize] = make_color(255, 255, 0, 255);
    colors[GameSpyColor::ChatOwnerEmote as usize] = make_color(128, 255, 0, 255);
    colors[GameSpyColor::ChatPrivate as usize] = make_color(0, 0, 255, 255);
    colors[GameSpyColor::ChatPrivateEmote as usize] = make_color(0, 255, 255, 255);
    colors[GameSpyColor::ChatPrivateOwner as usize] = make_color(255, 0, 255, 255);
    colors[GameSpyColor::ChatPrivateOwnerEmote as usize] = make_color(255, 128, 255, 255);
    colors[GameSpyColor::ChatBuddy as usize] = make_color(255, 0, 255, 255);
    colors[GameSpyColor::ChatSelf as usize] = make_color(255, 0, 128, 255);
    colors[GameSpyColor::AcceptTrue as usize] = make_color(0, 255, 0, 255);
    colors[GameSpyColor::AcceptFalse as usize] = make_color(255, 0, 0, 255);
    colors[GameSpyColor::MapSelected as usize] = make_color(255, 255, 0, 255);
    colors[GameSpyColor::MapUnselected as usize] = make_color(255, 255, 255, 255);
    colors[GameSpyColor::Motd as usize] = make_color(255, 255, 255, 255);
    colors[GameSpyColor::MotdHeading as usize] = make_color(255, 255, 0, 255);
    colors
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameSpyBuddyStatus {
    Offline,
    Online,
    Lobby,
    Staging,
    Loading,
    Playing,
    Matching,
}

#[derive(Debug, Clone)]
pub struct GameSpyGroupRoom {
    pub name: AsciiString,
    pub translated_name: String,
    pub group_id: i32,
    pub num_waiting: i32,
    pub max_waiting: i32,
    pub num_games: i32,
    pub num_playing: i32,
}

impl Default for GameSpyGroupRoom {
    fn default() -> Self {
        Self {
            name: AsciiString::new(),
            translated_name: String::new(),
            group_id: 0,
            num_waiting: 0,
            max_waiting: 0,
            num_games: 0,
            num_playing: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlayerInfo {
    pub name: AsciiString,
    pub locale: AsciiString,
    pub wins: i32,
    pub losses: i32,
    pub profile_id: i32,
    pub flags: i32,
    pub rank_points: i32,
    pub side: i32,
    pub preorder: i32,
}

impl Default for PlayerInfo {
    fn default() -> Self {
        Self {
            name: AsciiString::new(),
            locale: AsciiString::new(),
            wins: 0,
            losses: 0,
            profile_id: 0,
            flags: 0,
            rank_points: 0,
            side: 0,
            preorder: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct BuddyInfo {
    pub id: GPProfile,
    pub name: AsciiString,
    pub email: AsciiString,
    pub country_code: AsciiString,
    pub status: GameSpyBuddyStatus,
    pub status_string: String,
    pub location_string: String,
}

#[derive(Debug, Clone)]
pub struct BuddyMessage {
    pub timestamp: u64,
    pub sender_id: GPProfile,
    pub sender_nick: AsciiString,
    pub recipient_id: GPProfile,
    pub recipient_nick: AsciiString,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct GameSpyStagingRoom {
    pub id: i32,
    pub name: String,
    pub has_password: bool,
    pub ladder_ip: AsciiString,
    pub ladder_port: u16,
    pub map_name: AsciiString,
    pub max_players: i32,
    pub num_players: i32,
    pub num_observers: i32,
    pub allow_observers: bool,
    pub use_stats: bool,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub version: u32,
    pub host_ping: AsciiString,
    pub ping_string: AsciiString,
    pub player_names: [AsciiString; MAX_SLOTS],
    pub slot_profiles: [i32; MAX_SLOTS],
    pub slot_wins: [i32; MAX_SLOTS],
    pub slot_losses: [i32; MAX_SLOTS],
    pub slot_faction: [i32; MAX_SLOTS],
    pub slot_color: [i32; MAX_SLOTS],
}

impl Default for GameSpyStagingRoom {
    fn default() -> Self {
        Self {
            id: 0,
            name: String::new(),
            has_password: false,
            ladder_ip: AsciiString::new(),
            ladder_port: 0,
            map_name: AsciiString::new(),
            max_players: 0,
            num_players: 0,
            num_observers: 0,
            allow_observers: false,
            use_stats: false,
            exe_crc: 0,
            ini_crc: 0,
            version: 0,
            host_ping: AsciiString::new(),
            ping_string: AsciiString::new(),
            player_names: std::array::from_fn(|_| AsciiString::new()),
            slot_profiles: [0; MAX_SLOTS],
            slot_wins: [0; MAX_SLOTS],
            slot_losses: [0; MAX_SLOTS],
            slot_faction: [0; MAX_SLOTS],
            slot_color: [0; MAX_SLOTS],
        }
    }
}

#[derive(Debug, Clone)]
pub struct ChatEntry {
    pub text: String,
    pub color: Color,
    pub window_id: Option<u32>,
}

#[derive(Debug, Default)]
pub struct GameSpyInfo {
    group_rooms: HashMap<i32, GameSpyGroupRoom>,
    got_group_rooms: bool,
    current_group_room: i32,
    player_info_map: BTreeMap<String, PlayerInfo>,
    buddy_map: HashMap<GPProfile, BuddyInfo>,
    buddy_request_map: HashMap<GPProfile, BuddyInfo>,
    buddy_messages: VecDeque<BuddyMessage>,
    local_name: AsciiString,
    local_base_name: AsciiString,
    local_profile_id: i32,
    local_email: AsciiString,
    local_password: AsciiString,
    cached_stats: Option<super::persistent_storage_thread::PSPlayerStats>,
    staging_rooms: HashMap<i32, GameSpyStagingRoom>,
    staging_rooms_changed: bool,
    current_staging_room: Option<i32>,
    staging_room_host: bool,
    staging_room_joiner: bool,
    staging_room_list_complete: bool,
    disallow_asian_text: bool,
    disallow_non_asian_text: bool,
    motd: AsciiString,
    config: AsciiString,
    ping_string: AsciiString,
    ignored_names: HashSet<String>,
    saved_ignore: HashMap<i32, AsciiString>,
    internal_ip: u32,
    external_ip: u32,
    disconnected_after_game_start: Option<i32>,
    preorder_profiles: HashSet<i32>,
    max_messages_per_update: i32,
    additional_disconnects: i32,
    chat_entries: VecDeque<ChatEntry>,
    registered_text_windows: HashSet<u32>,
}

impl GameSpyInfo {
    pub fn new() -> Self {
        Self {
            max_messages_per_update: 10,
            ..Default::default()
        }
    }

    pub fn reset(&mut self) {
        *self = GameSpyInfo::new();
    }

    pub fn clear_group_room_list(&mut self) {
        self.group_rooms.clear();
        self.got_group_rooms = false;
    }

    pub fn get_group_room_list(&self) -> &HashMap<i32, GameSpyGroupRoom> {
        &self.group_rooms
    }

    pub fn add_group_room(&mut self, room: GameSpyGroupRoom) {
        self.group_rooms.insert(room.group_id, room);
        self.got_group_rooms = true;
    }

    pub fn got_group_room_list(&self) -> bool {
        self.got_group_rooms
    }

    pub fn join_group_room(&mut self, group_id: i32) {
        self.current_group_room = group_id;
    }

    pub fn leave_group_room(&mut self) {
        self.current_group_room = 0;
    }

    pub fn join_best_group_room(&mut self) {
        if self.current_group_room != 0 {
            return;
        }
        if let Some(room) = self
            .group_rooms
            .values()
            .max_by_key(|room| room.num_waiting)
        {
            self.current_group_room = room.group_id;
        }
    }

    pub fn set_current_group_room(&mut self, group_id: i32) {
        self.current_group_room = group_id;
    }

    pub fn get_current_group_room(&self) -> i32 {
        self.current_group_room
    }

    pub fn update_player_info(&mut self, info: PlayerInfo, old_nick: Option<AsciiString>) {
        if let Some(old) = old_nick {
            self.player_info_map.remove(&old.as_str().to_lowercase());
        }
        let key = info.name.as_str().to_lowercase();
        self.player_info_map.insert(key, info);
    }

    pub fn player_left_group_room(&mut self, nick: AsciiString) {
        let key = nick.as_str().to_lowercase();
        self.player_info_map.remove(&key);
    }

    pub fn get_player_info_map(&self) -> &BTreeMap<String, PlayerInfo> {
        &self.player_info_map
    }

    pub fn update_player_stats(
        &mut self,
        profile_id: i32,
        wins: i32,
        losses: i32,
        rank_points: i32,
        side: i32,
    ) -> Option<PlayerInfo> {
        for info in self.player_info_map.values_mut() {
            if info.profile_id == profile_id {
                info.wins = wins;
                info.losses = losses;
                info.rank_points = rank_points;
                info.side = side;
                return Some(info.clone());
            }
        }
        None
    }

    pub fn clear_player_info(&mut self) {
        self.player_info_map.clear();
    }

    pub fn get_buddy_map(&self) -> &HashMap<GPProfile, BuddyInfo> {
        &self.buddy_map
    }

    pub fn get_buddy_request_map(&self) -> &HashMap<GPProfile, BuddyInfo> {
        &self.buddy_request_map
    }

    pub fn get_buddy_messages(&self) -> &VecDeque<BuddyMessage> {
        &self.buddy_messages
    }

    pub fn push_buddy_message(&mut self, message: BuddyMessage) {
        self.buddy_messages.push_back(message);
    }

    pub fn clear_buddy_messages(&mut self) {
        self.buddy_messages.clear();
    }

    pub fn add_buddy_request(
        &mut self,
        profile: GPProfile,
        nick: String,
        email: String,
        country_code: String,
    ) {
        let info = BuddyInfo {
            id: profile,
            name: AsciiString::from(&nick),
            email: AsciiString::from(&email),
            country_code: AsciiString::from(&country_code),
            status: GameSpyBuddyStatus::Online,
            status_string: String::new(),
            location_string: String::new(),
        };
        self.buddy_request_map.insert(profile, info);
    }

    pub fn clear_buddy_requests(&mut self) {
        self.buddy_request_map.clear();
    }

    pub fn remove_buddy(&mut self, profile: GPProfile) {
        self.buddy_map.remove(&profile);
    }

    pub fn remove_buddy_request(&mut self, profile: GPProfile) {
        self.buddy_request_map.remove(&profile);
    }

    pub fn update_buddy_status(
        &mut self,
        profile: GPProfile,
        nick: String,
        email: String,
        country_code: String,
        location: String,
        status_value: i32,
        status_string: String,
    ) {
        let status = match status_value {
            0 => GameSpyBuddyStatus::Offline,
            1 => GameSpyBuddyStatus::Online,
            2 => GameSpyBuddyStatus::Lobby,
            3 => GameSpyBuddyStatus::Staging,
            4 => GameSpyBuddyStatus::Loading,
            5 => GameSpyBuddyStatus::Playing,
            6 => GameSpyBuddyStatus::Matching,
            _ => GameSpyBuddyStatus::Online,
        };
        let info = BuddyInfo {
            id: profile,
            name: AsciiString::from(&nick),
            email: AsciiString::from(&email),
            country_code: AsciiString::from(&country_code),
            status,
            status_string,
            location_string: location,
        };
        self.buddy_map.insert(profile, info);
    }

    pub fn is_buddy(&self, id: GPProfile) -> bool {
        self.buddy_map.contains_key(&id)
    }

    pub fn set_local_name(&mut self, name: AsciiString) {
        self.local_name = name;
    }

    pub fn get_local_name(&self) -> AsciiString {
        self.local_name.clone()
    }

    pub fn set_local_base_name(&mut self, name: AsciiString) {
        self.local_base_name = name;
    }

    pub fn get_local_base_name(&self) -> AsciiString {
        self.local_base_name.clone()
    }

    pub fn set_local_profile_id(&mut self, profile_id: i32) {
        self.local_profile_id = profile_id;
    }

    pub fn get_local_profile_id(&self) -> i32 {
        self.local_profile_id
    }

    pub fn set_local_email(&mut self, email: AsciiString) {
        self.local_email = email;
    }

    pub fn get_local_email(&self) -> AsciiString {
        self.local_email.clone()
    }

    pub fn set_local_password(&mut self, passwd: AsciiString) {
        self.local_password = passwd;
    }

    pub fn get_local_password(&self) -> AsciiString {
        self.local_password.clone()
    }

    pub fn set_cached_local_player_stats(
        &mut self,
        stats: super::persistent_storage_thread::PSPlayerStats,
    ) {
        self.cached_stats = Some(stats);
    }

    pub fn get_cached_local_player_stats(
        &self,
    ) -> Option<super::persistent_storage_thread::PSPlayerStats> {
        self.cached_stats.clone()
    }

    pub fn clear_staging_room_list(&mut self) {
        self.staging_rooms.clear();
        self.staging_rooms_changed = true;
    }

    pub fn get_staging_room_list(&self) -> &HashMap<i32, GameSpyStagingRoom> {
        &self.staging_rooms
    }

    pub fn find_staging_room_by_id(&self, id: i32) -> Option<&GameSpyStagingRoom> {
        self.staging_rooms.get(&id)
    }

    pub fn add_staging_room(&mut self, room: GameSpyStagingRoom) {
        self.staging_rooms.insert(room.id, room);
        self.staging_rooms_changed = true;
    }

    pub fn update_staging_room(&mut self, room: GameSpyStagingRoom) {
        self.staging_rooms.insert(room.id, room);
        self.staging_rooms_changed = true;
    }

    pub fn remove_staging_room(&mut self, room: &GameSpyStagingRoom) {
        self.staging_rooms.remove(&room.id);
        self.staging_rooms_changed = true;
    }

    pub fn has_staging_room_list_changed(&mut self) -> bool {
        let changed = self.staging_rooms_changed;
        self.staging_rooms_changed = false;
        changed
    }

    pub fn leave_staging_room(&mut self) {
        self.current_staging_room = None;
        self.staging_room_host = false;
        self.staging_room_joiner = false;
    }

    pub fn mark_as_staging_room_host(&mut self) {
        self.staging_room_host = true;
        self.staging_room_joiner = false;
    }

    pub fn mark_as_staging_room_joiner(&mut self, game: i32) {
        self.current_staging_room = Some(game);
        self.staging_room_joiner = true;
        self.staging_room_host = false;
    }

    pub fn saw_full_game_list(&mut self) {
        self.staging_room_list_complete = true;
    }

    pub fn am_i_host(&self) -> bool {
        self.staging_room_host
    }

    pub fn get_current_staging_room(&self) -> Option<&GameSpyStagingRoom> {
        self.current_staging_room
            .and_then(|id| self.staging_rooms.get(&id))
    }

    pub fn get_current_staging_room_id(&self) -> i32 {
        self.current_staging_room.unwrap_or(0)
    }

    pub fn set_disallow_asian_text(&mut self, val: bool) {
        self.disallow_asian_text = val;
    }

    pub fn set_disallow_non_asian_text(&mut self, val: bool) {
        self.disallow_non_asian_text = val;
    }

    pub fn get_disallow_asian_text(&self) -> bool {
        self.disallow_asian_text
    }

    pub fn get_disallow_non_asian_text(&self) -> bool {
        self.disallow_non_asian_text
    }

    pub fn get_max_messages_per_update(&self) -> i32 {
        self.max_messages_per_update
    }

    pub fn register_text_window(&mut self, window_id: u32) {
        self.registered_text_windows.insert(window_id);
    }

    pub fn unregister_text_window(&mut self, window_id: u32) {
        self.registered_text_windows.remove(&window_id);
    }

    pub fn add_text(&mut self, message: String, color: Color, window_id: Option<u32>) -> usize {
        let entry = ChatEntry {
            text: message,
            color,
            window_id,
        };
        self.chat_entries.push_back(entry);
        self.chat_entries.len().saturating_sub(1)
    }

    pub fn drain_chat_entries(&mut self) -> VecDeque<ChatEntry> {
        std::mem::take(&mut self.chat_entries)
    }

    pub fn add_chat_from_player(
        &mut self,
        player: &PlayerInfo,
        message: String,
        is_public: bool,
        is_action: bool,
        window_id: Option<u32>,
    ) {
        let prefix = if is_action { "* " } else { "" };
        let scope = if is_public { "" } else { "(private) " };
        let text = format!("{}{}{}: {}", prefix, scope, player.name.as_str(), message);
        let color = if player.profile_id == self.local_profile_id {
            default_gamespy_colors()[GameSpyColor::ChatSelf as usize]
        } else if self.is_buddy(player.profile_id) {
            default_gamespy_colors()[GameSpyColor::ChatBuddy as usize]
        } else {
            default_gamespy_colors()[GameSpyColor::ChatNormal as usize]
        };
        self.add_text(text, color, window_id);
    }

    pub fn add_chat(
        &mut self,
        nick: AsciiString,
        profile_id: i32,
        message: String,
        is_public: bool,
        is_action: bool,
        window_id: Option<u32>,
    ) {
        let mut player = PlayerInfo::default();
        player.name = nick;
        player.profile_id = profile_id;
        self.add_chat_from_player(&player, message, is_public, is_action, window_id);
    }

    pub fn send_chat(&mut self, message: String, is_action: bool, window_id: Option<u32>) -> bool {
        if message.trim().is_empty() {
            return false;
        }
        self.add_chat(
            self.local_name.clone(),
            self.local_profile_id,
            message,
            true,
            is_action,
            window_id,
        );
        true
    }

    pub fn set_motd(&mut self, motd: AsciiString) {
        self.motd = motd;
    }

    pub fn get_motd(&self) -> AsciiString {
        self.motd.clone()
    }

    pub fn set_config(&mut self, config: AsciiString) {
        self.config = config;
    }

    pub fn get_config(&self) -> AsciiString {
        self.config.clone()
    }

    pub fn set_ping_string(&mut self, ping: AsciiString) {
        self.ping_string = ping;
    }

    pub fn get_ping_string(&self) -> AsciiString {
        self.ping_string.clone()
    }

    pub fn get_ping_value(&self, other_ping: &AsciiString) -> i32 {
        let bytes = other_ping.as_str().as_bytes();
        if bytes.len() < 2 {
            return 0;
        }
        let mut total = 0;
        let mut count = 0;
        for chunk in bytes.chunks(2) {
            if chunk.len() < 2 {
                break;
            }
            if let Ok(val) = u8::from_str_radix(std::str::from_utf8(chunk).unwrap_or("ff"), 16) {
                if val != 0xFF {
                    total += val as i32;
                    count += 1;
                }
            }
        }
        if count == 0 {
            0
        } else {
            total / count
        }
    }

    pub fn add_to_saved_ignore_list(&mut self, profile_id: i32, nick: AsciiString) {
        self.saved_ignore.insert(profile_id, nick);
    }

    pub fn remove_from_saved_ignore_list(&mut self, profile_id: i32) {
        self.saved_ignore.remove(&profile_id);
    }

    pub fn is_saved_ignored(&self, profile_id: i32) -> bool {
        self.saved_ignore.contains_key(&profile_id)
    }

    pub fn return_saved_ignore_list(&self) -> HashMap<i32, AsciiString> {
        self.saved_ignore.clone()
    }

    pub fn load_saved_ignore_list(&mut self) {
        // Stored in memory for now; filesystem persistence handled elsewhere.
    }

    pub fn return_ignore_list(&self) -> HashSet<String> {
        self.ignored_names.clone()
    }

    pub fn add_to_ignore_list(&mut self, nick: AsciiString) {
        self.ignored_names.insert(nick.as_str().to_lowercase());
    }

    pub fn remove_from_ignore_list(&mut self, nick: AsciiString) {
        self.ignored_names.remove(&nick.as_str().to_lowercase());
    }

    pub fn is_ignored(&self, nick: AsciiString) -> bool {
        self.ignored_names.contains(&nick.as_str().to_lowercase())
    }

    pub fn set_local_ips(&mut self, internal_ip: u32, external_ip: u32) {
        self.internal_ip = internal_ip;
        self.external_ip = external_ip;
    }

    pub fn get_internal_ip(&self) -> u32 {
        self.internal_ip
    }

    pub fn get_external_ip(&self) -> u32 {
        self.external_ip
    }

    pub fn is_disconnected_after_game_start(&self) -> Option<i32> {
        self.disconnected_after_game_start
    }

    pub fn mark_as_disconnected_after_game_start(&mut self, reason: i32) {
        self.disconnected_after_game_start = Some(reason);
    }

    pub fn did_player_preorder(&self, profile_id: i32) -> bool {
        self.preorder_profiles.contains(&profile_id)
    }

    pub fn mark_player_as_preorder(&mut self, profile_id: i32) {
        self.preorder_profiles.insert(profile_id);
    }

    pub fn set_max_messages_per_update(&mut self, num: i32) {
        self.max_messages_per_update = num.max(1);
    }

    pub fn get_additional_disconnects(&self) -> i32 {
        self.additional_disconnects
    }

    pub fn clear_additional_disconnects(&mut self) {
        self.additional_disconnects = 0;
    }

    pub fn read_additional_disconnects(&mut self) {
        // Placeholder for file-based read; keep memory value.
    }

    pub fn update_additional_game_spy_disconnections(&mut self, count: i32) {
        self.additional_disconnects = count;
    }
}

fn now_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

static THE_GAMESPY_INFO: OnceLock<Arc<Mutex<GameSpyInfo>>> = OnceLock::new();

pub fn init_gamespy_info() -> Arc<Mutex<GameSpyInfo>> {
    THE_GAMESPY_INFO
        .get_or_init(|| Arc::new(Mutex::new(GameSpyInfo::new())))
        .clone()
}

pub fn get_gamespy_info() -> Option<Arc<Mutex<GameSpyInfo>>> {
    THE_GAMESPY_INFO.get().cloned()
}

pub fn teardown_gamespy_info() {
    if let Some(info) = THE_GAMESPY_INFO.get() {
        if let Ok(mut guard) = info.lock() {
            guard.reset();
        }
    }
}

pub fn set_up_gamespy(motd: &str, config: &str) {
    let info = init_gamespy_info();
    if let Ok(mut guard) = info.lock() {
        guard.set_motd(AsciiString::from(motd));
        guard.set_config(AsciiString::from(config));
        let (disallow_asian, disallow_non_asian) = read_disallow_text_prefs();
        guard.set_disallow_asian_text(disallow_asian);
        guard.set_disallow_non_asian_text(disallow_non_asian);
    }
    super::peer_thread::init_peer_message_queue();
    super::buddy_thread::init_buddy_message_queue();
    super::persistent_storage_thread::init_ps_message_queue();
}

fn read_disallow_text_prefs() -> (bool, bool) {
    let prefs = game_engine::common::preferences::CustomMatchPreferences::new();
    (
        prefs.get_disallow_asian_text(),
        prefs.get_disallow_non_asian_text(),
    )
}

pub fn tear_down_gamespy() {
    teardown_gamespy_info();
    super::peer_thread::teardown_peer_message_queue();
    super::buddy_thread::teardown_buddy_message_queue();
    super::persistent_storage_thread::teardown_ps_message_queue();
}

impl BuddyMessage {
    pub fn new(
        sender_id: GPProfile,
        sender_nick: AsciiString,
        recipient_id: GPProfile,
        recipient_nick: AsciiString,
        message: String,
    ) -> Self {
        Self {
            timestamp: now_timestamp(),
            sender_id,
            sender_nick,
            recipient_id,
            recipient_nick,
            message,
        }
    }
}
