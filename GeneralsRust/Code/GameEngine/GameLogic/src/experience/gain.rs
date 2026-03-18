//! Experience Gain System - Handles awarding experience from damage and kills
//!
//! This module implements the experience gain mechanics matching the C++ implementation,
//! including damage-based XP, kill bonuses, and squad sharing.

use crate::common::types::{ObjectID, VeterancyLevel};
use crate::experience::{ExperienceRequirements, ExperienceTracker};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

/// Result of an experience gain operation
#[derive(Debug, Clone, PartialEq)]
pub struct ExperienceGainResult {
    /// Whether a promotion occurred
    pub promoted: bool,

    /// Old veterancy level (if promoted)
    pub old_level: Option<VeterancyLevel>,

    /// New veterancy level (if promoted)
    pub new_level: Option<VeterancyLevel>,

    /// Amount of experience gained
    pub experience_gained: i32,
}

impl ExperienceGainResult {
    /// Create result for no promotion
    pub fn no_promotion(experience_gained: i32) -> Self {
        Self {
            promoted: false,
            old_level: None,
            new_level: None,
            experience_gained,
        }
    }

    /// Create result for promotion
    pub fn promotion(
        old_level: VeterancyLevel,
        new_level: VeterancyLevel,
        experience_gained: i32,
    ) -> Self {
        Self {
            promoted: true,
            old_level: Some(old_level),
            new_level: Some(new_level),
            experience_gained,
        }
    }
}

/// Experience gain manager - Handles awarding experience to objects
///
/// This system matches the C++ Object::onDamageDealt and Object::onKill callbacks
pub struct ExperienceGainManager {
    /// Map of object ID to experience tracker
    trackers: HashMap<ObjectID, Arc<Mutex<ExperienceTracker>>>,

    /// Map of object ID to experience requirements (based on cost)
    requirements: HashMap<ObjectID, ExperienceRequirements>,
}

impl ExperienceGainManager {
    /// Create a new experience gain manager
    pub fn new() -> Self {
        Self {
            trackers: HashMap::new(),
            requirements: HashMap::new(),
        }
    }

    /// Register an object's experience tracker
    ///
    /// # Parameters
    /// - `object_id`: ID of the object
    /// - `tracker`: Experience tracker for this object
    /// - `build_cost`: Build cost of the object (for calculating requirements)
    pub fn register_object(
        &mut self,
        object_id: ObjectID,
        tracker: Arc<Mutex<ExperienceTracker>>,
        build_cost: i32,
    ) {
        self.trackers.insert(object_id, tracker);
        self.requirements.insert(
            object_id,
            ExperienceRequirements::from_build_cost(build_cost),
        );
    }

    /// Unregister an object's experience tracker
    pub fn unregister_object(&mut self, object_id: ObjectID) {
        self.trackers.remove(&object_id);
        self.requirements.remove(&object_id);
    }

    /// Award experience for damage dealt (matches C++ Object::onDamageDealt)
    ///
    /// # Parameters
    /// - `attacker_id`: ID of the object that dealt damage
    /// - `damage_dealt`: Amount of damage dealt
    /// - `can_scale`: Whether to apply experience scalar bonuses
    ///
    /// # Returns
    /// Result indicating if promotion occurred
    pub fn award_damage_experience(
        &self,
        attacker_id: ObjectID,
        damage_dealt: f32,
        can_scale: bool,
    ) -> Option<ExperienceGainResult> {
        // Get the attacker's tracker and requirements
        let tracker = self.trackers.get(&attacker_id)?;
        let requirements = self.requirements.get(&attacker_id)?;

        // Calculate experience from damage
        let experience_gain = ExperienceTracker::calculate_damage_experience(damage_dealt);

        if experience_gain <= 0 {
            return None;
        }

        // Award experience
        let mut tracker_guard = tracker.lock().ok()?;
        let _old_level = tracker_guard.get_veterancy_level();

        let promotion = tracker_guard.add_experience_points(
            experience_gain,
            can_scale,
            requirements.as_array(),
        );

        let new_level = tracker_guard.get_veterancy_level();

        if let Some(old) = promotion {
            Some(ExperienceGainResult::promotion(
                old,
                new_level,
                experience_gain,
            ))
        } else {
            Some(ExperienceGainResult::no_promotion(experience_gain))
        }
    }

