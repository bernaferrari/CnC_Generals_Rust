//! Achievement and Medal System
//!
//! Tracks player achievements, battle honors, and medals earned during gameplay.
//! Based on the original Generals battle honors system with extensions.

use std::collections::HashMap;
use std::time::Duration;

use super::post_game_stats::PlayerPostGameStats;

/// Achievement/Medal types
/// Matches C++ BattleHonors and extends with additional achievements
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AchievementType {
    // Combat achievements
    TopGun,      // Most units destroyed
    IronWall,    // Most units built
    Veteran,     // Most promotions earned
    Demolisher,  // Most buildings destroyed
    Constructor, // Most buildings built
    Survivor,    // Fewest units lost

    // Economic achievements
    Tycoon,    // Most money earned
    Efficient, // Best money efficiency
    Wealthy,   // Highest peak money
    Frugal,    // Least money spent

    // Special achievements
    QuickVictory, // Won in under 10 minutes
    Domination,   // Perfect victory (no losses)
    LastStand,    // Won with final building
    Comeback,     // Won after losing 80% of base

    // Skill achievements
    Multitasker,     // Highest APM
    Strategist,      // Used all general powers
    TechMaster,      // All upgrades researched
    ExpansionExpert, // Most supply centers

    // Faction-specific achievements
    SuperweaponMaster, // Fired multiple superweapons
    InfantryCommander, // Most infantry units
    ArmorCommander,    // Most vehicle units
    AirCommander,      // Most aircraft units

    // Special honors
    WarHero,          // Awarded for exceptional performance
    LegendaryGeneral, // Career achievement
    FirstBlood,       // First kill of the match
    Untouchable,      // No units lost in first 10 minutes
}

impl AchievementType {
    pub fn name(&self) -> &'static str {
        match self {
            Self::TopGun => "Top Gun",
            Self::IronWall => "Iron Wall",
            Self::Veteran => "Veteran Commander",
            Self::Demolisher => "Demolisher",
            Self::Constructor => "Master Constructor",
            Self::Survivor => "Survivor",
            Self::Tycoon => "Tycoon",
            Self::Efficient => "Efficient Commander",
            Self::Wealthy => "Wealthy",
            Self::Frugal => "Frugal",
            Self::QuickVictory => "Quick Victory",
            Self::Domination => "Domination",
            Self::LastStand => "Last Stand",
            Self::Comeback => "Comeback",
            Self::Multitasker => "Multitasker",
            Self::Strategist => "Strategist",
            Self::TechMaster => "Tech Master",
            Self::ExpansionExpert => "Expansion Expert",
            Self::SuperweaponMaster => "Superweapon Master",
            Self::InfantryCommander => "Infantry Commander",
            Self::ArmorCommander => "Armor Commander",
            Self::AirCommander => "Air Commander",
            Self::WarHero => "War Hero",
            Self::LegendaryGeneral => "Legendary General",
            Self::FirstBlood => "First Blood",
            Self::Untouchable => "Untouchable",
        }
    }

    pub fn description(&self) -> &'static str {
        match self {
            Self::TopGun => "Destroyed the most enemy units",
            Self::IronWall => "Built the most units",
            Self::Veteran => "Earned the most veterancy promotions",
            Self::Demolisher => "Destroyed the most enemy buildings",
            Self::Constructor => "Built the most buildings",
            Self::Survivor => "Lost the fewest units",
            Self::Tycoon => "Earned the most money",
            Self::Efficient => "Best money efficiency rating",
            Self::Wealthy => "Achieved highest peak wealth",
            Self::Frugal => "Spent the least money",
            Self::QuickVictory => "Won the game in under 10 minutes",
            Self::Domination => "Won without losing a single unit",
            Self::LastStand => "Won with only your final building remaining",
            Self::Comeback => "Won after losing 80% of your base",
            Self::Multitasker => "Highest actions per minute",
            Self::Strategist => "Used all available general powers",
            Self::TechMaster => "Researched all available upgrades",
            Self::ExpansionExpert => "Built the most supply centers",
            Self::SuperweaponMaster => "Fired multiple superweapons",
            Self::InfantryCommander => "Built the most infantry units",
            Self::ArmorCommander => "Built the most vehicle units",
            Self::AirCommander => "Built the most aircraft units",
            Self::WarHero => "Exceptional overall performance",
            Self::LegendaryGeneral => "Legendary career achievements",
            Self::FirstBlood => "First kill of the match",
            Self::Untouchable => "No units lost in first 10 minutes",
        }
    }

    /// Get medal tier (1-5, where 5 is highest)
    pub fn tier(&self) -> u32 {
        match self {
            Self::LegendaryGeneral | Self::WarHero | Self::Domination => 5,
            Self::QuickVictory | Self::Comeback | Self::LastStand => 4,
            Self::TopGun | Self::Demolisher | Self::Tycoon | Self::TechMaster => 3,
            Self::IronWall | Self::Constructor | Self::Efficient | Self::Strategist => 2,
            _ => 1,
        }
    }
}

