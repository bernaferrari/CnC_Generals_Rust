//! BehaviorModule - Rust conversion of C++ BehaviorModule
//!
//! Base class for all behavior modules in the object system.
//! Author: Steven Johnson (C++ version)
//! Rust conversion: 2025

use crate::common::{
    Bool, Coord2D, Coord3D, CoordOrigin, Int, ModuleData, ObjectID, Real, UnsignedInt, Xfer,
    XferVersion,
};
use crate::object::Object;
use std::result::Result;
use std::sync::{Arc, Mutex, RwLock};

pub use crate::modules::BehaviorModuleInterface;
pub use crate::modules::CountermeasuresBehaviorInterface;
use game_engine::common::system::Snapshotable;
use game_engine::common::thing::module::BaseModuleData;

pub(crate) fn xfer_behavior_module_base_versions(xfer: &mut dyn Xfer) -> Result<(), String> {
    let mut behavior_module_version: XferVersion = 1;
    xfer.xfer_version(&mut behavior_module_version, 1)
        .map_err(|e| e.to_string())?;

    let mut object_module_version: XferVersion = 1;
    xfer.xfer_version(&mut object_module_version, 1)
        .map_err(|e| e.to_string())?;

    let mut module_version: XferVersion = 1;
    xfer.xfer_version(&mut module_version, 1)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub(crate) fn xfer_update_module_base_state(
    xfer: &mut dyn Xfer,
    next_call_frame_and_phase: &mut UnsignedInt,
) -> Result<(), String> {
    let mut update_module_version: XferVersion = 1;
    xfer.xfer_version(&mut update_module_version, 1)
        .map_err(|e| e.to_string())?;

    xfer_behavior_module_base_versions(xfer)?;

    xfer.xfer_unsigned_int(next_call_frame_and_phase)
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub trait SpyVisionUpdate: Send + Sync {
    fn set_disabled_until_frame(
        &mut self,
        _frame: UnsignedInt,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

// Specialized behavior interfaces
pub trait ParkingPlaceBehaviorInterface: Send + Sync {
    fn should_reserve_door_when_queued(&self, thing_template: &ObjectTemplate) -> Bool;
    fn has_available_space_for(&self, thing_template: &ObjectTemplate) -> Bool;
    fn has_reserved_space(&self, id: ObjectID) -> Bool;
    fn get_space_index(&self, id: ObjectID) -> Int;
    fn reserve_space(&mut self, id: ObjectID, parking_offset: Real, info: &mut PPInfo) -> Bool;
    fn release_space(&mut self, id: ObjectID);
    fn reserve_runway(&mut self, id: ObjectID, for_landing: Bool) -> Bool;
    fn calc_pp_info(&self, id: ObjectID, info: &mut PPInfo);
    fn release_runway(&mut self, id: ObjectID);
    fn get_runway_count(&self) -> Int;
    fn get_runway_reservation(&self, r: Int, reservation_type: RunwayReservationType) -> ObjectID;
    fn transfer_runway_reservation_to_next_in_line_for_takeoff(&mut self, id: ObjectID);
    fn get_approach_height(&self) -> Real;
    fn get_landing_deck_height_offset(&self) -> Real;
    fn set_healee(&mut self, healee: Option<Arc<RwLock<Object>>>, add: Bool);
    fn kill_all_parked_units(&mut self);
    fn defect_all_parked_units(&mut self, new_team: Arc<RwLock<Team>>, detection_time: UnsignedInt);
    fn calc_best_parking_assignment(
        &mut self,
        id: ObjectID,
        pos: &mut Coord3D,
        old_index: Option<&mut Int>,
        new_index: Option<&mut Int>,
    ) -> Bool;
    fn get_taxi_locations(&self, id: ObjectID) -> Option<&Vec<Coord3D>>;
    fn get_creation_locations(&self, id: ObjectID) -> Option<&Vec<Coord3D>>;
}

pub trait RebuildHoleBehaviorInterface: Send + Sync {
    fn start_rebuild_process(
        &mut self,
        rebuild_template: Arc<dyn crate::common::ThingTemplate>,
        spawner_id: ObjectID,
    );
    fn get_spawner_id(&self) -> ObjectID;
    fn get_reconstructed_building_id(&self) -> ObjectID;
    fn get_rebuild_template(&self) -> Option<Arc<dyn crate::common::ThingTemplate>>;
}

pub trait BridgeBehaviorInterface: Send + Sync {
    fn set_tower(&mut self, tower_type: BridgeTowerType, tower: Option<Arc<RwLock<Object>>>);
    fn get_tower_id(&self, tower_type: BridgeTowerType) -> ObjectID;
    fn create_scaffolding(&mut self);
    fn remove_scaffolding(&mut self);
    fn is_scaffold_in_motion(&self) -> Bool;
    fn is_scaffold_present(&self) -> Bool;
}

pub trait BridgeTowerBehaviorInterface: Send + Sync {
    fn set_bridge(&mut self, bridge: Option<Arc<RwLock<Object>>>);
    fn get_bridge_id(&self) -> ObjectID;
    fn set_tower_type(&mut self, tower_type: BridgeTowerType);
}

pub trait BridgeScaffoldBehaviorInterface: Send + Sync {
    fn set_positions(&mut self, create_pos: &Coord3D, rise_to_pos: &Coord3D, build_pos: &Coord3D);
    fn set_motion(&mut self, target_motion: ScaffoldTargetMotion);
    fn get_current_motion(&self) -> ScaffoldTargetMotion;
    fn reverse_motion(&mut self);
    fn set_lateral_speed(&mut self, lateral_speed: Real);
    fn set_vertical_speed(&mut self, vertical_speed: Real);
}

pub trait OverchargeBehaviorInterface: Send + Sync {
    fn toggle(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn enable(&mut self, enable: Bool) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
    fn is_overcharge_active(&self) -> Bool;
}

pub trait TransportPassengerInterface: Send + Sync {
    fn try_to_evacuate(&mut self, expose_stealthed_units: Bool) -> Bool;
}

pub trait CaveInterface: Send + Sync {
    fn try_to_set_cave_index(&mut self, new_index: Int);
    fn set_original_team(&mut self, old_team: Option<Arc<RwLock<Team>>>);
}

pub trait LandMineInterface: Send + Sync {
    fn set_scoot_parms(&mut self, start: &Coord3D, end: &Coord3D);
    fn disarm(&mut self);
}

// Supporting types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunwayReservationType {
    Takeoff,
    Landing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BridgeTowerType {
    North,
    South,
    East,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScaffoldTargetMotion {
    Still,
    Rise,
    BuildAcross,
    TearDownAcross,
    Sink,
}

#[derive(Debug, Clone)]
pub struct PPInfo {
    pub parking_space: Coord3D,
    pub parking_orientation: Real,
    pub runway_prep: Coord3D,
    pub runway_start: Coord3D,
    pub runway_end: Coord3D,
    pub runway_exit: Coord3D,
    pub runway_landing_start: Coord3D,
    pub runway_landing_end: Coord3D,
    pub runway_approach: Coord3D,
    pub hangar_internal: Coord3D,
    pub runway_takeoff_dist: Real,
    pub hangar_internal_orient: Real,
}

impl Default for PPInfo {
    fn default() -> Self {
        Self {
            parking_space: Coord3D::origin(),
            parking_orientation: 0.0,
            runway_prep: Coord3D::origin(),
            runway_start: Coord3D::origin(),
            runway_end: Coord3D::origin(),
            runway_exit: Coord3D::origin(),
            runway_landing_start: Coord3D::origin(),
            runway_landing_end: Coord3D::origin(),
            runway_approach: Coord3D::origin(),
            hangar_internal: Coord3D::origin(),
            runway_takeoff_dist: 0.0,
            hangar_internal_orient: 0.0,
        }
    }
}

/// BehaviorModuleData - Configuration data for behavior modules
#[derive(Debug, Clone)]
pub struct BehaviorModuleData {
    pub base: BaseModuleData,
}

impl BehaviorModuleData {
    pub fn new() -> Self {
        Self {
            base: BaseModuleData::new(),
        }
    }
}

impl Default for BehaviorModuleData {
    fn default() -> Self {
        Self::new()
    }
}

impl game_engine::common::thing::module::ModuleData for BehaviorModuleData {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn set_module_tag_name_key(&mut self, key: game_engine::common::thing::module::NameKeyType) {
        self.base.set_module_tag_name_key(key);
    }

    fn get_module_tag_name_key(&self) -> game_engine::common::thing::module::NameKeyType {
        self.base.get_module_tag_name_key()
    }
}

impl Snapshotable for BehaviorModuleData {
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

/// BehaviorModule - Base implementation for all behavior modules
pub struct BehaviorModule {
    pub thing: Arc<RwLock<Object>>,
    pub module_data: Arc<dyn ModuleData>,
}

impl BehaviorModule {
    pub fn new(thing: Arc<RwLock<Object>>, module_data: Arc<dyn ModuleData>) -> Self {
        Self { thing, module_data }
    }

    pub fn get_interface_mask() -> Int {
        0
    }

    pub fn get_module_type() -> ModuleType {
        ModuleType::Behavior
    }

    // CRC for network synchronization
    pub fn crc(
        &self,
        _xfer: &mut dyn Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }

    // Transfer for serialization/deserialization
    pub fn xfer(
        &mut self,
        xfer: &mut dyn Xfer,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let current_version: XferVersion = 1;
        let mut version = current_version;
        xfer.xfer_version(&mut version, current_version)?;
        Ok(())
    }

    // Load post process
    pub fn load_post_process(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Ok(())
    }
}

pub use crate::common::{ObjectTemplate, Team};
pub use game_engine::common::thing::module::ModuleType;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_behavior_module_creation() {
        // Test basic behavior module functionality
        // This would require mock implementations of dependencies
    }

    #[test]
    fn test_behavior_interfaces() {
        // Test interface functionality
    }
}
