// FILE: game_info/mod.rs
// Port of GameInfo.cpp/GameInfo.h - Game setup state information
// Author: Rust port, original by Matthew D. Campbell, December 2001
//
// Maintains information about the game setup and slot list throughout the game.
// Matches C++ behavior exactly for network compatibility.

use once_cell::sync::OnceCell;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

/// Maximum number of player slots (matches C++ MAX_SLOTS = 8)
pub const MAX_SLOTS: usize = 8;

/// Player template constants
pub const PLAYERTEMPLATE_RANDOM: i32 = -1;
pub const PLAYERTEMPLATE_OBSERVER: i32 = -2;
pub const PLAYERTEMPLATE_MIN: i32 = PLAYERTEMPLATE_OBSERVER;

/// Default network CRC interval (matches C++ NET_CRC_INTERVAL)
pub const NET_CRC_INTERVAL: i32 = 100;

/// Maximum LAN options string length (matches C++ m_lanMaxOptionsLength)
pub const LAN_MAX_OPTIONS_LENGTH: usize = 1400;

type MapPlayersProvider = Arc<dyn Fn(&str) -> Option<i32> + Send + Sync>;

static MAP_PLAYERS_PROVIDER: OnceCell<MapPlayersProvider> = OnceCell::new();

pub fn set_map_players_provider(provider: MapPlayersProvider) -> bool {
    MAP_PLAYERS_PROVIDER.set(provider).is_ok()
}

fn lookup_map_players(map_name: &str) -> Option<i32> {
    let normalized = map_name.to_lowercase();
    MAP_PLAYERS_PROVIDER
        .get()
        .and_then(|provider| (provider)(&normalized))
}

#[derive(Debug, Clone)]
pub struct MultiplayerSettingsView {
    pub show_random_player_template: bool,
    pub show_random_start_pos: bool,
    pub show_random_color: bool,
    pub observer_color: Option<i32>,
    pub random_color: Option<i32>,
    pub color_values: Vec<i32>,
}

type MultiplayerSettingsProvider = Arc<dyn Fn() -> MultiplayerSettingsView + Send + Sync>;

static MULTIPLAYER_SETTINGS_PROVIDER: OnceCell<MultiplayerSettingsProvider> = OnceCell::new();

pub fn set_multiplayer_settings_provider(provider: MultiplayerSettingsProvider) -> bool {
    MULTIPLAYER_SETTINGS_PROVIDER.set(provider).is_ok()
}

fn lookup_multiplayer_settings() -> Option<MultiplayerSettingsView> {
    MULTIPLAYER_SETTINGS_PROVIDER
        .get()
        .map(|provider| (provider)())
}

type GameTextProvider = Arc<dyn Fn(&str) -> String + Send + Sync>;

static GAME_TEXT_PROVIDER: OnceCell<GameTextProvider> = OnceCell::new();

pub fn set_game_text_provider(provider: GameTextProvider) -> bool {
    GAME_TEXT_PROVIDER.set(provider).is_ok()
}

fn lookup_game_text(key: &str) -> String {
    GAME_TEXT_PROVIDER
        .get()
        .map(|provider| (provider)(key))
        .unwrap_or_else(|| key.to_string())
}

type PlayerTemplateDisplayNameProvider = Arc<dyn Fn(i32) -> Option<String> + Send + Sync>;

static PLAYER_TEMPLATE_DISPLAY_NAME_PROVIDER: OnceCell<PlayerTemplateDisplayNameProvider> =
    OnceCell::new();

pub fn set_player_template_display_name_provider(
    provider: PlayerTemplateDisplayNameProvider,
) -> bool {
    PLAYER_TEMPLATE_DISPLAY_NAME_PROVIDER.set(provider).is_ok()
}

fn lookup_player_template_display_name(index: i32) -> Option<String> {
    PLAYER_TEMPLATE_DISPLAY_NAME_PROVIDER
        .get()
        .and_then(|provider| (provider)(index))
}

#[derive(Debug, Clone, Copy)]
struct LocalSlotView {
    local_ip: u32,
    local_team: i32,
    local_orig_player_template: i32,
    has_local: bool,
}

