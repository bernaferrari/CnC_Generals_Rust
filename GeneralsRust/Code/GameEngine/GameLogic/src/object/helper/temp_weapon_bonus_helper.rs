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
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::drawable::TintStatus;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

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

    /// C++ UpdateModule base state: packed next-call frame and phase.
    next_call_frame_and_phase: u32,

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
            next_call_frame_and_phase: 0,
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

fn weapon_bonus_to_cpp_value(bonus: WeaponBonusConditionType) -> u32 {
    match bonus {
        WeaponBonusConditionType::Invalid => u32::MAX,
        WeaponBonusConditionType::Garrisoned => 0,
        WeaponBonusConditionType::Horde => 1,
        WeaponBonusConditionType::ContinuousFireMean => 2,
        WeaponBonusConditionType::ContinuousFireFast => 3,
        WeaponBonusConditionType::Nationalism => 4,
        WeaponBonusConditionType::PlayerUpgrade => 5,
        WeaponBonusConditionType::DroneSpotting | WeaponBonusConditionType::DroneSpotForStrike => 6,
        WeaponBonusConditionType::Demoralized | WeaponBonusConditionType::DemoralizedObsolete => 7,
        WeaponBonusConditionType::Enthusiastic => 8,
        WeaponBonusConditionType::Veteran => 9,
        WeaponBonusConditionType::Elite => 10,
        WeaponBonusConditionType::Hero => 11,
        WeaponBonusConditionType::BattlePlanBombardment => 12,
        WeaponBonusConditionType::BattlePlanHoldTheLine => 13,
        WeaponBonusConditionType::BattlePlanSearchAndDestroy => 14,
        WeaponBonusConditionType::Subliminal => 15,
        WeaponBonusConditionType::SoloHumanEasy => 16,
        WeaponBonusConditionType::SoloHumanNormal => 17,
        WeaponBonusConditionType::SoloHumanHard => 18,
        WeaponBonusConditionType::SoloAiEasy => 19,
        WeaponBonusConditionType::SoloAiNormal => 20,
        WeaponBonusConditionType::SoloAiHard => 21,
        WeaponBonusConditionType::TargetFaerieFire => 22,
        WeaponBonusConditionType::Fanaticism => 23,
        WeaponBonusConditionType::FrenzyOne => 24,
        WeaponBonusConditionType::FrenzyTwo => 25,
        WeaponBonusConditionType::FrenzyThree => 26,
    }
}

fn weapon_bonus_from_cpp_value(value: u32) -> WeaponBonusConditionType {
    match value {
        u32::MAX => WeaponBonusConditionType::Invalid,
        0 => WeaponBonusConditionType::Garrisoned,
        1 => WeaponBonusConditionType::Horde,
        2 => WeaponBonusConditionType::ContinuousFireMean,
        3 => WeaponBonusConditionType::ContinuousFireFast,
        4 => WeaponBonusConditionType::Nationalism,
        5 => WeaponBonusConditionType::PlayerUpgrade,
        6 => WeaponBonusConditionType::DroneSpotting,
        7 => WeaponBonusConditionType::DemoralizedObsolete,
        8 => WeaponBonusConditionType::Enthusiastic,
        9 => WeaponBonusConditionType::Veteran,
        10 => WeaponBonusConditionType::Elite,
        11 => WeaponBonusConditionType::Hero,
        12 => WeaponBonusConditionType::BattlePlanBombardment,
        13 => WeaponBonusConditionType::BattlePlanHoldTheLine,
        14 => WeaponBonusConditionType::BattlePlanSearchAndDestroy,
        15 => WeaponBonusConditionType::Subliminal,
        16 => WeaponBonusConditionType::SoloHumanEasy,
        17 => WeaponBonusConditionType::SoloHumanNormal,
        18 => WeaponBonusConditionType::SoloHumanHard,
        19 => WeaponBonusConditionType::SoloAiEasy,
        20 => WeaponBonusConditionType::SoloAiNormal,
        21 => WeaponBonusConditionType::SoloAiHard,
        22 => WeaponBonusConditionType::TargetFaerieFire,
        23 => WeaponBonusConditionType::Fanaticism,
        24 => WeaponBonusConditionType::FrenzyOne,
        25 => WeaponBonusConditionType::FrenzyTwo,
        26 => WeaponBonusConditionType::FrenzyThree,
        _ => WeaponBonusConditionType::Invalid,
    }
}

