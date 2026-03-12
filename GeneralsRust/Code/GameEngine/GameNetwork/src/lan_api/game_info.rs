//! LAN game information structures
//!
//! This module defines the structures for managing game information in LAN games,
//! including game state, options, and player slot management.

use crate::lan_api::{DiscoveryMethod, LanPlayer};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::{IpAddr, SocketAddr};
use uuid::Uuid;

/// Maximum number of players supported
pub const MAX_PLAYERS: usize = 8;

/// Game state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameState {
    /// Game is in lobby/setup phase
    Lobby,
    /// Game is starting
    Starting,
    /// Game is in progress
    InProgress,
    /// Game has ended
    Ended,
}

impl Default for GameState {
    fn default() -> Self {
        Self::Lobby
    }
}

/// Game difficulty level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameDifficulty {
    Easy,
    Normal,
    Hard,
    Brutal,
}

impl Default for GameDifficulty {
    fn default() -> Self {
        Self::Normal
    }
}

/// Starting money amount
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum StartingMoney {
    Low,    // 5000
    Normal, // 10000
    High,   // 20000
    Unlimited,
}

impl Default for StartingMoney {
    fn default() -> Self {
        Self::Normal
    }
}

/// Victory condition
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum VictoryCondition {
    /// Destroy all enemy units and structures
    Annihilation,
    /// Hold specific areas for time
    Control,
    /// Capture the flag/artifact
    Capture,
}

impl Default for VictoryCondition {
    fn default() -> Self {
        Self::Annihilation
    }
}

/// Game options that can be configured
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameOptions {
    /// Map name to play on
    pub map_name: String,
    /// Random seed for deterministic gameplay
    pub seed: u32,
    /// Game speed multiplier
    pub speed: f32,
    /// Starting money amount
    pub starting_money: StartingMoney,
    /// Game difficulty
    pub difficulty: GameDifficulty,
    /// Victory condition
    pub victory_condition: VictoryCondition,
    /// Game time limit in minutes (0 = no limit)
    pub time_limit: u32,
    /// Whether superweapons are enabled
    pub superweapons_enabled: bool,
    /// Whether crates are enabled
    pub crates_enabled: bool,
    /// Whether fog of war is enabled
    pub fog_of_war: bool,
    /// Custom game rules/modifiers
    pub custom_rules: HashMap<String, String>,
}

impl Default for GameOptions {
    fn default() -> Self {
        Self {
            map_name: "".to_string(),
            seed: 0,
            speed: 1.0,
            starting_money: StartingMoney::default(),
            difficulty: GameDifficulty::default(),
            victory_condition: VictoryCondition::default(),
            time_limit: 0,
            superweapons_enabled: true,
            crates_enabled: true,
            fog_of_war: true,
            custom_rules: HashMap::new(),
        }
    }
}

impl GameOptions {
    /// Serialize game options to a string (for network transmission)
    pub fn to_string(&self) -> serde_json::Result<String> {
        serde_json::to_string(self)
    }

    /// Deserialize game options from a string
    pub fn from_string(options_str: &str) -> serde_json::Result<Self> {
        serde_json::from_str(options_str)
    }

    /// Create a compact representation for legacy compatibility
    pub fn to_legacy_string(&self) -> String {
        format!(
            "map={};seed={};money={:?};diff={:?};speed={};time={};sw={};crates={};fog={}",
            self.map_name,
            self.seed,
            self.starting_money,
            self.difficulty,
            self.speed,
            self.time_limit,
            self.superweapons_enabled as u8,
            self.crates_enabled as u8,
            self.fog_of_war as u8
        )
    }