impl Default for LocalSlotView {
    fn default() -> Self {
        Self {
            local_ip: 0,
            local_team: -1,
            local_orig_player_template: PLAYERTEMPLATE_RANDOM,
            has_local: false,
        }
    }
}

static LOCAL_SLOT_VIEW: OnceCell<RwLock<LocalSlotView>> = OnceCell::new();

fn local_slot_view_cell() -> &'static RwLock<LocalSlotView> {
    LOCAL_SLOT_VIEW.get_or_init(|| RwLock::new(LocalSlotView::default()))
}

fn update_local_slot_view(view: LocalSlotView) {
    if let Ok(mut guard) = local_slot_view_cell().write() {
        *guard = view;
    }
}

fn get_local_slot_view() -> LocalSlotView {
    local_slot_view_cell()
        .read()
        .map(|guard| *guard)
        .unwrap_or_default()
}

fn is_slot_local_ally(slot: &GameSlot) -> bool {
    let view = get_local_slot_view();
    if !view.has_local {
        return false;
    }

    if slot.is_player_by_ip(view.local_ip) {
        return true;
    }

    if slot.get_team_number() >= 0 && slot.get_team_number() == view.local_team {
        return true;
    }

    if view.local_orig_player_template == PLAYERTEMPLATE_OBSERVER {
        return true;
    }

    false
}

/// Slot states (matches C++ SlotState enum)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
#[derive(Default)]
pub enum SlotState {
    Open = 0,
    #[default]
    Closed = 1,
    EasyAI = 2,
    MedAI = 3,
    BrutalAI = 4,
    Player = 5,
}


/// Firewall/NAT behavior types (matches C++ FirewallHelperClass::FirewallBehaviorType)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(u8)]
#[derive(Default)]
pub enum FirewallBehaviorType {
    Unknown = 0,
    #[default]
    Simple = 1,
    DumbMangling = 2,
    SmartMangling = 4,
    NetgearBug = 8,
    SimplePortAllocation = 16,
    RelativePortAllocation = 32,
    DestinationPortDelta = 64,
}


/// Game slot - maintains information about the contents of a game slot
/// Matches C++ GameSlot class exactly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSlot {
    state: SlotState,
    is_accepted: bool,
    has_map: bool,
    is_muted: bool,
    color: i32,           // -1 for random
    start_pos: i32,       // -1 for random
    player_template: i32, // PlayerTemplate index
    team_number: i32,     // -1 for none
    orig_color: i32,
    orig_start_pos: i32,
    orig_player_template: i32,
    name: String,
    ip: u32,
    port: u16,
    nat_behavior: FirewallBehaviorType,
    last_frame_in_game: u32,
    disconnected: bool,
}

impl GameSlot {
    /// Create a new empty game slot
    pub fn new() -> Self {
        Self {
            state: SlotState::Closed,
            is_accepted: false,
            has_map: true,
            is_muted: false,
            color: -1,
            start_pos: -1,
            player_template: -1,
            team_number: -1,
            orig_color: -1,
            orig_start_pos: -1,
            orig_player_template: -1,
            name: String::new(),
            ip: 0,
            port: 0,
            nat_behavior: FirewallBehaviorType::Simple,
            last_frame_in_game: 0,
            disconnected: false,
        }
    }

    /// Reset slot to default state (matches C++ GameSlot::reset)
    pub fn reset(&mut self) {
        self.state = SlotState::Closed;
        self.is_accepted = false;
        self.has_map = true;
        self.color = -1;
        self.start_pos = -1;
        self.player_template = -1;
        self.team_number = -1;
        self.nat_behavior = FirewallBehaviorType::Simple;
        self.last_frame_in_game = 0;
        self.disconnected = false;
        self.port = 0;
        self.is_muted = false;
        self.orig_player_template = -1;
        self.orig_start_pos = -1;
        self.orig_color = -1;
        self.name.clear();
        self.ip = 0;
    }

    /// Accept the current options (matches C++ setAccept)
    pub fn set_accept(&mut self) {
        self.is_accepted = true;
    }

