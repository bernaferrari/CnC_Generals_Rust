//! ObjectWeaponStatusHelper - Updates model conditions based on weapon status
//!
//! This helper module runs every frame to update an object's model condition
//! based on its weapon status. It adjusts visual states like:
//!
//! - RELOADING - Weapon is reloading
//! - BETWEEN_FIRING_SHOTS - In the middle of a burst
//! - WEAPON_READY - Weapon is ready to fire
//! - WEAPON_LOCKED - Weapon is locked/disabled
//!
//! Unlike other helpers, this one:
//! - Runs EVERY frame (UPDATE_SLEEP_NONE)
//! - Runs in the FINAL update phase (after all normal updates)
//! - Must be on objects that have weapons
//!
//! This ensures weapon visuals stay synchronized with weapon state,
//! allowing smooth animations for reloading, firing, etc.
//!
//! Original C++ Authors: Steven Johnson, Colin Day (September 2002)
//! Rust conversion: 2025

use super::{ObjectHelperInterface, SleepyUpdatePhase, UpdateSleepTime};
use crate::common::*;

/// Module data for ObjectWeaponStatusHelper
///
/// No configuration parameters needed for this helper
#[derive(Debug, Clone)]
pub struct ObjectWeaponStatusHelperModuleData {}

impl ObjectWeaponStatusHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ObjectWeaponStatusHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Weapon status states for model conditions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WeaponStatus {
    /// Weapon is ready to fire
    Ready,
    /// Weapon is reloading
    Reloading,
    /// Weapon is between shots in a burst
    BetweenShots,
    /// Weapon is locked/disabled
    Locked,
    /// No weapon or unknown state
    None,
}

/// ObjectWeaponStatusHelper - Updates model conditions for weapon status
///
/// Unlike other helpers, this runs every frame in the final update phase
/// to keep weapon visuals synchronized with weapon state.
#[derive(Debug)]
pub struct ObjectWeaponStatusHelper {
    /// Module data
    #[allow(dead_code)]
    module_data: ObjectWeaponStatusHelperModuleData,

    /// Current weapon status (for tracking changes)
    current_status: WeaponStatus,

    /// Whether object has any weapons
    has_weapons: bool,
}

impl ObjectWeaponStatusHelper {
    /// Create a new ObjectWeaponStatusHelper
    ///
    /// # Arguments
    /// * `module_data` - Module configuration
    /// * `has_weapons` - Whether the object has any weapons
    ///
    /// # Panics
    /// In debug builds, panics if has_weapons is false (should not instantiate
    /// this helper on objects without weapons)
    pub fn new(module_data: ObjectWeaponStatusHelperModuleData, has_weapons: bool) -> Self {
        debug_assert!(
            has_weapons,
            "ObjectWeaponStatusHelper should not be instantiated if object has no weapons"
        );

        Self {
            module_data,
            current_status: WeaponStatus::None,
            has_weapons,
        }
    }

    /// Get the current weapon status
    pub fn get_current_status(&self) -> WeaponStatus {
        self.current_status
    }

    /// Update the weapon status
    ///
    /// This would normally query the weapon system to determine the current
    /// status, then update model conditions accordingly.
    ///
    /// Returns true if the status changed.
    pub fn update_weapon_status(&mut self, new_status: WeaponStatus) -> bool {
        if self.current_status != new_status {
            self.current_status = new_status;
            true
        } else {
            false
        }
    }

    /// Get the update phase for this helper
    ///
    /// Weapon status helper runs in the FINAL phase, after all normal updates.
    pub fn get_update_phase(&self) -> SleepyUpdatePhase {
        SleepyUpdatePhase::Final
    }

    /// Check if object has weapons
    pub fn has_weapons(&self) -> bool {
        self.has_weapons
    }
}

