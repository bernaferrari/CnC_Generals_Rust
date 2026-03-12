//! LAN player information and state management
//!
//! This module defines player structures and state tracking for LAN games.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use uuid::Uuid;

/// Player state in a LAN game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerState {
    /// Player is in the lobby
    InLobby,
    /// Player is in game setup
    InGameSetup,
    /// Player is loading the game
    Loading,
    /// Player is actively playing
    InGame,
    /// Player has disconnected
    Disconnected,
    /// Player is inactive/alt-tabbed
    Inactive,
}

impl Default for PlayerState {
    fn default() -> Self {
        Self::InLobby
    }
}

/// Player role in the game
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlayerRole {
    /// Regular player
    Player,
    /// Game host
    Host,
    /// Observer/spectator
    Observer,
}

impl Default for PlayerRole {
    fn default() -> Self {
        Self::Player
    }
}

/// Connection quality assessment
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionQuality {
    /// Excellent connection (low latency, no packet loss)
    Excellent,
    /// Good connection (acceptable latency)
    Good,
    /// Fair connection (higher latency, occasional drops)
    Fair,
    /// Poor connection (high latency, frequent issues)
    Poor,
    /// Unknown or not yet measured
    Unknown,
}

impl Default for ConnectionQuality {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Player information for LAN games
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanPlayer {
    /// Unique player identifier
    pub id: Uuid,
    /// Player display name (max 12 characters as per C++ implementation)
    pub name: String,
    /// Player's IP address
    pub ip: IpAddr,
    /// Player's port for direct communication
    pub port: u16,
    /// Current player state
    pub state: PlayerState,
    /// Player's role in the game
    pub role: PlayerRole,
    /// Login name (legacy compatibility)
    pub login_name: String,
    /// Host/machine name (legacy compatibility)
    pub host_name: String,
    /// Player rank/rating
    pub rank: u32,
    /// Player's chosen color (RGB value)
    pub color: Option<u32>,
    /// Player's chosen faction
    pub faction: Option<String>,
    /// Player's team number
    pub team: u8,
    /// Whether player has accepted current game settings
    pub has_accepted: bool,
    /// Whether player has the required map
    pub has_map: bool,
    /// Player's ready state for game start
    pub is_ready: bool,
    /// Connection quality assessment
    pub connection_quality: ConnectionQuality,
    /// Round-trip time in milliseconds
    pub ping: Option<u32>,
    /// Packet loss percentage (0.0 - 1.0)
    pub packet_loss: Option<f32>,
    /// Game executable CRC for version checking
    pub exe_crc: Option<u32>,
    /// Game INI/config CRC for mod compatibility
    pub ini_crc: Option<u32>,
    /// Serial number hash for duplicate checking
    pub serial_hash: Option<String>,
    /// When this player joined
    pub joined_at: DateTime<Utc>,
    /// Last time we heard from this player
    pub last_heard: DateTime<Utc>,
    /// Whether the player is currently active (not alt-tabbed)
    pub is_active: bool,
    /// Player statistics
    pub stats: PlayerStats,
}

impl LanPlayer {
    /// Create a new LAN player
    pub fn new(name: String, ip: IpAddr, port: u16) -> Self {
        let now = Utc::now();

        Self {
            id: Uuid::new_v4(),
            name,
            ip,
            port,
            state: PlayerState::default(),
            role: PlayerRole::default(),
            login_name: String::new(),
            host_name: String::new(),
            rank: 0,
            color: None,
            faction: None,
            team: 0,
            has_accepted: false,
            has_map: false,
            is_ready: false,
            connection_quality: ConnectionQuality::default(),
            ping: None,
            packet_loss: None,
            exe_crc: None,
            ini_crc: None,
            serial_hash: None,
            joined_at: now,
            last_heard: now,
            is_active: true,
            stats: PlayerStats::default(),
        }
    }

