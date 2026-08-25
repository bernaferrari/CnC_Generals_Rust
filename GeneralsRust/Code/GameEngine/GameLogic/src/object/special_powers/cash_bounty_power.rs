//! CashBountyPower
//!
//! Port of CashBountyPower.h and CashBountyPower.cpp
//! Author: Steven Johnson (C++), Rust Port
//!
//! Sets the player's cash bounty percentage when the object is created.
//! When enemy units are killed, the controlling player receives a percentage
//! of the killed unit's build cost as cash.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INI, INIError};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::science::ScienceType;
use crate::common::{ObjectID, Real};
use crate::helpers::TheGameLogic;
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;

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
    base_module: crate::object::special_power_module::SpecialPowerModule,
}

impl CashBountyPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<CashBountyPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            owner_object_id,
            base_module: super::interface::make_base_module(owner_object_id, &data.base),
            data,
        }
    }

    /// Find the bounty value for the current player.
    /// In C++ this checks upgrade pairs, but that feature is #ifdef NOT_IN_USE.
    /// Matches C++ CashBountyPower::findBounty().
    fn find_bounty(&self) -> Real {
        self.data.default_bounty
    }

    /// Apply bounty with no science gate (C++ onSpecialPowerCreation).
    fn apply_bounty(&self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return;
        };
        let Ok(owner_guard) = owner.read() else {
            return;
        };
        let Some(player) = owner_guard.get_controlling_player() else {
            return;
        };
        let bounty = self.find_bounty();
        if let Ok(mut player_write) = player.write() {
            if bounty > player_write.get_cash_bounty() {
                player_write.set_cash_bounty(bounty);
            }
        }
    }

    /// Apply the bounty if the player already has the required science.
    /// Matches C++ CashBountyPower::onObjectCreated().
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

        let required_science = self
            .data
            .base
            .special_power_template
            .as_ref()
            .map(|t| t.get_required_science())
            .unwrap_or(ScienceType::default());
        if player_guard.has_science(required_science) {
            drop(player_guard);
            self.apply_bounty();
        }
    }

    fn dispatch_do_special_power(
        &mut self,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) {
        // C++ CashBountyPower does not override doSpecialPower*.
        self.base_module.do_special_power(command_options);
    }

    fn dispatch_do_special_power_at_object(
        &mut self,
        object_id: crate::object::special_power_module::ObjectId,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) {
        self.base_module
            .do_special_power_at_object(object_id, command_options);
    }

    fn dispatch_do_special_power_at_location(
        &mut self,
        location: &crate::common::Coord3D,
        angle: f32,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) {
        self.base_module
            .do_special_power_at_location(location, angle, command_options);
    }

    fn dispatch_reference_thing_template(&self) -> Option<String> {
        None
    }

    fn dispatch_on_special_power_creation(&mut self) {
        self.apply_bounty();
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
        self.base_module.initialize_from_owner();
        self.apply_bounty_if_applicable();
    }
}

impl BehaviorModuleInterface for CashBountyPower {
    fn get_module_name(&self) -> &'static str {
        "CashBountyPower"
    }
    fn get_special_power_module_interface(
        &mut self,
    ) -> Option<&mut dyn crate::modules::SpecialPowerModuleInterface> {
        Some(self)
    }
    fn get_special_power_module_interface_const(
        &self,
    ) -> Option<&dyn crate::modules::SpecialPowerModuleInterface> {
        Some(self)
    }
}
super::interface::impl_special_power_subclass!(CashBountyPower);

impl Snapshotable for CashBountyPower {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        super::interface::xfer_special_power_subclass(
            &mut self.base_module,
            xfer,
            "CashBountyPower",
        )
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        super::interface::load_post_process_special_power_subclass(&mut self.base_module)
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