    /// Parse from legacy string format
    pub fn from_legacy_string(legacy_str: &str) -> Option<Self> {
        let mut options = Self::default();

        for pair in legacy_str.split(';') {
            if let Some((key, value)) = pair.split_once('=') {
                match key {
                    "map" => options.map_name = value.to_string(),
                    "seed" => {
                        if let Ok(seed) = value.parse() {
                            options.seed = seed;
                        }
                    }
                    "speed" => {
                        if let Ok(speed) = value.parse() {
                            options.speed = speed;
                        }
                    }
                    "time" => {
                        if let Ok(time) = value.parse() {
                            options.time_limit = time;
                        }
                    }
                    "sw" => {
                        options.superweapons_enabled = value == "1";
                    }
                    "crates" => {
                        options.crates_enabled = value == "1";
                    }
                    "fog" => {
                        options.fog_of_war = value == "1";
                    }
                    _ => {
                        // Store unknown options in custom rules
                        options
                            .custom_rules
                            .insert(key.to_string(), value.to_string());
                    }
                }
            }
        }

        Some(options)
    }
}

/// Player slot in a game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameSlot {
    /// Slot number (0-7)
    pub slot_number: u8,
    /// Player in this slot (if any)
    pub player: Option<LanPlayer>,
    /// Whether this slot is open for players
    pub is_open: bool,
    /// Whether this slot is filled by AI
    pub is_ai: bool,
    /// AI difficulty if this is an AI slot
    pub ai_difficulty: Option<GameDifficulty>,
    /// Player's chosen faction/army
    pub faction: Option<String>,
    /// Player's chosen color
    pub color: Option<u32>,
    /// Whether the player has accepted the current game settings
    pub has_accepted: bool,
    /// Whether the player has the required map
    pub has_map: bool,
    /// Player's team number (0-3, same team = allied)
    pub team: u8,
    /// Last time we heard from this player
    pub last_heard: Option<DateTime<Utc>>,
}

impl GameSlot {
    /// Create a new empty slot
    pub fn new(slot_number: u8) -> Self {
        Self {
            slot_number,
            player: None,
            is_open: true,
            is_ai: false,
            ai_difficulty: None,
            faction: None,
            color: None,
            has_accepted: false,
            has_map: false,
            team: 0,
            last_heard: None,
        }
    }

    /// Create a new AI slot
    pub fn new_ai(slot_number: u8, difficulty: GameDifficulty) -> Self {
        Self {
            slot_number,
            player: None,
            is_open: false,
            is_ai: true,
            ai_difficulty: Some(difficulty),
            faction: None,
            color: None,
            has_accepted: true, // AI always accepts
            has_map: true,      // AI always has map
            team: 0,
            last_heard: None,
        }
    }

    /// Check if this slot is empty
    pub fn is_empty(&self) -> bool {
        self.player.is_none() && !self.is_ai
    }

    /// Check if this slot has a human player
    pub fn is_human(&self) -> bool {
        self.player.is_some()
    }

    /// Get the display name for this slot
    pub fn get_display_name(&self) -> String {
        if let Some(ref player) = self.player {
            player.name.clone()
        } else if self.is_ai {
            format!(
                "AI ({})",
                self.ai_difficulty
                    .map_or("Normal".to_string(), |d| format!("{:?}", d))
            )
        } else if self.is_open {
            "Open".to_string()
        } else {
            "Closed".to_string()
        }
    }

    /// Set a player in this slot
    pub fn set_player(&mut self, player: LanPlayer) {
        self.has_accepted = player.has_accepted;
        self.has_map = player.has_map;
        self.player = Some(player);
        self.is_open = false;
        self.is_ai = false;
        self.ai_difficulty = None;
        self.last_heard = Some(Utc::now());
    }

    /// Remove the player from this slot
    pub fn clear_player(&mut self) {
        self.player = None;
        self.is_open = true;
        self.is_ai = false;
        self.ai_difficulty = None;
        self.has_accepted = false;
        self.has_map = false;
        self.faction = None;
        self.color = None;
        self.last_heard = None;
    }

