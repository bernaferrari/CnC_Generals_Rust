//! Collision system modules for game objects
//!
//! This module contains comprehensive collision handling implementations including:
//! - Collision geometry and detection (box, sphere, cylinder)
//! - Spatial partitioning for efficient queries
//! - Collision response (push, slide, crush, projectile hits)
//! - Crate collision behaviors
//! - Fire weapon collision
//! - Squish collision for crushing units
//!
//! The collision system matches C++ PartitionManager.cpp functionality

pub mod collide_module;
pub mod collision_geometry;
pub mod collision_response;
pub mod collision_system;
pub mod crate_collide;
pub mod fire_weapon_collide;
pub mod partition_manager;
pub mod squish_collide;

// Re-export key collision types for convenience
pub use collision_geometry::{
    collision_test, CollideInfo, CollideLocAndNormal, GeometryInfo, GeometryType,
};
pub use collision_response::{
    CollisionResponseConfig, CollisionResponseHandler, CollisionResponseType,
    TerrainCollisionHandler,
};
pub use collision_system::{
    with_collision_system, with_collision_system_mut, CollisionSystem, CollisionSystemStatistics,
    COLLISION_SYSTEM,
};
pub use partition_manager::{
    CellCoord, PartitionFilter, PartitionManager, PartitionStatistics, PARTITION_MANAGER,
};

use crate::common::{
    GameError, ObjectId, ObjectStatusMaskType, ObjectStatusTypes, PlayerId, Relationship,
    VeterancyLevel, INVALID_ID,
};
use crate::damage::{
    DamageInfo as EngineDamageInfo, DamageType as EngineDamageType, DeathType as EngineDeathType,
};
use crate::object::Object;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

/// 3D coordinate structure used by collision modules
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Coord3D {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self::ZERO
    }

    pub fn origin() -> Self {
        Self::ZERO
    }

    pub fn dot(&self, other: &Coord3D) -> f32 {
        self.x * other.x + self.y * other.y + self.z * other.z
    }

    pub fn distance_to(&self, other: &Coord3D) -> f32 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Object status mask type for tracking unit states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ObjectStatusMask(pub u64);

impl ObjectStatusMask {
    pub const fn empty() -> Self {
        Self(0)
    }

    pub const fn from_status(status: ObjectStatusTypes) -> Self {
        Self(ObjectStatusMaskType::from_status(status).bits())
    }

    pub const fn from_mask(mask: ObjectStatusMaskType) -> Self {
        Self(mask.bits())
    }

    pub const fn bits(self) -> u64 {
        self.0
    }

    pub fn to_mask(self) -> ObjectStatusMaskType {
        ObjectStatusMaskType::from_bits_retain(self.0)
    }

    pub fn test_for_all(&self, mask: ObjectStatusMask) -> bool {
        (self.0 & mask.0) == mask.0
    }

    pub fn test_for_any(&self, mask: ObjectStatusMask) -> bool {
        (self.0 & mask.0) != 0
    }
}

/// Damage types that can be applied to objects
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DamageType {
    Crush,
    Explosion,
    Fire,
    // Add other damage types as needed
}

impl From<DamageType> for EngineDamageType {
    fn from(value: DamageType) -> Self {
        match value {
            DamageType::Crush => EngineDamageType::Crush,
            DamageType::Explosion => EngineDamageType::Explosion,
            DamageType::Fire => EngineDamageType::Flame,
        }
    }
}

/// Death types for objects
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DeathType {
    Crushed,
    Explosion,
    Normal,
    // Add other death types as needed
}

impl From<DeathType> for EngineDeathType {
    fn from(value: DeathType) -> Self {
        match value {
            DeathType::Crushed => EngineDeathType::Crushed,
            DeathType::Explosion => EngineDeathType::Exploded,
            DeathType::Normal => EngineDeathType::Normal,
        }
    }
}

/// Damage information structure
#[derive(Debug, Clone)]
pub struct DamageInfo {
    pub damage_type: DamageType,
    pub death_type: DeathType,
    pub source_id: ObjectId,
    pub amount: f32,
}

/// Base collision module trait
pub trait CollideModule: Send + Sync {
    /// Called when collision occurs
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), CollisionError>;

    /// Check if this module would like to collide with another object
    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool;

    /// Module-specific identification methods
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