    /// Unaccept - options changed (matches C++ unAccept)
    pub fn un_accept(&mut self) {
        if self.is_human() {
            self.is_accepted = false;
        }
    }

    /// Get acceptance status
    pub fn is_accepted(&self) -> bool {
        self.is_accepted
    }

    /// Set map availability (matches C++ setMapAvailability)
    pub fn set_map_availability(&mut self, has_map: bool) {
        if self.is_human() {
            self.has_map = has_map;
        }
    }

    /// Check if slot has the map
    pub fn has_map(&self) -> bool {
        self.has_map
    }

    /// Set slot state (matches C++ setState)
    pub fn set_state(&mut self, state: SlotState, name: String, ip: u32) {
        // Don't reset AI settings if just changing AI difficulty
        let preserve_ai_settings = self.is_ai()
            && matches!(
                state,
                SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI
            );

        if !preserve_ai_settings {
            self.color = -1;
            self.start_pos = -1;
            self.player_template = -1;
            self.team_number = -1;
        }

        if state == SlotState::Player {
            self.reset();
            self.state = state;
            self.name = name;
        } else {
            self.state = state;
            self.is_accepted = true;
            self.has_map = true;

            self.name = match state {
                SlotState::Open => lookup_game_text("GUI:Open"),
                SlotState::EasyAI => lookup_game_text("GUI:EasyAI"),
                SlotState::MedAI => lookup_game_text("GUI:MediumAI"),
                SlotState::BrutalAI => lookup_game_text("GUI:HardAI"),
                SlotState::Closed => lookup_game_text("GUI:Closed"),
                _ => String::new(),
            };
        }

        self.ip = ip;
    }

    pub fn get_state(&self) -> SlotState {
        self.state
    }
    pub fn set_color(&mut self, color: i32) {
        self.color = color;
    }
    pub fn get_color(&self) -> i32 {
        self.color
    }
    pub fn set_start_pos(&mut self, start_pos: i32) {
        self.start_pos = start_pos;
    }
    pub fn get_start_pos(&self) -> i32 {
        self.start_pos
    }

    pub fn set_player_template(&mut self, player_template: i32) {
        self.player_template = player_template;
        if player_template <= PLAYERTEMPLATE_MIN {
            self.start_pos = -1;
        }
    }

    pub fn get_player_template(&self) -> i32 {
        self.player_template
    }
    pub fn set_team_number(&mut self, team_number: i32) {
        self.team_number = team_number;
    }
    pub fn get_team_number(&self) -> i32 {
        self.team_number
    }
    pub fn set_name(&mut self, name: String) {
        self.name = name;
    }
    pub fn get_name(&self) -> &str {
        &self.name
    }
    pub fn set_ip(&mut self, ip: u32) {
        self.ip = ip;
    }
    pub fn get_ip(&self) -> u32 {
        self.ip
    }
    pub fn set_port(&mut self, port: u16) {
        self.port = port;
    }
    pub fn get_port(&self) -> u16 {
        self.port
    }
    pub fn set_nat_behavior(&mut self, nat_behavior: FirewallBehaviorType) {
        self.nat_behavior = nat_behavior;
    }
    pub fn get_nat_behavior(&self) -> FirewallBehaviorType {
        self.nat_behavior
    }

    /// Save off original info (matches C++ saveOffOriginalInfo)
    pub fn save_off_original_info(&mut self) {
        self.orig_player_template = self.player_template;
        self.orig_start_pos = self.start_pos;
        self.orig_color = self.color;
    }

    pub fn get_original_player_template(&self) -> i32 {
        self.orig_player_template
    }
    pub fn get_original_color(&self) -> i32 {
        self.orig_color
    }
    pub fn get_original_start_pos(&self) -> i32 {
        self.orig_start_pos
    }

    /// Get apparent player template (may hide actual value from non-allies)
    pub fn get_apparent_player_template(&self) -> i32 {
        if let Some(settings) = lookup_multiplayer_settings() {
            if settings.show_random_player_template && !is_slot_local_ally(self) {
                return self.orig_player_template;
            }
        }

        self.player_template
    }