    /// Set this slot as AI
    pub fn set_ai(&mut self, difficulty: GameDifficulty) {
        self.player = None;
        self.is_open = false;
        self.is_ai = true;
        self.ai_difficulty = Some(difficulty);
        self.has_accepted = true;
        self.has_map = true;
    }

    /// Close this slot (not available for players)
    pub fn close(&mut self) {
        self.player = None;
        self.is_open = false;
        self.is_ai = false;
        self.ai_difficulty = None;
        self.has_accepted = false;
        self.has_map = false;
    }
}

/// Information about a LAN game
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanGameInfo {
    /// Game name/title
    pub name: String,
    /// Host player's IP address
    pub host_ip: IpAddr,
    /// Game port
    pub port: u16,
    /// Unique game identifier
    pub game_id: Uuid,
    /// Current game state
    pub state: GameState,
    /// Game configuration options
    pub options: GameOptions,
    /// Player slots (8 slots max)
    pub slots: [GameSlot; MAX_PLAYERS],
    /// Whether this is a direct connect game
    pub is_direct_connect: bool,
    /// Current number of human players
    pub player_count: u8,
    /// Maximum players allowed
    pub max_players: u8,
    /// Whether the game is password protected
    pub has_password: bool,
    /// Game version/compatibility hash
    pub version_hash: u32,
    /// Map CRC for validation
    pub map_crc: Option<u32>,
    /// When this game was created
    pub created_at: DateTime<Utc>,
    /// Last time we heard from this game
    pub last_heard: DateTime<Utc>,
    /// How this game was discovered
    pub discovery_method: DiscoveryMethod,
    /// Whether the game is publicly advertised
    pub is_public: bool,
    /// Publicly reachable host IP if available
    pub public_host: Option<IpAddr>,
    /// Public port advertised for direct connections
    pub public_port: Option<u16>,
}

impl LanGameInfo {
    /// Create a new game info
    pub fn new(name: String, host_ip: IpAddr, port: u16) -> Self {
        let game_id = Uuid::new_v4();
        let now = Utc::now();

        // Initialize all slots as open
        let slots = std::array::from_fn(|i| GameSlot::new(i as u8));

        Self {
            name,
            host_ip,
            port,
            game_id,
            state: GameState::default(),
            options: GameOptions::default(),
            slots,
            is_direct_connect: false,
            player_count: 0,
            max_players: MAX_PLAYERS as u8,
            has_password: false,
            version_hash: 0,
            map_crc: None,
            created_at: now,
            last_heard: now,
            discovery_method: DiscoveryMethod::Broadcast,
            is_public: true,
            public_host: None,
            public_port: None,
        }
    }

    /// Returns the advertised public endpoint if present.
    pub fn public_endpoint(&self) -> Option<SocketAddr> {
        match (self.public_host, self.public_port) {
            (Some(host), Some(port)) => Some(SocketAddr::new(host, port)),
            _ => None,
        }
    }

    /// Check if the game is full
    pub fn is_full(&self) -> bool {
        self.player_count >= self.max_players
    }

    /// Check if the game has started
    pub fn has_started(&self) -> bool {
        matches!(
            self.state,
            GameState::Starting | GameState::InProgress | GameState::Ended
        )
    }

    /// Get the host player (player in slot 0)
    pub fn get_host(&self) -> Option<&LanPlayer> {
        self.slots[0].player.as_ref()
    }

    /// Get all human players
    pub fn get_players(&self) -> Vec<&LanPlayer> {
        self.slots
            .iter()
            .filter_map(|slot| slot.player.as_ref())
            .collect()
    }

    /// Get a player by IP address
    pub fn get_player_by_ip(&self, ip: IpAddr) -> Option<&LanPlayer> {
        self.slots
            .iter()
            .filter_map(|slot| slot.player.as_ref())
            .find(|player| player.ip == ip)
    }

    /// Get a player's slot number by IP address
    pub fn get_slot_by_ip(&self, ip: IpAddr) -> Option<u8> {
        self.slots
            .iter()
            .find(|slot| slot.player.as_ref().map(|p| p.ip) == Some(ip))
            .map(|slot| slot.slot_number)
    }

