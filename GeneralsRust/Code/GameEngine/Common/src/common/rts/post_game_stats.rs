//! Post-Game Statistics System
//!
//! Comprehensive statistics screen data for end-of-game analysis.
//! Aggregates data from MissionStats, ScoreKeeper, and AcademyStats
//! for display in post-game screens and replays.

use std::collections::HashMap;
use std::time::Duration;

/// Maximum number of players in a game
pub const MAX_PLAYERS: usize = 8;

/// Game result for a player
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GameResult {
    Victory,
    Defeat,
    Draw,
    Disconnected,
    Observer,
}

/// Player faction/side
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlayerSide {
    USA,
    China,
    GLA,
    Custom(String),
}

/// Post-game statistics for a single player
#[derive(Debug, Clone)]
pub struct PlayerPostGameStats {
    /// Player index (0-7)
    pub player_index: usize,
    /// Player name
    pub player_name: String,
    /// Player side/faction
    pub side: PlayerSide,
    /// Game result
    pub result: GameResult,
    /// Final score
    pub final_score: i32,

    // Economic stats
    /// Total money earned
    pub money_earned: i32,
    /// Total money spent
    pub money_spent: i32,
    /// Money remaining at end
    pub money_remaining: i32,
    /// Peak money held
    pub peak_money: i32,

    // Unit stats
    /// Units built
    pub units_built: i32,
    /// Units lost
    pub units_lost: i32,
    /// Enemy units destroyed
    pub units_destroyed: i32,
    /// Unit kill/death ratio
    pub unit_kd_ratio: f32,

    // Building stats
    /// Buildings built
    pub buildings_built: i32,
    /// Buildings lost
    pub buildings_lost: i32,
    /// Enemy buildings destroyed
    pub buildings_destroyed: i32,
    /// Building kill/death ratio
    pub building_kd_ratio: f32,

    // Capture stats
    /// Tech buildings captured
    pub tech_buildings_captured: i32,
    /// Faction buildings captured
    pub faction_buildings_captured: i32,

    // Combat stats
    /// Total damage dealt
    pub damage_dealt: i64,
    /// Total damage taken
    pub damage_taken: i64,
    /// Veterancy promotions earned
    pub promotions_earned: i32,
    /// Heroic units created
    pub heroic_units: i32,

    // Resource stats
    /// Supply centers built
    pub supply_centers_built: i32,
    /// Supply income rate (per minute)
    pub supply_rate: f32,
    /// Secondary income methods used
    pub secondary_income_count: i32,

    // Special abilities
    /// General powers used
    pub general_powers_used: i32,
    /// General points spent
    pub general_points_spent: i32,
    /// Superweapons fired
    pub superweapons_fired: i32,

    // Efficiency metrics
    /// Actions per minute
    pub apm: f32,
    /// Average idle time for production buildings (seconds)
    pub avg_idle_time: f32,
    /// Time without power (seconds)
    pub time_without_power: f32,

    // Detailed unit breakdown (unit type name -> count)
    pub units_built_by_type: HashMap<String, i32>,
    pub units_lost_by_type: HashMap<String, i32>,
    pub units_destroyed_by_type: HashMap<String, i32>,
}

impl PlayerPostGameStats {
    pub fn new(player_index: usize, player_name: String) -> Self {
        Self {
            player_index,
            player_name,
            side: PlayerSide::USA,
            result: GameResult::Defeat,
            final_score: 0,
            money_earned: 0,
            money_spent: 0,
            money_remaining: 0,
            peak_money: 0,
            units_built: 0,
            units_lost: 0,
            units_destroyed: 0,
            unit_kd_ratio: 0.0,
            buildings_built: 0,
            buildings_lost: 0,
            buildings_destroyed: 0,
            building_kd_ratio: 0.0,
            tech_buildings_captured: 0,
            faction_buildings_captured: 0,
            damage_dealt: 0,
            damage_taken: 0,
            promotions_earned: 0,
            heroic_units: 0,
            supply_centers_built: 0,
            supply_rate: 0.0,
            secondary_income_count: 0,
            general_powers_used: 0,
            general_points_spent: 0,
            superweapons_fired: 0,
            apm: 0.0,
            avg_idle_time: 0.0,
            time_without_power: 0.0,
            units_built_by_type: HashMap::new(),
            units_lost_by_type: HashMap::new(),
            units_destroyed_by_type: HashMap::new(),
        }
    }