    pub fn get_apparent_player_template_display_name(&self) -> String {
        if let Some(settings) = lookup_multiplayer_settings() {
            if settings.show_random_player_template
                && self.orig_player_template == PLAYERTEMPLATE_RANDOM
                && !is_slot_local_ally(self)
            {
                return lookup_game_text("GUI:Random");
            }
        }

        if self.orig_player_template == PLAYERTEMPLATE_OBSERVER {
            return lookup_game_text("GUI:Observer");
        }

        if self.player_template < 0 {
            return lookup_game_text("GUI:Random");
        }

        lookup_player_template_display_name(self.player_template)
            .unwrap_or_else(|| lookup_game_text("GUI:Random"))
    }

    pub fn get_apparent_color(&self) -> i32 {
        if let Some(settings) = lookup_multiplayer_settings() {
            if self.orig_player_template == PLAYERTEMPLATE_OBSERVER {
                if let Some(observer_color) = settings.observer_color {
                    return observer_color;
                }
            }

            if settings.show_random_color && !is_slot_local_ally(self) {
                return self.orig_color;
            }
        }

        self.color
    }

    pub fn get_apparent_start_pos(&self) -> i32 {
        if let Some(settings) = lookup_multiplayer_settings() {
            if settings.show_random_start_pos && !is_slot_local_ally(self) {
                return self.orig_start_pos;
            }
        }

        self.start_pos
    }

    /// Is this slot occupied by a human player?
    pub fn is_human(&self) -> bool {
        self.state == SlotState::Player
    }

    /// Is this slot occupied (by a human or an AI)?
    pub fn is_occupied(&self) -> bool {
        matches!(
            self.state,
            SlotState::Player | SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI
        )
    }

    /// Is this slot occupied by an AI?
    pub fn is_ai(&self) -> bool {
        matches!(
            self.state,
            SlotState::EasyAI | SlotState::MedAI | SlotState::BrutalAI
        )
    }

    /// Does this slot contain the given user?
    pub fn is_player_by_name(&self, user_name: &str) -> bool {
        self.state == SlotState::Player && self.name.eq_ignore_ascii_case(user_name)
    }

    /// Is this slot at this IP?
    pub fn is_player_by_ip(&self, ip: u32) -> bool {
        self.state == SlotState::Player && self.ip == ip
    }

    /// Is this slot open?
    pub fn is_open(&self) -> bool {
        self.state == SlotState::Open
    }

    pub fn set_last_frame_in_game(&mut self, frame: u32) {
        self.last_frame_in_game = frame;
    }

    pub fn mark_as_disconnected(&mut self) {
        self.disconnected = true;
    }

    pub fn last_frame_in_game(&self) -> u32 {
        self.last_frame_in_game
    }

    pub fn disconnected(&self) -> bool {
        self.is_human() && self.disconnected
    }

    pub fn mute(&mut self, is_muted: bool) {
        self.is_muted = is_muted;
    }

    pub fn is_muted(&self) -> bool {
        self.is_muted
    }
}

impl Default for GameSlot {
    fn default() -> Self {
        Self::new()
    }
}

/// Money type (simple wrapper for starting cash)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Money {
    amount: u32,
}

impl Money {
    pub fn new(amount: u32) -> Self {
        Self { amount }
    }

    pub fn init(&mut self) {
        self.amount = 0;
    }

    pub fn deposit(&mut self, amount: u32) {
        self.amount = self.amount.saturating_add(amount);
    }

    pub fn count_money(&self) -> u32 {
        self.amount
    }
}

impl Default for Money {
    fn default() -> Self {
        Self::new(10000) // Default starting cash
    }
}

/// Game info - maintains information about the game setup and slot list
/// Matches C++ GameInfo class exactly
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameInfo {
    pub(crate) preorder_mask: i32,
    crc_interval: i32,
    in_game: bool,
    in_progress: bool,
    surrendered: bool,
    pub(crate) game_id: i32,
    slots: Vec<GameSlot>,
    local_ip: u32,

    // Game options
    map_name: String,
    map_crc: u32,
    map_size: u32,
    map_mask: i32,
    seed: i32,
    use_stats: i32,
    starting_cash: Money,
    superweapon_restriction: u16,
    old_factions_only: bool,
}

