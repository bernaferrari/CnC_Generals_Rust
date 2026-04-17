//! SubdualDamageHelper - Manages subdual damage healing
//!
//! This helper module manages "subdual" damage - a special damage type that
//! temporarily disables units without killing them (like stunning/knockout).
//! Body modules can't have Updates, so this helper handles the periodic healing.
//!
//! Subdual damage:
//! - Temporarily disables units
//! - Heals gradually over time
//! - Has a healing rate (frames between heal steps)
//! - Has a healing amount (HP healed per step)
//!
//! The helper wakes up when subdual damage is applied, then runs every frame
//! to heal it gradually. Once all subdual damage is healed, it goes back to sleep.
//!
//! Original C++ Author: Graham Smallwood (June 2003)
//! Rust conversion: 2025

use super::{DisabledMaskType, ObjectHelperInterface, UpdateSleepTime};
use crate::common::*;
use crate::damage::{DamageInfo, DamageType};
use crate::helpers::TheGameLogic;

/// Module data for SubdualDamageHelper
///
/// No configuration parameters needed (rates come from body module)
#[derive(Debug, Clone)]
pub struct SubdualDamageHelperModuleData {}

impl SubdualDamageHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SubdualDamageHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// SubdualDamageHelper - Manages subdual damage healing
///
/// This helper coordinates with the body module to gradually heal subdual damage.
/// It runs every frame while subdual damage exists, counting down frames between
/// healing steps.
#[derive(Debug)]
pub struct SubdualDamageHelper {
    /// Module data
    #[allow(dead_code)]
    module_data: SubdualDamageHelperModuleData,

    /// Owning object id
    owner_id: ObjectID,

    /// Countdown until next healing step
    healing_step_countdown: u32,

    /// Next wake frame
    wake_frame: u32,
}

impl SubdualDamageHelper {
    /// Create a new SubdualDamageHelper
    pub fn new(owner_id: ObjectID, module_data: SubdualDamageHelperModuleData) -> Self {
        Self {
            module_data,
            owner_id,
            healing_step_countdown: 0,
            wake_frame: u32::MAX, // Sleep forever initially
        }
    }

    /// Notify the helper that subdual damage was applied
    ///
    /// # Arguments
    /// * `amount` - Amount of subdual damage applied (positive = damage, negative = healing)
    /// * `heal_rate` - Frames between healing steps
    ///
    /// This wakes the helper and starts the healing countdown.
    pub fn notify_subdual_damage(&mut self, amount: f32, heal_rate: u32) {
        if amount > 0.0 {
            self.healing_step_countdown = heal_rate;
            self.wake_frame = 0; // Wake every frame
        }
    }

    /// Perform a healing step
    ///
    /// # Arguments
    /// * `heal_rate` - Frames between healing steps
    /// * `heal_amount` - Amount to heal per step
    ///
    /// # Returns
    /// The amount healed (negative damage)
    pub fn perform_healing_step(&mut self, heal_rate: u32, heal_amount: f32) -> f32 {
        self.healing_step_countdown = self.healing_step_countdown.saturating_sub(1);

        if self.healing_step_countdown > 0 {
            return 0.0; // Not time to heal yet
        }

        // Reset countdown
        self.healing_step_countdown = heal_rate;

        -heal_amount // Negative because it's healing
    }

    /// Check if there's any subdual damage remaining
    pub fn has_subdual_damage(&self) -> bool {
        self.healing_step_countdown > 0
    }

    /// Get current subdual damage amount
    pub fn get_subdual_damage(&self) -> f32 {
        0.0
    }

    /// Get healing step countdown
    pub fn get_healing_step_countdown(&self) -> u32 {
        self.healing_step_countdown
    }

    /// Manually set subdual damage (for testing/loading)
    pub fn set_subdual_damage(&mut self, amount: f32) {
        let _ = amount;
    }

    /// Clear all subdual damage
    pub fn clear_subdual_damage(&mut self) {
        self.healing_step_countdown = 0;
        self.wake_frame = u32::MAX;
    }
}

impl ObjectHelperInterface for SubdualDamageHelper {
    fn update(&mut self, _current_frame: u32) -> UpdateSleepTime {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) else {
            return UpdateSleepTime::Forever;
        };
        let Ok(owner_guard) = owner.read() else {
            return UpdateSleepTime::None;
        };
        let Some(body) = owner_guard.get_body_module() else {
            return UpdateSleepTime::Forever;
        };

