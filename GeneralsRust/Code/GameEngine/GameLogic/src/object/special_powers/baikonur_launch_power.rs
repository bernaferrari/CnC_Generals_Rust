// FILE: baikonur_launch_power.rs
// Port of BaikonurLaunchPower.h and BaikonurLaunchPower.cpp
// Author: Rust Port
// Desc: Triggers the Baikonur launch sequence (scripted end-game power).

use crate::common::{AsciiString, Coord3D};
use crate::helpers::TheThingFactory;
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface,
};
use crate::object::special_power_template::{
    find_or_create_special_power_template, AudioEventRts, SpecialPowerTemplate,
};
use crate::object_creation_list::nuggets::INVALID_ANGLE;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

#[derive(Debug, Clone)]
pub struct BaikonurLaunchPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    pub base: SpecialPowerModuleData,
    pub detonation_object: AsciiString,
}

impl Default for BaikonurLaunchPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: SpecialPowerModuleData::default(),
            detonation_object: AsciiString::new(),
        }
    }
}

impl BaikonurLaunchPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, BAIKONUR_LAUNCH_POWER_FIELDS)
    }
}

impl Snapshotable for BaikonurLaunchPowerModuleData {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

crate::impl_legacy_module_data_with_key_field!(BaikonurLaunchPowerModuleData, module_tag_name_key);

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
    data.base.initiate_sound = crate::common::audio::AudioEventRts::new(*token);
    Ok(())
}

fn parse_detonation_object(
    _ini: &mut INI,
    data: &mut BaikonurLaunchPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.detonation_object = AsciiString::from(*token);
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
        parse: parse_detonation_object,
    },
];

#[derive(Debug)]
pub struct BaikonurLaunchPower {
    base: SpecialPowerModule,
    data: Arc<BaikonurLaunchPowerModuleData>,
}

impl BaikonurLaunchPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_id: ObjectId,
        data: Arc<BaikonurLaunchPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_id, data.base.clone());
        Self { base, data }
    }

    fn spawn_detonation(&self, loc: &Coord3D) {
        if self.data.detonation_object.is_empty() {
            return;
        }

        let template = match TheThingFactory::find_template(self.data.detonation_object.as_str()) {
            Some(template) => template,
            None => return,
        };

        let Some(owner_arc) =
            crate::helpers::TheGameLogic::find_object_by_id(self.base.get_owner_object_id())
        else {
            return;
        };
        let Ok(owner_guard) = owner_arc.read() else {
            return;
        };
        let Some(team) = owner_guard.get_team() else {
            return;
        };
        let Ok(team_guard) = team.read() else {
            return;
        };

        let Ok(factory) = TheThingFactory::get() else {
            return;
        };
        if let Ok(detonation) = factory.new_object(template.clone(), &*team_guard) {
            if let Ok(mut detonation_guard) = detonation.write() {
                let _ = detonation_guard.set_position(loc);
            }
        }
    }
}

impl SpecialPowerModuleInterface for BaikonurLaunchPower {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        self.base.is_module_for_power(special_power_template)
    }

    fn get_percent_ready(&self) -> f32 {
        SpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn get_power_name(&self) -> String {
        SpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.base.get_special_power_template_full()
    }

    fn get_required_science(&self) -> crate::common::science::ScienceType {
        self.base.get_required_science()
    }

    fn on_special_power_creation(&mut self) {
        self.base.on_special_power_creation()
    }

    fn set_ready_frame(&mut self, frame: FrameCount) {
        self.base.set_ready_frame(frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        SpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        if let Some(owner_arc) =
            crate::helpers::TheGameLogic::find_object_by_id(self.base.get_owner_object_id())
        {
            if let Ok(mut owner_guard) = owner_arc.write() {
                if owner_guard.is_disabled() {
                    return;
                }
                self.base.do_special_power(command_options);
                owner_guard
                    .set_model_condition_state(crate::common::ModelConditionFlags::DOOR_1_OPENING);
                return;
            }
        }
        self.base.do_special_power(command_options);
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        let Some(obj_arc) = crate::object::registry::OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };
        let pos = *obj_guard.get_position();
        SpecialPowerModuleInterface::do_special_power_at_location(
            self,
            &pos,
            INVALID_ANGLE,
            command_options,
        );
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(owner_arc) =
            crate::helpers::TheGameLogic::find_object_by_id(self.base.get_owner_object_id())
        {
            if let Ok(owner_guard) = owner_arc.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        self.base
            .do_special_power_at_location(location, angle, command_options);
        self.spawn_detonation(location);
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &crate::waypoint::Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        self.base
            .do_special_power_using_waypoints(waypoint, command_options)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        self.base.mark_special_power_triggered(location)
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        self.base.start_power_recharge_at(current_frame)
    }

    fn get_initiate_sound(&self) -> &AudioEventRts {
        self.base.get_initiate_sound()
    }

    fn is_script_only(&self) -> bool {
        self.base.is_script_only()
    }

    fn get_reference_thing_template(&self) -> Option<String> {
        self.base.get_reference_thing_template()
    }
}

impl EngineSpecialPowerModuleInterface for BaikonurLaunchPower {
    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.activate()
    }

    fn can_activate(&self) -> bool {
        self.base.can_activate()
    }

    fn get_power_type(&self) -> u32 {
        self.base.get_power_type()
    }

    fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.base.start_power_recharge()
    }

    fn get_ready_frame(&self) -> u32 {
        self.base.get_ready_frame()
    }

    fn is_ready(&self) -> bool {
        self.base.is_ready()
    }

    fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>> {
        self.base.get_special_power_template()
    }

    fn get_power_name(&self) -> String {
        self.base.get_power_name()
    }

    fn get_percent_ready(&self) -> f32 {
        self.base.get_percent_ready()
    }

    fn pause_countdown(&mut self, pause: bool) {
        self.base.pause_countdown(pause)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        self.base.mark_special_power_triggered(location)
    }
}

impl Snapshotable for BaikonurLaunchPower {
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

impl Module for BaikonurLaunchPower {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }

    fn get_module_name_key(&self) -> NameKeyType {
        self.base.get_module_name_key()
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        self.data.module_tag_name_key
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for BaikonurLaunchPower {
    fn get_special_power(&mut self) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface(
        &mut self,
    ) -> Option<&mut dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }

    fn get_special_power_module_interface_const(
        &self,
    ) -> Option<&dyn EngineSpecialPowerModuleInterface> {
        Some(self)
    }
}

impl crate::modules::BehaviorModule for BaikonurLaunchPower {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    fn on_destroy(&mut self) {
        self.base.on_destroy();
    }
}
