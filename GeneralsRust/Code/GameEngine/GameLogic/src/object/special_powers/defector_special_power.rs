//! DefectorSpecialPower
//!
//! Port of DefectorSpecialPower.h and DefectorSpecialPower.cpp
//! Author: Mark Lorenzen (C++), Rust Port
//!
//! Allows the player to convert enemy units to their side by targeting them.
//! The targeted unit defects to the caster's team. Uses the SpecialPowerTemplate's
//! detection time for the defector duration.

use std::sync::{Arc, RwLock};

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;
use crate::team::Team;

/// Module data for DefectorSpecialPower.
/// Matches C++ DefectorSpecialPowerModuleData.
#[derive(Debug, Clone)]
pub struct DefectorSpecialPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// The radius around the target for cursor display.
    /// Matches C++ m_fatCursorRadius.
    pub fat_cursor_radius: Real,
}

impl Default for DefectorSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            fat_cursor_radius: 0.0,
        }
    }
}

impl DefectorSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DEFECTOR_SPECIAL_POWER_FIELDS)
    }
}

impl ModuleData for DefectorSpecialPowerModuleData {
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

impl Snapshotable for DefectorSpecialPowerModuleData {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

/// DefectorSpecialPower module.
///
/// Matches C++ DefectorSpecialPower which extends SpecialPowerModule.
/// When activated on an enemy object, the target defects to the caster's team.
pub struct DefectorSpecialPower {
    module_name_key: NameKeyType,
    data: Arc<DefectorSpecialPowerModuleData>,
    owner_object_id: ObjectID,
}

impl DefectorSpecialPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<DefectorSpecialPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Execute the defector power on a target object.
    /// Matches C++ DefectorSpecialPower::doSpecialPowerAtObject().
    pub fn do_special_power_at_object(&self, target_object_id: ObjectID) -> Result<(), String> {
        // Check if the owner is disabled
        if let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) {
            if let Ok(owner_guard) = owner.read() {
                if owner_guard.is_disabled() {
                    return Ok(());
                }
            }
        }

        // Sanity checks
        let Some(target) = TheGameLogic::find_object_by_id(target_object_id) else {
            return Ok(());
        };
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };

        // Get the detection time from the special power template
        let detection_time = self
            .data
            .base
            .special_power_template
            .as_ref()
            .map(|t| t.get_detection_time())
            .unwrap_or(0);

        // Get the caster's default team
        let new_team = {
            let Ok(owner_guard) = owner.read() else {
                return Ok(());
            };
            let Some(player) = owner_guard.get_controlling_player() else {
                return Ok(());
            };
            let Ok(player_guard) = player.read() else {
                return Ok(());
            };
            player_guard.get_default_team()
        };

        // Make the target defect to the caster's team
        // Matches C++: objectToMakeDefector->defect(self->getControllingPlayer()->getDefaultTeam(), time)
        if let Some(team) = new_team {
            if let Ok(mut target_guard) = target.write() {
                target_guard.defect(Some(team), detection_time);
            }
        }

        Ok(())
    }

    /// Execute the defector power at a location - returns immediately.
    /// Matches C++ DefectorSpecialPower::doSpecialPowerAtLocation() which only allows targeting objects.
    pub fn do_special_power_at_location(
        &self,
        _location: &crate::common::Coord3D,
    ) -> Result<(), String> {
        // C++: "only allowed at objects" - returns immediately
        Ok(())
    }
}

impl Module for DefectorSpecialPower {
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

impl Snapshotable for DefectorSpecialPower {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        // Version 1: Initial version - extends base class only
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("DefectorSpecialPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ DefectorSpecialPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for DefectorSpecialPower {
    fn get_module_name(&self) -> &'static str {
        "DefectorSpecialPower"
    }
}

// INI field parsers

fn parse_fat_cursor_radius(
    _ini: &mut INI,
    data: &mut DefectorSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.fat_cursor_radius = INI::parse_real(token)?;
    Ok(())
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut DefectorSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

const DEFECTOR_SPECIAL_POWER_FIELDS: &[FieldParse<DefectorSpecialPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "FatCursorRadius",
        parse: parse_fat_cursor_radius,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defector_default() {
        let data = DefectorSpecialPowerModuleData::default();
        assert_eq!(data.fat_cursor_radius, 0.0);
    }

    #[test]
    fn test_do_special_power_at_location_returns_ok() {
        let data = DefectorSpecialPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = DefectorSpecialPower::new(0, 0, arc_data);
        let loc = crate::common::Coord3D::new(0.0, 0.0, 0.0);
        // Should return Ok (does nothing - only objects allowed)
        assert!(power.do_special_power_at_location(&loc).is_ok());
    }
}
