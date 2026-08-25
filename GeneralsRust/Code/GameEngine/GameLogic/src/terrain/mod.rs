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
use crate::ai::THE_AI;
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
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Wave 341: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// Host-only path: leftover scripts still raise water; Main applies DAMAGE_WATER.
static PENDING_HOST_WATER_RISE_DAMAGE: Mutex<Vec<f32>> = Mutex::new(Vec::new());

fn queue_host_water_rise_damage(amount: f32) {
    if amount > 0.0 {
        if let Ok(mut pending) = PENDING_HOST_WATER_RISE_DAMAGE.lock() {
            pending.push(amount);
        }
    }
}

/// Drain leftover WATER_CHANGE_HEIGHT damage for the live host.
pub fn take_pending_host_water_rise_damage() -> Vec<f32> {
    PENDING_HOST_WATER_RISE_DAMAGE
        .lock()
        .map(|mut pending| pending.drain(..).collect())
        .unwrap_or_default()
}

/// Host-only path: leftover water-height changes restamp live Water cells.
static PENDING_HOST_PATHFIND_RECALC: Mutex<bool> = Mutex::new(false);

fn queue_host_pathfind_recalculation() {
    if let Ok(mut pending) = PENDING_HOST_PATHFIND_RECALC.lock() {
        *pending = true;
    }
}

