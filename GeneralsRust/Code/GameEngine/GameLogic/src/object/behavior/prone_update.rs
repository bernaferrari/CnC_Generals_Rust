//! ProneUpdate - Rust conversion of C++ ProneUpdate
//!
//! Update module to encapsulate what it means to be "prone".
//! Units go prone when damaged, based on damage-to-frames ratio.
//! Author: Graham Smallwood, March 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::xfer::XferExt;
use crate::common::{
    AsciiString, ModelConditionFlags, ModuleData, ObjectStatusMaskType, Real, XferVersion,
};
use crate::modules::{
    BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime, UPDATE_SLEEP_NONE,
};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct ProneUpdateModuleData {
    pub base: BehaviorModuleData,
    /// Conversion from damage dealt to number of frames we cower
    pub damage_to_frames_ratio: Real,
}

impl Default for ProneUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            damage_to_frames_ratio: 1.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(ProneUpdateModuleData, base);

impl ProneUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, PRONE_UPDATE_FIELDS)
    }
}

fn parse_damage_to_frames_ratio(
    _ini: &mut INI,
    data: &mut ProneUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens.first().ok_or(INIError::InvalidData)?;
    data.damage_to_frames_ratio = INI::parse_real(token)?;
    Ok(())
}

const PRONE_UPDATE_FIELDS: &[FieldParse<ProneUpdateModuleData>] = &[FieldParse {
    token: "DamageToFramesRatio",
    parse: parse_damage_to_frames_ratio,
}];

/// ProneUpdate module - Makes units go prone when damaged
pub struct ProneUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<ProneUpdateModuleData>,
    /// Number of frames remaining in prone state
    prone_frames: i32,
}

impl ProneUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
        .downcast_ref::<ProneUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            prone_frames: 0,
        })
    }

    /// Make this unit go prone based on damage taken
    /// damage_dealt: The actual damage dealt to the unit
    pub fn go_prone(&mut self, damage_dealt: i32) {
        let was_prone = self.prone_frames > 0;
        self.prone_frames += (damage_dealt as f32 * self.module_data.damage_to_frames_ratio) as i32;

        if !was_prone && self.prone_frames > 0 {
            self.start_prone_effects();
        }
    }

    /// Start prone visual and gameplay effects
    fn start_prone_effects(&self) {
        if let Some(me_arc) = self.object.upgrade() {
            if let Ok(mut me) = me_arc.write() {
                // Set NO_ATTACK status so unit can't fire while prone
                me.set_status(ObjectStatusMaskType::NO_ATTACK, true);
                me.set_model_condition_state(ModelConditionFlags::PRONE);
            }
        }
    }

    /// Stop prone visual and gameplay effects
    fn stop_prone_effects(&self) {
        if let Some(me_arc) = self.object.upgrade() {
            if let Ok(mut me) = me_arc.write() {
                // Clear NO_ATTACK status
                me.set_status(ObjectStatusMaskType::NO_ATTACK, false);
                me.clear_model_condition_state(ModelConditionFlags::PRONE);
            }
        }
    }
}

impl UpdateModuleInterface for ProneUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if self.prone_frames > 0 {
            self.prone_frames -= 1;
            if self.prone_frames == 0 {
                self.stop_prone_effects();
            }
        }
        UPDATE_SLEEP_NONE
    }
}

impl BehaviorModuleInterface for ProneUpdate {
    fn get_module_name(&self) -> &'static str {
        "ProneUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

impl Snapshotable for ProneUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer.xfer_i32(&mut self.prone_frames)
            .map_err(|e| format!("Failed to xfer prone_frames: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes ProneUpdate through the common Module trait.
pub struct ProneUpdateModule {
    behavior: ProneUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<ProneUpdateModuleData>,
}

impl ProneUpdateModule {
    pub fn new(
        behavior: ProneUpdate,
        module_name: &AsciiString,
        module_data: Arc<ProneUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut ProneUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for ProneUpdateModule {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.crc(xfer)
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        self.behavior.xfer(xfer)
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        self.behavior.load_post_process()
    }
}

impl Module for ProneUpdateModule {
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
        self.module_data.get_module_tag_name_key()
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

/// Interface for ProneUpdate behavior
pub trait ProneUpdateInterface {
    fn go_prone(&mut self, damage_dealt: i32);
}

impl ProneUpdateInterface for ProneUpdate {
    fn go_prone(&mut self, damage_dealt: i32) {
        ProneUpdate::go_prone(self, damage_dealt);
    }
}

pub struct ProneUpdateFactory;
impl ProneUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(ProneUpdate::new(thing, module_data)?))
    }
}
