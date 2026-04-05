//! DemoralizeSpecialPower
//!
//! Port of DemoralizeSpecialPower.h and DemoralizeSpecialPower.cpp
//! Author: Colin Day, July 2002 (C++), Rust Port
//!
//! GLA ability that slows nearby enemy infantry. The range and duration
//! increase based on the number of units the source object has captured
//! (contained within it). Only affects enemy/neutral infantry in range.
//!
//! C++ is guarded by #ifdef ALLOW_DEMORALIZE but we implement it unconditionally.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{Coord3D, KindOf, ObjectID, Real, Relationship, UnsignedInt};
use crate::helpers::TheGameLogic;
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;

/// Module data for DemoralizeSpecialPower.
/// Matches C++ DemoralizeSpecialPowerModuleData.
#[derive(Debug, Clone)]
pub struct DemoralizeSpecialPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Base range for this special power. Matches C++ m_baseRange.
    pub base_range: Real,
    /// Additional range per captured unit. Matches C++ m_bonusRangePerCaptured.
    pub bonus_range_per_captured: Real,
    /// Maximum range regardless of captured count. Matches C++ m_maxRange.
    pub max_range: Real,
    /// Base duration of demoralization in frames. Matches C++ m_baseDurationInFrames.
    pub base_duration_in_frames: UnsignedInt,
    /// Additional duration per captured unit in frames. Matches C++ m_bonusDurationPerCapturedInFrames.
    pub bonus_duration_per_captured_in_frames: UnsignedInt,
    /// Maximum duration in frames. Matches C++ m_maxDurationInFrames.
    pub max_duration_in_frames: UnsignedInt,
    /// FX list name to play. Matches C++ m_fxList.
    pub fx_list_name: String,
}

impl Default for DemoralizeSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            base_range: 0.0,
            bonus_range_per_captured: 0.0,
            max_range: 0.0,
            base_duration_in_frames: 0,
            bonus_duration_per_captured_in_frames: 0,
            max_duration_in_frames: 0,
            fx_list_name: String::new(),
        }
    }
}

impl DemoralizeSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DEMORALIZE_SPECIAL_POWER_FIELDS)
    }
}

impl ModuleData for DemoralizeSpecialPowerModuleData {
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

impl Snapshotable for DemoralizeSpecialPowerModuleData {
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

/// DemoralizeSpecialPower module.
///
/// Matches C++ DemoralizeSpecialPower which extends SpecialPowerModule.
/// Scans area around target location and demoralizes enemy infantry.
pub struct DemoralizeSpecialPower {
    module_name_key: NameKeyType,
    data: Arc<DemoralizeSpecialPowerModuleData>,
    owner_object_id: ObjectID,
}

impl DemoralizeSpecialPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<DemoralizeSpecialPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Compute effective range and duration based on captured unit count.
    /// Matches C++ DemoralizeSpecialPower::doSpecialPowerAtLocation() lines 99-118.
    fn compute_effect_parameters(&self) -> (Real, UnsignedInt) {
        let mut duration = self.data.base_duration_in_frames;
        let mut range = self.data.base_range;

        // Check contained units for bonuses
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if let Some(contain) = owner_guard.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        let contain_count = contain_guard.get_contained_count() as UnsignedInt;

                        // Bonus duration per captured unit, capped at max
                        duration = duration.saturating_add(
                            self.data
                                .bonus_duration_per_captured_in_frames
                                .saturating_mul(contain_count),
                        );
                        if duration > self.data.max_duration_in_frames {
                            duration = self.data.max_duration_in_frames;
                        }

                        // Bonus range per captured unit, capped at max
                        range += self.data.bonus_range_per_captured
                            * contain_count as Real;
                        if range > self.data.max_range {
                            range = self.data.max_range;
                        }
                    }
                }
            }
        }

        (range, duration)
    }

    /// Execute demoralize at a location.
    /// Matches C++ DemoralizeSpecialPower::doSpecialPowerAtLocation().
    pub fn do_special_power_at_location(
        &self,
        loc: &Coord3D,
    ) -> Result<(), String> {
        // Check disabled
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        let (range, duration) = self.compute_effect_parameters();

        if range <= 0.0 || duration == 0 {
            return Ok(());
        }

        // Get owner's map status for filtering (C++ PartitionFilterSameMapStatus)
        let owner_off_map = if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            owner.read().map(|g| g.is_off_map()).unwrap_or(false)
        } else {
            false
        };

        // Scan objects in range
        // C++ uses PartitionManager with filters:
        //   PartitionFilterRelationship (ALLOW_ENEMIES | ALLOW_NEUTRAL)
        //   PartitionFilterAcceptByKindOf (KINDOF_INFANTRY)
        //   PartitionFilterSameMapStatus
        let object_ids = crate::helpers::ThePartitionManager::get()
            .map(|mgr| mgr.get_objects_in_range(loc, range))
            .unwrap_or_default();

        for obj_id in object_ids {
            let Some(obj_arc) = TheGameLogic::find_object_by_id(obj_id) else {
                continue;
            };

            let should_affect = {
                let Ok(obj_guard) = obj_arc.read() else {
                    continue;
                };

                // Must be infantry (C++ PartitionFilterAcceptByKindOf)
                if !obj_guard.is_kind_of(KindOf::Infantry) {
                    continue;
                }

                // Same map status (C++ PartitionFilterSameMapStatus)
                if obj_guard.is_off_map() != owner_off_map {
                    continue;
                }

                // Enemy or neutral relationship (C++ PartitionFilterRelationship)
                let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
                    continue;
                };
                let Ok(owner_guard) = owner.read() else {
                    continue;
                };
                matches!(
                    owner_guard.relationship_to(&obj_guard),
                    Relationship::Enemies | Relationship::Neutral
                )
            };

            if !should_affect {
                continue;
            }

            // Apply demoralize (C++: ai->setDemoralized(duration))
            if let Ok(obj_guard) = obj_arc.read() {
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(mut ai_guard) = ai.lock() {
                        ai_guard.set_demoralized(duration);
                    }
                }
            };
        }

        // Play FX at destination (C++: FXList::doFXPos(m_fxList, loc))
        if !self.data.fx_list_name.is_empty() {
            if let Some(fx_list) =
                crate::helpers::TheFXListStore::find_fx_list(&self.data.fx_list_name)
            {
                let _ = fx_list.do_fx_at_position(loc);
            }
        }

        Ok(())
    }

    /// Execute demoralize at an object's position.
    /// Matches C++ DemoralizeSpecialPower::doSpecialPowerAtObject().
    pub fn do_special_power_at_object(&self, obj_id: ObjectID) -> Result<(), String> {
        // Check disabled
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        let Some(obj) = TheGameLogic::find_object_by_id(obj_id) else {
            return Ok(());
        };

        let Ok(obj_guard) = obj.read() else {
            return Ok(());
        };

        let pos = *obj_guard.get_position();
        drop(obj_guard);

        self.do_special_power_at_location(&pos)
    }
}

