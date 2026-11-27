//! Terrain Logic System - Rust Implementation
//!
//! Logical terrain representation for the game logic side.
//! Based on TerrainLogic.h from the original C++ implementation.
//!
//! This module provides:
//! - Height map management and terrain height queries
//! - Bridge management and pathfinding layer support
//! - Waypoint system for AI navigation
//! - Water table and dynamic water effects
//! - Line of sight calculations
//! - Terrain flattening for buildings

use crate::ai::pathfind_complete::GridCoord;
use crate::ai::the_ai;
use crate::common::CoordOrigin;
use crate::common::*;
use crate::damage::{DamageInfo, DamageType, DeathType};
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::PathfindLayerEnum;
use crate::path::{LAYER_Z_CLOSE_ENOUGH_F, PATHFIND_CELL_SIZE_F};
use crate::physics::{SurfaceType, TerrainQuery};
use crate::polygon_trigger::{PolygonTrigger, PolygonTriggerList};
use crate::system::map_loader::MapWaypoint;
use game_engine::system::geometry::GeometryType as EngineGeometryType;
use generals_sim::terrain::{WATER_GRID_NAME_CPP, WaterTransition, WaterTransitions};
pub(crate) use generals_sim::terrain::MAX_DYNAMIC_WATER_ENTRIES;
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

// Retained standalone Core bridge/wall adapters; Main terrain phases never
// choose their owner from registry emptiness.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Maximum terrain name length
pub const MAX_TERRAIN_NAME_LEN: usize = 64;

pub use generals_sim::terrain::{CAMERA_WAYPOINT_PATH_LIMIT, WAYPOINT_PATH_LIMIT, WaveGuide1Bind};
use generals_sim::terrain::{TerrainBoundaries, WaypointGraph};

/// Retain the original string representation at the legacy API boundary.
pub type Waypoint = generals_sim::terrain::Waypoint<AsciiString>;

pub use generals_sim::terrain::{BridgeAttackInfo, BridgeInfo};
use generals_sim::terrain::BridgeList;

/// Legacy names and layer ordinals cross the terrain adapter unchanged.
pub type Bridge = generals_sim::terrain::Bridge<AsciiString, PathfindLayerEnum>;

/// Water handle for dynamic water management
#[derive(Debug, Clone)]
pub struct WaterHandle {
    name: AsciiString,
    current_height: f32,
    base_height: f32,
    bounds: Region3D,
}

impl WaterHandle {
    pub fn new(name: AsciiString, height: f32, bounds: Region3D) -> Self {
        Self {
            name,
            current_height: height,
            base_height: height,
            bounds,
        }
    }

    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    pub fn get_current_height(&self) -> f32 {
        self.current_height
    }

    pub fn set_height(&mut self, height: f32) {
        self.current_height = height;
    }

    pub fn get_bounds(&self) -> &Region3D {
        &self.bounds
    }
}

/// Positional save data for the receiving timed water transitions.
pub type TerrainDynamicWaterSnapshotEntry = WaterTransition<AsciiString>;

/// Main terrain logic system
pub struct TerrainLogic {
    height_map: generals_sim::terrain::TerrainHeightMap,
    boundaries: TerrainBoundaries,
    waypoints: WaypointGraph<AsciiString>,
    bridges: BridgeList<AsciiString, PathfindLayerEnum>,
    /// Filename for terrain data
    filename_string: AsciiString,
    /// Query-mode map load marker.
    ///
    /// When `load_map(..., true)` is used we still populate logical terrain state,
    /// but we suppress the follow-up `new_map` finalization so probe-only loads do
    /// not trigger the client-facing side effects that C++ skips in query mode.
    query_load_pending: bool,
    /// Water grid enabled flag
    water_grid_enabled: bool,
    water_grid: generals_sim::terrain::WaterGridState,
    /// Grid water handle
    grid_water_handle: WaterHandle,
    /// Dynamic water tables to update
    water_to_update: WaterTransitions<AsciiString>,
    /// Map of named water handles
    water_handles: HashMap<AsciiString, WaterHandle>,
    /// Map of trigger-ID keyed handles for identity-stable water operations.
    water_handles_by_trigger_id: HashMap<Int, WaterHandle>,
    /// Authored bridge geometry; height_map owns the live logical heights.
    authored_bridges: Vec<crate::system::map_loader::BridgeData>,
    /// Polygon trigger areas for scripts
    /// Matches C++ ThePolygonTriggerListPtr from PolygonTrigger.h
    trigger_areas: PolygonTriggerList,
}

// Terrain behavior is divided by C++ subsystem responsibility. Each child module extends
// the same `TerrainLogic` type, so the public API and call ordering remain unchanged.
mod bridge;
mod bridges;
#[cfg(test)]
mod closest_waypoint_tests;
mod geometry;
#[cfg(test)]
mod height_owner_tests;
mod map_height;
mod query;
mod terrain_ops;
#[cfg(test)]
mod terrain_ray;
pub use generals_sim::terrain::{
    TerrainHeightChange, TerrainHeightSamples, TerrainRayCast, source_height_cell_floor,
};

#[cfg(test)]
mod tests;
mod water;
pub use water::WaterHeightChange;
mod waypoint;

// Keep the wrapper's original public path (`crate::terrain::TerrainQueryWrapper`).
pub use query::TerrainQueryWrapper;

// These geometry helpers are shared by the legacy Bridge implementation above and the
// split behavior modules. They remain private to the terrain subsystem.
use geometry::{path_with_map_variants, point_in_rotated_rect};

/// The published world's terrain logic (C++ `TheTerrainLogic`).
pub fn get_terrain_logic() -> Arc<RwLock<TerrainLogic>> {
    Arc::clone(crate::system::engine_stores::active_services().terrain())
}

/// Initialize terrain logic with physics engine
/// Sets up terrain query interface for physics/locomotor integration
pub fn init_terrain_physics_integration() {
    use crate::physics::get_physics_engine;

    // Get physics engine
    if let Ok(mut physics) = get_physics_engine().write() {
        // Create wrapper that implements TerrainQuery
        let wrapper = TerrainQueryWrapper::new(get_terrain_logic());
        let terrain_query: Arc<dyn TerrainQuery> = Arc::new(wrapper);
        physics.set_terrain_query(terrain_query);
    }
}

#[cfg(test)]
mod construction_flatten_cpp_tests;

#[cfg(test)]
mod map_height_source_tests;
