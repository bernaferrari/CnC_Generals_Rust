//! RadarUpdate - Rust conversion of C++ RadarUpdate
//!
//! Update module for radar buildings.
//! Author: Colin Day, April 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::{
    AsciiString, Bool, LegacyModuleData, ModelConditionFlags, ModuleData, NameKeyType, Real,
    UnsignedInt, XferVersion,
};
use crate::helpers::TheGameLogic;
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::{xfer_update_module_base_state, BehaviorModuleData};
use crate::object::Object as GameObject;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, RadarUpdateConfig, RadarUpdateInterface,
};
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

impl Snapshotable for RadarUpdateModuleData {
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

impl LegacyModuleData for RadarUpdateModuleData {
    fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        game_engine::common::thing::module::ModuleData::set_module_tag_name_key(
            &mut self.base,
            key,
        );
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        game_engine::common::thing::module::ModuleData::get_module_tag_name_key(&self.base)
    }

    fn get_radar_update_config(&self) -> Option<RadarUpdateConfig> {
        Some(self.to_config())
    }
}

impl EngineModuleData for RadarUpdateModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        LegacyModuleData::set_module_tag_name_key(self, key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        LegacyModuleData::get_module_tag_name_key(self)
    }

    fn get_radar_update_config(&self) -> Option<RadarUpdateConfig> {
        Some(self.to_config())
    }
}

impl crate::common::types::ModuleData for RadarUpdateModuleData {
    fn get_radar_update_config(&self) -> Option<RadarUpdateConfig> {
        Some(self.to_config())
    }
}

impl RadarUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, RADAR_UPDATE_FIELDS)
    }

    fn to_config(&self) -> RadarUpdateConfig {
        RadarUpdateConfig {
            radar_extend_time: self.radar_extend_time,
        }
    }

    fn from_config(config: RadarUpdateConfig, module_tag_name_key: NameKeyType) -> Self {
        let mut data = Self {
            base: BehaviorModuleData::default(),
            radar_extend_time: config.radar_extend_time,
        };
        LegacyModuleData::set_module_tag_name_key(&mut data, module_tag_name_key);
        data
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
    data.radar_extend_time = INI::parse_duration_real(token)?;
    Ok(())
}

const RADAR_UPDATE_FIELDS: &[FieldParse<RadarUpdateModuleData>] = &[FieldParse {
    token: "RadarExtendTime",
    parse: parse_radar_extend_time,
}];

#[allow(dead_code)]
pub struct RadarUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: RadarUpdateConfig,
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
        let config = module_data
            .get_radar_update_config()
            .ok_or("Invalid module data")?;

        Ok(Self::new_with_config(object, config))
    }

    pub fn new_with_config(
        object: Arc<RwLock<GameObject>>,
        module_data: RadarUpdateConfig,
    ) -> Self {
        Self {
            object: Arc::downgrade(&object),
            module_data,
            next_call_frame_and_phase: 0,
            extend_done_frame: 0,
            extend_complete: false,
            radar_active: false,
        }
    }

    pub fn extend_radar(&mut self) {
        let current_frame = TheGameLogic::get_frame();
        self.extend_done_frame = current_frame + self.module_data.radar_extend_time as UnsignedInt;
        self.radar_active = true;

        if let Some(object) = self.object.upgrade() {
            if let Ok(mut object) = object.write() {
                object.set_model_condition_state(ModelConditionFlags::RADAR_EXTENDING);
            }
        }
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

            if let Some(object) = self.object.upgrade() {
                if let Ok(mut object) = object.write() {
                    let _ = object.clear_and_set_model_condition_flags(
                        ModelConditionFlags::RADAR_EXTENDING,
                        ModelConditionFlags::RADAR_UPGRADED,
                    );
                }
            }
        }

        UpdateSleepTime::None
    }
}

impl RadarUpdateInterface for RadarUpdate {
    fn extend_radar(&mut self) {
        self.extend_radar();
    }

    fn is_radar_active(&self) -> bool {
        self.is_radar_active()
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

    pub fn from_module_data(
        object: Arc<RwLock<GameObject>>,
        module_name: &AsciiString,
        module_data: Arc<dyn EngineModuleData>,
    ) -> Option<Self> {
        let config = module_data.get_radar_update_config()?;
        let behavior = RadarUpdate::new_with_config(object, config);
        Some(Self::new(
            behavior,
            module_name,
            Arc::new(RadarUpdateModuleData::from_config(
                config,
                module_data.get_module_tag_name_key(),
            )),
        ))
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
        EngineModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }

    fn get_radar_update_interface(&mut self) -> Option<&mut dyn RadarUpdateInterface> {
        Some(&mut self.behavior)
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

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::ini::INI;

    #[test]
    fn radar_extend_time_parses_cpp_duration_real() {
        let mut data = RadarUpdateModuleData::default();
        let mut ini = INI::new();

        parse_radar_extend_time(&mut ini, &mut data, &["1.5s"]).expect("duration real");

        assert!((data.radar_extend_time - 45.0).abs() < 0.001);
    }

    #[test]
    fn radar_update_interface_extends_without_downcast() {
        let data = Arc::new(RadarUpdateModuleData {
            radar_extend_time: 10.0,
            ..Default::default()
        });
        let config = data.to_config();
        let mut module = RadarUpdateModule {
            behavior: RadarUpdate {
                object: Weak::new(),
                module_data: config,
                next_call_frame_and_phase: 0,
                extend_done_frame: 0,
                extend_complete: false,
                radar_active: false,
            },
            module_name_key: NameKeyGenerator::name_to_key("RadarUpdate"),
            module_data: data,
        };

        let radar = module
            .get_radar_update_interface()
            .expect("radar update interface");
        radar.extend_radar();

        assert!(radar.is_radar_active());
    }
}
