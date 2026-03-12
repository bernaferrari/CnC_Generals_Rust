//! Multiplayer Rankings and ELO System
//!
//! Tracks player rankings, ELO ratings, win/loss records,
//! and leaderboard standings for online multiplayer.
//! Based on original GameSpy integration with modern extensions.

use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::post_game_stats::GameResult;

/// ELO rating constant - determines rating volatility
/// Standard chess uses 32, we use 24 for slightly slower changes
const K_FACTOR: f64 = 24.0;

/// Starting ELO rating for new players
const STARTING_ELO: i32 = 1200;

/// Minimum ELO rating (prevents negative ratings)
const MIN_ELO: i32 = 100;

/// Maximum ELO rating ceiling
const MAX_ELO: i32 = 3000;

/// Game mode for ranking purposes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GameMode {
    OneVOne,     // 1v1 matches
    TwoVTwo,     // 2v2 team matches
    ThreeVThree, // 3v3 team matches
    FourVFour,   // 4v4 team matches
    FreeForAll,  // Free-for-all (no teams)
    Custom,      // Custom game (unranked)
}

impl GameMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::OneVOne => "1v1",
            Self::TwoVTwo => "2v2",
            Self::ThreeVThree => "3v3",
            Self::FourVFour => "4v4",
            Self::FreeForAll => "FFA",
            Self::Custom => "Custom",
        }
    }

    pub fn is_ranked(&self) -> bool {
        !matches!(self, Self::Custom)
    }
}

/// Player ranking data for a specific game mode
#[derive(Debug, Clone)]
pub struct PlayerRanking {
    /// Player unique identifier
    pub player_id: String,
    /// Player name
    pub player_name: String,
    /// ELO rating
    pub elo_rating: i32,
    /// Peak ELO ever achieved
    pub peak_elo: i32,
    /// Total games played
    pub games_played: i32,
    /// Games won
    pub games_won: i32,
    /// Games lost
    pub games_lost: i32,
    /// Games drawn
    pub games_drawn: i32,
    /// Current win streak
    pub win_streak: i32,
    /// Best win streak ever
    pub best_win_streak: i32,
    /// Last game timestamp
    pub last_game_time: u64,
    /// Ladder rank (1 = highest)
    pub ladder_rank: Option<i32>,
}

impl PlayerRanking {
    pub fn new(player_id: String, player_name: String) -> Self {
        Self {
            player_id,
            player_name,
            elo_rating: STARTING_ELO,
            peak_elo: STARTING_ELO,
            games_played: 0,
            games_won: 0,
            games_lost: 0,
            games_drawn: 0,
            win_streak: 0,
            best_win_streak: 0,
            last_game_time: 0,
            ladder_rank: None,
        }
    }

    /// Get win rate percentage
    pub fn win_rate(&self) -> f64 {
        if self.games_played > 0 {
            (self.games_won as f64 / self.games_played as f64) * 100.0
        } else {
            0.0
        }
    }

    /// Check if player is active (played in last 30 days)
    pub fn is_active(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or(Duration::ZERO)
            .as_secs();
        let days_since = (now - self.last_game_time) / 86400;
        days_since < 30
    }

    /// Get player tier based on ELO
    pub fn get_tier(&self) -> PlayerTier {
        PlayerTier::from_elo(self.elo_rating)
    }
}

/// Player skill tiers based on ELO ranges
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PlayerTier {
    Bronze,
    Silver,
    Gold,
    Platinum,
    Diamond,
    Master,
    Grandmaster,
}

impl PlayerTier {
    pub fn from_elo(elo: i32) -> Self {
        match elo {
            0..=999 => Self::Bronze,
            1000..=1199 => Self::Silver,
            1200..=1499 => Self::Gold,
            1500..=1799 => Self::Platinum,
            1800..=2099 => Self::Diamond,
            2100..=2399 => Self::Master,
            _ => Self::Grandmaster,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            Self::Bronze => "Bronze",
            Self::Silver => "Silver",
            Self::Gold => "Gold",
            Self::Platinum => "Platinum",
            Self::Diamond => "Diamond",
            Self::Master => "Master",
            Self::Grandmaster => "Grandmaster",
        }
    }

