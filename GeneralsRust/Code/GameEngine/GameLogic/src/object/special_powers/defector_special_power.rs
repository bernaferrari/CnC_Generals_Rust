// FILE: defector_special_power.rs
// Port of DefectorSpecialPower.h and DefectorSpecialPower.cpp
// Author: Rust Port
// Desc: General can click command cursor on any enemy, and it becomes theirs (defection power)

use crate::common::science::ScienceType;
use crate::common::{AsciiString, Coord3D, LegacyModuleData};
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface as ObjSpecialPowerModuleInterface, Waypoint,
};
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::special_power_template::SpecialPowerTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for defector special power
#[derive(Debug, Clone)]
pub struct DefectorSpecialPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Base special power data
    pub base: SpecialPowerModuleData,

    /// Radius around target to reveal (fat cursor)
    pub fat_cursor_radius: f32,
}

impl Default for DefectorSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
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

impl Snapshotable for DefectorSpecialPowerModuleData {
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

crate::impl_legacy_module_data_with_key_field!(DefectorSpecialPowerModuleData, module_tag_name_key);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut DefectorSpecialPowerModuleData,
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
    data: &mut DefectorSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = crate::common::audio::AudioEventRts::new(*token);
    Ok(())
}

fn parse_real_field(setter: &mut dyn FnMut(f32), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_real(token)?);
    Ok(())
}

const DEFECTOR_SPECIAL_POWER_FIELDS: &[FieldParse<DefectorSpecialPowerModuleData>] = &[
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
        token: "FatCursorRadius",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.fat_cursor_radius = v, tokens),
    },
];

/// Defector special power implementation
/// Allows a general to convert enemy units to their own side
#[derive(Debug, Clone)]
pub struct DefectorSpecialPower {
    /// Base special power module
    base: SpecialPowerModule,

    /// Defector-specific module data
    defector_data: Arc<DefectorSpecialPowerModuleData>,
    module_name_key: NameKeyType,
}

impl DefectorSpecialPower {
    /// Create a new defector special power
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectId,
        data: Arc<DefectorSpecialPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_object_id, data.base.clone());
        Self {
            base,
            defector_data: data,
            module_name_key,
        }
    }

    /// Get the defector module data
    pub fn get_defector_module_data(&self) -> &DefectorSpecialPowerModuleData {
        &self.defector_data
    }

    /// Make an object defect to our team
    fn make_object_defect(&self, object_id: ObjectId) {
        // Get detection time from template
        let detection_time = if let Some(template) = self.base.get_special_power_template_full() {
            template.get_detection_time()
        } else {
            0
        };

        let owner_team = OBJECT_REGISTRY
            .get_object(self.base.get_owner_object_id())
            .and_then(|arc| {
                let owner_guard = arc.read().ok()?;
                owner_guard
                    .get_controlling_player()
                    .and_then(|player| player.read().ok()?.get_default_team())
            });
        let Some(owner_team) = owner_team else {
            return;
        };

        let Some(target_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(mut target_guard) = target_arc.write() else {
            return;
        };
        target_guard.defect(Some(owner_team), detection_time);
    }
}

// Implement the special power module interface
impl ObjSpecialPowerModuleInterface for DefectorSpecialPower {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        self.base.is_module_for_power(special_power_template)
    }

    fn get_percent_ready(&self) -> f32 {
        crate::object::special_power_module::SpecialPowerModuleInterface::get_percent_ready(
            &self.base,
        )
    }

    fn get_power_name(&self) -> String {
        crate::object::special_power_module::SpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        self.base.get_special_power_template_full()
    }

    fn get_required_science(&self) -> ScienceType {
        self.base.get_required_science()
    }

    fn on_special_power_creation(&mut self) {
        self.base.on_special_power_creation()
    }

    fn set_ready_frame(&mut self, frame: u32) {
        self.base.set_ready_frame(frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        crate::object::special_power_module::SpecialPowerModuleInterface::pause_countdown(
            &mut self.base,
            pause,
        )
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        // Defector power requires a target object, not location-less
        self.base.do_special_power(command_options)
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

        // Sanity check - need valid object ID
        if object_id == 0 {
            return;
        }

        // Call base class to handle triggers
        self.base
            .do_special_power_at_object(object_id, command_options);

        // Make the target defect
        self.make_object_defect(object_id);
    }

    fn do_special_power_at_location(
        &mut self,
        _location: &Coord3D,
        _angle: f32,
        _command_options: SpecialPowerCommandOptions,
    ) {
        if let Some(owner_arc) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(owner_guard) = owner_arc.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        // Defector power is only allowed at objects, not locations
        // Do nothing
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Waypoints not used for defector power
        self.base
            .do_special_power_using_waypoints(waypoint, command_options)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        crate::object::special_power_module::SpecialPowerModuleInterface::mark_special_power_triggered(
            &mut self.base,
            location,
        )
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        self.base.start_power_recharge_at(current_frame)
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

impl EngineSpecialPowerModuleInterface for DefectorSpecialPower {
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

impl Snapshotable for DefectorSpecialPower {
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
        LegacyModuleData::get_module_tag_name_key(self.defector_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.defector_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for DefectorSpecialPower {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defector_special_power_creation() {
        let data = DefectorSpecialPowerModuleData::default();
        let power = DefectorSpecialPower::new(0, 1, Arc::new(data));

        assert!(power.is_ready());
    }

    #[test]
    fn test_defector_fat_cursor_radius() {
        let data = DefectorSpecialPowerModuleData {
            fat_cursor_radius: 150.0,
            ..Default::default()
        };

        let power = DefectorSpecialPower::new(0, 1, Arc::new(data));
        assert_eq!(power.get_defector_module_data().fat_cursor_radius, 150.0);
    }

    #[test]
    fn test_do_special_power_at_location_ignored() {
        let data = DefectorSpecialPowerModuleData::default();
        let mut power = DefectorSpecialPower::new(0, 1, Arc::new(data));

        // Should not panic - just returns without doing anything
        let location = Coord3D::new(100.0, 200.0, 0.0);
        crate::modules::SpecialPowerModuleInterface::do_special_power_at_location(
            &mut power,
            &location,
            0.0,
            SpecialPowerCommandOptions::empty(),
        );
    }
}