    /// Create a host player
    pub fn new_host(name: String, ip: IpAddr, port: u16) -> Self {
        let mut player = Self::new(name, ip, port);
        player.role = PlayerRole::Host;
        player.has_accepted = true; // Host always accepts initially
        player.has_map = true;
        player.is_ready = true;
        player
    }

    /// Check if this player is the host
    pub fn is_host(&self) -> bool {
        self.role == PlayerRole::Host
    }

    /// Check if this player is an observer
    pub fn is_observer(&self) -> bool {
        self.role == PlayerRole::Observer
    }

    /// Update the player's last heard time
    pub fn update_last_heard(&mut self) {
        self.last_heard = Utc::now();
    }

    /// Set player state
    pub fn set_state(&mut self, state: PlayerState) {
        self.state = state;
        self.update_last_heard();
    }

    /// Set player as accepted
    pub fn set_accepted(&mut self, accepted: bool) {
        self.has_accepted = accepted;
        self.update_last_heard();
    }

    /// Set player map status
    pub fn set_has_map(&mut self, has_map: bool) {
        self.has_map = has_map;
        self.update_last_heard();
    }

    /// Set player ready status
    pub fn set_ready(&mut self, ready: bool) {
        self.is_ready = ready;
        self.update_last_heard();
    }

    /// Set player active status
    pub fn set_active(&mut self, active: bool) {
        self.is_active = active;
        self.state = if active {
            PlayerState::InLobby // Or previous state
        } else {
            PlayerState::Inactive
        };
        self.update_last_heard();
    }

    /// Update connection metrics
    pub fn update_connection(&mut self, ping: u32, packet_loss: f32) {
        self.ping = Some(ping);
        self.packet_loss = Some(packet_loss);

        // Assess connection quality
        self.connection_quality = match (ping, packet_loss) {
            (p, l) if p <= 50 && l < 0.01 => ConnectionQuality::Excellent,
            (p, l) if p <= 100 && l < 0.05 => ConnectionQuality::Good,
            (p, l) if p <= 200 && l < 0.10 => ConnectionQuality::Fair,
            _ => ConnectionQuality::Poor,
        };

        self.update_last_heard();
    }

    /// Check if player has timed out
    pub fn is_timed_out(&self, timeout: std::time::Duration) -> bool {
        Utc::now()
            .signed_duration_since(self.last_heard)
            .to_std()
            .unwrap_or_default()
            > timeout
    }

    /// Get display name with status indicators
    pub fn get_display_name(&self) -> String {
        let mut name = self.name.clone();

        if !self.is_active {
            name.push_str(" (Away)");
        }

        match self.connection_quality {
            ConnectionQuality::Poor => name.push_str(" (*)"),
            ConnectionQuality::Fair => name.push_str(" (!)"),
            _ => {}
        }

        name
    }

    /// Get status string for UI display
    pub fn get_status_string(&self) -> String {
        match self.state {
            PlayerState::InLobby => {
                if self.has_accepted {
                    "Ready".to_string()
                } else {
                    "Not Ready".to_string()
                }
            }
            PlayerState::InGameSetup => "Setting up".to_string(),
            PlayerState::Loading => "Loading".to_string(),
            PlayerState::InGame => "Playing".to_string(),
            PlayerState::Disconnected => "Disconnected".to_string(),
            PlayerState::Inactive => "Inactive".to_string(),
        }
    }

    /// Get ping string for display
    pub fn get_ping_string(&self) -> String {
        match self.ping {
            Some(ping) => format!("{}ms", ping),
            None => "?".to_string(),
        }
    }

    /// Check if player is compatible (same version, etc.)
    pub fn is_compatible(&self, our_exe_crc: Option<u32>, our_ini_crc: Option<u32>) -> bool {
        // Check executable compatibility
        if let (Some(their_crc), Some(our_crc)) = (self.exe_crc, our_exe_crc) {
            if their_crc != our_crc {
                return false;
            }
        }

        // Check mod/INI compatibility
        if let (Some(their_crc), Some(our_crc)) = (self.ini_crc, our_ini_crc) {
            if their_crc != our_crc {
                return false;
            }
        }

        true
    }

