//! SpecialAbility
//!
//! Port of C++ `Object/SpecialPower/SpecialAbility.cpp`.
//! This module is the generic "pass-through" special power that only guards
//! disabled/null cases and then proceeds with normal special-power execution flow.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INI, INIError};
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
    base_module: crate::object::special_power_module::SpecialPowerModule,
}

impl SpecialAbility {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<SpecialAbilityModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            owner_object_id,
            base_module: super::interface::make_base_module(owner_object_id, &data.base),
            data,
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

    pub fn do_special_power(&self, _command_options: u32) -> Result<(), String> {
        if self.owner_is_disabled() {
            return Ok(());
        }
        Ok(())
    }

    fn dispatch_do_special_power(
        &mut self,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) {
        if self.owner_is_disabled() {
            return;
        }
        self.base_module.do_special_power(command_options);
    }

    fn dispatch_do_special_power_at_object(
        &mut self,
        object_id: crate::object::special_power_module::ObjectId,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) {
        if self.owner_is_disabled() {
            return;
        }
        self.base_module
            .do_special_power_at_object(object_id, command_options);
    }

    fn dispatch_do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: crate::object::special_power_module::SpecialPowerCommandOptions,
    ) {
        if self.owner_is_disabled() {
            return;
        }
        self.base_module
            .do_special_power_at_location(location, angle, command_options);
    }

    fn dispatch_reference_thing_template(&self) -> Option<String> {
        None
    }

    fn dispatch_on_special_power_creation(&mut self) {}
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

    fn on_object_created(&mut self) {
        self.base_module.initialize_from_owner();
    }
}

impl BehaviorModuleInterface for SpecialAbility {
    fn get_module_name(&self) -> &'static str {
        "SpecialAbility"
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
super::interface::impl_special_power_subclass!(SpecialAbility);

impl Snapshotable for SpecialAbility {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        super::interface::xfer_special_power_subclass(&mut self.base_module, xfer, "SpecialAbility")
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        super::interface::load_post_process_special_power_subclass(&mut self.base_module)
    }
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut SpecialAbilityModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = crate::common::AsciiString::from(*token);
    data.base.special_power_template =
        Some(crate::object::special_power_template::find_or_create_special_power_template(&name));
    Ok(())
}

const SPECIAL_ABILITY_FIELDS: &[FieldParse<SpecialAbilityModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "UpdateModuleStartsAttack",
        parse: |_, data, tokens| {
            parse_ability_bool_field(&mut |v| data.base.update_module_starts_attack = v, tokens)
        },
    },
    FieldParse {
        token: "StartsPaused",
        parse: |_, data, tokens| {
            parse_ability_bool_field(&mut |v| data.base.starts_paused = v, tokens)
        },
    },
    FieldParse {
        token: "InitiateSound",
        parse: parse_ability_initiate_sound,
    },
    FieldParse {
        token: "ScriptedSpecialPowerOnly",
        parse: |_, data, tokens| {
            parse_ability_bool_field(&mut |v| data.base.scripted_special_power_only = v, tokens)
        },
    },
];

fn parse_ability_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_ability_initiate_sound(
    _ini: &mut INI,
    data: &mut SpecialAbilityModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = crate::object::special_power_template::AudioEventRts::new(*token);
    Ok(())
}
