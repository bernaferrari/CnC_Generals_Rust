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

/// Maximum terrain name length
pub const MAX_TERRAIN_NAME_LEN: usize = 64;
const WATER_GRID_NAME_CPP: &str = "Water Grid";
const WATER_GRID_NAME_LEGACY: &str = "GridWater";
const MAX_DYNAMIC_WATER_ENTRIES: usize = 64;

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
        self.bridge_info.damage_state_changed = false;
        if self.bridge_info.bridge_object_id == crate::common::INVALID_ID {
            return;
        }

        let Some(obj_arc) =
            crate::helpers::TheGameLogic::find_object_by_id(self.bridge_info.bridge_object_id)
        else {
            return;
        };
        let Ok(obj_guard) = obj_arc.read() else {
            return;
        };

        let next_state = if obj_guard.is_destroyed() {
            BodyDamageType::Rubble
        } else {
            let max = obj_guard.get_max_health().max(f32::EPSILON);
            let percent = (obj_guard.get_health() / max).clamp(0.0, 1.0);

            if percent >= 0.75 {
                BodyDamageType::Pristine
            } else if percent >= 0.5 {
                BodyDamageType::Damaged
            } else if percent >= 0.25 {
                BodyDamageType::ReallyDamaged
            } else {
                BodyDamageType::Rubble
            }
        };

        if next_state != self.bridge_info.cur_damage_state {
            self.bridge_info.cur_damage_state = next_state;
            self.bridge_info.damage_state_changed = true;
        }
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
    pub fn set_tower_object_id(&mut self, id: ObjectID, which: BridgeTowerType) {
        match which {
            BridgeTowerType::From => self.bridge_info.tower_object_id[0] = id,
            BridgeTowerType::To => self.bridge_info.tower_object_id[1] = id,
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
    /// Map boundaries
    boundaries: Vec<ICoord2D>,
    /// Border size in cells (matches map loader)
    border_size: i32,
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

impl TerrainLogic {
    fn bridge_pathfinder_bounds(bridge_info: &BridgeInfo) -> (GridCoord, GridCoord) {
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

        (
            GridCoord::new(
                (min_x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (min_y / PATHFIND_CELL_SIZE_F).floor() as i32,
            ),
            GridCoord::new(
                (max_x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (max_y / PATHFIND_CELL_SIZE_F).floor() as i32,
            ),
        )
    }

    fn bridge_info_from_parts(
        position: Coord3D,
        angle: Real,
        halfsize_x: Real,
        halfsize_y: Real,
        bridge_object_id: ObjectID,
    ) -> BridgeInfo {
        let c = angle.cos();
        let s = angle.sin();

        let from_left = Coord3D::new(
            position.x - halfsize_x * c - halfsize_y * s,
            position.y + halfsize_y * c - halfsize_x * s,
            position.z,
        );
        let to_left = Coord3D::new(
            position.x + halfsize_x * c - halfsize_y * s,
            position.y + halfsize_y * c + halfsize_x * s,
            position.z,
        );
        let from_right = Coord3D::new(
            position.x - halfsize_x * c + halfsize_y * s,
            position.y - halfsize_y * c - halfsize_x * s,
            position.z,
        );
        let to_right = Coord3D::new(
            position.x + halfsize_x * c + halfsize_y * s,
            position.y - halfsize_y * c + halfsize_x * s,
            position.z,
        );

        let mut bridge_info = BridgeInfo::new();
        bridge_info.from_left = from_left;
        bridge_info.from_right = from_right;
        bridge_info.to_left = to_left;
        bridge_info.to_right = to_right;
        bridge_info.from = Coord3D::new(
            (from_left.x + from_right.x) * 0.5,
            (from_left.y + from_right.y) * 0.5,
            (from_left.z + from_right.z) * 0.5,
        );
        bridge_info.to = Coord3D::new(
            (to_left.x + to_right.x) * 0.5,
            (to_left.y + to_right.y) * 0.5,
            (to_left.z + to_right.z) * 0.5,
        );
        bridge_info.bridge_width = halfsize_y * 2.0;
        bridge_info.bridge_object_id = bridge_object_id;
        bridge_info
    }

    pub(crate) fn bridge_info_from_object(bridge_obj: &Object) -> BridgeInfo {
        let position = *bridge_obj.get_position();
        let angle = bridge_obj.get_orientation();
        let geometry = bridge_obj.get_geometry_info();
        Self::bridge_info_from_parts(
            position,
            angle,
            geometry.get_major_radius(),
            geometry.get_minor_radius(),
            bridge_obj.get_id(),
        )
    }

    fn register_bridge_with_pathfinder(bridge_info: &BridgeInfo) -> Option<PathfindLayerEnum> {
        let (min_coord, max_coord) = Self::bridge_pathfinder_bounds(bridge_info);
        let ai_guard = THE_AI.read().ok()?;
        let pathfinder = ai_guard.pathfinder()?;
        let mut pathfinder_guard = pathfinder.write().ok()?;
        Some(pathfinder_guard.add_bridge((min_coord, max_coord)))
    }

    fn remove_bridge_at(&mut self, location: &Coord3D) -> bool {
        let mut current = &mut self.bridge_list_head;
        loop {
            let should_remove = match current.as_ref() {
                Some(bridge) => bridge.is_point_on_bridge(location),
                None => return false,
            };

            if should_remove {
                let next = current.as_mut().and_then(|bridge| bridge.next.take());
                *current = next;
                self.bridge_damage_states_changed = true;
                return true;
            }

            current = &mut current.as_mut().expect("bridge node exists").next;
        }
    }

    pub fn new() -> Self {
        Self {
            map_data: Vec::new(),
            map_dx: 0,
            map_dy: 0,
            boundaries: Vec::new(),
            border_size: 0,
            active_boundary: 0,
            waypoint_list_head: None,
            bridge_list_head: None,
            bridge_damage_states_changed: false,
            filename_string: String::new().into(),
            query_load_pending: false,
            water_grid_enabled: false,
            grid_water_handle: WaterHandle::new(
                WATER_GRID_NAME_CPP.to_string().into(),
                0.0,
                Region3D::default(),
            ),
            water_to_update: Vec::new(),
            water_handles: HashMap::new(),
            water_handles_by_trigger_id: HashMap::new(),
            terrain_data: None,
            trigger_areas: PolygonTriggerList::new(),
        }
    }

    /// Load map data from parsed map file
    /// Reference: C++ TerrainLogic.cpp loadMap() integration
    ///
    /// # Arguments
    /// * `map_data` - Parsed map data from MapLoader
    pub fn load_map_data(&mut self, map_data: crate::system::map_loader::MapData) {
        self.query_load_pending = false;

        // Store heightmap
        self.map_data = map_data.heightmap.clone();
        self.map_dx = map_data.width as i32;
        self.map_dy = map_data.height as i32;
        self.boundaries = map_data.boundaries.clone();
        self.border_size = map_data.border_size;

        // Store terrain data including bridges
        self.terrain_data = Some(TerrainData {
            heightmap: map_data.heightmap,
            width: map_data.width as i32,
            height: map_data.height as i32,
            bridges: map_data.bridges,
        });

        // Rebuild grid-water handle using C++ sentinel name and map extent.
        let grid_height = map_data
            .water_height
            .unwrap_or(self.grid_water_handle.get_current_height());
        self.grid_water_handle = WaterHandle::new(
            WATER_GRID_NAME_CPP.to_string().into(),
            grid_height,
            self.get_extent_including_border(),
        );

        // Load polygon trigger areas from map data
        self.trigger_areas.clear();
        self.water_handles.clear();
        self.water_handles_by_trigger_id.clear();
        for trigger in map_data.polygon_triggers {
            self.add_trigger_area(trigger);
        }

        // Load waypoints and links
        self.waypoint_list_head = None;
        for waypoint in &map_data.waypoints {
            self.add_waypoint_from_map(waypoint);
        }
        for (id1, id2) in &map_data.waypoint_links {
            self.add_waypoint_link(*id1, *id2);
        }
    }

    /// Snapshot parsed map bridge geometry.
    pub fn bridge_data_snapshot(&self) -> Vec<crate::system::map_loader::BridgeData> {
        self.terrain_data
            .as_ref()
            .map(|terrain_data| terrain_data.bridges.clone())
            .unwrap_or_default()
    }

    /// Get map extent including border in world coordinates.
    pub fn get_extent_including_border(&self) -> Region3D {
        let width = (self.map_dx.max(0) as f32) * MAP_XY_FACTOR;
        let height = (self.map_dy.max(0) as f32) * MAP_XY_FACTOR;
        Region3D::new(
            Coord3D::new(0.0, 0.0, 0.0),
            Coord3D::new(width, height, 0.0),
        )
    }

    /// Get maximum pathfind extent (playable area excluding border).
    pub fn get_maximum_pathfind_extent(&self) -> Region3D {
        let width = (self.map_dx.max(0) as f32) * MAP_XY_FACTOR;
        let height = (self.map_dy.max(0) as f32) * MAP_XY_FACTOR;
        let border = (self.border_size.max(0) as f32) * MAP_XY_FACTOR;

        let lo_x = border.min(width);
        let lo_y = border.min(height);
        let hi_x = (width - border).max(lo_x);
        let hi_y = (height - border).max(lo_y);

        Region3D::new(Coord3D::new(lo_x, lo_y, 0.0), Coord3D::new(hi_x, hi_y, 0.0))
    }

    /// Get the map extent in world coordinates.
    /// Reference: C++ TerrainLogic::getExtent()
    ///
    /// Returns the bounding box of the playable map area.
    pub fn get_extent(&self) -> Region3D {
        // Use the maximum pathfind extent as the primary extent
        self.get_maximum_pathfind_extent()
    }

    /// Initialize the terrain system
    pub fn init(&mut self) {
        // Initialize terrain system
        self.reset();
    }

    /// Reset the terrain system
    pub fn reset(&mut self) {
        self.map_data.clear();
        self.border_size = 0;
        self.waypoint_list_head = None;
        self.bridge_list_head = None;
        self.water_to_update.clear();
        self.water_handles.clear();
        self.bridge_damage_states_changed = false;
        self.trigger_areas.clear();
        self.query_load_pending = false;
    }

    /// Update the terrain system
    pub fn update(&mut self) {
        // Update dynamic water tables
        self.update_dynamic_water();

        // Update bridge damage states
        self.update_bridge_damage_states();
    }

    /// Load map from file
    pub fn load_map(&mut self, filename: AsciiString, _query: bool) -> bool {
        self.filename_string = filename.clone();
        self.query_load_pending = false;
        let requested = filename.as_str();

        let Some(map_path) = self.resolve_map_path(requested) else {
            log::warn!("TerrainLogic::load_map: map file '{}' not found", requested);
            return false;
        };

        match crate::system::map_loader::MapLoader::load(&map_path) {
            Ok(map_data) => {
                self.reset();
                self.filename_string = filename;
                self.load_map_data(map_data);
                self.query_load_pending = _query;
                true
            }
            Err(err) => {
                log::error!(
                    "TerrainLogic::load_map: failed to parse map '{}' (resolved '{}'): {:?}",
                    requested,
                    map_path.display(),
                    err
                );
                false
            }
        }
    }

    fn resolve_map_path(&self, filename: &str) -> Option<PathBuf> {
        let trimmed = filename.trim();
        if trimmed.is_empty() {
            return None;
        }

        let input = PathBuf::from(trimmed);
        let mut variants = path_with_map_variants(&input);
        if input.extension().is_none() && input.components().count() == 1 {
            variants.push(input.join(format!("{trimmed}.map")));
            variants.push(input.join(format!("{trimmed}.MAP")));
        }

        let mut candidates = Vec::new();
        if input.is_absolute() {
            candidates.extend(variants);
        } else {
            candidates.extend(variants.clone());

            if let Ok(cwd) = std::env::current_dir() {
                for relative in &variants {
                    candidates.push(cwd.join(relative));
                    candidates.push(cwd.join("Maps").join(relative));
                    candidates.push(cwd.join("maps").join(relative));
                    candidates.push(cwd.join("Data").join("Maps").join(relative));
                    candidates.push(cwd.join("data").join("maps").join(relative));
                    candidates.push(cwd.join("GeneralsZHData").join("Maps").join(relative));
                }
            }
        }

        let mut seen = HashSet::new();
        for candidate in candidates {
            if !seen.insert(candidate.clone()) {
                continue;
            }
            if candidate.is_file() {
                return Some(candidate);
            }
        }
        None
    }

    /// Set source map filename used by `get_source_filename()`.
    ///
    /// C++ parity: `TerrainLogic::loadMap()` stores `m_filenameString` before
    /// finalization and client notification paths consume that value.
    pub fn set_source_filename(&mut self, filename: AsciiString) {
        self.filename_string = filename;
    }

    /// Initialize for new map
    ///
    /// C++ parity: this is a post-load finalize step, not a full terrain reset.
    /// It aligns waypoint Z with loaded ground height and enables water grid only
    /// when the map defines the legacy `WaveGuide1` marker.
    pub fn new_map(&mut self, _save_game: bool) {
        if self.query_load_pending {
            self.query_load_pending = false;
            return;
        }

        let mut waypoint_heights = Vec::new();
        let mut current = self.waypoint_list_head.as_deref();
        while let Some(waypoint) = current {
            let loc = waypoint.get_location();
            waypoint_heights.push((
                waypoint.get_id(),
                self.get_ground_height(loc.x, loc.y, None),
            ));
            current = waypoint.get_next();
        }

        for (id, z) in waypoint_heights {
            if let Some(waypoint) = self.get_waypoint_by_id_mut(id) {
                waypoint.set_location_z(z);
            }
        }

        let wave_guide = AsciiString::from("WaveGuide1");
        self.enable_water_grid(self.get_waypoint_by_name(&wave_guide).is_some());
    }

    /// Get ground height at position
    pub fn get_ground_height(&self, x: f32, y: f32, normal: Option<&mut Coord3D>) -> f32 {
        if self.map_data.is_empty() || self.map_dx <= 0 || self.map_dy <= 0 {
            if let Some(n) = normal {
                *n = Coord3D::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        }

        let map_x = x / MAP_XY_FACTOR;
        let map_y = y / MAP_XY_FACTOR;

        // Bounds check
        if map_x < 0.0
            || map_y < 0.0
            || map_x > (self.map_dx - 1).max(0) as f32
            || map_y > (self.map_dy - 1).max(0) as f32
        {
            if let Some(n) = normal {
                *n = Coord3D::new(0.0, 0.0, 1.0);
            }
            return 0.0;
        }

        let ixf = map_x.floor();
        let iyf = map_y.floor();
        let fx = map_x - ixf;
        let fy = map_y - iyf;

        let ix = ixf as i32;
        let iy = iyf as i32;

        let x_extent = self.map_dx;
        let y_extent = self.map_dy;

        let get_height_sample = |gx: i32, gy: i32| -> f32 {
            let idx = (gy * x_extent + gx) as usize;
            if gx >= 0 && gy >= 0 && gx < x_extent && gy < y_extent && idx < self.map_data.len() {
                self.map_data[idx] as f32
            } else {
                0.0
            }
        };

        let p0 = get_height_sample(ix, iy);
        let p1 = get_height_sample(ix + 1, iy);
        let p2 = get_height_sample(ix + 1, iy + 1);
        let p3 = get_height_sample(ix, iy + 1);

        // Triangle-based barycentric interpolation matching C++ BaseHeightMapRenderObjClass::getHeightMapHeight
        // C++ tessellation: diagonal from (0,0) to (1,1)
        //   3-----2
        //   |    /|
        //   |  /  |
        //   |/    |
        //   0-----1
        let height = if fy > fx {
            // Upper triangle: vertices p0, p2, p3
            (p3 + (1.0 - fy) * (p0 - p3) + fx * (p2 - p3)) * MAP_HEIGHT_SCALE
        } else {
            // Lower triangle: vertices p0, p1, p2
            (p1 + fy * (p2 - p1) + (1.0 - fx) * (p0 - p1)) * MAP_HEIGHT_SCALE
        };

        if let Some(n) = normal {
            let dx = (p1 - p0) / MAP_XY_FACTOR;
            let dy = (p3 - p0) / MAP_XY_FACTOR;
            let mut nx = -dx;
            let mut ny = -dy;
            let mut nz = 1.0;
            let len = (nx * nx + ny * ny + nz * nz).sqrt();
            if len > f32::EPSILON {
                nx /= len;
                ny /= len;
                nz /= len;
            }
            *n = Coord3D::new(nx, ny, nz);
        }

        height
    }

    /// Get layer height at position
    pub fn get_layer_height(
        &self,
        x: f32,
        y: f32,
        layer: PathfindLayerEnum,
        normal: Option<&mut Coord3D>,
        clip: bool,
    ) -> f32 {
        let pos = Coord3D::new(x, y, 0.0);

        // Check if position is on a bridge for this layer
        if layer != PathfindLayerEnum::Ground {
            if let Some(bridge) = self.find_bridge_layer_at(&pos, layer, clip) {
                return bridge.get_bridge_height(&pos, normal);
            }
        }

        // Fall back to ground height
        self.get_ground_height(x, y, normal)
    }

    /// Find closest edge point
    pub fn find_closest_edge_point(&self, closest_to: &Coord3D) -> Coord3D {
        let extent = self.get_maximum_pathfind_extent();
        let distances = [
            (closest_to.y - extent.lo.y).abs(), // top
            (closest_to.x - extent.hi.x).abs(), // right
            (closest_to.y - extent.hi.y).abs(), // bottom
            (closest_to.x - extent.lo.x).abs(), // left
        ];
        let mut best_index = 0usize;
        let mut best_distance = distances[0];
        for (idx, distance) in distances.iter().copied().enumerate().skip(1) {
            if distance < best_distance {
                best_distance = distance;
                best_index = idx;
            }
        }

        let mut ret = *closest_to;
        match best_index {
            0 => ret.y = extent.lo.y,
            1 => ret.x = extent.hi.x,
            2 => ret.y = extent.hi.y,
            _ => ret.x = extent.lo.x,
        }
        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Determine the highest pathfinding layer that should be used for a destination position.
    ///
    /// Mirrors the C++ intent: pick the highest layer at/below the position.
    pub fn get_highest_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        self.get_highest_layer_for_destination_with_health(pos, false)
    }

    fn get_wall_height(&self) -> Real {
        THE_AI
            .read()
            .ok()
            .and_then(|ai| ai.get_ai_data().read().ok().map(|data| data.wall_height))
            .unwrap_or(0.0)
    }

    fn is_point_on_wall(&self, pos: &Coord3D) -> bool {
        if let Ok(ai_guard) = THE_AI.read() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(pathfinder_guard) = pathfinder.read() {
                    return pathfinder_guard.is_point_on_wall(pos);
                }
            }
        }
        self.is_point_on_wall_fallback(pos)
    }

    fn is_point_on_wall_fallback(&self, pos: &Coord3D) -> bool {
        let cell_pad = PATHFIND_CELL_SIZE_F * 0.5;
        for obj in OBJECT_REGISTRY.get_all_objects() {
            if let Ok(obj_guard) = obj.read() {
                if !obj_guard.is_any_kind_of(&[KindOf::Barrier]) {
                    continue;
                }
                let wall_pos = obj_guard.get_position();
                let geom = obj_guard.get_template().get_template_geometry_info();
                let radius = geom.get_bounding_circle_radius();
                let dx = wall_pos.x - pos.x;
                let dy = wall_pos.y - pos.y;
                let dist_sq = dx * dx + dy * dy;
                let allowed = radius + cell_pad;
                if dist_sq <= allowed * allowed {
                    return true;
                }
            }
        }
        false
    }

    /// Variant that can optionally ignore broken bridges.
    pub fn get_highest_layer_for_destination_with_health(
        &self,
        pos: &Coord3D,
        only_healthy_bridges: bool,
    ) -> PathfindLayerEnum {
        let ground_z = self.get_ground_height(pos.x, pos.y, None);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = pos.z - ground_z;

        let wall_height = self.get_wall_height();
        if best_distance > wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = pos.z - wall_height;
            if delta >= 0.0 && delta.abs() < best_distance.abs() {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if only_healthy_bridges && info.cur_damage_state == BodyDamageType::Rubble {
                current = bridge.next.as_deref();
                continue;
            }
            if bridge.is_point_on_bridge(pos) {
                let bridge_z = bridge.get_bridge_height(pos, None);
                let delta = pos.z - bridge_z;
                if delta >= 0.0 && delta.abs() < best_distance.abs() {
                    best_layer = bridge.get_layer();
                    best_distance = delta;
                }
            }
            current = bridge.next.as_deref();
        }

        best_layer
    }

    /// Find farthest edge point
    /// Determine the layer for a destination position (C++ getLayerForDestination).
    pub fn get_layer_for_destination(&self, pos: &Coord3D) -> PathfindLayerEnum {
        let ground_z = self.get_ground_height(pos.x, pos.y, None);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = (pos.z - ground_z).abs();

        let wall_height = self.get_wall_height();
        if best_distance > wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = (pos.z - wall_height).abs();
            if delta < best_distance {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.is_point_on_bridge(pos) {
                let bridge_z = bridge.get_bridge_height(pos, None);
                let delta = (pos.z - bridge_z).abs();
                if delta < best_distance {
                    best_layer = bridge.get_layer();
                    best_distance = delta;
                }
            }
            current = bridge.next.as_deref();
        }

        best_layer
    }

    pub fn find_farthest_edge_point(&self, farthest_from: &Coord3D) -> Coord3D {
        let extent = self.get_maximum_pathfind_extent();
        let width = extent.hi.x - extent.lo.x;
        let height = extent.hi.y - extent.lo.y;

        let mut ret = *farthest_from;
        if farthest_from.x < width * 0.5 {
            ret.x = extent.hi.x;
        } else {
            ret.x = extent.lo.x;
        }

        if farthest_from.y < height * 0.5 {
            ret.y = extent.hi.y;
        } else {
            ret.y = extent.lo.y;
        }

        ret.z = self.get_ground_height(ret.x, ret.y, None);
        ret
    }

    /// Check clear line of sight
    pub fn is_clear_line_of_sight(&self, pos1: &Coord3D, pos2: &Coord3D) -> bool {
        // Terrain-only line of sight check.
        //
        // C++ reference: `PartitionManager::isClearLineOfSightTerrain`.
        //
        // This intentionally ignores dynamic objects (bridges/structures) until the unified
        // partition/occlusion system is ported; it only considers terrain elevation.

        let delta = *pos2 - *pos1;
        let distance_xy = (delta.x * delta.x + delta.y * delta.y).sqrt();
        if distance_xy <= 0.001 {
            return true;
        }

        // Sample at a conservative step. Too coarse gives false positives; too fine costs perf.
        // Special power targeting is not called per-object per-frame, so this is acceptable.
        let step_len = 10.0_f32;
        let steps = (distance_xy / step_len).ceil().clamp(2.0, 512.0) as u32;

        // Allow some slack so units on small bumps don't block LOS.
        let clearance = 5.0_f32;

        for i in 1..steps {
            let t = i as f32 / steps as f32;
            let x = pos1.x + delta.x * t;
            let y = pos1.y + delta.y * t;
            let expected_z = pos1.z + delta.z * t;
            let ground_z = self.get_ground_height(x, y, None);
            if ground_z > expected_z + clearance {
                return false;
            }
        }

        true
    }

    /// Get source filename
    pub fn get_source_filename(&self) -> &AsciiString {
        &self.filename_string
    }

    /// Check if point is underwater
    pub fn is_underwater(
        &self,
        x: f32,
        y: f32,
        water_z: Option<&mut f32>,
        terrain_z: Option<&mut f32>,
    ) -> bool {
        let terrain_height = self.get_ground_height(x, y, None);

        if let Some(tz) = terrain_z {
            *tz = terrain_height;
        }

        let Some(water_handle) = self.get_water_handle(x, y) else {
            return false;
        };

        let is_grid = std::ptr::eq(water_handle, &self.grid_water_handle);
        let w_z = if is_grid {
            self.grid_water_handle.get_current_height()
        } else {
            self.get_water_height(water_handle)
        };
        if let Some(wz) = water_z {
            *wz = w_z;
        }

        terrain_height < w_z
    }

    /// Check if cell is cliff
    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        if self.map_dx <= 0 || self.map_dy <= 0 || self.map_data.is_empty() {
            return false;
        }

        let map_x = (x / MAP_XY_FACTOR) as i32;
        let map_y = (y / MAP_XY_FACTOR) as i32;

        if map_x < 0 || map_x >= self.map_dx || map_y < 0 || map_y >= self.map_dy {
            return false;
        }

        let idx = (map_y * self.map_dx + map_x) as usize;
        if idx >= self.map_data.len() {
            return false;
        }

        let height = self.map_data[idx] as f32 * MAP_HEIGHT_SCALE;
        let cliff_threshold = MAP_HEIGHT_SCALE * 8.0;

        let neighbors = [
            (map_x - 1, map_y),
            (map_x + 1, map_y),
            (map_x, map_y - 1),
            (map_x, map_y + 1),
        ];

        for (nx, ny) in neighbors {
            if nx < 0 || nx >= self.map_dx || ny < 0 || ny >= self.map_dy {
                continue;
            }
            let nidx = (ny * self.map_dx + nx) as usize;
            if nidx >= self.map_data.len() {
                continue;
            }
            let nheight = self.map_data[nidx] as f32 * MAP_HEIGHT_SCALE;
            if (height - nheight).abs() >= cliff_threshold {
                return true;
            }
        }

        false
    }

    /// Get water handle at location
    pub fn get_water_handle(&self, x: f32, y: f32) -> Option<&WaterHandle> {
        let query = ICoord3D::new((x + 0.5).floor() as Int, (y + 0.5).floor() as Int, 0);

        let mut best_trigger_id: Option<Int> = None;
        let mut best_water_z = 0.0f32;

        for trigger in self.trigger_areas.get_triggers() {
            if !trigger.is_water_area() || !trigger.point_in_trigger_int(&query) {
                continue;
            }

            let Some(point0) = trigger.get_point(0) else {
                continue;
            };
            let trigger_water_z = point0.z as f32;
            if trigger_water_z >= best_water_z {
                best_water_z = trigger_water_z;
                best_trigger_id = Some(trigger.get_id());
            }
        }

        // C++ parity subset: optional grid-water override when enabled.
        if self.water_grid_enabled {
            let bounds = self.grid_water_handle.get_bounds();
            if x >= bounds.lo.x && x <= bounds.hi.x && y >= bounds.lo.y && y <= bounds.hi.y {
                let grid_z = self.grid_water_handle.get_current_height();
                if grid_z >= best_water_z {
                    return Some(&self.grid_water_handle);
                }
            }
        }

        if let Some(trigger_id) = best_trigger_id {
            return self.water_handles_by_trigger_id.get(&trigger_id);
        }
        None
    }

    /// Get water handle by name
    pub fn get_water_handle_by_name(&self, name: &AsciiString) -> Option<&WaterHandle> {
        if Self::is_grid_water_name(name) {
            return Some(&self.grid_water_handle);
        }

        let trigger_id = self.resolve_water_trigger_id(name);
        if trigger_id >= 0 {
            return self.water_handles_by_trigger_id.get(&trigger_id);
        }

        None
    }

    /// Get water height
    pub fn get_water_height(&self, water: &WaterHandle) -> f32 {
        water.get_current_height()
    }

    fn is_grid_water_name(name: &AsciiString) -> bool {
        name.as_str().eq_ignore_ascii_case(WATER_GRID_NAME_CPP)
            || name.as_str().eq_ignore_ascii_case(WATER_GRID_NAME_LEGACY)
    }

    fn water_bounds_from_trigger(trigger: &PolygonTrigger, height: f32) -> Region3D {
        let bounds = trigger.get_bounds();
        Region3D::new(
            Coord3D::new(bounds.lo.x as f32, bounds.lo.y as f32, height),
            Coord3D::new(bounds.hi.x as f32, bounds.hi.y as f32, height),
        )
    }

    fn resolve_water_height_for_entry(
        &self,
        trigger_id: Int,
        water_name: &AsciiString,
    ) -> Option<f32> {
        if Self::is_grid_water_name(water_name) {
            return Some(self.grid_water_handle.get_current_height());
        }

        if trigger_id >= 0 {
            if let Some(handle) = self.water_handles_by_trigger_id.get(&trigger_id) {
                return Some(handle.get_current_height());
            }
            if let Some(trigger) = self.trigger_areas.get_by_id(trigger_id) {
                if trigger.is_water_area() {
                    if let Some(point) = trigger.get_point(0) {
                        return Some(point.z as f32);
                    }
                }
            }
        }

        None
    }

    fn update_polygon_water_height_by_id(
        &mut self,
        trigger_id: Int,
        height: f32,
    ) -> Option<(AsciiString, Region3D)> {
        let trigger = self.trigger_areas.get_by_id_mut(trigger_id)?;
        if !trigger.is_water_area() {
            return None;
        }

        let point_count = trigger.get_num_points();
        for idx in 0..point_count {
            if let Some(mut point) = trigger.get_point(idx).cloned() {
                point.z = height as Int;
                trigger.set_point(point, idx);
            }
        }

        let trigger_name = trigger.get_trigger_name().clone();
        let bounds = Self::water_bounds_from_trigger(trigger, height);
        Some((trigger_name, bounds))
    }

    fn update_polygon_water_height_by_name(
        &mut self,
        water_name: &AsciiString,
        height: f32,
    ) -> Option<(Int, AsciiString, Region3D)> {
        let trigger = self.trigger_areas.get_by_name_mut(water_name.as_str())?;
        if !trigger.is_water_area() {
            return None;
        }
        let trigger_id = trigger.get_id();

        let point_count = trigger.get_num_points();
        for idx in 0..point_count {
            if let Some(mut point) = trigger.get_point(idx).cloned() {
                point.z = height as Int;
                trigger.set_point(point, idx);
            }
        }

        let trigger_name = trigger.get_trigger_name().clone();
        let bounds = Self::water_bounds_from_trigger(trigger, height);
        Some((trigger_id, trigger_name, bounds))
    }

    fn sync_water_handle_for_trigger(
        &mut self,
        trigger_id: Int,
        trigger_name: &AsciiString,
        height: f32,
        bounds: Region3D,
    ) {
        if let Some(handle) = self.water_handles_by_trigger_id.get_mut(&trigger_id) {
            handle.name = trigger_name.clone();
            handle.set_height(height);
            handle.bounds = bounds;
        } else {
            self.water_handles_by_trigger_id.insert(
                trigger_id,
                WaterHandle::new(trigger_name.clone(), height, bounds),
            );
        }

        // Keep the name-keyed cache aligned with C++ first-match lookup behavior.
        if self.resolve_water_trigger_id(trigger_name) == trigger_id {
            if let Some(name_handle) = self.water_handles.get_mut(trigger_name) {
                name_handle.set_height(height);
                name_handle.bounds = bounds;
            } else if let Some(trigger_handle) =
                self.water_handles_by_trigger_id.get(&trigger_id).cloned()
            {
                self.water_handles
                    .insert(trigger_name.clone(), trigger_handle);
            }
        }
    }

    fn sync_named_water_handle(
        &mut self,
        water_name: &AsciiString,
        height: f32,
        bounds: Option<Region3D>,
    ) {
        if let Some(handle) = self.water_handles.get_mut(water_name) {
            handle.set_height(height);
            if let Some(region) = bounds {
                handle.bounds = region;
            }
        } else if let Some(region) = bounds {
            self.water_handles.insert(
                water_name.clone(),
                WaterHandle::new(water_name.clone(), height, region),
            );
        }
    }

    fn apply_water_rise_damage(&self, affected_region: &Region3D, damage_amount: f32) {
        if damage_amount <= 0.0 {
            return;
        }

        let center = Coord3D::new(
            (affected_region.lo.x + affected_region.hi.x) * 0.5,
            (affected_region.lo.y + affected_region.hi.y) * 0.5,
            0.0,
        );
        let width = affected_region.hi.x - affected_region.lo.x;
        let height = affected_region.hi.y - affected_region.lo.y;
        let max_dist = (width * width + height * height).sqrt();

        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return;
        };

        for object_id in partition.get_objects_in_range(&center, max_dist) {
            let Some(obj_arc) = OBJECT_REGISTRY.get_object(object_id) else {
                continue;
            };
            let Ok(mut obj_guard) = obj_arc.write() else {
                continue;
            };
            let pos = *obj_guard.get_position();
            if self.is_underwater(pos.x, pos.y, None, None) {
                let mut damage = DamageInfo::with_simple(
                    damage_amount,
                    crate::common::INVALID_ID,
                    DamageType::Water,
                    DeathType::Normal,
                );
                let _ = obj_guard.attempt_damage(&mut damage);
            }
        }
    }

    fn request_pathfind_recalculation(&self) {
        let pathfinder = if let Ok(ai_guard) = THE_AI.read() {
            ai_guard.pathfinder()
        } else {
            None
        };
        let Some(pathfinder) = pathfinder else {
            return;
        };
        let mut pathfinder_guard = match pathfinder.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        pathfinder_guard.rebuild_from_terrain(self);
    }

    fn set_water_height_internal(
        &mut self,
        trigger_id: Int,
        water_name: &AsciiString,
        height: f32,
        damage_amount: f32,
        force_pathfind_update: bool,
    ) {
        if Self::is_grid_water_name(water_name) {
            let previous_height = self.grid_water_handle.get_current_height();
            self.grid_water_handle.set_height(height);

            if damage_amount > 0.0 && height > previous_height {
                let affected = self.grid_water_handle.get_bounds();
                self.apply_water_rise_damage(affected, damage_amount);
            }
            if force_pathfind_update || (previous_height - height).abs() > f32::EPSILON {
                self.request_pathfind_recalculation();
            }
            return;
        }

        let previous_height = self
            .resolve_water_height_for_entry(trigger_id, water_name)
            .unwrap_or(height);

        let mut resolved_name = water_name.clone();
        let mut resolved_trigger_id = trigger_id;
        let mut affected_region = None;
        if trigger_id >= 0 {
            if let Some((name, bounds)) = self.update_polygon_water_height_by_id(trigger_id, height)
            {
                resolved_name = name;
                resolved_trigger_id = trigger_id;
                affected_region = Some(bounds);
            }
        } else if let Some((id, name, bounds)) =
            self.update_polygon_water_height_by_name(water_name, height)
        {
            resolved_trigger_id = id;
            resolved_name = name;
            affected_region = Some(bounds);
        }

        if let Some(bounds) = affected_region {
            if resolved_trigger_id >= 0 {
                self.sync_water_handle_for_trigger(
                    resolved_trigger_id,
                    &resolved_name,
                    height,
                    bounds,
                );
            } else {
                self.sync_named_water_handle(&resolved_name, height, Some(bounds));
            }
        } else {
            if trigger_id >= 0 {
                log::warn!(
                    "TerrainLogic::set_water_height_internal missing water trigger id {}",
                    trigger_id
                );
            }
            self.sync_named_water_handle(&resolved_name, height, None);
        }

        if damage_amount > 0.0 && height > previous_height {
            if let Some(region) = affected_region {
                self.apply_water_rise_damage(&region, damage_amount);
            }
        }

        if force_pathfind_update || (previous_height - height).abs() > f32::EPSILON {
            self.request_pathfind_recalculation();
        }
    }

    fn resolve_named_water_handle_identity(
        &self,
        water_name: &AsciiString,
    ) -> Option<(Int, &WaterHandle)> {
        if Self::is_grid_water_name(water_name) {
            return Some((-1, &self.grid_water_handle));
        }
        let trigger_id = self.resolve_water_trigger_id(water_name);
        if trigger_id >= 0 {
            return self
                .water_handles_by_trigger_id
                .get(&trigger_id)
                .map(|handle| (trigger_id, handle));
        }
        None
    }

    /// Set water height
    pub fn set_water_height(
        &mut self,
        water_name: &AsciiString,
        height: f32,
        damage_amount: f32,
        force_pathfind_update: bool,
    ) {
        self.set_water_height_internal(
            self.resolve_water_trigger_id(water_name),
            water_name,
            height,
            damage_amount,
            force_pathfind_update,
        );
    }

    /// Change water height over time
    pub fn change_water_height_over_time(
        &mut self,
        water_name: &AsciiString,
        final_height: f32,
        transition_time_seconds: f32,
        damage_amount: f32,
    ) {
        let Some((trigger_id, water_handle)) = self.resolve_named_water_handle_identity(water_name)
        else {
            return;
        };
        let resolved_name = water_handle.get_name().clone();
        let current_height = water_handle.get_current_height();

        // C++ parity: remove existing transition for this water handle before adding a new one.
        self.water_to_update.retain(|entry| {
            if trigger_id >= 0 && entry.trigger_id >= 0 {
                entry.trigger_id != trigger_id
            } else {
                !entry
                    .water_name
                    .as_str()
                    .eq_ignore_ascii_case(resolved_name.as_str())
            }
        });

        // C++ parity: fixed-size dynamic water transition list.
        if self.water_to_update.len() >= MAX_DYNAMIC_WATER_ENTRIES {
            log::warn!(
                "TerrainLogic dynamic water transition limit ({}) reached",
                MAX_DYNAMIC_WATER_ENTRIES
            );
            return;
        }

        let frames_to_complete = (transition_time_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        if frames_to_complete <= 0 {
            return;
        }

        let change_per_frame = (final_height - current_height) / frames_to_complete as f32;
        self.water_to_update.push(DynamicWaterEntry {
            trigger_id,
            water_name: resolved_name,
            change_per_frame,
            target_height: final_height,
            damage_amount,
            current_height,
        });
    }

    fn resolve_water_trigger_id(&self, water_name: &AsciiString) -> Int {
        for trigger in self.trigger_areas.get_triggers() {
            if trigger.is_water_area() && trigger.get_trigger_name() == water_name {
                return trigger.get_id();
            }
        }
        -1
    }

    pub fn snapshot_dynamic_water_entries(&self) -> Vec<TerrainDynamicWaterSnapshotEntry> {
        let mut entries = Vec::with_capacity(self.water_to_update.len());
        for entry in &self.water_to_update {
            entries.push(TerrainDynamicWaterSnapshotEntry {
                trigger_id: if entry.trigger_id >= 0 {
                    entry.trigger_id
                } else {
                    self.resolve_water_trigger_id(&entry.water_name)
                },
                water_name: entry.water_name.clone(),
                change_per_frame: entry.change_per_frame,
                target_height: entry.target_height,
                damage_amount: entry.damage_amount,
                current_height: entry.current_height,
            });
        }
        entries
    }

    pub fn restore_dynamic_water_entries(
        &mut self,
        entries: Vec<TerrainDynamicWaterSnapshotEntry>,
    ) -> Result<(), String> {
        self.water_to_update.clear();
        for mut entry in entries {
            if self.water_to_update.len() >= MAX_DYNAMIC_WATER_ENTRIES {
                return Err(format!(
                    "TerrainLogic::restore_dynamic_water_entries exceeds max dynamic entries ({})",
                    MAX_DYNAMIC_WATER_ENTRIES
                ));
            }
            if entry.trigger_id >= 0 {
                let trigger = self
                    .trigger_areas
                    .get_by_id(entry.trigger_id)
                    .ok_or_else(|| {
                        format!(
                            "TerrainLogic::restore_dynamic_water_entries missing trigger id '{}'",
                            entry.trigger_id
                        )
                    })?;
                if trigger.get_water_handle().is_none() {
                    return Err(format!(
                        "TerrainLogic::restore_dynamic_water_entries trigger '{}' has no water handle",
                        entry.trigger_id
                    ));
                }
                if !self
                    .water_handles_by_trigger_id
                    .contains_key(&entry.trigger_id)
                {
                    return Err(format!(
                        "TerrainLogic::restore_dynamic_water_entries missing water handle for trigger id '{}'",
                        entry.trigger_id
                    ));
                }
                entry.water_name = trigger.get_trigger_name().clone();
            }

            if entry.water_name.is_empty() {
                return Err(
                    "TerrainLogic::restore_dynamic_water_entries missing water handle name"
                        .to_string(),
                );
            }

            let is_grid_name = entry
                .water_name
                .as_str()
                .eq_ignore_ascii_case(WATER_GRID_NAME_CPP)
                || entry
                    .water_name
                    .as_str()
                    .eq_ignore_ascii_case(WATER_GRID_NAME_LEGACY)
                || entry.water_name == *self.grid_water_handle.get_name();

            if !is_grid_name
                && entry.trigger_id < 0
                && self.get_water_handle_by_name(&entry.water_name).is_none()
            {
                return Err(format!(
                    "TerrainLogic::restore_dynamic_water_entries missing water handle '{}'",
                    entry.water_name
                ));
            }

            self.water_to_update.push(DynamicWaterEntry {
                trigger_id: entry.trigger_id,
                water_name: entry.water_name,
                change_per_frame: entry.change_per_frame,
                target_height: entry.target_height,
                damage_amount: entry.damage_amount,
                current_height: entry.current_height,
            });
        }
        Ok(())
    }

    /// Get first waypoint
    pub fn get_first_waypoint(&self) -> Option<&Waypoint> {
        self.waypoint_list_head.as_ref().map(|w| w.as_ref())
    }

    /// Get waypoint by name
    pub fn get_waypoint_by_name(&self, name: &AsciiString) -> Option<&Waypoint> {
        let mut current = self.waypoint_list_head.as_deref();
        while let Some(waypoint) = current {
            if waypoint.get_name() == name {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref();
        }
        None
    }

    /// Get waypoint by ID
    pub fn get_waypoint_by_id(&self, id: WaypointID) -> Option<&Waypoint> {
        let mut current = self.waypoint_list_head.as_deref();
        while let Some(waypoint) = current {
            if waypoint.get_id() == id {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref();
        }
        None
    }

    /// Get closest waypoint that matches a path label
    pub fn get_closest_waypoint_on_path(&self, pos: &Coord3D, label: &str) -> Option<&Waypoint> {
        let mut current = self.waypoint_list_head.as_deref();
        let mut best: Option<&Waypoint> = None;
        let mut best_dist_sqr = f32::MAX;

        while let Some(waypoint) = current {
            if waypoint.matches_path_label(label) {
                let dx = waypoint.location.x - pos.x;
                let dy = waypoint.location.y - pos.y;
                let dist_sqr = dx * dx + dy * dy;
                if dist_sqr < best_dist_sqr {
                    best_dist_sqr = dist_sqr;
                    best = Some(waypoint);
                }
            }
            current = waypoint.next.as_deref();
        }

        best
    }

    fn get_waypoint_by_id_mut(&mut self, id: WaypointID) -> Option<&mut Waypoint> {
        let mut current = self.waypoint_list_head.as_deref_mut();
        while let Some(waypoint) = current {
            if waypoint.get_id() == id {
                return Some(waypoint);
            }
            current = waypoint.next.as_deref_mut();
        }
        None
    }

    fn add_waypoint_from_map(&mut self, waypoint: &MapWaypoint) {
        let mut location = Coord3D::new(
            waypoint.location.x,
            waypoint.location.y,
            waypoint.location.z,
        );
        location.z = self.get_ground_height(location.x, location.y, None);
        let new_waypoint = Waypoint::new(
            waypoint.id,
            AsciiString::from(waypoint.name.as_str()),
            &location,
            AsciiString::from(waypoint.path_label1.as_str()),
            AsciiString::from(waypoint.path_label2.as_str()),
            AsciiString::from(waypoint.path_label3.as_str()),
            waypoint.bi_directional,
        );

        let mut boxed = Box::new(new_waypoint);
        boxed.next = self.waypoint_list_head.take();
        self.waypoint_list_head = Some(boxed);
    }

    fn add_waypoint_link(&mut self, id1: WaypointID, id2: WaypointID) {
        if id1 == id2 {
            return;
        }

        let should_link_back = {
            let Some(way1) = self.get_waypoint_by_id_mut(id1) else {
                return;
            };
            if !way1.has_link(id2) {
                way1.add_link(id2);
            }
            way1.get_bi_directional()
        };

        if should_link_back {
            if let Some(way2) = self.get_waypoint_by_id_mut(id2) {
                if !way2.has_link(id1) {
                    way2.add_link(id1);
                }
            }
        }
    }

    // ============================================================================
    // TRIGGER AREA METHODS
    // Matches C++ ThePolygonTriggerListPtr interface
    // ============================================================================

    /// Get trigger area by name
    /// Matches C++ ThePolygonTriggerListPtr->getPolygonTriggerByName
    pub fn get_trigger_area_by_name(&self, name: &str) -> Option<&PolygonTrigger> {
        self.trigger_areas.get_by_name(name)
    }

    /// Get mutable trigger area by name
    pub fn get_trigger_area_by_name_mut(&mut self, name: &str) -> Option<&mut PolygonTrigger> {
        self.trigger_areas.get_by_name_mut(name)
    }

    /// Get all trigger areas
    pub fn get_trigger_areas(&self) -> &PolygonTriggerList {
        &self.trigger_areas
    }

    /// Get mutable trigger areas list
    pub fn get_trigger_areas_mut(&mut self) -> &mut PolygonTriggerList {
        &mut self.trigger_areas
    }

    /// Add a trigger area
    pub fn add_trigger_area(&mut self, trigger: PolygonTrigger) {
        let trigger_name_ascii = trigger.get_trigger_name().clone();
        let trigger_name = trigger_name_ascii.to_string();

        if trigger.is_water_area() {
            let trigger_id = trigger.get_id();
            let water_height = trigger
                .get_point(0)
                .map(|point| point.z as f32)
                .unwrap_or(self.grid_water_handle.get_current_height());
            let bounds = trigger.get_bounds();
            let water_bounds = Region3D::new(
                Coord3D::new(bounds.lo.x as f32, bounds.lo.y as f32, water_height),
                Coord3D::new(bounds.hi.x as f32, bounds.hi.y as f32, water_height),
            );
            let handle = WaterHandle::new(trigger_name_ascii.clone(), water_height, water_bounds);
            self.water_handles_by_trigger_id
                .insert(trigger_id, handle.clone());
            self.water_handles
                .entry(trigger_name_ascii.clone())
                .or_insert(handle);
        }

        self.trigger_areas.add(trigger);

        let area_tracker = crate::scripting::engine::get_area_tracker();
        if let Err(err) = area_tracker.register_polygon_area(&trigger_name) {
            log::warn!(
                "Failed to register polygon trigger area '{}' with script tracker: {}",
                trigger_name,
                err
            );
        }
    }

    /// Get first bridge
    pub fn get_first_bridge(&self) -> Option<&Bridge> {
        self.bridge_list_head.as_ref().map(|b| b.as_ref())
    }

    /// Find bridge at location
    pub fn find_bridge_at(&self, location: &Coord3D) -> Option<&Bridge> {
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.is_point_on_bridge(location) {
                return Some(bridge);
            }
            current = bridge.next.as_deref();
        }
        None
    }

    /// Find bridge at location (mutable)
    pub fn find_bridge_at_mut(&mut self, location: &Coord3D) -> Option<&mut Bridge> {
        let mut current = self.bridge_list_head.as_deref_mut();
        while let Some(bridge) = current {
            if bridge.is_point_on_bridge(location) {
                return Some(bridge);
            }
            current = bridge.next.as_deref_mut();
        }
        None
    }

    /// Delete the first bridge that contains the given location.
    pub fn delete_bridge_at(&mut self, location: &Coord3D) -> bool {
        let Some(bridge) = self.find_bridge_at(location) else {
            return false;
        };

        let bridge_object_id = bridge.get_bridge_info().bridge_object_id;
        let bridge_layer = bridge.get_layer();

        if let Some(ai_guard) = THE_AI.read().ok() {
            if let Some(pathfinder) = ai_guard.pathfinder() {
                if let Ok(mut pathfinder_guard) = pathfinder.write() {
                    pathfinder_guard.change_bridge_state(bridge_layer, false);
                }
            }
        }

        if bridge_object_id != crate::common::INVALID_ID {
            let _ = crate::helpers::TheGameLogic::destroy_object_by_id(bridge_object_id);
        }

        self.remove_bridge_at(location)
    }

    /// Find bridge at layer
    pub fn find_bridge_layer_at(
        &self,
        location: &Coord3D,
        layer: PathfindLayerEnum,
        clip: bool,
    ) -> Option<&Bridge> {
        if layer == PathfindLayerEnum::Ground {
            return None;
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer && (!clip || bridge.is_point_on_bridge(location)) {
                return Some(bridge);
            }
            current = bridge.next.as_deref();
        }
        None
    }

    /// Determines whether the object interacts with the bridge on specified layer.
    pub fn object_interacts_with_bridge_layer(
        &self,
        obj: &Object,
        layer: PathfindLayerEnum,
        consider_bridge_health: bool,
    ) -> bool {
        if layer == PathfindLayerEnum::Ground {
            return false;
        }
        if layer == PathfindLayerEnum::Wall {
            if matches!(obj.get_layer(), crate::common::PathfindLayerEnum::Wall) {
                return true;
            }
            return self.is_point_on_wall(obj.get_position());
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer {
                let mut matches = false;
                if bridge.is_point_on_bridge(obj.get_position()) {
                    matches = true;
                }

                let mut radius = obj.get_geometry_info().get_minor_radius();
                radius += PATHFIND_CELL_SIZE_F * 0.5;
                let mut bounds = Region2D::default();
                bounds.lo.x = obj.get_position().x - radius;
                bounds.lo.y = obj.get_position().y - radius;
                bounds.hi.x = obj.get_position().x + radius;
                bounds.hi.y = obj.get_position().y + radius;

                if bridge.is_cell_on_end(&bounds) {
                    matches = true;
                }

                if matches {
                    let bridge_height = bridge.get_bridge_height(obj.get_position(), None);
                    let delta = (obj.get_position().z - bridge_height).abs();
                    if delta > LAYER_Z_CLOSE_ENOUGH_F {
                        return false;
                    }
                    if consider_bridge_health
                        && bridge.get_bridge_info().cur_damage_state == BodyDamageType::Rubble
                    {
                        return false;
                    }
                    return true;
                }
                return false;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Determines whether the object interacts with the bridge end on specified layer.
    pub fn object_interacts_with_bridge_end(&self, obj: &Object, layer: PathfindLayerEnum) -> bool {
        if layer == PathfindLayerEnum::Ground {
            return false;
        }

        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer {
                let mut radius = obj.get_geometry_info().get_minor_radius();
                radius += PATHFIND_CELL_SIZE_F * 0.5;
                let mut bounds = Region2D::default();
                bounds.lo.x = obj.get_position().x - radius;
                bounds.lo.y = obj.get_position().y - radius;
                bounds.hi.x = obj.get_position().x + radius;
                bounds.hi.y = obj.get_position().y + radius;

                if bridge.is_cell_on_end(&bounds) {
                    let bridge_height = bridge.get_bridge_height(obj.get_position(), None);
                    let delta = (obj.get_position().z - bridge_height).abs();
                    if delta > LAYER_Z_CLOSE_ENOUGH_F {
                        return false;
                    }
                    return true;
                }
                return false;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Add bridge to logic
    pub fn add_bridge_to_logic(&mut self, bridge_info: BridgeInfo, template_name: AsciiString) {
        let mut new_bridge = Box::new(Bridge::new(bridge_info, template_name));
        let layer = Self::register_bridge_with_pathfinder(new_bridge.get_bridge_info())
            .unwrap_or(PathfindLayerEnum::Bridge1);
        new_bridge.set_layer(layer);
        new_bridge.next = self.bridge_list_head.take();
        self.bridge_list_head = Some(new_bridge);
    }

    /// Add a landmark bridge to logic from an existing object.
    /// Reference: C++ TerrainLogic::addLandmarkBridgeToLogic()
    ///
    /// Landmark bridges are placed as objects in the map and need to be
    /// registered with the terrain system for pathfinding and height queries.
    pub fn add_landmark_bridge_to_logic(&mut self, bridge_obj: &Object) {
        let bridge_info = Self::bridge_info_from_object(bridge_obj);
        let template_name = bridge_obj.get_template().get_name().clone();
        self.add_bridge_to_logic(bridge_info, template_name);
    }

    /// Delete a specific bridge from the terrain system.
    /// Reference: C++ TerrainLogic::deleteBridge()
    ///
    /// Removes the bridge from the list and destroys its associated object.
    pub fn delete_bridge(&mut self, location: &Coord3D) -> bool {
        self.delete_bridge_at(location)
    }

    /// Enable/disable water grid
    pub fn enable_water_grid(&mut self, enable: bool) {
        self.water_grid_enabled = enable;
        if !enable {
            return;
        }

        // C++ parity: enabling water grid also validates map-specific vertex-water
        // settings against GlobalData::vertexWaterAvailableMaps (with stripped-name
        // fallback for save/load map paths). The visual-side configuration calls are
        // not fully ported yet, but we keep parity checks and diagnostics here.
        let Some(global) = game_engine::common::ini::get_global_data() else {
            return;
        };
        let global = global.read();
        let map_name = global.map_name.trim();
        if map_name.is_empty() {
            return;
        }

        let map_leaf = map_name.rsplit(['\\', '/']).next().unwrap_or(map_name);
        let mut matched = false;
        for configured in &global.vertex_water_available_maps {
            let configured = configured.trim();
            if configured.is_empty() {
                continue;
            }
            if configured.eq_ignore_ascii_case(map_name) {
                matched = true;
                break;
            }
            let configured_leaf = configured.rsplit(['\\', '/']).next().unwrap_or(configured);
            if configured_leaf.eq_ignore_ascii_case(map_leaf) {
                matched = true;
                break;
            }
        }

        if !matched {
            log::error!(
                "Water grid enabled for map '{}' but no matching vertex-water setting exists in GlobalData::vertex_water_available_maps",
                map_name
            );
        }
    }

    /// Get active boundary
    pub fn get_active_boundary(&self) -> i32 {
        self.active_boundary
    }

    /// Set active boundary
    pub fn set_active_boundary(&mut self, new_active_boundary: i32) {
        self.active_boundary = new_active_boundary;
    }

<<<<<<< Updated upstream
    /// Flatten terrain under a building/object.
    /// Reference: C++ TerrainLogic::flattenTerrain() in TerrainLogic.cpp
    ///
    /// Computes the average height under the object's footprint, then lowers
    /// all terrain cells within the footprint to that average. Only lowers,
    /// never raises — matching C++ setRawMapHeight behavior.
    pub fn flatten_terrain(&mut self, obj: &Arc<RwLock<Object>>) {
        let obj_guard = obj.read().unwrap();
=======
    /// Flatten terrain under object. Reference: C++ TerrainLogic::flattenTerrain()
    pub fn flatten_terrain(&mut self, obj: &Arc<RwLock<Object>>) {
        let obj_guard = match obj.read() {
            Ok(g) => g,
            Err(_) => return,
        };

>>>>>>> Stashed changes
        if obj_guard.get_geometry_info().get_is_small() {
            return;
        }

<<<<<<< Updated upstream
        let pos = obj_guard.get_position();
        let geom = obj_guard.get_geometry_info();

        match geom.get_geometry_type() {
            EngineGeometryType::Box => {
                let angle = obj_guard.get_orientation();
                let halfsize_x = geom.get_major_radius();
                let halfsize_y = geom.get_minor_radius();
=======
        let pos = *obj_guard.get_position();
        let geometry_type = obj_guard.get_geometry_info().get_geometry_type();
        let major_radius = obj_guard.get_geometry_info().get_major_radius();
        let minor_radius = obj_guard.get_geometry_info().get_minor_radius();
        let angle = obj_guard.get_orientation();
        drop(obj_guard);

        if self.map_data.is_empty() || self.map_dx <= 0 || self.map_dy <= 0 {
            return;
        }

        match geometry_type {
            EngineGeometryType::Box => {
                let halfsize_x = major_radius;
                let halfsize_y = minor_radius;
>>>>>>> Stashed changes
                let c = angle.cos();
                let s = angle.sin();

                let top_left_x = pos.x - halfsize_x * c - halfsize_y * s;
                let top_left_y = pos.y + halfsize_y * c - halfsize_x * s;
                let top_right_x = pos.x + halfsize_x * c - halfsize_y * s;
                let top_right_y = pos.y + halfsize_y * c + halfsize_x * s;
                let bottom_right_x = pos.x + halfsize_x * c + halfsize_y * s;
                let bottom_right_y = pos.y - halfsize_y * c + halfsize_x * s;
                let bottom_left_x = pos.x - halfsize_x * c + halfsize_y * s;
                let bottom_left_y = pos.y - halfsize_y * c - halfsize_x * s;

                let min_x = top_left_x
                    .min(top_right_x)
                    .min(bottom_right_x)
                    .min(bottom_left_x);
                let max_x = top_left_x
                    .max(top_right_x)
                    .max(bottom_right_x)
                    .max(bottom_left_x);
                let min_y = top_left_y
                    .min(top_right_y)
                    .min(bottom_right_y)
                    .min(bottom_left_y);
                let max_y = top_left_y
                    .max(top_right_y)
                    .max(bottom_right_y)
                    .max(bottom_left_y);

                let i_min_x = (min_x / MAP_XY_FACTOR).floor() as i32;
<<<<<<< Updated upstream
                let _i_min_y = (min_y / MAP_XY_FACTOR).floor() as i32;
                let i_max_x = (max_x / MAP_XY_FACTOR).floor() as i32;
                let i_max_y = (max_y / MAP_XY_FACTOR).floor() as i32;

                // First pass: sample average height within the rotated box
                let mut total_height: f32 = 0.0;
                let mut num_samples: i32 = 0;
                for i in i_min_x..=i_max_x {
                    // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
                    for j in 0..=i_max_y {
                        let test_pt_x = i as f32 * MAP_XY_FACTOR;
                        let test_pt_y = j as f32 * MAP_XY_FACTOR;
                        let match_tri = Self::point_in_triangle_2d(
                            top_left_x,
                            top_left_y,
                            top_right_x,
                            top_right_y,
                            bottom_left_x,
                            bottom_left_y,
                            test_pt_x,
                            test_pt_y,
                        ) || Self::point_in_triangle_2d(
                            top_right_x,
                            top_right_y,
                            bottom_right_x,
                            bottom_right_y,
                            bottom_left_x,
                            bottom_left_y,
                            test_pt_x,
                            test_pt_y,
                        );
                        if match_tri {
                            total_height += self.get_ground_height(test_pt_x, test_pt_y, None);
=======
                let i_min_y = (min_y / MAP_XY_FACTOR).floor() as i32;
                let i_max_x = (max_x / MAP_XY_FACTOR).floor() as i32;
                let i_max_y = (max_y / MAP_XY_FACTOR).floor() as i32;

                // PARITY_NOTE: C++ uses Point_In_Triangle_2D for box containment;
                // our cross-product point-in-rotated-rect test is mathematically equivalent.
                let mut total_height = 0.0f32;
                let mut num_samples: i32 = 0;
                for i in i_min_x..=i_max_x {
                    for j in i_min_y..=i_max_y {
                        let test_x = i as f32 * MAP_XY_FACTOR;
                        let test_y = j as f32 * MAP_XY_FACTOR;
                        if point_in_rotated_rect(
                            test_x, test_y, pos.x, pos.y, halfsize_x, halfsize_y, c, s,
                        ) {
                            total_height += self.get_ground_height(test_x, test_y, None);
>>>>>>> Stashed changes
                            num_samples += 1;
                        }
                    }
                }
                if num_samples == 0 {
                    return;
                }
<<<<<<< Updated upstream
                let avg_height = total_height / num_samples as f32;
                let mut raw_data_height = (0.5 + avg_height / MAP_HEIGHT_SCALE).floor() as i32;

                // Compare to center height — setRawMapHeight only lowers
=======

                let avg_height = total_height / num_samples as f32;
                let mut raw_data_height = (0.5f32 + avg_height / MAP_HEIGHT_SCALE).floor() as i32;

                // C++ setRawMapHeight only lowers, not raise
>>>>>>> Stashed changes
                let center_height =
                    (self.get_ground_height(pos.x, pos.y, None) / MAP_HEIGHT_SCALE).floor() as i32;
                if raw_data_height > center_height {
                    raw_data_height = center_height;
                }
<<<<<<< Updated upstream

                // Second pass: flatten 3x3 area around each matching cell
                for i in i_min_x..=i_max_x {
                    for j in 0..=i_max_y {
                        let test_pt_x = i as f32 * MAP_XY_FACTOR;
                        let test_pt_y = j as f32 * MAP_XY_FACTOR;
                        let match_tri = Self::point_in_triangle_2d(
                            top_left_x,
                            top_left_y,
                            top_right_x,
                            top_right_y,
                            bottom_left_x,
                            bottom_left_y,
                            test_pt_x,
                            test_pt_y,
                        ) || Self::point_in_triangle_2d(
                            top_right_x,
                            top_right_y,
                            bottom_right_x,
                            bottom_right_y,
                            bottom_left_x,
                            bottom_left_y,
                            test_pt_x,
                            test_pt_y,
                        );
                        if match_tri {
                            // Set 3x3 area: center + 4 cardinal + 4 diagonal neighbors
                            for di in -1..=1 {
                                for dj in -1..=1 {
=======
                let raw_data_height = raw_data_height.clamp(0, 255) as u8;

                for i in i_min_x..=i_max_x {
                    for j in i_min_y..=i_max_y {
                        let test_x = i as f32 * MAP_XY_FACTOR;
                        let test_y = j as f32 * MAP_XY_FACTOR;
                        if point_in_rotated_rect(
                            test_x, test_y, pos.x, pos.y, halfsize_x, halfsize_y, c, s,
                        ) {
                            for di in -1i32..=1 {
                                for dj in -1i32..=1 {
>>>>>>> Stashed changes
                                    self.set_raw_map_height(i + di, j + dj, raw_data_height);
                                }
                            }
                        }
                    }
                }
            }
            EngineGeometryType::Sphere | EngineGeometryType::Cylinder => {
<<<<<<< Updated upstream
                let radius = geom.get_major_radius();
                let radius_sqr = radius * radius;
                let i_min_x = ((pos.x - radius) / MAP_XY_FACTOR).floor() as i32;
                let _i_min_y = ((pos.y - radius) / MAP_XY_FACTOR).floor() as i32;
                let i_max_x = ((pos.x + radius) / MAP_XY_FACTOR).floor() as i32;
                let i_max_y = ((pos.y + radius) / MAP_XY_FACTOR).floor() as i32;

                // First pass: sample average height within the circle
                let mut total_height: f32 = 0.0;
                let mut num_samples: i32 = 0;
                for i in i_min_x..=i_max_x {
                    // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
                    for j in 0..=i_max_y {
                        let test_pt_x = i as f32 * MAP_XY_FACTOR;
                        let test_pt_y = j as f32 * MAP_XY_FACTOR;
                        let dx = test_pt_x - pos.x;
                        let dy = test_pt_y - pos.y;
                        if dx * dx + dy * dy < radius_sqr {
                            total_height += self.get_ground_height(test_pt_x, test_pt_y, None);
=======
                // PARITY_NOTE: C++ treats Sphere same as Cylinder ("not quite right, but close enough")
                let radius = major_radius;
                let radius_sqr = radius * radius;

                let i_min_x = ((pos.x - radius) / MAP_XY_FACTOR).floor() as i32;
                let i_min_y = ((pos.y - radius) / MAP_XY_FACTOR).floor() as i32;
                let i_max_x = ((pos.x + radius) / MAP_XY_FACTOR).floor() as i32;
                let i_max_y = ((pos.y + radius) / MAP_XY_FACTOR).floor() as i32;

                let mut total_height = 0.0f32;
                let mut num_samples: i32 = 0;
                for i in i_min_x..=i_max_x {
                    for j in i_min_y..=i_max_y {
                        let test_x = i as f32 * MAP_XY_FACTOR;
                        let test_y = j as f32 * MAP_XY_FACTOR;
                        let dx = test_x - pos.x;
                        let dy = test_y - pos.y;
                        if dx * dx + dy * dy < radius_sqr {
                            total_height += self.get_ground_height(test_x, test_y, None);
>>>>>>> Stashed changes
                            num_samples += 1;
                        }
                    }
                }
                if num_samples == 0 {
                    return;
                }
<<<<<<< Updated upstream
                let avg_height = total_height / num_samples as f32;
                let raw_data_height = (0.5 + avg_height / MAP_HEIGHT_SCALE).floor() as i32;

                // Second pass: flatten 3x3 area around each matching cell
                for i in i_min_x..=i_max_x {
                    for j in 0..=i_max_y {
                        let test_pt_x = i as f32 * MAP_XY_FACTOR;
                        let test_pt_y = j as f32 * MAP_XY_FACTOR;
                        let dx = test_pt_x - pos.x;
                        let dy = test_pt_y - pos.y;
                        if dx * dx + dy * dy < radius_sqr {
                            for di in -1..=1 {
                                for dj in -1..=1 {
=======

                let avg_height = total_height / num_samples as f32;
                let raw_data_height = (0.5f32 + avg_height / MAP_HEIGHT_SCALE)
                    .floor()
                    .clamp(0.0, 255.0) as u8;

                for i in i_min_x..=i_max_x {
                    for j in i_min_y..=i_max_y {
                        let test_x = i as f32 * MAP_XY_FACTOR;
                        let test_y = j as f32 * MAP_XY_FACTOR;
                        let dx = test_x - pos.x;
                        let dy = test_y - pos.y;
                        if dx * dx + dy * dy < radius_sqr {
                            for di in -1i32..=1 {
                                for dj in -1i32..=1 {
>>>>>>> Stashed changes
                                    self.set_raw_map_height(i + di, j + dj, raw_data_height);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

<<<<<<< Updated upstream
    /// Dig a deep circular gorge into the terrain beneath an object.
    /// Reference: C++ TerrainLogic::createCraterInTerrain() in TerrainLogic.cpp
    ///
    /// Creates a crater with radial displacement — deepest at center,
    /// tapering to zero at the edge of the object's radius.
    pub fn create_crater_in_terrain(&mut self, obj: &Arc<RwLock<Object>>) {
        let obj_guard = obj.read().unwrap();
=======
    /// Create crater in terrain. Reference: C++ TerrainLogic::createCraterInTerrain()
    pub fn create_crater_in_terrain(&mut self, obj: &Arc<RwLock<Object>>) {
        let obj_guard = match obj.read() {
            Ok(g) => g,
            Err(_) => return,
        };

>>>>>>> Stashed changes
        if obj_guard.get_geometry_info().get_is_small() {
            return;
        }

<<<<<<< Updated upstream
        let pos = obj_guard.get_position();
        let radius = obj_guard.get_geometry_info().get_major_radius();
        if radius <= 0.0 {
=======
        let pos = *obj_guard.get_position();
        let radius = obj_guard.get_geometry_info().get_major_radius();
        drop(obj_guard);

        if radius <= 0.0f32 {
            return;
        }

        if self.map_data.is_empty() || self.map_dx <= 0 || self.map_dy <= 0 {
>>>>>>> Stashed changes
            return;
        }

        let i_min_x = ((pos.x - radius) / MAP_XY_FACTOR).floor() as i32;
<<<<<<< Updated upstream
        let _i_min_y = ((pos.y - radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_x = ((pos.x + radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_y = ((pos.y + radius) / MAP_XY_FACTOR).floor() as i32;

        for i in i_min_x..=i_max_x {
            // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
=======
        let i_min_y = ((pos.y - radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_x = ((pos.x + radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_y = ((pos.y + radius) / MAP_XY_FACTOR).floor() as i32;

        // PARITY_NOTE: C++ iterates j from 0 instead of i_min_y — replicated for parity
        for i in i_min_x..=i_max_x {
>>>>>>> Stashed changes
            for j in 0..=i_max_y {
                let delta_x = i as f32 * MAP_XY_FACTOR - pos.x;
                let delta_y = j as f32 * MAP_XY_FACTOR - pos.y;
                let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();

                if distance < radius {
<<<<<<< Updated upstream
                    let displacement_amount = radius * (1.0 - distance / radius);
                    let current_height = self.get_raw_map_height(i, j);
                    let target_height = (1i32).max(current_height - displacement_amount as i32);
                    self.set_raw_map_height(i, j, target_height);
                }
            }
        }
    }

    /// Set raw map height at grid position — only lowers, never raises.
    /// Reference: C++ W3DTerrainVisual::setRawMapHeight() in W3DTerrainVisual.cpp
    ///
    /// The C++ implementation only writes if the new height is lower than
    /// the current height, and accounts for border size offset.
    fn set_raw_map_height(&mut self, x: i32, y: i32, height: i32) {
        if x < 0 || y < 0 || x >= self.map_dx || y >= self.map_dy {
            return;
        }
        let idx = (y * self.map_dx + x) as usize;
        if idx >= self.map_data.len() {
            return;
        }
        let height_clamped = height.max(0).min(255) as u8;
        if self.map_data[idx] > height_clamped {
            self.map_data[idx] = height_clamped;
        }
    }

    /// Get raw map height at grid position.
    /// Reference: C++ W3DTerrainVisual::getRawMapHeight() in W3DTerrainVisual.cpp
    fn get_raw_map_height(&self, x: i32, y: i32) -> i32 {
        if x < 0 || y < 0 || x >= self.map_dx || y >= self.map_dy {
            return 0;
        }
        let idx = (y * self.map_dx + x) as usize;
        if idx >= self.map_data.len() {
            return 0;
        }
        self.map_data[idx] as i32
    }

    /// 2D point-in-triangle test using cross products.
    /// Reference: C++ Point_In_Triangle_2D
    fn point_in_triangle_2d(
        v0x: f32,
        v0y: f32,
        v1x: f32,
        v1y: f32,
        v2x: f32,
        v2y: f32,
        px: f32,
        py: f32,
    ) -> bool {
        let d1 = (px - v1x) * (v0y - v1y) - (v0x - v1x) * (py - v1y);
        let d2 = (px - v2x) * (v1y - v2y) - (v1x - v2x) * (py - v2y);
        let d3 = (px - v0x) * (v2y - v0y) - (v2x - v0x) * (py - v0y);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

        !(has_neg && has_pos)
=======
                    let displacement = radius * (1.0f32 - distance / radius);
                    let current = self.get_raw_map_height(i, j) as i32;
                    let target = (current - displacement as i32).max(1);
                    self.set_raw_map_height(i, j, target.clamp(0, 255) as u8);
                }
            }
        }
>>>>>>> Stashed changes
    }

    // Private helper methods

    fn get_raw_map_height(&self, i: i32, j: i32) -> u8 {
        if i < 0 || j < 0 || i >= self.map_dx || j >= self.map_dy {
            return 0;
        }
        let idx = (j * self.map_dx + i) as usize;
        if idx < self.map_data.len() {
            self.map_data[idx]
        } else {
            0
        }
    }

    /// Reference: C++ TerrainVisual::setRawMapHeight() — only lowers, never raises.
    fn set_raw_map_height(&mut self, i: i32, j: i32, new_height: u8) {
        if i < 0 || j < 0 || i >= self.map_dx || j >= self.map_dy {
            return;
        }
        let idx = (j * self.map_dx + i) as usize;
        if idx < self.map_data.len() && new_height < self.map_data[idx] {
            self.map_data[idx] = new_height;
        }
    }

    /// Check if point is inside a bridge polygon using point-in-polygon test
    /// Reference: C++ TerrainLogic.cpp Bridge::isPointOnBridge()
    ///
    /// Uses ray casting algorithm for polygon containment test
    fn is_point_in_bridge(&self, x: f32, y: f32) -> Option<&crate::system::map_loader::BridgeData> {
        let terrain_data = self.terrain_data.as_ref()?;

        for bridge in &terrain_data.bridges {
            if self.point_in_polygon(x, y, &bridge.polygon) {
                return Some(bridge);
            }
        }

        None
    }

    /// Point-in-polygon test using ray casting algorithm
    /// Reference: Standard computational geometry algorithm
    ///
    /// # Arguments
    /// * `x` - X coordinate of point to test
    /// * `y` - Y coordinate of point to test
    /// * `polygon` - Polygon vertices (must be closed, first != last)
    ///
    /// # Returns
    /// true if point is inside polygon, false otherwise
    fn point_in_polygon(
        &self,
        x: f32,
        y: f32,
        polygon: &[crate::system::map_loader::Coord2D],
    ) -> bool {
        if polygon.len() < 3 {
            return false;
        }

        let mut inside = false;
        let n = polygon.len();

        let mut j = n - 1;
        for i in 0..n {
            let xi = polygon[i].x;
            let yi = polygon[i].y;
            let xj = polygon[j].x;
            let yj = polygon[j].y;

            // Ray casting algorithm
            let intersect = ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi);

            if intersect {
                inside = !inside;
            }

            j = i;
        }

        inside
    }

    /// Update dynamic water tables
    fn update_dynamic_water(&mut self) {
        let do_damage_this_frame =
            crate::helpers::TheGameLogic::get_frame() % LOGICFRAMES_PER_SECOND == 0;
        let mut retained = Vec::with_capacity(self.water_to_update.len());
        let mut entries = std::mem::take(&mut self.water_to_update);
        for mut entry in entries.drain(..) {
            entry.current_height += entry.change_per_frame;

            let reached_target = if entry.change_per_frame > 0.0 {
                entry.current_height >= entry.target_height
            } else {
                entry.current_height <= entry.target_height
            };

            if reached_target {
                entry.current_height = entry.target_height;
                self.set_water_height_internal(
                    entry.trigger_id,
                    &entry.water_name,
                    entry.current_height,
                    entry.damage_amount,
                    true,
                );
            } else {
                let per_frame_damage = if do_damage_this_frame {
                    entry.damage_amount
                } else {
                    0.0
                };
                self.set_water_height_internal(
                    entry.trigger_id,
                    &entry.water_name,
                    entry.current_height,
                    per_frame_damage,
                    false,
                );
                retained.push(entry);
            }
        }
        self.water_to_update = retained;
    }

    /// Update bridge damage states
    pub fn update_bridge_damage_states(&mut self) {
        self.bridge_damage_states_changed = false;
        let mut current = self.bridge_list_head.as_deref_mut();
        while let Some(bridge) = current {
            bridge.update_damage_state();
            if bridge.get_bridge_info().damage_state_changed {
                self.bridge_damage_states_changed = true;
            }
            current = bridge.next.as_deref_mut();
        }
        // Match C++ behavior: always flag an update pass.
        self.bridge_damage_states_changed = true;
    }

    /// Checks if the specified bridge object has just been repaired.
    pub fn is_bridge_repaired(&self, bridge_id: ObjectID) -> bool {
        if bridge_id == crate::common::INVALID_ID {
            return false;
        }
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if info.bridge_object_id == bridge_id {
                return info.damage_state_changed
                    && info.cur_damage_state != BodyDamageType::Rubble;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Checks if the specified bridge object has just broken (entered rubble state).
    pub fn is_bridge_broken(&self, bridge_id: ObjectID) -> bool {
        if bridge_id == crate::common::INVALID_ID {
            return false;
        }
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if info.bridge_object_id == bridge_id {
                return info.damage_state_changed
                    && info.cur_damage_state == BodyDamageType::Rubble;
            }
            current = bridge.next.as_deref();
        }
        false
    }

    /// Gets the attack points for a bridge.
    ///
    /// Bridges have two targetable points at either end. This method calculates
    /// those points based on the bridge's geometry.
    ///
    /// Reference: TerrainLogic.cpp lines 1905-1934 getBridgeAttackPoints()
    pub fn get_bridge_attack_points(
        &self,
        bridge_id: ObjectID,
        attack_info: &mut BridgeAttackInfo,
    ) {
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            let info = bridge.get_bridge_info();
            if info.bridge_object_id == bridge_id {
                // Found the right bridge - calculate attack points
                // C++ lines 1914-1926

                // Calculate direction vector from 'from' to 'to' (normalized)
                let mut delta = Coord3D::new(
                    info.to.x - info.from.x,
                    info.to.y - info.from.y,
                    info.to.z - info.from.z,
                );
                let delta_len = delta.length();
                if delta_len > f32::EPSILON {
                    delta.x /= delta_len;
                    delta.y /= delta_len;
                    delta.z /= delta_len;
                }

                // Calculate width vector to get half-width offset
                let width = Coord3D::new(
                    info.from_right.x - info.from_left.x,
                    info.from_right.y - info.from_left.y,
                    info.from_right.z - info.from_left.z,
                );
                let half_width = width.length() / 2.0;

                // Attack point 1: at 'from' end, offset by half-width along bridge direction
                attack_info.attack_point1.x = info.from.x + delta.x * half_width;
                attack_info.attack_point1.y = info.from.y + delta.y * half_width;
                attack_info.attack_point1.z = info.from.z + delta.z * half_width;

                // Attack point 2: at 'to' end, offset by half-width back along bridge direction
                attack_info.attack_point2.x = info.to.x - delta.x * half_width;
                attack_info.attack_point2.y = info.to.y - delta.y * half_width;
                attack_info.attack_point2.z = info.to.z - delta.z * half_width;

                return;
            }
            current = bridge.next.as_deref();
        }

        // Fallback: if bridge not found, use object position for both points
        // C++ lines 1930-1932
        attack_info.attack_point1 = Coord3D::origin();
        attack_info.attack_point2 = Coord3D::origin();
    }

    /// Calculate terrain slope at position
    /// Reference: TerrainLogic.cpp lines 190-234 getTerrainSlope()
    ///
    /// Algorithm:
    /// 1. Sample height at 4 neighboring points
    /// 2. Calculate gradient vectors
    /// 3. Compute slope angle from gradient magnitude
    fn calculate_slope(&self, x: Real, y: Real) -> Real {
        const SAMPLE_OFFSET: Real = 1.0; // 1 world unit offset for gradient calculation

        // Sample heights at 4 neighboring points
        // C++ TerrainLogic.cpp lines 195-198
        let _h_center = self.get_ground_height(x, y, None);
        let h_north = self.get_ground_height(x, y + SAMPLE_OFFSET, None);
        let h_south = self.get_ground_height(x, y - SAMPLE_OFFSET, None);
        let h_east = self.get_ground_height(x + SAMPLE_OFFSET, y, None);
        let h_west = self.get_ground_height(x - SAMPLE_OFFSET, y, None);

        // Calculate gradients in X and Y directions
        // C++ TerrainLogic.cpp lines 200-201
        let gradient_x = (h_east - h_west) / (2.0 * SAMPLE_OFFSET);
        let gradient_y = (h_north - h_south) / (2.0 * SAMPLE_OFFSET);

        // Calculate slope magnitude
        // C++ TerrainLogic.cpp lines 203-204
        let gradient_magnitude = (gradient_x * gradient_x + gradient_y * gradient_y).sqrt();

        // Convert to degrees
        // C++ TerrainLogic.cpp line 206
        let slope_radians = gradient_magnitude.atan();
        let slope_degrees = slope_radians.to_degrees();

        slope_degrees
    }

    /// Get surface type at world position
    /// Maps terrain to SurfaceType enum for physics queries
    fn get_surface_type_at(&self, x: f32, y: f32) -> SurfaceType {
        // Check if underwater first
        let mut water_z = 0.0;
        let mut terrain_z = 0.0;
        if self.is_underwater(x, y, Some(&mut water_z), Some(&mut terrain_z)) {
            return SurfaceType::Water;
        }

        // Check if on bridge
        let pos = Coord3D::new(x, y, terrain_z);
        if let Some(_bridge) = self.find_bridge_at(&pos) {
            return SurfaceType::Bridge;
        }

        // Check slope for cliff detection
        let slope = self.calculate_slope(x, y);
        const CLIFF_THRESHOLD: Real = 45.0;
        if slope >= CLIFF_THRESHOLD {
            return SurfaceType::Cliff;
        }

        // Default to ground
        SurfaceType::Ground
    }

    /// Get water depth at position (0.0 if no water)
    /// Reference: TerrainLogic.cpp lines 157-189 getWaterDepth()
    fn get_water_depth_at(&self, x: f32, y: f32) -> f32 {
        let mut water_z = 0.0;
        let mut terrain_z = 0.0;
        if self.is_underwater(x, y, Some(&mut water_z), Some(&mut terrain_z)) {
            water_z - terrain_z
        } else {
            0.0
        }
    }
}

/// Implement TerrainQuery trait for PhysicsEngine integration
/// Reference: TerrainLogic.cpp matching C++ interface
impl TerrainQuery for TerrainLogic {
    /// Get ground height at position
    /// Reference: TerrainLogic.cpp lines 44-156 getGroundHeight()
    fn get_ground_height(&self, x: Real, y: Real) -> Real {
        self.get_ground_height(x, y, None)
    }

    /// Get water depth at position
    /// Reference: TerrainLogic.cpp lines 157-189 getWaterDepth()
    fn get_water_depth(&self, x: Real, y: Real) -> Real {
        self.get_water_depth_at(x, y)
    }

    /// Get terrain slope angle at position (in degrees)
    /// Reference: TerrainLogic.cpp lines 190-234 getTerrainSlope()
    fn get_terrain_slope(&self, x: Real, y: Real) -> Real {
        self.calculate_slope(x, y)
    }

    /// Check if position is on a bridge
    /// Reference: TerrainLogic.cpp lines 235-278 isOnBridge()
    ///
    /// This implementation uses the loaded bridge data from the map file
    /// and performs point-in-polygon tests to determine if a position
    /// is on any bridge surface.
    fn is_on_bridge(&self, pos: &Coord3D) -> (Bool, Real) {
        // First try the old bridge list (for compatibility)
        if let Some(bridge) = self.find_bridge_at(pos) {
            let height = bridge.get_bridge_height(pos, None);
            return (true, height);
        }

        // Then check loaded bridge data from map file
        if let Some(bridge_data) = self.is_point_in_bridge(pos.x, pos.y) {
            let height = bridge_data.get_height_at(pos.x, pos.y);
            return (true, height);
        }

        (false, 0.0)
    }

    /// Check if position is a cliff (steep slope)
    /// Reference: TerrainLogic.cpp lines 279-298 isCliff()
    fn is_cliff(&self, pos: &Coord3D) -> Bool {
        const CLIFF_THRESHOLD: Real = 45.0;
        let slope = self.calculate_slope(pos.x, pos.y);
        slope >= CLIFF_THRESHOLD
    }

    /// Get surface type at position
    /// Reference: TerrainLogic.cpp lines 299-324 getSurfaceType()
    fn get_surface_type(&self, x: Real, y: Real) -> SurfaceType {
        self.get_surface_type_at(x, y)
    }
}

/// Wrapper to make Arc<RwLock<TerrainLogic>> implement TerrainQuery
/// This allows the global terrain instance to be used by the physics engine
#[derive(Clone)]
pub struct TerrainQueryWrapper(Arc<RwLock<TerrainLogic>>);

impl TerrainQueryWrapper {
    pub fn new(terrain: Arc<RwLock<TerrainLogic>>) -> Self {
        Self(terrain)
    }
}

impl TerrainQuery for TerrainQueryWrapper {
    fn get_ground_height(&self, x: Real, y: Real) -> Real {
        if let Ok(terrain) = self.0.read() {
            terrain.get_ground_height(x, y, None)
        } else {
            0.0
        }
    }

    fn get_water_depth(&self, x: Real, y: Real) -> Real {
        if let Ok(terrain) = self.0.read() {
            terrain.get_water_depth_at(x, y)
        } else {
            0.0
        }
    }

    fn get_terrain_slope(&self, x: Real, y: Real) -> Real {
        if let Ok(terrain) = self.0.read() {
            terrain.calculate_slope(x, y)
        } else {
            0.0
        }
    }

    fn is_on_bridge(&self, pos: &Coord3D) -> (Bool, Real) {
        if let Ok(terrain) = self.0.read() {
            // First try the old bridge list (for compatibility)
            if let Some(bridge) = terrain.find_bridge_at(pos) {
                let height = bridge.get_bridge_height(pos, None);
                return (true, height);
            }

            // Then check loaded bridge data from map file
            if let Some(bridge_data) = terrain.is_point_in_bridge(pos.x, pos.y) {
                let height = bridge_data.get_height_at(pos.x, pos.y);
                return (true, height);
            }
        }
        (false, 0.0)
    }

    fn is_cliff(&self, pos: &Coord3D) -> Bool {
        const CLIFF_THRESHOLD: Real = 45.0;
        if let Ok(terrain) = self.0.read() {
            let slope = terrain.calculate_slope(pos.x, pos.y);
            return slope >= CLIFF_THRESHOLD;
        }
        false
    }

    fn get_surface_type(&self, x: Real, y: Real) -> SurfaceType {
        if let Ok(terrain) = self.0.read() {
            terrain.get_surface_type_at(x, y)
        } else {
            SurfaceType::Ground
        }
    }
}

fn point_in_rotated_rect(
    px: f32,
    py: f32,
    cx: f32,
    cy: f32,
    half_w: f32,
    half_h: f32,
    cos_a: f32,
    sin_a: f32,
) -> bool {
    let dx = px - cx;
    let dy = py - cy;
    let local_x = dx * cos_a + dy * sin_a;
    let local_y = -dx * sin_a + dy * cos_a;
    local_x.abs() <= half_w && local_y.abs() <= half_h
}

fn point_in_convex_quad(point: &Coord2D, quad: &[Coord2D; 4]) -> bool {
    let mut has_positive = false;
    let mut has_negative = false;

    for edge in 0..4 {
        let a = quad[edge];
        let b = quad[(edge + 1) % 4];
        let cross = cross_2d(&a, &b, point);
        if cross > 1.0e-5 {
            has_positive = true;
        } else if cross < -1.0e-5 {
            has_negative = true;
        }

        if has_positive && has_negative {
            return false;
        }
    }

    true
}

fn cross_2d(a: &Coord2D, b: &Coord2D, p: &Coord2D) -> f32 {
    let ab_x = b.x - a.x;
    let ab_y = b.y - a.y;
    let ap_x = p.x - a.x;
    let ap_y = p.y - a.y;
    ab_x * ap_y - ab_y * ap_x
}

fn path_with_map_variants(input: &Path) -> Vec<PathBuf> {
    let mut variants = vec![input.to_path_buf()];
    if input.extension().is_none() {
        variants.push(input.with_extension("map"));
        variants.push(input.with_extension("MAP"));
    }
    variants
}

fn line_in_region(line1: &Coord2D, line2: &Coord2D, region: &Region2D) -> bool {
    // Liang-Barsky clipping for axis-aligned rectangle.
    let x0 = line1.x;
    let y0 = line1.y;
    let x1 = line2.x;
    let y1 = line2.y;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let mut t0 = 0.0f32;
    let mut t1 = 1.0f32;

    let clip = |p: f32, q: f32, t0: &mut f32, t1: &mut f32| -> bool {
        if p.abs() <= f32::EPSILON {
            return q >= 0.0;
        }
        let r = q / p;
        if p < 0.0 {
            if r > *t1 {
                return false;
            }
            if r > *t0 {
                *t0 = r;
            }
        } else {
            if r < *t0 {
                return false;
            }
            if r < *t1 {
                *t1 = r;
            }
        }
        true
    };

    let left = region.lo.x;
    let right = region.hi.x;
    let top = region.lo.y;
    let bottom = region.hi.y;

    if !clip(-dx, x0 - left, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dx, right - x0, &mut t0, &mut t1) {
        return false;
    }
    if !clip(-dy, y0 - top, &mut t0, &mut t1) {
        return false;
    }
    if !clip(dy, bottom - y0, &mut t0, &mut t1) {
        return false;
    }

    t0 <= t1
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn make_water_trigger(
        id: Int,
        name: &str,
        z: Int,
        min_x: Int,
        min_y: Int,
        max_x: Int,
        max_y: Int,
    ) -> PolygonTrigger {
        let mut trigger = PolygonTrigger::new(id, AsciiString::from(name), Vec::new());
        trigger.set_water_area(true);
        trigger.add_point(ICoord3D::new(min_x, min_y, z));
        trigger.add_point(ICoord3D::new(max_x, min_y, z));
        trigger.add_point(ICoord3D::new(max_x, max_y, z));
        trigger.add_point(ICoord3D::new(min_x, max_y, z));
        trigger
    }

    #[test]
    fn bridge_info_from_parts_matches_expected_rectangle() {
        let bridge_info = TerrainLogic::bridge_info_from_parts(
            Coord3D::new(10.0, 20.0, 3.0),
            0.0,
            6.0,
            2.0,
            crate::object::INVALID_ID,
        );

        assert_eq!(bridge_info.from_left, Coord3D::new(4.0, 22.0, 3.0));
        assert_eq!(bridge_info.from_right, Coord3D::new(4.0, 18.0, 3.0));
        assert_eq!(bridge_info.to_left, Coord3D::new(16.0, 22.0, 3.0));
        assert_eq!(bridge_info.to_right, Coord3D::new(16.0, 18.0, 3.0));
        assert_eq!(bridge_info.bridge_width, 4.0);
    }

    #[test]
    fn delete_bridge_at_removes_bridge_from_list() {
        let bridge_info = TerrainLogic::bridge_info_from_parts(
            Coord3D::new(10.0, 20.0, 0.0),
            0.0,
            6.0,
            2.0,
            crate::object::INVALID_ID,
        );

        let mut terrain = TerrainLogic::new();
        let mut bridge = Box::new(Bridge::new(bridge_info, AsciiString::from("TestBridge")));
        bridge.set_layer(PathfindLayerEnum::Bridge1);
        terrain.bridge_list_head = Some(bridge);

        let hit_point = Coord3D::new(10.0, 20.0, 0.0);
        assert!(terrain.delete_bridge_at(&hit_point));
        assert!(terrain.find_bridge_at(&hit_point).is_none());
    }

    #[test]
    fn bridge_point_test_rejects_bounds_only_false_positive() {
        let mut info = BridgeInfo::new();
        info.from_left = Coord3D::new(0.0, 0.0, 0.0);
        info.from_right = Coord3D::new(2.0, 2.0, 0.0);
        info.to_right = Coord3D::new(0.0, 4.0, 0.0);
        info.to_left = Coord3D::new(-2.0, 2.0, 0.0);

        let bridge = Bridge::new(info, AsciiString::from("TestBridge"));
        let false_positive = Coord3D::new(2.0, 0.0, 0.0); // inside AABB, outside rotated bridge quad
        let inside = Coord3D::new(0.0, 2.0, 0.0);

        assert!(!bridge.is_point_on_bridge(&false_positive));
        assert!(bridge.is_point_on_bridge(&inside));
    }

    #[test]
    fn terrain_load_map_returns_false_for_invalid_map_data() {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        let map_path = std::env::temp_dir().join(format!(
            "generalsrust_terrain_invalid_{}_{}.map",
            std::process::id(),
            timestamp
        ));

        std::fs::write(&map_path, b"not-a-valid-map-datachunk").expect("write map fixture");

        let mut terrain = TerrainLogic::new();
        let loaded = terrain.load_map(
            AsciiString::from(map_path.to_string_lossy().as_ref()),
            false,
        );
        assert!(!loaded);

        let _ = std::fs::remove_file(&map_path);
    }

    #[test]
    fn terrain_query_load_skips_new_map_finalization() {
        let mut terrain = TerrainLogic::new();

        terrain.add_waypoint_from_map(&MapWaypoint {
            id: 1,
            name: "WaveGuide1".to_string(),
            location: crate::system::map_loader::Coord3D::new(20.0, 20.0, 5.0),
            path_label1: String::new(),
            path_label2: String::new(),
            path_label3: String::new(),
            bi_directional: false,
        });

        terrain.query_load_pending = true;
        terrain.new_map(false);

        assert!(
            !terrain.water_grid_enabled,
            "query-mode load should skip the follow-up new_map side effects"
        );
        assert!(terrain
            .get_waypoint_by_name(&AsciiString::from("WaveGuide1"))
            .is_some());
    }

    #[test]
    fn water_handle_lookup_by_name_prefers_trigger_identity_over_name_cache() {
        let mut terrain = TerrainLogic::new();
        terrain.add_trigger_area(make_water_trigger(11, "SharedWater", 12, 0, 0, 40, 40));
        terrain.add_trigger_area(make_water_trigger(
            22,
            "SharedWater",
            28,
            100,
            100,
            140,
            140,
        ));

        terrain.water_handles.insert(
            AsciiString::from("SharedWater"),
            WaterHandle::new(
                AsciiString::from("SharedWater"),
                999.0,
                Region3D::new(
                    Coord3D::new(-1.0, -1.0, -1.0),
                    Coord3D::new(-1.0, -1.0, -1.0),
                ),
            ),
        );

        let by_name = terrain
            .get_water_handle_by_name(&AsciiString::from("SharedWater"))
            .expect("expected first matching water trigger");
        assert_eq!(by_name.get_current_height(), 12.0);
        assert_eq!(by_name.get_bounds().lo.z, 12.0);

        let first_location = terrain
            .get_water_handle(10.0, 10.0)
            .expect("expected first trigger to resolve by location");
        assert_eq!(first_location.get_current_height(), 12.0);

        let second_location = terrain
            .get_water_handle(110.0, 110.0)
            .expect("expected second trigger to resolve by location");
        assert_eq!(second_location.get_current_height(), 28.0);
    }

    #[test]
    fn water_handle_lookup_by_name_ignores_orphaned_name_cache_entries() {
        let mut terrain = TerrainLogic::new();
        terrain.water_handles.insert(
            AsciiString::from("OrphanedWater"),
            WaterHandle::new(
                AsciiString::from("OrphanedWater"),
                42.0,
                Region3D::new(Coord3D::new(1.0, 1.0, 1.0), Coord3D::new(2.0, 2.0, 2.0)),
            ),
        );

        assert!(
            terrain
                .get_water_handle_by_name(&AsciiString::from("OrphanedWater"))
                .is_none(),
            "C++ TerrainLogic::getWaterHandleByName only resolves polygon-trigger water handles"
        );
    }

    #[test]
    fn ground_height_returns_zero_for_empty_terrain() {
        let terrain = TerrainLogic::new();
        let h = terrain.get_ground_height(50.0, 50.0, None);
        assert_eq!(h, 0.0, "Empty terrain should return 0.0 height");
    }

    #[test]
    fn ground_height_triangle_interpolation_lower() {
        let mut terrain = TerrainLogic::new();
        let map_data = crate::system::map_loader::MapData {
            heightmap: vec![0, 128, 0, 255],
            width: 2,
            height: 2,
            border_size: 0,
            boundaries: vec![],
            bridges: vec![],
            water_height: None,
            waypoints: vec![],
            waypoint_links: vec![],
            polygon_triggers: vec![],
            texture_tiles: vec![],
        };
        terrain.load_map_data(map_data);

        let h00 = terrain.get_ground_height(0.0, 0.0, None);
        let h10 = terrain.get_ground_height(10.0, 0.0, None);
        let h01 = terrain.get_ground_height(0.0, 10.0, None);
        let h11 = terrain.get_ground_height(10.0, 10.0, None);

        assert!(
            h00 < h11,
            "Corner heights should reflect heightmap: h00={}, h11={}",
            h00,
            h11
        );

        let h_center = terrain.get_ground_height(5.0, 5.0, None);
        let expected = 127.5 * MAP_HEIGHT_SCALE;
        assert!(
            (h_center - expected).abs() < 0.1,
            "Center height should match C++ triangle interpolation: got {}, expected {}",
            h_center,
            expected
        );
    }

    #[test]
    fn ground_height_triangle_interpolation_upper() {
        let mut terrain = TerrainLogic::new();
        let map_data = crate::system::map_loader::MapData {
            heightmap: vec![0, 255, 255, 0],
            width: 2,
            height: 2,
            border_size: 0,
            boundaries: vec![],
            bridges: vec![],
            water_height: None,
            waypoints: vec![],
            waypoint_links: vec![],
            polygon_triggers: vec![],
            texture_tiles: vec![],
        };
        terrain.load_map_data(map_data);

        let h = terrain.get_ground_height(2.0, 8.0, None);
        assert!(h > 0.0, "Upper triangle height should be > 0, got {}", h);
    }

    #[test]
    fn ground_height_matches_cpp_triangle_split() {
        let mut terrain = TerrainLogic::new();
        let map_data = crate::system::map_loader::MapData {
            heightmap: vec![0, 100, 200, 255],
            width: 2,
            height: 2,
            border_size: 0,
            boundaries: vec![],
            bridges: vec![],
            water_height: None,
            waypoints: vec![],
            waypoint_links: vec![],
            polygon_triggers: vec![],
            texture_tiles: vec![],
        };
        terrain.load_map_data(map_data);

        let h = terrain.get_ground_height(5.0, 5.0, None);
        let p0 = 0.0;
        let p1 = 100.0;
        let p2 = 255.0;
        let p3 = 200.0;
        let fx = 0.5;
        let fy = 0.5;
        let expected = if fy > fx {
            p3 + (1.0 - fy) * (p0 - p3) + fx * (p2 - p3)
        } else {
            p1 + fy * (p2 - p1) + (1.0 - fx) * (p0 - p1)
        } * MAP_HEIGHT_SCALE;

        assert!(
            (h - expected).abs() < 0.01,
            "Triangle interpolation mismatch: got {}, expected {}",
            h,
            expected
        );
    }

    #[test]
    fn ground_height_with_border_offset() {
        let mut terrain = TerrainLogic::new();
        let mut heightmap = vec![0u8; 49];
        for i in 0..7 {
            for j in 0..7 {
                heightmap[i * 7 + j] = 128;
            }
        }
        let map_data = crate::system::map_loader::MapData {
            heightmap,
            width: 7,
            height: 7,
            border_size: 1,
            boundaries: vec![],
            bridges: vec![],
            water_height: None,
            waypoints: vec![],
            waypoint_links: vec![],
            polygon_triggers: vec![],
            texture_tiles: vec![],
        };
        terrain.load_map_data(map_data);

        let h = terrain.get_ground_height(10.0, 10.0, None);
        let expected = 128.0 * MAP_HEIGHT_SCALE;
        assert!(
            (h - expected).abs() < 0.1,
            "Height with border offset: got {}, expected {}",
            h,
            expected
        );
    }

    #[test]
    fn ground_height_clamped_at_edges() {
        let mut terrain = TerrainLogic::new();
        let map_data = crate::system::map_loader::MapData {
            heightmap: vec![128; 4],
            width: 2,
            height: 2,
            border_size: 0,
            boundaries: vec![],
            bridges: vec![],
            water_height: None,
            waypoints: vec![],
            waypoint_links: vec![],
            polygon_triggers: vec![],
            texture_tiles: vec![],
        };
        terrain.load_map_data(map_data);

        let h_outside = terrain.get_ground_height(-10.0, -10.0, None);
        assert!(
            h_outside >= 0.0,
            "Out-of-bounds height should be non-negative"
        );
    }

    #[test]
    fn ground_height_normal_computed() {
        let mut terrain = TerrainLogic::new();
        let map_data = crate::system::map_loader::MapData {
            heightmap: vec![0, 0, 0, 255],
            width: 2,
            height: 2,
            border_size: 0,
            boundaries: vec![],
            bridges: vec![],
            water_height: None,
            waypoints: vec![],
            waypoint_links: vec![],
            polygon_triggers: vec![],
            texture_tiles: vec![],
        };
        terrain.load_map_data(map_data);

        let mut normal = Coord3D::new(0.0, 0.0, 0.0);
        let _h = terrain.get_ground_height(5.0, 5.0, Some(&mut normal));
        let len = (normal.x * normal.x + normal.y * normal.y + normal.z * normal.z).sqrt();
        assert!(
            (len - 1.0).abs() < 0.01,
            "Normal should be unit length, got len={}",
            len
        );
        assert!(normal.z > 0.0, "Normal should point upward");
    }
}

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
