// FILE: spy_vision_special_power.rs
// Port of SpyVisionSpecialPower.h and SpyVisionSpecialPower.cpp
// Author: Rust Port
// Desc: Special Power will spy on the vision of all enemy players

use crate::common::science::ScienceType;
use crate::common::{AsciiString, Coord3D, LegacyModuleData};
use crate::modules::SpecialPowerModuleInterface as EngineSpecialPowerModuleInterface;
use crate::object::special_power_module::{
    FrameCount, ObjectId, SpecialPowerCommandOptions, SpecialPowerModule, SpecialPowerModuleData,
    SpecialPowerModuleInterface as ObjSpecialPowerModuleInterface, Waypoint,
};

use crate::object::registry::OBJECT_REGISTRY;
use crate::object::special_power_template::find_or_create_special_power_template;
use crate::object::special_power_template::SpecialPowerTemplate;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData, NameKeyType};
use std::sync::Arc;

/// Module data for spy vision special power
#[derive(Debug, Clone)]
pub struct SpyVisionSpecialPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Base special power data
    pub base: SpecialPowerModuleData,

    /// Base duration in frames
    pub base_duration_in_frames: u32,

    /// Additional duration per captured unit (in prison transport)
    pub bonus_duration_per_captured_in_frames: u32,

    /// Maximum duration regardless of captured units
    pub max_duration_in_frames: u32,
}

impl Default for SpyVisionSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: SpecialPowerModuleData::default(),
            base_duration_in_frames: 0,
            bonus_duration_per_captured_in_frames: 0,
            max_duration_in_frames: 0,
        }
    }
}

impl SpyVisionSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, SPY_VISION_SPECIAL_POWER_FIELDS)
    }
}

impl Snapshotable for SpyVisionSpecialPowerModuleData {
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

crate::impl_legacy_module_data_with_key_field!(
    SpyVisionSpecialPowerModuleData,
    module_tag_name_key
);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut SpyVisionSpecialPowerModuleData,
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
    data: &mut SpyVisionSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.base.initiate_sound = crate::common::audio::AudioEventRts::new(*token);
    Ok(())
}

fn parse_duration_field(setter: &mut dyn FnMut(u32), tokens: &[&str]) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

const SPY_VISION_SPECIAL_POWER_FIELDS: &[FieldParse<SpyVisionSpecialPowerModuleData>] = &[
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
        token: "BaseDuration",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |v| data.base_duration_in_frames = v, tokens)
        },
    },
    FieldParse {
        token: "BonusDurationPerCaptured",
        parse: |_, data, tokens| {
            parse_duration_field(
                &mut |v| data.bonus_duration_per_captured_in_frames = v,
                tokens,
            )
        },
    },
    FieldParse {
        token: "MaxDuration",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |v| data.max_duration_in_frames = v, tokens)
        },
    },
];

/// Spy vision special power implementation
/// Reveals enemy player vision for a duration
/// Duration increases based on number of captured units in the prison transport
#[derive(Debug, Clone)]
pub struct SpyVisionSpecialPower {
    /// Base special power module
    base: SpecialPowerModule,

    /// Spy vision-specific module data
    spy_vision_data: Arc<SpyVisionSpecialPowerModuleData>,
    module_name_key: NameKeyType,
}

