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
            .as_any()
            .downcast_ref::<PowerPlantUpdateModuleData>()
            .ok_or("Invalid module data type for PowerPlantUpdate")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
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

        xfer.xfer_bool(&mut self.extended)
            .map_err(|e| format!("Failed to xfer extended: {:?}", e))?;
        xfer.xfer_unsigned_int(&mut self.extend_done_frame)
            .map_err(|e| format!("Failed to xfer extend_done_frame: {:?}", e))?;
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
    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
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

    #[test]
    fn test_power_plant_creation() {
        let data = PowerPlantUpdateModuleData::default();
        assert_eq!(data.rods_extend_time, 0);
    }
}