/// Medal/Achievement awarded to a player
#[derive(Debug, Clone)]
pub struct Achievement {
    pub achievement_type: AchievementType,
    pub earned_timestamp: u64,
    pub game_id: Option<String>,
    pub value: Option<f64>, // Optional numeric value (score, count, etc.)
}

impl Achievement {
    pub fn new(achievement_type: AchievementType) -> Self {
        Self {
            achievement_type,
            earned_timestamp: 0,
            game_id: None,
            value: None,
        }
    }

    pub fn with_value(mut self, value: f64) -> Self {
        self.value = Some(value);
        self
    }

    pub fn with_game_id(mut self, game_id: String) -> Self {
        self.game_id = Some(game_id);
        self
    }
}

/// Achievement calculator for post-game analysis
/// Matches C++ SkirmishBattleHonors pattern
pub struct AchievementCalculator {
    /// Minimum thresholds for achievements
    thresholds: HashMap<AchievementType, f64>,
}

impl AchievementCalculator {
    pub fn new() -> Self {
        let mut thresholds = HashMap::new();

        // Set thresholds (tuned for balanced gameplay)
        thresholds.insert(AchievementType::TopGun, 50.0); // 50+ units destroyed
        thresholds.insert(AchievementType::IronWall, 100.0); // 100+ units built
        thresholds.insert(AchievementType::Veteran, 20.0); // 20+ promotions
        thresholds.insert(AchievementType::Demolisher, 10.0); // 10+ buildings destroyed
        thresholds.insert(AchievementType::Constructor, 20.0); // 20+ buildings built
        thresholds.insert(AchievementType::Tycoon, 50000.0); // $50,000+ earned
        thresholds.insert(AchievementType::Multitasker, 100.0); // 100+ APM
        thresholds.insert(AchievementType::ExpansionExpert, 5.0); // 5+ supply centers

        Self { thresholds }
    }