impl SpyVisionSpecialPower {
    /// Create a new spy vision special power
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectId,
        data: Arc<SpyVisionSpecialPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_object_id, data.base.clone());
        Self {
            base,
            spy_vision_data: data,
            module_name_key,
        }
    }

    /// Get the spy vision module data
    pub fn get_spy_vision_module_data(&self) -> &SpyVisionSpecialPowerModuleData {
        &self.spy_vision_data
    }

    /// Calculate duration based on captured units
    fn calculate_duration(&self) -> u32 {
        let mut duration = self.spy_vision_data.base_duration_in_frames;

        if let Some(obj) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(guard) = obj.read() {
                if let Some(contain) = guard.get_contain() {
                    if let Ok(contain_guard) = contain.lock() {
                        let captured_count = contain_guard.get_contained_count() as u32;
                        duration += captured_count
                            * self.spy_vision_data.bonus_duration_per_captured_in_frames;
                    }
                }
            }
        }

        // Cap at maximum
        if duration > self.spy_vision_data.max_duration_in_frames {
            duration = self.spy_vision_data.max_duration_in_frames;
        }

        duration
    }

    /// Activate spy vision update module
    fn activate_spy_vision(&self, duration: u32) {
        if let Some(obj) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(guard) = obj.read() {
                if let Some(module) = guard.find_update_module("SpyVisionUpdate") {
                    let _ = module.with_module_downcast::<
                        crate::object::update::spy_vision_update::SpyVisionUpdateModule,
                        _,
                        _,
                    >(|module| {
                        module.behavior_mut().activate_spy_vision(duration);
                    });
                } else {
                    log::warn!(
                        "SpyVisionUpdate module not found on object {}",
                        self.base.get_owner_object_id()
                    );
                }
            }
        }
    }

    /// Get owner object ID
    fn get_owner_object_id(&self) -> ObjectId {
        self.base.get_owner_object_id()
    }
}

// Implement the special power module interface
impl ObjSpecialPowerModuleInterface for SpyVisionSpecialPower {
    fn is_module_for_power(&self, special_power_template: &SpecialPowerTemplate) -> bool {
        ObjSpecialPowerModuleInterface::is_module_for_power(&self.base, special_power_template)
    }

    fn get_percent_ready(&self) -> f32 {
        ObjSpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn get_power_name(&self) -> String {
        ObjSpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        ObjSpecialPowerModuleInterface::get_special_power_template_full(&self.base)
    }

    fn get_required_science(&self) -> ScienceType {
        ObjSpecialPowerModuleInterface::get_required_science(&self.base)
    }

    fn on_special_power_creation(&mut self) {
        ObjSpecialPowerModuleInterface::on_special_power_creation(&mut self.base)
    }

    fn set_ready_frame(&mut self, frame: u32) {
        ObjSpecialPowerModuleInterface::set_ready_frame(&mut self.base, frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        ObjSpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
        if let Some(owner_obj) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) {
            if let Ok(owner_guard) = owner_obj.read() {
                if owner_guard.is_disabled() {
                    return;
                }
            }
        }

        // Call base class to handle triggers
        ObjSpecialPowerModuleInterface::do_special_power(&mut self.base, command_options);

        // Calculate duration based on captured units
        let duration = self.calculate_duration();

        // Activate spy vision
        self.activate_spy_vision(duration);
    }

    fn do_special_power_at_object(
        &mut self,
        object_id: ObjectId,
        command_options: SpecialPowerCommandOptions,
    ) {
        let _ = object_id;
        // Spy vision doesn't target objects, delegate to location-less version
        ObjSpecialPowerModuleInterface::do_special_power(self, command_options)
    }

    fn do_special_power_at_location(
        &mut self,
        _location: &Coord3D,
        _angle: f32,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Spy vision doesn't target locations, delegate to location-less version
        ObjSpecialPowerModuleInterface::do_special_power(self, command_options)
    }

    fn do_special_power_using_waypoints(
        &mut self,
        waypoint: &Waypoint,
        command_options: SpecialPowerCommandOptions,
    ) {
        // Waypoints not used for spy vision
        ObjSpecialPowerModuleInterface::do_special_power_using_waypoints(
            &mut self.base,
            waypoint,
            command_options,
        )
    }

    fn mark_special_power_triggered(&mut self, location: Option<&Coord3D>) {
        ObjSpecialPowerModuleInterface::mark_special_power_triggered(&mut self.base, location)
    }

    fn start_power_recharge_at(&mut self, current_frame: FrameCount) {
        ObjSpecialPowerModuleInterface::start_power_recharge_at(&mut self.base, current_frame)
    }

    fn get_initiate_sound(&self) -> &crate::object::special_power_template::AudioEventRts {
        ObjSpecialPowerModuleInterface::get_initiate_sound(&self.base)
    }

    fn is_script_only(&self) -> bool {
        ObjSpecialPowerModuleInterface::is_script_only(&self.base)
    }

    fn get_reference_thing_template(&self) -> Option<String> {
        None
    }
}

impl EngineSpecialPowerModuleInterface for SpyVisionSpecialPower {
    fn activate(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        EngineSpecialPowerModuleInterface::activate(&mut self.base)
    }

