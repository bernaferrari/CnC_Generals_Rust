//! ObjectDefectionHelper - Manages defection timer and visual effects
//!
//! This helper module manages the "undetected defector" state for units.
//! When a unit defects to another side (e.g., via Colonel Burton's ability),
//! it remains undetected for a period of time. This module:
//!
//! - Tracks the defection detection timer
//! - Flashes the unit to indicate detection countdown
//! - Plays audio warnings as detection approaches
//! - Clears the undetected state when timer expires or unit attacks
//!
//! The flash rate increases logarithmically as detection approaches, creating
//! tension and urgency for the player.
//!
//! Original C++ Authors: Steven Johnson, Colin Day (September 2002)
//! Rust conversion: 2025

use super::{DisabledMaskType, ObjectHelperInterface, UpdateSleepTime};
use crate::common::*;
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};
use std::sync::{Arc, RwLock};

/// Maximum defection detection time (10 seconds at 30 FPS)
pub const DEFECTION_DETECTION_TIME_MAX: u32 = 30 * 10; // LOGICFRAMES_PER_SECOND * 10
pub const LOGICFRAMES_PER_SECOND: u32 = 30;

/// Module data for ObjectDefectionHelper
///
/// No configuration parameters needed for this helper
#[derive(Debug, Clone)]
pub struct ObjectDefectionHelperModuleData {}

impl ObjectDefectionHelperModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for ObjectDefectionHelperModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// ObjectDefectionHelper - Manages defection timer and visual effects
///
/// This helper tracks when an undetected defector will be revealed and
/// provides visual/audio feedback to the player as the timer counts down.
#[derive(Debug)]
pub struct ObjectDefectionHelper {
    /// Module data
    #[allow(dead_code)]
    module_data: ObjectDefectionHelperModuleData,

    /// Defection detection start frame (absolute frame, NOT counter)
    defection_detection_start: u32,

    /// Defection detection end frame (absolute frame, NOT counter)
    defection_detection_end: u32,

    /// Flash phase - tracks the flashing rate logarithmic curve
    defection_detection_flash_phase: f32,

    /// Whether to do defector FX (AmericaInfPilot uses defect to become temporarily "invulnerable")
    do_defector_fx: bool,

    /// C++ UpdateModule base state: packed next-call frame and phase.
    next_call_frame_and_phase: u32,

    /// Next wake frame
    wake_frame: u32,
}

impl ObjectDefectionHelper {
    /// Create a new ObjectDefectionHelper
    pub fn new(module_data: ObjectDefectionHelperModuleData) -> Self {
        Self {
            module_data,
            defection_detection_start: 0,
            defection_detection_end: 0,
            defection_detection_flash_phase: 0.0,
            do_defector_fx: false,
            next_call_frame_and_phase: 0,
            wake_frame: 0,
        }
    }

    /// Start the defection timer
    ///
    /// # Arguments
    /// * `num_frames` - Number of frames until detection
    /// * `with_defector_fx` - Whether to show visual/audio effects
    /// * `current_frame` - Current game frame
    /// * `is_undetected_defector` - Whether object is currently an undetected defector
    pub fn start_defection_timer(
        &mut self,
        num_frames: u32,
        with_defector_fx: bool,
        current_frame: u32,
        is_undetected_defector: bool,
    ) {
        if !is_undetected_defector {
            self.wake_frame = u32::MAX; // Sleep forever
            return;
        }

        self.defection_detection_start = current_frame;
        self.defection_detection_end = current_frame + num_frames;
        self.defection_detection_flash_phase = 0.0;
        self.do_defector_fx = with_defector_fx;
        self.wake_frame = 0; // Wake every frame
    }

    /// Check if the timer has expired
    pub fn has_timer_expired(&self, current_frame: u32) -> bool {
        current_frame >= self.defection_detection_end
    }

    /// Get time remaining in frames
    pub fn get_time_remaining(&self, current_frame: u32) -> u32 {
        if current_frame >= self.defection_detection_end {
            0
        } else {
            self.defection_detection_end - current_frame
        }
    }

