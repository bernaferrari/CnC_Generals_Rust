//! Science UI and Notification System
//!
//! Provides UI data structures and notification helpers for the science/rank system.
//! This includes:
//! - Level-up notification data
//! - Science purchase UI data
//! - Available science calculations for UI display
//! - Rank progress information

use crate::common::*;
use crate::player::{Player, ScienceVec};
use game_engine::common::rts::{ScienceType, SCIENCE_INVALID};

/// Notification event for rank level-up
#[derive(Debug, Clone)]
pub struct LevelUpNotification {
    /// Player index that leveled up
    pub player_index: Int,
    /// Old rank level
    pub old_level: Int,
    /// New rank level
    pub new_level: Int,
    /// Science purchase points granted by this level-up
    pub science_points_granted: Int,
    /// Sciences granted by this level-up
    pub sciences_granted: ScienceVec,
    /// New rank name
    pub rank_name: String,
}

impl LevelUpNotification {
    /// Create a level-up notification from rank info
    pub fn from_rank_level_change(
        player_index: Int,
        old_level: Int,
        new_level: Int,
    ) -> Option<Self> {
        use crate::system::rank_info::the_rank_info_store;

        if let Some(rank_store) = the_rank_info_store() {
            if let Some(rank_info) = rank_store.get_rank_info(new_level as usize) {
                return Some(Self {
                    player_index,
                    old_level,
                    new_level,
                    science_points_granted: rank_info.science_purchase_points_granted,
                    sciences_granted: rank_info.sciences_granted.clone(),
                    rank_name: rank_info.rank_name.to_string(),
                });
            }
        }

        None
    }

    /// Check if this level-up grants any sciences
    pub fn grants_sciences(&self) -> bool {
        !self.sciences_granted.is_empty()
    }

    /// Check if this level-up grants science purchase points
    pub fn grants_points(&self) -> bool {
        self.science_points_granted > 0
    }
}

/// Information about a purchasable science for UI display
#[derive(Debug, Clone)]
pub struct PurchasableScienceInfo {
    /// Science identifier
    pub science: ScienceType,
    /// Display name
    pub display_name: String,
    /// Description text
    pub description: String,
    /// Cost in science purchase points
    pub cost: Int,
    /// Whether player can currently afford it
    pub can_afford: bool,
    /// Whether player has all prerequisites
    pub has_prereqs: bool,
    /// Missing prerequisite sciences
    pub missing_prereqs: ScienceVec,
}

impl PurchasableScienceInfo {
    /// Create from science type and player state
    pub fn from_science(player: &Player, science: ScienceType) -> Option<Self> {
        use game_engine::common::rts::science::get_science_store;

        let science_store = get_science_store()?;
        let science_info = science_store.find_science_info(science)?;

        let cost = science_store.get_science_purchase_cost(science);
        let can_afford = cost <= player.get_science_purchase_points();
        let has_prereqs = science_store.player_has_prereqs_for_science(player, science);

        // Calculate missing prerequisites
        let mut missing_prereqs = Vec::new();
        for &prereq in &science_info.prereq_sciences {
            if !player.has_science(prereq) {
                missing_prereqs.push(prereq);
            }
        }

        Some(Self {
            science,
            display_name: science_info.display_name.clone(),
            description: science_info.description.clone(),
            cost,
            can_afford,
            has_prereqs,
            missing_prereqs,
        })
    }

    /// Check if this science is currently purchasable
    pub fn is_purchasable(&self) -> bool {
        self.can_afford && self.has_prereqs
    }
}

/// Rank progress information for UI display
#[derive(Debug, Clone)]
pub struct RankProgressInfo {
    /// Current rank level
    pub current_level: Int,
    /// Current rank name
    pub current_rank_name: String,
    /// Current skill points
    pub current_skill_points: Int,
    /// Skill points needed for current level
    pub current_level_threshold: Int,
    /// Skill points needed for next level (None if at max)
    pub next_level_threshold: Option<Int>,
    /// Next rank name (None if at max)
    pub next_rank_name: Option<String>,
    /// Progress to next level (0.0 to 1.0, or 1.0 if at max)
    pub progress_percentage: Real,
}

impl RankProgressInfo {
    /// Calculate from player state
    pub fn from_player(player: &Player) -> Option<Self> {
        use crate::system::rank_info::the_rank_info_store;

        let rank_store = the_rank_info_store()?;
        let current_level = player.get_rank_level();
        let current_skill_points = player.get_skill_points();

        // Get current rank info
        let current_rank = rank_store.get_rank_info(current_level as usize)?;
        let current_level_threshold = current_rank.skill_points_needed;

        // Try to get next rank info
        let (next_level_threshold, next_rank_name, progress_percentage) =
            if let Some(next_rank) = rank_store.get_rank_info((current_level + 1) as usize) {
                let next_threshold = next_rank.skill_points_needed;
                let points_in_level = current_skill_points - current_level_threshold;
                let points_needed = next_threshold - current_level_threshold;

                let progress = if points_needed > 0 {
                    (points_in_level as Real / points_needed as Real).clamp(0.0, 1.0)
                } else {
                    1.0
                };

                (
                    Some(next_threshold),
                    Some(next_rank.rank_name.to_string()),
                    progress,
                )
            } else {
                // At max rank
                (None, None, 1.0)
            };

        Some(Self {
            current_level,
            current_rank_name: current_rank.rank_name.to_string(),
            current_skill_points,
            current_level_threshold,
            next_level_threshold,
            next_rank_name,
            progress_percentage,
        })
    }

