//! HeightDieUpdate - Death from falling damage
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{Bool, ModuleData, Real};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct HeightDieUpdateModuleData {
    pub base: BehaviorModuleData,
    pub target_height: Real,
    pub target_height_bonus: Real,
    pub only_when_moving_down: Bool,
}

impl Default for HeightDieUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            target_height: 0.0,
            target_height_bonus: 0.0,
            only_when_moving_down: true,
        }
    }
}

crate::impl_behavior_module_data_via_base!(HeightDieUpdateModuleData, base);

pub struct HeightDieUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<HeightDieUpdateModuleData>,
    last_height: Real,
}

impl HeightDieUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<HeightDieUpdateModuleData>()
            .ok_or("Invalid module data")?;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            last_height: 0.0,
        })
    }
}

impl UpdateModuleInterface for HeightDieUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        if let Some(object) = self.object.upgrade() {
            // Get current position - matches C++ HeightDieUpdate.cpp line 118
            let current_pos = if let Ok(obj) = object.read() {
                *obj.get_position()
            } else {
                return UpdateSleepTime::Frames(5);
            };

            // Check if only dying when moving down - matches C++ lines 124-130
            let direction_ok = if self.module_data.only_when_moving_down {
                current_pos.z < self.last_height
            } else {
                true
            };

            // Update last height for next check
            self.last_height = current_pos.z;

            // Calculate target height (simplified - terrain height + target_height)
            // Full C++ implementation uses TheTerrainLogic->getGroundHeight()
            // For now, use target_height directly as absolute height
            let target_height =
                self.module_data.target_height + self.module_data.target_height_bonus;

            // If below target height and direction is OK, kill the object
            // Matches C++ lines 200-222
            if current_pos.z < target_height && direction_ok {
                // Kill the object
                if let Ok(mut obj_write) = object.write() {
                    obj_write.kill(None, None);
                }
            }
        }

        UpdateSleepTime::Frames(5)
    }
}

impl BehaviorModuleInterface for HeightDieUpdate {
    fn get_module_name(&self) -> &'static str {
        "HeightDieUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

pub struct HeightDieUpdateFactory;
impl HeightDieUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(HeightDieUpdate::new(thing, module_data)?))
    }
}