impl ObjectHelperInterface for ObjectWeaponStatusHelper {
    fn update(&mut self, _current_frame: u32) -> UpdateSleepTime {
        // This helper must run every frame to keep weapon visuals synchronized
        // The actual model condition adjustment would happen here, calling:
        // getObject()->adjustModelConditionForWeaponStatus();

        // Unlike other helpers, this one NEVER sleeps
        UpdateSleepTime::None
    }

    fn get_module_name(&self) -> &str {
        "ObjectWeaponStatusHelper"
    }

    fn sleep_until(&mut self, _wake_frame: u32) {
        // This helper never sleeps, so we ignore sleep requests
        // It must run every frame
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_weapon_status_helper_creation() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let helper = ObjectWeaponStatusHelper::new(data, true);

        assert_eq!(helper.current_status, WeaponStatus::None);
        assert!(helper.has_weapons);
        assert_eq!(helper.get_update_phase(), SleepyUpdatePhase::Final);
    }

    #[test]
    #[should_panic(expected = "should not be instantiated if object has no weapons")]
    #[cfg(debug_assertions)]
    fn test_creation_without_weapons_panics() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let _helper = ObjectWeaponStatusHelper::new(data, false);
    }

    #[test]
    fn test_update_weapon_status() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let mut helper = ObjectWeaponStatusHelper::new(data, true);

        // Initial status
        assert_eq!(helper.get_current_status(), WeaponStatus::None);

        // Change to ready - should return true
        assert!(helper.update_weapon_status(WeaponStatus::Ready));
        assert_eq!(helper.get_current_status(), WeaponStatus::Ready);

        // Same status - should return false
        assert!(!helper.update_weapon_status(WeaponStatus::Ready));
        assert_eq!(helper.get_current_status(), WeaponStatus::Ready);

        // Change to reloading - should return true
        assert!(helper.update_weapon_status(WeaponStatus::Reloading));
        assert_eq!(helper.get_current_status(), WeaponStatus::Reloading);
    }

    #[test]
    fn test_weapon_status_transitions() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let mut helper = ObjectWeaponStatusHelper::new(data, true);

        // Test typical firing sequence
        helper.update_weapon_status(WeaponStatus::Ready);
        helper.update_weapon_status(WeaponStatus::BetweenShots);
        helper.update_weapon_status(WeaponStatus::Reloading);
        helper.update_weapon_status(WeaponStatus::Ready);

        assert_eq!(helper.get_current_status(), WeaponStatus::Ready);
    }

    #[test]
    fn test_update_returns_none() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let mut helper = ObjectWeaponStatusHelper::new(data, true);

        // This helper always returns None (never sleeps)
        let result = helper.update(100);
        assert_eq!(result, UpdateSleepTime::None);

        let result = helper.update(200);
        assert_eq!(result, UpdateSleepTime::None);
    }

    #[test]
    fn test_sleep_until_ignored() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let mut helper = ObjectWeaponStatusHelper::new(data, true);

        // Sleep requests are ignored - helper must run every frame
        helper.sleep_until(1000);

        // Still returns None (never sleeps)
        let result = helper.update(500);
        assert_eq!(result, UpdateSleepTime::None);
    }

    #[test]
    fn test_weapon_status_enum() {
        // Test weapon status comparisons
        assert_eq!(WeaponStatus::Ready, WeaponStatus::Ready);
        assert_ne!(WeaponStatus::Ready, WeaponStatus::Reloading);
        assert_ne!(WeaponStatus::Reloading, WeaponStatus::BetweenShots);
        assert_ne!(WeaponStatus::BetweenShots, WeaponStatus::Locked);
        assert_ne!(WeaponStatus::Locked, WeaponStatus::None);
    }

    #[test]
    fn test_update_phase() {
        let data = ObjectWeaponStatusHelperModuleData::new();
        let helper = ObjectWeaponStatusHelper::new(data, true);

        // Weapon status helper runs in FINAL phase
        assert_eq!(helper.get_update_phase(), SleepyUpdatePhase::Final);

        // Verify phase ordering
        assert!(SleepyUpdatePhase::Final > SleepyUpdatePhase::Normal);
    }
}
