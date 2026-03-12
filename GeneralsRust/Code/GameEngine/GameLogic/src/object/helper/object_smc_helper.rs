//! ObjectSMCHelper - Manages Special Model Condition states
//!
//! This helper module manages temporary "Special Model Condition" (SMC) states.
//! SMCs are visual/state flags that affect how objects are rendered and behave,
//! such as:
//!
//! - Special effects being active
//! - Temporary visual overlays
//! - Animation state overrides
//! - Power-up effects
//!
//! Like the repulsor helper, this is a simple timer-based helper that:
//! 1. Sleeps until forcibly awakened
//! 2. When awakened, clears the special model condition states
//! 3. Goes back to sleep
//!
//! The actual SMC states are managed by the object system - this helper
//! just provides the timer/cleanup mechanism.
//!
//! Original C++ Authors: Steven Johnson, Colin Day (September 2002)
//! Rust conversion: 2025

use super::{ObjectHelperInterface, UpdateSleepTime};
use crate::common::*;

/// Module data for ObjectSMCHelper
///
/// No configuration parameters needed for this helper
#[derive(Debug, Clone)]
pub struct ObjectSMCHelperModuleData {}

impl ObjectSMCHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ObjectSMCHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Special Model Condition flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SpecialModelConditionFlags(pub u32);

impl SpecialModelConditionFlags {
    pub const NONE: SpecialModelConditionFlags = SpecialModelConditionFlags(0);

    /// Check if any flags are set
    pub fn is_any_set(&self) -> bool {
        self.0 != 0
    }

    /// Check if a specific flag is set
    pub fn has_flag(&self, flag: u32) -> bool {
        (self.0 & flag) != 0
    }

    /// Set a flag
    pub fn set_flag(&mut self, flag: u32) {
        self.0 |= flag;
    }

    /// Clear a flag
    pub fn clear_flag(&mut self, flag: u32) {
        self.0 &= !flag;
    }

    /// Clear all flags
    pub fn clear_all(&mut self) {
        self.0 = 0;
    }
}

/// ObjectSMCHelper - Manages Special Model Condition states
///
/// This helper clears temporary SMC states when awakened. It's similar to
/// the repulsor helper in that it sleeps until needed, then acts and sleeps again.
#[derive(Debug)]
pub struct ObjectSMCHelper {
    /// Module data
    module_data: ObjectSMCHelperModuleData,

    /// Next wake frame
    wake_frame: u32,

    /// Whether SMC states need to be cleared
    needs_clear: bool,

    /// Current SMC flags (for tracking)
    current_flags: SpecialModelConditionFlags,
}

impl ObjectSMCHelper {
    /// Create a new ObjectSMCHelper
    pub fn new(module_data: ObjectSMCHelperModuleData) -> Self {
        Self {
            module_data,
            wake_frame: u32::MAX, // Sleep forever initially
            needs_clear: false,
            current_flags: SpecialModelConditionFlags::NONE,
        }
    }

    /// Wake the helper to clear SMC states
    ///
    /// This should be called when special model conditions need to be cleared.
    pub fn wake_for_clear(&mut self, current_frame: u32) {
        self.needs_clear = true;
        self.wake_frame = current_frame; // Wake immediately
    }

    /// Check if SMC states need clearing
    pub fn needs_clearing(&self) -> bool {
        self.needs_clear
    }

    /// Mark SMC states as cleared
    pub fn mark_cleared(&mut self) {
        self.needs_clear = false;
        self.current_flags.clear_all();
        self.wake_frame = u32::MAX; // Sleep forever
    }

    /// Set SMC flags (for external tracking)
    pub fn set_flags(&mut self, flags: SpecialModelConditionFlags) {
        self.current_flags = flags;
    }

    /// Get current SMC flags
    pub fn get_flags(&self) -> SpecialModelConditionFlags {
        self.current_flags
    }

    /// Check if any SMC flags are set
    pub fn has_any_flags(&self) -> bool {
        self.current_flags.is_any_set()
    }
}

impl ObjectHelperInterface for ObjectSMCHelper {
    fn update(&mut self, _current_frame: u32) -> UpdateSleepTime {
        // If we get here, clear the SMC states
        self.needs_clear = true;

        // Go back to sleep until forcibly awakened
        UpdateSleepTime::Forever
    }

    fn get_module_name(&self) -> &str {
        "ObjectSMCHelper"
    }

    fn sleep_until(&mut self, wake_frame: u32) {
        self.wake_frame = wake_frame;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_smc_helper_creation() {
        let data = ObjectSMCHelperModuleData::new();
        let helper = ObjectSMCHelper::new(data);

        assert_eq!(helper.wake_frame, u32::MAX);
        assert!(!helper.needs_clear);
        assert!(!helper.has_any_flags());
    }

    #[test]
    fn test_smc_flags() {
        let mut flags = SpecialModelConditionFlags::NONE;
        assert!(!flags.is_any_set());

        flags.set_flag(1);
        assert!(flags.is_any_set());
        assert!(flags.has_flag(1));
        assert!(!flags.has_flag(2));

        flags.set_flag(2);
        assert!(flags.has_flag(1));
        assert!(flags.has_flag(2));

        flags.clear_flag(1);
        assert!(!flags.has_flag(1));
        assert!(flags.has_flag(2));

        flags.clear_all();
        assert!(!flags.is_any_set());
    }

    #[test]
    fn test_wake_for_clear() {
        let data = ObjectSMCHelperModuleData::new();
        let mut helper = ObjectSMCHelper::new(data);

        assert!(!helper.needs_clearing());

        helper.wake_for_clear(100);

        assert!(helper.needs_clearing());
        assert_eq!(helper.wake_frame, 100);
    }

    #[test]
    fn test_mark_cleared() {
        let data = ObjectSMCHelperModuleData::new();
        let mut helper = ObjectSMCHelper::new(data);

        let mut flags = SpecialModelConditionFlags::NONE;
        flags.set_flag(1);
        helper.set_flags(flags);
        helper.wake_for_clear(100);

        assert!(helper.needs_clearing());
        assert!(helper.has_any_flags());

        helper.mark_cleared();

        assert!(!helper.needs_clearing());
        assert!(!helper.has_any_flags());
        assert_eq!(helper.wake_frame, u32::MAX); // Sleep forever
    }

    #[test]
    fn test_set_and_get_flags() {
        let data = ObjectSMCHelperModuleData::new();
        let mut helper = ObjectSMCHelper::new(data);

        let mut flags = SpecialModelConditionFlags::NONE;
        flags.set_flag(1);
        flags.set_flag(4);

        helper.set_flags(flags);

        let retrieved = helper.get_flags();
        assert!(retrieved.has_flag(1));
        assert!(retrieved.has_flag(4));
        assert!(!retrieved.has_flag(2));
        assert!(helper.has_any_flags());
    }

    #[test]
    fn test_update_returns_forever() {
        let data = ObjectSMCHelperModuleData::new();
        let mut helper = ObjectSMCHelper::new(data);

        let result = helper.update(100);

        assert_eq!(result, UpdateSleepTime::Forever);
        assert!(helper.needs_clear); // Should be set by update
    }

    #[test]
    fn test_sleep_until() {
        let data = ObjectSMCHelperModuleData::new();
        let mut helper = ObjectSMCHelper::new(data);

        helper.sleep_until(500);
        assert_eq!(helper.wake_frame, 500);

        helper.sleep_until(1000);
        assert_eq!(helper.wake_frame, 1000);
    }
}