    /// Award experience for killing an enemy (matches C++ Object::onKill)
    ///
    /// # Parameters
    /// - `killer_id`: ID of the object that got the kill
    /// - `victim_id`: ID of the object that was killed
    /// - `victim_cost`: Build cost of the victim
    /// - `victim_level`: Veterancy level of the victim
    /// - `killer_is_ally`: True if killer is an ally (no XP for team kills)
    /// - `can_scale`: Whether to apply experience scalar bonuses
    ///
    /// # Returns
    /// Result indicating if promotion occurred
    pub fn award_kill_experience(
        &self,
        killer_id: ObjectID,
        victim_id: ObjectID,
        victim_cost: i32,
        _victim_level: VeterancyLevel,
        killer_is_ally: bool,
        can_scale: bool,
    ) -> Option<ExperienceGainResult> {
        // Get the killer's tracker and requirements
        let killer_tracker = self.trackers.get(&killer_id)?;
        let killer_requirements = self.requirements.get(&killer_id)?;

        // Get the victim's tracker to calculate experience value
        let victim_tracker = self.trackers.get(&victim_id)?;
        let victim_tracker_guard = victim_tracker.lock().ok()?;

        // Calculate experience value for the kill
        let experience_gain =
            victim_tracker_guard.get_experience_value(victim_cost, killer_is_ally);

        if experience_gain <= 0 {
            return None;
        }

        drop(victim_tracker_guard);

        // Award experience to killer
        let mut killer_tracker_guard = killer_tracker.lock().ok()?;
        let _old_level = killer_tracker_guard.get_veterancy_level();

        let promotion = killer_tracker_guard.add_experience_points(
            experience_gain,
            can_scale,
            killer_requirements.as_array(),
        );

        let new_level = killer_tracker_guard.get_veterancy_level();

        if let Some(old) = promotion {
            Some(ExperienceGainResult::promotion(
                old,
                new_level,
                experience_gain,
            ))
        } else {
            Some(ExperienceGainResult::no_promotion(experience_gain))
        }
    }

    /// Share experience among squad members (matches C++ squad experience sharing)
    ///
    /// # Parameters
    /// - `primary_id`: ID of the primary object that gained experience
    /// - `squad_member_ids`: IDs of other squad members to share with
    /// - `experience_amount`: Amount of experience to share
    /// - `share_percentage`: Percentage to share with each member (typically 0.5 = 50%)
    /// - `can_scale`: Whether to apply experience scalar bonuses
    ///
    /// # Returns
    /// Map of object IDs to their gain results
    pub fn share_squad_experience(
        &self,
        primary_id: ObjectID,
        squad_member_ids: &[ObjectID],
        experience_amount: i32,
        share_percentage: f32,
        can_scale: bool,
    ) -> HashMap<ObjectID, ExperienceGainResult> {
        let mut results = HashMap::new();

        // Calculate shared amount
        let shared_amount = (experience_amount as f32 * share_percentage) as i32;

        if shared_amount <= 0 {
            return results;
        }

        // Award to each squad member
        for &member_id in squad_member_ids {
            // Don't share with self
            if member_id == primary_id {
                continue;
            }

            // Get member's tracker and requirements
            if let (Some(tracker), Some(requirements)) = (
                self.trackers.get(&member_id),
                self.requirements.get(&member_id),
            ) {
                if let Ok(mut tracker_guard) = tracker.lock() {
                    let _old_level = tracker_guard.get_veterancy_level();

                    let promotion = tracker_guard.add_experience_points(
                        shared_amount,
                        can_scale,
                        requirements.as_array(),
                    );

                    let new_level = tracker_guard.get_veterancy_level();

                    let result = if let Some(old) = promotion {
                        ExperienceGainResult::promotion(old, new_level, shared_amount)
                    } else {
                        ExperienceGainResult::no_promotion(shared_amount)
                    };

                    results.insert(member_id, result);
                }
            }
        }

        results
    }