    /// Check if should flash this frame
    ///
    /// Returns (should_flash, flash_color) where color is (r, g, b)
    pub fn should_flash(&mut self, current_frame: u32) -> (bool, Option<(f32, f32, f32)>) {
        if !self.do_defector_fx {
            return (false, None);
        }

        // Check if timer has expired - flash white once
        if current_frame >= self.defection_detection_end {
            return (true, Some((1.0, 1.0, 1.0))); // White flash
        }

        // Calculate logarithmic flash rate
        let last_phase = (self.defection_detection_flash_phase as i32) & 1;
        let time_left = self.get_time_remaining(current_frame);

        // Flash rate increases as time runs out
        let time_ratio = 1.0 - (time_left as f32 / DEFECTION_DETECTION_TIME_MAX as f32);
        self.defection_detection_flash_phase += 0.5 * time_ratio;

        let this_phase = (self.defection_detection_flash_phase as i32) & 1;

        // Flash when transitioning from phase 1 to phase 0
        if last_phase == 1 && this_phase == 0 {
            (true, None) // Normal selected flash
        } else {
            (false, None)
        }
    }

    /// Get defection detection end frame
    pub fn get_defection_detection_end(&self) -> u32 {
        self.defection_detection_end
    }

    /// Check if defector FX is enabled
    pub fn is_defector_fx_enabled(&self) -> bool {
        self.do_defector_fx
    }
}

impl ObjectHelperInterface for ObjectDefectionHelper {
    fn update(&mut self, current_frame: u32) -> UpdateSleepTime {
        // This will be called by the object system with context about:
        // - Whether object is undetected defector
        // - Whether object is dead
        // - Whether object is firing weapon
        //
        // For now, return the basic timer logic

        if current_frame >= self.defection_detection_end {
            // Timer expired - should clear defector state and sleep forever
            UpdateSleepTime::Forever
        } else {
            // Continue updating every frame to check conditions
            UpdateSleepTime::None
        }
    }

    fn get_module_name(&self) -> &str {
        "ObjectDefectionHelper"
    }

    fn sleep_until(&mut self, wake_frame: u32) {
        self.wake_frame = wake_frame;
    }

    /// Defection helper must process all disabled types
    fn get_disabled_types_to_process(&self) -> DisabledMaskType {
        DisabledMaskType::All
    }
}

