// FILE: fire_weapon_power.rs
// Port of FireWeaponPower.h and FireWeaponPower.cpp
// Author: Rust Port
// Desc: Reloads ammo and orders AI to fire a weapon via special power

use crate::ai::CommandSourceType;
use crate::common::{AsciiString, Coord3D, LegacyModuleData, TurretType};
use crate::modules::AIUpdateInterfaceExt;
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface as ObjSpecialPowerModuleInterface, Waypoint,
};
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::special_power_template::SpecialPowerTemplate;
use game_engine::common::game_common::MAX_TURRETS;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for FireWeaponPower
#[derive(Debug, Clone)]
pub struct FireWeaponPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Base special power data
    pub base: SpecialPowerModuleData,
    /// Max shots to fire
    pub max_shots_to_fire: u32,
}

fn turret_type_for_index(index: usize) -> Option<TurretType> {
    match index {
        0 => Some(TurretType::Primary),
        1 => Some(TurretType::Secondary),
        _ => None,
    }
}

impl Default for FireWeaponPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: SpecialPowerModuleData::default(),
            max_shots_to_fire: 1,
        }
    }
}

impl FireWeaponPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, FIRE_WEAPON_POWER_FIELDS)
    }
}

impl Snapshotable for FireWeaponPowerModuleData {
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

crate::impl_legacy_module_data_with_key_field!(FireWeaponPowerModuleData, module_tag_name_key);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut FireWeaponPowerModuleData,
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
    data: &mut FireWeaponPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = crate::common::audio::AudioEventRts::new(*token);
    Ok(())
}

fn parse_unsigned_int_field(setter: &mut dyn FnMut(u32), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_unsigned_int(token)?);
    Ok(())
}

const FIRE_WEAPON_POWER_FIELDS: &[FieldParse<FireWeaponPowerModuleData>] = &[
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
        token: "MaxShotsToFire",
        parse: |_, data, tokens| {
            parse_unsigned_int_field(&mut |v| data.max_shots_to_fire = v, tokens)
        },
    },
];

/// Fire weapon special power implementation
#[derive(Debug, Clone)]
pub struct FireWeaponPower {
    base: SpecialPowerModule,
    fire_weapon_data: Arc<FireWeaponPowerModuleData>,
    module_name_key: NameKeyType,
}

impl FireWeaponPower {
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectId,
        data: Arc<FireWeaponPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_object_id, data.base.clone());
        Self {
            base,
            fire_weapon_data: data,
            module_name_key,
        }
    }

    fn reload_and_fire_at_location(&self, location: &Coord3D, target: Option<ObjectId>) {
        let owner_id = self.base.get_owner_object_id();
        let Some(owner_arc) = OBJECT_REGISTRY.get_object(owner_id) else {
            return;
        };
        let Ok(mut owner_guard) = owner_arc.write() else {
            return;
        };

        if owner_guard.is_disabled() {
            return;
        }

        let _ = owner_guard.reload_all_ammo(true);

        if let Some(ai) = owner_guard.get_ai() {
            let max_shots = self.fire_weapon_data.max_shots_to_fire as i32;
            match target {
                Some(target_id) => {
                    ai.ai_attack_object_id(target_id, max_shots, CommandSourceType::FromAi);
                    if let Some(target_arc) = OBJECT_REGISTRY.get_object(target_id) {
                        if let Ok(mut ai_guard) = ai.lock() {
                            for idx in 0..MAX_TURRETS {
                                if let Some(turret) = turret_type_for_index(idx) {
                                    ai_guard.set_turret_target_object(
                                        turret,
                                        Some(&target_arc),
                                        false,
                                    );
                                }
                            }
                        }
                    }
                }
                None => {
                    ai.ai_attack_position(location, max_shots, CommandSourceType::FromAi);
                    if let Ok(mut ai_guard) = ai.lock() {
                        for idx in 0..MAX_TURRETS {
                            if let Some(turret) = turret_type_for_index(idx) {
                                ai_guard.set_turret_target_position(turret, location);
                            }
                        }
                    }
                }
            }
        }
    }
}

impl ObjSpecialPowerModuleInterface for FireWeaponPower {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        self.base.is_module_for_power(special_power_template)
    }

    fn get_percent_ready(&self) -> f32 {
        ObjSpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn get_power_name(&self) -> String {
        ObjSpecialPowerModuleInterface::get_power_name(&self.base)
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

    fn set_ready_frame(&mut self, frame: u32) {
        self.base.set_ready_frame(frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        ObjSpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        if let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(owner_guard) = owner_arc.read() {
                if owner_guard.is_disabled() {
                    return;
                }
                let pos = *owner_guard.get_position();
                self.base.do_special_power(command_options);
                self.reload_and_fire_at_location(&pos, None);
            }
        }
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(owner_guard) = owner_arc.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        self.base
            .do_special_power_at_object(object_id, command_options);
        if let Some(target_arc) = OBJECT_REGISTRY.get_object(object_id) {
            if let Ok(target_guard) = target_arc.read() {
                let pos = *target_guard.get_position();
                self.reload_and_fire_at_location(&pos, Some(object_id));
            }
        }
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(owner_guard) = owner_arc.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        self.base
            .do_special_power_at_location(location, angle, command_options);
        self.reload_and_fire_at_location(location, None);
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        self.base
            .do_special_power_using_waypoints(waypoint, command_options)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        ObjSpecialPowerModuleInterface::mark_special_power_triggered(&mut self.base, location)
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        ObjSpecialPowerModuleInterface::start_power_recharge_at(&mut self.base, current_frame)
    }

    fn get_initiate_sound(&self) -> &crate::object::special_power_template::AudioEventRts {
        self.base.get_initiate_sound()
    }

    fn is_script_only(&self) -> bool {
        self.base.is_script_only()
    }

    fn get_reference_thing_template(&self) -> Option<String> {
        None
    }
}

impl EngineSpecialPowerModuleInterface for FireWeaponPower {
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

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.base.get_special_power_template_full()
    }

    fn get_power_name(&self) -> String {
        crate::modules::SpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_percent_ready(&self) -> f32 {
        crate::modules::SpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn pause_countdown(&mut self, pause: bool) {
        crate::modules::SpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&crate::common::Coord3D>) {
        crate::modules::SpecialPowerModuleInterface::mark_special_power_triggered(
            &mut self.base,
            location,
        )
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power(
            self,
            command_options,
        );
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_object(
            self,
            object_id,
            command_options,
        );
    }

    fn do_special_power_at_location(
        &mut self,
        location: &Coord3D,
        angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_at_location(
            self,
            location,
            angle,
            command_options,
        );
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        crate::object::special_power_module::SpecialPowerModuleInterface::do_special_power_using_waypoints(
            self,
            waypoint,
            command_options,
        );
    }
}

impl Snapshotable for FireWeaponPower {
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
        LegacyModuleData::get_module_tag_name_key(self.fire_weapon_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.fire_weapon_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for FireWeaponPower {
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