    /// Calculate derived statistics (ratios, rates, etc.)
    pub fn calculate_derived_stats(&mut self, game_duration: Duration) {
        // Calculate K/D ratios
        self.unit_kd_ratio = if self.units_lost > 0 {
            self.units_destroyed as f32 / self.units_lost as f32
        } else if self.units_destroyed > 0 {
            f32::INFINITY
        } else {
            0.0
        };

        self.building_kd_ratio = if self.buildings_lost > 0 {
            self.buildings_destroyed as f32 / self.buildings_lost as f32
        } else if self.buildings_destroyed > 0 {
            f32::INFINITY
        } else {
            0.0
        };

        // Calculate supply rate (per minute)
        let minutes = game_duration.as_secs_f32() / 60.0;
        if minutes > 0.0 {
            self.supply_rate = self.money_earned as f32 / minutes;
        }
    }

    /// Get overall efficiency rating (0-100)
    pub fn get_efficiency_rating(&self) -> f32 {
        let mut rating = 0.0;
        let mut factors = 0;

        // K/D ratio contribution (max 30 points)
        if self.unit_kd_ratio.is_finite() {
            rating += (self.unit_kd_ratio.min(3.0) / 3.0) * 30.0;
            factors += 1;
        }

        // Economic efficiency (max 25 points)
        if self.money_spent > 0 {
            let efficiency = self.money_earned as f32 / self.money_spent as f32;
            rating += efficiency.min(2.0) / 2.0 * 25.0;
            factors += 1;
        }

        // Production efficiency (max 25 points)
        if self.avg_idle_time.is_finite() {
            // Lower idle time is better (assume max 600 seconds idle is worst)
            let idle_factor = 1.0 - (self.avg_idle_time.min(600.0) / 600.0);
            rating += idle_factor * 25.0;
            factors += 1;
        }

        // Resource utilization (max 20 points)
        if self.money_earned > 0 {
            let utilization =
                (self.money_earned - self.money_remaining) as f32 / self.money_earned as f32;
            rating += utilization * 20.0;
            factors += 1;
        }

        if factors > 0 {
            rating.min(100.0)
        } else {
            0.0
        }
    }
}

/// Complete post-game statistics for all players
#[derive(Debug, Clone)]
pub struct PostGameStatistics {
    /// Game duration
    pub game_duration: Duration,
    /// Game mode (Skirmish, Multiplayer, Campaign, etc.)
    pub game_mode: String,
    /// Map name
    pub map_name: String,
    /// Timestamp when game started
    pub game_start_time: u64,
    /// Timestamp when game ended
    pub game_end_time: u64,
    /// Individual player statistics
    pub player_stats: Vec<PlayerPostGameStats>,
    /// Team information (player indices grouped by team)
    pub teams: Vec<Vec<usize>>,
    /// Winner team index (if applicable)
    pub winner_team: Option<usize>,
}

impl PostGameStatistics {
    pub fn new(game_mode: String, map_name: String) -> Self {
        Self {
            game_duration: Duration::ZERO,
            game_mode,
            map_name,
            game_start_time: 0,
            game_end_time: 0,
            player_stats: Vec::new(),
            teams: Vec::new(),
            winner_team: None,
        }
    }

    /// Add a player's statistics
    pub fn add_player(&mut self, stats: PlayerPostGameStats) {
        self.player_stats.push(stats);
    }

    /// Get statistics for a specific player
    pub fn get_player_stats(&self, player_index: usize) -> Option<&PlayerPostGameStats> {
        self.player_stats
            .iter()
            .find(|s| s.player_index == player_index)
    }

    /// Get all victorious players
    pub fn get_victors(&self) -> Vec<&PlayerPostGameStats> {
        self.player_stats
            .iter()
            .filter(|s| s.result == GameResult::Victory)
            .collect()
    }

    /// Get the player with the highest score
    pub fn get_highest_score_player(&self) -> Option<&PlayerPostGameStats> {
        self.player_stats.iter().max_by_key(|s| s.final_score)
    }

    /// Get the player with the most kills
    pub fn get_most_kills_player(&self) -> Option<&PlayerPostGameStats> {
        self.player_stats
            .iter()
            .max_by_key(|s| s.units_destroyed + s.buildings_destroyed)
    }

    /// Get team statistics (aggregated)
    pub fn get_team_stats(&self, team_index: usize) -> Option<PlayerPostGameStats> {
        if team_index >= self.teams.len() {
            return None;
        }

        let team_members = &self.teams[team_index];
        if team_members.is_empty() {
            return None;
        }

        // Create aggregate stats
        let mut team_stats =
            PlayerPostGameStats::new(team_index, format!("Team {}", team_index + 1));

        for &player_idx in team_members {
            if let Some(player) = self.get_player_stats(player_idx) {
                // Aggregate all numeric stats
                team_stats.final_score += player.final_score;
                team_stats.money_earned += player.money_earned;
                team_stats.money_spent += player.money_spent;
                team_stats.units_built += player.units_built;
                team_stats.units_lost += player.units_lost;
                team_stats.units_destroyed += player.units_destroyed;
                team_stats.buildings_built += player.buildings_built;
                team_stats.buildings_lost += player.buildings_lost;
                team_stats.buildings_destroyed += player.buildings_destroyed;
                team_stats.damage_dealt += player.damage_dealt;
                team_stats.damage_taken += player.damage_taken;
            }
        }

        // Recalculate derived stats
        team_stats.calculate_derived_stats(self.game_duration);

        Some(team_stats)
    }