/// Helper trait for legacy modules that still operate on concrete `Object` handles.
pub trait LegacyCollideAdapter: Send + Sync {
    /// Legacy collision handler using the raw object handle.
    fn legacy_on_collide(
        &mut self,
        other: Arc<RwLock<Object>>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), GameError>;

    /// Legacy collision predicate using the raw object handle.
    fn legacy_would_like_to_collide_with(
        &self,
        other: Arc<RwLock<Object>>,
    ) -> Result<bool, GameError>;
}

fn resolve_object_handle(other: &dyn GameObject) -> Result<Arc<RwLock<Object>>, CollisionError> {
    other.as_object_handle().ok_or_else(|| {
        CollisionError::InvalidObject("GameObject did not expose an Object handle".into())
    })
}

impl<T: LegacyCollideAdapter> CollideModule for T {
    fn on_collide(
        &mut self,
        other: Option<&dyn GameObject>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        let Some(other_obj) = other else {
            return Ok(());
        };

        let handle = resolve_object_handle(other_obj)?;
        self.legacy_on_collide(handle, loc, normal)
            .map_err(|err| CollisionError::InvalidObject(err.to_string()))
    }

    fn would_like_to_collide_with(&self, other: &dyn GameObject) -> bool {
        match resolve_object_handle(other) {
            Ok(handle) => self
                .legacy_would_like_to_collide_with(handle)
                .unwrap_or(false),
            Err(_) => false,
        }
    }
}

/// Game object trait - simplified interface for collision system
pub trait GameObject: Send + Sync {
    fn get_id(&self) -> ObjectId;
    fn get_position(&self) -> Coord3D;
    fn get_orientation(&self) -> f32;
    fn get_controlling_player(&self) -> PlayerId;
    fn get_veterancy_level(&self) -> VeterancyLevel;
    fn get_relationship(&self, other: &dyn GameObject) -> Relationship;
    fn get_crusher_level(&self) -> u32;
    fn is_effectively_dead(&self) -> bool;
    fn is_significantly_above_terrain(&self) -> bool;
    fn is_using_airborne_locomotor(&self) -> bool;
    fn get_status_bits(&self) -> ObjectStatusMask;
    fn attempt_damage(&mut self, damage: &DamageInfo) -> Result<(), String>;
    fn set_undetected_defector(&mut self, value: bool);

    /// Set or clear status bits. Default implementation is a no-op so tests can opt-in.
    fn set_status(&self, _mask: ObjectStatusMask, _set: bool) {}

    /// Try to expose the backing `Object` handle when available.
    fn as_object_handle(&self) -> Option<Arc<RwLock<Object>>> {
        None
    }
}

impl GameObject for Arc<RwLock<Object>> {
    fn get_id(&self) -> ObjectId {
        self.read().map(|obj| obj.get_id()).unwrap_or(INVALID_ID)
    }

    fn get_position(&self) -> Coord3D {
        self.read()
            .map(|obj| {
                let pos = obj.get_position();
                Coord3D::new(pos.x, pos.y, pos.z)
            })
            .unwrap_or_else(|_| Coord3D::origin())
    }

    fn get_orientation(&self) -> f32 {
        self.read().map(|obj| obj.get_orientation()).unwrap_or(0.0)
    }

    fn get_controlling_player(&self) -> PlayerId {
        self.read()
            .ok()
            .and_then(|obj| obj.get_player_id())
            .unwrap_or(PlayerId::NEUTRAL)
    }

    fn get_veterancy_level(&self) -> VeterancyLevel {
        self.read()
            .map(|obj| obj.get_veterancy_level())
            .unwrap_or(VeterancyLevel::Regular)
    }

    fn get_relationship(&self, other: &dyn GameObject) -> Relationship {
        if let Some(other_handle) = other.as_object_handle() {
            if Arc::ptr_eq(&other_handle, self) {
                return Relationship::Friend;
            }
            if let (Ok(this_guard), Ok(other_guard)) = (self.read(), other_handle.read()) {
                return this_guard.relationship_to(&other_guard);
            }
        }
        Relationship::Neutral
    }

    fn get_crusher_level(&self) -> u32 {
        self.read().map(|obj| obj.get_crusher_level()).unwrap_or(0)
    }

    fn is_effectively_dead(&self) -> bool {
        self.read()
            .map(|obj| obj.is_effectively_dead())
            .unwrap_or(true)
    }

    fn is_significantly_above_terrain(&self) -> bool {
        self.read()
            .map(|obj| obj.is_significantly_above_terrain())
            .unwrap_or(false)
    }

