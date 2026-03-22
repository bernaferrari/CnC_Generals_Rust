//! PLAYER INITIALIZATION SYSTEM
//!
//! Complete player initialization from map data based on:
//! - /GeneralsMD/Code/GameEngine/Include/Common/PlayerList.h
//! - /GeneralsMD/Code/GameEngine/Include/Common/PlayerTemplate.h
//! - /GeneralsMD/Code/GameEngine/Source/GameLogic/GameLogic.cpp
//!
//! This module handles creating players from map data, setting colors,
//! resources, alliances, and handicaps.

use game_engine::common::rts::player_template::PlayerTemplate;
use game_engine::common::rts::Money;
use game_engine::common::rts::NameKeyType;
use std::collections::HashMap;

/// Maximum number of player slots
pub const MAX_PLAYER_COUNT: usize = 8;

/// Default starting money amounts (from C++ PlayerTemplate.ini)
pub const DEFAULT_STARTING_MONEY: u32 = 10000;

/// Player color type (RGBA format)
pub type PlayerColor = u32;

/// Player index type
pub type PlayerIndex = usize;

/// Default player colors (from C++ player templates)
/// Matches colors from PlayerTemplate.ini
pub const DEFAULT_PLAYER_COLORS: [PlayerColor; 8] = [
    0xFFFF0000, // Red
    0xFF0000FF, // Blue
    0xFF00FF00, // Green
    0xFFFFFF00, // Yellow
    0xFFFF8000, // Orange
    0xFF00FFFF, // Cyan
    0xFFFF00FF, // Magenta
    0xFFFFFFFF, // White
];

/// Player relationship types
/// Matches C++ Relationship enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlayerRelationship {
    Ally,
    Enemy,
    Neutral,
}

/// Player difficulty/handicap level
/// Matches C++ Difficulty enum from GameLogic
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Difficulty {
    Easy,
    Normal,
    Hard,
    Brutal,
}

impl Difficulty {
    /// Get resource multiplier for difficulty
    /// Easy players get more resources, Hard players get less
    pub fn get_resource_multiplier(&self) -> f32 {
        match self {
            Difficulty::Easy => 1.5,   // 50% more resources
            Difficulty::Normal => 1.0, // Normal resources
            Difficulty::Hard => 0.75,  // 25% less resources
            Difficulty::Brutal => 0.5, // 50% less resources
        }
    }

    /// Get build time multiplier for difficulty
    /// Easy builds faster, Hard builds slower
    pub fn get_build_time_multiplier(&self) -> f32 {
        match self {
            Difficulty::Easy => 0.75,  // 25% faster
            Difficulty::Normal => 1.0, // Normal speed
            Difficulty::Hard => 1.25,  // 25% slower
            Difficulty::Brutal => 1.5, // 50% slower
        }
    }
}

/// Create a player template seeded from defaults, matching C++ PlayerTemplate.ini usage.
pub fn make_player_template(name: &str, side: &str) -> PlayerTemplate {
    let mut template = PlayerTemplate::new(name.to_string());
    template.display_name = name.to_string();
    template.side = side.to_string();
    template.base_side = side.to_string();
    template.preferred_color = DEFAULT_PLAYER_COLORS[0];
    template.starting_money = Money::new_with_amount(DEFAULT_STARTING_MONEY);
    template
}

/// Create an observer player template (no side, no starting units).
pub fn make_observer_template() -> PlayerTemplate {
    let mut template = PlayerTemplate::new("Observer".to_string());
    template.display_name = "Observer".to_string();
    template.is_observer = true;
    template.playable = false;
    template.preferred_color = 0xFFAAAAAA;
    template.starting_money = Money::new_with_amount(0);
    template
}

/// Player instance in a game
/// Matches C++ Player class from Player.h
#[derive(Debug)]
pub struct Player {
    pub index: PlayerIndex,
    pub name: String,
    pub original_name: String,
    pub template: PlayerTemplate,
    pub color: PlayerColor,
    pub night_color: PlayerColor,
    pub current_money: u32,
    pub is_human: bool,
    pub is_ai: bool,
    pub difficulty: Difficulty,
    pub relationships: HashMap<PlayerIndex, PlayerRelationship>,
    pub handicap: f32, // 1.0 = normal, >1.0 = advantage, <1.0 = disadvantage
    pub start_position: Option<(f32, f32, f32)>,
    pub is_defeated: bool,
    pub is_observer: bool,
    pub mp_start_index: i32,
    pub is_preorder: bool,
    pub is_skirmish: bool,
    pub skirmish_difficulty: Option<Difficulty>,
}