    /// Clone player for network transmission (excluding large data)
    pub fn to_network_player(&self) -> NetworkPlayer {
        NetworkPlayer {
            id: self.id,
            name: self.name.clone(),
            ip: self.ip,
            port: self.port,
            state: self.state,
            role: self.role,
            team: self.team,
            color: self.color,
            faction: self.faction.clone(),
            has_accepted: self.has_accepted,
            has_map: self.has_map,
            is_ready: self.is_ready,
            is_active: self.is_active,
            ping: self.ping,
            connection_quality: self.connection_quality,
        }
    }
}

impl Default for LanPlayer {
    fn default() -> Self {
        Self::new("Unknown".to_string(), "127.0.0.1".parse().unwrap(), 0)
    }
}

/// Lightweight player structure for network transmission
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPlayer {
    pub id: Uuid,
    pub name: String,
    pub ip: IpAddr,
    pub port: u16,
    pub state: PlayerState,
    pub role: PlayerRole,
    pub team: u8,
    pub color: Option<u32>,
    pub faction: Option<String>,
    pub has_accepted: bool,
    pub has_map: bool,
    pub is_ready: bool,
    pub is_active: bool,
    pub ping: Option<u32>,
    pub connection_quality: ConnectionQuality,
}

impl NetworkPlayer {
    /// Convert back to full LanPlayer (with default values for missing fields)
    pub fn to_lan_player(&self) -> LanPlayer {
        let now = Utc::now();
        LanPlayer {
            id: self.id,
            name: self.name.clone(),
            ip: self.ip,
            port: self.port,
            state: self.state,
            role: self.role,
            login_name: String::new(),
            host_name: String::new(),
            rank: 0,
            color: self.color,
            faction: self.faction.clone(),
            team: self.team,
            has_accepted: self.has_accepted,
            has_map: self.has_map,
            is_ready: self.is_ready,
            connection_quality: self.connection_quality,
            ping: self.ping,
            packet_loss: None,
            exe_crc: None,
            ini_crc: None,
            serial_hash: None,
            joined_at: now,
            last_heard: now,
            is_active: self.is_active,
            stats: PlayerStats::default(),
        }
    }
}

/// Player statistics for tracking performance and behavior
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PlayerStats {
    /// Total games played
    pub games_played: u32,
    /// Games won
    pub games_won: u32,
    /// Games lost
    pub games_lost: u32,
    /// Average game duration in seconds
    pub avg_game_duration: u32,
    /// Total disconnections
    pub total_disconnects: u32,
    /// Commands sent per minute average
    pub avg_cpm: f32,
    /// Preferred faction
    pub preferred_faction: Option<String>,
    /// Last game timestamp
    pub last_game: Option<DateTime<Utc>>,
}

impl PlayerStats {
    /// Calculate win rate
    pub fn win_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.games_won as f32 / self.games_played as f32
        }
    }

    /// Calculate disconnect rate
    pub fn disconnect_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.total_disconnects as f32 / self.games_played as f32
        }
    }

    /// Update stats after a game
    pub fn update_after_game(&mut self, won: bool, duration: u32, disconnected: bool) {
        self.games_played += 1;

        if won && !disconnected {
            self.games_won += 1;
        } else {
            self.games_lost += 1;
        }

        if disconnected {
            self.total_disconnects += 1;
        }

        // Update average duration
        let total_duration = self.avg_game_duration * (self.games_played - 1) + duration;
        self.avg_game_duration = total_duration / self.games_played;

        self.last_game = Some(Utc::now());
    }
}

/// Player lookup and management utilities
pub struct PlayerManager {
    players: std::collections::HashMap<Uuid, LanPlayer>,
    ip_to_id: std::collections::HashMap<IpAddr, Uuid>,
}

