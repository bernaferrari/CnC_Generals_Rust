//! PowerPlantUpdate - Rust conversion of C++ PowerPlantUpdate
//!
//! Update module for power plant buildings that generate power for a player.
//! Author: Amit Kumar, August 2002 (C++ version)
//! Rust conversion: 2025

use crate::common::types::ModelConditionFlags;
use crate::common::{AsciiString, Bool, ModuleData, UnsignedInt};
use crate::modules::{
    BehaviorModuleInterface, PowerPlantUpdateInterface, UpdateModuleInterface, UpdateSleepTime,
};
use crate::object::behavior::behavior_module::xfer_update_module_base_state;
use crate::object::Object as GameObject;
use crate::system::game_logic::get_game_logic;
use game_engine::common::ini::{FieldParse, INIError, INI};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_engine::common::system::{Snapshotable, Xfer};
use game_engine::common::thing::module::{
    Module, ModuleData as EngineModuleData, NameKeyType, Thing as ModuleThing,
};
use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

#[allow(dead_code)]
const UPDATE_SLEEP_NONE: UpdateSleepTime = UpdateSleepTime::None;
const UPDATE_SLEEP_FOREVER: UpdateSleepTime = UpdateSleepTime::Forever;

/// Module data for PowerPlantUpdate
#[derive(Clone, Debug)]
pub struct PowerPlantUpdateModuleData {
    module_tag_name_key: NameKeyType,
    pub rods_extend_time: UnsignedInt,
}

impl Default for PowerPlantUpdateModuleData {
    fn default() -> Self {
        Self {
            module_tag_name_key: 0,
            rods_extend_time: 0,
        }
    }
}

impl PowerPlantUpdateModuleData {
    pub fn parse_from_ini(&mut self, ini: &mut INI) -> Result<(), INIError> {
        ini.init_from_ini_with_fields(self, POWER_PLANT_UPDATE_FIELDS)
    }
}

const POWER_PLANT_UPDATE_FIELDS: &[FieldParse<PowerPlantUpdateModuleData>] = &[FieldParse {
    token: "RodsExtendTime",
    parse: |_, data, tokens| {
        let value = tokens.iter().copied().find(|token| *token != "=");
        data.rods_extend_time =
            INI::parse_duration_unsigned_int(value.ok_or(INIError::InvalidData)?)?;
        Ok(())
    },
}];

crate::impl_legacy_module_data_with_key_field!(PowerPlantUpdateModuleData, module_tag_name_key);

impl Snapshotable for PowerPlantUpdateModuleData {
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

/// PowerPlantUpdate behavior module
pub struct PowerPlantUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<PowerPlantUpdateModuleData>,
    next_call_frame_and_phase: UnsignedInt,
    extended: Bool,
    extend_done_frame: UnsignedInt,
}

impl PowerPlantUpdate {
    /// Create a new PowerPlantUpdate instance
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .downcast_ref::<PowerPlantUpdateModuleData>()
            .ok_or("Invalid module data type for PowerPlantUpdate")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            next_call_frame_and_phase: 0,
            extended: false,
            extend_done_frame: 0,
        })
    }

    /// Extend or retract the power plant rods
    pub fn extend_rods(&mut self, extend: Bool) {
        // Matches C++ PowerPlantUpdate::extendRods (sets upgrading flag and wake frame)
        if extend && !self.extended {
            if let Some(object) = self.object.upgrade() {
                if let Ok(mut obj) = object.write() {
                    let current_frame = get_game_logic()
                        .lock()
                        .map(|logic| logic.get_frame())
                        .unwrap_or(0);
                    self.extend_done_frame = current_frame + self.module_data.rods_extend_time;

                    // Set upgrading model condition so the animation plays while extending
                    obj.set_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADING);
                }
            }
        } else if !extend && self.extended {
            self.extended = false;
            self.extend_done_frame = 0;

            if let Some(object) = self.object.upgrade() {
                if let Ok(mut obj) = object.write() {
                    // Clear both upgrading and upgraded visual flags immediately
                    obj.clear_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADING);
                    obj.clear_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADED);
                }
            }
        }
    }

    /// Check if rods are extended
    pub fn is_extended(&self) -> Bool {
        self.extended
    }
}

