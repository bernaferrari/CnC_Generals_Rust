//! Player Science and Rank System for Special Powers
//!
//! Manages player rank progression and science unlocks that gate special power availability.
//! Matches C++ Player class science system and rank progression.

use crate::common::*;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock, RwLock};

/// Player rank levels (matches C++ general rank system)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlayerRank {
    Recruit = 0,
    Veteran = 1,
    Elite = 2,
    Heroic = 3,
}

impl PlayerRank {
    /// Get rank from experience points
    pub fn from_experience(exp: Int) -> Self {
        if exp >= 15000 {
            PlayerRank::Heroic
        } else if exp >= 5000 {
            PlayerRank::Elite
        } else if exp >= 1000 {
            PlayerRank::Veteran
        } else {
            PlayerRank::Recruit
        }
    }

    /// Get experience required for this rank
    pub fn experience_required(&self) -> Int {
        match self {
            PlayerRank::Recruit => 0,
            PlayerRank::Veteran => 1000,
            PlayerRank::Elite => 5000,
            PlayerRank::Heroic => 15000,
        }
    }

    /// Get display name
    pub fn name(&self) -> &'static str {
        match self {
            PlayerRank::Recruit => "Recruit",
            PlayerRank::Veteran => "Veteran",
            PlayerRank::Elite => "Elite",
            PlayerRank::Heroic => "Heroic",
        }
    }
}

/// Player science/technology system
/// Tracks researched technologies and purchased upgrades
#[derive(Debug, Clone)]
pub struct PlayerScience {
    /// Player ID
    pub player_id: ObjectID,
    /// Current player rank
    pub rank: PlayerRank,
    /// Current experience points
    pub experience: Int,
    /// Unlocked sciences/technologies (by name)
    pub sciences: HashMap<AsciiString, bool>,
}

impl PlayerScience {
    pub fn new(player_id: ObjectID) -> Self {
        Self {
            player_id,
            rank: PlayerRank::Recruit,
            experience: 0,
            sciences: HashMap::new(),
        }
    }

    /// Check if player has specific science
    /// Matches C++ Player::hasScience(const ScienceType *)
    pub fn has_science(&self, science_name: &str) -> bool {
        self.sciences
            .get(&AsciiString::from(science_name))
            .copied()
            .unwrap_or(false)
    }

    /// Check if player has all required sciences
    pub fn has_all_sciences(&self, required: &[AsciiString]) -> bool {
        required
            .iter()
            .all(|sci| self.has_science(&sci.to_string()))
    }

    /// Add science to player
    /// Matches C++ Player::addScience(const ScienceType *)
    pub fn add_science(&mut self, science_name: AsciiString) {
        log::info!(
            "Player {} unlocked science: {}",
            self.player_id,
            science_name
        );
        self.sciences.insert(science_name.clone(), true);
    }

    /// Remove science from player (for testing or special conditions)
    pub fn remove_science(&mut self, science_name: &str) {
        self.sciences.remove(&AsciiString::from(science_name));
    }

    /// Add experience and update rank
    /// Matches C++ Player::addExperience(int amount)
    pub fn add_experience(&mut self, amount: Int) {
        let old_rank = self.rank;
        self.experience += amount;
        self.rank = PlayerRank::from_experience(self.experience);

        if self.rank != old_rank {
            log::info!(
                "Player {} rank up: {} -> {} (exp: {})",
                self.player_id,
                old_rank.name(),
                self.rank.name(),
                self.experience
            );
        }
    }

    /// Get current rank
    pub fn get_rank(&self) -> PlayerRank {
        self.rank
    }

    /// Check if rank requirement is met
    pub fn has_rank(&self, required_rank: PlayerRank) -> bool {
        self.rank >= required_rank
    }

    /// Get experience points
    pub fn get_experience(&self) -> Int {
        self.experience
    }

    /// Get progress to next rank (0.0 to 1.0)
    pub fn get_rank_progress(&self) -> Real {
        let current_req = self.rank.experience_required();
        let next_rank_exp = match self.rank {
            PlayerRank::Recruit => PlayerRank::Veteran.experience_required(),
            PlayerRank::Veteran => PlayerRank::Elite.experience_required(),
            PlayerRank::Elite => PlayerRank::Heroic.experience_required(),
            PlayerRank::Heroic => return 1.0, // Max rank
        };

        let range = next_rank_exp - current_req;
        let progress = self.experience - current_req;
        (progress as Real / range as Real).clamp(0.0, 1.0)
    }

    /// Clear all sciences (for new game or reset)
    pub fn clear_sciences(&mut self) {
        self.sciences.clear();
    }

    /// Reset player to starting state
    pub fn reset(&mut self) {
        self.rank = PlayerRank::Recruit;
        self.experience = 0;
        self.sciences.clear();
    }
}

/// Science requirement definition for special powers
#[derive(Debug, Clone)]
pub struct ScienceRequirement {
    /// Science name
    pub science_name: AsciiString,
    /// Whether this is optional (OR requirement)
    pub optional: bool,
}

impl ScienceRequirement {
    pub fn required(name: impl Into<AsciiString>) -> Self {
        Self {
            science_name: name.into(),
            optional: false,
        }
    }

    pub fn optional(name: impl Into<AsciiString>) -> Self {
        Self {
            science_name: name.into(),
            optional: true,
        }
    }
}

/// Science requirement checker
pub struct ScienceChecker;

