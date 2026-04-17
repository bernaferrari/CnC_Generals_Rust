//! TempWeaponBonusHelper - Manages temporary weapon bonus effects
//!
//! This helper module manages temporary weapon bonuses applied to objects,
//! such as:
//!
//! - Frenzy (increased damage/rate of fire)
//! - Armor-piercing rounds
//! - Enhanced accuracy
//! - Other temporary combat bonuses
//!
//! When a bonus is applied:
//! 1. Helper wakes up and sets the bonus condition
//! 2. Visual effects are applied (tint, animations, etc.)
//! 3. Timer starts counting down
//! 4. When timer expires, bonus is cleared
//! 5. Visual effects are removed
//!
//! Re-applying the same bonus resets the timer. Applying a different bonus
//! clears the old one first.
//!
//! Original C++ Author: Graham Smallwood (June 2003)
//! Rust conversion: 2025

use super::{DisabledMaskType, ObjectHelperInterface, UpdateSleepTime};
use crate::common::*;
use crate::object::drawable::TintStatus;

/// Module data for TempWeaponBonusHelper
///
/// No configuration parameters needed for this helper
#[derive(Debug, Clone)]
pub struct TempWeaponBonusHelperModuleData {}

impl TempWeaponBonusHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for TempWeaponBonusHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// TempWeaponBonusHelper - Manages temporary weapon bonuses
///
/// This helper is sleep-driven. It wakes when a bonus needs to be cleared,
/// clears it, and goes back to sleep.
#[derive(Debug)]
pub struct TempWeaponBonusHelper {
    /// Module data
    #[allow(dead_code)]
    module_data: TempWeaponBonusHelperModuleData,

    /// Owning object id
    owner_id: ObjectID,

    /// The current weapon bonus condition
    current_bonus: WeaponBonusConditionType,

    /// Frame when the bonus should be removed
    frame_to_remove: u32,

    /// Next wake frame
    wake_frame: u32,

    /// Current tint status (for tracking visual effects)
    current_tint: TintStatus,
}

impl TempWeaponBonusHelper {
    /// Create a new TempWeaponBonusHelper
    pub fn new(owner_id: ObjectID, module_data: TempWeaponBonusHelperModuleData) -> Self {
        Self {
            module_data,
            owner_id,
            current_bonus: WeaponBonusConditionType::Invalid,
            frame_to_remove: 0,
            wake_frame: u32::MAX, // Sleep forever initially
            current_tint: TintStatus::NONE,
        }
    }

    /// Apply a temporary weapon bonus
    ///
    /// # Arguments
    /// * `bonus` - The bonus type to apply
    /// * `duration` - Duration in frames
    /// * `current_frame` - Current game frame
    ///
    /// # Returns
    /// The bonus that was cleared (if different from the new bonus)
    pub fn do_temp_weapon_bonus(
        &mut self,
        bonus: WeaponBonusConditionType,
        duration: u32,
        current_frame: u32,
    ) -> Option<WeaponBonusConditionType> {
        // Clear any different bonus we may have
        let cleared_bonus = if self.current_bonus != bonus
            && self.current_bonus != WeaponBonusConditionType::Invalid
        {
            let old = self.current_bonus;
            self.clear_temp_weapon_bonus();
            Some(old)
        } else {
            None
        };

        // Set the new bonus (or reset timer for same bonus)
        self.current_bonus = bonus;
        self.frame_to_remove = current_frame + duration;

        if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.set_weapon_bonus_condition(bonus);
                if let Some(drawable) = owner_guard.get_drawable() {
                    if let Ok(mut draw_guard) = drawable.write() {
                        draw_guard.set_tint_status(TintStatus::FRENZY);
                    }
                }
            }
        }

        // Apply visual effects
        match bonus {
            WeaponBonusConditionType::FrenzyOne
            | WeaponBonusConditionType::FrenzyTwo
            | WeaponBonusConditionType::FrenzyThree => {
                self.current_tint = TintStatus::FRENZY;
            }
            _ => {
                // Other bonuses may have different visual effects
                self.current_tint = TintStatus::NONE;
            }
        }

        // Wake up when it's time to remove the bonus
        self.wake_frame = self.frame_to_remove;

        cleared_bonus
    }

    /// Clear the current weapon bonus
    pub fn clear_temp_weapon_bonus(&mut self) -> Option<WeaponBonusConditionType> {
        if self.current_bonus != WeaponBonusConditionType::Invalid {
            let cleared = self.current_bonus;

            self.current_bonus = WeaponBonusConditionType::Invalid;
            self.frame_to_remove = 0;
            self.current_tint = TintStatus::NONE;
            self.wake_frame = u32::MAX; // Sleep forever

            if let Some(owner) = crate::helpers::TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.clear_weapon_bonus_condition(cleared);
                    if let Some(drawable) = owner_guard.get_drawable() {
                        if let Ok(mut draw_guard) = drawable.write() {
                            draw_guard.clear_tint_status(TintStatus::FRENZY);
                        }
                    }
                }
            }

            Some(cleared)
        } else {
            None
        }
    }

    /// Get the current bonus being tracked
    pub fn get_current_bonus(&self) -> WeaponBonusConditionType {
        self.current_bonus
    }

    /// Get the frame when bonus should be removed
    pub fn get_frame_to_remove(&self) -> u32 {
        self.frame_to_remove
    }

    /// Check if a bonus is currently active
    pub fn has_active_bonus(&self) -> bool {
        self.current_bonus != WeaponBonusConditionType::Invalid
    }

    /// Get time remaining until bonus expires
    pub fn get_time_remaining(&self, current_frame: u32) -> u32 {
        if current_frame >= self.frame_to_remove {
            0
        } else {
            self.frame_to_remove - current_frame
        }
    }

    /// Get current tint status
    pub fn get_current_tint(&self) -> TintStatus {
        self.current_tint
    }
}