    pub fn min_elo(&self) -> i32 {
        match self {
            Self::Bronze => 0,
            Self::Silver => 1000,
            Self::Gold => 1200,
            Self::Platinum => 1500,
            Self::Diamond => 1800,
            Self::Master => 2100,
            Self::Grandmaster => 2400,
        }
    }
}

/// ELO calculator for ranking updates
/// Uses standard ELO formula with K-factor adjustment
pub struct ELOCalculator {
    k_factor: f64,
}

impl ELOCalculator {
    pub fn new() -> Self {
        Self { k_factor: K_FACTOR }
    }

    pub fn with_k_factor(k_factor: f64) -> Self {
        Self { k_factor }
    }

    /// Calculate expected win probability for player A vs player B
    /// Uses standard ELO formula: E = 1 / (1 + 10^((Rb - Ra)/400))
    pub fn expected_score(&self, rating_a: i32, rating_b: i32) -> f64 {
        1.0 / (1.0 + 10.0_f64.powf((rating_b - rating_a) as f64 / 400.0))
    }

    /// Calculate new ELO rating after a game
    /// actual_score: 1.0 for win, 0.5 for draw, 0.0 for loss
    pub fn calculate_new_rating(
        &self,
        current_rating: i32,
        opponent_rating: i32,
        actual_score: f64,
    ) -> i32 {
        let expected = self.expected_score(current_rating, opponent_rating);
        let change = (self.k_factor * (actual_score - expected)) as i32;
        let new_rating = current_rating + change;

        // Clamp to valid range
        new_rating.max(MIN_ELO).min(MAX_ELO)
    }

    /// Calculate rating changes for a team game
    /// Returns (team1_new_avg, team2_new_avg)
    pub fn calculate_team_ratings(
        &self,
        team1_ratings: &[i32],
        team2_ratings: &[i32],
        team1_won: bool,
    ) -> (Vec<i32>, Vec<i32>) {
        // Calculate average team ratings
        let team1_avg = team1_ratings.iter().sum::<i32>() / team1_ratings.len() as i32;
        let team2_avg = team2_ratings.iter().sum::<i32>() / team2_ratings.len() as i32;

        // Determine actual score
        let actual_score = if team1_won { 1.0 } else { 0.0 };

        // Calculate new ratings for each player
        let team1_new: Vec<i32> = team1_ratings
            .iter()
            .map(|&rating| self.calculate_new_rating(rating, team2_avg, actual_score))
            .collect();

        let team2_new: Vec<i32> = team2_ratings
            .iter()
            .map(|&rating| self.calculate_new_rating(rating, team1_avg, 1.0 - actual_score))
            .collect();

        (team1_new, team2_new)
    }
}

impl Default for ELOCalculator {
    fn default() -> Self {
        Self::new()
    }
}

/// Multiplayer ranking system manager
pub struct RankingSystem {
    /// Rankings per game mode
    rankings: HashMap<GameMode, HashMap<String, PlayerRanking>>,
    /// ELO calculator
    elo_calculator: ELOCalculator,
}

impl RankingSystem {
    pub fn new() -> Self {
        Self {
            rankings: HashMap::new(),
            elo_calculator: ELOCalculator::new(),
        }
    }

    /// Get or create a player ranking for a game mode
    pub fn get_or_create_ranking(
        &mut self,
        mode: GameMode,
        player_id: String,
        player_name: String,
    ) -> &mut PlayerRanking {
        self.rankings
            .entry(mode)
            .or_insert_with(HashMap::new)
            .entry(player_id.clone())
            .or_insert_with(|| PlayerRanking::new(player_id, player_name))
    }

