//! Base CollideModule implementation
//!
//! This module provides the core collision behavior system for game objects.
//! When objects collide (either with other objects or with the ground), the
//! collision modules handle the response logic.

use super::{CollisionError, Coord3D, GameObject};
use crate::common::*;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

/// Module data for basic collision modules
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CollideModuleData {
    // Base behavior module data would go here
    // Currently empty as per the C++ version
}

impl CollideModuleData {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for CollideModuleData {
    fn default() -> Self {
        Self::new()
    }
}

/// Interface trait for collision modules
///
/// This provides the core collision functionality that all collision
/// modules must implement.
pub trait CollideModuleInterface: Send + Sync {
    /// Called when two objects collide (or when object collides with ground)
    ///
    /// Note that 'other' can be None, indicating a collision with the ground.
    /// This method handles the response for the object that THIS module belongs to.
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), CollisionError>;

    /// Check if this object would like to collide with another object
    /// Used for things like pilots determining if they can "enter" something
    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool;

    /// Identification methods for specific collision types
    fn is_hijacked_vehicle_crate_collide(&self) -> bool {
        false
    }
    fn is_sabotage_building_crate_collide(&self) -> bool {
        false
    }
    fn is_car_bomb_crate_collide(&self) -> bool {
        false
    }
    fn is_railroad(&self) -> bool {
        false
    }
    fn is_salvage_crate_collide(&self) -> bool {
        false
    }
}

/// Base collision module implementation
///
/// This provides a base implementation that specific collision modules can extend.
/// It combines the behavior module functionality with the collision interface.
pub struct CollideModule {
    /// Reference to the owning game object
    object_id: ObjectId,
    /// Module configuration data
    module_data: CollideModuleData,
    /// Thread-safe state for the module
    state: Arc<Mutex<CollideModuleState>>,
}

#[derive(Debug)]
struct CollideModuleState {
    /// Whether the module is currently active
    is_active: bool,
    /// Last collision timestamp for cooldown logic
    last_collision_time: u64,
}

impl CollideModule {
    pub fn new(object_id: ObjectId, module_data: CollideModuleData) -> Self {
        Self {
            object_id,
            module_data,
            state: Arc::new(Mutex::new(CollideModuleState {
                is_active: true,
                last_collision_time: 0,
            })),
        }
    }

    pub fn get_object_id(&self) -> ObjectId {
        self.object_id
    }

    pub fn get_module_data(&self) -> &CollideModuleData {
        &self.module_data
    }

    /// Set the active state of this collision module
    pub fn set_active(&self, active: bool) -> Result<(), CollisionError> {
        let mut state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        state.is_active = active;
        Ok(())
    }

    /// Check if the module is currently active
    pub fn is_active(&self) -> Result<bool, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.is_active)
    }

    /// Update the last collision time (used for cooldown logic)
    pub fn update_collision_time(&self, time: u64) -> Result<(), CollisionError> {
        let mut state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        state.last_collision_time = time;
        Ok(())
    }

    /// Get the time since last collision
    pub fn get_time_since_last_collision(&self, current_time: u64) -> Result<u64, CollisionError> {
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(current_time.saturating_sub(state.last_collision_time))
    }

    /// Serialization support (equivalent to xfer in C++)
    pub fn serialize(&self) -> Result<Vec<u8>, CollisionError> {
        // Basic serialization - in a real implementation this would use
        // a proper serialization format
        let state = self.state.lock().map_err(|e| {
            CollisionError::InvalidObject(format!("Failed to acquire state lock: {}", e))
        })?;

        let mut data = Vec::new();
        data.extend_from_slice(&self.object_id.to_le_bytes());
        data.push(if state.is_active { 1 } else { 0 });
        data.extend_from_slice(&state.last_collision_time.to_le_bytes());

        Ok(data)
    }

    /// Deserialization support (equivalent to xfer in C++)
    pub fn deserialize(data: &[u8]) -> Result<Self, CollisionError> {
        if data.len() < 13 {
            // 4 + 1 + 8 bytes minimum
            return Err(CollisionError::InvalidObject(
                "Insufficient data for deserialization".to_string(),
            ));
        }

        let object_id = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let is_active = data[4] != 0;
        let last_collision_time = u64::from_le_bytes([
            data[5], data[6], data[7], data[8], data[9], data[10], data[11], data[12],
        ]);

        let module = Self::new(object_id, CollideModuleData::new());
        module.set_active(is_active)?;
        module.update_collision_time(last_collision_time)?;

        Ok(module)
    }
}

impl CollideModuleInterface for CollideModule {
    fn on_collide(
        &mut self,
        _other: Option<&dyn GameObject>,
        _loc: &Coord3D,
        _normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        // Base implementation does nothing - derived classes should override
        Ok(())
    }

    fn would_like_to_collide_with(&self, _other: &dyn GameObject) -> bool {
        false
    }
}

impl game_engine::common::system::Snapshotable for CollideModule {
    fn crc(&self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())
    }

    fn xfer(&mut self, xfer: &mut dyn game_engine::common::system::Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|err| err.to_string())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

/// Factory for creating collision modules
pub struct CollideModuleFactory;

impl CollideModuleFactory {
    pub fn create_basic_collide_module(object_id: ObjectId) -> CollideModule {
        CollideModule::new(object_id, CollideModuleData::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collide_module_creation() {
        let module = CollideModule::new(123, CollideModuleData::new());
        assert_eq!(module.get_object_id(), 123);
        assert!(module.is_active().unwrap());
    }

    #[test]
    fn test_collide_module_state() {
        let module = CollideModule::new(123, CollideModuleData::new());

        // Test initial state
        assert!(module.is_active().unwrap());

        // Test setting inactive
        module.set_active(false).unwrap();
        assert!(!module.is_active().unwrap());

        // Test setting active again
        module.set_active(true).unwrap();
        assert!(module.is_active().unwrap());
    }

    #[test]
    fn test_collision_timing() {
        let module = CollideModule::new(123, CollideModuleData::new());

        // Test initial time
        assert_eq!(module.get_time_since_last_collision(1000).unwrap(), 1000);

        // Update collision time
        module.update_collision_time(500).unwrap();
        assert_eq!(module.get_time_since_last_collision(1000).unwrap(), 500);
    }

    #[test]
    fn test_serialization() {
        let module = CollideModule::new(123, CollideModuleData::new());
        module.set_active(false).unwrap();
        module.update_collision_time(42).unwrap();

        let serialized = module.serialize().unwrap();
        let deserialized = CollideModule::deserialize(&serialized).unwrap();

        assert_eq!(deserialized.get_object_id(), 123);
        assert!(!deserialized.is_active().unwrap());
        assert_eq!(deserialized.get_time_since_last_collision(100).unwrap(), 58);
    }

    #[test]
    fn test_factory() {
        let module = CollideModuleFactory::create_basic_collide_module(456);
        assert_eq!(module.get_object_id(), 456);
        assert!(module.is_active().unwrap());
    }
}
