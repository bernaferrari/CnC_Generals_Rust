//! ExperienceTracker - Core experience tracking (matches C++ ExperienceTracker.h/cpp)
//!
//! This is the lightweight tracker that each Object owns to track its experience
//! points and veterancy level. It matches the C++ implementation exactly.

use crate::common::types::{ObjectID, VeterancyLevel};
use crate::helpers::TheGameLogic;

/// Core experience tracker (matches C++ ExperienceTracker class)
///
/// Each object has one of these to track its experience and veterancy level.
/// This is a direct port of the C++ ExperienceTracker class.
#[derive(Debug, Clone)]
pub struct ExperienceTracker {
    /// Object ID of the owner
    owner_id: ObjectID,

    /// Current veterancy level (LEVEL_REGULAR, LEVEL_VETERAN, LEVEL_ELITE, LEVEL_HEROIC)
    current_level: VeterancyLevel,

    /// Current experience points accumulated
    current_experience: i32,

    /// ID of object that receives our experience (for transferred experience)
    /// Set to INVALID_ID if we keep our own experience
    experience_sink: ObjectID,

    /// Scalar multiplier for experience gain (default 1.0)
    /// Modified by ExperienceScalarUpgrade and other effects
    experience_scalar: f32,
}

impl ExperienceTracker {
    /// Invalid object ID constant
    pub const INVALID_ID: ObjectID = 0;

    /// Experience gain from damage formula: damage * 0.1
    pub const DAMAGE_TO_XP_RATIO: f32 = 0.1;

    /// Experience gain from kill formula: target_cost * 0.5
    pub const KILL_XP_RATIO: f32 = 0.5;
    /// Default experience thresholds for each veterancy level.
    pub const DEFAULT_EXPERIENCE_REQUIRED: [i32; 4] = [0, 100, 300, 600];

    /// Create a new experience tracker for an object
    pub fn new(owner_id: ObjectID) -> Self {
        Self {
            owner_id,
            current_level: VeterancyLevel::Regular,
            current_experience: 0,
            experience_sink: Self::INVALID_ID,
            experience_scalar: 1.0,
        }
    }

    /// Get the owner object ID
    pub fn owner_id(&self) -> ObjectID {
        self.owner_id
    }

    /// Get current veterancy level (matches C++ getVeterancyLevel)
    pub fn get_veterancy_level(&self) -> VeterancyLevel {
        self.current_level
    }

    /// Get current experience points (matches C++ getCurrentExperience)
    pub fn get_current_experience(&self) -> i32 {
        self.current_experience
    }

    /// Get the experience scalar multiplier (matches C++ getExperienceScalar)
    pub fn get_experience_scalar(&self) -> f32 {
        self.experience_scalar
    }

    /// Set the experience scalar multiplier (matches C++ setExperienceScalar)
    pub fn set_experience_scalar(&mut self, scalar: f32) {
        self.experience_scalar = scalar;
    }

    /// Set experience sink - redirect experience to another object (matches C++ setExperienceSink)
    ///
    /// This is used when units should transfer their experience to another object,
    /// such as when aircraft return experience to their airfield.
    pub fn set_experience_sink(&mut self, sink: ObjectID) {
        self.experience_sink = sink;
    }

    /// Get the experience sink ID
    pub fn get_experience_sink(&self) -> ObjectID {
        self.experience_sink
    }

    /// Check if we're redirecting experience to another object
    pub fn has_experience_sink(&self) -> bool {
        self.experience_sink != Self::INVALID_ID
    }

    /// Set veterancy level using default experience requirements
    ///
    /// Convenience method that uses DEFAULT_EXPERIENCE_REQUIRED.
    /// This is used for explicit setting (e.g., from crates or scripts).
    /// Returns the old level if it changed.
    pub fn set_veterancy_level(&mut self, new_level: VeterancyLevel) -> Option<VeterancyLevel> {
        self.set_veterancy_level_with_requirements(new_level, &Self::DEFAULT_EXPERIENCE_REQUIRED)
    }