impl Player {
    pub fn new(index: PlayerIndex, template: PlayerTemplate, is_human: bool) -> Self {
        let color = if template.preferred_color == 0 {
            DEFAULT_PLAYER_COLORS[0]
        } else {
            template.preferred_color
        };
        let starting_money = template.starting_money.count_money();
        let is_observer = template.is_observer;

        Self {
            index,
            name: template.name.clone(),
            original_name: template.name.clone(),
            template,
            color,
            night_color: color,
            current_money: starting_money,
            is_human,
            is_ai: !is_human,
            difficulty: Difficulty::Normal,
            relationships: HashMap::new(),
            handicap: 1.0,
            start_position: None,
            is_defeated: false,
            is_observer,
            mp_start_index: 0,
            is_preorder: false,
            is_skirmish: false,
            skirmish_difficulty: None,
        }
    }

    /// Set relationship with another player
    pub fn set_relationship(&mut self, other_index: PlayerIndex, relationship: PlayerRelationship) {
        self.relationships.insert(other_index, relationship);
    }

    /// Get relationship with another player
    pub fn get_relationship(&self, other_index: PlayerIndex) -> PlayerRelationship {
        *self
            .relationships
            .get(&other_index)
            .unwrap_or(&PlayerRelationship::Neutral)
    }

    /// Check if allied with another player
    pub fn is_allied_with(&self, other_index: PlayerIndex) -> bool {
        self.get_relationship(other_index) == PlayerRelationship::Ally
    }

    /// Check if enemy with another player
    pub fn is_enemy_with(&self, other_index: PlayerIndex) -> bool {
        self.get_relationship(other_index) == PlayerRelationship::Enemy
    }

    /// Set difficulty/handicap
    pub fn set_difficulty(&mut self, difficulty: Difficulty) {
        self.difficulty = difficulty;

        // Apply difficulty multipliers
        let resource_mult = difficulty.get_resource_multiplier();
        self.current_money =
            (self.template.starting_money.count_money() as f32 * resource_mult) as u32;
    }

    pub fn set_mp_start_index(&mut self, index: i32) {
        self.mp_start_index = index;
    }

    pub fn set_preorder(&mut self, value: bool) {
        self.is_preorder = value;
    }

    pub fn set_skirmish(&mut self, value: bool) {
        self.is_skirmish = value;
    }

    pub fn set_skirmish_difficulty(&mut self, difficulty: Option<Difficulty>) {
        self.skirmish_difficulty = difficulty;
    }

    /// Set custom handicap multiplier
    pub fn set_handicap(&mut self, handicap: f32) {
        self.handicap = handicap.max(0.1).min(10.0); // Clamp to reasonable range
    }

    /// Get effective resource income multiplier
    pub fn get_effective_resource_multiplier(&self) -> f32 {
        self.difficulty.get_resource_multiplier() * self.handicap
    }

    /// Get effective build speed multiplier
    pub fn get_effective_build_speed_multiplier(&self) -> f32 {
        self.difficulty.get_build_time_multiplier() / self.handicap
    }

    /// Mark player as defeated
    pub fn set_defeated(&mut self, defeated: bool) {
        self.is_defeated = defeated;
    }

    /// Check if player is still in the game
    pub fn is_active(&self) -> bool {
        !self.is_defeated && !self.is_observer
    }
}

/// Player list manager
/// Matches C++ PlayerList from PlayerList.h
pub struct PlayerList {
    players: Vec<Player>,
    local_player_index: Option<PlayerIndex>,
}

impl PlayerList {
    pub fn new() -> Self {
        Self {
            players: Vec::with_capacity(MAX_PLAYER_COUNT),
            local_player_index: None,
        }
    }

    /// Add a player to the list
    pub fn add_player(&mut self, player: Player) -> PlayerIndex {
        let index = player.index;
        self.players.push(player);
        index
    }

    /// Get player by index
    pub fn get_player(&self, index: PlayerIndex) -> Option<&Player> {
        self.players.iter().find(|p| p.index == index)
    }

    /// Get mutable player by index
    pub fn get_player_mut(&mut self, index: PlayerIndex) -> Option<&mut Player> {
        self.players.iter_mut().find(|p| p.index == index)
    }

    /// Get all players
    pub fn get_all_players(&self) -> &[Player] {
        &self.players
    }

    /// Get all mutable players
    pub fn get_all_players_mut(&mut self) -> &mut [Player] {
        &mut self.players
    }