    fn is_using_airborne_locomotor(&self) -> bool {
        self.read()
            .map(|obj| obj.is_using_airborne_locomotor())
            .unwrap_or(false)
    }

    fn get_status_bits(&self) -> ObjectStatusMask {
        self.read()
            .map(|obj| ObjectStatusMask(obj.get_status_bits().bits() as u64))
            .unwrap_or_else(|_| ObjectStatusMask::empty())
    }

    fn attempt_damage(&mut self, damage: &DamageInfo) -> Result<(), String> {
        match self.write() {
            Ok(mut obj) => {
                let mut packet = EngineDamageInfo::with_simple(
                    damage.amount,
                    damage.source_id,
                    EngineDamageType::from(damage.damage_type),
                    EngineDeathType::from(damage.death_type),
                );
                obj.attempt_damage(&mut packet)
                    .map_err(|err| err.to_string())
            }
            Err(_) => Err("Failed to lock object for damage processing".to_string()),
        }
    }

    fn set_undetected_defector(&mut self, value: bool) {
        if let Ok(mut obj) = self.write() {
            obj.set_undetected_defector(value);
        }
    }

    fn set_status(&self, mask: ObjectStatusMask, set: bool) {
        if let Ok(mut obj) = self.write() {
            let bitmask = ObjectStatusMaskType::from_bits_truncate(mask.0);
            obj.set_status(bitmask, set);
        }
    }

    fn as_object_handle(&self) -> Option<Arc<RwLock<Object>>> {
        Some(self.clone())
    }
}

impl<T: GameObject + ?Sized> GameObject for Box<T> {
    fn get_id(&self) -> ObjectId {
        (**self).get_id()
    }

    fn get_position(&self) -> Coord3D {
        (**self).get_position()
    }

    fn get_orientation(&self) -> f32 {
        (**self).get_orientation()
    }

    fn get_controlling_player(&self) -> PlayerId {
        (**self).get_controlling_player()
    }

    fn get_veterancy_level(&self) -> VeterancyLevel {
        (**self).get_veterancy_level()
    }

    fn get_relationship(&self, other: &dyn GameObject) -> Relationship {
        (**self).get_relationship(other)
    }

    fn get_crusher_level(&self) -> u32 {
        (**self).get_crusher_level()
    }

    fn is_effectively_dead(&self) -> bool {
        (**self).is_effectively_dead()
    }

    fn is_significantly_above_terrain(&self) -> bool {
        (**self).is_significantly_above_terrain()
    }

    fn is_using_airborne_locomotor(&self) -> bool {
        (**self).is_using_airborne_locomotor()
    }

    fn get_status_bits(&self) -> ObjectStatusMask {
        (**self).get_status_bits()
    }

    fn attempt_damage(&mut self, damage: &DamageInfo) -> Result<(), String> {
        (**self).attempt_damage(damage)
    }

    fn set_undetected_defector(&mut self, value: bool) {
        (**self).set_undetected_defector(value)
    }

    fn set_status(&self, mask: ObjectStatusMask, set: bool) {
        (**self).set_status(mask, set)
    }

    fn as_object_handle(&self) -> Option<Arc<RwLock<Object>>> {
        (**self).as_object_handle()
    }
}

/// Errors that can occur during collision processing
#[derive(Debug, Clone)]
pub enum CollisionError {
    InvalidObject(String),
    InvalidPosition(String),
    DamageApplicationFailed(String),
    AudioSystemError(String),
    PartitionManagerError(String),
}

impl std::fmt::Display for CollisionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CollisionError::InvalidObject(msg) => write!(f, "Invalid object: {}", msg),
            CollisionError::InvalidPosition(msg) => write!(f, "Invalid position: {}", msg),
            CollisionError::DamageApplicationFailed(msg) => {
                write!(f, "Damage application failed: {}", msg)
            }
            CollisionError::AudioSystemError(msg) => write!(f, "Audio system error: {}", msg),
            CollisionError::PartitionManagerError(msg) => {
                write!(f, "Partition manager error: {}", msg)
            }
        }
    }
}

impl std::error::Error for CollisionError {}

impl From<CollisionError> for GameError {
    fn from(err: CollisionError) -> Self {
        match err {
            CollisionError::InvalidObject(msg)
            | CollisionError::InvalidPosition(msg)
            | CollisionError::DamageApplicationFailed(msg)
            | CollisionError::AudioSystemError(msg)
            | CollisionError::PartitionManagerError(msg) => {
                GameError::ModuleError(format!("collision: {}", msg))
            }
        }
    }
}

