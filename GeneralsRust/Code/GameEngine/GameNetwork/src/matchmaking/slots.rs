//! Player slot management for multiplayer lobbies
//!
//! This module provides comprehensive slot management including:
//! - Team assignment and balancing
//! - Faction/army selection
//! - Player color assignment
//! - AI slot configuration
//! - Starting position management

use crate::error::{NetworkError, NetworkResult};
use crate::lan_api::game_info::{GameDifficulty, GameSlot};
use crate::lan_api::LanPlayer;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use uuid::Uuid;

/// Maximum number of player slots
pub const MAX_SLOTS: usize = 8;

/// Maximum number of teams
pub const MAX_TEAMS: u8 = 4;

/// Player faction/army choices for C&C Generals Zero Hour
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Faction {
    // USA Generals
    UsaAirForce,
    UsaSuperweapon,
    UsaLaser,

    // China Generals
    ChinaNuke,
    ChinaTank,
    ChinaInfantry,

    // GLA Generals
    GlaDemolition,
    GlaSteath,
    GlaToxin,

    // Random (auto-select)
    Random,

    // Observer (spectator)
    Observer,
}

impl Faction {
    /// Get all playable factions
    pub fn all_playable() -> Vec<Self> {
        vec![
            Self::UsaAirForce,
            Self::UsaSuperweapon,
            Self::UsaLaser,
            Self::ChinaNuke,
            Self::ChinaTank,
            Self::ChinaInfantry,
            Self::GlaDemolition,
            Self::GlaSteath,
            Self::GlaToxin,
        ]
    }

    /// Get faction name for display
    pub fn display_name(&self) -> &'static str {
        match self {
            Self::UsaAirForce => "USA Air Force",
            Self::UsaSuperweapon => "USA Superweapon",
            Self::UsaLaser => "USA Laser",
            Self::ChinaNuke => "China Nuke",
            Self::ChinaTank => "China Tank",
            Self::ChinaInfantry => "China Infantry",
            Self::GlaDemolition => "GLA Demolition",
            Self::GlaSteath => "GLA Stealth",
            Self::GlaToxin => "GLA Toxin",
            Self::Random => "Random",
            Self::Observer => "Observer",
        }
    }

    /// Get short faction code
    pub fn code(&self) -> &'static str {
        match self {
            Self::UsaAirForce => "USA_AF",
            Self::UsaSuperweapon => "USA_SW",
            Self::UsaLaser => "USA_LA",
            Self::ChinaNuke => "CHI_NU",
            Self::ChinaTank => "CHI_TA",
            Self::ChinaInfantry => "CHI_IN",
            Self::GlaDemolition => "GLA_DE",
            Self::GlaSteath => "GLA_ST",
            Self::GlaToxin => "GLA_TO",
            Self::Random => "RANDOM",
            Self::Observer => "OBS",
        }
    }

    /// Check if faction is observer
    pub fn is_observer(&self) -> bool {
        matches!(self, Self::Observer)
    }
}

impl Default for Faction {
    fn default() -> Self {
        Self::Random
    }
}

impl std::fmt::Display for Faction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

/// Player colors (using standard RTS color palette)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[repr(u8)]
pub enum PlayerColor {
    Red = 0,
    Blue = 1,
    Green = 2,
    Yellow = 3,
    Orange = 4,
    Purple = 5,
    Cyan = 6,
    Pink = 7,
}

impl PlayerColor {
    /// Get all available colors
    pub fn all() -> &'static [Self] {
        const COLORS: [PlayerColor; 8] = [
            PlayerColor::Red,
            PlayerColor::Blue,
            PlayerColor::Green,
            PlayerColor::Yellow,
            PlayerColor::Orange,
            PlayerColor::Purple,
            PlayerColor::Cyan,
            PlayerColor::Pink,
        ];
        &COLORS
    }

    /// Get color name
    pub fn name(&self) -> &'static str {
        match self {
            Self::Red => "Red",
            Self::Blue => "Blue",
            Self::Green => "Green",
            Self::Yellow => "Yellow",
            Self::Orange => "Orange",
            Self::Purple => "Purple",
            Self::Cyan => "Cyan",
            Self::Pink => "Pink",
        }
    }

    /// Get RGB value (for UI display)
    pub fn rgb(&self) -> (u8, u8, u8) {
        match self {
            Self::Red => (255, 0, 0),
            Self::Blue => (0, 0, 255),
            Self::Green => (0, 255, 0),
            Self::Yellow => (255, 255, 0),
            Self::Orange => (255, 165, 0),
            Self::Purple => (128, 0, 128),
            Self::Cyan => (0, 255, 255),
            Self::Pink => (255, 192, 203),
        }
    }

    /// From slot number (default assignment)
    pub fn from_slot(slot: u8) -> Self {
        match slot % 8 {
            0 => Self::Red,
            1 => Self::Blue,
            2 => Self::Green,
            3 => Self::Yellow,
            4 => Self::Orange,
            5 => Self::Purple,
            6 => Self::Cyan,
            7 => Self::Pink,
            _ => Self::Red,
        }
    }
}

