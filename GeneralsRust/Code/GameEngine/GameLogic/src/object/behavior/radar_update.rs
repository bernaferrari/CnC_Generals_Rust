//! RadarUpdate - Rust conversion of C++ RadarUpdate
//!
//! Update module for radar buildings.
//! Author: Colin Day, April 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{AsciiString, Bool, ModuleData, Real, UnsignedInt, XferVersion};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{Module, ModuleData as EngineModuleData, NameKeyType};
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct RadarUpdateModuleData {
    pub base: BehaviorModuleData,
    pub radar_extend_time: Real,
}

impl Default for RadarUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            radar_extend_time: 0.0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(RadarUpdateModuleData, base);

impl RadarUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RADAR_UPDATE_FIELDS)
    }
}

fn parse_radar_extend_time(
    _ini: &mut INI,
    data: &mut RadarUpdateModuleData,
    tokens: &[&str],
) -> Result<(), INIError> {
    let token = tokens
        .iter()
        .copied()
        .find(|t| *t != "=")
        .ok_or(INIError::InvalidData)?;
    let frames = INI::parse_duration_unsigned_int(token)? as Real;
    data.radar_extend_time = frames;
    Ok(())
}

const RADAR_UPDATE_FIELDS: &[FieldParse<RadarUpdateModuleData>] = &[FieldParse {
    token: "RadarExtendTime",
    parse: parse_radar_extend_time,
}];

#[allow(dead_code)]
pub struct RadarUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<RadarUpdateModuleData>,
    /// UpdateModule scheduler state serialized by the C++ base class.
    next_call_frame_and_phase: UnsignedInt,
    extend_done_frame: UnsignedInt,
    extend_complete: Bool,
    radar_active: Bool,
}

impl RadarUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<RadarUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            extend_done_frame: 0,
            extend_complete: false,
            radar_active: false,
        })
    }

    pub fn extend_radar(&mut self) {
        let current_frame = TheGameLogic::get_frame();
        self.extend_done_frame = current_frame + self.module_data.radar_extend_time as UnsignedInt;
        self.radar_active = true;
    }

    pub fn is_radar_active(&self) -> Bool {
        self.radar_active
    }
}

impl Snapshotable for RadarUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;
        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;
        xfer.xfer_unsigned_int(&mut self.extend_done_frame)
            .map_err(|e| format!("Failed to xfer extend_done_frame: {:?}", e))?;
        xfer.xfer_bool(&mut self.extend_complete)
            .map_err(|e| format!("Failed to xfer extend_complete: {:?}", e))?;
        xfer.xfer_bool(&mut self.radar_active)
            .map_err(|e| format!("Failed to xfer radar_active: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl UpdateModuleInterface for RadarUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        let current_frame = TheGameLogic::get_frame();

        if self.extend_done_frame == 0 || self.extend_complete {
            return UpdateSleepTime::None;
        }

        if current_frame > self.extend_done_frame {
            self.extend_complete = true;
            self.extend_done_frame = 0;
        }

        UpdateSleepTime::None
    }
}

impl BehaviorModuleInterface for RadarUpdate {
    fn get_module_name(&self) -> &'static str {
        "RadarUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

/// Glue that exposes RadarUpdate through the common Module trait.
pub struct RadarUpdateModule {
    behavior: RadarUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<RadarUpdateModuleData>,
}

impl RadarUpdateModule {
    pub fn new(
        behavior: RadarUpdate,
        module_name: &AsciiString,
        module_data: Arc<RadarUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut RadarUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for RadarUpdateModule {
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

impl Module for RadarUpdateModule {
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

pub struct RadarUpdateFactory;
impl RadarUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(RadarUpdate::new(thing, module_data)?))
    }
}