impl Module for DemoralizeSpecialPower {
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

impl Snapshotable for DemoralizeSpecialPower {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DemoralizeSpecialPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ DemoralizeSpecialPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for DemoralizeSpecialPower {
    fn get_module_name(&self) -> &'static str {
        "DemoralizeSpecialPower"
    }
}

// INI field parsers

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template = Some(
        crate::object::special_power_template::find_or_create_special_power_template(&name),
    );
    Ok(())
}

fn parse_base_range(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    data.base_range = INI::parse_real(token)?;
    Ok(())
}

fn parse_bonus_range_per_captured(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    data.bonus_range_per_captured = INI::parse_real(token)?;
    Ok(())
}

fn parse_max_range(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    data.max_range = INI::parse_real(token)?;
    Ok(())
}

fn parse_base_duration(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    // C++ uses INI::parseDurationUnsignedInt which converts seconds*30 to frames
    data.base_duration_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_bonus_duration_per_captured(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    data.bonus_duration_per_captured_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_max_duration(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    data.max_duration_in_frames = INI::parse_duration_unsigned_int(token)?;
    Ok(())
}

fn parse_fx_list(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.iter().find(|t| **t != "=").ok_or(INIError::InvalidData)?;
    data.fx_list_name = (*token).to_string();
    Ok(())
}

const DEMORALIZE_SPECIAL_POWER_FIELDS: &[FieldParse<DemoralizeSpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "BaseRange",
        parse: parse_base_range,
    },
    FieldParse {
        token: "BonusRangePerCaptured",
        parse: parse_bonus_range_per_captured,
    },
    FieldParse {
        token: "MaxRange",
        parse: parse_max_range,
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
    FieldParse {
        token: "FXList",
        parse: parse_fx_list,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_demoralize_default() {
        let data = DemoralizeSpecialPowerModuleData::default();
        assert_eq!(data.base_range, 0.0);
        assert_eq!(data.bonus_range_per_captured, 0.0);
        assert_eq!(data.max_range, 0.0);
        assert_eq!(data.base_duration_in_frames, 0);
        assert_eq!(data.bonus_duration_per_captured_in_frames, 0);
        assert_eq!(data.max_duration_in_frames, 0);
    }

    #[test]
    fn test_demoralize_module_name() {
        let data = DemoralizeSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = DemoralizeSpecialPower::new(0, 0, arc_data);
        assert_eq!(power.get_module_name(), "DemoralizeSpecialPower");
    }

    #[test]
    fn test_compute_effect_parameters_no_owner() {
        let mut data = DemoralizeSpecialPowerModuleData::default();
        data.base_range = 100.0;
        data.base_duration_in_frames = 300;
        let arc_data = Arc::new(data);
        let power = DemoralizeSpecialPower::new(0, 0, arc_data);
        let (range, duration) = power.compute_effect_parameters();
        assert_eq!(range, 100.0);
        assert_eq!(duration, 300);
    }

    #[test]
    fn test_do_special_power_at_location_no_owner() {
        let data = DemoralizeSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = DemoralizeSpecialPower::new(0, 0, arc_data);
        // Should return Ok without panicking
        assert!(power.do_special_power_at_location(&Coord3D::new(0.0, 0.0, 0.0)).is_ok());
    }

    #[test]
    fn test_do_special_power_at_object_no_owner() {
        let data = DemoralizeSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = DemoralizeSpecialPower::new(0, 0, arc_data);
        assert!(power.do_special_power_at_object(999).is_ok());
    }
}
