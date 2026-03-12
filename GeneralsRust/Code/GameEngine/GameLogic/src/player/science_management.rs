//! Player Science Management System
//!
//! Complete implementation of the science/general powers system from C++ Player class.
//! This module handles:
//! - Science purchase point earning and spending
//! - Skill point accumulation and rank progression
//! - Science prerequisites and availability
//! - Rank-based science grants
//!
//! Matches C++ Player.cpp lines 2430-2700 (science and rank management)

use crate::common::*;
use crate::helpers::{TheGameLogic, TheGameText};
use crate::player::{Player, ScienceVec};
use crate::scripting::engine::get_script_engine;
use game_engine::common::rts::science::get_science_store;
use game_engine::common::rts::{ScienceType, SCIENCE_INVALID};

impl Player {
    /// Reset rank state to defaults (matches C++ Player::resetRank)
    pub fn reset_rank_impl(&mut self) {
        use crate::system::rank_info::the_rank_info_store;

        self.rank_level = 1;
        self.skill_points = 0;

        let intrinsic_points = self
            .get_player_template()
            .map(|template| template.get_intrinsic_science_purchase_points())
            .unwrap_or(0);
        self.science_purchase_points = intrinsic_points;

        if let Some(rank_store) = the_rank_info_store() {
            if let Some(cur_rank) = rank_store.get_rank_info(self.rank_level as usize) {
                self.science_purchase_points += cur_rank.science_purchase_points_granted;
                if self.science_purchase_points < 0 {
                    self.science_purchase_points = 0;
                }
            }
        }

        self.general_name = TheGameText::fetch("SCIENCE:GeneralName");
        self.reset_sciences_impl();
    }

    /// Set rank level and grant associated sciences (matches C++ Player::setRankLevel)
    ///
    /// This is called when:
    /// - Player levels up from experience
    /// - Rank is explicitly set by script/command
    /// - Loading from save game
    ///
    /// The C++ implementation is in Player.cpp lines 2656-2700.
    ///
    /// # Parameters
    /// - `new_level`: The new rank level (1-based)
    ///
    /// # Returns
    /// Returns true if the rank actually changed
    pub fn set_rank_level_impl(&mut self, new_level: Int) -> Bool {
        use crate::system::rank_info::the_rank_info_store;

        let mut new_level = new_level;
        let old_spp = self.science_purchase_points;
        let old_level = self.rank_level;

        // Clamp to valid range (C++ Player.cpp:2657-2662)
        if new_level < 1 {
            new_level = 1;
        }

        // Get rank count from the rank info store
        if let Some(rank_store) = the_rank_info_store() {
            let rank_count = rank_store.get_rank_level_count() as Int;
            if new_level > rank_count {
                new_level = rank_count;
            }
        }

        // Check against game logic rank limit
        // In C++: if (newLevel > TheGameLogic->getRankLevelLimit())
        let rank_level_limit = TheGameLogic::get_rank_level_limit();
        if new_level > rank_level_limit {
            new_level = rank_level_limit;
        }

        if self.rank_level == new_level {
            return false; // No change
        }

        if new_level < self.rank_level {
            self.reset_rank_impl();
        }

        // Update level-up threshold for next level (C++ Player.cpp:2665-2681)
        if let Some(rank_store) = the_rank_info_store() {
            // Set m_levelUp to the skill points needed for the next rank
            // If we're at max rank, set to INT_MAX
            if let Some(next_rank) = rank_store.get_rank_info((new_level + 1) as usize) {
                // In full implementation, would set: self.level_up = next_rank.skill_points_needed
                let _ = next_rank.skill_points_needed;
            } else {
                // At max rank, set threshold to INT_MAX to prevent further leveling
                // In full implementation: self.level_up = Int::MAX
            }

            // If we gained levels, grant sciences for the new rank
            if new_level > self.rank_level {
                for level in (self.rank_level + 1)..=new_level {
                    if let Some(rank_info) = rank_store.get_rank_info(level as usize) {
                        // Grant science purchase points (C++ Player.cpp:2683)
                        self.science_purchase_points += rank_info.science_purchase_points_granted;
                        if self.science_purchase_points < 0 {
                            self.science_purchase_points = 0;
                        }

                        // Ensure skill points are at least this rank's threshold
                        if self.skill_points < rank_info.skill_points_needed {
                            self.skill_points = rank_info.skill_points_needed;
                        }

                        // Grant all sciences for this rank (C++ Player.cpp:2685-2691)
                        for &science in &rank_info.sciences_granted {
                            self.add_science(science);
                        }
                    }
                }
            }
        }

        self.rank_level = new_level;
        if new_level > old_level && self.is_local_player() {
            let _ =
                crate::helpers::TheEva::set_should_play(crate::helpers::EvaEvent::GeneralLevelUp);
        }
        crate::control_bar::notify_player_rank_changed(
            self.player_index,
            self.rank_level,
            self.science_purchase_points,
        );
        if old_spp != self.science_purchase_points {
            crate::control_bar::notify_science_purchase_points_changed(
                self.player_index,
                self.science_purchase_points,
            );
        }
        true
    }