    /// Calculate achievements for a player based on their stats
    /// Returns list of achievements earned
    pub fn calculate_achievements(
        &self,
        player_stats: &PlayerPostGameStats,
        all_player_stats: &[PlayerPostGameStats],
        game_duration: Duration,
    ) -> Vec<Achievement> {
        let mut achievements = Vec::new();

        // Combat achievements (comparative)
        if self.is_top_in_category(player_stats, all_player_stats, |s| s.units_destroyed) {
            achievements.push(
                Achievement::new(AchievementType::TopGun)
                    .with_value(player_stats.units_destroyed as f64),
            );
        }

        if self.is_top_in_category(player_stats, all_player_stats, |s| s.units_built) {
            achievements.push(
                Achievement::new(AchievementType::IronWall)
                    .with_value(player_stats.units_built as f64),
            );
        }

        if self.is_top_in_category(player_stats, all_player_stats, |s| s.buildings_destroyed) {
            achievements.push(
                Achievement::new(AchievementType::Demolisher)
                    .with_value(player_stats.buildings_destroyed as f64),
            );
        }

        if self.is_top_in_category(player_stats, all_player_stats, |s| s.buildings_built) {
            achievements.push(
                Achievement::new(AchievementType::Constructor)
                    .with_value(player_stats.buildings_built as f64),
            );
        }

        // Best in each category achievements
        if self.is_lowest_in_category(player_stats, all_player_stats, |s| s.units_lost) {
            achievements.push(
                Achievement::new(AchievementType::Survivor)
                    .with_value(player_stats.units_lost as f64),
            );
        }

        if self.is_top_in_category(player_stats, all_player_stats, |s| s.money_earned) {
            achievements.push(
                Achievement::new(AchievementType::Tycoon)
                    .with_value(player_stats.money_earned as f64),
            );
        }

        // Threshold-based achievements
        if player_stats.promotions_earned
            >= *self
                .thresholds
                .get(&AchievementType::Veteran)
                .unwrap_or(&20.0) as i32
        {
            achievements.push(
                Achievement::new(AchievementType::Veteran)
                    .with_value(player_stats.promotions_earned as f64),
            );
        }

        if player_stats.apm
            >= *self
                .thresholds
                .get(&AchievementType::Multitasker)
                .unwrap_or(&100.0) as f32
        {
            achievements.push(
                Achievement::new(AchievementType::Multitasker).with_value(player_stats.apm as f64),
            );
        }

        if player_stats.supply_centers_built
            >= *self
                .thresholds
                .get(&AchievementType::ExpansionExpert)
                .unwrap_or(&5.0) as i32
        {
            achievements.push(
                Achievement::new(AchievementType::ExpansionExpert)
                    .with_value(player_stats.supply_centers_built as f64),
            );
        }

        // Special condition achievements
        if game_duration.as_secs() < 600 {
            // Under 10 minutes
            achievements.push(
                Achievement::new(AchievementType::QuickVictory)
                    .with_value(game_duration.as_secs() as f64),
            );
        }

        if player_stats.units_lost == 0 && player_stats.buildings_lost == 0 {
            achievements.push(Achievement::new(AchievementType::Domination));
        }

        if player_stats.superweapons_fired >= 3 {
            achievements.push(
                Achievement::new(AchievementType::SuperweaponMaster)
                    .with_value(player_stats.superweapons_fired as f64),
            );
        }

        // Efficiency achievement
        if player_stats.get_efficiency_rating() >= 90.0 {
            achievements.push(
                Achievement::new(AchievementType::Efficient)
                    .with_value(player_stats.get_efficiency_rating() as f64),
            );
        }

        // War Hero - exceptional overall performance
        if self.is_war_hero(player_stats, all_player_stats) {
            achievements.push(Achievement::new(AchievementType::WarHero));
        }

        achievements
    }

    /// Check if player is top in a specific category
    fn is_top_in_category<F>(
        &self,
        player_stats: &PlayerPostGameStats,
        all_stats: &[PlayerPostGameStats],
        stat_fn: F,
    ) -> bool
    where
        F: Fn(&PlayerPostGameStats) -> i32,
    {
        let player_value = stat_fn(player_stats);
        all_stats
            .iter()
            .all(|s| stat_fn(s) <= player_value || s.player_index == player_stats.player_index)
    }

    /// Check if player is lowest in a specific category
    fn is_lowest_in_category<F>(
        &self,
        player_stats: &PlayerPostGameStats,
        all_stats: &[PlayerPostGameStats],
        stat_fn: F,
    ) -> bool
    where
        F: Fn(&PlayerPostGameStats) -> i32,
    {
        let player_value = stat_fn(player_stats);
        all_stats
            .iter()
            .all(|s| stat_fn(s) >= player_value || s.player_index == player_stats.player_index)
    }

    /// Determine if player qualifies for War Hero achievement
    /// Must be top in at least 3 categories
    fn is_war_hero(
        &self,
        player_stats: &PlayerPostGameStats,
        all_stats: &[PlayerPostGameStats],
    ) -> bool {
        let mut top_categories = 0;

        if self.is_top_in_category(player_stats, all_stats, |s| s.units_destroyed) {
            top_categories += 1;
        }
        if self.is_top_in_category(player_stats, all_stats, |s| s.buildings_destroyed) {
            top_categories += 1;
        }
        if self.is_top_in_category(player_stats, all_stats, |s| s.money_earned) {
            top_categories += 1;
        }
        if self.is_top_in_category(player_stats, all_stats, |s| s.final_score) {
            top_categories += 1;
        }
        if player_stats.get_efficiency_rating() >= 85.0 {
            top_categories += 1;
        }

        top_categories >= 3
    }

