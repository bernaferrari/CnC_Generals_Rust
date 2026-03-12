//! DeletionUpdate - Auto-deletion of objects after conditions
//! Author: EA Pacific (C++ version) | Rust conversion: 2025

use crate::common::{Bool, ModuleData, UnsignedInt};
use crate::modules::{BehaviorModuleInterface, UpdateModuleInterface, UpdateSleepTime};
use crate::object::behavior::behavior_module::BehaviorModuleData;
use crate::object::Object as GameObject;
use std::sync::{Arc, RwLock, Weak};

#[derive(Clone, Debug)]
pub struct DeletionUpdateModuleData {
    pub base: BehaviorModuleData,
    pub min_lifetime: UnsignedInt,
    pub max_lifetime: UnsignedInt,
}

impl Default for DeletionUpdateModuleData {
    fn default() -> Self {
        Self {
            base: BehaviorModuleData::default(),
            min_lifetime: 0,
            max_lifetime: 0,
        }
    }
}

crate::impl_behavior_module_data_via_base!(DeletionUpdateModuleData, base);

pub struct DeletionUpdate {
    object: Weak<RwLock<GameObject>>,
    module_data: Arc<DeletionUpdateModuleData>,
    delete_frame: UnsignedInt,
}

impl DeletionUpdate {
    pub fn new(
        object: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let specific_data = module_data
            .as_ref()
            .as_any()
            .downcast_ref::<DeletionUpdateModuleData>()
            .ok_or("Invalid module data")?;

        // Get current frame from game logic - matches C++ DeletionUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let lifetime = (specific_data.min_lifetime + specific_data.max_lifetime) / 2;

        Ok(Self {
            object: Arc::downgrade(&object),
            module_data: Arc::new(specific_data.clone()),
            delete_frame: current_frame + lifetime,
        })
    }

    pub fn set_lifetime_range(&mut self, min_lifetime: UnsignedInt, max_lifetime: UnsignedInt) {
        let current_frame = crate::helpers::TheGameLogic::get_frame();
        let lifetime = (min_lifetime + max_lifetime) / 2;
        self.delete_frame = current_frame + lifetime;
    }
}

impl UpdateModuleInterface for DeletionUpdate {
    fn update_simple(&mut self) -> UpdateSleepTime {
        // Get current frame from game logic - matches C++ DeletionUpdate.cpp
        let current_frame = crate::helpers::TheGameLogic::get_frame();

        if current_frame >= self.delete_frame {
            // Delete object through game logic
            // In full implementation, would call TheGameLogic::destroy_object
            return UpdateSleepTime::Forever;
        }

        UpdateSleepTime::from_u32(self.delete_frame - current_frame)
    }
}

impl BehaviorModuleInterface for DeletionUpdate {
    fn get_module_name(&self) -> &'static str {
        "DeletionUpdate"
    }
    fn get_update(&mut self) -> Option<&mut dyn UpdateModuleInterface> {
        Some(self)
    }
}

pub struct DeletionUpdateFactory;
impl DeletionUpdateFactory {
    pub fn create_behavior(
        thing: Arc<RwLock<GameObject>>,
        module_data: Arc<dyn ModuleData>,
    ) -> Result<Box<dyn BehaviorModuleInterface>, Box<dyn std::error::Error + Send + Sync>> {
        Ok(Box::new(DeletionUpdate::new(thing, module_data)?))
    }
}