    /// Set local player index
    pub fn set_local_player(&mut self, index: PlayerIndex) {
        self.local_player_index = Some(index);
    }

    /// Get local player index
    pub fn get_local_player_index(&self) -> Option<PlayerIndex> {
        self.local_player_index
    }

    /// Get local player
    pub fn get_local_player(&self) -> Option<&Player> {
        self.local_player_index.and_then(|idx| self.get_player(idx))
    }

    /// Get number of players
    pub fn len(&self) -> usize {
        self.players.len()
    }

    /// Check if list is empty
    pub fn is_empty(&self) -> bool {
        self.players.is_empty()
    }

    /// Count active (non-defeated) players
    pub fn count_active_players(&self) -> usize {
        self.players.iter().filter(|p| p.is_active()).count()
    }

    /// Get all human players
    pub fn get_human_players(&self) -> Vec<&Player> {
        self.players.iter().filter(|p| p.is_human).collect()
    }

    /// Get all AI players
    pub fn get_ai_players(&self) -> Vec<&Player> {
        self.players.iter().filter(|p| p.is_ai).collect()
    }

    /// Clear all players
    pub fn clear(&mut self) {
        self.players.clear();
        self.local_player_index = None;
    }

    /// Initialize team alliances (all players on same team are allies)
    /// Matches C++ logic from PlayerList initialization
    pub fn init_team_alliances(&mut self) {
        // Prefer explicit template relationship directives when available.
        self.init_relationships_from_allies_enemies();
    }

    fn enforce_observer_neutral_relations(&mut self) {
        let player_count = self.players.len();

        for i in 0..player_count {
            for j in 0..player_count {
                if i == j {
                    continue;
                }

                // Observers should not be forced into enemy relationships.
                let either_observer = self.players[i].is_observer || self.players[j].is_observer;
                if either_observer {
                    if let Some(player) = self.get_player_mut(i) {
                        player.set_relationship(j, PlayerRelationship::Neutral);
                    }
                }
            }
        }
    }

    pub fn init_relationships_from_allies_enemies(&mut self) {
        let player_count = self.players.len();

        for i in 0..player_count {
            for j in 0..player_count {
                if i == j {
                    continue;
                }
                if let Some(player) = self.get_player_mut(i) {
                    player.set_relationship(j, PlayerRelationship::Enemy);
                }
            }
        }

        for i in 0..player_count {
            let allies = self.players[i].template.player_allies.clone();
            let enemies = self.players[i].template.player_enemies.clone();

            if !allies.is_empty() {
                for token in allies.split_whitespace() {
                    if let Some(index) = self.find_player_index_by_name(token) {
                        if let Some(player) = self.get_player_mut(i) {
                            player.set_relationship(index, PlayerRelationship::Ally);
                        }
                    }
                }
            }

            if !enemies.is_empty() {
                for token in enemies.split_whitespace() {
                    if let Some(index) = self.find_player_index_by_name(token) {
                        if let Some(player) = self.get_player_mut(i) {
                            player.set_relationship(index, PlayerRelationship::Enemy);
                        }
                    }
                }
            }

            let neutral_index = self.get_neutral_player_index();
            if let Some(player) = self.get_player_mut(i) {
                player.set_relationship(i, PlayerRelationship::Ally);
                if let Some(neutral_index) = neutral_index {
                    if neutral_index != i {
                        player.set_relationship(neutral_index, PlayerRelationship::Neutral);
                    }
                }
            }
        }

        self.enforce_observer_neutral_relations();
    }

    fn get_neutral_player_index(&self) -> Option<PlayerIndex> {
        self.players
            .iter()
            .position(|player| player.name.eq_ignore_ascii_case("Neutral"))
    }

    fn find_player_index_by_name(&self, name: &str) -> Option<PlayerIndex> {
        self.players
            .iter()
            .position(|player| player.original_name.eq_ignore_ascii_case(name))
    }
}

impl Default for PlayerList {
    fn default() -> Self {
        Self::new()
    }
}

/// Player initialization helper
/// Handles creating players from map data and templates
pub struct PlayerInitializer;

impl PlayerInitializer {
    /// Initialize players from map waypoints and templates
    /// Matches C++ player initialization from GameLogic::startNewGame
    pub fn init_from_map(
        num_players: usize,
        player_templates: &[PlayerTemplate],
        start_positions: &[(f32, f32, f32)],
    ) -> PlayerList {
        Self::init_from_map_with_human_flags(num_players, player_templates, start_positions, None)
    }