    /// Set veterancy level explicitly with custom experience requirements (matches C++ setVeterancyLevel)
    ///
    /// This is used for explicit setting (e.g., from crates or scripts).
    /// Returns the old level if it changed.
    pub fn set_veterancy_level_with_requirements(
        &mut self,
        new_level: VeterancyLevel,
        experience_required: &[i32],
    ) -> Option<VeterancyLevel> {
        if self.current_level != new_level {
            let old_level = self.current_level;
            self.current_level = new_level;

            // Set experience to minimum for this level
            self.current_experience = experience_required[new_level as usize];

            Some(old_level)
        } else {
            None
        }
    }

    /// Set minimum veterancy level (matches C++ setMinVeterancyLevel)
    ///
    /// Sets level to AT LEAST this value. If already >= this level, does nothing.
    /// Returns the old level if it changed.
    pub fn set_min_veterancy_level(
        &mut self,
        new_level: VeterancyLevel,
        experience_required: &[i32],
    ) -> Option<VeterancyLevel> {
        if self.current_level < new_level {
            let old_level = self.current_level;
            self.current_level = new_level;

            // Set experience to minimum for this level
            self.current_experience = experience_required[new_level as usize];

            Some(old_level)
        } else {
            None
        }
    }

    /// Add experience points (matches C++ addExperiencePoints)
    ///
    /// # Parameters
    /// - `experience_gain`: Base experience to add
    /// - `can_scale_for_bonus`: If true, apply experience scalar multiplier
    /// - `experience_required`: Array of XP required for each level [Regular, Veteran, Elite, Heroic]
    ///
    /// # Returns
    /// Returns the old level if promotion occurred, None otherwise
    pub fn add_experience_points(
        &mut self,
        experience_gain: i32,
        can_scale_for_bonus: bool,
        experience_required: &[i32],
    ) -> Option<VeterancyLevel> {
        if self.experience_sink != Self::INVALID_ID {
            if let Some(sink) = TheGameLogic::find_object_by_id(self.experience_sink) {
                if let Ok(sink_guard) = sink.read() {
                    if let Some(tracker) = sink_guard.get_experience_tracker() {
                        if let Ok(mut tracker_guard) = tracker.lock() {
                            return tracker_guard.add_experience_points(
                                experience_gain,
                                can_scale_for_bonus,
                                experience_required,
                            );
                        }
                    }
                }
            }
        }

        let old_level = self.current_level;

        // Calculate actual amount to gain
        let amount_to_gain = if can_scale_for_bonus {
            (experience_gain as f32 * self.experience_scalar) as i32
        } else {
            experience_gain
        };

        self.current_experience += amount_to_gain;

        // Check for level ups
        self.update_level_from_experience(experience_required);

        if old_level != self.current_level {
            Some(old_level)
        } else {
            None
        }
    }

    /// Gain enough experience to reach a specific level (matches C++ gainExpForLevel)
    ///
    /// # Returns
    /// Returns true if we gained at least one level, false otherwise
    pub fn gain_exp_for_level(
        &mut self,
        levels_to_gain: i32,
        can_scale_for_bonus: bool,
        experience_required: &[i32],
    ) -> bool {
        if levels_to_gain <= 0 {
            return false;
        }

        let new_level = (self.current_level as i32) + levels_to_gain;
        if new_level > VeterancyLevel::Heroic as i32 {
            return false;
        }

        if new_level > self.current_level as i32 {
            let experience_needed =
                experience_required[new_level as usize] - self.current_experience;
            self.add_experience_points(experience_needed, can_scale_for_bonus, experience_required);
            true
        } else {
            false
        }
    }

    /// Check if we can gain levels without actually doing it (matches C++ canGainExpForLevel)
    pub fn can_gain_exp_for_level(&self, levels_to_gain: i32) -> bool {
        if levels_to_gain <= 0 {
            return false;
        }

        let new_level = (self.current_level as i32) + levels_to_gain;
        if new_level > VeterancyLevel::Heroic as i32 {
            return false;
        }

        new_level > self.current_level as i32
    }

    /// Check if this object can be trained (gain at least one level)
    ///
    /// This is a convenience method that checks if the object can gain one level.
    /// Returns true if the object is not yet at max veterancy level.
    pub fn is_trainable(&self) -> bool {
        self.can_gain_exp_for_level(1)
    }

