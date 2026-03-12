//! Alliance system - Complete implementation of team alliances
//!
//! This module handles alliance relationships between players and teams,
//! including shared vision, shared radar, and team-based targeting.
//! Matches C++ Player and Team relationship systems.

use crate::common::*;
use crate::player::{Player, PlayerIndex, PLAYER_INDEX_INVALID};
use crate::team::{Team, TeamID, TEAM_ID_INVALID};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Alliance manager for handling all alliance-related operations
/// Provides centralized management of alliances between players and teams
#[derive(Debug)]
pub struct AllianceManager {
    /// Player-to-player alliances (bidirectional)
    player_alliances: HashMap<PlayerIndex, HashSet<PlayerIndex>>,

    /// Player-to-player enemies (bidirectional)
    player_enemies: HashMap<PlayerIndex, HashSet<PlayerIndex>>,

    /// Team-to-team alliances (bidirectional)
    team_alliances: HashMap<TeamID, HashSet<TeamID>>,

    /// Team-to-team enemies (bidirectional)
    team_enemies: HashMap<TeamID, HashSet<TeamID>>,

    /// Shared vision settings per player
    shared_vision_enabled: HashMap<PlayerIndex, bool>,

    /// Shared radar settings per player
    shared_radar_enabled: HashMap<PlayerIndex, bool>,
}

impl AllianceManager {
    /// Create a new alliance manager
    pub fn new() -> Self {
        Self {
            player_alliances: HashMap::new(),
            player_enemies: HashMap::new(),
            team_alliances: HashMap::new(),
            team_enemies: HashMap::new(),
            shared_vision_enabled: HashMap::new(),
            shared_radar_enabled: HashMap::new(),
        }
    }

    /// Reset all alliances
    pub fn reset(&mut self) {
        self.player_alliances.clear();
        self.player_enemies.clear();
        self.team_alliances.clear();
        self.team_enemies.clear();
        self.shared_vision_enabled.clear();
        self.shared_radar_enabled.clear();
    }

    // ==================== Player Alliance Management ====================

    /// Form alliance between two players (bidirectional)
    /// Matches C++ Player::setPlayerRelationship with ALLIES
    pub fn ally_players(&mut self, player1: PlayerIndex, player2: PlayerIndex) {
        if player1 == PLAYER_INDEX_INVALID || player2 == PLAYER_INDEX_INVALID {
            return;
        }

        // Remove any existing enemy relationship
        self.break_player_enemy(player1, player2);

        // Add bidirectional alliance
        self.player_alliances
            .entry(player1)
            .or_insert_with(HashSet::new)
            .insert(player2);
        self.player_alliances
            .entry(player2)
            .or_insert_with(HashSet::new)
            .insert(player1);

        // Enable shared vision by default
        self.shared_vision_enabled.insert(player1, true);
        self.shared_vision_enabled.insert(player2, true);

        // Enable shared radar by default
        self.shared_radar_enabled.insert(player1, true);
        self.shared_radar_enabled.insert(player2, true);
    }

    /// Make two players enemies (bidirectional)
    /// Matches C++ Player::setPlayerRelationship with ENEMY
    pub fn make_players_enemies(&mut self, player1: PlayerIndex, player2: PlayerIndex) {
        if player1 == PLAYER_INDEX_INVALID || player2 == PLAYER_INDEX_INVALID {
            return;
        }

        // Remove any existing alliance
        self.break_player_alliance(player1, player2);

        // Add bidirectional enemy relationship
        self.player_enemies
            .entry(player1)
            .or_insert_with(HashSet::new)
            .insert(player2);
        self.player_enemies
            .entry(player2)
            .or_insert_with(HashSet::new)
            .insert(player1);
    }

    /// Break alliance between two players
    pub fn break_player_alliance(&mut self, player1: PlayerIndex, player2: PlayerIndex) {
        if let Some(allies) = self.player_alliances.get_mut(&player1) {
            allies.remove(&player2);
        }
        if let Some(allies) = self.player_alliances.get_mut(&player2) {
            allies.remove(&player1);
        }
    }