    /// Find the first available slot
    pub fn find_available_slot(&self) -> Option<u8> {
        self.slots
            .iter()
            .find(|slot| slot.is_open && slot.is_empty())
            .map(|slot| slot.slot_number)
    }

    /// Add a player to the game
    pub fn add_player(&mut self, player: LanPlayer) -> Result<u8, String> {
        // Check if player is already in game
        if self.get_player_by_ip(player.ip).is_some() {
            return Err("Player already in game".to_string());
        }

        // Find available slot
        let slot_num = self.find_available_slot().ok_or("No available slots")?;

        // Add player to slot
        self.slots[slot_num as usize].set_player(player);
        self.player_count += 1;

        Ok(slot_num)
    }

    /// Remove a player from the game
    pub fn remove_player(&mut self, ip: IpAddr) -> bool {
        if let Some(slot_num) = self.get_slot_by_ip(ip) {
            self.slots[slot_num as usize].clear_player();
            if self.player_count > 0 {
                self.player_count -= 1;
            }
            true
        } else {
            false
        }
    }

    /// Set player's accept status
    pub fn set_player_accepted(&mut self, ip: IpAddr, accepted: bool) -> bool {
        if let Some(slot_num) = self.get_slot_by_ip(ip) {
            self.slots[slot_num as usize].has_accepted = accepted;
            true
        } else {
            false
        }
    }

    /// Set player's map status
    pub fn set_player_has_map(&mut self, ip: IpAddr, has_map: bool) -> bool {
        if let Some(slot_num) = self.get_slot_by_ip(ip) {
            self.slots[slot_num as usize].has_map = has_map;
            true
        } else {
            false
        }
    }

    /// Check if all players have accepted
    pub fn all_players_accepted(&self) -> bool {
        self.slots
            .iter()
            .filter(|slot| slot.is_human())
            .all(|slot| slot.has_accepted)
    }

    /// Check if all players have the map
    pub fn all_players_have_map(&self) -> bool {
        self.slots
            .iter()
            .filter(|slot| slot.is_human())
            .all(|slot| slot.has_map)
    }

    /// Get count of players who have accepted
    pub fn accepted_players_count(&self) -> u8 {
        self.slots
            .iter()
            .filter(|slot| slot.is_human() && slot.has_accepted)
            .count() as u8
    }

    /// Get count of players who have the map
    pub fn players_with_map_count(&self) -> u8 {
        self.slots
            .iter()
            .filter(|slot| slot.is_human() && slot.has_map)
            .count() as u8
    }

    /// Update last heard time
    pub fn update_last_heard(&mut self) {
        self.last_heard = Utc::now();
    }

    /// Update player's last heard time
    pub fn update_player_last_heard(&mut self, ip: IpAddr) {
        if let Some(slot_num) = self.get_slot_by_ip(ip) {
            self.slots[slot_num as usize].last_heard = Some(Utc::now());
        }
    }

    /// Check if the game is stale (not heard from recently)
    pub fn is_stale(&self, timeout: std::time::Duration) -> bool {
        Utc::now()
            .signed_duration_since(self.last_heard)
            .to_std()
            .unwrap_or_default()
            > timeout
    }

    /// Get a summary string for display
    pub fn get_summary(&self) -> String {
        format!(
            "{} ({}/{}) - {} - {}",
            self.name,
            self.player_count,
            self.max_players,
            self.options.map_name,
            if self.has_started() {
                "In Progress"
            } else {
                "In Lobby"
            }
        )
    }