    /// Transfer experience to a sink object (matches C++ experience sink system)
    ///
    /// This is used when units should redirect their experience to another object,
    /// such as when aircraft return experience to their airfield.
    ///
    /// # Parameters
    /// - `source_id`: ID of the object gaining experience
    /// - `sink_id`: ID of the object to receive the experience
    /// - `experience_amount`: Amount of experience to transfer
    /// - `can_scale`: Whether to apply experience scalar bonuses
    ///
    /// # Returns
    /// Result for the sink object
    pub fn transfer_to_sink(
        &self,
        source_id: ObjectID,
        sink_id: ObjectID,
        experience_amount: i32,
        can_scale: bool,
    ) -> Option<ExperienceGainResult> {
        // Get source tracker to check for scalar
        let source_tracker = self.trackers.get(&source_id)?;
        let source_guard = source_tracker.lock().ok()?;
        let source_scalar = source_guard.get_experience_scalar();
        drop(source_guard);

        // Get sink tracker and requirements
        let sink_tracker = self.trackers.get(&sink_id)?;
        let sink_requirements = self.requirements.get(&sink_id)?;

        // Calculate amount with source's scalar
        let scaled_amount = if can_scale {
            (experience_amount as f32 * source_scalar) as i32
        } else {
            experience_amount
        };

        // Award to sink
        let mut sink_guard = sink_tracker.lock().ok()?;
        let _old_level = sink_guard.get_veterancy_level();

        let promotion =
            sink_guard.add_experience_points(scaled_amount, false, sink_requirements.as_array());

        let new_level = sink_guard.get_veterancy_level();

        if let Some(old) = promotion {
            Some(ExperienceGainResult::promotion(
                old,
                new_level,
                scaled_amount,
            ))
        } else {
            Some(ExperienceGainResult::no_promotion(scaled_amount))
        }
    }
}

