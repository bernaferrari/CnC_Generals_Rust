//! SpyVisionSpecialPower
//!
//! Port of SpyVisionSpecialPower.h and SpyVisionSpecialPower.cpp
//! Author: Graham Smallwood (C++), Rust Port
//!
//! Reveals enemy unit vision to the player for a duration.
//! The duration can be extended by capturing units (contained count bonus).
//! Delegates to SpyVisionUpdate for the actual shroud/vision manipulation.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{ObjectID, UnsignedInt};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, ContainModuleInterface};
use crate::object::special_power_module::SpecialPowerModuleData;

/// Module data for SpyVisionSpecialPower.
/// Matches C++ SpyVisionSpecialPowerModuleData.
#[derive(Debug, Clone)]
pub struct SpyVisionSpecialPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Base duration in frames. Matches C++ m_baseDurationInFrames.
    pub base_duration_in_frames: UnsignedInt,
    /// Additional duration per captured unit in frames.
    /// Matches C++ m_bonusDurationPerCapturedInFrames.
    pub bonus_duration_per_captured_in_frames: UnsignedInt,
    /// Maximum duration in frames regardless of captured count.
    /// Matches C++ m_maxDurationInFrames.
    pub max_duration_in_frames: UnsignedInt,
}

impl Default for SpyVisionSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            base_duration_in_frames: 0,
            bonus_duration_per_captured_in_frames: 0,
            max_duration_in_frames: 0,
        }
    }
}

impl SpyVisionSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPY_VISION_SPECIAL_POWER_FIELDS)
    }
}

impl ModuleData for SpyVisionSpecialPowerModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.base.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.base.base.get_module_tag_name_key()
    }
}

impl Snapshotable for SpyVisionSpecialPowerModuleData {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(_xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// SpyVisionSpecialPower module.
///
/// Matches C++ SpyVisionSpecialPower which extends SpecialPowerModule.
/// When activated, calculates duration based on contained units and
/// delegates to the SpyVisionUpdate module on the same object.
pub struct SpyVisionSpecialPower {
    module_name_key: NameKeyType,
    data: Arc<SpyVisionSpecialPowerModuleData>,
    owner_object_id: ObjectID,
}

impl SpyVisionSpecialPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<SpyVisionSpecialPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Activate the spy vision special power.
    /// Matches C++ SpyVisionSpecialPower::doSpecialPower().
    pub fn do_special_power(&self, _command_options: u32) {
        // Check if disabled
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        // Calculate duration from module data
        // Matches C++ SpyVisionSpecialPower::doSpecialPower() duration calculation
        let mut duration = self.data.base_duration_in_frames;

        // Check if the owner object has a contain module for bonus duration
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            let contain = {
                let owner_read = owner.read().ok();
                owner_read.and_then(|guard| guard.get_contain())
            };

            if let Some(contain_arc) = contain {
                if let Ok(contain_guard) = contain_arc.lock() {
                    // For every captured unit we get a bonus
                    let contain_count = contain_guard.get_contain_count();
                    duration = duration.saturating_add(
                        contain_count
                            .saturating_mul(self.data.bonus_duration_per_captured_in_frames),
                    );

                    // Cap at the max
                    if self.data.max_duration_in_frames > 0
                        && duration > self.data.max_duration_in_frames
                    {
                        duration = self.data.max_duration_in_frames;
                    }
                }
            }

            // Find and activate the SpyVisionUpdate module
            // Matches C++: static const NameKeyType key_SpyVisionUpdate = NAMEKEY("SpyVisionUpdate");
            //             SpyVisionUpdate *update = (SpyVisionUpdate*)source->findUpdateModule(key_SpyVisionUpdate);
            if let Some(spy_update_handle) = {
                let owner_read = owner.read().ok();
                owner_read.and_then(|guard| guard.find_update_module("SpyVisionUpdate"))
            } {
                // Activate spy vision on the update module
                self.activate_spy_vision_update(&spy_update_handle, duration);
            }
        }
    }

    /// Activate spy vision on the SpyVisionUpdate module found on the object.
    fn activate_spy_vision_update(
        &self,
        _handle: &crate::object::BehaviorModuleHandle,
        duration: UnsignedInt,
    ) {
        // Try to access the SpyVisionUpdate through the behavior module system.
        // The SpyVisionUpdate is registered as a behavior module with name "SpyVisionUpdate".
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                // Try through find_update_behavior which gives us the BehaviorModuleInterface
                if let Some(behavior_arc) = owner_guard.find_update_behavior("SpyVisionUpdate") {
                    if let Ok(mut behavior_guard) = behavior_arc.lock() {
                        // Use the SpyVisionUpdate trait method
                        if let Some(spy) = behavior_guard.get_spy_vision_update() {
                            spy.activate_spy_vision(duration);
                        }
                    }
                }
            }
        }
    }
}

impl Module for SpyVisionSpecialPower {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }
}

impl Snapshotable for SpyVisionSpecialPower {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpyVisionSpecialPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ SpyVisionSpecialPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for SpyVisionSpecialPower {
    fn get_module_name(&self) -> &'static str {
        "SpyVisionSpecialPower"
    }
}

// INI field parsers

fn parse_base_duration(
    _ini: &mut INI,
    data: &mut SpyVisionSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    // C++ uses INI::parseDurationUnsignedInt which converts ms/seconds to frames.
    data.base_duration_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_bonus_duration_per_captured(
    _ini: &mut INI,
    data: &mut SpyVisionSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.bonus_duration_per_captured_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_max_duration(
    _ini: &mut INI,
    data: &mut SpyVisionSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.max_duration_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut SpyVisionSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

const SPY_VISION_SPECIAL_POWER_FIELDS: &[FieldParse<SpyVisionSpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "BaseDuration",
        parse: parse_base_duration,
    },
    FieldParse {
        token: "BonusDurationPerCaptured",
        parse: parse_bonus_duration_per_captured,
    },
    FieldParse {
        token: "MaxDuration",
        parse: parse_max_duration,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spy_vision_default() {
        let data = SpyVisionSpecialPowerModuleData::default();
        assert_eq!(data.base_duration_in_frames, 0);
        assert_eq!(data.bonus_duration_per_captured_in_frames, 0);
        assert_eq!(data.max_duration_in_frames, 0);
    }

    #[test]
    fn test_duration_capping() {
        let base = 1000u32;
        let bonus = 500u32;
        let max = 2500u32;

        // With 3 captured units: 1000 + 3*500 = 2500 -> capped at 2500
        let duration = base.saturating_add(3u32.saturating_mul(bonus));
        let capped = if max > 0 && duration > max {
            max
        } else {
            duration
        };
        assert_eq!(capped, 2500);

        // With 4 captured units: 1000 + 4*500 = 3000 -> capped at 2500
        let duration = base.saturating_add(4u32.saturating_mul(bonus));
        let capped = if max > 0 && duration > max {
            max
        } else {
            duration
        };
        assert_eq!(capped, 2500);
    }
}
