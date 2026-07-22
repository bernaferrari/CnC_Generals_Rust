//! KeepObjectDie - Destroy module that keeps the object in the world
//!
//! This module is used for objects like buildings that should remain visible after
//! destruction as rubble or wreckage. Instead of removing the object from the world,
//! it keeps it visible in a destroyed state.
//!
//! Original C++ location: GameLogic/Module/KeepObjectDie.h/.cpp
//! Original C++ Author: Kris Morness, November 2002
//! Rust conversion: 2025

use super::{DestroyModule, DestroyModuleData, DestroyResult};
use crate::common::ObjectID;
use crate::common::{ModuleData as LegacyModuleData, NameKeyType};
use crate::modules::DestroyModuleInterface;
use crate::object::Object;
use game_engine::common::system::{Snapshotable, Xfer};
use std::any::Any;
use std::sync::{Arc, RwLock, Weak};

/// Module data for KeepObjectDie
/// (matches C++ KeepObjectDieModuleData - no specific fields needed)
#[derive(Clone, Debug, Default)]
pub struct KeepObjectDieModuleData {
    module_tag_name_key: NameKeyType,
}

impl KeepObjectDieModuleData {
    /// Create new module data
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_module_tag_name_key(&mut self, key: NameKeyType) {
        self.module_tag_name_key = key;
    }

    pub fn get_module_tag_name_key(&self) -> NameKeyType {
        self.module_tag_name_key
    }
}

crate::impl_legacy_module_data_with_key_field!(KeepObjectDieModuleData, module_tag_name_key);

impl DestroyModuleData for KeepObjectDieModuleData {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

impl Snapshotable for KeepObjectDieModuleData {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        // Version 1: Initial version
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;

        // No additional fields to serialize
        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// KeepObjectDie - Keeps object in world after destruction
///
/// This module prevents the object from being removed from the game world when destroyed.
/// Instead, it remains visible in a destroyed state. This is commonly used for buildings
/// that should leave rubble after being destroyed.
///
/// (Matches C++ KeepObjectDie)
#[derive(Debug)]
pub struct KeepObjectDie {
    /// Weak reference to the owning object
    object_id: ObjectID,
    /// Module data
    #[allow(dead_code)]
    module_data: Arc<KeepObjectDieModuleData>,
}

impl KeepObjectDie {
    /// Create a new KeepObjectDie module
    /// (matches C++ KeepObjectDie constructor)
    pub fn new(object: Arc<RwLock<Object>>, module_data: Arc<KeepObjectDieModuleData>) -> Self {
        Self {
            object_id: object
                .read()
                .ok()
                .map(|g| g.get_id())
                .unwrap_or(crate::common::INVALID_ID),
            module_data,
        }
    }

    /// Get module name
    pub fn get_module_name() -> &'static str {
        "KeepObjectDie"
    }

    /// Get the owning object if still alive
    fn get_object(&self) -> Option<Arc<RwLock<Object>>> {
        (if self.object_id == crate::common::INVALID_ID {
            None
        } else {
            crate::helpers::TheGameLogic::find_object_by_id(self.object_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(self.object_id))
        })
    }
}

impl DestroyModule for KeepObjectDie {
    /// Called when object is being destroyed
    /// (matches C++ KeepObjectDie - does nothing, keeping object in world)
    fn on_destroy(&mut self, object: &mut Object) {
        // This module intentionally does nothing - it keeps the object in the world.
        // The object will remain visible as rubble/wreckage in its destroyed state.
        // This matches the C++ implementation which has an empty onDie() method.

        log::debug!(
            "KeepObjectDie: Object {} kept in world after destruction",
            object.get_id()
        );

        // NOTE: Unlike DestroyDie which calls TheGameLogic->destroyObject(),
        // this module keeps the object in the world. The object's visual state
        // should be updated elsewhere to show it as destroyed/damaged.
    }

    fn get_destroy_interface(&self) -> Option<&dyn DestroyModuleInterface> {
        // This module implements DestroyModuleInterface via the trait
        None
    }

    fn get_destroy_interface_mut(&mut self) -> Option<&mut dyn DestroyModuleInterface> {
        // This module implements DestroyModuleInterface via the trait
        None
    }
}

impl DestroyModuleInterface for KeepObjectDie {
    /// Called to perform destruction behavior
    /// (matches C++ DestroyModuleInterface::onDestroy)
    fn on_destroy(&mut self, object_id: crate::common::ObjectID) {
        if let Some(obj_arc) = self.get_object() {
            if let Ok(mut _obj) = obj_arc.write() {
                // Keep object in world - don't remove it
                log::debug!(
                    "KeepObjectDie: DestroyModuleInterface::on_destroy called for object {}",
                    object_id
                );

                // The object stays in the world but in a destroyed state
                // Visual updates should be handled by other systems
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_module_name() {
        assert_eq!(KeepObjectDie::get_module_name(), "KeepObjectDie");
    }

    #[test]
    fn test_module_data_creation() {
        let data = KeepObjectDieModuleData::new();
        assert_eq!(data.get_module_tag_name_key(), 0);
    }

    #[test]
    fn test_module_data_key_storage() {
        let mut data = KeepObjectDieModuleData::new();
        data.set_module_tag_name_key(42);
        assert_eq!(data.get_module_tag_name_key(), 42);
    }

    #[test]
    fn test_module_data_as_any() {
        let data = KeepObjectDieModuleData::new();
        let any_ref = data.as_any();
        assert!(any_ref.downcast_ref::<KeepObjectDieModuleData>().is_some());
    }

    #[test]
    fn test_module_data_default() {
        let data = KeepObjectDieModuleData::default();
        assert_eq!(data.get_module_tag_name_key(), 0);
    }

    // Note: Full integration tests would require a complete Object implementation
    // These tests verify the module data structure works correctly
}