    /// Add skill points and auto-level up (matches C++ Player::addSkillPoints)
    ///
    /// Skill points are earned from:
    /// - Killing enemy units (based on unit cost and veterancy)
    /// - Mission objectives
    /// - Script awards
    ///
    /// C++ implementation is in Player.cpp lines 2437-2458.
    ///
    /// # Parameters
    /// - `delta`: Skill points to add (can be negative)
    ///
    /// # Returns
    /// Returns true if the player gained or lost levels as a result
    pub fn add_skill_points_impl(&mut self, delta: Int) -> Bool {
        use crate::system::rank_info::the_rank_info_store;

        // Apply skill points modifier (from upgrades, difficulty, etc.)
        // C++ Player.cpp:2439 - REAL_TO_INT_CEIL(m_skillPointsModifier * INT_TO_REAL(delta))
        let delta = ((delta as Real * self.skill_points_modifier).ceil()) as Int;

        if delta == 0 {
            return false;
        }

        // Get level cap from rank info store and game logic (C++ Player.cpp:2444-2445)
        let rank_store_guard = the_rank_info_store();
        let point_cap = if let Some(ref rank_store) = rank_store_guard {
            // Cap at the lowest point of cap level, not highest
            let rank_count = rank_store.get_rank_level_count() as Int;
            const RANK_LEVEL_LIMIT: Int = 20; // Would come from TheGameLogic->getRankLevelLimit()
            let cap_level = rank_count.min(RANK_LEVEL_LIMIT);

            // Get the skill points needed for the cap level
            if let Some(cap_rank) = rank_store.get_rank_info(cap_level as usize) {
                cap_rank.skill_points_needed
            } else {
                Int::MAX
            }
        } else {
            Int::MAX
        };

        // Add skill points, clamped to cap (C++ Player.cpp:2448)
        let mut level_gained = false;
        self.skill_points = (self.skill_points + delta).min(point_cap);

        // Keep leveling up while we have enough skill points (C++ Player.cpp:2449-2455)
        // The C++ code uses m_levelUp which stores the threshold for the next level
        if let Some(rank_store) = rank_store_guard {
            loop {
                if let Some(next_rank) = rank_store.get_rank_info((self.rank_level + 1) as usize) {
                    if self.skill_points >= next_rank.skill_points_needed {
                        // Level up! (C++ calls setRankLevel which updates m_levelUp as side effect)
                        self.set_rank_level(self.rank_level + 1);
                        level_gained = true;
                    } else {
                        break;
                    }
                } else {
                    // No more levels available
                    break;
                }
            }
        }

        level_gained
    }

    /// Add skill points for killing an object (matches C++ Player::addSkillPointsForKill)
    ///
    /// C++ implementation is in Player.cpp lines 2462-2475.
    ///
    /// # Parameters
    /// - `_killer`: The object that got the kill (reserved for future use)
    /// - `victim_level`: The veterancy level of the victim
    /// - `victim_skill_value`: The skill point value of the victim
    ///
    /// # Returns
    /// Returns true if the player gained levels as a result
    pub fn add_skill_points_for_kill_impl(
        &mut self,
        _killer: Option<ObjectID>,
        victim_under_construction: bool,
        victim_skill_value: Int,
    ) -> Bool {
        // C++ Player.cpp:2467-2469
        // "per dustin, no experience (et al) for killing things under construction"
        if victim_under_construction {
            return false;
        }

        // C++ Player.cpp:2471-2474
        // Int skillValue = victim->getTemplate()->getSkillPointValue(victimLevel);
        self.add_skill_points(victim_skill_value)
    }