impl Default for ExperienceGainManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to create a test tracker with requirements
    fn create_test_tracker(object_id: ObjectID, build_cost: i32) -> Arc<Mutex<ExperienceTracker>> {
        Arc::new(Mutex::new(ExperienceTracker::new(object_id)))
    }

    #[test]
    fn test_experience_gain_manager_creation() {
        let manager = ExperienceGainManager::new();
        assert!(manager.trackers.is_empty());
        assert!(manager.requirements.is_empty());
    }

    #[test]
    fn test_register_object() {
        let mut manager = ExperienceGainManager::new();
        let tracker = create_test_tracker(1, 1000);

        manager.register_object(1, tracker, 1000);

        assert_eq!(manager.trackers.len(), 1);
        assert_eq!(manager.requirements.len(), 1);
    }

    #[test]
    fn test_unregister_object() {
        let mut manager = ExperienceGainManager::new();
        let tracker = create_test_tracker(1, 1000);

        manager.register_object(1, tracker, 1000);
        manager.unregister_object(1);

        assert!(manager.trackers.is_empty());
        assert!(manager.requirements.is_empty());
    }

    #[test]
    fn test_award_damage_experience() {
        let mut manager = ExperienceGainManager::new();
        let tracker = create_test_tracker(1, 1000);

        manager.register_object(1, tracker.clone(), 1000);

        // Deal 500 damage = 50 XP (damage * 0.1)
        let result = manager.award_damage_experience(1, 500.0, false);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.experience_gained, 50);
        assert!(!result.promoted);

        // Check tracker has the XP
        let tracker_guard = tracker.lock().unwrap();
        assert_eq!(tracker_guard.get_current_experience(), 50);
    }

    #[test]
    fn test_award_damage_experience_with_promotion() {
        let mut manager = ExperienceGainManager::new();
        let tracker = create_test_tracker(1, 1000);

        manager.register_object(1, tracker.clone(), 1000);

        // Deal 15000 damage = 1500 XP (enough for Veteran at 1000)
        let result = manager.award_damage_experience(1, 15000.0, false);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.experience_gained, 1500);
        assert!(result.promoted);
        assert_eq!(result.old_level, Some(VeterancyLevel::Regular));
        assert_eq!(result.new_level, Some(VeterancyLevel::Veteran));
    }

    #[test]
    fn test_award_kill_experience() {
        let mut manager = ExperienceGainManager::new();
        let killer_tracker = create_test_tracker(1, 1000);
        let victim_tracker = create_test_tracker(2, 600);

        manager.register_object(1, killer_tracker.clone(), 1000);
        manager.register_object(2, victim_tracker, 600);

        // Kill worth 300 XP (600 * 0.5)
        let result =
            manager.award_kill_experience(1, 2, 600, VeterancyLevel::Regular, false, false);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.experience_gained, 300);
        assert!(!result.promoted);

        // Check tracker has the XP
        let killer_guard = killer_tracker.lock().unwrap();
        assert_eq!(killer_guard.get_current_experience(), 300);
    }

    #[test]
    fn test_award_kill_experience_no_team_kill() {
        let mut manager = ExperienceGainManager::new();
        let killer_tracker = create_test_tracker(1, 1000);
        let victim_tracker = create_test_tracker(2, 600);

        manager.register_object(1, killer_tracker.clone(), 1000);
        manager.register_object(2, victim_tracker, 600);

        // Team kill = no XP
        let result = manager.award_kill_experience(1, 2, 600, VeterancyLevel::Regular, true, false);

        assert!(result.is_none());

        // Check tracker has no XP
        let killer_guard = killer_tracker.lock().unwrap();
        assert_eq!(killer_guard.get_current_experience(), 0);
    }

    #[test]
    fn test_award_kill_veteran_enemy_worth_more() {
        let mut manager = ExperienceGainManager::new();
        let killer_tracker = create_test_tracker(1, 1000);
        let victim_tracker = create_test_tracker(2, 600);

        // Set victim to Veteran level
        {
            let mut victim_guard = victim_tracker.lock().unwrap();
            let req = ExperienceRequirements::from_build_cost(600);
            victim_guard
                .set_veterancy_level_with_requirements(VeterancyLevel::Veteran, req.as_array());
        }

        manager.register_object(1, killer_tracker.clone(), 1000);
        manager.register_object(2, victim_tracker, 600);

        // Veteran kill worth more: 600 * 0.5 * 1.25 = 375 XP
        let result =
            manager.award_kill_experience(1, 2, 600, VeterancyLevel::Veteran, false, false);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.experience_gained, 375);
    }

    #[test]
    fn test_share_squad_experience() {
        let mut manager = ExperienceGainManager::new();
        let primary_tracker = create_test_tracker(1, 1000);
        let member1_tracker = create_test_tracker(2, 1000);
        let member2_tracker = create_test_tracker(3, 1000);

        manager.register_object(1, primary_tracker, 1000);
        manager.register_object(2, member1_tracker.clone(), 1000);
        manager.register_object(3, member2_tracker.clone(), 1000);

        let squad_members = vec![1, 2, 3];

        // Share 100 XP at 50%
        let results = manager.share_squad_experience(1, &squad_members, 100, 0.5, false);

        // Should have 2 results (not shared with self)
        assert_eq!(results.len(), 2);

        // Each member should get 50 XP
        assert!(results.contains_key(&2));
        assert!(results.contains_key(&3));

        let result2 = results.get(&2).unwrap();
        assert_eq!(result2.experience_gained, 50);

        let result3 = results.get(&3).unwrap();
        assert_eq!(result3.experience_gained, 50);

        // Verify trackers
        let member1_guard = member1_tracker.lock().unwrap();
        assert_eq!(member1_guard.get_current_experience(), 50);

        let member2_guard = member2_tracker.lock().unwrap();
        assert_eq!(member2_guard.get_current_experience(), 50);
    }

    #[test]
    fn test_transfer_to_sink() {
        let mut manager = ExperienceGainManager::new();
        let source_tracker = create_test_tracker(1, 600);
        let sink_tracker = create_test_tracker(2, 3000);

        // Set source to have 2x experience scalar
        {
            let mut source_guard = source_tracker.lock().unwrap();
            source_guard.set_experience_scalar(2.0);
        }

        manager.register_object(1, source_tracker, 600);
        manager.register_object(2, sink_tracker.clone(), 3000);

        // Transfer 100 XP with scaling
        let result = manager.transfer_to_sink(1, 2, 100, true);

        assert!(result.is_some());
        let result = result.unwrap();
        // 100 * 2.0 = 200 XP
        assert_eq!(result.experience_gained, 200);

        // Verify sink got the scaled XP
        let sink_guard = sink_tracker.lock().unwrap();
        assert_eq!(sink_guard.get_current_experience(), 200);
    }

    #[test]
    fn test_experience_scalar_application() {
        let mut manager = ExperienceGainManager::new();
        let tracker = create_test_tracker(1, 1000);

        // Set 2x experience scalar
        {
            let mut guard = tracker.lock().unwrap();
            guard.set_experience_scalar(2.0);
        }

        manager.register_object(1, tracker.clone(), 1000);

        // Deal 500 damage = 50 XP base, 100 XP with scalar
        let result = manager.award_damage_experience(1, 500.0, true);

        assert!(result.is_some());
        let result = result.unwrap();
        assert_eq!(result.experience_gained, 50); // Base amount recorded

        // Check tracker has scaled XP
        let tracker_guard = tracker.lock().unwrap();
        assert_eq!(tracker_guard.get_current_experience(), 100); // 50 * 2.0
    }
}