impl ScienceChecker {
    /// Check if player meets science requirements
    /// Returns (meets_requirements, missing_sciences)
    pub fn check_requirements(
        player_science: &PlayerScience,
        requirements: &[ScienceRequirement],
    ) -> (bool, Vec<AsciiString>) {
        if requirements.is_empty() {
            return (true, Vec::new());
        }

        let mut required_missing = Vec::new();
        let mut has_optional = false;

        for req in requirements {
            let has_science = player_science.has_science(&req.science_name.to_string());

            if req.optional {
                if has_science {
                    has_optional = true;
                }
            } else {
                if !has_science {
                    required_missing.push(req.science_name.clone());
                }
            }
        }

        // Check if requirements are met
        let has_all_required = required_missing.is_empty();
        let has_optional_req = requirements.iter().any(|r| r.optional);

        let meets_requirements = has_all_required && (!has_optional_req || has_optional);

        (meets_requirements, required_missing)
    }
}

/// Global player science manager
static PLAYER_SCIENCE_MANAGER: OnceLock<Arc<RwLock<PlayerScienceManager>>> = OnceLock::new();

/// Manager for all player science systems
#[derive(Debug)]
pub struct PlayerScienceManager {
    players: HashMap<ObjectID, PlayerScience>,
}

impl PlayerScienceManager {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
        }
    }

    /// Register a player
    pub fn register_player(&mut self, player_id: ObjectID) {
        self.players
            .insert(player_id, PlayerScience::new(player_id));
    }

    /// Get player science
    pub fn get_player(&self, player_id: ObjectID) -> Option<&PlayerScience> {
        self.players.get(&player_id)
    }

    /// Get mutable player science
    pub fn get_player_mut(&mut self, player_id: ObjectID) -> Option<&mut PlayerScience> {
        self.players.get_mut(&player_id)
    }

    /// Add science to player
    pub fn add_science(&mut self, player_id: ObjectID, science_name: AsciiString) {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.add_science(science_name);
        }
    }

    /// Add experience to player
    pub fn add_experience(&mut self, player_id: ObjectID, amount: Int) {
        if let Some(player) = self.players.get_mut(&player_id) {
            player.add_experience(amount);
        }
    }
}

impl Default for PlayerScienceManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Initialize global player science manager
pub fn initialize_player_science() {
    let _ =
        PLAYER_SCIENCE_MANAGER.get_or_init(|| Arc::new(RwLock::new(PlayerScienceManager::new())));
}

/// Get global player science manager
pub fn get_player_science_manager() -> Option<Arc<RwLock<PlayerScienceManager>>> {
    PLAYER_SCIENCE_MANAGER.get().cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_player_rank_progression() {
        assert_eq!(PlayerRank::from_experience(0), PlayerRank::Recruit);
        assert_eq!(PlayerRank::from_experience(500), PlayerRank::Recruit);
        assert_eq!(PlayerRank::from_experience(1000), PlayerRank::Veteran);
        assert_eq!(PlayerRank::from_experience(5000), PlayerRank::Elite);
        assert_eq!(PlayerRank::from_experience(15000), PlayerRank::Heroic);
    }

    #[test]
    fn test_player_science_basic() {
        let mut player = PlayerScience::new(1);

        assert!(!player.has_science("SCIENCE_A10"));

        player.add_science("SCIENCE_A10".into());
        assert!(player.has_science("SCIENCE_A10"));
    }

    #[test]
    fn test_player_science_multiple() {
        let mut player = PlayerScience::new(1);

        player.add_science("SCIENCE_A10".into());
        player.add_science("SCIENCE_NUKE".into());

        assert!(player.has_all_sciences(&vec!["SCIENCE_A10".into(), "SCIENCE_NUKE".into()]));

        assert!(!player.has_all_sciences(&vec!["SCIENCE_A10".into(), "SCIENCE_MISSING".into()]));
    }

    #[test]
    fn test_experience_and_rank() {
        let mut player = PlayerScience::new(1);
        assert_eq!(player.get_rank(), PlayerRank::Recruit);

        player.add_experience(1000);
        assert_eq!(player.get_rank(), PlayerRank::Veteran);

        player.add_experience(4000);
        assert_eq!(player.get_rank(), PlayerRank::Elite);

        player.add_experience(10000);
        assert_eq!(player.get_rank(), PlayerRank::Heroic);
    }

    #[test]
    fn test_rank_progress() {
        let mut player = PlayerScience::new(1);

        assert_eq!(player.get_rank_progress(), 0.0);

        player.add_experience(500); // Halfway to Veteran
        let progress = player.get_rank_progress();
        assert!((progress - 0.5).abs() < 0.01);

        player.add_experience(500); // Now Veteran
        assert_eq!(player.get_rank(), PlayerRank::Veteran);
    }

    #[test]
    fn test_science_requirements() {
        let mut player = PlayerScience::new(1);
        player.add_science("SCIENCE_A".into());

        let requirements = vec![
            ScienceRequirement::required("SCIENCE_A"),
            ScienceRequirement::required("SCIENCE_B"),
        ];

        let (meets, missing) = ScienceChecker::check_requirements(&player, &requirements);
        assert!(!meets);
        assert_eq!(missing.len(), 1);
        assert_eq!(missing[0], "SCIENCE_B");

        player.add_science("SCIENCE_B".into());
        let (meets, missing) = ScienceChecker::check_requirements(&player, &requirements);
        assert!(meets);
        assert!(missing.is_empty());
    }

    #[test]
    fn test_player_science_manager() {
        let mut manager = PlayerScienceManager::new();

        manager.register_player(1);
        manager.register_player(2);

        manager.add_science(1, "SCIENCE_A".into());
        manager.add_experience(2, 5000);

        assert!(manager.get_player(1).unwrap().has_science("SCIENCE_A"));
        assert!(!manager.get_player(2).unwrap().has_science("SCIENCE_A"));
        assert_eq!(manager.get_player(2).unwrap().get_rank(), PlayerRank::Elite);
    }
}
