//! ExperienceTracker - Core experience tracking (matches C++ ExperienceTracker.h/cpp)
//!
//! This is the lightweight tracker that each Object owns to track its experience
//! points and veterancy level. It matches the C++ implementation exactly.

use crate::common::types::{ObjectID, VeterancyLevel};
use crate::common::{Xfer, XferMode, XferVersion};
use crate::helpers::TheGameLogic;

/// Wave 420: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

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
    /// Explicit IsTrainable override when the owner template is not registered.
    /// `None` looks up the owner template and defaults to false (C++ default).
    trainable_override: Option<bool>,
    /// Test / host-wiring ExperienceValue table when the owner template is absent.
    experience_value_table: Option<[i32; 4]>,
}

impl ExperienceTracker {
    /// Invalid object ID constant
    pub const INVALID_ID: ObjectID = 0;

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
            trainable_override: None,
            experience_value_table: None,
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
    pub fn set_experience_sink(&mut self, sink: ObjectID) {
        self.experience_sink = sink;
    }

    /// Override template `IsTrainable` (tests / host wiring). C++ reads the
    /// owner ThingTemplate; default when lookup fails is FALSE.
    pub fn set_trainable_override(&mut self, trainable: bool) {
        self.trainable_override = Some(trainable);
    }

    /// C++ ThingTemplate::getExperienceValue table when no owner object exists.
    pub fn set_experience_value_table(&mut self, values: [i32; 4]) {
        self.experience_value_table = Some(values);
    }

    /// Get the experience sink ID
    pub fn get_experience_sink(&self) -> ObjectID {
        self.experience_sink
    }

    /// Check if we're redirecting experience to another object
    pub fn has_experience_sink(&self) -> bool {
        self.experience_sink != Self::INVALID_ID
    }

    fn level_from_index(level_index: usize) -> VeterancyLevel {
        match level_index {
            1 => VeterancyLevel::Veteran,
            2 => VeterancyLevel::Elite,
            3 => VeterancyLevel::Heroic,
            _ => VeterancyLevel::Regular,
        }
    }

    fn get_owner_template_experience_required(&self, level_index: usize) -> Option<i32> {
        // Wave 420: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let owner = TheGameLogic::find_object_by_id(self.owner_id)?;
        let owner_guard = owner.read().ok()?;
        Some(
            owner_guard
                .get_template()
                .get_experience_required(level_index),
        )
    }

    fn get_owner_template_experience_value(&self, level_index: usize) -> Option<i32> {
        // Wave 420: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let owner = TheGameLogic::find_object_by_id(self.owner_id)?;
        let owner_guard = owner.read().ok()?;
        Some(owner_guard.get_template().get_experience_value(level_index))
    }

    fn owner_is_trainable(&self) -> Option<bool> {
        // Wave 420: empty dual-world → None.
        if dual_world_registry_unavailable() {
            return None;
        }

        let owner = TheGameLogic::find_object_by_id(self.owner_id)?;
        let owner_guard = owner.read().ok()?;
        Some(owner_guard.get_template().is_trainable())
    }

    fn fallback_experience_required(experience_required: &[i32], level_index: usize) -> i32 {
        experience_required
            .get(level_index)
            .copied()
            .or_else(|| Self::DEFAULT_EXPERIENCE_REQUIRED.get(level_index).copied())
            .unwrap_or(0)
    }