/// Drain leftover `forceMapRecalculation` so the live host restamps Water cells.
pub fn take_pending_host_pathfind_recalculation() -> bool {
    PENDING_HOST_PATHFIND_RECALC
        .lock()
        .map(|mut pending| {
            let was = *pending;
            *pending = false;
            was
        })
        .unwrap_or(false)
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
const WATER_GRID_NAME_CPP: &str = "Water Grid";
const WATER_GRID_NAME_LEGACY: &str = "GridWater";
const MAX_DYNAMIC_WATER_ENTRIES: usize = 64;
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

/// Bridge information structure
#[derive(Debug, Clone)]
pub struct BridgeInfo {
    /// The points that the bridge was drawn using
    pub from: Coord3D,
    pub to: Coord3D,
    /// Width of the bridge
    pub bridge_width: f32,
    /// The 4 corners of the rectangle that the bridge covers
    pub from_left: Coord3D,
    pub from_right: Coord3D,
    pub to_left: Coord3D,
    pub to_right: Coord3D,
    /// The index to the drawable bridges
    pub bridge_index: i32,
    /// Current damage state
    pub cur_damage_state: BodyDamageType,
    /// Associated object IDs
    pub bridge_object_id: ObjectID,
    pub tower_object_id: [ObjectID; BRIDGE_MAX_TOWERS],
    /// Whether damage state changed
    pub damage_state_changed: bool,
}

impl BridgeInfo {
    pub fn new() -> Self {
        Self {
            from: Coord3D::origin(),
            to: Coord3D::origin(),
            bridge_width: 0.0,
            from_left: Coord3D::origin(),
            from_right: Coord3D::origin(),
            to_left: Coord3D::origin(),
            to_right: Coord3D::origin(),
            bridge_index: -1,
            cur_damage_state: BodyDamageType::Pristine,
            bridge_object_id: crate::common::INVALID_ID,
            tower_object_id: [crate::common::INVALID_ID; BRIDGE_MAX_TOWERS],
            damage_state_changed: false,
        }
    }
}

/// Bridge attack information
#[derive(Debug, Clone)]
pub struct BridgeAttackInfo {
    /// Points that can be attacked
    pub attack_point1: Coord3D,
    pub attack_point2: Coord3D,
}

impl BridgeAttackInfo {
    pub fn new() -> Self {
        Self {
            attack_point1: Coord3D::origin(),
            attack_point2: Coord3D::origin(),
        }
    }
}

/// C++ `ShroudStatusStoreRestore` buckets used by `setActiveBoundary`.
#[derive(Debug, Default)]
struct BoundaryShroudStore {
    fogged: Vec<(i32, i32, i32)>,
    revealed: Vec<(i32, i32, i32)>,
}

/// Bridge class for terrain logic
#[derive(Debug)]
pub struct Bridge {
    /// Link for traversing all bridges
    next: Option<Box<Bridge>>,
    /// Bridge template name
    template_name: AsciiString,
    /// Bridge information
    bridge_info: BridgeInfo,
    /// 2D bounds for quick screening
    bounds: Region2D,
    /// Pathfind layer for this bridge
    layer: PathfindLayerEnum,
}

impl Clone for Bridge {
    fn clone(&self) -> Self {
        Self {
            next: None,
            template_name: self.template_name.clone(),
            bridge_info: self.bridge_info.clone(),
            bounds: self.bounds.clone(),
            layer: self.layer,
        }
    }
}

impl Bridge {
    pub fn new(bridge_info: BridgeInfo, template_name: AsciiString) -> Self {
        // Calculate bounds from bridge info
        let min_x = bridge_info
            .from_left
            .x
            .min(bridge_info.from_right.x)
            .min(bridge_info.to_left.x)
            .min(bridge_info.to_right.x);
        let max_x = bridge_info
            .from_left
            .x
            .max(bridge_info.from_right.x)
            .max(bridge_info.to_left.x)
            .max(bridge_info.to_right.x);
        let min_y = bridge_info
            .from_left
            .y
            .min(bridge_info.from_right.y)
            .min(bridge_info.to_left.y)
            .min(bridge_info.to_right.y);
        let max_y = bridge_info
            .from_left
            .y
            .max(bridge_info.from_right.y)
            .max(bridge_info.to_left.y)
            .max(bridge_info.to_right.y);

        let bounds = Region2D::new(Coord2D::new(min_x, min_y), Coord2D::new(max_x, max_y));

        Self {
            next: None,
            template_name,
            bridge_info,
            bounds,
            layer: PathfindLayerEnum::Ground,
        }
    }

    /// Get bridge template name
    pub fn get_bridge_template_name(&self) -> &AsciiString {
        &self.template_name
    }

    /// Get next bridge in list
    pub fn get_next(&self) -> Option<&Bridge> {
        self.next.as_ref().map(|b| b.as_ref())
    }

    /// Get height for an object on bridge
    pub fn get_bridge_height(&self, location: &Coord3D, normal: Option<&mut Coord3D>) -> f32 {
        let p1 = self.bridge_info.from_left;
        let p2 = self.bridge_info.from_right;
        let p3 = self.bridge_info.to_left;

        let v1 = p2 - p1;
        let v2 = p3 - p1;
        let mut n = v1.cross(v2);
        let n_len = n.length();
        if n_len <= f32::EPSILON {
            if let Some(out) = normal {
                *out = Coord3D::new(0.0, 0.0, 1.0);
            }
            return p1.z;
        }

        n /= n_len;

        let z = if n.z.abs() > f32::EPSILON {
            p1.z - (n.x * (location.x - p1.x) + n.y * (location.y - p1.y)) / n.z
        } else {
            p1.z
        };

        if let Some(out) = normal {
            *out = n;
        }

        z
    }

    /// Get bridge logical info
    pub fn get_bridge_info(&self) -> &BridgeInfo {
        &self.bridge_info
    }
    pub(crate) fn bridge_info_mut(&mut self) -> &mut BridgeInfo {
        &mut self.bridge_info
    }

    /// Check if point is on bridge
    pub fn is_point_on_bridge(&self, location: &Coord3D) -> bool {
        // Simple bounds check first
        if location.x < self.bounds.lo.x
            || location.x > self.bounds.hi.x
            || location.y < self.bounds.lo.y
            || location.y > self.bounds.hi.y
        {
            return false;
        }

        let p = Coord2D::new(location.x, location.y);
        let quad = [
            Coord2D::new(self.bridge_info.from_left.x, self.bridge_info.from_left.y),
            Coord2D::new(self.bridge_info.from_right.x, self.bridge_info.from_right.y),
            Coord2D::new(self.bridge_info.to_right.x, self.bridge_info.to_right.y),
            Coord2D::new(self.bridge_info.to_left.x, self.bridge_info.to_left.y),
        ];

        point_in_convex_quad(&p, &quad)
    }

    /// Check if a cell region lies on the end of the bridge.
    pub fn is_cell_on_end(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.bridge_info.from_right - self.bridge_info.from_left;
        let len = end_vector.length();
        if len <= f32::EPSILON {
            return false;
        }
        end_vector /= len;
        end_vector *= PATHFIND_CELL_SIZE_F;

        let mut from_left = self.bridge_info.from_left;
        from_left.x += end_vector.x;
        from_left.y += end_vector.y;
        let mut from_right = self.bridge_info.from_right;
        from_right.x -= end_vector.x;
        from_right.y -= end_vector.y;

        let mut to_left = self.bridge_info.to_left;
        to_left.x += end_vector.x;
        to_left.y += end_vector.y;
        let mut to_right = self.bridge_info.to_right;
        to_right.x -= end_vector.x;
        to_right.y -= end_vector.y;

        let line1 = Coord2D::new(from_left.x, from_left.y);
        let line2 = Coord2D::new(from_right.x, from_right.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }
        let line1 = Coord2D::new(to_left.x, to_left.y);
        let line2 = Coord2D::new(to_right.x, to_right.y);
        line_in_region(&line1, &line2, cell)
    }

    /// Update damage state
    pub fn update_damage_state(&mut self) {
        crate::terrain_bridge::update_damage_state(self);
    }

    /// Get layer
    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    /// Set layer
    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        self.layer = layer;
    }

    /// Get bounds
    pub fn get_bounds(&self) -> &Region2D {
        &self.bounds
    }

    /// Set bridge object ID
    pub fn set_bridge_object_id(&mut self, id: ObjectID) {
        self.bridge_info.bridge_object_id = id;
    }

    /// Set tower object ID
    /// C++ `Bridge::setTowerObjectID` indexes `towerObjectID[which]`.
    pub fn set_tower_object_id(&mut self, id: ObjectID, which: BridgeTowerType) {
        let index = which as usize;
        if index < self.bridge_info.tower_object_id.len() {
            self.bridge_info.tower_object_id[index] = id;
        }
    }

    /// Check if a cell region lies on the side of the bridge.
    /// Reference: C++ TerrainLogic.cpp Bridge::isCellOnSide()
    ///
    /// This is used to determine if a pathfinding cell touches the sides
    /// of a bridge, which affects pathfinding calculations.
    pub fn is_cell_on_side(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.bridge_info.from_right - self.bridge_info.from_left;
        let len = end_vector.length();
        if len <= f32::EPSILON {
            return false;
        }
        end_vector /= len;
        // Offset by 0.51 pathfind cells for side detection
        end_vector *= PATHFIND_CELL_SIZE_F * 0.51;

        let mut from_left = self.bridge_info.from_left;
        from_left.x -= end_vector.x;
        from_left.y -= end_vector.y;

        let mut from_right = self.bridge_info.from_right;
        from_right.x += end_vector.x;
        from_right.y += end_vector.y;

        let mut to_left = self.bridge_info.to_left;
        to_left.x -= end_vector.x;
        to_left.y -= end_vector.y;

        let mut to_right = self.bridge_info.to_right;
        to_right.x += end_vector.x;
        to_right.y += end_vector.y;

        // Check left side of bridge
        let line1 = Coord2D::new(from_left.x, from_left.y);
        let line2 = Coord2D::new(to_left.x, to_left.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        // Check right side of bridge
        let line1 = Coord2D::new(from_right.x, from_right.y);
        let line2 = Coord2D::new(to_right.x, to_right.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        // Check with additional offset for wider detection
        from_left.x -= end_vector.x;
        from_left.y -= end_vector.y;
        from_right.x += end_vector.x;
        from_right.y += end_vector.y;
        to_left.x -= end_vector.x;
        to_left.y -= end_vector.y;
        to_right.x += end_vector.x;
        to_right.y += end_vector.y;

        let line1 = Coord2D::new(from_left.x, from_left.y);
        let line2 = Coord2D::new(to_left.x, to_left.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        let line1 = Coord2D::new(from_right.x, from_right.y);
        let line2 = Coord2D::new(to_right.x, to_right.y);
        line_in_region(&line1, &line2, cell)
    }

    /// Check if a pathfind cell is an entry point to the bridge.
    /// Reference: C++ TerrainLogic.cpp Bridge::isCellEntryPoint()
    ///
    /// Entry points are the areas at either end of the bridge where
    /// units can transition onto the bridge surface.
    pub fn is_cell_entry_point(&self, cell: &Region2D) -> bool {
        let mut end_vector = self.bridge_info.from_right - self.bridge_info.from_left;
        let len = end_vector.length();
        if len <= f32::EPSILON {
            return false;
        }
        end_vector /= len;
        // Offset by 1 pathfind cell
        end_vector *= PATHFIND_CELL_SIZE_F;

        let mut bridge_vector = self.bridge_info.to - self.bridge_info.from;
        let bridge_len = bridge_vector.length();
        if bridge_len <= f32::EPSILON {
            return false;
        }
        bridge_vector /= bridge_len;
        // Offset by half a pathfind cell along bridge direction
        bridge_vector *= PATHFIND_CELL_SIZE_F * 0.5;

        // Calculate entry point at 'from' end
        let mut from_left = self.bridge_info.from_left;
        from_left.x -= bridge_vector.x;
        from_left.y -= bridge_vector.y;
        from_left.x += end_vector.x;
        from_left.y += end_vector.y;

        let mut from_right = self.bridge_info.from_right;
        from_right.x -= bridge_vector.x;
        from_right.y -= bridge_vector.y;
        from_right.x -= end_vector.x;
        from_right.y -= end_vector.y;

        // Check 'from' entry point
        let line1 = Coord2D::new(from_left.x, from_left.y);
        let line2 = Coord2D::new(from_right.x, from_right.y);
        if line_in_region(&line1, &line2, cell) {
            return true;
        }

        // Calculate entry point at 'to' end
        let mut to_left = self.bridge_info.to_left;
        to_left.x += bridge_vector.x;
        to_left.y += bridge_vector.y;
        to_left.x += end_vector.x;
        to_left.y += end_vector.y;

        let mut to_right = self.bridge_info.to_right;
        to_right.x += bridge_vector.x;
        to_right.y += bridge_vector.y;
        to_right.x -= end_vector.x;
        to_right.y -= end_vector.y;

        // Check 'to' entry point
        let line1 = Coord2D::new(to_left.x, to_left.y);
        let line2 = Coord2D::new(to_right.x, to_right.y);
        line_in_region(&line1, &line2, cell)
    }
}

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

/// Dynamic water entry for animating water height over time
#[derive(Debug)]
struct DynamicWaterEntry {
    /// Polygon trigger ID associated with this water table (C++ xfer identity key).
    trigger_id: Int,
    /// Water table identity (name assigned from map trigger/editor).
    water_name: AsciiString,
    /// How much height to add each frame (negative = lowering)
    change_per_frame: f32,
    /// Target height we want to reach
    target_height: f32,
    /// Amount of damage to do to objects underwater
    damage_amount: f32,
    /// Current height (we track this ourselves)
    current_height: f32,
}

#[derive(Debug, Clone)]
pub struct TerrainDynamicWaterSnapshotEntry {
    pub trigger_id: Int,
    pub water_name: AsciiString,
    pub change_per_frame: f32,
    pub target_height: f32,
    pub damage_amount: f32,
    pub current_height: f32,
}

/// Terrain data loaded from map file
#[derive(Debug)]
struct TerrainData {
    heightmap: Vec<u8>,
    width: i32,
    height: i32,
    bridges: Vec<crate::system::map_loader::BridgeData>,
}

/// Main terrain logic system
pub struct TerrainLogic {
    /// Array of height samples
    map_data: Vec<u8>,
    /// Width of map samples
    map_dx: i32,
    /// Height of map samples
    map_dy: i32,
    /// Minimum loaded terrain height in world coordinates.
    map_min_z: f32,
    /// Maximum loaded terrain height in world coordinates.
    map_max_z: f32,
    /// Map boundaries
    boundaries: Vec<ICoord2D>,
    /// Border size in cells (matches map loader)
    border_size: i32,
    /// Packed WorldHeightMap cliff bits (8 cells/byte).
    cliff_state: crate::terrain_cliff::CliffBitfield,

    /// Active boundary index
    active_boundary: i32,
    /// Waypoint list head
    waypoint_list_head: Option<Box<Waypoint>>,
    /// Bridge list head
    bridge_list_head: Option<Box<Bridge>>,
    /// Bridge damage states changed flag
    bridge_damage_states_changed: bool,
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
    /// Grid water handle
    grid_water_handle: WaterHandle,
    /// Dynamic water tables to update
    water_to_update: Vec<DynamicWaterEntry>,
    /// Map of named water handles
    water_handles: HashMap<AsciiString, WaterHandle>,
    /// Map of trigger-ID keyed handles for identity-stable water operations.
    water_handles_by_trigger_id: HashMap<Int, WaterHandle>,
    /// Loaded terrain data (heightmap and bridges)
    terrain_data: Option<TerrainData>,
    /// Polygon trigger areas for scripts
    /// Matches C++ ThePolygonTriggerListPtr from PolygonTrigger.h
    trigger_areas: PolygonTriggerList,
}

// Terrain behavior is divided by C++ subsystem responsibility. Each child module extends
// the same `TerrainLogic` type, so the public API and call ordering remain unchanged.
mod bridge;
mod bridges;
mod geometry;
mod map_height;
mod query;
mod terrain_ops;
#[cfg(test)]
mod tests;
mod water;
mod waypoint;

// Keep the wrapper's original public path (`crate::terrain::TerrainQueryWrapper`).
pub use query::TerrainQueryWrapper;

// These geometry helpers are shared by the legacy Bridge implementation above and the
// split behavior modules. They remain private to the terrain subsystem.
use geometry::{
    cross_2d, line_in_region, path_with_map_variants, point_in_convex_quad, point_in_rotated_rect,
};

// Global terrain logic instance
lazy_static! {
    pub static ref THE_TERRAIN_LOGIC: Arc<RwLock<TerrainLogic>> =
        Arc::new(RwLock::new(TerrainLogic::new()));
}

/// Get reference to global terrain logic instance
/// Convenience accessor for terrain queries
pub fn get_terrain_logic() -> &'static Arc<RwLock<TerrainLogic>> {
    &THE_TERRAIN_LOGIC
}

/// Initialize terrain logic with physics engine
/// Sets up terrain query interface for physics/locomotor integration
pub fn init_terrain_physics_integration() {
    use crate::physics::get_physics_engine;

    // Get physics engine
    if let Ok(mut physics) = get_physics_engine().write() {
        // Create wrapper that implements TerrainQuery
        let wrapper = TerrainQueryWrapper::new(THE_TERRAIN_LOGIC.clone());
        let terrain_query: Arc<dyn TerrainQuery> = Arc::new(wrapper);
        physics.set_terrain_query(terrain_query);
    }
}