impl std::fmt::Display for PlayerColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// Extended slot information with team, faction, and color
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlotInfo {
    /// Base slot information
    pub slot: GameSlot,

    /// Selected faction
    pub faction: Option<Faction>,

    /// Assigned color
    pub color: PlayerColor,

    /// Starting position on map (0-7)
    pub starting_position: u8,

    /// Handicap multiplier (0.5 = 50%, 1.0 = normal, 2.0 = 200%)
    pub handicap: f32,

    /// Whether this slot is locked (can't be changed)
    pub is_locked: bool,
}

impl SlotInfo {
    /// Create new slot info from game slot
    pub fn new(slot: GameSlot) -> Self {
        let color = PlayerColor::from_slot(slot.slot_number);

        Self {
            starting_position: slot.slot_number,
            faction: None,
            color,
            handicap: 1.0,
            is_locked: false,
            slot,
        }
    }

    /// Check if slot has a player
    pub fn has_player(&self) -> bool {
        self.slot.player.is_some()
    }

    /// Get player name (if any)
    pub fn get_player_name(&self) -> Option<String> {
        self.slot.player.as_ref().map(|p| p.name.clone())
    }

    /// Set faction
    pub fn set_faction(&mut self, faction: Faction) -> NetworkResult<()> {
        if self.is_locked {
            return Err(NetworkError::invalid_command("Slot is locked".to_string()));
        }

        self.faction = Some(faction);
        Ok(())
    }

    /// Set color (if not taken)
    pub fn set_color(&mut self, color: PlayerColor) -> NetworkResult<()> {
        if self.is_locked {
            return Err(NetworkError::invalid_command("Slot is locked".to_string()));
        }

        self.color = color;
        Ok(())
    }

    /// Set team
    pub fn set_team(&mut self, team: u8) -> NetworkResult<()> {
        if self.is_locked {
            return Err(NetworkError::invalid_command("Slot is locked".to_string()));
        }

        if team >= MAX_TEAMS {
            return Err(NetworkError::invalid_command(format!(
                "Team must be 0-{}",
                MAX_TEAMS - 1
            )));
        }

        self.slot.team = team;
        Ok(())
    }

    /// Lock slot configuration
    pub fn lock(&mut self) {
        self.is_locked = true;
    }

    /// Unlock slot configuration
    pub fn unlock(&mut self) {
        self.is_locked = false;
    }
}

/// Slot manager for organizing players in a game
pub struct SlotManager {
    /// All player slots
    slots: Vec<SlotInfo>,

    /// Maximum number of human players
    max_players: usize,

    /// Slot assignments by player ID
    player_slots: HashMap<Uuid, u8>,

    /// Slot assignments by IP (for LAN games)
    ip_slots: HashMap<IpAddr, u8>,

    /// Whether slots are balanced by team
    auto_balance_teams: bool,
}

impl SlotManager {
    /// Create new slot manager
    pub fn new(max_players: usize) -> Self {
        let slots = (0..MAX_SLOTS)
            .map(|i| SlotInfo::new(GameSlot::new(i as u8)))
            .collect();

        Self {
            slots,
            max_players: max_players.min(MAX_SLOTS),
            player_slots: HashMap::new(),
            ip_slots: HashMap::new(),
            auto_balance_teams: true,
        }
    }

    /// Get slot by number
    pub fn get_slot(&self, slot_num: u8) -> NetworkResult<&SlotInfo> {
        self.slots
            .get(slot_num as usize)
            .ok_or_else(|| NetworkError::invalid_command("Invalid slot number".to_string()))
    }

    /// Get mutable slot by number
    pub fn get_slot_mut(&mut self, slot_num: u8) -> NetworkResult<&mut SlotInfo> {
        self.slots
            .get_mut(slot_num as usize)
            .ok_or_else(|| NetworkError::invalid_command("Invalid slot number".to_string()))
    }

