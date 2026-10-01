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
use crate::object::Object;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
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

    /// C++ UpdateModule base state: packed next-call frame and phase.
    next_call_frame_and_phase: u32,

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
            next_call_frame_and_phase: 0,
            wake_frame: u32::MAX, // Sleep forever initially
        }
    }

    /// Apply a status damage effect with duration
    ///
    /// # Arguments
    /// * `status` - The status type to apply
    /// * `duration` - Duration value floored to an integer logic-frame count, matching C++ `REAL_TO_INT_FLOOR`
    /// * `current_frame` - Current game frame
    ///
    /// # Returns
    /// The status that was cleared (if different from the new status)
    pub fn do_status_damage(&mut self, status: ObjectStatusTypes, duration: Real) {
        let owner_id = self.owner_id;
        self.start_status_damage(status, duration, |changed_status, enabled| {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                owner_guard
                    .set_status(ObjectStatusMaskType::from_status(changed_status), enabled);
                });
        });
    }

    /// Shared status/timer transition. The callback applies each object-side
    /// status change at the same point in the transition for both owner paths.
    fn start_status_damage(
        &mut self,
        status: ObjectStatusTypes,
        duration: Real,
        mut apply_status: impl FnMut(ObjectStatusTypes, bool),
    ) {
        let duration_frames = duration.floor() as u32;
        if self.status_to_heal != status {
            self.clear_status_condition_with(|old_status| apply_status(old_status, false));
        }

        apply_status(status, true);
        self.status_to_heal = status;
        self.frame_to_heal = TheGameLogic::get_frame().saturating_add(duration_frames);
        self.wake_frame = self.frame_to_heal;
    }

    /// Apply the status directly to the object that owns this helper.
    /// Object calls this while already mutably borrowing itself, so resolving
    /// `owner_id` through the global registry would re-enter the same object.
    pub(crate) fn do_status_damage_in_owner(
        &mut self,
        status: ObjectStatusTypes,
        duration: Real,
        owner: &mut Object,
    ) {
        self.start_status_damage(status, duration, |changed_status, enabled| {
            owner.set_status(ObjectStatusMaskType::from_status(changed_status), enabled);
        });
    }

    /// Clear the current status condition
    pub fn clear_status_condition(&mut self) {
        let owner_id = self.owner_id;
        self.clear_status_condition_with(|status| {
            let _ = crate::object::registry::OBJECT_REGISTRY.with_object_mut(owner_id, |owner_guard| {
                owner_guard.set_status(ObjectStatusMaskType::from_status(status), false);
                });
        });
    }

    fn clear_status_condition_with(&mut self, mut clear_status: impl FnMut(ObjectStatusTypes)) {
        if self.status_to_heal != ObjectStatusTypes::None {
            clear_status(self.status_to_heal);
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

    pub fn set_frame_to_heal_for_test(&mut self, frame: u32) {
        self.frame_to_heal = frame;
    }

    #[cfg(test)]
    pub fn set_status_to_heal_for_test(&mut self, status: ObjectStatusTypes) {
        self.status_to_heal = status;
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

    /// Owner-explicit variant of [`ObjectHelperInterface::update`].
    ///
    /// `Object::update` passes itself here instead of the helper re-finding its
    /// owner through `TheGameLogic::find_object_by_id(self.owner_id)` (the path
    /// `clear_status_condition` takes), which never returns in isolated/local-instance
    /// contexts. Logic mirrors the C++ StatusDamageHelper update (Graham Smallwood,
    /// June 2003 — see file header) and matches the trait impl minus the global lookup.
    pub fn update_in_owner(&mut self, current_frame: u32, owner: &mut Object) -> UpdateSleepTime {
        // We are sleep-driven, so seeing an update means our timer is ready
        debug_assert!(
            self.frame_to_heal <= current_frame,
            "StatusDamageHelper woke up too soon"
        );

        // Clear the status condition directly on the owner (mirrors
        // clear_status_condition without the global owner lookup).
        if self.status_to_heal != ObjectStatusTypes::None {
            owner.set_status(
                ObjectStatusMaskType::from_status(self.status_to_heal),
                false,
            );
            self.status_to_heal = ObjectStatusTypes::None;
            self.frame_to_heal = 0;
            self.wake_frame = u32::MAX;
        }

        // Sleep forever until next status is applied
        UpdateSleepTime::Forever
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
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| format!("StatusDamageHelper crc version: {err:?}"))?;

        let mut object_helper_version = CURRENT_VERSION;
        xfer.xfer_version(&mut object_helper_version, CURRENT_VERSION)
            .map_err(|err| format!("StatusDamageHelper crc object helper version: {err:?}"))?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|err| format!("StatusDamageHelper crc update module base: {err}"))?;

        let mut status = self.status_to_heal as u32;
        xfer.xfer_unsigned_int(&mut status)
            .map_err(|err| format!("StatusDamageHelper crc status_to_heal: {err:?}"))?;
        let mut frame_to_heal = self.frame_to_heal;
        xfer.xfer_unsigned_int(&mut frame_to_heal)
            .map_err(|err| format!("StatusDamageHelper crc frame_to_heal: {err:?}"))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| format!("StatusDamageHelper xfer version: {err:?}"))?;

        let mut object_helper_version = CURRENT_VERSION;
        xfer.xfer_version(&mut object_helper_version, CURRENT_VERSION)
            .map_err(|err| format!("StatusDamageHelper xfer object helper version: {err:?}"))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|err| format!("StatusDamageHelper xfer update module base: {err}"))?;

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
        saved.next_call_frame_and_phase = 0x2234;
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
        assert_eq!(
            loaded.next_call_frame_and_phase,
            saved.next_call_frame_and_phase
        );
        assert_eq!(loaded.wake_frame, saved.frame_to_heal);
    }
}