    /// Initialize players with optional human flags (from SidesList).
    pub fn init_from_map_with_human_flags(
        num_players: usize,
        player_templates: &[PlayerTemplate],
        start_positions: &[(f32, f32, f32)],
        human_flags: Option<&[bool]>,
    ) -> PlayerList {
        let mut player_list = PlayerList::new();

        let player_count = num_players.min(MAX_PLAYER_COUNT);

        for i in 0..player_count {
            // Get template for this player
            let template = player_templates
                .get(i)
                .cloned()
                .unwrap_or_else(|| make_player_template(&format!("Player {}", i + 1), "USA"));

            // Create player (first player is human by default)
            let is_human = human_flags
                .and_then(|flags| flags.get(i).copied())
                .unwrap_or(i == 0);
            let mut player = Player::new(i, template, is_human);

            // Assign color (preferred color if specified, else cycle defaults)
            let preferred = player.template.preferred_color;
            player.color = if preferred != 0 {
                preferred
            } else {
                DEFAULT_PLAYER_COLORS[i % DEFAULT_PLAYER_COLORS.len()]
            };

            // Assign start position if available
            if let Some(&pos) = start_positions.get(i) {
                player.start_position = Some(pos);
            }

            player_list.add_player(player);
        }

        // Set first player as local
        if !player_list.is_empty() {
            player_list.set_local_player(0);
        }

        // Initialize alliances
        player_list.init_team_alliances();

        player_list
    }

    /// Apply handicaps to players
    pub fn apply_handicaps(player_list: &mut PlayerList, handicaps: &[(PlayerIndex, f32)]) {
        for &(index, handicap) in handicaps {
            if let Some(player) = player_list.get_player_mut(index) {
                player.set_handicap(handicap);
            }
        }
    }

    /// Set player difficulties
    pub fn set_difficulties(
        player_list: &mut PlayerList,
        difficulties: &[(PlayerIndex, Difficulty)],
    ) {
        for &(index, difficulty) in difficulties {
            if let Some(player) = player_list.get_player_mut(index) {
                player.set_difficulty(difficulty);
            }
        }
    }

    /// Setup free-for-all (all vs all)
    pub fn setup_ffa(player_list: &mut PlayerList) {
        let player_count = player_list.len();

        for i in 0..player_count {
            if let Some(player) = player_list.get_player_mut(i) {
                for j in 0..player_count {
                    if i != j {
                        player.set_relationship(j, PlayerRelationship::Enemy);
                    }
                }
            }
        }
    }

