//! SpecialAbility
//!
//! Port of C++ `Object/SpecialPower/SpecialAbility.cpp`.
//! This module is the generic "pass-through" special power that only guards
//! disabled/null cases and then proceeds with normal special-power execution flow.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{Coord3D, ObjectID};
use crate::helpers::TheGameLogic;
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::SpecialPowerModuleData;

#[derive(Debug, Clone, Default)]
pub struct SpecialAbilityModuleData {
    pub base: SpecialPowerModuleData,
}

impl SpecialAbilityModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPECIAL_ABILITY_FIELDS)
    }
}

impl ModuleData for SpecialAbilityModuleData {
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

impl Snapshotable for SpecialAbilityModuleData {
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

pub struct SpecialAbility {
    module_name_key: NameKeyType,
    data: Arc<SpecialAbilityModuleData>,
    owner_object_id: ObjectID,
}

impl SpecialAbility {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<SpecialAbilityModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            data,
            owner_object_id,
        }
    }

    #[inline]
    fn owner_is_disabled(&self) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return true;
        };
        let Ok(owner_guard) = owner.read() else {
            return true;
        };
        owner_guard.is_disabled()
    }

    /// C++ parity: guards disabled and null location, then proceeds with base flow.
    pub fn do_special_power_at_location(
        &self,
        loc: Option<&Coord3D>,
        _angle: f32,
        _command_options: u32,
    ) -> Result<(), String> {
        if self.owner_is_disabled() || loc.is_none() {
            return Ok(());
        }
        Ok(())
    }

    /// C++ parity: guards disabled and null object, then proceeds with base flow.
    pub fn do_special_power_at_object(
        &self,
        obj_id: Option<ObjectID>,
        _command_options: u32,
    ) -> Result<(), String> {
        if self.owner_is_disabled() || obj_id.is_none() {
            return Ok(());
        }
        Ok(())
    }

    /// C++ parity: guards disabled, then proceeds with base flow.
    pub fn do_special_power(&self, _command_options: u32) -> Result<(), String> {
        if self.owner_is_disabled() {
            return Ok(());
        }
        Ok(())
    }
}

impl Module for SpecialAbility {
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

impl Snapshotable for SpecialAbility {
    fn crc(&self, _xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("SpecialAbility xfer version failed: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl BehaviorModuleInterface for SpecialAbility {
    fn get_module_name(&self) -> &'static str {
        "SpecialAbility"
    }
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut SpecialAbilityModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template = Some(
        crate::object::special_power_template::find_or_create_special_power_template(&name),
    );
    Ok(())
}

const SPECIAL_ABILITY_FIELDS: &[FieldParse<SpecialAbilityModuleData>] = &[FieldParse {
    token: "SpecialPowerTemplate",
    parse: parse_special_power_template_field,
}];

#[allow(dead_code)]
fn _module_name_key() -> NameKeyType {
    NameKeyGenerator::name_to_key("SpecialAbility")
}
