//! CashBountyPower
//!
//! Port of CashBountyPower.h and CashBountyPower.cpp
//! Author: Steven Johnson (C++), Rust Port
//!
//! Sets the player's cash bounty percentage when the object is created.
//! When enemy units are killed, the controlling player receives a percentage
//! of the killed unit's build cost as cash.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::science::ScienceType;
use crate::common::{ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;
use crate::player::player_list;

/// Module data for CashBountyPower.
/// Matches C++ CashBountyPowerModuleData.
#[derive(Debug, Clone)]
pub struct CashBountyPowerModuleData {
    pub base: SpecialPowerModuleData,
    /// Cash bounty percentage (parsed via INI::parsePercentToReal, 0.0 - 1.0)
    pub default_bounty: Real,
}

impl Default for CashBountyPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            default_bounty: 0.0,
        }
    }
}

impl CashBountyPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, CASH_BOUNTY_POWER_FIELDS)
    }
}

impl ModuleData for CashBountyPowerModuleData {
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

impl Snapshotable for CashBountyPowerModuleData {
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

/// CashBountyPower module.
///
/// Matches C++ CashBountyPower which extends SpecialPowerModule.
/// When the owning object is created, it sets the controlling player's
/// cash bounty to the configured percentage (if the player has the
/// required science).
pub struct CashBountyPower {
    module_name_key: NameKeyType,
    data: Arc<CashBountyPowerModuleData>,
    owner_object_id: ObjectID,
}

impl CashBountyPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<CashBountyPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    /// Find the bounty value for the current player.
    /// In C++ this checks upgrade pairs, but that feature is #ifdef NOT_IN_USE.
    /// Matches C++ CashBountyPower::findBounty().
    fn find_bounty(&self) -> Real {
        self.data.default_bounty
    }

    /// Apply the bounty to the controlling player.
    /// Matches C++ CashBountyPower::onObjectCreated() and onSpecialPowerCreation().
    fn apply_bounty_if_applicable(&self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(player) = owner_guard.get_controlling_player() else {
            return;
        };
        let Ok(player_guard) = player.read() else {
            return;
        };

        // Check if the player has the required science
        let required_science = self
            .data
            .base
            .special_power_template
            .as_ref()
            .map(|t| t.get_required_science())
            .unwrap_or(ScienceType::default());
        if player_guard.has_science(required_science) {
            let bounty = self.find_bounty();
            drop(player_guard);
            if let Ok(mut player_write) = player.write() {
                if bounty > player_write.get_cash_bounty() {
                    player_write.set_cash_bounty(bounty);
                }
            }
        }
    }
}

impl Module for CashBountyPower {
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

    fn on_object_created(&mut self) {
        // Matches C++ CashBountyPower::onObjectCreated()
        self.apply_bounty_if_applicable();
    }
}

impl Snapshotable for CashBountyPower {
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
            .map_err(|e| format!("CashBountyPower xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        // Matches C++ CashBountyPower::loadPostProcess()
        // After loading, reapply bounty for the loaded player
        self.apply_bounty_if_applicable();
        Ok(())
    }
}

impl BehaviorModuleInterface for CashBountyPower {
    fn get_module_name(&self) -> &'static str {
        "CashBountyPower"
    }
}

// INI field parsers

fn parse_bounty(
    _ini: &mut INI,
    data: &mut CashBountyPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .find(|t| **t != "=")
        .ok_or(INIError::InvalidData)?;
    // C++ uses INI::parsePercentToReal which converts percentage (e.g., "20%") to real (0.2)
    data.default_bounty = INI::parse_percent_to_real(token)?;
    Ok(())
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut CashBountyPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

const CASH_BOUNTY_POWER_FIELDS: &[FieldParse<CashBountyPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "Bounty",
        parse: parse_bounty,
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cash_bounty_default() {
        let data = CashBountyPowerModuleData::default();
        assert_eq!(data.default_bounty, 0.0);
    }

    #[test]
    fn test_find_bounty() {
        let mut data = CashBountyPowerModuleData::default();
        data.default_bounty = 0.2;
        let arc_data = Arc::new(data);
        let power = CashBountyPower::new(0, 0, arc_data);
        assert!((power.find_bounty() - 0.2).abs() < f32::EPSILON);
    }
}
