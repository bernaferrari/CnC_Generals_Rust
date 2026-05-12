//! StatusDamageHelper - Clears status conditions on a timer
//!
//! This helper module manages temporary status conditions that are applied
//! to objects for a duration (e.g., stunned, slowed, etc.). It:
//!
//! - Tracks which status condition to clear
//! - Maintains a timer for when to clear it
//! - Automatically clears the status when the timer expires
//! - Handles re-application of the same status (resets timer)
//! - Handles different status types (clears old one, applies new one)
//!
//! This is used for various status effects like:
//! - Stunned (from EMP)
//! - Slowed (from toxins)
//! - Confused
//! - etc.
//!
//! Original C++ Author: Graham Smallwood (June 2003)
//! Rust conversion: 2025

use super::{DisabledMaskType, ObjectHelperInterface, UpdateSleepTime};
use crate::common::*;
use crate::helpers::TheGameLogic;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Object status types that can be temporarily applied
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
/// Module data for StatusDamageHelper
///
/// No configuration parameters needed for this helper
pub struct StatusDamageHelperModuleData {}

impl StatusDamageHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for StatusDamageHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// StatusDamageHelper - Clears status conditions on a timer
///
/// This helper is sleep-driven. It wakes up only when a status needs to be
/// cleared, clears it, and goes back to sleep.
#[derive(Debug)]
pub struct StatusDamageHelper {
    /// Module data
    #[allow(dead_code)]
    module_data: StatusDamageHelperModuleData,

    /// Owning object id
    owner_id: ObjectID,

    /// The status condition to heal/clear
    status_to_heal: ObjectStatusTypes,

    /// Frame when the status should be cleared
    frame_to_heal: u32,

    /// Next wake frame
    wake_frame: u32,
}

impl StatusDamageHelper {
    /// Create a new StatusDamageHelper
    pub fn new(owner_id: ObjectID, module_data: StatusDamageHelperModuleData) -> Self {
        Self {
            module_data,
            owner_id,
            status_to_heal: ObjectStatusTypes::None,
            frame_to_heal: 0,
            wake_frame: u32::MAX, // Sleep forever initially
        }
    }

    /// Apply a status damage effect with duration
    ///
    /// # Arguments
    /// * `status` - The status type to apply
    /// * `duration` - Duration in seconds (will be converted to frames)
    /// * `current_frame` - Current game frame
    ///
    /// # Returns
    /// The status that was cleared (if different from the new status)
    pub fn do_status_damage(&mut self, status: ObjectStatusTypes, duration: Real) {
        let duration_frames = duration.floor() as u32;

        // Clear any different status we may have.
        if self.status_to_heal != status {
            self.clear_status_condition();
        }

        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
            if let Ok(mut owner_guard) = owner.write() {
                owner_guard.set_status(ObjectStatusMaskType::from_status(status), true);
            }
        }

        self.status_to_heal = status;
        self.frame_to_heal = TheGameLogic::get_frame().saturating_add(duration_frames);
        self.wake_frame = self.frame_to_heal;
    }

    /// Clear the current status condition
    pub fn clear_status_condition(&mut self) {
        if self.status_to_heal != ObjectStatusTypes::None {
            if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_id) {
                if let Ok(mut owner_guard) = owner.write() {
                    owner_guard.set_status(
                        ObjectStatusMaskType::from_status(self.status_to_heal),
                        false,
                    );
                }
            }
            self.status_to_heal = ObjectStatusTypes::None;
            self.frame_to_heal = 0;
            self.wake_frame = u32::MAX;
        }
    }

    /// Get the current status being tracked
    pub fn get_status_to_heal(&self) -> ObjectStatusTypes {
        self.status_to_heal
    }

    /// Get the frame when healing should occur
    pub fn get_frame_to_heal(&self) -> u32 {
        self.frame_to_heal
    }

    /// Check if a status is currently being tracked
    pub fn has_active_status(&self) -> bool {
        self.status_to_heal != ObjectStatusTypes::None
    }

    /// Get time remaining until status clears
    pub fn get_time_remaining(&self, current_frame: u32) -> u32 {
        if current_frame >= self.frame_to_heal {
            0
        } else {
            self.frame_to_heal - current_frame
        }
    }
}

impl ObjectHelperInterface for StatusDamageHelper {
    fn update(&mut self, current_frame: u32) -> UpdateSleepTime {
        // We are sleep-driven, so seeing an update means our timer is ready
        debug_assert!(
            self.frame_to_heal <= current_frame,
            "StatusDamageHelper woke up too soon"
        );

        // Clear the status condition
        self.clear_status_condition();

        // Sleep forever until next status is applied
        UpdateSleepTime::Forever
    }

    fn get_module_name(&self) -> &str {
        "StatusDamageHelper"
    }

    fn sleep_until(&mut self, wake_frame: u32) {
        self.wake_frame = wake_frame;
    }

    /// Status damage helper must process all disabled types
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::All
    }
}

impl Snapshotable for StatusDamageHelper {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| format!("StatusDamageHelper xfer version: {err:?}"))?;

        let mut status = self.status_to_heal as u32;
        xfer.xfer_unsigned_int(&mut status)
            .map_err(|err| format!("StatusDamageHelper xfer status_to_heal: {err:?}"))?;
        self.status_to_heal = ObjectStatusTypes::from_u32(status);

        xfer.xfer_unsigned_int(&mut self.frame_to_heal)
            .map_err(|err| format!("StatusDamageHelper xfer frame_to_heal: {err:?}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.wake_frame = if self.status_to_heal == ObjectStatusTypes::None {
            u32::MAX
        } else {
            self.frame_to_heal
        };
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    #[test]
    fn test_status_damage_helper_creation() {
        let data = StatusDamageHelperModuleData::new();
        let helper = StatusDamageHelper::new(INVALID_ID, data);

        assert_eq!(helper.status_to_heal, ObjectStatusTypes::None);
        assert_eq!(helper.frame_to_heal, 0);
        assert_eq!(helper.wake_frame, u32::MAX);
        assert!(!helper.has_active_status());
    }

    #[test]
    fn test_disabled_types_processing() {
        let data = StatusDamageHelperModuleData::new();
        let helper = StatusDamageHelper::new(INVALID_ID, data);

        assert_eq!(
            helper.get_disabled_types_to_process(),
            DisabledMaskType::All
        );
    }

    #[test]
    fn xfer_preserves_status_timer_state() {
        let mut saved = StatusDamageHelper::new(INVALID_ID, StatusDamageHelperModuleData::new());
        saved.status_to_heal = ObjectStatusTypes::Immobile;
        saved.frame_to_heal = 1234;
        saved.wake_frame = saved.frame_to_heal;

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 1);
            saved.xfer(&mut xfer).unwrap();
        }

        bytes.set_position(0);
        let mut loaded = StatusDamageHelper::new(INVALID_ID, StatusDamageHelperModuleData::new());
        {
            let mut xfer = XferLoad::new(&mut bytes, 1);
            loaded.xfer(&mut xfer).unwrap();
        }
        loaded.load_post_process().unwrap();

        assert_eq!(loaded.status_to_heal, saved.status_to_heal);
        assert_eq!(loaded.frame_to_heal, saved.frame_to_heal);
        assert_eq!(loaded.wake_frame, saved.frame_to_heal);
    }
}