impl PlayerManager {
    /// Create a new player manager
    pub fn new() -> Self {
        Self {
            players: std::collections::HashMap::new(),
            ip_to_id: std::collections::HashMap::new(),
        }
    }

    /// Add a player
    pub fn add_player(&mut self, player: LanPlayer) {
        self.ip_to_id.insert(player.ip, player.id);
        self.players.insert(player.id, player);
    }

    /// Remove a player by ID
    pub fn remove_player(&mut self, id: Uuid) -> Option<LanPlayer> {
        if let Some(player) = self.players.remove(&id) {
            self.ip_to_id.remove(&player.ip);
            Some(player)
        } else {
            None
        }
    }

    /// Remove a player by IP
    pub fn remove_player_by_ip(&mut self, ip: IpAddr) -> Option<LanPlayer> {
        if let Some(&id) = self.ip_to_id.get(&ip) {
            self.remove_player(id)
        } else {
            None
        }
    }

    /// Get a player by ID
    pub fn get_player(&self, id: Uuid) -> Option<&LanPlayer> {
        self.players.get(&id)
    }

    /// Get a mutable reference to a player by ID
    pub fn get_player_mut(&mut self, id: Uuid) -> Option<&mut LanPlayer> {
        self.players.get_mut(&id)
    }

    /// Get a player by IP
    pub fn get_player_by_ip(&self, ip: IpAddr) -> Option<&LanPlayer> {
        self.ip_to_id.get(&ip).and_then(|&id| self.players.get(&id))
    }

    /// Get a mutable reference to a player by IP
    pub fn get_player_by_ip_mut(&mut self, ip: IpAddr) -> Option<&mut LanPlayer> {
        if let Some(&id) = self.ip_to_id.get(&ip) {
            self.players.get_mut(&id)
        } else {
            None
        }
    }

    /// Get all players
    pub fn get_all_players(&self) -> Vec<&LanPlayer> {
        self.players.values().collect()
    }

    /// Get all active players
    pub fn get_active_players(&self) -> Vec<&LanPlayer> {
        self.players
            .values()
            .filter(|player| player.is_active && player.state != PlayerState::Disconnected)
            .collect()
    }

    /// Update player last heard time
    pub fn update_player_heartbeat(&mut self, ip: IpAddr) -> bool {
        if let Some(player) = self.get_player_by_ip_mut(ip) {
            player.update_last_heard();
            true
        } else {
            false
        }
    }

    /// Remove timed out players
    pub fn remove_timed_out_players(&mut self, timeout: std::time::Duration) -> Vec<LanPlayer> {
        let mut removed = Vec::new();
        let now = Utc::now();

        self.players.retain(|&_id, player| {
            if now
                .signed_duration_since(player.last_heard)
                .to_std()
                .unwrap_or_default()
                > timeout
            {
                self.ip_to_id.remove(&player.ip);
                removed.push(player.clone());
                false
            } else {
                true
            }
        });

        removed
    }

    /// Get player count
    pub fn player_count(&self) -> usize {
        self.players.len()
    }

    /// Clear all players
    pub fn clear(&mut self) {
        self.players.clear();
        self.ip_to_id.clear();
    }
}

impl Default for PlayerManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_lan_player_creation() {
        let ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let player = LanPlayer::new("TestPlayer".to_string(), ip, 8088);

