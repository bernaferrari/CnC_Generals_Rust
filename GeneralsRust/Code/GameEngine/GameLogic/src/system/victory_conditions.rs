//! VICTORY CONDITIONS AND SCORE TRACKING
//!
//! Complete victory/defeat detection and score system based on:
//! - /GeneralsMD/Code/GameEngine/Include/GameLogic/VictoryConditions.h
//! - /GeneralsMD/Code/GameEngine/Source/GameLogic/VictoryConditions.cpp
//! - /GeneralsMD/Code/GameEngine/Include/Common/ScoreKeeper.h
//!
//! This module handles:
//! - Victory condition checking
//! - Defeat/elimination detection
//! - Score tracking per player
//! - Game end states

use super::player_init::{Player, PlayerIndex, PlayerList};
use std::collections::HashMap;
use std::time::{Duration, Instant};

/// Victory condition types
/// Matches C++ VictoryType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VictoryType {
    /// Destroy all enemy forces
    Annihilation,
    /// Hold specific map positions for time
    CaptureTheFlag,
    /// Score-based victory
    ScoreLimit,
    /// Time-based victory
    TimeLimit,
    /// Custom script-defined victory
    Custom,
}

/// Player game result
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Victory,
    Defeat,
    Draw,
    Undecided,
}

/// Score categories for tracking
/// Matches C++ score types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ScoreCategory {
    UnitsBuilt,
    UnitsDestroyed,
    BuildingsBuilt,
    BuildingsDestroyed,
    UnitsLost,
    BuildingsLost,
    SuppliesCollected,
    SuppliesSpent,
    DamageDealt,
    DamageTaken,
    KillValue,
    DeathValue,
    Total,
}

/// Score data per player
#[derive(Debug, Clone)]
pub struct PlayerScore {
    pub player_index: PlayerIndex,
    pub scores: HashMap<ScoreCategory, u64>,
}

impl PlayerScore {
    pub fn new(player_index: PlayerIndex) -> Self {
        let mut scores = HashMap::new();

        // Initialize all categories to 0
        scores.insert(ScoreCategory::UnitsBuilt, 0);
        scores.insert(ScoreCategory::UnitsDestroyed, 0);
        scores.insert(ScoreCategory::BuildingsBuilt, 0);
        scores.insert(ScoreCategory::BuildingsDestroyed, 0);
        scores.insert(ScoreCategory::UnitsLost, 0);
        scores.insert(ScoreCategory::BuildingsLost, 0);
        scores.insert(ScoreCategory::SuppliesCollected, 0);
        scores.insert(ScoreCategory::SuppliesSpent, 0);
        scores.insert(ScoreCategory::DamageDealt, 0);
        scores.insert(ScoreCategory::DamageTaken, 0);
        scores.insert(ScoreCategory::KillValue, 0);
        scores.insert(ScoreCategory::DeathValue, 0);
        scores.insert(ScoreCategory::Total, 0);

        Self {
            player_index,
            scores,
        }
    }

    /// Add to score in category
    pub fn add_score(&mut self, category: ScoreCategory, amount: u64) {
        *self.scores.entry(category).or_insert(0) += amount;

        // Update total (except when adding to Total directly)
        if category != ScoreCategory::Total {
            *self.scores.entry(ScoreCategory::Total).or_insert(0) += amount;
        }
    }

    /// Get score for category
    pub fn get_score(&self, category: ScoreCategory) -> u64 {
        *self.scores.get(&category).unwrap_or(&0)
    }

    /// Get total score
    pub fn get_total_score(&self) -> u64 {
        self.get_score(ScoreCategory::Total)
    }

    /// Calculate kill/death ratio
    pub fn get_kill_death_ratio(&self) -> f64 {
        let kills = self.get_score(ScoreCategory::KillValue);
        let deaths = self.get_score(ScoreCategory::DeathValue);

        if deaths == 0 {
            kills as f64
        } else {
            kills as f64 / deaths as f64
        }
    }
}

/// Score keeper tracking all player scores
/// Matches C++ ScoreKeeper
pub struct ScoreKeeper {
    player_scores: HashMap<PlayerIndex, PlayerScore>,
    game_start_time: Instant,
}

impl ScoreKeeper {
    pub fn new() -> Self {
        Self {
            player_scores: HashMap::new(),
            game_start_time: Instant::now(),
        }
    }

