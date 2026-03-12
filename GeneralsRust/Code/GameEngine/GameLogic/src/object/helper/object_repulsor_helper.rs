//! ObjectRepulsorHelper - Manages repulsor status bit clearing
//!
//! This helper module manages the OBJECT_STATUS_REPULSOR status bit.
//! When an object has the repulsor status (physics-based unit pushing),
//! this helper ensures the status bit is cleared after the repulsion effect.
//!
//! The repulsor effect itself is handled by the physics system - this helper
//! just manages the status bit lifecycle. The helper sleeps until forcibly
//! awakened by the repulsor system, then clears the bit and goes back to sleep.
//!
//! Original C++ Authors: Steven Johnson (December 2002)
//! Rust conversion: 2025

use super::{ObjectHelperInterface, UpdateSleepTime};
use crate::common::*;

/// Module data for ObjectRepulsorHelper
///
/// No configuration parameters needed for this helper
#[derive(Debug, Clone)]
pub struct ObjectRepulsorHelperModuleData {}

impl ObjectRepulsorHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ObjectRepulsorHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// ObjectRepulsorHelper - Manages repulsor status bit
///
/// This is a simple helper that wakes up when the object is given repulsor
/// status, clears the status bit, and goes back to sleep. The actual repulsion
/// physics is handled by other systems.
#[derive(Debug)]
pub struct ObjectRepulsorHelper {
    /// Module data
    module_data: ObjectRepulsorHelperModuleData,

    /// Next wake frame
    wake_frame: u32,

    /// Whether the repulsor status needs to be cleared
    needs_clear: bool,
}

impl ObjectRepulsorHelper {
    /// Create a new ObjectRepulsorHelper
    pub fn new(module_data: ObjectRepulsorHelperModuleData) -> Self {
        Self {
            module_data,
            wake_frame: u32::MAX, // Sleep forever initially
            needs_clear: false,
        }
    }

    /// Wake the helper to clear repulsor status
    ///
    /// This should be called by the repulsor system when it applies
    /// repulsor status to an object.
    pub fn wake_for_clear(&mut self, current_frame: u32) {
        self.needs_clear = true;
        self.wake_frame = current_frame; // Wake immediately
    }

    /// Check if repulsor status needs clearing
    pub fn needs_clearing(&self) -> bool {
        self.needs_clear
    }

    /// Mark repulsor status as cleared
    pub fn mark_cleared(&mut self) {
        self.needs_clear = false;
        self.wake_frame = u32::MAX; // Sleep forever
    }

    /// Check whether the repulsor status should be cleared this frame.
    pub fn should_clear(&self, current_frame: u32) -> bool {
        self.needs_clear && current_frame >= self.wake_frame
    }
}

impl ObjectHelperInterface for ObjectRepulsorHelper {
    fn update(&mut self, _current_frame: u32) -> UpdateSleepTime {
        // If we get here, clear the repulsor status
        // The object system will call mark_cleared() after clearing the bit
        self.needs_clear = true;

        // Go back to sleep until forcibly awakened
        UpdateSleepTime::Forever
    }

    fn get_module_name(&self) -> &str {
        "ObjectRepulsorHelper"
    }

    fn sleep_until(&mut self, wake_frame: u32) {
        self.wake_frame = wake_frame;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_repulsor_helper_creation() {
        let data = ObjectRepulsorHelperModuleData::new();
        let helper = ObjectRepulsorHelper::new(data);

        assert_eq!(helper.wake_frame, u32::MAX);
        assert!(!helper.needs_clear);
    }

    #[test]
    fn test_wake_for_clear() {
        let data = ObjectRepulsorHelperModuleData::new();
        let mut helper = ObjectRepulsorHelper::new(data);

        assert!(!helper.needs_clearing());

        helper.wake_for_clear(100);

        assert!(helper.needs_clearing());
        assert_eq!(helper.wake_frame, 100);
    }

    #[test]
    fn test_mark_cleared() {
        let data = ObjectRepulsorHelperModuleData::new();
        let mut helper = ObjectRepulsorHelper::new(data);

        helper.wake_for_clear(100);
        assert!(helper.needs_clearing());

        helper.mark_cleared();

        assert!(!helper.needs_clearing());
        assert_eq!(helper.wake_frame, u32::MAX); // Sleep forever
    }

    #[test]
    fn test_update_returns_forever() {
        let data = ObjectRepulsorHelperModuleData::new();
        let mut helper = ObjectRepulsorHelper::new(data);

        let result = helper.update(100);

        assert_eq!(result, UpdateSleepTime::Forever);
        assert!(helper.needs_clear); // Should be set by update
    }

    #[test]
    fn test_sleep_until() {
        let data = ObjectRepulsorHelperModuleData::new();
        let mut helper = ObjectRepulsorHelper::new(data);

        helper.sleep_until(500);
        assert_eq!(helper.wake_frame, 500);

        helper.sleep_until(1000);
        assert_eq!(helper.wake_frame, 1000);
    }
}
