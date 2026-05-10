//! BaikonurLaunchPower
//!
//! Port of C++ `Object/SpecialPower/BaikonurLaunchPower.cpp`.

use std::sync::Arc;

use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};

use crate::common::{AsciiString, Coord3D, ModelConditionFlags, ObjectID};
use crate::helpers::{TheGameLogic, TheThingFactory};
use crate::modules::BehaviorModuleInterface;
use crate::object::special_power_module::{
    SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
};
use crate::object::special_power_template::{find_or_create_special_power_template, AudioEventRts};

#[derive(Debug, Clone)]
pub struct BaikonurLaunchPowerModuleData {
    pub base: SpecialPowerModuleData,
    pub detonation_object: AsciiString,
}

impl Default for BaikonurLaunchPowerModuleData {
    fn default() -> Self {
        Self {
            base: SpecialPowerModuleData::default(),
            detonation_object: AsciiString::default(),
        }
    }
}

impl BaikonurLaunchPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BAIKONUR_LAUNCH_POWER_FIELDS)
    }
}

impl ModuleData for BaikonurLaunchPowerModuleData {
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

impl Snapshotable for BaikonurLaunchPowerModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base.load_post_process()
    }
}

pub struct BaikonurLaunchPower {
    module_name_key: NameKeyType,
    data: Arc<BaikonurLaunchPowerModuleData>,
    owner_object_id: ObjectID,
    base_module: SpecialPowerModule,
}

impl BaikonurLaunchPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectID,
        data: Arc<BaikonurLaunchPowerModuleData>,
    ) -> Self {
        Self {
            module_name_key,
            owner_object_id,
            base_module: SpecialPowerModule::new(owner_object_id, data.base.clone()),
            data,
        }
    }

    fn owner_is_disabled(&self) -> bool {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return false;
        };
        owner
            .read()
            .map(|guard| guard.is_disabled())
            .unwrap_or(false)
    }

    fn open_launch_door(&self) {
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner.write() else {
            return;
        };
        owner_guard.set_model_condition_state(ModelConditionFlags::DOOR_1_OPENING);
    }

    pub fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        if self.owner_is_disabled() {
            return;
        }

        self.base_module.do_special_power(command_options);
        self.open_launch_door();
    }

    pub fn do_special_power_at_location(
        &mut self,
        loc: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) -> Result<(), String> {
        if self.owner_is_disabled() {
            return Ok(());
        }

        self.base_module
            .do_special_power_at_location(loc, angle, command_options);
        self.spawn_detonation(loc)
    }

    fn spawn_detonation(&self, loc: &Coord3D) -> Result<(), String> {
        let Some(template) = TheThingFactory::find_template(self.data.detonation_object.as_str())
        else {
            return Ok(());
        };
        let Some(owner) = TheGameLogic::find_object_by_id(self.owner_object_id) else {
            return Ok(());
        };
        let owner_guard = owner
            .read()
            .map_err(|_| "BaikonurLaunchPower owner lock poisoned".to_string())?;
        let Some(team) = owner_guard.get_team() else {
            return Ok(());
        };
        let team_guard = team
            .read()
            .map_err(|_| "BaikonurLaunchPower team lock poisoned".to_string())?;
        let factory = TheThingFactory::get().map_err(|err| err.to_string())?;
        let detonation = factory
            .new_object(template, &team_guard)
            .map_err(|err| err.to_string())?;
        detonation
            .write()
            .map_err(|_| "BaikonurLaunchPower detonation lock poisoned".to_string())?
            .set_position(loc)?;
        Ok(())
    }
}

impl Module for BaikonurLaunchPower {
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

impl Snapshotable for BaikonurLaunchPower {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.base_module.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| format!("BaikonurLaunchPower xfer version failed: {err:?}"))?;
        self.base_module.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.base_module.load_post_process()
    }
}

impl BehaviorModuleInterface for BaikonurLaunchPower {
    fn get_module_name(&self) -> &'static str {
        "BaikonurLaunchPower"
    }
}

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut BaikonurLaunchPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    let name = AsciiString::from(*token);
    data.base.special_power_template = Some(find_or_create_special_power_template(&name));
    Ok(())
}

fn parse_bool_field(setter: &mut dyn FnMut(bool), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_bool(token)?);
    Ok(())
}

fn parse_audio_event(
    _ini: &mut INI,
    data: &mut BaikonurLaunchPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = AudioEventRts::new(*token);
    Ok(())
}

fn parse_ascii_string_field(
    setter: &mut dyn FnMut(AsciiString),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(AsciiString::from(*token));
    Ok(())
}

const BAIKONUR_LAUNCH_POWER_FIELDS: &[FieldParse<BaikonurLaunchPowerModuleData>] = &[
    FieldParse {
        token: "SpecialPowerTemplate",
        parse: parse_special_power_template_field,
    },
    FieldParse {
        token: "UpdateModuleStartsAttack",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.base.update_module_starts_attack = v, tokens)
        },
    },
    FieldParse {
        token: "StartsPaused",
        parse: |_, data, tokens| parse_bool_field(&mut |v| data.base.starts_paused = v, tokens),
    },
    FieldParse {
        token: "InitiateSound",
        parse: parse_audio_event,
    },
    FieldParse {
        token: "ScriptedSpecialPowerOnly",
        parse: |_, data, tokens| {
            parse_bool_field(&mut |v| data.base.scripted_special_power_only = v, tokens)
        },
    },
    FieldParse {
        token: "DetonationObject",
        parse: |_, data, tokens| {
            parse_ascii_string_field(&mut |v| data.detonation_object = v, tokens)
        },
    },
];