impl UpdateModuleInterface for PowerPlantUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if let Some(object) = self.object.upgrade() {
            if let Ok(mut obj) = object.write() {
                let current_frame = get_game_logic()
                    .lock()
                    .map(|logic| logic.get_frame())
                    .unwrap_or(0);

                // Check if extension is complete
                if !self.extended && self.extend_done_frame > 0 {
                    if current_frame >= self.extend_done_frame {
                        self.extended = true;
                        self.extend_done_frame = 0;

                        // Replace upgrading with upgraded model condition (matches C++)
                        obj.clear_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADING);
                        obj.set_model_condition_state(ModelConditionFlags::POWER_PLANT_UPGRADED);
                    }
                }

                // Only need to update while extending
                if self.extend_done_frame > 0 {
                    return UpdateSleepTime::Frames(1);
                }

                return UPDATE_SLEEP_FOREVER;
            }
        }

        UPDATE_SLEEP_FOREVER
    }
}

impl BehaviorModuleInterface for PowerPlantUpdate {
    fn get_module_name(&self) -> &'static str {
        "PowerPlantUpdate"
    }

    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }

    fn get_power_plant_update_interface(&mut self) -> Option<&mut dyn PowerPlantUpdateInterface> {
        Some(self)
    }
}

impl PowerPlantUpdateInterface for PowerPlantUpdate {
    fn extend_rods(&mut self, extend: Bool) {
        self.extend_rods(extend);
    }
}

impl Snapshotable for PowerPlantUpdate {
    fn crc(&self, _xfer: &mut dyn Xfer) -> Result<(), String> {
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: game_engine::common::system::xfer::XferVersion = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer version: {:?}", e))?;

        xfer_update_module_base_state(xfer, &mut self.next_call_frame_and_phase)?;

        xfer.xfer_bool(&mut self.extended)
            .map_err(|e| format!("Failed to xfer extended: {:?}", e))?;
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Glue that exposes PowerPlantUpdate through the common Module trait.
pub struct PowerPlantUpdateModule {
    behavior: PowerPlantUpdate,
    module_name_key: NameKeyType,
    module_data: Arc<PowerPlantUpdateModuleData>,
}

impl PowerPlantUpdateModule {
    pub fn new(
        behavior: PowerPlantUpdate,
        module_name: &AsciiString,
        module_data: Arc<PowerPlantUpdateModuleData>,
    ) -> Self {
        let module_name_key = NameKeyGenerator::name_to_key(module_name.as_str());
        Self {
            behavior,
            module_name_key,
            module_data,
        }
    }

    pub fn behavior_mut(&mut self) -> &mut PowerPlantUpdate {
        &mut self.behavior
    }
}

impl Snapshotable for PowerPlantUpdateModule {
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

impl Module for PowerPlantUpdateModule {
    fn get_module_name_key(&self) -> NameKeyType {
        self.module_name_key
    }

    fn get_module_tag_name_key(&self) -> NameKeyType {
        EngineModuleData::get_module_tag_name_key(self.module_data.as_ref())
    }

    fn get_module_data(&self) -> &dyn EngineModuleData {
        self.module_data.as_ref()
    }
}

// Factory for creating PowerPlantUpdate instances
pub struct PowerPlantUpdateFactory;

impl PowerPlantUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        let behavior = PowerPlantUpdate::new(thing, module_data)?;
        Ok(Box::new(behavior))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    #[test]
    fn test_power_plant_creation() {
        let data = PowerPlantUpdateModuleData::default();
        assert_eq!(data.rods_extend_time, 0);
    }

    #[test]
    fn power_plant_update_xfer_preserves_cpp_runtime_fields_only() {
        let module_data = Arc::new(PowerPlantUpdateModuleData::default());
        let mut saved = PowerPlantUpdate {
            object: Weak::new(),
            module_data: module_data.clone(),
            next_call_frame_and_phase: 0x1234,
            extended: true,
            extend_done_frame: 1234,
        };

        let mut bytes = Vec::new();
        {
            let cursor = Cursor::new(&mut bytes);
            let mut save = XferSave::new(cursor, 1);
            save.open("power_plant_update").unwrap();
            saved.xfer(&mut save).unwrap();
            save.close().unwrap();
        }

        let mut loaded = PowerPlantUpdate {
            object: Weak::new(),
            module_data,
            next_call_frame_and_phase: 0,
            extended: false,
            extend_done_frame: 77,
        };
        {
            let mut load = XferLoad::new(Cursor::new(bytes), 1);
            load.open("power_plant_update").unwrap();
            loaded.xfer(&mut load).unwrap();
            load.close().unwrap();
        }

        assert!(loaded.extended);
        assert_eq!(loaded.next_call_frame_and_phase, 0x1234);
        assert_eq!(loaded.extend_done_frame, 77);
    }
}