    /// Initialize score tracking for a player
    pub fn init_player(&mut self, player_index: PlayerIndex) {
        self.player_scores
            .insert(player_index, PlayerScore::new(player_index));
    }

    /// Add score for a player
    pub fn add_score(&mut self, player_index: PlayerIndex, category: ScoreCategory, amount: u64) {
        if let Some(score) = self.player_scores.get_mut(&player_index) {
            score.add_score(category, amount);
        }
    }

    /// Get player score
    pub fn get_player_score(&self, player_index: PlayerIndex) -> Option<&PlayerScore> {
        self.player_scores.get(&player_index)
    }

    /// Get all scores
    pub fn get_all_scores(&self) -> &HashMap<PlayerIndex, PlayerScore> {
        &self.player_scores
    }

    /// Get game duration
    pub fn get_game_duration(&self) -> Duration {
        self.game_start_time.elapsed()
    }

    /// Get highest scoring player
    pub fn get_highest_scorer(&self) -> Option<PlayerIndex> {
        self.player_scores
            .iter()
            .max_by_key(|(_, score)| score.get_total_score())
            .map(|(idx, _)| *idx)
    }

    /// Clear all scores
    pub fn clear(&mut self) {
        self.player_scores.clear();
        self.game_start_time = Instant::now();
    }
}

impl Default for ScoreKeeper {
    fn default() -> Self {
        Self::new()
    }
}

/// Victory condition checker
/// Matches C++ VictoryConditions
pub struct VictoryConditions {
    victory_type: VictoryType,
    score_limit: u64,
    time_limit: Option<Duration>,
    game_results: HashMap<PlayerIndex, GameResult>,
    game_ended: bool,
    winner_indices: Vec<PlayerIndex>,
}

impl VictoryConditions {
    pub fn new(victory_type: VictoryType) -> Self {
        Self {
            victory_type,
            score_limit: 10000,
            time_limit: None,
            game_results: HashMap::new(),
            game_ended: false,
            winner_indices: Vec::new(),
        }
    }

    /// Set score limit for score-based victory
    pub fn set_score_limit(&mut self, limit: u64) {
        self.score_limit = limit;
    }

    /// Set time limit for time-based victory
    pub fn set_time_limit(&mut self, duration: Duration) {
        self.time_limit = Some(duration);
    }

    /// Check victory conditions
    /// Matches C++ VictoryConditions::update()
    pub fn check_conditions(
        &mut self,
        player_list: &PlayerList,
        score_keeper: &ScoreKeeper,
    ) -> bool {
        if self.game_ended {
            return true;
        }

        match self.victory_type {
            VictoryType::Annihilation => self.check_annihilation(player_list),
            VictoryType::ScoreLimit => self.check_score_limit(player_list, score_keeper),
            VictoryType::TimeLimit => self.check_time_limit(score_keeper),
            VictoryType::CaptureTheFlag | VictoryType::Custom => {
                // Would be handled by scripts
                false
            }
        }
    }

    /// Check annihilation victory (last player/team standing)
    fn check_annihilation(&mut self, player_list: &PlayerList) -> bool {
        let active_players: Vec<&Player> = player_list
            .get_all_players()
            .iter()
            .filter(|p| p.is_active())
            .collect();

        if active_players.len() <= 1 {
            // Game over - one or zero players remain
            for player in player_list.get_all_players() {
                if player.is_active() {
                    self.game_results.insert(player.index, GameResult::Victory);
                    self.winner_indices.push(player.index);
                } else {
                    self.game_results.insert(player.index, GameResult::Defeat);
                }
            }

            self.game_ended = true;
            return true;
        }

        false
    }

    /// Check score limit victory
    fn check_score_limit(&mut self, player_list: &PlayerList, score_keeper: &ScoreKeeper) -> bool {
        // Check if any player has reached score limit
        for player in player_list.get_all_players() {
            if let Some(score) = score_keeper.get_player_score(player.index) {
                if score.get_total_score() >= self.score_limit {
                    // This player wins
                    self.game_results.insert(player.index, GameResult::Victory);
                    self.winner_indices.push(player.index);

                    // All others lose
                    for other_player in player_list.get_all_players() {
                        if other_player.index != player.index {
                            self.game_results
                                .insert(other_player.index, GameResult::Defeat);
                        }
                    }

                    self.game_ended = true;
                    return true;
                }
            }
        }

        false
    }