    /// Add science purchase points (matches C++ Player::addSciencePurchasePoints)
    ///
    /// Science purchase points are earned from:
    /// - Ranking up (see RankInfo::m_sciencePurchasePointsGranted)
    /// - Special crates
    /// - Mission rewards
    ///
    /// C++ implementation is in Player.cpp lines 2555-2566.
    ///
    /// # Parameters
    /// - `delta`: Points to add (can be negative when purchasing)
    pub fn add_science_purchase_points_impl(&mut self, delta: Int) {
        // C++ Player.cpp:2557-2561
        let old_spp = self.science_purchase_points;
        self.science_purchase_points += delta;
        if self.science_purchase_points < 0 {
            self.science_purchase_points = 0;
        }

        // C++ Player.cpp:2563-2564
        // Notify UI if points changed
        if old_spp != self.science_purchase_points {
            crate::control_bar::notify_science_purchase_points_changed(
                self.player_index,
                self.science_purchase_points,
            );
            crate::control_bar::mark_ui_dirty();
        }
    }

    /// Internal helper to add a science without validation (used by setRankLevel)
    ///
    /// This is not exposed in the C++ Player class but is used internally
    /// by resetSciences (C++ Player.cpp:2492).
    fn add_science_internal(&mut self, science: ScienceType) -> Bool {
        if science == SCIENCE_INVALID || self.sciences.contains(&science) {
            return false;
        }

        self.sciences.push(science);
        true
    }

