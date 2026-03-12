// FILE: demoralize_special_power.rs
// Port of DemoralizeSpecialPower.h and DemoralizeSpecialPower.cpp
// Author: Rust Port
// Desc: Demoralize enemies in a radius based on contained captives

use crate::common::{AsciiString, Coord3D, KindOf, LegacyModuleData, Relationship, UnsignedInt};
use crate::effects::FXList;
use crate::helpers::{TheFXListStore, ThePartitionManager};
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

/// Module data for demoralize special power
#[derive(Debug, Clone)]
pub struct DemoralizeSpecialPowerModuleData {
    pub module_tag_name_key: NameKeyType,
    /// Base special power data
    pub base: SpecialPowerModuleData,

    /// Base effect range
    pub base_range: f32,
    /// Bonus range per captured unit
    pub bonus_range_per_captured: f32,
    /// Maximum range cap
    pub max_range: f32,
    /// Base effect duration in frames
    pub base_duration_frames: UnsignedInt,
    /// Bonus duration per captured unit in frames
    pub bonus_duration_per_captured_frames: UnsignedInt,
    /// Maximum duration cap in frames
    pub max_duration_frames: UnsignedInt,
    /// Optional FX list to play at target location
    pub fx_list: Option<Arc<FXList>>,
}

impl Default for DemoralizeSpecialPowerModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            base: SpecialPowerModuleData::default(),
            base_range: 0.0,
            bonus_range_per_captured: 0.0,
            max_range: 0.0,
            base_duration_frames: 0,
            bonus_duration_per_captured_frames: 0,
            max_duration_frames: 0,
            fx_list: None,
        }
    }
}

impl DemoralizeSpecialPowerModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, DEMORALIZE_SPECIAL_POWER_FIELDS)
    }
}

impl Snapshotable for DemoralizeSpecialPowerModuleData {
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
    DemoralizeSpecialPowerModuleData,
    module_tag_name_key
);

fn parse_special_power_template_field(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
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
    data: &mut DemoralizeSpecialPowerModuleData,
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

fn parse_duration_field(
    setter: &mut dyn FnMut(UnsignedInt),
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    setter(INI::parse_duration_unsigned_int(token)?);
    Ok(())
}

fn parse_fx_list_field(
    _ini: &mut INI,
    data: &mut DemoralizeSpecialPowerModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.fx_list = TheFXListStore::find_fx_list(token);
    Ok(())
}

const DEMORALIZE_SPECIAL_POWER_FIELDS: &[FieldParse<DemoralizeSpecialPowerModuleData>] = &[
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
        token: "BaseRange",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.base_range = v, tokens),
    },
    FieldParse {
        token: "BonusRangePerCaptured",
        parse: |_, data, tokens| {
            parse_real_field(&mut |v| data.bonus_range_per_captured = v, tokens)
        },
    },
    FieldParse {
        token: "MaxRange",
        parse: |_, data, tokens| parse_real_field(&mut |v| data.max_range = v, tokens),
    },
    FieldParse {
        token: "BaseDuration",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |v| data.base_duration_frames = v, tokens)
        },
    },
    FieldParse {
        token: "BonusDurationPerCaptured",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |v| data.bonus_duration_per_captured_frames = v, tokens)
        },
    },
    FieldParse {
        token: "MaxDuration",
        parse: |_, data, tokens| {
            parse_duration_field(&mut |v| data.max_duration_frames = v, tokens)
        },
    },
    FieldParse {
        token: "FXList",
        parse: parse_fx_list_field,
    },
];

/// Demoralize special power implementation
#[derive(Debug, Clone)]
pub struct DemoralizeSpecialPower {
    /// Base special power module
    base: SpecialPowerModule,
    /// Demoralize-specific module data
    demoralize_data: Arc<DemoralizeSpecialPowerModuleData>,
    module_name_key: NameKeyType,
}

impl DemoralizeSpecialPower {
    /// Create a new demoralize special power
    pub fn new(
        module_name_key: NameKeyType,
        owner_object_id: ObjectId,
        data: Arc<DemoralizeSpecialPowerModuleData>,
    ) -> Self {
        let base = SpecialPowerModule::new(owner_object_id, data.base.clone());
        Self {
            base,
            demoralize_data: data,
            module_name_key,
        }
    }

    fn compute_range_and_duration(&self, source: &crate::object::Object) -> (f32, UnsignedInt) {
        let mut duration = self.demoralize_data.base_duration_frames;
        let mut range = self.demoralize_data.base_range;

        if let Some(contain) = source.get_contain() {
            if let Ok(contain_guard) = contain.lock() {
                let count = contain_guard.get_contain_count();
                duration = duration.saturating_add(
                    count.saturating_mul(self.demoralize_data.bonus_duration_per_captured_frames),
                );
                if duration > self.demoralize_data.max_duration_frames {
                    duration = self.demoralize_data.max_duration_frames;
                }

                range += (count as f32) * self.demoralize_data.bonus_range_per_captured;
                if range > self.demoralize_data.max_range {
                    range = self.demoralize_data.max_range;
                }
            }
        }

        (range, duration)
    }

    fn apply_demoralize(&self, location: &Coord3D, source: &crate::object::Object) {
        let (range, duration) = self.compute_range_and_duration(source);
        let Some(partition) = ThePartitionManager::get() else {
            return;
        };
        let candidates = partition.get_objects_in_range(location, range);

        for obj_id in candidates {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(obj_id) else {
                continue;
            };
            let Ok(obj_guard) = obj_arc.read() else {
                continue;
            };

            if obj_guard.is_destroyed() {
                continue;
            }
            if obj_guard.is_off_map() != source.is_off_map() {
                continue;
            }

            if !obj_guard.is_kind_of(KindOf::Infantry) {
                continue;
            }

            match source.relationship_to(&obj_guard) {
                Relationship::Enemy | Relationship::Neutral => {}
                _ => continue,
            }

            if let Some(ai) = obj_guard.get_ai_update_interface() {
                if let Ok(mut ai_guard) = ai.lock() {
                    ai_guard.set_demoralized(duration);
                }
            }
        }
    }
}

impl ObjSpecialPowerModuleInterface for DemoralizeSpecialPower {
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

    fn set_ready_frame(&mut self, frame: FrameCount) {
        self.base.set_ready_frame(frame)
    }

    fn pause_countdown(&mut self, pause: bool) {
        ObjSpecialPowerModuleInterface::pause_countdown(&mut self.base, pause)
    }

    fn do_special_power(&mut self, command_options: SpecialPowerCommandOptions) {
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

        let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };
        let pos = *obj_guard.get_position();
        ObjSpecialPowerModuleInterface::do_special_power_at_location(
            self,
            &pos,
            0.0,
            command_options,
        );
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

        let Some(source_arc) = OBJECT_REGISTRY.get_object(self.base.get_owner_object_id()) else {
            return;
        };
        let Ok(source_guard) = source_arc.read() else {
            return;
        };

        self.apply_demoralize(location, &source_guard);

        if let Some(fx_list) = self.demoralize_data.fx_list.as_ref() {
            let _ = fx_list.do_fx_at_position(location);
        }
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

impl EngineSpecialPowerModuleInterface for DemoralizeSpecialPower {
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

impl Snapshotable for DemoralizeSpecialPower {
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

impl Module for DemoralizeSpecialPower {
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
        LegacyModuleData::get_module_tag_name_key(self.demoralize_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn ModuleData {
        self.demoralize_data.as_ref()
    }

    fn on_object_created(&mut self) {
        self.base.on_object_created();
    }
}

impl crate::modules::BehaviorModuleInterface for DemoralizeSpecialPower {
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