    /// Update rankings after a game
    pub fn update_after_game(
        &mut self,
        mode: GameMode,
        player_id: &str,
        opponent_id: &str,
        result: GameResult,
    ) {
        if !mode.is_ranked() {
            return; // Don't update rankings for unranked modes
        }

        // Get current ratings
        let player_rating = self
            .rankings
            .get(&mode)
            .and_then(|m| m.get(player_id))
            .map(|r| r.elo_rating)
            .unwrap_or(STARTING_ELO);

        let opponent_rating = self
            .rankings
            .get(&mode)
            .and_then(|m| m.get(opponent_id))
            .map(|r| r.elo_rating)
            .unwrap_or(STARTING_ELO);

        // Calculate score (1.0 win, 0.5 draw, 0.0 loss)
        let actual_score = match result {
            GameResult::Victory => 1.0,
            GameResult::Draw => 0.5,
            GameResult::Defeat => 0.0,
            _ => return, // Don't update for disconnect/observer
        };

        // Calculate new rating
        let new_rating =
            self.elo_calculator
                .calculate_new_rating(player_rating, opponent_rating, actual_score);

        // Update player ranking
        if let Some(mode_rankings) = self.rankings.get_mut(&mode) {
            if let Some(ranking) = mode_rankings.get_mut(player_id) {
                ranking.elo_rating = new_rating;
                ranking.peak_elo = ranking.peak_elo.max(new_rating);
                ranking.games_played += 1;
                ranking.last_game_time = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::ZERO)
                    .as_secs();

                match result {
                    GameResult::Victory => {
                        ranking.games_won += 1;
                        ranking.win_streak += 1;
                        ranking.best_win_streak = ranking.best_win_streak.max(ranking.win_streak);
                    }
                    GameResult::Defeat => {
                        ranking.games_lost += 1;
                        ranking.win_streak = 0;
                    }
                    GameResult::Draw => {
                        ranking.games_drawn += 1;
                    }
                    _ => {}
                }
            }
        }
    }

    /// Get leaderboard for a game mode (sorted by ELO)
    pub fn get_leaderboard(&self, mode: GameMode) -> Vec<PlayerRanking> {
        if let Some(mode_rankings) = self.rankings.get(&mode) {
            let mut rankings: Vec<PlayerRanking> = mode_rankings.values().cloned().collect();
            rankings.sort_by(|a, b| b.elo_rating.cmp(&a.elo_rating));
            rankings
        } else {
            Vec::new()
        }
    }

    /// Get player's rank in leaderboard
    pub fn get_player_rank(&self, mode: GameMode, player_id: &str) -> Option<i32> {
        let leaderboard = self.get_leaderboard(mode);
        leaderboard
            .iter()
            .position(|r| r.player_id == player_id)
            .map(|pos| (pos + 1) as i32)
    }

    /// Get top N players for a game mode
    pub fn get_top_players(&self, mode: GameMode, n: usize) -> Vec<PlayerRanking> {
        let mut leaderboard = self.get_leaderboard(mode);
        leaderboard.truncate(n);
        leaderboard
    }

    /// Get player statistics
    pub fn get_player_stats(&self, mode: GameMode, player_id: &str) -> Option<&PlayerRanking> {
        self.rankings.get(&mode).and_then(|m| m.get(player_id))
    }
}

impl Default for RankingSystem {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_elo_expected_score() {
        let calculator = ELOCalculator::new();

        // Equal ratings should have 50% win probability
        let expected = calculator.expected_score(1200, 1200);
        assert!((expected - 0.5).abs() < 0.01);

        // Higher rating should have >50% win probability
        let expected = calculator.expected_score(1400, 1200);
        assert!(expected > 0.5);

        // Lower rating should have <50% win probability
        let expected = calculator.expected_score(1200, 1400);
        assert!(expected < 0.5);
    }