    fn experience_required_for_level(
        &self,
        level_index: usize,
        experience_required: &[i32],
    ) -> i32 {
        self.get_owner_template_experience_required(level_index)
            .unwrap_or_else(|| Self::fallback_experience_required(experience_required, level_index))
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
            self.current_experience =
                self.experience_required_for_level(new_level as usize, experience_required);

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
            self.current_experience =
                self.experience_required_for_level(new_level as usize, experience_required);

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
                            let forwarded_experience_gain =
                                (experience_gain as f32 * self.experience_scalar) as i32;
                            return tracker_guard.add_experience_points(
                                forwarded_experience_gain,
                                can_scale_for_bonus,
                                experience_required,
                            );
                        }
                    }
                }
            }
        }

        if !self.is_trainable() {
            return None;
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

        let max_level = VeterancyLevel::Heroic as i32;
        let mut new_level = (self.current_level as i32) + levels_to_gain;
        if new_level > max_level {
            new_level = max_level;
        }

        if new_level > self.current_level as i32 {
            let experience_needed = self
                .experience_required_for_level(new_level as usize, experience_required)
                - self.current_experience;
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

        let max_level = VeterancyLevel::Heroic as i32;
        let mut new_level = (self.current_level as i32) + levels_to_gain;
        // C++: return true if we can gain levels, even if we can't gain ALL requested.
        if new_level > max_level {
            new_level = max_level;
        }
        new_level > self.current_level as i32
    }

    /// Check if this object can be trained (gain at least one level)
    ///
    /// This is a convenience method that checks if the object can gain one level.
    /// Returns true if the object is not yet at max veterancy level.
    pub fn is_trainable(&self) -> bool {
        if let Some(override_trainable) = self.trainable_override {
            return override_trainable;
        }
        self.owner_is_trainable().unwrap_or(false)
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
        if !self.is_trainable() {
            return None;
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
        let mut level_index: usize = 0;
        let max_level = VeterancyLevel::Heroic as usize;

        while (level_index + 1) <= max_level
            && self.current_experience
                >= self.experience_required_for_level(level_index + 1, experience_required)
        {
            level_index += 1;
        }

        self.current_level = Self::level_from_index(level_index);
    }

    /// Calculate experience value for killing this object (matches C++ getExperienceValue)
    ///
    /// # Parameters
    /// - `object_cost`: Build cost of this object
    /// - `killer_is_ally`: True if killer is an ally (no XP for team kills)
    ///
    /// # Returns
    /// Experience points to award to the killer
    pub fn get_experience_value(&self, _object_cost: i32, killer_is_ally: bool) -> i32 {
        // No experience for killing an ally
        if killer_is_ally {
            return 0;
        }

        // C++ ExperienceTracker::getExperienceValue — template table only.
        self.get_owner_template_experience_value(self.current_level as usize)
            .or_else(|| {
                self.experience_value_table
                    .and_then(|table| table.get(self.current_level as usize).copied())
            })
            .unwrap_or(0)
    }

    /// C++ ExperienceTracker has no damage/hit XP API.
    pub fn calculate_damage_experience(_damage_dealt: f32) -> i32 {
        0
    }

    /// Serialize tracker state for save/load parity with C++ ExperienceTracker::xfer.
    pub fn xfer_state(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)
            .map_err(|e| e.to_string())?;

        // C++ parity: xferUser(&m_currentLevel, sizeof(VeterancyLevel))
        let mut current_level = self.current_level as i32;
        unsafe {
            xfer.xfer_user(
                (&mut current_level as *mut i32).cast::<u8>(),
                std::mem::size_of::<VeterancyLevel>(),
            )
        }
        .map_err(|e| e.to_string())?;
        if xfer.get_xfer_mode() == XferMode::Load {
            self.current_level = match current_level {
                1 => VeterancyLevel::Veteran,
                2 => VeterancyLevel::Elite,
                3 => VeterancyLevel::Heroic,
                _ => VeterancyLevel::Regular,
            };
        }

        xfer.xfer_int(&mut self.current_experience)
            .map_err(|e| e.to_string())?;
        xfer.xfer_object_id(&mut self.experience_sink)
            .map_err(|e| e.to_string())?;
        xfer.xfer_real(&mut self.experience_scalar)
            .map_err(|e| e.to_string())?;

        Ok(())
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
    fn trainable_tracker(id: ObjectID) -> ExperienceTracker {
        let mut tracker = ExperienceTracker::new(id);
        tracker.set_trainable_override(true);
        tracker
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
        let mut tracker = trainable_tracker(123);
        let req = test_experience_requirements();

        let old_level = tracker.add_experience_points(50, false, &req);
        assert_eq!(old_level, None); // No promotion
        assert_eq!(tracker.get_current_experience(), 50);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Regular);
    }

    #[test]
    fn test_add_experience_with_promotion() {
        let mut tracker = trainable_tracker(123);
        let req = test_experience_requirements();

        // Add enough for Veteran
        let old_level = tracker.add_experience_points(150, false, &req);
        assert_eq!(old_level, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_current_experience(), 150);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Veteran);
    }

    #[test]
    fn test_multiple_promotions() {
        let mut tracker = trainable_tracker(123);
        let req = test_experience_requirements();

        // Add enough to go straight to Elite
        let old_level = tracker.add_experience_points(400, false, &req);
        assert_eq!(old_level, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_current_experience(), 400);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Elite);
    }

    #[test]
    fn test_experience_scalar() {
        let mut tracker = trainable_tracker(123);
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
        let mut tracker = trainable_tracker(123);
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
        // C++ clamps newLevel to LEVEL_LAST, so Regular+4 still reaches Heroic.
        assert!(tracker.can_gain_exp_for_level(4));

        let mut elite = ExperienceTracker::new(124);
        elite.set_veterancy_level(VeterancyLevel::Elite);
        // Elite + 2 clamps to Heroic and is still a gain.
        assert!(elite.can_gain_exp_for_level(2));

        let mut heroic = ExperienceTracker::new(125);
        heroic.set_veterancy_level(VeterancyLevel::Heroic);
        assert!(!heroic.can_gain_exp_for_level(1));
    }

    #[test]
    fn test_is_trainable() {
        let mut tracker = ExperienceTracker::new(123);
        // C++ default: ThingTemplate::isTrainable is FALSE when owner lookup fails.
        assert!(!tracker.is_trainable());

        tracker.set_trainable_override(true);
        assert!(tracker.is_trainable());

        // IsTrainable is a template flag, not "can still level".
        tracker.set_veterancy_level(VeterancyLevel::Heroic);
        assert!(tracker.is_trainable());

        tracker.set_trainable_override(false);
        assert!(!tracker.is_trainable());
    }

    #[test]
    fn test_set_experience_and_level_requires_trainable() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();
        assert_eq!(tracker.set_experience_and_level(400, &req), None);
        assert_eq!(tracker.get_current_experience(), 0);
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Regular);

        tracker.set_trainable_override(true);
        let old = tracker.set_experience_and_level(400, &req);
        assert_eq!(old, Some(VeterancyLevel::Regular));
        assert_eq!(tracker.get_veterancy_level(), VeterancyLevel::Elite);
    }

    #[test]
    fn test_experience_value_calculation() {
        let tracker = ExperienceTracker::new(123);

        // No owner template and no table → 0 (C++ uses ThingTemplate only).
        let xp = tracker.get_experience_value(1000, false);
        assert_eq!(xp, 0);

        // No XP for killing ally
        let xp = tracker.get_experience_value(1000, true);
        assert_eq!(xp, 0);
    }

    #[test]
    fn test_experience_value_scales_with_level() {
        let mut tracker = ExperienceTracker::new(123);
        let req = test_experience_requirements();
        tracker.set_experience_value_table([20, 40, 80, 120]);

        tracker.set_veterancy_level_with_requirements(VeterancyLevel::Veteran, &req);

        let xp = tracker.get_experience_value(1000, false);
        assert_eq!(xp, 40);
    }

    #[test]
    fn test_damage_experience_calculation() {
        let xp = ExperienceTracker::calculate_damage_experience(100.0);
        assert_eq!(xp, 0);
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
