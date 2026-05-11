//! CleanupAreaPower
//!
//! Port of CleanupAreaPower.h and CleanupAreaPower.cpp
//! Author: Kris Morness, September 2002 (C++), Rust Port
//!
//! Makes use of the CleanupHazardUpdate module by augmenting the cleanup range
//! until there is nothing left to clean up, at which time it goes idle.
//! Used by the Ambulance to clean mines and other hazards in an area.
//!
//! Key behavior:
//! - doSpecialPowerAtLocation: delegates to CleanupHazardUpdate::setCleanupAreaParameters()
//! - The CleanupHazardUpdate module handles the actual movement and cleanup logic

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{Coord3D, ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, CleanupHazardUpdateInterface};
use crate::object::special_power_module::SpecialPowerModuleData;

/// Module data for CleanupAreaPower.
/// Matches C++ CleanupAreaPowerModuleData.
#[derive(Debug, Clone)]
pub struct CleanupAreaPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Maximum move distance from the cleanup location.
    /// Matches C++ m_cleanupMoveRange.
    pub cleanup_move_range: Real,
}

impl Default for CleanupAreaPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            cleanup_move_range: 0.0,
        }
    }
}

impl CleanupAreaPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CLEANUP_AREA_POWER_FIELDS)
    }
}

impl ModuleData for CleanupAreaPowerModuleData {
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

impl Snapshotable for CleanupAreaPowerModuleData {
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

/// CleanupAreaPower module.
///
/// Matches C++ CleanupAreaPower which extends SpecialPowerModule.
/// Sets cleanup area parameters on the owning object's CleanupHazardUpdate module.
pub struct CleanupAreaPower {
    module_name_key: NameKeyType,
    data: Arc<CleanupAreaPowerModuleData>,
    owner_object_id: ObjectID,
}

impl CleanupAreaPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<CleanupAreaPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Execute cleanup area power at location.
    /// Matches C++ CleanupAreaPower::doSpecialPowerAtLocation().
    pub fn do_special_power_at_location(&self, loc: &Coord3D) -> Result<(), String> {
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

        let move_range = self.data.cleanup_move_range;

        // Find the CleanupHazardUpdate module on the owner
        // C++: obj->findUpdateModule("CleanupHazardUpdate")
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        let Ok(owner_guard) = owner.read() else {
            return Ok(());
        };

        let Some(module) = owner_guard.find_update_module("CleanupHazardUpdate") else {
            log::debug!("CleanupAreaPower: owner missing CleanupHazardUpdate module");
            return Ok(());
        };

        // Delegate to CleanupHazardUpdate::setCleanupAreaParameters(loc, range)
        // C++: update->setCleanupAreaParameters(loc, data->m_cleanupMoveRange)
        module.with_module_downcast::<crate::object::behavior::cleanup_hazard_update::CleanupHazardUpdate, _, _>(|behavior| {
            CleanupHazardUpdateInterface::set_cleanup_area_parameters(behavior, loc, move_range);
        });

        Ok(())
    }
}

impl Module for CleanupAreaPower {
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

impl Snapshotable for CleanupAreaPower {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("CleanupAreaPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ CleanupAreaPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for CleanupAreaPower {
    fn get_module_name(&self) -> &'static str {
        "CleanupAreaPower"
    }
}

// INI field parsers

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut CleanupAreaPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_max_move_distance_from_location(
    _ini: &mut INI,
    data: &mut CleanupAreaPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.cleanup_move_range = INI::parse_real(token)?;
    Ok(())
}

const CLEANUP_AREA_POWER_FIELDS: &[FieldParse<CleanupAreaPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "MaxMoveDistanceFromLocation",
        parse: parse_max_move_distance_from_location,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cleanup_area_default() {
        let data = CleanupAreaPowerModuleData::default();
        assert_eq!(data.cleanup_move_range, 0.0);
    }

    #[test]
    fn test_cleanup_area_module_name() {
        let data = CleanupAreaPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = CleanupAreaPower::new(0, 0, arc_data);
        assert_eq!(power.get_module_name(), "CleanupAreaPower");
    }

    #[test]
    fn test_do_special_power_at_location_no_owner() {
        let data = CleanupAreaPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = CleanupAreaPower::new(0, 0, arc_data);
        // Should return Ok without panicking when owner doesn't exist
        assert!(power
            .do_special_power_at_location(&Coord3D::new(0.0, 0.0, 0.0))
            .is_ok());
    }
}