        assert_eq!(player.name, "TestPlayer");
        assert_eq!(player.ip, ip);
        assert_eq!(player.port, 8088);
        assert_eq!(player.state, PlayerState::InLobby);
        assert_eq!(player.role, PlayerRole::Player);
        assert!(!player.has_accepted);
        assert!(!player.has_map);
        assert!(player.is_active);
    }

    #[test]
    fn test_host_player_creation() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let host = LanPlayer::new_host("Host".to_string(), ip, 8086);

        assert!(host.is_host());
        assert!(!host.is_observer());
        assert!(host.has_accepted); // Host should accept by default
        assert_eq!(host.role, PlayerRole::Host);
    }

    #[test]
    fn test_connection_quality_assessment() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let mut player = LanPlayer::new("Test".to_string(), ip, 8088);

        // Test excellent connection
        player.update_connection(30, 0.001);
        assert_eq!(player.connection_quality, ConnectionQuality::Excellent);

        // Test good connection
        player.update_connection(80, 0.03);
        assert_eq!(player.connection_quality, ConnectionQuality::Good);

        // Test fair connection
        player.update_connection(150, 0.08);
        assert_eq!(player.connection_quality, ConnectionQuality::Fair);

        // Test poor connection
        player.update_connection(300, 0.15);
        assert_eq!(player.connection_quality, ConnectionQuality::Poor);
    }

    #[test]
    fn test_player_compatibility() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let mut player = LanPlayer::new("Test".to_string(), ip, 8088);

        player.exe_crc = Some(0x12345678);
        player.ini_crc = Some(0x87654321);

        // Test compatible
        assert!(player.is_compatible(Some(0x12345678), Some(0x87654321)));

        // Test incompatible executable
        assert!(!player.is_compatible(Some(0x11111111), Some(0x87654321)));

        // Test incompatible INI
        assert!(!player.is_compatible(Some(0x12345678), Some(0x11111111)));

        // Test unknown CRCs (should be compatible)
        assert!(player.is_compatible(None, None));
    }

    #[test]
    fn test_player_manager() {
        let mut manager = PlayerManager::new();

        let ip1 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1));
        let ip2 = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2));

        let player1 = LanPlayer::new("Player1".to_string(), ip1, 8088);
        let player2 = LanPlayer::new("Player2".to_string(), ip2, 8088);

        let id1 = player1.id;
        let id2 = player2.id;

        // Add players
        manager.add_player(player1);
        manager.add_player(player2);

        assert_eq!(manager.player_count(), 2);

        // Test lookup by ID
        assert!(manager.get_player(id1).is_some());
        assert_eq!(manager.get_player(id1).unwrap().name, "Player1");

        // Test lookup by IP
        assert!(manager.get_player_by_ip(ip2).is_some());
        assert_eq!(manager.get_player_by_ip(ip2).unwrap().name, "Player2");

        // Test removal by IP
        let removed = manager.remove_player_by_ip(ip1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().name, "Player1");
        assert_eq!(manager.player_count(), 1);

        // Test removal by ID
        let removed = manager.remove_player(id2);
        assert!(removed.is_some());
        assert_eq!(manager.player_count(), 0);
    }

    #[test]
    fn test_network_player_conversion() {
        let ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let original = LanPlayer::new("Test".to_string(), ip, 8088);

        let network_player = original.to_network_player();
        let converted_back = network_player.to_lan_player();

        assert_eq!(converted_back.id, original.id);
        assert_eq!(converted_back.name, original.name);
        assert_eq!(converted_back.ip, original.ip);
        assert_eq!(converted_back.port, original.port);
        assert_eq!(converted_back.state, original.state);
    }

    #[test]
    fn test_player_stats() {
        let mut stats = PlayerStats::default();

        // Test initial state
        assert_eq!(stats.win_rate(), 0.0);
        assert_eq!(stats.disconnect_rate(), 0.0);

        // Play some games
        stats.update_after_game(true, 1200, false); // Win, 20 minutes
        stats.update_after_game(false, 800, false); // Loss, 13 minutes
        stats.update_after_game(true, 1000, true); // Win but disconnected

        assert_eq!(stats.games_played, 3);
        assert_eq!(stats.games_won, 1); // Only first game counts as win
        assert_eq!(stats.games_lost, 2);
        assert_eq!(stats.total_disconnects, 1);
        assert_eq!(stats.avg_game_duration, 1000); // (1200 + 800 + 1000) / 3

        assert!((stats.win_rate() - 1.0 / 3.0).abs() < 0.001);
        assert!((stats.disconnect_rate() - 1.0 / 3.0).abs() < 0.001);
    }
}
