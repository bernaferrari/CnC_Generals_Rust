//! FireWeaponPower
//!
//! Port of FireWeaponPower.h and FireWeaponPower.cpp
//! Author: Kris Morness, August 2003 (C++), Rust Port
//!
//! Simply loads and fires a specific weapon controlled by a superweapon timer.
//! When activated:
//! 1. Checks if owner is disabled
//! 2. Calls base class (SpecialPowerModule) to handle recharge/timer
//! 3. Reloads all ammunition
//! 4. Issues attack command to AI (position, object, or self)
//! 5. Sets turret target positions/objects

use std::sync::Arc;

use game_engine::common::game_common::MAX_TURRETS;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::ai::CommandSourceType;
use crate::common::{Coord3D, ObjectID, UnsignedInt};
use crate::helpers::TheGameLogic;
use crate::modules::{AIUpdateInterfaceExt, BehaviorModuleInterface};
use crate::object::special_power_module::SpecialPowerModuleData;

/// Module data for FireWeaponPower.
/// Matches C++ FireWeaponPowerModuleData.
#[derive(Debug, Clone)]
pub struct FireWeaponPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Maximum number of shots to fire when power is activated.
    /// Matches C++ m_maxShotsToFire (default 1).
    pub max_shots_to_fire: UnsignedInt,
}

impl Default for FireWeaponPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            max_shots_to_fire: 1, // Matches C++ constructor default
        }
    }
}

impl FireWeaponPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_WEAPON_POWER_FIELDS)
    }
}

impl ModuleData for FireWeaponPowerModuleData {
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

impl Snapshotable for FireWeaponPowerModuleData {
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

/// FireWeaponPower module.
///
/// Matches C++ FireWeaponPower which extends SpecialPowerModule.
/// Reloads ammo and fires weapons at a target position or object.
pub struct FireWeaponPower {
    module_name_key: NameKeyType,
    data: Arc<FireWeaponPowerModuleData>,
    owner_object_id: ObjectID,
}

impl FireWeaponPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<FireWeaponPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Reload all ammo and issue AI attack commands.
    fn fire_weapon_at_location(&self, loc: &Coord3D) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return;
        };

        // Reload all ammo (C++: self->reloadAllAmmo(TRUE))
        if let Ok(mut owner_guard) = owner.write() {
            let _ = owner_guard.reload_all_ammo(true);
        }

        // Get AI and issue attack commands via AIUpdateInterfaceExt
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                // C++: ai->aiAttackPosition(loc, maxShotsToFire, CMD_FROM_AI)
                ai.ai_attack_position(
                    loc,
                    self.data.max_shots_to_fire as i32,
                    CommandSourceType::FromAi,
                );
            }
        };
    }

    /// Execute fire weapon at location.
    /// Matches C++ FireWeaponPower::doSpecialPowerAtLocation().
    pub fn do_special_power_at_location(&self, loc: &Coord3D) -> Result<(), String> {
        // Check disabled
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        self.fire_weapon_at_location(loc);
        Ok(())
    }

    /// Execute fire weapon at an object.
    /// Matches C++ FireWeaponPower::doSpecialPowerAtObject().
    pub fn do_special_power_at_object(&self, obj_id: ObjectID) -> Result<(), String> {
        // Check disabled
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        // Reload all ammo
        if let Ok(mut owner_guard) = owner.write() {
            let _ = owner_guard.reload_all_ammo(true);
        }

        // Get AI and issue attack object command via AIUpdateInterfaceExt
        // C++: ai->aiAttackObject(obj, maxShotsToFire, CMD_FROM_AI)
        if let Ok(owner_guard) = owner.read() {
            if let Some(ai) = owner_guard.get_ai_update_interface() {
                ai.ai_attack_object_id(
                    obj_id,
                    self.data.max_shots_to_fire as i32,
                    CommandSourceType::FromAi,
                );
            }
        }

        Ok(())
    }

    /// Execute fire weapon with no target (fire at own position).
    /// Matches C++ FireWeaponPower::doSpecialPower().
    pub fn do_special_power(&self) -> Result<(), String> {
        // Check disabled
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        if let Ok(owner_guard) = owner.read() {
            if owner_guard.is_disabled() {
                return Ok(());
            }
        }

        // Get own position and fire at it
        let pos = if let Ok(owner_guard) = owner.read() {
            *owner_guard.get_position()
        } else {
            return Ok(());
        };

        self.fire_weapon_at_location(&pos);
        Ok(())
    }
}

impl Module for FireWeaponPower {
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

impl Snapshotable for FireWeaponPower {
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
            .map_err(|e| format!("FireWeaponPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ FireWeaponPower::loadPostProcess()
        Ok(())
    }
}

impl BehaviorModuleInterface for FireWeaponPower {
    fn get_module_name(&self) -> &'static str {
        "FireWeaponPower"
    }
}

// INI field parsers

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut FireWeaponPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_max_shots_to_fire(
    _ini: &mut INI,
    data: &mut FireWeaponPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    data.max_shots_to_fire = INI::parse_unsigned_int(token)?;
    Ok(())
}

const FIRE_WEAPON_POWER_FIELDS: &[FieldParse<FireWeaponPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "MaxShotsToFire",
        parse: parse_max_shots_to_fire,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fire_weapon_power_default() {
        let data = FireWeaponPowerModuleData::default();
        assert_eq!(data.max_shots_to_fire, 1);
    }

    #[test]
    fn test_fire_weapon_power_module_name() {
        let data = FireWeaponPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = FireWeaponPower::new(0, 0, arc_data);
        assert_eq!(power.get_module_name(), "FireWeaponPower");
    }

    #[test]
    fn test_do_special_power_no_owner() {
        let data = FireWeaponPowerModuleData::default();
        let arc_data = Arc::new(data);
        let power = FireWeaponPower::new(0, 0, arc_data);
        // Should return Ok without panicking when owner doesn't exist
        assert!(power.do_special_power().is_ok());
        assert!(power
            .do_special_power_at_location(&Coord3D::new(0.0, 0.0, 0.0))
            .is_ok());
        assert!(power.do_special_power_at_object(999).is_ok());
    }
}