    /// Find available slot
    pub fn find_available_slot(&self) -> Option<u8> {
        self.slots
            .iter()
            .find(|slot| slot.slot.is_open && slot.slot.is_empty())
            .map(|slot| slot.slot.slot_number)
    }

    /// Add player to slot
    pub fn add_player(
        &mut self,
        player_id: Uuid,
        player_ip: IpAddr,
        player: LanPlayer,
    ) -> NetworkResult<u8> {
        // Check if player already has a slot
        if self.player_slots.contains_key(&player_id) {
            return Err(NetworkError::invalid_command(
                "Player already in a slot".to_string(),
            ));
        }

        // Find available slot
        let slot_num = self
            .find_available_slot()
            .ok_or_else(|| NetworkError::invalid_command("No available slots".to_string()))?;

        // Add player to slot
        let slot = self.get_slot_mut(slot_num)?;
        slot.slot.set_player(player);

        // Track assignments
        self.player_slots.insert(player_id, slot_num);
        self.ip_slots.insert(player_ip, slot_num);

        // Auto-assign team if enabled
        if self.auto_balance_teams {
            self.balance_teams()?;
        }

        Ok(slot_num)
    }

    /// Remove player from slot
    pub fn remove_player(&mut self, player_id: Uuid) -> NetworkResult<()> {
        let slot_num = self
            .player_slots
            .remove(&player_id)
            .ok_or_else(|| NetworkError::invalid_command("Player not in any slot".to_string()))?;

        let player_ip = {
            let slot = self.get_slot_mut(slot_num)?;
            let player_ip = slot.slot.player.as_ref().map(|player| player.ip);
            slot.slot.clear_player();
            slot.faction = None;
            player_ip
        };

        if let Some(player_ip) = player_ip {
            self.ip_slots.remove(&player_ip);
        }

        Ok(())
    }

    /// Move player to different slot
    pub fn move_player(&mut self, player_id: Uuid, new_slot: u8) -> NetworkResult<()> {
        // Check new slot is available
        let target_slot = self.get_slot(new_slot)?;
        if !target_slot.slot.is_empty() {
            return Err(NetworkError::invalid_command(
                "Target slot is occupied".to_string(),
            ));
        }

        // Get current slot
        let current_slot_num =
            self.player_slots.get(&player_id).copied().ok_or_else(|| {
                NetworkError::invalid_command("Player not in any slot".to_string())
            })?;

        // Move player
        let player = {
            let current_slot = self.get_slot_mut(current_slot_num)?;
            let player = current_slot
                .slot
                .player
                .clone()
                .ok_or_else(|| NetworkError::generic("Slot has no player".to_string()))?;

            current_slot.slot.clear_player();
            player
        };

        let target_slot = self.get_slot_mut(new_slot)?;
        target_slot.slot.set_player(player.clone());

        // Update tracking
        self.player_slots.insert(player_id, new_slot);
        self.ip_slots.insert(player.ip, new_slot);

        Ok(())
    }

    /// Set player faction
    pub fn set_player_faction(&mut self, player_id: Uuid, faction: Faction) -> NetworkResult<()> {
        let slot_num =
            self.player_slots.get(&player_id).copied().ok_or_else(|| {
                NetworkError::invalid_command("Player not in any slot".to_string())
            })?;

        let slot = self.get_slot_mut(slot_num)?;
        slot.set_faction(faction)?;

        Ok(())
    }

    /// Set player color
    pub fn set_player_color(&mut self, player_id: Uuid, color: PlayerColor) -> NetworkResult<()> {
        let slot_num =
            self.player_slots.get(&player_id).copied().ok_or_else(|| {
                NetworkError::invalid_command("Player not in any slot".to_string())
            })?;

        // Check if color is already taken
        if self.is_color_taken(color, Some(slot_num)) {
            return Err(NetworkError::invalid_command(
                "Color already taken".to_string(),
            ));
        }

        let slot = self.get_slot_mut(slot_num)?;
        slot.set_color(color)?;

        Ok(())
    }

    /// Set player team
    pub fn set_player_team(&mut self, player_id: Uuid, team: u8) -> NetworkResult<()> {
        let slot_num =
            self.player_slots.get(&player_id).copied().ok_or_else(|| {
                NetworkError::invalid_command("Player not in any slot".to_string())
            })?;

        let slot = self.get_slot_mut(slot_num)?;
        slot.set_team(team)?;

        Ok(())
    }