    /// Check if player is at max rank
    pub fn is_at_max_rank(&self) -> bool {
        self.next_level_threshold.is_none()
    }

    /// Get skill points needed to reach next level (None if at max)
    pub fn skill_points_to_next_level(&self) -> Option<Int> {
        self.next_level_threshold
            .map(|threshold| threshold - self.current_skill_points)
    }
}

/// Science tree UI data structure
///
/// This provides all information needed to display the science tree UI,
/// including what sciences are available, what the player has, and what
/// can be purchased.
#[derive(Debug, Clone)]
pub struct ScienceTreeUIData {
    /// Sciences the player currently has
    pub owned_sciences: ScienceVec,
    /// Sciences the player can purchase right now
    pub purchasable_sciences: Vec<PurchasableScienceInfo>,
    /// Sciences the player could purchase later (lacks prereqs or points)
    pub future_sciences: Vec<PurchasableScienceInfo>,
    /// Current science purchase points
    pub science_points: Int,
    /// Rank progress info
    pub rank_progress: RankProgressInfo,
}

impl ScienceTreeUIData {
    /// Build UI data from player state
    pub fn from_player(player: &Player) -> Option<Self> {
        let owned_sciences = player.get_sciences().clone();
        let science_points = player.get_science_purchase_points();
        let rank_progress = RankProgressInfo::from_player(player)?;

        // Get purchasable and future sciences
        let (purchasable_now, potentially_purchasable) = player.get_purchasable_sciences();

        let purchasable_sciences: Vec<PurchasableScienceInfo> = purchasable_now
            .into_iter()
            .filter_map(|science| PurchasableScienceInfo::from_science(player, science))
            .collect();

        let future_sciences: Vec<PurchasableScienceInfo> = potentially_purchasable
            .into_iter()
            .filter_map(|science| PurchasableScienceInfo::from_science(player, science))
            .collect();

        Some(Self {
            owned_sciences,
            purchasable_sciences,
            future_sciences,
            science_points,
            rank_progress,
        })
    }

    /// Get count of purchasable sciences
    pub fn purchasable_count(&self) -> usize {
        self.purchasable_sciences.len()
    }

    /// Check if any sciences are available to purchase
    pub fn has_purchasable_sciences(&self) -> bool {
        !self.purchasable_sciences.is_empty()
    }

    /// Get sciences that can be afforded right now
    pub fn affordable_sciences(&self) -> Vec<&PurchasableScienceInfo> {
        self.purchasable_sciences
            .iter()
            .filter(|info| info.can_afford)
            .collect()
    }
}

impl Player {
    /// Get level-up notification for display
    ///
    /// This would be called by the UI system when a level-up occurs
    /// to display appropriate feedback to the player.
    pub fn get_level_up_notification(&self, old_level: Int) -> Option<LevelUpNotification> {
        LevelUpNotification::from_rank_level_change(
            self.get_player_index(),
            old_level,
            self.get_rank_level(),
        )
    }

    /// Get science tree UI data for this player
    ///
    /// This provides all information needed to render the science tree UI.
    pub fn get_science_tree_ui_data(&self) -> Option<ScienceTreeUIData> {
        ScienceTreeUIData::from_player(self)
    }

    /// Get rank progress info for UI display
    ///
    /// This shows the player's progress toward the next rank.
    pub fn get_rank_progress_info(&self) -> Option<RankProgressInfo> {
        RankProgressInfo::from_player(self)
    }

    /// Get detailed info about a specific science
    ///
    /// Used for tooltips and detailed views in the UI.
    pub fn get_science_info_for_ui(&self, science: ScienceType) -> Option<PurchasableScienceInfo> {
        PurchasableScienceInfo::from_science(self, science)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Player, PlayerIndex};

    #[test]
    fn test_rank_progress_info_creation() {
        let player = Player::new(0 as PlayerIndex);

        // Even without rank store, this shouldn't crash
        let progress = player.get_rank_progress_info();
        // Will be None if rank store not initialized
        if let Some(info) = progress {
            assert_eq!(info.current_level, 1);
            assert_eq!(info.current_skill_points, 0);
        }
    }

    #[test]
    fn test_science_tree_ui_data_creation() {
        let player = Player::new(0 as PlayerIndex);

        // Even without science store, this shouldn't crash
        let ui_data = player.get_science_tree_ui_data();
        if let Some(data) = ui_data {
            assert_eq!(data.science_points, 0);
            assert_eq!(data.owned_sciences.len(), 0);
        }
    }

    #[test]
    fn test_purchasable_science_info_fields() {
        // Test that we can create and manipulate the info struct
        let info = PurchasableScienceInfo {
            science: 100,
            display_name: "Test Science".to_string(),
            description: "A test science".to_string(),
            cost: 5,
            can_afford: true,
            has_prereqs: true,
            missing_prereqs: Vec::new(),
        };

        assert!(info.is_purchasable());
        assert_eq!(info.cost, 5);
    }
}