    /// Set experience and recalculate level (matches C++ setExperienceAndLevel)
    ///
    /// This is used when loading from save or explicitly setting experience.
    /// Returns the old level if it changed.
    pub fn set_experience_and_level(
        &mut self,
        experience: i32,
        experience_required: &[i32],
    ) -> Option<VeterancyLevel> {
        if self.experience_sink != Self::INVALID_ID {
            if let Some(sink) = TheGameLogic::find_object_by_id(self.experience_sink) {
                if let Ok(sink_guard) = sink.read() {
                    if let Some(tracker) = sink_guard.get_experience_tracker() {
                        if let Ok(mut tracker_guard) = tracker.lock() {
                            return tracker_guard
                                .set_experience_and_level(experience, experience_required);
                        }
                    }
                }
            }
        }

        let old_level = self.current_level;
        self.current_experience = experience;

        self.update_level_from_experience(experience_required);

        if old_level != self.current_level {
            Some(old_level)
        } else {
            None
        }
    }

    /// Check if we're accepting experience points (matches C++ isAcceptingExperiencePoints)
    pub fn is_accepting_experience_points(&self) -> bool {
        self.is_trainable() || self.experience_sink != Self::INVALID_ID
    }

    /// Update level based on current experience (internal helper)
    fn update_level_from_experience(&mut self, experience_required: &[i32]) {
        // Find the highest level we qualify for
        let mut level_index = 0;

        // Check if we qualify for Veteran
        if self.current_experience >= experience_required[VeterancyLevel::Veteran as usize] {
            level_index = VeterancyLevel::Veteran as usize;
        }

        // Check if we qualify for Elite
        if self.current_experience >= experience_required[VeterancyLevel::Elite as usize] {
            level_index = VeterancyLevel::Elite as usize;
        }

        // Check if we qualify for Heroic
        if self.current_experience >= experience_required[VeterancyLevel::Heroic as usize] {
            level_index = VeterancyLevel::Heroic as usize;
        }

        self.current_level = match level_index {
            0 => VeterancyLevel::Regular,
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            _ => VeterancyLevel::Regular,
        };
    }

    /// Calculate experience value for killing this object (matches C++ getExperienceValue)
    ///
    /// # Parameters
    /// - `object_cost`: Build cost of this object
    /// - `killer_is_ally`: True if killer is an ally (no XP for team kills)
    ///
    /// # Returns
    /// Experience points to award to the killer
    pub fn get_experience_value(&self, object_cost: i32, killer_is_ally: bool) -> i32 {
        // No experience for killing an ally
        if killer_is_ally {
            return 0;
        }

        // Experience scales with veterancy level and object cost
        // Base formula: object_cost * 0.5 * (1.0 + level * 0.25)
        let level_multiplier = 1.0 + (self.current_level as i32 as f32) * 0.25;
        let base_value = (object_cost as f32 * Self::KILL_XP_RATIO) as i32;
        (base_value as f32 * level_multiplier) as i32
    }

    /// Calculate experience from damage dealt (matches C++ formula)
    ///
    /// # Parameters
    /// - `damage_dealt`: Amount of damage dealt to target
    ///
    /// # Returns
    /// Experience points to award
    pub fn calculate_damage_experience(damage_dealt: f32) -> i32 {
        (damage_dealt * Self::DAMAGE_TO_XP_RATIO) as i32
    }
}