impl GameInfo {
    /// Create a new GameInfo instance
    pub fn new() -> Self {
        let mut info = Self {
            preorder_mask: 0,
            crc_interval: NET_CRC_INTERVAL,
            in_game: false,
            in_progress: false,
            surrendered: false,
            game_id: 0,
            slots: vec![GameSlot::new(); MAX_SLOTS],
            local_ip: 0,
            map_name: "NOMAP".to_string(),
            map_crc: 0,
            map_size: 0,
            map_mask: 0,
            seed: Self::get_tick_count() as i32,
            use_stats: 1,
            starting_cash: Money::default(),
            superweapon_restriction: 0,
            old_factions_only: false,
        };
        info.init();
        info
    }

    /// Initialize (matches C++ init)
    pub fn init(&mut self) {
        self.reset();
    }

    /// Reset to default state (matches C++ reset)
    pub fn reset(&mut self) {
        self.crc_interval = NET_CRC_INTERVAL;
        self.in_game = false;
        self.in_progress = false;
        self.game_id = 0;
        self.map_name = "NOMAP".to_string();
        self.map_mask = 0;
        self.seed = Self::get_tick_count() as i32;
        self.use_stats = 1;
        self.surrendered = false;
        self.old_factions_only = false;
        self.map_crc = 0;
        self.map_size = 0;
        self.superweapon_restriction = 0;
        self.starting_cash = Money::default();
        self.preorder_mask = 0;

        for slot in &mut self.slots {
            slot.reset();
        }

        self.refresh_local_slot_view();
    }