impl ObjectHelperInterface for TempWeaponBonusHelper {
    fn update(&mut self, current_frame: u32) -> UpdateSleepTime {
        // We are sleep-driven, so seeing an update means our timer is ready
        debug_assert!(
            self.frame_to_remove <= current_frame,
            "TempWeaponBonusHelper woke up too soon"
        );

        // Clear the weapon bonus
        self.clear_temp_weapon_bonus();

        // Sleep forever until next bonus is applied
        UpdateSleepTime::Forever
    }

    fn get_module_name(&self) -> &str {
        "TempWeaponBonusHelper"
    }

    fn sleep_until(&mut self, wake_frame: u32) {
        self.wake_frame = wake_frame;
    }

    /// Temp weapon bonus helper must process all disabled types
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::All
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_temp_weapon_bonus_helper_creation() {
        let data = TempWeaponBonusHelperModuleData::new();
        let helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        assert_eq!(helper.current_bonus, WeaponBonusConditionType::Invalid);
        assert_eq!(helper.frame_to_remove, 0);
        assert_eq!(helper.wake_frame, u32::MAX);
        assert_eq!(helper.current_tint, TintStatus::NONE);
        assert!(!helper.has_active_bonus());
    }

    #[test]
    fn test_apply_weapon_bonus() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        let current_frame = 100;
        let duration = 150; // 5 seconds at 30 FPS

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, duration, current_frame);

        assert_eq!(helper.current_bonus, WeaponBonusConditionType::FrenzyOne);
        assert_eq!(helper.frame_to_remove, current_frame + duration);
        assert_eq!(helper.wake_frame, current_frame + duration);
        assert_eq!(helper.current_tint, TintStatus::FRENZY);
        assert!(helper.has_active_bonus());
    }

    #[test]
    fn test_clear_weapon_bonus() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 150, 100);
        assert!(helper.has_active_bonus());

        let cleared = helper.clear_temp_weapon_bonus();

        assert_eq!(cleared, Some(WeaponBonusConditionType::FrenzyOne));
        assert!(!helper.has_active_bonus());
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::Invalid);
        assert_eq!(helper.frame_to_remove, 0);
        assert_eq!(helper.current_tint, TintStatus::NONE);
        assert_eq!(helper.wake_frame, u32::MAX);
    }

    #[test]
    fn test_reapply_same_bonus() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        // Apply frenzy for 150 frames
        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 150, 100);
        assert_eq!(helper.frame_to_remove, 250);

        // Reapply frenzy for 90 frames - should reset timer
        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 90, 120);
        assert_eq!(helper.frame_to_remove, 210); // 120 + 90
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::FrenzyOne);
        assert_eq!(helper.current_tint, TintStatus::FRENZY);
    }

    #[test]
    fn test_apply_different_bonus() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        // Apply frenzy
        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 150, 100);
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::FrenzyOne);

        // Apply fanaticism - should clear frenzy
        let cleared = helper.do_temp_weapon_bonus(WeaponBonusConditionType::Fanaticism, 90, 120);

        assert_eq!(cleared, Some(WeaponBonusConditionType::FrenzyOne));
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::Fanaticism);
        assert_eq!(helper.frame_to_remove, 210);
    }

    #[test]
    fn test_time_remaining() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 300, 100);

        assert_eq!(helper.get_time_remaining(100), 300);
        assert_eq!(helper.get_time_remaining(250), 150);
        assert_eq!(helper.get_time_remaining(400), 0);
        assert_eq!(helper.get_time_remaining(500), 0);
    }

    #[test]
    fn test_update_clears_bonus() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 150, 100);

        let result = helper.update(250);

        assert_eq!(result, UpdateSleepTime::Forever);
        assert!(!helper.has_active_bonus());
        assert_eq!(helper.current_tint, TintStatus::NONE);
    }

    #[test]
    fn test_disabled_types_processing() {
        let data = TempWeaponBonusHelperModuleData::new();
        let helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        assert_eq!(
            helper.get_disabled_types_to_process(),
            DisabledMaskType::All
        );
    }

    #[test]
    fn test_multiple_bonus_types() {
        let data = TempWeaponBonusHelperModuleData::new();
        let mut helper = TempWeaponBonusHelper::new(INVALID_ID, data);

        // Test different bonus types
        helper.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyOne, 100, 0);
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::FrenzyOne);

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::Fanaticism, 100, 100);
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::Fanaticism);

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::TargetFaerieFire, 100, 200);
        assert_eq!(
            helper.current_bonus,
            WeaponBonusConditionType::TargetFaerieFire
        );

        helper.do_temp_weapon_bonus(WeaponBonusConditionType::SoloAiHard, 100, 300);
        assert_eq!(helper.current_bonus, WeaponBonusConditionType::SoloAiHard);
    }
}