    #[test]
    fn test_elo_rating_calculation() {
        let calculator = ELOCalculator::new();

        // Win against equal opponent
        let new_rating = calculator.calculate_new_rating(1200, 1200, 1.0);
        assert!(new_rating > 1200);
        assert!(new_rating < 1230); // Should gain ~12 points

        // Loss against equal opponent
        let new_rating = calculator.calculate_new_rating(1200, 1200, 0.0);
        assert!(new_rating < 1200);
        assert!(new_rating > 1170); // Should lose ~12 points

        // Draw against equal opponent
        let new_rating = calculator.calculate_new_rating(1200, 1200, 0.5);
        assert_eq!(new_rating, 1200); // No change on expected result
    }

    #[test]
    fn test_player_tier() {
        assert_eq!(PlayerTier::from_elo(500), PlayerTier::Bronze);
        assert_eq!(PlayerTier::from_elo(1100), PlayerTier::Silver);
        assert_eq!(PlayerTier::from_elo(1300), PlayerTier::Gold);
        assert_eq!(PlayerTier::from_elo(1600), PlayerTier::Platinum);
        assert_eq!(PlayerTier::from_elo(1900), PlayerTier::Diamond);
        assert_eq!(PlayerTier::from_elo(2200), PlayerTier::Master);
        assert_eq!(PlayerTier::from_elo(2500), PlayerTier::Grandmaster);
    }

    #[test]
    fn test_ranking_system() {
        let mut system = RankingSystem::new();

        // Create player rankings
        let _ranking1 = system.get_or_create_ranking(
            GameMode::OneVOne,
            "player1".to_string(),
            "Player One".to_string(),
        );

        let _ranking2 = system.get_or_create_ranking(
            GameMode::OneVOne,
            "player2".to_string(),
            "Player Two".to_string(),
        );

        // Simulate a game
        system.update_after_game(GameMode::OneVOne, "player1", "player2", GameResult::Victory);

        // Check updated rankings
        let player1 = system
            .get_player_stats(GameMode::OneVOne, "player1")
            .unwrap();
        assert_eq!(player1.games_played, 1);
        assert_eq!(player1.games_won, 1);
        assert!(player1.elo_rating > STARTING_ELO);

        // Check win streak
        assert_eq!(player1.win_streak, 1);
    }

    #[test]
    fn test_leaderboard() {
        let mut system = RankingSystem::new();

        // Create multiple players with different ratings
        let ranking1 = system.get_or_create_ranking(
            GameMode::OneVOne,
            "player1".to_string(),
            "Player One".to_string(),
        );
        ranking1.elo_rating = 1500;

        let ranking2 = system.get_or_create_ranking(
            GameMode::OneVOne,
            "player2".to_string(),
            "Player Two".to_string(),
        );
        ranking2.elo_rating = 1300;

        let ranking3 = system.get_or_create_ranking(
            GameMode::OneVOne,
            "player3".to_string(),
            "Player Three".to_string(),
        );
        ranking3.elo_rating = 1700;

        // Get leaderboard
        let leaderboard = system.get_leaderboard(GameMode::OneVOne);

        // Check sorted order (highest first)
        assert_eq!(leaderboard.len(), 3);
        assert_eq!(leaderboard[0].elo_rating, 1700);
        assert_eq!(leaderboard[1].elo_rating, 1500);
        assert_eq!(leaderboard[2].elo_rating, 1300);

        // Check player rank
        assert_eq!(
            system.get_player_rank(GameMode::OneVOne, "player3"),
            Some(1)
        );
        assert_eq!(
            system.get_player_rank(GameMode::OneVOne, "player1"),
            Some(2)
        );
        assert_eq!(
            system.get_player_rank(GameMode::OneVOne, "player2"),
            Some(3)
        );
    }

    #[test]
    fn test_win_rate() {
        let mut ranking = PlayerRanking::new("player1".to_string(), "Player One".to_string());

        ranking.games_played = 10;
        ranking.games_won = 7;
        ranking.games_lost = 3;

        assert_eq!(ranking.win_rate(), 70.0);
    }
}