    /// Platform-specific tick count (matches C++ GetTickCount)
    fn get_tick_count() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64
    }

    /// Clear all slots (matches C++ clearSlotList)
    pub fn clear_slot_list(&mut self) {
        for slot in &mut self.slots {
            slot.set_state(SlotState::Closed, String::new(), 0);
        }
    }

    /// Get number of players (human and AI) in game
    pub fn get_num_players(&self) -> usize {
        self.slots.iter().filter(|s| s.is_occupied()).count()
    }

    /// Get number of non-observer players
    pub fn get_num_non_observer_players(&self) -> usize {
        self.slots
            .iter()
            .filter(|s| s.is_occupied() && s.get_player_template() != PLAYERTEMPLATE_OBSERVER)
            .count()
    }

    /// Get maximum players from map (requires map cache)
    pub fn get_max_players(&self) -> i32 {
        if self.map_name.is_empty() {
            return -1;
        }

        lookup_map_players(&self.map_name).unwrap_or(-1)
    }

    /// Mark as having entered the game
    pub fn enter_game(&mut self) {
        self.reset();
        self.in_game = true;
        self.in_progress = false;
        self.refresh_local_slot_view();
    }

    /// Mark as having left the game
    pub fn leave_game(&mut self) {
        self.reset();
    }

    /// Start the game with given ID
    pub fn start_game(&mut self, game_id: i32) {
        self.game_id = game_id;
        self.close_open_slots();
        self.in_progress = true;
    }

    /// End the game
    pub fn end_game(&mut self) {
        self.in_game = false;
        self.in_progress = false;
    }

    pub fn get_game_id(&self) -> i32 {
        self.game_id
    }
    pub fn set_in_game(&mut self) {
        self.in_game = true;
    }
    pub fn is_in_game(&self) -> bool {
        self.in_game
    }
    pub fn is_game_in_progress(&self) -> bool {
        self.in_progress
    }
    pub fn set_game_in_progress(&mut self, in_progress: bool) {
        self.in_progress = in_progress;
    }

    /// Set slot information
    pub fn set_slot(&mut self, slot_num: usize, slot_info: GameSlot) {
        if slot_num >= MAX_SLOTS {
            return;
        }

        let mut slot = slot_info;

        // Host is always accepted and has map
        if slot_num == 0 {
            slot.set_accept();
            slot.set_map_availability(true);
        }

        self.slots[slot_num] = slot;
        self.refresh_local_slot_view();
    }

    /// Get mutable slot reference
    pub fn get_slot_mut(&mut self, slot_num: usize) -> Option<&mut GameSlot> {
        if slot_num >= MAX_SLOTS {
            None
        } else {
            Some(&mut self.slots[slot_num])
        }
    }

    /// Get immutable slot reference
    pub fn get_slot(&self, slot_num: usize) -> Option<&GameSlot> {
        if slot_num >= MAX_SLOTS {
            None
        } else {
            Some(&self.slots[slot_num])
        }
    }

    /// Is the local player the host?
    pub fn am_i_host(&self) -> bool {
        if !self.in_game {
            return false;
        }

        self.slots.first()
            .map(|s| s.is_player_by_ip(self.local_ip))
            .unwrap_or(false)
    }

    /// Get local slot number
    pub fn get_local_slot_num(&self) -> i32 {
        if !self.in_game {
            return -1;
        }

        for (i, slot) in self.slots.iter().enumerate() {
            if slot.is_player_by_ip(self.local_ip) {
                return i as i32;
            }
        }
        -1
    }

    fn refresh_local_slot_view(&self) {
        let mut view = LocalSlotView {
            local_ip: self.local_ip,
            local_team: -1,
            local_orig_player_template: PLAYERTEMPLATE_RANDOM,
            has_local: false,
        };

        for slot in &self.slots {
            if slot.is_player_by_ip(self.local_ip) {
                view.local_team = slot.get_team_number();
                view.local_orig_player_template = slot.get_original_player_template();
                view.has_local = true;
                break;
            }
        }

        update_local_slot_view(view);
    }

    /// Get slot number for user name
    pub fn get_slot_num_by_name(&self, user_name: &str) -> i32 {
        if !self.in_game {
            return -1;
        }

        for (i, slot) in self.slots.iter().enumerate() {
            if slot.is_player_by_name(user_name) {
                return i as i32;
            }
        }
        -1
    }

    // Game option getters/setters
    pub fn set_map(&mut self, map_name: String) {
        self.map_name = map_name;
    }
    pub fn get_map(&self) -> &str {
        &self.map_name
    }
    pub fn set_map_crc(&mut self, map_crc: u32) {
        self.map_crc = map_crc;
    }
    pub fn get_map_crc(&self) -> u32 {
        self.map_crc
    }
    pub fn set_map_size(&mut self, map_size: u32) {
        self.map_size = map_size;
    }
    pub fn get_map_size(&self) -> u32 {
        self.map_size
    }
    pub fn set_map_contents_mask(&mut self, mask: i32) {
        self.map_mask = mask;
    }
    pub fn get_map_contents_mask(&self) -> i32 {
        self.map_mask
    }
    pub fn set_seed(&mut self, seed: i32) {
        self.seed = seed;
    }
    pub fn get_seed(&self) -> i32 {
        self.seed
    }
    pub fn get_use_stats(&self) -> i32 {
        self.use_stats
    }
    pub fn set_use_stats(&mut self, use_stats: i32) {
        self.use_stats = use_stats;
    }
    pub fn get_superweapon_restriction(&self) -> u16 {
        self.superweapon_restriction
    }
    pub fn set_superweapon_restriction(&mut self, restriction: u16) {
        self.superweapon_restriction = restriction;
    }
    pub fn get_starting_cash(&self) -> &Money {
        &self.starting_cash
    }
    pub fn set_starting_cash(&mut self, starting_cash: Money) {
        self.starting_cash = starting_cash;
    }
    pub fn old_factions_only(&self) -> bool {
        self.old_factions_only
    }
    pub fn set_old_factions_only(&mut self, old_factions_only: bool) {
        self.old_factions_only = old_factions_only;
    }
    pub fn set_local_ip(&mut self, ip: u32) {
        self.local_ip = ip;
        self.refresh_local_slot_view();
    }
    pub fn get_local_ip(&self) -> u32 {
        self.local_ip
    }
    pub fn set_crc_interval(&mut self, val: i32) {
        self.crc_interval = val.min(100);
    }
    pub fn get_crc_interval(&self) -> i32 {
        self.crc_interval
    }
    pub fn have_we_surrendered(&self) -> bool {
        self.surrendered
    }
    pub fn mark_as_surrendered(&mut self) {
        self.surrendered = true;
    }

    /// Check if color is taken by another slot
    pub fn is_color_taken(&self, color_idx: i32, slot_to_ignore: i32) -> bool {
        for (i, slot) in self.slots.iter().enumerate() {
            if i as i32 != slot_to_ignore && slot.get_color() == color_idx {
                return true;
            }
        }
        false
    }

    /// Check if start position is taken by another slot
    pub fn is_start_position_taken(&self, position_idx: i32, slot_to_ignore: i32) -> bool {
        for (i, slot) in self.slots.iter().enumerate() {
            if i as i32 != slot_to_ignore && slot.get_start_pos() == position_idx {
                return true;
            }
        }
        false
    }

    /// Reset accepted flag on all players (host always accepted)
    pub fn reset_accepted(&mut self) {
        if let Some(slot) = self.slots.get_mut(0) {
            slot.set_accept();
        }

        for slot in self.slots.iter_mut().skip(1) {
            slot.un_accept();
        }
    }

    /// Reset start spots for new map
    pub fn reset_start_spots(&mut self) {
        for slot in &mut self.slots {
            slot.set_start_pos(-1);
        }
    }

    /// Adjust slots for map player count
    pub fn adjust_slots_for_map(&mut self) {
        let max_players = self.get_max_players();
        if max_players <= 0 {
            return;
        }

        let mut num_player_slots =
            self.slots.iter().filter(|slot| slot.is_occupied()).count() as i32;

        for i in 0..MAX_SLOTS {
            let occupied = self.slots[i].is_occupied();
            if max_players > num_player_slots {
                if !occupied {
                    let mut new_slot = GameSlot::new();
                    new_slot.set_state(SlotState::Open, String::new(), 0);
                    self.set_slot(i, new_slot);
                    num_player_slots += 1;
                }
            } else if !occupied {
                let mut new_slot = GameSlot::new();
                new_slot.set_state(SlotState::Closed, String::new(), 0);
                self.set_slot(i, new_slot);
            }
        }
    }

    /// Close all open slots
    pub fn close_open_slots(&mut self) {
        for slot in &mut self.slots {
            if !slot.is_occupied() {
                slot.set_state(SlotState::Closed, String::new(), 0);
            }
        }
    }

    /// Is this a skirmish game? (1 human + 1+ AI, not sandbox)
    pub fn is_skirmish(&self) -> bool {
        let mut saw_ai = false;
        let local_slot = self.get_local_slot_num();
        if local_slot < 0 {
            return false;
        }

        for (i, slot) in self.slots.iter().enumerate() {
            if i as i32 == local_slot {
                continue;
            }

            if slot.is_human() {
                return false;
            }

            if slot.is_ai() {
                if is_slot_local_ally(slot) {
                    return false;
                }
                saw_ai = true;
            }
        }

        saw_ai
    }

    /// Is this a multiplayer game? (2+ humans)
    pub fn is_multiplayer(&self) -> bool {
        let local_slot = self.get_local_slot_num();

        for (i, slot) in self.slots.iter().enumerate() {
            if i as i32 == local_slot {
                continue;
            }

            if slot.is_human() {
                return true;
            }
        }

        false
    }

    /// Is this a sandbox game? (everyone on same team)
    pub fn is_sandbox(&self) -> bool {
        let local_slot = self.get_local_slot_num();
        if local_slot < 0 {
            return false;
        }

        let local_team = self.slots[local_slot as usize].get_team_number();

        for (i, slot) in self.slots.iter().enumerate() {
            if i as i32 == local_slot {
                continue;
            }

            if slot.is_occupied() {
                let team = slot.get_team_number();
                if team < 0 || team != local_team {
                    return false;
                }
            }
        }

        true
    }

    /// Check if player has preorder
    pub fn is_player_preorder(&self, index: usize) -> bool {
        if index >= MAX_SLOTS {
            return false;
        }
        (self.preorder_mask & (1 << index)) != 0
    }

    /// Mark player as preorder
    pub fn mark_player_as_preorder(&mut self, index: usize) {
        if index < MAX_SLOTS {
            self.preorder_mask |= 1 << index;
        }
    }
}