    /// Break enemy relationship between two players
    pub fn break_player_enemy(&mut self, player1: PlayerIndex, player2: PlayerIndex) {
        if let Some(enemies) = self.player_enemies.get_mut(&player1) {
            enemies.remove(&player2);
        }
        if let Some(enemies) = self.player_enemies.get_mut(&player2) {
            enemies.remove(&player1);
        }
    }

    /// Check if two players are allied
    pub fn are_players_allied(&self, player1: PlayerIndex, player2: PlayerIndex) -> bool {
        if player1 == player2 {
            return true; // Player is always allied with themselves
        }

        self.player_alliances
            .get(&player1)
            .map(|allies| allies.contains(&player2))
            .unwrap_or(false)
    }

    /// Check if two players are enemies
    pub fn are_players_enemies(&self, player1: PlayerIndex, player2: PlayerIndex) -> bool {
        self.player_enemies
            .get(&player1)
            .map(|enemies| enemies.contains(&player2))
            .unwrap_or(false)
    }

    /// Get all allies of a player
    pub fn get_player_allies(&self, player: PlayerIndex) -> Vec<PlayerIndex> {
        self.player_alliances
            .get(&player)
            .map(|allies| allies.iter().copied().collect())
            .unwrap_or_else(Vec::new)
    }

    /// Get all enemies of a player
    pub fn get_player_enemies(&self, player: PlayerIndex) -> Vec<PlayerIndex> {
        self.player_enemies
            .get(&player)
            .map(|enemies| enemies.iter().copied().collect())
            .unwrap_or_else(Vec::new)
    }

    // ==================== Team Alliance Management ====================

    /// Form alliance between two teams (bidirectional)
    /// Matches C++ Team::setOverrideTeamRelationship with ALLIES
    pub fn ally_teams(&mut self, team1: TeamID, team2: TeamID) {
        if team1 == TEAM_ID_INVALID || team2 == TEAM_ID_INVALID {
            return;
        }

        // Remove any existing enemy relationship
        self.break_team_enemy(team1, team2);

        // Add bidirectional alliance
        self.team_alliances
            .entry(team1)
            .or_insert_with(HashSet::new)
            .insert(team2);
        self.team_alliances
            .entry(team2)
            .or_insert_with(HashSet::new)
            .insert(team1);
    }

    /// Make two teams enemies (bidirectional)
    /// Matches C++ Team::setOverrideTeamRelationship with ENEMY
    pub fn make_teams_enemies(&mut self, team1: TeamID, team2: TeamID) {
        if team1 == TEAM_ID_INVALID || team2 == TEAM_ID_INVALID {
            return;
        }

        // Remove any existing alliance
        self.break_team_alliance(team1, team2);

        // Add bidirectional enemy relationship
        self.team_enemies
            .entry(team1)
            .or_insert_with(HashSet::new)
            .insert(team2);
        self.team_enemies
            .entry(team2)
            .or_insert_with(HashSet::new)
            .insert(team1);
    }

    /// Break alliance between two teams
    pub fn break_team_alliance(&mut self, team1: TeamID, team2: TeamID) {
        if let Some(allies) = self.team_alliances.get_mut(&team1) {
            allies.remove(&team2);
        }
        if let Some(allies) = self.team_alliances.get_mut(&team2) {
            allies.remove(&team1);
        }
    }

    /// Break enemy relationship between two teams
    pub fn break_team_enemy(&mut self, team1: TeamID, team2: TeamID) {
        if let Some(enemies) = self.team_enemies.get_mut(&team1) {
            enemies.remove(&team2);
        }
        if let Some(enemies) = self.team_enemies.get_mut(&team2) {
            enemies.remove(&team1);
        }
    }

    /// Check if two teams are allied
    pub fn are_teams_allied(&self, team1: TeamID, team2: TeamID) -> bool {
        if team1 == team2 {
            return true; // Team is always allied with itself
        }

        self.team_alliances
            .get(&team1)
            .map(|allies| allies.contains(&team2))
            .unwrap_or(false)
    }

    /// Check if two teams are enemies
    pub fn are_teams_enemies(&self, team1: TeamID, team2: TeamID) -> bool {
        self.team_enemies
            .get(&team1)
            .map(|enemies| enemies.contains(&team2))
            .unwrap_or(false)
    }

