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

/// C++ WaveGuideUpdate::startMoving WaveGuide1 bind result.
#[derive(Debug, Clone, Copy)]
pub enum WaveGuide1Bind {
    Follow {
        first: Coord3D,
        last: Coord3D,
        angle: f32,
    },
    MissingWaypoint,
    InvalidPath,
}

/// Maximum terrain name length
pub const MAX_TERRAIN_NAME_LEN: usize = 64;
/// C++ `AIUpdate.cpp`: `enum {WAYPOINT_PATH_LIMIT=1024}`. Caps `link[0]` walks
/// so ShellMapMD 1-link rings (Car_Path) cannot grow an unbounded Vec.
pub const WAYPOINT_PATH_LIMIT: usize = 1024;
/// C++ `W3DView.h`: `enum {MAX_WAYPOINTS=25}` for camera path collection.
pub const CAMERA_WAYPOINT_PATH_LIMIT: usize = 25;

/// Waypoint helper class for waypoint info in terrain logic
#[derive(Debug)]
pub struct Waypoint {
    /// Unique integer identifier
    id: WaypointID,
    /// Name
    name: AsciiString,
    /// Location
    location: Coord3D,
    /// Next waypoint in linked list
    next: Option<Box<Waypoint>>,
    /// Directed graph of waypoints (up to 8 links)
    links: Vec<WaypointID>,
    /// Path labels for waypoint classification
    path_label1: AsciiString,
    path_label2: AsciiString,
    path_label3: AsciiString,
    /// Whether path is bidirectional
    bi_directional: bool,
}

impl Waypoint {
    const MAX_LINKS: usize = 8;

    pub fn new(
        id: WaypointID,
        name: AsciiString,
        location: &Coord3D,
        label1: AsciiString,
        label2: AsciiString,
        label3: AsciiString,
        bi_directional: bool,
    ) -> Self {
        Self {
            id,
            name,
            location: *location,
            next: None,
            links: Vec::with_capacity(Self::MAX_LINKS),
            path_label1: label1,
            path_label2: label2,
            path_label3: label3,
            bi_directional,
        }
    }

    /// Get the next waypoint in the linked list
    pub fn get_next(&self) -> Option<&Waypoint> {
        self.next.as_ref().map(|w| w.as_ref())
    }

    /// Get number of links from this waypoint
    pub fn get_num_links(&self) -> usize {
        self.links.len()
    }

    /// Get the nth directed link
    pub fn get_link(&self, index: usize) -> Option<WaypointID> {
        if index < self.links.len() {
            Some(self.links[index])
        } else {
            None
        }
    }

    /// Get the waypoint's name
    pub fn get_name(&self) -> &AsciiString {
        &self.name
    }

    /// Get the waypoint ID
    pub fn get_id(&self) -> WaypointID {
        self.id
    }

    /// Get the waypoint location
    pub fn get_location(&self) -> &Coord3D {
        &self.location
    }

    /// Get path labels
    pub fn get_path_label1(&self) -> &AsciiString {
        &self.path_label1
    }

    pub fn get_path_label2(&self) -> &AsciiString {
        &self.path_label2
    }

    pub fn get_path_label3(&self) -> &AsciiString {
        &self.path_label3
    }

    /// Get bidirectional flag
    pub fn get_bi_directional(&self) -> bool {
        self.bi_directional
    }

    /// Add a link to another waypoint
    pub fn add_link(&mut self, waypoint: WaypointID) {
        if self.links.len() < Self::MAX_LINKS {
            self.links.push(waypoint);
        }
    }

    pub fn has_link(&self, waypoint: WaypointID) -> bool {
        self.links.iter().any(|id| *id == waypoint)
    }

    /// Set location Z coordinate
    pub fn set_location_z(&mut self, z: f32) {
        self.location.z = z;
    }

    pub fn matches_path_label(&self, label: &str) -> bool {
        self.path_label1.as_str().eq_ignore_ascii_case(label)
            || self.path_label2.as_str().eq_ignore_ascii_case(label)
            || self.path_label3.as_str().eq_ignore_ascii_case(label)
    }
}

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
    /// Map boundaries
    boundaries: Vec<ICoord2D>,
    /// Active boundary index
    active_boundary: i32,
    /// Waypoint list head
    waypoint_list_head: Option<Box<Waypoint>>,
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