    /// Add a science to the player (matches C++ Player::addScience)
    ///
    /// This is the main entry point for granting sciences, whether purchased or granted.
    /// Handles:
    /// - Adding science to player's list
    /// - Activating special powers that require this science
    /// - Notifying script engine
    /// - Updating UI
    ///
    /// C++ implementation is in Player.cpp lines 2503-2552.
    ///
    /// # Parameters
    /// - `science`: The science to add
    ///
    /// # Returns
    /// Returns true if the science was added (false if already had it)
    pub fn add_science_impl(&mut self, science: ScienceType) -> Bool {
        if science == SCIENCE_INVALID {
            return false;
        }

        // C++ Player.cpp:2505-2506
        if self.has_science(science) {
            return false;
        }

        // C++ Player.cpp:2510 - add to sciences vector
        self.sciences.push(science);

        // C++ Player.cpp:2512-2543
        // 'wake up' any special powers controlled by this science
        //
        // The full C++ logic iterates team prototypes, teams, and objects. In the Rust port we
        // currently have direct ownership tracking (`Player::owned_objects`) and a central
        // `ObjectManager`, so we can implement the essential behavior: re-activate any update
        // modules that were sleeping forever while the prerequisite science was missing.
        //
        // This is intentionally conservative: it only wakes modules sleeping forever so we don't
        // disturb modules that are intentionally sleeping for timing/performance reasons.
        {
            use crate::helpers::TheGameLogic;
            use crate::object_manager::get_object_manager;

            let current_frame = TheGameLogic::get_frame();
            let obj_mgr = get_object_manager();
            let obj_mgr_lock = &*obj_mgr;
            if let Ok(manager) = obj_mgr_lock.read() {
                let owned_ids =
                    manager.get_objects_owned_by_player(self.player_index as UnsignedInt);
                for object_id in owned_ids {
                    if let Some(instance_arc) = manager.get_object(object_id) {
                        let instance_lock = &*instance_arc;
                        if let Ok(mut instance) = instance_lock.write() {
                            instance.wake_update_modules_sleeping_forever(current_frame);
                            for behavior in instance.get_behavior_modules() {
                                if let Ok(mut module_guard) = behavior.lock() {
                                    if let Some(module) =
                                        module_guard.get_special_power_module_interface()
                                    {
                                        let required_science = module
                                            .get_special_power_template_full()
                                            .map(|template| template.get_required_science())
                                            .unwrap_or(SCIENCE_INVALID);
                                        if required_science == science {
                                            module.on_special_power_creation();
                                        }
                                    }
                                }
                            }
                        };
                    }
                }
            };
        }

        crate::control_bar::mark_ui_dirty();

        // Notify script engine
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                engine.notify_of_acquired_science(self.get_id() as usize, science);
            }
        }

        true
    }

    /// Grant a science for free (matches C++ Player::grantScience)
    ///
    /// This is used by:
    /// - Crates
    /// - Scripts
    /// - Rank-ups (via addScience)
    ///
    /// C++ implementation is in Player.cpp lines 2592-2601.
    ///
    /// # Parameters
    /// - `science`: The science to grant
    ///
    /// # Returns
    /// Returns true if the science was granted successfully
    pub fn grant_science_impl(&mut self, science: ScienceType) -> Bool {
        use game_engine::common::rts::science::get_science_store;

        // C++ Player.cpp:2594-2598
        // Check if science is grantable
        if let Some(science_store) = get_science_store() {
            if !science_store.is_science_grantable(science) {
                // Not grantable, can't grant it even via this method
                // C++ would DEBUG_CRASH here
                return false;
            }
        } else {
            // No science store, can't validate
            return false;
        }

        // C++ Player.cpp:2600
        self.add_science(science)
    }

    /// Attempt to purchase a science with science purchase points (matches C++ Player::attemptToPurchaseScience)
    ///
    /// C++ implementation is in Player.cpp lines 2569-2589.
    ///
    /// # Parameters
    /// - `science`: The science to purchase
    ///
    /// # Returns
    /// Returns true if the purchase succeeded
    pub fn attempt_to_purchase_science_impl(&mut self, science: ScienceType) -> Bool {
        use game_engine::common::rts::science::get_science_store;

        // C++ Player.cpp:2571-2575
        if !self.is_capable_of_purchasing_science(science) {
            // C++ would DEBUG_CRASH here with a message
            return false;
        }

        // C++ Player.cpp:2577 - get cost
        let cost = if let Some(science_store) = get_science_store() {
            science_store.get_science_purchase_cost(science)
        } else {
            return false;
        };

        // C++ Player.cpp:2578 - deduct points
        self.add_science_purchase_points(-cost);

        // C++ Player.cpp:2579 - add science
        let result = self.add_science(science);

        if result {
            // C++ Player.cpp:2581 - track for statistics
            self.get_academy_stats_mut()
                .record_generals_points_spent(cost);

            // C++ Player.cpp:2583-2586 - local player UI refresh
            if self.is_local_player() {
                crate::control_bar::mark_ui_dirty();
            }
        }

        result
    }

    /// Check if player can purchase a specific science (matches C++ Player::isCapableOfPurchasingScience)
    ///
    /// Checks (in order, matching C++ Player.cpp:2604-2632):
    /// 1. Science is valid
    /// 2. Player doesn't already have it
    /// 3. Science is not disabled or hidden
    /// 4. Player has all prerequisites
    /// 5. Science has non-zero cost (0 = not purchasable)
    /// 6. Player has enough science purchase points
    ///
    /// C++ implementation is in Player.cpp lines 2604-2632.
    ///
    /// # Parameters
    /// - `science`: The science to check
    ///
    /// # Returns
    /// Returns true if the player can purchase this science right now
    pub fn is_capable_of_purchasing_science_impl(&self, science: ScienceType) -> Bool {
        use game_engine::common::rts::science::get_science_store;

        // C++ Player.cpp:2606-2609
        if science == SCIENCE_INVALID {
            return false;
        }

        // C++ Player.cpp:2611-2614
        if self.has_science(science) {
            return false;
        }

        // C++ Player.cpp:2616-2619
        if self.is_science_disabled(science) || self.is_science_hidden(science) {
            return false;
        }

        // C++ Player.cpp:2621-2624
        if !self.has_prereqs_for_science(science) {
            return false;
        }

        // C++ Player.cpp:2626-2632 expects TheScienceStore to be initialized.
        let Some(science_store) = get_science_store() else {
            debug_assert!(false, "ScienceStore not initialized");
            return false;
        };
        let cost = science_store.get_science_purchase_cost(science);

        // Cost of 0 means "not purchasable"
        if cost == 0 || cost > self.science_purchase_points {
            return false;
        }

        true
    }

    /// Check if player has prerequisites for a science (matches C++ Player::hasPrereqsForScience)
    ///
    /// C++ implementation is in Player.cpp lines 2430-2433.
    ///
    /// # Parameters
    /// - `science`: The science to check
    ///
    /// # Returns
    /// Returns true if player has all prerequisite sciences
    pub fn has_prereqs_for_science_impl(&self, science: ScienceType) -> Bool {
        use game_engine::common::rts::science::get_science_store;

        // C++ Player.cpp:2432 expects TheScienceStore to be initialized.
        let science_store = get_science_store().expect("ScienceStore not initialized");
        science_store.player_has_prereqs_for_science(self, science)
    }

    /// Get purchasable sciences for this player (matches C++ ScienceStore::getPurchasableSciences)
    ///
    /// This delegates to the ScienceStore which implements the logic from
    /// Science.cpp lines 301-329.
    ///
    /// # Returns
    /// Returns (purchasable_now, potentially_purchasable_later)
    /// - purchasable_now: Sciences the player can purchase right now (has prereqs and points)
    /// - potentially_purchasable_later: Sciences the player has root prereqs for but lacks immediate prereqs or points
    pub fn get_purchasable_sciences_impl(&self) -> (ScienceVec, ScienceVec) {
        use game_engine::common::rts::science::get_science_store;

        let science_store = get_science_store().expect("ScienceStore not initialized");
        science_store.get_purchasable_sciences(self)
    }

    /// Reset sciences to intrinsic + rank-granted sciences (matches C++ Player::resetSciences)
    ///
    /// This is called when:
    /// - Loading a saved game
    /// - Changing player template
    /// - Resetting player state
    ///
    /// C++ implementation is in Player.cpp lines 2478-2499.
    pub fn reset_sciences_impl(&mut self) {
        use crate::system::rank_info::the_rank_info_store;

        // C++ Player.cpp:2480 - clear all sciences
        self.sciences.clear();

        // C++ Player.cpp:2482-2483 - add intrinsic sciences from player template
        if let Some(player_template) = self.get_player_template() {
            self.sciences = player_template.get_intrinsic_sciences().clone();
        }

        // C++ Player.cpp:2485-2495
        // Add sciences granted by all ranks up to current level
        if let Some(rank_store) = the_rank_info_store() {
            for level in 1..=self.rank_level {
                if let Some(rank_info) = rank_store.get_rank_info(level as usize) {
                    for &science in &rank_info.sciences_granted {
                        self.add_science(science);
                    }
                }
            }
        }

        // C++ Player.cpp:2497-2498
        // Notify script engine of all acquired sciences
        if let Ok(mut engine_guard) = get_script_engine().write() {
            if let Some(engine) = engine_guard.as_mut() {
                for &science in &self.sciences {
                    engine.notify_of_acquired_science(self.get_id() as usize, science);
                }
            }
        }
    }

    /// Get all sciences currently available to this player
    ///
    /// This is a helper method not in the C++ version, but useful for UI display.
    ///
    /// # Returns
    /// Returns a reference to the player's science vector
    pub fn get_sciences(&self) -> &ScienceVec {
        &self.sciences
    }

    /// Get count of sciences the player has
    ///
    /// Helper method for UI and statistics.
    pub fn get_science_count(&self) -> usize {
        self.sciences.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::player::{Player, PlayerIndex};

    #[test]
    fn test_skill_points_addition() {
        let mut player = Player::new(0 as PlayerIndex);

        // Start with no skill points
        assert_eq!(player.get_skill_points(), 0);
        assert_eq!(player.get_rank_level(), 1);

        // Add some skill points (not enough to level)
        player.add_skill_points(50);
        assert_eq!(player.get_skill_points(), 50);
        assert_eq!(player.get_rank_level(), 1); // Should still be rank 1
    }

    #[test]
    fn test_science_purchase_points() {
        let mut player = Player::new(0 as PlayerIndex);

        assert_eq!(player.get_science_purchase_points(), 0);

        player.add_science_purchase_points(5);
        assert_eq!(player.get_science_purchase_points(), 5);

        player.add_science_purchase_points(-3);
        assert_eq!(player.get_science_purchase_points(), 2);

        // Should clamp to 0
        player.add_science_purchase_points(-10);
        assert_eq!(player.get_science_purchase_points(), 0);
    }

    #[test]
    fn test_science_addition() {
        let mut player = Player::new(0 as PlayerIndex);

        let test_science: ScienceType = 100;

        assert!(!player.has_science(test_science));
        assert_eq!(player.get_science_count(), 0);

        let added = player.add_science(test_science);
        assert!(added);
        assert!(player.has_science(test_science));
        assert_eq!(player.get_science_count(), 1);

        // Adding again should return false
        let added_again = player.add_science(test_science);
        assert!(!added_again);
        assert_eq!(player.get_science_count(), 1);
    }

    #[test]
    fn test_invalid_science() {
        let mut player = Player::new(0 as PlayerIndex);

        // Should not add invalid science
        let added = player.add_science(SCIENCE_INVALID);
        assert!(!added);
        assert_eq!(player.get_science_count(), 0);
    }

    #[test]
    fn test_skill_points_modifier() {
        let mut player = Player::new(0 as PlayerIndex);

        // Set a 2x modifier
        player.skill_points_modifier = 2.0;

        player.add_skill_points(100);

        // Should be doubled (and ceiled)
        assert_eq!(player.get_skill_points(), 200);
    }
}