    /// Get all allies of a team
    pub fn get_team_allies(&self, team: TeamID) -> Vec<TeamID> {
        self.team_alliances
            .get(&team)
            .map(|allies| allies.iter().copied().collect())
            .unwrap_or_else(Vec::new)
    }

    /// Get all enemies of a team
    pub fn get_team_enemies(&self, team: TeamID) -> Vec<TeamID> {
        self.team_enemies
            .get(&team)
            .map(|enemies| enemies.iter().copied().collect())
            .unwrap_or_else(Vec::new)
    }

    // ==================== Shared Vision Management ====================

    /// Enable shared vision for a player with their allies
    pub fn enable_shared_vision(&mut self, player: PlayerIndex) {
        self.shared_vision_enabled.insert(player, true);
    }

    /// Disable shared vision for a player with their allies
    pub fn disable_shared_vision(&mut self, player: PlayerIndex) {
        self.shared_vision_enabled.insert(player, false);
    }

    /// Check if a player has shared vision enabled
    pub fn has_shared_vision_enabled(&self, player: PlayerIndex) -> bool {
        self.shared_vision_enabled
            .get(&player)
            .copied()
            .unwrap_or(true)
    }

    /// Get all players who share vision with the given player
    pub fn get_vision_shared_players(&self, player: PlayerIndex) -> Vec<PlayerIndex> {
        if !self.has_shared_vision_enabled(player) {
            return vec![player]; // Only see own units
        }

        let mut shared = vec![player];
        if let Some(allies) = self.player_alliances.get(&player) {
            for &ally in allies {
                if self.has_shared_vision_enabled(ally) {
                    shared.push(ally);
                }
            }
        }
        shared
    }

    // ==================== Shared Radar Management ====================

    /// Enable shared radar for a player with their allies
    pub fn enable_shared_radar(&mut self, player: PlayerIndex) {
        self.shared_radar_enabled.insert(player, true);
    }

    /// Disable shared radar for a player with their allies
    pub fn disable_shared_radar(&mut self, player: PlayerIndex) {
        self.shared_radar_enabled.insert(player, false);
    }

    /// Check if a player has shared radar enabled
    pub fn has_shared_radar_enabled(&self, player: PlayerIndex) -> bool {
        self.shared_radar_enabled
            .get(&player)
            .copied()
            .unwrap_or(true)
    }

    /// Get all players who share radar with the given player
    pub fn get_radar_shared_players(&self, player: PlayerIndex) -> Vec<PlayerIndex> {
        if !self.has_shared_radar_enabled(player) {
            return vec![player]; // Only see own radar
        }

        let mut shared = vec![player];
        if let Some(allies) = self.player_alliances.get(&player) {
            for &ally in allies {
                if self.has_shared_radar_enabled(ally) {
                    shared.push(ally);
                }
            }
        }
        shared
    }

    // ==================== Targeting Logic ====================

    /// Check if a player/team can target another player/team
    /// Returns true if they are enemies or neutral (with appropriate settings)
    pub fn can_player_target_player(&self, attacker: PlayerIndex, target: PlayerIndex) -> bool {
        if attacker == target {
            return false; // Cannot target self
        }

        // Can target if enemy
        if self.are_players_enemies(attacker, target) {
            return true;
        }

        // Cannot target if allied
        if self.are_players_allied(attacker, target) {
            return false;
        }

        // Can target neutral players by default
        true
    }

    /// Check if a team can target another team
    pub fn can_team_target_team(&self, attacker: TeamID, target: TeamID) -> bool {
        if attacker == target {
            return false; // Cannot target self
        }

        // Can target if enemy
        if self.are_teams_enemies(attacker, target) {
            return true;
        }

        // Cannot target if allied
        if self.are_teams_allied(attacker, target) {
            return false;
        }

        // Can target neutral teams by default
        true
    }

    // ==================== Diplomacy Changes ====================