    /// Check time limit victory (highest score wins)
    fn check_time_limit(&mut self, score_keeper: &ScoreKeeper) -> bool {
        if let Some(time_limit) = self.time_limit {
            if score_keeper.get_game_duration() >= time_limit {
                // Time's up - highest scorer wins
                if let Some(winner_index) = score_keeper.get_highest_scorer() {
                    self.game_results.insert(winner_index, GameResult::Victory);
                    self.winner_indices.push(winner_index);

                    // Set results for all other players
                    for (player_index, _) in score_keeper.get_all_scores() {
                        if *player_index != winner_index {
                            self.game_results.insert(*player_index, GameResult::Defeat);
                        }
                    }
                }

                self.game_ended = true;
                return true;
            }
        }

        false
    }

    /// Get game result for a player
    pub fn get_player_result(&self, player_index: PlayerIndex) -> GameResult {
        *self
            .game_results
            .get(&player_index)
            .unwrap_or(&GameResult::Undecided)
    }

    /// Check if game has ended
    pub fn is_game_ended(&self) -> bool {
        self.game_ended
    }

    /// Get winners
    pub fn get_winners(&self) -> &[PlayerIndex] {
        &self.winner_indices
    }

    /// Get victory type
    pub fn get_victory_type(&self) -> VictoryType {
        self.victory_type
    }

    /// Reset conditions
    pub fn reset(&mut self) {
        self.game_results.clear();
        self.winner_indices.clear();
        self.game_ended = false;
    }
}

/// Elimination detection
/// Handles checking if a player should be eliminated
pub struct EliminationDetector;

/// Multiplayer elimination flags from C++ VictoryConditions.h.
/// `NO_BUILDINGS` and `NO_UNITS` can be combined.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultiplayerEliminationFlags(u8);

impl MultiplayerEliminationFlags {
    pub const NONE: Self = Self(0);
    pub const NO_BUILDINGS: Self = Self(1);
    pub const NO_UNITS: Self = Self(2);
    pub const DEFAULT: Self = Self(Self::NO_BUILDINGS.0 | Self::NO_UNITS.0);

    #[inline]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }
}

impl EliminationDetector {
    /// Check if player should be eliminated
    /// Returns true if player has no units/buildings based on C++ victory flags.
    ///
    /// C++ parity note:
    /// - `VICTORY_NOUNITS | VICTORY_NOBUILDINGS`: eliminate when both are zero (`hasAnyObjects == false`)
    /// - `VICTORY_NOUNITS`: eliminate when units are zero
    /// - `VICTORY_NOBUILDINGS`: eliminate when buildings are zero
    ///
    /// `can_build` is currently ignored for parity because C++ VictoryConditions.cpp does not
    /// include production capability in defeat checks.
    pub fn should_eliminate_player(
        player: &Player,
        unit_count: usize,
        building_count: usize,
        can_build: bool,
    ) -> bool {
        Self::should_eliminate_player_with_flags(
            player,
            unit_count,
            building_count,
            can_build,
            MultiplayerEliminationFlags::DEFAULT,
        )
    }

    /// Check if player should be eliminated using explicit C++ multiplayer victory flags.
    pub fn should_eliminate_player_with_flags(
        player: &Player,
        unit_count: usize,
        building_count: usize,
        _can_build: bool,
        flags: MultiplayerEliminationFlags,
    ) -> bool {
        if !player.is_active() {
            return false;
        }

        let no_units = flags.contains(MultiplayerEliminationFlags::NO_UNITS);
        let no_buildings = flags.contains(MultiplayerEliminationFlags::NO_BUILDINGS);

        match (no_units, no_buildings) {
            (true, true) => unit_count == 0 && building_count == 0,
            (true, false) => unit_count == 0,
            (false, true) => building_count == 0,
            (false, false) => false,
        }
    }

    /// Auto-eliminate players using caller-provided unit/building census data.
    pub fn auto_eliminate_players_with<F>(
        player_list: &mut PlayerList,
        flags: MultiplayerEliminationFlags,
        mut census: F,
    ) -> Vec<PlayerIndex>
    where
        F: FnMut(&Player) -> (usize, usize, bool),
    {
        let mut eliminated = Vec::new();

        for player in player_list.get_all_players_mut() {
            if !player.is_active() {
                continue;
            }

            let (unit_count, building_count, can_build) = census(player);
            if Self::should_eliminate_player_with_flags(
                player,
                unit_count,
                building_count,
                can_build,
                flags,
            ) {
                player.set_defeated(true);
                eliminated.push(player.index);
            }
        }

        eliminated
    }