/// Thread-safe collision manager for handling all collision events
pub struct CollisionManager {
    modules: Arc<Mutex<HashMap<ObjectId, Vec<Box<dyn CollideModule>>>>>,
}

impl CollisionManager {
    pub fn new() -> Self {
        Self {
            modules: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn register_collide_module(
        &self,
        object_id: ObjectId,
        module: Box<dyn CollideModule>,
    ) -> Result<(), CollisionError> {
        let mut modules = self
            .modules
            .lock()
            .map_err(|e| CollisionError::InvalidObject(format!("Failed to acquire lock: {}", e)))?;

        modules
            .entry(object_id)
            .or_insert_with(Vec::new)
            .push(module);
        Ok(())
    }

    pub fn handle_collision(
        &self,
        object_id: ObjectId,
        other: Option<&dyn GameObject>,
        loc: &Coord3D,
        normal: &Coord3D,
    ) -> Result<(), CollisionError> {
        let mut modules = self
            .modules
            .lock()
            .map_err(|e| CollisionError::InvalidObject(format!("Failed to acquire lock: {}", e)))?;

        if let Some(object_modules) = modules.get_mut(&object_id) {
            let object_handle = crate::helpers::TheGameLogic::find_object_by_id(object_id);
            for module in object_modules.iter_mut() {
                if let Some(handle) = &object_handle {
                    if let Ok(obj_guard) = handle.read() {
                        if obj_guard.test_status(ObjectStatusTypes::NoCollisions) {
                            break;
                        }
                    }
                }
                module.on_collide(other, loc, normal)?;
            }
        }

        Ok(())
    }

    pub fn would_like_to_collide_with(
        &self,
        object_id: ObjectId,
        other: &dyn GameObject,
    ) -> Result<bool, CollisionError> {
        let modules = self
            .modules
            .lock()
            .map_err(|e| CollisionError::InvalidObject(format!("Failed to acquire lock: {}", e)))?;

        if let Some(object_modules) = modules.get(&object_id) {
            for module in object_modules.iter() {
                if module.would_like_to_collide_with(other) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn would_like_to_collide_with_matching<F>(
        &self,
        object_id: ObjectId,
        other: &dyn GameObject,
        matcher: F,
    ) -> Result<bool, CollisionError>
    where
        F: Fn(&dyn CollideModule) -> bool,
    {
        let modules = self
            .modules
            .lock()
            .map_err(|e| CollisionError::InvalidObject(format!("Failed to acquire lock: {}", e)))?;

        if let Some(object_modules) = modules.get(&object_id) {
            for module in object_modules.iter() {
                if matcher(module.as_ref()) && module.would_like_to_collide_with(other) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    pub fn unregister_object(&self, object_id: ObjectId) -> Result<(), CollisionError> {
        let mut modules = self
            .modules
            .lock()
            .map_err(|e| CollisionError::InvalidObject(format!("Failed to acquire lock: {}", e)))?;

        modules.remove(&object_id);
        Ok(())
    }
}

impl Default for CollisionManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global collision manager instance
lazy_static::lazy_static! {
    pub static ref COLLISION_MANAGER: CollisionManager = CollisionManager::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_coord3d_operations() {
        let pos1 = Coord3D::new(1.0, 2.0, 3.0);
        let pos2 = Coord3D::new(4.0, 5.0, 6.0);

        assert_eq!(pos1.dot(&pos2), 32.0);
        assert!((pos1.distance_to(&pos2) - 5.196).abs() < 0.01);
    }

    #[test]
    fn test_object_status_mask() {
        let mask1 = ObjectStatusMask(0b1010);
        let mask2 = ObjectStatusMask(0b1000);
        let mask3 = ObjectStatusMask(0b0010);

        assert!(mask1.test_for_any(mask2));
        assert!(mask1.test_for_any(mask3));
        assert!(mask1.test_for_all(mask2));
        assert!(mask1.test_for_all(mask3));
        assert!(!mask2.test_for_all(mask1));
    }

    #[test]
    fn test_collision_manager() {
        let manager = CollisionManager::new();

        // Test would require a mock CollideModule implementation
        // This is a basic structure test
        assert!(manager.modules.lock().is_ok());
    }
}