    /// Setup teams (players with same team ID are allies)
    pub fn setup_teams(player_list: &mut PlayerList, team_assignments: &[(PlayerIndex, u32)]) {
        let mut teams: HashMap<u32, Vec<PlayerIndex>> = HashMap::new();

        // Group players by team
        for &(player_index, team_id) in team_assignments {
            teams
                .entry(team_id)
                .or_insert_with(Vec::new)
                .push(player_index);
        }

        // Set up relationships
        let player_count = player_list.len();
        for i in 0..player_count {
            if let Some(player) = player_list.get_player_mut(i) {
                // Find this player's team
                let my_team = team_assignments
                    .iter()
                    .find(|&&(idx, _)| idx == i)
                    .map(|&(_, team)| team);

                for j in 0..player_count {
                    if i == j {
                        continue;
                    }

                    let other_team = team_assignments
                        .iter()
                        .find(|&&(idx, _)| idx == j)
                        .map(|&(_, team)| team);

                    let relationship = if my_team.is_some() && my_team == other_team {
                        PlayerRelationship::Ally
                    } else {
                        PlayerRelationship::Enemy
                    };

                    player.set_relationship(j, relationship);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_difficulty_multipliers() {
        assert_eq!(Difficulty::Easy.get_resource_multiplier(), 1.5);
        assert_eq!(Difficulty::Normal.get_resource_multiplier(), 1.0);
        assert_eq!(Difficulty::Hard.get_resource_multiplier(), 0.75);
        assert_eq!(Difficulty::Brutal.get_resource_multiplier(), 0.5);
    }

    #[test]
    fn test_player_creation() {
        let template = make_player_template("Player 1", "USA");
        let player = Player::new(0, template, true);

        assert_eq!(player.index, 0);
        assert!(player.is_human);
        assert!(!player.is_ai);
        assert_eq!(player.current_money, DEFAULT_STARTING_MONEY);
        assert!(!player.is_defeated);
    }

    #[test]
    fn test_player_relationships() {
        let template = make_player_template("Player 1", "USA");
        let mut player = Player::new(0, template, true);

        player.set_relationship(1, PlayerRelationship::Ally);
        player.set_relationship(2, PlayerRelationship::Enemy);

        assert!(player.is_allied_with(1));
        assert!(player.is_enemy_with(2));
        assert_eq!(player.get_relationship(3), PlayerRelationship::Neutral);
    }

    #[test]
    fn test_player_list() {
        let mut player_list = PlayerList::new();

        let template1 = make_player_template("Player 1", "USA");
        let template2 = make_player_template("Player 2", "China");

        let player1 = Player::new(0, template1, true);
        let player2 = Player::new(1, template2, false);

        player_list.add_player(player1);
        player_list.add_player(player2);

        assert_eq!(player_list.len(), 2);
        assert_eq!(player_list.count_active_players(), 2);

        player_list.set_local_player(0);
        assert_eq!(player_list.get_local_player_index(), Some(0));
    }

    #[test]
    fn test_player_initializer() {
        let templates = vec![
            make_player_template("Player 1", "USA"),
            make_player_template("Player 2", "China"),
        ];

        let start_positions = vec![(100.0, 100.0, 0.0), (500.0, 500.0, 0.0)];

        let player_list = PlayerInitializer::init_from_map(2, &templates, &start_positions);

        assert_eq!(player_list.len(), 2);
        assert_eq!(player_list.get_local_player_index(), Some(0));

        let player1 = player_list.get_player(0).unwrap();
        assert_eq!(player1.start_position, Some((100.0, 100.0, 0.0)));
    }

    #[test]
    fn test_ffa_setup() {
        let templates = vec![
            make_player_template("Player 1", "USA"),
            make_player_template("Player 2", "China"),
            make_player_template("Player 3", "GLA"),
        ];

        let mut player_list = PlayerInitializer::init_from_map(3, &templates, &[]);
        PlayerInitializer::setup_ffa(&mut player_list);

        let player1 = player_list.get_player(0).unwrap();
        assert!(player1.is_enemy_with(1));
        assert!(player1.is_enemy_with(2));
    }

    #[test]
    fn test_team_setup() {
        let templates = vec![
            make_player_template("Player 1", "USA"),
            make_player_template("Player 2", "China"),
            make_player_template("Player 3", "GLA"),
            make_player_template("Player 4", "USA"),
        ];

        let mut player_list = PlayerInitializer::init_from_map(4, &templates, &[]);

        // Team 0: Players 0, 1
        // Team 1: Players 2, 3
        let teams = vec![(0, 0), (1, 0), (2, 1), (3, 1)];
        PlayerInitializer::setup_teams(&mut player_list, &teams);

        let player1 = player_list.get_player(0).unwrap();
        assert!(player1.is_allied_with(1)); // Same team
        assert!(player1.is_enemy_with(2)); // Different team
        assert!(player1.is_enemy_with(3)); // Different team
    }

    #[test]
    fn test_init_team_alliances_applies_template_allies() {
        let mut alpha = make_player_template("Alpha", "USA");
        let mut bravo = make_player_template("Bravo", "China");
        alpha.player_allies = "Bravo".to_string();
        bravo.player_allies = "Alpha".to_string();

        let templates = vec![alpha, bravo];
        let player_list = PlayerInitializer::init_from_map(2, &templates, &[]);

        let a = player_list.get_player(0).unwrap();
        let b = player_list.get_player(1).unwrap();
        assert!(a.is_allied_with(1));
        assert!(b.is_allied_with(0));
    }

    #[test]
    fn test_init_team_alliances_keeps_observers_neutral() {
        let templates = vec![
            make_player_template("Player", "USA"),
            make_observer_template(),
        ];
        let player_list = PlayerInitializer::init_from_map(2, &templates, &[]);

        let player = player_list.get_player(0).unwrap();
        let observer = player_list.get_player(1).unwrap();
        assert_eq!(player.get_relationship(1), PlayerRelationship::Neutral);
        assert_eq!(observer.get_relationship(0), PlayerRelationship::Neutral);
    }

    #[test]
    fn test_init_relationships_from_allies_enemies_keeps_observers_neutral() {
        let mut player_list = PlayerList::new();
        player_list.add_player(Player::new(0, make_player_template("Player", "USA"), true));
        player_list.add_player(Player::new(1, make_observer_template(), false));

        player_list.init_relationships_from_allies_enemies();

        let player = player_list.get_player(0).unwrap();
        let observer = player_list.get_player(1).unwrap();
        assert_eq!(player.get_relationship(1), PlayerRelationship::Neutral);
        assert_eq!(observer.get_relationship(0), PlayerRelationship::Neutral);
    }
}