    /// Auto-eliminate players with no units/buildings
    ///
    /// Uses runtime `crate::player::player_list()` object presence when available.
    /// If runtime player census is unavailable for a system player entry, it falls back to
    /// non-destructive defaults (no elimination) for safety.
    pub fn auto_eliminate_players(player_list: &mut PlayerList) -> Vec<PlayerIndex> {
        let mut runtime_census: HashMap<PlayerIndex, (usize, usize, bool)> = HashMap::new();

        if let Ok(runtime_players) = crate::player::player_list().read() {
            for runtime_player_arc in runtime_players.iter() {
                let Ok(runtime_player) = runtime_player_arc.read() else {
                    continue;
                };

                let index = runtime_player.get_player_index();
                if index < 0 {
                    continue;
                }

                let unit_count = if runtime_player.has_any_units() { 1 } else { 0 };
                let building_count = if runtime_player.has_any_buildings_counts_for_victory() {
                    1
                } else {
                    0
                };
                let has_any_objects = runtime_player.has_any_objects();
                runtime_census.insert(
                    index as usize,
                    (unit_count, building_count, has_any_objects),
                );
            }
        }

        Self::auto_eliminate_players_with(
            player_list,
            MultiplayerEliminationFlags::DEFAULT,
            |player| {
                runtime_census
                    .get(&player.index)
                    .copied()
                    .unwrap_or((usize::MAX, usize::MAX, true))
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::super::player_init::make_player_template;
    use super::*;

    #[test]
    fn test_player_score() {
        let mut score = PlayerScore::new(0);

        score.add_score(ScoreCategory::UnitsBuilt, 10);
        score.add_score(ScoreCategory::BuildingsBuilt, 5);
        score.add_score(ScoreCategory::UnitsDestroyed, 20);

        assert_eq!(score.get_score(ScoreCategory::UnitsBuilt), 10);
        assert_eq!(score.get_score(ScoreCategory::BuildingsBuilt), 5);
        assert_eq!(score.get_score(ScoreCategory::UnitsDestroyed), 20);
        assert_eq!(score.get_total_score(), 35);
    }

    #[test]
    fn test_score_keeper() {
        let mut keeper = ScoreKeeper::new();

        keeper.init_player(0);
        keeper.init_player(1);

        keeper.add_score(0, ScoreCategory::UnitsBuilt, 100);
        keeper.add_score(1, ScoreCategory::UnitsBuilt, 50);

        assert_eq!(keeper.get_player_score(0).unwrap().get_total_score(), 100);
        assert_eq!(keeper.get_player_score(1).unwrap().get_total_score(), 50);

        assert_eq!(keeper.get_highest_scorer(), Some(0));
    }

    #[test]
    fn test_kill_death_ratio() {
        let mut score = PlayerScore::new(0);

        score.add_score(ScoreCategory::KillValue, 100);
        score.add_score(ScoreCategory::DeathValue, 50);

        assert_eq!(score.get_kill_death_ratio(), 2.0);
    }

    #[test]
    fn test_annihilation_victory() {
        let mut conditions = VictoryConditions::new(VictoryType::Annihilation);
        let mut player_list = PlayerList::new();

        let template1 = make_player_template("Player 1", "USA");
        let template2 = make_player_template("Player 2", "China");

        let player1 = Player::new(0, template1, true);
        let player2 = Player::new(1, template2, false);

        player_list.add_player(player1);
        let player2_index = player_list.add_player(player2);

        let score_keeper = ScoreKeeper::new();

        // Initially, game not ended
        assert!(!conditions.check_conditions(&player_list, &score_keeper));

        // Eliminate player 2
        player_list
            .get_player_mut(player2_index)
            .unwrap()
            .set_defeated(true);

        // Now game should end
        assert!(conditions.check_conditions(&player_list, &score_keeper));
        assert!(conditions.is_game_ended());
        assert_eq!(conditions.get_player_result(0), GameResult::Victory);
        assert_eq!(conditions.get_player_result(1), GameResult::Defeat);
    }

    #[test]
    fn test_score_limit_victory() {
        let mut conditions = VictoryConditions::new(VictoryType::ScoreLimit);
        conditions.set_score_limit(1000);

        let mut player_list = PlayerList::new();
        let mut score_keeper = ScoreKeeper::new();

        let template = make_player_template("Player 1", "USA");
        let player = Player::new(0, template, true);
        player_list.add_player(player);

        score_keeper.init_player(0);

        // Add score below limit
        score_keeper.add_score(0, ScoreCategory::UnitsBuilt, 500);
        assert!(!conditions.check_conditions(&player_list, &score_keeper));

        // Add score to reach limit
        score_keeper.add_score(0, ScoreCategory::UnitsBuilt, 500);
        assert!(conditions.check_conditions(&player_list, &score_keeper));
        assert_eq!(conditions.get_player_result(0), GameResult::Victory);
    }

    #[test]
    fn test_time_limit_victory() {
        let mut conditions = VictoryConditions::new(VictoryType::TimeLimit);
        conditions.set_time_limit(Duration::from_secs(0)); // Immediate timeout for test

        let mut player_list = PlayerList::new();
        let mut score_keeper = ScoreKeeper::new();

        let template = make_player_template("Player 1", "USA");
        let player = Player::new(0, template, true);
        player_list.add_player(player);

        score_keeper.init_player(0);
        score_keeper.add_score(0, ScoreCategory::UnitsBuilt, 100);

        // Time limit should trigger immediately
        assert!(conditions.check_conditions(&player_list, &score_keeper));
        assert_eq!(conditions.get_player_result(0), GameResult::Victory);
    }

    #[test]
    fn test_game_duration() {
        use std::thread;

        let keeper = ScoreKeeper::new();

        thread::sleep(Duration::from_millis(100));

        let duration = keeper.get_game_duration();
        assert!(duration.as_millis() >= 100);
    }

    #[test]
    fn test_elimination_detector_default_flags_match_cxx() {
        let template = make_player_template("Player 1", "USA");
        let player = Player::new(0, template, true);

        // Default C++ MP flags are NOUNITS | NOBUILDINGS.
        assert!(EliminationDetector::should_eliminate_player(
            &player, 0, 0, true
        ));
        assert!(!EliminationDetector::should_eliminate_player(
            &player, 0, 1, false
        ));
        assert!(!EliminationDetector::should_eliminate_player(
            &player, 1, 0, false
        ));
    }

    #[test]
    fn test_elimination_detector_flag_variants() {
        let template = make_player_template("Player 1", "USA");
        let player = Player::new(0, template, true);

        assert!(EliminationDetector::should_eliminate_player_with_flags(
            &player,
            0,
            9,
            false,
            MultiplayerEliminationFlags::NO_UNITS
        ));
        assert!(!EliminationDetector::should_eliminate_player_with_flags(
            &player,
            1,
            0,
            false,
            MultiplayerEliminationFlags::NO_UNITS
        ));

        assert!(EliminationDetector::should_eliminate_player_with_flags(
            &player,
            9,
            0,
            true,
            MultiplayerEliminationFlags::NO_BUILDINGS
        ));
        assert!(!EliminationDetector::should_eliminate_player_with_flags(
            &player,
            0,
            1,
            false,
            MultiplayerEliminationFlags::NO_BUILDINGS
        ));

        assert!(!EliminationDetector::should_eliminate_player_with_flags(
            &player,
            0,
            0,
            false,
            MultiplayerEliminationFlags::NONE
        ));
    }

    #[test]
    fn test_auto_eliminate_players_with_census() {
        let mut player_list = PlayerList::new();

        let p1 = Player::new(0, make_player_template("P1", "USA"), true);
        let p2 = Player::new(1, make_player_template("P2", "China"), false);
        player_list.add_player(p1);
        player_list.add_player(p2);

        let eliminated = EliminationDetector::auto_eliminate_players_with(
            &mut player_list,
            MultiplayerEliminationFlags::DEFAULT,
            |player| {
                if player.index == 0 {
                    (0, 0, false)
                } else {
                    (2, 1, true)
                }
            },
        );

        assert_eq!(eliminated, vec![0]);
        assert!(player_list.get_player(0).unwrap().is_defeated);
        assert!(!player_list.get_player(1).unwrap().is_defeated);
    }

    #[test]
    fn test_auto_eliminate_players_legacy_entry_point_is_no_op_without_census() {
        let mut player_list = PlayerList::new();
        let p1 = Player::new(0, make_player_template("P1", "USA"), true);
        player_list.add_player(p1);

        let eliminated = EliminationDetector::auto_eliminate_players(&mut player_list);
        assert!(eliminated.is_empty());
        assert!(!player_list.get_player(0).unwrap().is_defeated);
    }
}