impl Snapshotable for ObjectDefectionHelper {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| format!("ObjectDefectionHelper crc version: {err:?}"))?;

        let mut object_helper_version = CURRENT_VERSION;
        xfer.xfer_version(&mut object_helper_version, CURRENT_VERSION)
            .map_err(|err| format!("ObjectDefectionHelper crc object helper version: {err:?}"))?;
        let mut next_call_frame_and_phase = self.next_call_frame_and_phase;
        xfer_update_module_base_state(xfer, &mut next_call_frame_and_phase)
            .map_err(|err| format!("ObjectDefectionHelper crc update module base: {err}"))?;

        let mut defection_detection_start = self.defection_detection_start;
        xfer.xfer_unsigned_int(&mut defection_detection_start)
            .map_err(|err| format!("ObjectDefectionHelper crc detection_start: {err:?}"))?;
        let mut defection_detection_end = self.defection_detection_end;
        xfer.xfer_unsigned_int(&mut defection_detection_end)
            .map_err(|err| format!("ObjectDefectionHelper crc detection_end: {err:?}"))?;
        let mut defection_detection_flash_phase = self.defection_detection_flash_phase;
        xfer.xfer_real(&mut defection_detection_flash_phase)
            .map_err(|err| format!("ObjectDefectionHelper crc flash_phase: {err:?}"))?;
        let mut do_defector_fx = self.do_defector_fx;
        xfer.xfer_bool(&mut do_defector_fx)
            .map_err(|err| format!("ObjectDefectionHelper crc do_defector_fx: {err:?}"))?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| format!("ObjectDefectionHelper xfer version: {err:?}"))?;

        let mut object_helper_version = CURRENT_VERSION;
        xfer.xfer_version(&mut object_helper_version, CURRENT_VERSION)
            .map_err(|err| format!("ObjectDefectionHelper xfer object helper version: {err:?}"))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|err| format!("ObjectDefectionHelper xfer update module base: {err}"))?;

        xfer.xfer_unsigned_int(&mut self.defection_detection_start)
            .map_err(|err| format!("ObjectDefectionHelper xfer detection_start: {err:?}"))?;
        xfer.xfer_unsigned_int(&mut self.defection_detection_end)
            .map_err(|err| format!("ObjectDefectionHelper xfer detection_end: {err:?}"))?;
        xfer.xfer_real(&mut self.defection_detection_flash_phase)
            .map_err(|err| format!("ObjectDefectionHelper xfer flash_phase: {err:?}"))?;
        xfer.xfer_bool(&mut self.do_defector_fx)
            .map_err(|err| format!("ObjectDefectionHelper xfer do_defector_fx: {err:?}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

    #[test]
    fn test_defection_helper_creation() {
        let data = ObjectDefectionHelperModuleData::new();
        let helper = ObjectDefectionHelper::new(data);

        assert_eq!(helper.defection_detection_start, 0);
        assert_eq!(helper.defection_detection_end, 0);
        assert_eq!(helper.defection_detection_flash_phase, 0.0);
        assert!(!helper.do_defector_fx);
    }

    #[test]
    fn test_start_defection_timer() {
        let data = ObjectDefectionHelperModuleData::new();
        let mut helper = ObjectDefectionHelper::new(data);

        let current_frame = 100;
        let duration = 300; // 10 seconds

        helper.start_defection_timer(duration, true, current_frame, true);

        assert_eq!(helper.defection_detection_start, current_frame);
        assert_eq!(helper.defection_detection_end, current_frame + duration);
        assert!(helper.do_defector_fx);
        assert_eq!(helper.wake_frame, 0); // Wake every frame
    }

    #[test]
    fn test_timer_expiration() {
        let data = ObjectDefectionHelperModuleData::new();
        let mut helper = ObjectDefectionHelper::new(data);

        helper.start_defection_timer(300, true, 100, true);

        assert!(!helper.has_timer_expired(100));
        assert!(!helper.has_timer_expired(300));
        assert!(helper.has_timer_expired(400));
        assert!(helper.has_timer_expired(500));
    }

    #[test]
    fn test_time_remaining() {
        let data = ObjectDefectionHelperModuleData::new();
        let mut helper = ObjectDefectionHelper::new(data);

        helper.start_defection_timer(300, true, 100, true);

        assert_eq!(helper.get_time_remaining(100), 300);
        assert_eq!(helper.get_time_remaining(200), 200);
        assert_eq!(helper.get_time_remaining(350), 50);
        assert_eq!(helper.get_time_remaining(400), 0);
        assert_eq!(helper.get_time_remaining(500), 0);
    }

    #[test]
    fn test_flash_at_expiration() {
        let data = ObjectDefectionHelperModuleData::new();
        let mut helper = ObjectDefectionHelper::new(data);

        helper.start_defection_timer(300, true, 100, true);

        // Flash white at expiration
        let (should_flash, color) = helper.should_flash(400);
        assert!(should_flash);
        assert_eq!(color, Some((1.0, 1.0, 1.0)));
    }

    #[test]
    fn test_no_fx_when_disabled() {
        let data = ObjectDefectionHelperModuleData::new();
        let mut helper = ObjectDefectionHelper::new(data);

        helper.start_defection_timer(300, false, 100, true); // FX disabled

        let (should_flash, _) = helper.should_flash(200);
        assert!(!should_flash); // No flash when FX disabled
    }

    #[test]
    fn test_disabled_types_processing() {
        let data = ObjectDefectionHelperModuleData::new();
        let helper = ObjectDefectionHelper::new(data);

        assert_eq!(
            helper.get_disabled_types_to_process(),
            DisabledMaskType::All
        );
    }

    #[test]
    fn test_not_undetected_defector() {
        let data = ObjectDefectionHelperModuleData::new();
        let mut helper = ObjectDefectionHelper::new(data);

        helper.start_defection_timer(300, true, 100, false); // Not undetected

        assert_eq!(helper.wake_frame, u32::MAX); // Sleep forever
    }

    #[test]
    fn xfer_preserves_detection_timer_state() {
        let mut saved = ObjectDefectionHelper::new(ObjectDefectionHelperModuleData::new());
        saved.start_defection_timer(300, true, 100, true);
        saved.defection_detection_flash_phase = 2.75;
        saved.next_call_frame_and_phase = 0x1234;
        saved.wake_frame = 42;

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 1);
            saved.xfer(&mut xfer).unwrap();
        }

        bytes.set_position(0);
        let mut loaded = ObjectDefectionHelper::new(ObjectDefectionHelperModuleData::new());
        {
            let mut xfer = XferLoad::new(&mut bytes, 1);
            loaded.xfer(&mut xfer).unwrap();
        }

        assert_eq!(
            loaded.defection_detection_start,
            saved.defection_detection_start
        );
        assert_eq!(
            loaded.defection_detection_end,
            saved.defection_detection_end
        );
        assert_eq!(
            loaded.defection_detection_flash_phase,
            saved.defection_detection_flash_phase
        );
        assert_eq!(loaded.do_defector_fx, saved.do_defector_fx);
        assert_eq!(
            loaded.next_call_frame_and_phase,
            saved.next_call_frame_and_phase
        );
    }
}