    /// Check if color is taken by another slot
    fn is_color_taken(&self, color: PlayerColor, exclude_slot: Option<u8>) -> bool {
        self.slots.iter().any(|slot| {
            if let Some(exclude) = exclude_slot {
                if slot.slot.slot_number == exclude {
                    return false;
                }
            }

            slot.has_player() && slot.color == color
        })
    }

    /// Get available colors
    pub fn get_available_colors(&self, current_slot: Option<u8>) -> Vec<PlayerColor> {
        PlayerColor::all()
            .iter()
            .copied()
            .filter(|&color| !self.is_color_taken(color, current_slot))
            .collect()
    }

    /// Balance teams automatically
    pub fn balance_teams(&mut self) -> NetworkResult<()> {
        let player_slots: Vec<_> = self.slots.iter_mut().filter(|s| s.has_player()).collect();

        let player_count = player_slots.len();
        if player_count < 2 {
            return Ok(()); // Nothing to balance
        }

        // Distribute players evenly across 2 teams
        for (i, slot) in player_slots.into_iter().enumerate() {
            slot.slot.team = (i % 2) as u8;
        }

        Ok(())
    }

    /// Get team composition
    pub fn get_team_composition(&self) -> HashMap<u8, Vec<String>> {
        let mut teams: HashMap<u8, Vec<String>> = HashMap::new();

        for slot in &self.slots {
            if let Some(player_name) = slot.get_player_name() {
                teams
                    .entry(slot.slot.team)
                    .or_insert_with(Vec::new)
                    .push(player_name);
            }
        }

        teams
    }

    /// Check if teams are balanced
    pub fn are_teams_balanced(&self) -> bool {
        let teams = self.get_team_composition();
        if teams.len() < 2 {
            return true;
        }

        let mut team_sizes: Vec<_> = teams.values().map(|v| v.len()).collect();
        team_sizes.sort();

        // Teams are balanced if size difference is at most 1
        team_sizes.last().unwrap() - team_sizes.first().unwrap() <= 1
    }

    /// Add AI to slot
    pub fn add_ai(&mut self, slot_num: u8, difficulty: GameDifficulty) -> NetworkResult<()> {
        let slot = self.get_slot_mut(slot_num)?;

        if !slot.slot.is_empty() {
            return Err(NetworkError::invalid_command(
                "Slot is not empty".to_string(),
            ));
        }

        slot.slot.set_ai(difficulty);
        Ok(())
    }

    /// Get all slots
    pub fn get_all_slots(&self) -> &[SlotInfo] {
        &self.slots
    }

    /// Get occupied slot count
    pub fn get_occupied_count(&self) -> usize {
        self.slots.iter().filter(|s| !s.slot.is_empty()).count()
    }

    /// Get human player count
    pub fn get_human_player_count(&self) -> usize {
        self.slots.iter().filter(|s| s.slot.is_human()).count()
    }

    /// Get AI count
    pub fn get_ai_count(&self) -> usize {
        self.slots.iter().filter(|s| s.slot.is_ai).count()
    }

    /// Lock all slots
    pub fn lock_all_slots(&mut self) {
        for slot in &mut self.slots {
            slot.lock();
        }
    }

    /// Unlock all slots
    pub fn unlock_all_slots(&mut self) {
        for slot in &mut self.slots {
            slot.unlock();
        }
    }