impl Default for GameInfo {
    fn default() -> Self {
        Self::new()
    }
}

// Re-exports
pub use serialization::{game_info_to_ascii_string, parse_ascii_string_to_game_info};
pub use snapshot::{
    GameInfoSnapshot, GameSlotSnapshot, SkirmishGameInfo, SKIRMISH_GAME_INFO_VERSION,
};

mod serialization;
mod snapshot;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_game_slot_default() {
        let slot = GameSlot::new();
        assert_eq!(slot.get_state(), SlotState::Closed);
        assert!(!slot.is_human());
        assert!(!slot.is_ai());
        assert!(!slot.is_occupied());
    }

    #[test]
    fn test_game_slot_player() {
        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, "TestPlayer".to_string(), 0x12345678);

        assert_eq!(slot.get_state(), SlotState::Player);
        assert!(slot.is_human());
        assert!(!slot.is_ai());
        assert!(slot.is_occupied());
        assert_eq!(slot.get_name(), "TestPlayer");
        assert_eq!(slot.get_ip(), 0x12345678);
    }

    #[test]
    fn test_game_slot_ai() {
        // Initialize game text provider for tests
        let _ = set_game_text_provider(Arc::new(|key| match key {
            "GUI:EasyAI" => "Easy AI".to_string(),
            "GUI:MediumAI" => "Medium AI".to_string(),
            "GUI:HardAI" => "Hard AI".to_string(),
            "GUI:Open" => "Open".to_string(),
            "GUI:Closed" => "Closed".to_string(),
            other => other.to_string(),
        }));

        let mut slot = GameSlot::new();
        slot.set_state(SlotState::MedAI, String::new(), 0);

        assert_eq!(slot.get_state(), SlotState::MedAI);
        assert!(!slot.is_human());
        assert!(slot.is_ai());
        assert!(slot.is_occupied());
        assert_eq!(slot.get_name(), "Medium AI");
    }

    #[test]
    fn test_game_info_default() {
        let info = GameInfo::new();
        assert!(!info.is_in_game());
        assert!(!info.is_game_in_progress());
        assert_eq!(info.get_num_players(), 0);
        assert_eq!(info.get_map(), "NOMAP");
    }

    #[test]
    fn test_game_info_slots() {
        let mut info = GameInfo::new();

        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, "Player1".to_string(), 0x11111111);
        info.set_slot(0, slot);

        assert_eq!(info.get_num_players(), 1);

        let retrieved = info.get_slot(0).unwrap();
        assert_eq!(retrieved.get_name(), "Player1");
        assert_eq!(retrieved.get_ip(), 0x11111111);
    }

    #[test]
    fn test_color_and_position_taken() {
        let mut info = GameInfo::new();

        let mut slot = GameSlot::new();
        slot.set_state(SlotState::Player, "Player1".to_string(), 0);
        slot.set_color(5);
        slot.set_start_pos(3);
        info.set_slot(0, slot);

        assert!(info.is_color_taken(5, -1));
        assert!(!info.is_color_taken(6, -1));
        assert!(info.is_start_position_taken(3, -1));
        assert!(!info.is_start_position_taken(4, -1));

        // Same slot should be ignored
        assert!(!info.is_color_taken(5, 0));
        assert!(!info.is_start_position_taken(3, 0));
    }

    #[test]
    fn test_money() {
        let mut money = Money::new(1000);
        assert_eq!(money.count_money(), 1000);

        money.deposit(500);
        assert_eq!(money.count_money(), 1500);

        money.init();
        assert_eq!(money.count_money(), 0);
    }
}