    /// Sort players by score (descending)
    pub fn get_leaderboard(&self) -> Vec<&PlayerPostGameStats> {
        let mut sorted: Vec<&PlayerPostGameStats> = self.player_stats.iter().collect();
        sorted.sort_by(|a, b| b.final_score.cmp(&a.final_score));
        sorted
    }

    /// Generate a summary report string
    pub fn generate_summary(&self) -> String {
        let mut report = String::new();
        report.push_str(&format!("=== Game Statistics Summary ===\n"));
        report.push_str(&format!("Map: {}\n", self.map_name));
        report.push_str(&format!("Mode: {}\n", self.game_mode));
        report.push_str(&format!(
            "Duration: {}:{:02}\n\n",
            self.game_duration.as_secs() / 60,
            self.game_duration.as_secs() % 60
        ));

        report.push_str("Player Rankings:\n");
        for (rank, player) in self.get_leaderboard().iter().enumerate() {
            report.push_str(&format!(
                "{}. {} - Score: {} ({:?})\n",
                rank + 1,
                player.player_name,
                player.final_score,
                player.result
            ));
            report.push_str(&format!(
                "   Units: {}/{}/{} (B/L/K) | Buildings: {}/{}/{}\n",
                player.units_built,
                player.units_lost,
                player.units_destroyed,
                player.buildings_built,
                player.buildings_lost,
                player.buildings_destroyed
            ));
        }

        report
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_stats_creation() {
        let stats = PlayerPostGameStats::new(0, "Player 1".to_string());
        assert_eq!(stats.player_index, 0);
        assert_eq!(stats.player_name, "Player 1");
        assert_eq!(stats.final_score, 0);
    }

    #[test]
    fn test_kd_ratio_calculation() {
        let mut stats = PlayerPostGameStats::new(0, "Player 1".to_string());
        stats.units_destroyed = 100;
        stats.units_lost = 50;

        stats.calculate_derived_stats(Duration::from_secs(600));

        assert_eq!(stats.unit_kd_ratio, 2.0);
    }

    #[test]
    fn test_efficiency_rating() {
        let mut stats = PlayerPostGameStats::new(0, "Player 1".to_string());
        stats.units_destroyed = 100;
        stats.units_lost = 50;
        stats.money_earned = 10000;
        stats.money_spent = 8000;
        stats.money_remaining = 2000;
        stats.avg_idle_time = 30.0;

        stats.calculate_derived_stats(Duration::from_secs(600));

        let rating = stats.get_efficiency_rating();
        assert!(rating > 0.0 && rating <= 100.0);
    }

    #[test]
    fn test_post_game_stats() {
        let mut game_stats =
            PostGameStatistics::new("Skirmish".to_string(), "Tournament Desert".to_string());
        game_stats.game_duration = Duration::from_secs(1800);

        let mut player1 = PlayerPostGameStats::new(0, "Player 1".to_string());
        player1.final_score = 5000;
        player1.result = GameResult::Victory;

        let mut player2 = PlayerPostGameStats::new(1, "Player 2".to_string());
        player2.final_score = 3000;
        player2.result = GameResult::Defeat;

        game_stats.add_player(player1);
        game_stats.add_player(player2);

        assert_eq!(game_stats.player_stats.len(), 2);
        assert_eq!(
            game_stats.get_highest_score_player().unwrap().player_index,
            0
        );
        assert_eq!(game_stats.get_victors().len(), 1);
    }

    #[test]
    fn test_team_stats_aggregation() {
        let mut game_stats =
            PostGameStatistics::new("Team Match".to_string(), "Tournament Island".to_string());

        let mut player1 = PlayerPostGameStats::new(0, "Player 1".to_string());
        player1.final_score = 3000;
        player1.units_destroyed = 50;

        let mut player2 = PlayerPostGameStats::new(1, "Player 2".to_string());
        player2.final_score = 2000;
        player2.units_destroyed = 30;

        game_stats.add_player(player1);
        game_stats.add_player(player2);
        game_stats.teams = vec![vec![0, 1]];

        let team_stats = game_stats.get_team_stats(0).unwrap();
        assert_eq!(team_stats.final_score, 5000);
        assert_eq!(team_stats.units_destroyed, 80);
    }
}