    /// Get a formatted summary of achievements
    pub fn format_achievements(&self, achievements: &[Achievement]) -> String {
        let mut summary = String::new();
        summary.push_str("=== Achievements Earned ===\n");

        if achievements.is_empty() {
            summary.push_str("No achievements earned.\n");
            return summary;
        }

        // Group by tier
        let mut by_tier: HashMap<u32, Vec<&Achievement>> = HashMap::new();
        for achievement in achievements {
            by_tier
                .entry(achievement.achievement_type.tier())
                .or_insert_with(Vec::new)
                .push(achievement);
        }

        // Display from highest tier to lowest
        for tier in (1..=5).rev() {
            if let Some(tier_achievements) = by_tier.get(&tier) {
                summary.push_str(&format!("\nTier {} Achievements:\n", tier));
                for achievement in tier_achievements {
                    summary.push_str(&format!(
                        "  * {} - {}\n",
                        achievement.achievement_type.name(),
                        achievement.achievement_type.description()
                    ));
                    if let Some(value) = achievement.value {
                        summary.push_str(&format!("    Value: {:.0}\n", value));
                    }
                }
            }
        }

        summary
    }
}

impl Default for AchievementCalculator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_achievement_types() {
        let achievement = Achievement::new(AchievementType::TopGun);
        assert_eq!(achievement.achievement_type.name(), "Top Gun");
        assert_eq!(achievement.achievement_type.tier(), 3);
    }

    #[test]
    fn test_top_gun_achievement() {
        let calculator = AchievementCalculator::new();

        let mut player1 = PlayerPostGameStats::new(0, "Player 1".to_string());
        player1.units_destroyed = 100;

        let mut player2 = PlayerPostGameStats::new(1, "Player 2".to_string());
        player2.units_destroyed = 50;

        let all_stats = vec![player1.clone(), player2.clone()];
        let achievements =
            calculator.calculate_achievements(&player1, &all_stats, Duration::from_secs(1800));

        assert!(achievements
            .iter()
            .any(|a| a.achievement_type == AchievementType::TopGun));
    }

    #[test]
    fn test_quick_victory_achievement() {
        let calculator = AchievementCalculator::new();

        let player = PlayerPostGameStats::new(0, "Player 1".to_string());
        let all_stats = vec![player.clone()];

        let achievements = calculator.calculate_achievements(
            &player,
            &all_stats,
            Duration::from_secs(500), // Under 10 minutes
        );

        assert!(achievements
            .iter()
            .any(|a| a.achievement_type == AchievementType::QuickVictory));
    }

    #[test]
    fn test_domination_achievement() {
        let calculator = AchievementCalculator::new();

        let mut player = PlayerPostGameStats::new(0, "Player 1".to_string());
        player.units_lost = 0;
        player.buildings_lost = 0;

        let all_stats = vec![player.clone()];
        let achievements =
            calculator.calculate_achievements(&player, &all_stats, Duration::from_secs(1800));

        assert!(achievements
            .iter()
            .any(|a| a.achievement_type == AchievementType::Domination));
    }

    #[test]
    fn test_war_hero_achievement() {
        let calculator = AchievementCalculator::new();

        let mut player1 = PlayerPostGameStats::new(0, "Player 1".to_string());
        player1.units_destroyed = 100;
        player1.buildings_destroyed = 20;
        player1.money_earned = 100000;
        player1.final_score = 10000;
        player1.units_lost = 10;
        player1.money_spent = 80000;
        player1.calculate_derived_stats(Duration::from_secs(1800));

        let player2 = PlayerPostGameStats::new(1, "Player 2".to_string());

        let all_stats = vec![player1.clone(), player2];
        let achievements =
            calculator.calculate_achievements(&player1, &all_stats, Duration::from_secs(1800));

        assert!(achievements
            .iter()
            .any(|a| a.achievement_type == AchievementType::WarHero));
    }

    #[test]
    fn test_achievement_formatting() {
        let calculator = AchievementCalculator::new();

        let achievements = vec![
            Achievement::new(AchievementType::TopGun).with_value(100.0),
            Achievement::new(AchievementType::QuickVictory).with_value(480.0),
        ];

        let formatted = calculator.format_achievements(&achievements);
        assert!(formatted.contains("Top Gun"));
        assert!(formatted.contains("Quick Victory"));
    }
}