    /// Validate slot configuration
    pub fn validate_configuration(&self) -> NetworkResult<()> {
        // Check for duplicate colors among human players
        let mut colors_used = std::collections::HashSet::new();
        for slot in &self.slots {
            if slot.has_player() {
                if !colors_used.insert(slot.color) {
                    return Err(NetworkError::invalid_command(
                        "Duplicate player colors".to_string(),
                    ));
                }
            }
        }

        // Check minimum players
        if self.get_occupied_count() < 2 {
            return Err(NetworkError::invalid_command(
                "Need at least 2 players to start".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::net::Ipv4Addr;

    #[test]
    fn test_faction_display() {
        assert_eq!(Faction::UsaAirForce.display_name(), "USA Air Force");
        assert_eq!(Faction::ChinaTank.code(), "CHI_TA");
        assert!(Faction::Observer.is_observer());
        assert!(!Faction::UsaLaser.is_observer());
    }

    #[test]
    fn test_player_color() {
        let red = PlayerColor::Red;
        assert_eq!(red.name(), "Red");
        assert_eq!(red.rgb(), (255, 0, 0));

        let color = PlayerColor::from_slot(1);
        assert_eq!(color, PlayerColor::Blue);
    }

    #[test]
    fn test_slot_manager_creation() {
        let manager = SlotManager::new(8);
        assert_eq!(manager.get_human_player_count(), 0);
        assert_eq!(manager.get_ai_count(), 0);
    }

    #[test]
    fn test_add_remove_player() {
        let mut manager = SlotManager::new(8);

        let player_id = Uuid::new_v4();
        let player_ip = IpAddr::V4(Ipv4Addr::new(192, 168, 1, 100));
        let player = LanPlayer {
            id: player_id,
            name: "TestPlayer".to_string(),
            ip: player_ip,
            ..Default::default()
        };

        let slot_num = manager.add_player(player_id, player_ip, player).unwrap();
        assert_eq!(manager.get_human_player_count(), 1);

        manager.remove_player(player_id).unwrap();
        assert_eq!(manager.get_human_player_count(), 0);
    }

    #[test]
    fn test_faction_selection() {
        let mut manager = SlotManager::new(8);

        let player_id = Uuid::new_v4();
        let player_ip = IpAddr::V4(Ipv4Addr::LOCALHOST);
        let player = LanPlayer {
            id: player_id,
            name: "Test".to_string(),
            ip: player_ip,
            ..Default::default()
        };

        manager.add_player(player_id, player_ip, player).unwrap();
        manager
            .set_player_faction(player_id, Faction::UsaAirForce)
            .unwrap();

        let slot_num = *manager.player_slots.get(&player_id).unwrap();
        let slot = manager.get_slot(slot_num).unwrap();
        assert_eq!(slot.faction, Some(Faction::UsaAirForce));
    }

    #[test]
    fn test_color_assignment() {
        let mut manager = SlotManager::new(8);

        let player1_id = Uuid::new_v4();
        let player1 = LanPlayer {
            id: player1_id,
            name: "Player1".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1)),
            ..Default::default()
        };

        manager.add_player(player1_id, player1.ip, player1).unwrap();

        // Try to set color
        manager
            .set_player_color(player1_id, PlayerColor::Blue)
            .unwrap();

        let slot_num = *manager.player_slots.get(&player1_id).unwrap();
        let slot = manager.get_slot(slot_num).unwrap();
        assert_eq!(slot.color, PlayerColor::Blue);

        // Add second player and check color conflict
        let player2_id = Uuid::new_v4();
        let player2 = LanPlayer {
            id: player2_id,
            name: "Player2".to_string(),
            ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, 2)),
            ..Default::default()
        };

        manager.add_player(player2_id, player2.ip, player2).unwrap();

        // Should fail to set same color
        assert!(manager
            .set_player_color(player2_id, PlayerColor::Blue)
            .is_err());
    }

    #[test]
    fn test_team_balancing() {
        let mut manager = SlotManager::new(8);

        // Add 4 players
        for i in 0..4 {
            let player_id = Uuid::new_v4();
            let player = LanPlayer {
                id: player_id,
                name: format!("Player{}", i),
                ip: IpAddr::V4(Ipv4Addr::new(192, 168, 1, i + 1)),
                ..Default::default()
            };
            manager.add_player(player_id, player.ip, player).unwrap();
        }

        manager.balance_teams().unwrap();
        assert!(manager.are_teams_balanced());

        let teams = manager.get_team_composition();
        assert_eq!(teams.len(), 2);
        assert_eq!(teams.get(&0).unwrap().len(), 2);
        assert_eq!(teams.get(&1).unwrap().len(), 2);
    }

    #[test]
    fn test_ai_management() {
        let mut manager = SlotManager::new(8);

        manager.add_ai(0, GameDifficulty::Hard).unwrap();
        assert_eq!(manager.get_ai_count(), 1);

        // Can't add AI to same slot
        assert!(manager.add_ai(0, GameDifficulty::Easy).is_err());
    }

    #[test]
    fn test_validation() {
        let mut manager = SlotManager::new(8);

        // Should fail with less than 2 players
        assert!(manager.validate_configuration().is_err());

        // Add 2 AI
        manager.add_ai(0, GameDifficulty::Normal).unwrap();
        manager.add_ai(1, GameDifficulty::Normal).unwrap();

        // Should pass now
        assert!(manager.validate_configuration().is_ok());
    }
}