impl Default for ExperienceTracker {
    fn default() -> Self {
        Self::new(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Standard experience requirements for testing
    fn test_experience_requirements() -> [i32; 4] {
        [
            0,   // Regular - no XP required
            100, // Veteran - 100 XP
            300, // Elite - 300 XP
            600, // Heroic - 600 XP
        ]
    }

    #[test]
    fn test_tracker_creation() {
        let tracker = ExperienceTracker::new(123);
        assert_eq!(tracker.owner_id(), 123);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Regular);
        assert_eq!(tracker.get_current_experience(), 0);
        assert_eq!(tracker.get_experience_scalar(), 1.0);
    }

    #[test]
    fn test_add_experience_no_promotion() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        let old_level = tracker.add_experience_points(50, false, &req);
        assert_eq!(old_level, None); // No promotion
        assert_eq!(tracker.get_current_experience(), 50);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Regular);
    }

    #[test]
    fn test_add_experience_with_promotion() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        // Add enough for Veteran
        let old_level = tracker.add_experience_points(150, false, &req);
        assert_eq!(old_level, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_current_experience(), 150);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Veteran);
    }

    #[test]
    fn test_multiple_promotions() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        // Add enough to go straight to Elite
        let old_level = tracker.add_experience_points(400, false, &req);
        assert_eq!(old_level, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_current_experience(), 400);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Elite);
    }

    #[test]
    fn test_experience_scalar() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        tracker.set_experience_scalar(2.0); // Double XP

        // Add 50 XP with scaling
        let _old_level = tracker.add_experience_points(50, true, &req);
        assert_eq!(tracker.get_current_experience(), 100); // 50 * 2.0
    }

    #[test]
    fn test_set_veterancy_level() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        let old_level = tracker.set_veterancy_level_with_requirements(VeterancyLevel::Elite, &req);
        assert_eq!(old_level, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Elite);
        assert_eq!(tracker.get_current_experience(), 300); // Minimum for Elite
    }

    #[test]
    fn test_set_veterancy_level_default() {
        let mut tracker = ExperienceTracker::new(123);

        let old_level = tracker.set_veterancy_level(VeterancyLevel::Elite);
        assert_eq!(old_level, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Elite);
        assert_eq!(tracker.get_current_experience(), 300); // Minimum for Elite with defaults
    }

    #[test]
    fn test_set_min_veterancy_level() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        // Set to Veteran
        tracker.set_min_veterancy_level(VeterancyLevel::Veteran, &req);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Veteran);

        // Try to set to Regular - should have no effect
        let old_level = tracker.set_min_veterancy_level(VeterancyLevel::Regular, &req);
        assert_eq!(old_level, None);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Veteran);
    }

    #[test]
    fn test_gain_exp_for_level() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        // Gain 2 levels
        let gained = tracker.gain_exp_for_level(2, false, &req);
        assert!(gained);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Elite);
        assert_eq!(tracker.get_current_experience(), 300); // Exactly at Elite threshold
    }

    #[test]
    fn test_can_gain_exp_for_level() {
        let tracker = ExperienceTracker::new(123);

        assert!(tracker.can_gain_exp_for_level(1)); // Can gain 1 level
        assert!(tracker.can_gain_exp_for_level(3)); // Can gain 3 levels
        assert!(!tracker.can_gain_exp_for_level(4)); // Can't gain 4 levels (only 3 levels available)
    }

    #[test]
    fn test_is_trainable() {
        let mut tracker = ExperienceTracker::new(123);

        // Regular unit can be trained
        assert!(tracker.is_trainable());

        // Promote to Heroic (max level)
        tracker.set_veterancy_level(VeterancyLevel::Heroic);

        // Heroic unit cannot be trained further
        assert!(!tracker.is_trainable());
    }

    #[test]
    fn test_experience_value_calculation() {
        let tracker = ExperienceTracker::new(123);

        // Regular unit worth 1000
        let xp = tracker.get_experience_value(1000, false);
        assert_eq!(xp, 500); // 1000 * 0.5 * 1.0 = 500

        // No XP for killing ally
        let xp = tracker.get_experience_value(1000, true);
        assert_eq!(xp, 0);
    }

    #[test]
    fn test_experience_value_scales_with_level() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();

        // Promote to Veteran
        tracker.set_veterancy_level_with_requirements(VeterancyLevel::Veteran, &req);

        // Veteran unit worth more XP
        let xp = tracker.get_experience_value(1000, false);
        assert_eq!(xp, 625); // 1000 * 0.5 * 1.25 = 625
    }

    #[test]
    fn test_damage_experience_calculation() {
        let xp = ExperienceTracker::calculate_damage_experience(100.0);
        assert_eq!(xp, 10); // 100 * 0.1 = 10
    }

    #[test]
    fn test_experience_sink() {
        let mut tracker = ExperienceTracker::new(123);

        assert!(!tracker.has_experience_sink());

        tracker.set_experience_sink(456);
        assert!(tracker.has_experience_sink());
        assert_eq!(tracker.get_experience_sink(), 456);
    }
}