impl Snapshotable for TempWeaponBonusHelper {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|err| format!("TempWeaponBonusHelper xfer version: {err:?}"))?;

        let mut object_helper_version = CURRENT_VERSION;
        xfer.xfer_version(&mut object_helper_version, CURRENT_VERSION)
            .map_err(|err| format!("TempWeaponBonusHelper xfer object helper version: {err:?}"))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)
            .map_err(|err| format!("TempWeaponBonusHelper xfer update module base: {err}"))?;

        let mut bonus = weapon_bonus_to_cpp_value(self.current_bonus);
        xfer.xfer_unsigned_int(&mut bonus)
            .map_err(|err| format!("TempWeaponBonusHelper xfer current_bonus: {err:?}"))?;
        self.current_bonus = weapon_bonus_from_cpp_value(bonus);

        xfer.xfer_unsigned_int(&mut self.frame_to_remove)
            .map_err(|err| format!("TempWeaponBonusHelper xfer frame_to_remove: {err:?}"))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        if self.current_bonus == WeaponBonusConditionType::Invalid {
            self.frame_to_remove = 0;
            self.current_tint = TintStatus::NONE;
            self.wake_frame = u32::MAX;
        } else {
            self.current_tint = match self.current_bonus {
                WeaponBonusConditionType::FrenzyOne
                | WeaponBonusConditionType::FrenzyTwo
                | WeaponBonusConditionType::FrenzyThree => TintStatus::FRENZY,
                _ => TintStatus::NONE,
            };
            self.wake_frame = self.frame_to_remove;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::system::{xfer_load::XferLoad, xfer_save::XferSave};
    use std::io::Cursor;

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

    #[test]
    fn xfer_preserves_temp_weapon_bonus_timer_state() {
        let mut saved =
            TempWeaponBonusHelper::new(INVALID_ID, TempWeaponBonusHelperModuleData::new());
        saved.do_temp_weapon_bonus(WeaponBonusConditionType::FrenzyTwo, 90, 120);
        saved.next_call_frame_and_phase = 0x4234;

        let mut bytes = Cursor::new(Vec::new());
        {
            let mut xfer = XferSave::new(&mut bytes, 1);
            saved.xfer(&mut xfer).unwrap();
        }

        bytes.set_position(0);
        let mut loaded =
            TempWeaponBonusHelper::new(INVALID_ID, TempWeaponBonusHelperModuleData::new());
        {
            let mut xfer = XferLoad::new(&mut bytes, 1);
            loaded.xfer(&mut xfer).unwrap();
        }
        loaded.load_post_process().unwrap();

        assert_eq!(loaded.current_bonus, saved.current_bonus);
        assert_eq!(loaded.frame_to_remove, saved.frame_to_remove);
        assert_eq!(
            loaded.next_call_frame_and_phase,
            saved.next_call_frame_and_phase
        );
        assert_eq!(loaded.wake_frame, saved.frame_to_remove);
        assert_eq!(loaded.current_tint, TintStatus::FRENZY);
    }

    #[test]
    fn weapon_bonus_cpp_wire_values_match_retail_order() {
        assert_eq!(
            weapon_bonus_to_cpp_value(WeaponBonusConditionType::Invalid),
            u32::MAX
        );
        assert_eq!(
            weapon_bonus_to_cpp_value(WeaponBonusConditionType::Garrisoned),
            0
        );
        assert_eq!(
            weapon_bonus_to_cpp_value(WeaponBonusConditionType::DemoralizedObsolete),
            7
        );
        assert_eq!(
            weapon_bonus_to_cpp_value(WeaponBonusConditionType::FrenzyThree),
            26
        );
        assert_eq!(
            weapon_bonus_from_cpp_value(7),
            WeaponBonusConditionType::DemoralizedObsolete
        );
    }
}