    fn can_activate(&self) -> bool {
        EngineSpecialPowerModuleInterface::can_activate(&self.base)
    }

    fn get_power_type(&self) -> u32 {
        EngineSpecialPowerModuleInterface::get_power_type(&self.base)
    }

    fn start_power_recharge(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        EngineSpecialPowerModuleInterface::start_power_recharge(&mut self.base)
    }

    fn get_ready_frame(&self) -> u32 {
        EngineSpecialPowerModuleInterface::get_ready_frame(&self.base)
    }

    fn is_ready(&self) -> bool {
        EngineSpecialPowerModuleInterface::is_ready(&self.base)
    }

    fn get_special_power_template(&self) -> Option<Arc<dyn std::any::Any>> {
        EngineSpecialPowerModuleInterface::get_special_power_template(&self.base)
    }

    fn get_special_power_template_full(&self) -> Option<Arc<SpecialPowerTemplate>> {
        EngineSpecialPowerModuleInterface::get_special_power_template_full(&self.base)
    }

    fn get_power_name(&self) -> String {
        EngineSpecialPowerModuleInterface::get_power_name(&self.base)
    }

    fn get_percent_ready(&self) -> f32 {
        EngineSpecialPowerModuleInterface::get_percent_ready(&self.base)
    }

    fn pause_countdown(&mut self, pause: bool) {
        EngineSpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn mark_special_power_triggered(&mut self, location: Option<&crate::common::Coord3D>) {
        EngineSpecialPowerModuleInterface::mark_special_power_triggered(&mut self.base, location)
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

impl Snapshotable for SpyVisionSpecialPower {
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

impl Module for SpyVisionSpecialPower {
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
        LegacyModuleData::get_module_tag_name_key(self.spy_vision_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.spy_vision_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for SpyVisionSpecialPower {
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
    fn test_spy_vision_special_power_creation() {
        let data = SpyVisionSpecialPowerModuleData::default();
        let power = SpyVisionSpecialPower::new(0, 1, Arc::new(data));

        assert!(power.is_ready());
    }

    #[test]
    fn test_calculate_duration_base() {
        let data = SpyVisionSpecialPowerModuleData {
            base_duration_in_frames: 1500,
            bonus_duration_per_captured_in_frames: 300,
            max_duration_in_frames: 3000,
            ..Default::default()
        };

        let power = SpyVisionSpecialPower::new(0, 1, Arc::new(data));

        // Without captured units, should be base duration
        assert_eq!(power.calculate_duration(), 1500);
    }

    #[test]
    fn test_spy_vision_durations() {
        let data = SpyVisionSpecialPowerModuleData {
            base_duration_in_frames: 1000,
            bonus_duration_per_captured_in_frames: 500,
            max_duration_in_frames: 5000,
            ..Default::default()
        };

        assert_eq!(data.base_duration_in_frames, 1000);
        assert_eq!(data.bonus_duration_per_captured_in_frames, 500);
        assert_eq!(data.max_duration_in_frames, 5000);
    }

    #[test]
    fn test_do_special_power_at_object_delegates() {
        let data = SpyVisionSpecialPowerModuleData::default();
        let mut power = SpyVisionSpecialPower::new(0, 1, Arc::new(data));

        // Should delegate to location-less version
        crate::modules::SpecialPowerModuleInterface::do_special_power_at_object(
            &mut power,
            42,
            SpecialPowerCommandOptions::empty(),
        );
    }
}