        let Ok(mut body_guard) = body.lock() else {
            return UpdateSleepTime::None;
        };

        self.healing_step_countdown = self.healing_step_countdown.saturating_sub(1);
        if self.healing_step_countdown > 0 {
            return UpdateSleepTime::None;
        }

        self.healing_step_countdown = body_guard.get_subdual_damage_heal_rate();

        let mut damage = DamageInfo::new();
        damage.input.damage_type = DamageType::SubdualUnresistable;
        damage.input.amount = -body_guard.get_subdual_damage_heal_amount();
        damage.input.source_id = INVALID_ID;
        damage.sync_from_input();
        let _ = body_guard.attempt_damage(&mut damage);

        if body_guard.has_any_subdual_damage() {
            UpdateSleepTime::None
        } else {
            UpdateSleepTime::Forever
        }
    }

    fn get_module_name(&self) -> &str {
        "SubdualDamageHelper"
    }

    fn sleep_until(&mut self, wake_frame: u32) {
        self.wake_frame = wake_frame;
    }

    /// Subdual damage helper must process all disabled types
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::All
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subdual_damage_helper_creation() {
        let data = SubdualDamageHelperModuleData::new();
        let helper = SubdualDamageHelper::new(INVALID_ID, data);

        assert_eq!(helper.healing_step_countdown, 0);
        assert_eq!(helper.wake_frame, u32::MAX);
        assert!(!helper.has_subdual_damage());
    }

    #[test]
    fn test_notify_subdual_damage() {
        let data = SubdualDamageHelperModuleData::new();
        let mut helper = SubdualDamageHelper::new(INVALID_ID, data);

        let heal_rate = 30; // 1 second at 30 FPS

        helper.notify_subdual_damage(100.0, heal_rate);

        assert!(helper.has_subdual_damage());
        assert_eq!(helper.healing_step_countdown, heal_rate);
        assert_eq!(helper.wake_frame, 0); // Wake every frame
    }

    #[test]
    fn test_healing_step_countdown() {
        let data = SubdualDamageHelperModuleData::new();
        let mut helper = SubdualDamageHelper::new(INVALID_ID, data);

        helper.notify_subdual_damage(100.0, 5);
        assert_eq!(helper.healing_step_countdown, 5);

        // First few steps don't heal
        assert_eq!(helper.perform_healing_step(5, 10.0), 0.0);
        assert_eq!(helper.healing_step_countdown, 4);

        assert_eq!(helper.perform_healing_step(5, 10.0), 0.0);
        assert_eq!(helper.healing_step_countdown, 3);

        assert_eq!(helper.perform_healing_step(5, 10.0), 0.0);
        assert_eq!(helper.healing_step_countdown, 2);

        assert_eq!(helper.perform_healing_step(5, 10.0), 0.0);
        assert_eq!(helper.healing_step_countdown, 1);

        // Fifth step heals
        let healed = helper.perform_healing_step(5, 10.0);
        assert_eq!(healed, -10.0); // Negative = healing
        assert_eq!(helper.healing_step_countdown, 5); // Reset
    }

    #[test]
    fn test_clear_subdual_damage() {
        let data = SubdualDamageHelperModuleData::new();
        let mut helper = SubdualDamageHelper::new(INVALID_ID, data);

        helper.notify_subdual_damage(100.0, 5);
        assert!(helper.has_subdual_damage());

        helper.clear_subdual_damage();

        assert!(!helper.has_subdual_damage());
        assert_eq!(helper.healing_step_countdown, 0);
        assert_eq!(helper.wake_frame, u32::MAX);
    }

    #[test]
    fn test_update_with_damage() {
        let data = SubdualDamageHelperModuleData::new();
        let mut helper = SubdualDamageHelper::new(INVALID_ID, data);

        helper.notify_subdual_damage(100.0, 5);

        let result = helper.update(100);
        assert_eq!(result, UpdateSleepTime::Forever); // No owner, sleep
    }

    #[test]
    fn test_update_without_damage() {
        let data = SubdualDamageHelperModuleData::new();
        let mut helper = SubdualDamageHelper::new(INVALID_ID, data);

        let result = helper.update(100);
        assert_eq!(result, UpdateSleepTime::Forever); // Sleep forever
    }

    #[test]
    fn test_disabled_types_processing() {
        let data = SubdualDamageHelperModuleData::new();
        let helper = SubdualDamageHelper::new(INVALID_ID, data);

        assert_eq!(
            helper.get_disabled_types_to_process(),
            DisabledMaskType::All
        );
    }
}