    /// Convert to properties map for network transmission
    pub fn to_properties(&self) -> HashMap<String, String> {
        let mut props = HashMap::new();

        props.insert("game_name".to_string(), self.name.clone());
        props.insert("in_progress".to_string(), self.has_started().to_string());
        props.insert("player_count".to_string(), self.player_count.to_string());
        props.insert("max_players".to_string(), self.max_players.to_string());
        props.insert(
            "direct_connect".to_string(),
            self.is_direct_connect.to_string(),
        );
        props.insert("has_password".to_string(), self.has_password.to_string());
        props.insert("map_name".to_string(), self.options.map_name.clone());
        props.insert("version_hash".to_string(), self.version_hash.to_string());

        if let Some(map_crc) = self.map_crc {
            props.insert("map_crc".to_string(), map_crc.to_string());
        }

        props
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lan_api::PlayerState;
    use std::net::Ipv4Addr;

    #[test]
    fn test_game_options_serialization() {
        let options = GameOptions {
            map_name: "Test Map".to_string(),
            seed: 12345,
            starting_money: StartingMoney::High,
            ..Default::default()
        };

        let serialized = options.to_string().unwrap();
        let deserialized = GameOptions::from_string(&serialized).unwrap();

        assert_eq!(deserialized.map_name, "Test Map");
        assert_eq!(deserialized.seed, 12345);
        assert_eq!(deserialized.starting_money, StartingMoney::High);
    }

    #[test]
    fn test_game_options_legacy_format() {
        let options = GameOptions {
            map_name: "Test".to_string(),
            seed: 999,
            speed: 1.5,
            superweapons_enabled: false,
            ..Default::default()
        };

        let legacy_str = options.to_legacy_string();
        assert!(legacy_str.contains("map=Test"));
        assert!(legacy_str.contains("seed=999"));
        assert!(legacy_str.contains("speed=1.5"));
        assert!(legacy_str.contains("sw=0"));

        let parsed = GameOptions::from_legacy_string(&legacy_str).unwrap();
        assert_eq!(parsed.map_name, "Test");
        assert_eq!(parsed.seed, 999);
        assert_eq!(parsed.speed, 1.5);
        assert!(!parsed.superweapons_enabled);
    }

    #[test]
    fn test_game_info_player_management() {
        let host_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let mut game = LanGameInfo::new("Test Game".to_string(), host_ip, 8087);

        let player = LanPlayer {
            id: Uuid::new_v4(),
            name: "TestPlayer".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100)),
            port: 8088,
            state: PlayerState::InLobby,
            last_heard: Utc::now(),
            ..Default::default()
        };

        // Add player
        let slot = game.add_player(player.clone()).unwrap();
        assert_eq!(game.player_count, 1);
        assert_eq!(slot, 0);

        // Check player is in game
        assert!(game.get_player_by_ip(player.ip).is_some());
        assert_eq!(game.get_slot_by_ip(player.ip), Some(0));

        // Remove player
        assert!(game.remove_player(player.ip));
        assert_eq!(game.player_count, 0);
        assert!(game.get_player_by_ip(player.ip).is_none());
    }

    #[test]
    fn test_game_slot_management() {
        let mut slot = GameSlot::new(0);

        assert!(slot.is_empty());
        assert!(!slot.is_human());
        assert_eq!(slot.get_display_name(), "Open");

        // Set AI
        slot.set_ai(GameDifficulty::Hard);
        assert!(!slot.is_empty());
        assert!(!slot.is_human());
        assert!(slot.is_ai);
        assert!(slot.get_display_name().contains("AI"));
        assert!(slot.has_accepted);
        assert!(slot.has_map);

        // Set player
        let player = LanPlayer {
            id: Uuid::new_v4(),
            name: "Test".to_string(),
            ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            ..Default::default()
        };

        slot.set_player(player.clone());
        assert!(!slot.is_empty());
        assert!(slot.is_human());
        assert!(!slot.is_ai);
        assert_eq!(slot.get_display_name(), "Test");

        // Clear slot
        slot.clear_player();
        assert!(slot.is_empty());
        assert!(!slot.is_human());
        assert!(!slot.is_ai);
    }
}