    /// Change diplomacy between players during game
    /// Matches C++ scripting actions for diplomacy
    pub fn change_player_diplomacy(
        &mut self,
        player1: PlayerIndex,
        player2: PlayerIndex,
        relationship: Relationship,
    ) {
        match relationship {
            Relationship::Ally => self.ally_players(player1, player2),
            Relationship::Enemy => self.make_players_enemies(player1, player2),
            Relationship::Neutral => {
                self.break_player_alliance(player1, player2);
                self.break_player_enemy(player1, player2);
            }
            _ => {}
        }
    }

    /// Change diplomacy between teams during game
    pub fn change_team_diplomacy(
        &mut self,
        team1: TeamID,
        team2: TeamID,
        relationship: Relationship,
    ) {
        match relationship {
            Relationship::Ally => self.ally_teams(team1, team2),
            Relationship::Enemy => self.make_teams_enemies(team1, team2),
            Relationship::Neutral => {
                self.break_team_alliance(team1, team2);
                self.break_team_enemy(team1, team2);
            }
            _ => {}
        }
    }
}

impl Default for AllianceManager {
    fn default() -> Self {
        Self::new()
    }
}

// ==================== Global Alliance Manager ====================

use std::sync::OnceLock;

/// Global alliance manager instance
static ALLIANCE_MANAGER: OnceLock<RwLock<AllianceManager>> = OnceLock::new();

/// Get global alliance manager instance
pub fn get_alliance_manager() -> &'static RwLock<AllianceManager> {
    ALLIANCE_MANAGER.get_or_init(|| RwLock::new(AllianceManager::new()))
}

/// Convenience alias for C++ compatibility
pub use get_alliance_manager as TheAllianceManager;

// ==================== Victory Condition Support ====================

/// Team victory checker
#[derive(Debug)]
pub struct TeamVictoryChecker {
    /// Teams that must be defeated for victory
    defeat_teams: HashSet<TeamID>,

    /// Teams that must survive for victory
    survival_teams: HashSet<TeamID>,

    /// Players that must be defeated for victory
    defeat_players: HashSet<PlayerIndex>,

    /// Players that must survive for victory
    survival_players: HashSet<PlayerIndex>,
}

impl TeamVictoryChecker {
    /// Create a new team victory checker
    pub fn new() -> Self {
        Self {
            defeat_teams: HashSet::new(),
            survival_teams: HashSet::new(),
            defeat_players: HashSet::new(),
            survival_players: HashSet::new(),
        }
    }

    /// Add team that must be defeated
    pub fn add_defeat_team(&mut self, team: TeamID) {
        self.defeat_teams.insert(team);
    }

    /// Add team that must survive
    pub fn add_survival_team(&mut self, team: TeamID) {
        self.survival_teams.insert(team);
    }

    /// Add player that must be defeated
    pub fn add_defeat_player(&mut self, player: PlayerIndex) {
        self.defeat_players.insert(player);
    }

    /// Add player that must survive
    pub fn add_survival_player(&mut self, player: PlayerIndex) {
        self.survival_players.insert(player);
    }

    /// Check if victory conditions are met
    /// Returns (victory, defeat) tuple
    pub fn check_victory_conditions(
        &self,
        alive_teams: &HashSet<TeamID>,
        alive_players: &HashSet<PlayerIndex>,
    ) -> (bool, bool) {
        // Check defeat conditions (all survival teams/players dead = defeat)
        let defeat = !self.survival_teams.is_subset(alive_teams)
            || !self.survival_players.is_subset(alive_players);

        // Check victory conditions (all defeat teams/players dead = victory)
        let all_defeat_teams_dead = self
            .defeat_teams
            .iter()
            .all(|team| !alive_teams.contains(team));
        let all_defeat_players_dead = self
            .defeat_players
            .iter()
            .all(|player| !alive_players.contains(player));

        let victory = all_defeat_teams_dead && all_defeat_players_dead;

        (victory, defeat)
    }

    /// Reset victory conditions
    pub fn reset(&mut self) {
        self.defeat_teams.clear();
        self.survival_teams.clear();
        self.defeat_players.clear();
        self.survival_players.clear();
    }
}

impl Default for TeamVictoryChecker {
    fn default() -> Self {
        Self::new()
    }
}
