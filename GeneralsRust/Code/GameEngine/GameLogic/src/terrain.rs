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

use crate::common::CoordOrigin;
use crate::common::*;
use crate::object::*;
use crate::path::PathfindLayerEnum;
use crate::path::{LAYER_Z_CLOSE_ENOUGH_F, PATHFIND_CELL_SIZE_F};
use crate::physics::{SurfaceType, TerrainQuery};
use crate::polygon_trigger::{PolygonTrigger, PolygonTriggerList};
use crate::system::map_loader::MapWaypoint;
use lazy_static::lazy_static;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

/// Maximum terrain name length
pub const MAX_TERRAIN_NAME_LEN: usize = 64;

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

/// Shared handle to water table for animating water height over time
#[derive(Clone, Debug)]
pub struct DynamicWaterHandle(std::sync::Arc<std::sync::RwLock<WaterHandle>>);

impl DynamicWaterHandle {
    pub fn new(handle: std::sync::Arc<std::sync::RwLock<WaterHandle>>) -> Self {
        Self(handle)
    }

    pub fn get(&self) -> std::sync::Arc<std::sync::RwLock<WaterHandle>> {
        self.0.clone()
    }
}

/// Dynamic water entry for animating water height over time
#[derive(Debug)]
struct DynamicWaterEntry {
    /// Shared pointer to water table to edit
    water_handle: DynamicWaterHandle,
    /// How much height to add each frame (negative = lowering)  
    change_per_frame: f32,
    /// Target height we want to reach
    target_height: f32,
    /// Amount of damage to do to objects underwater
    damage_amount: f32,
    /// Current height (we track this ourselves)
    current_height: f32,
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
    /// Water grid enabled flag
    water_grid_enabled: bool,
    /// Grid water handle
    grid_water_handle: WaterHandle,
    /// Dynamic water tables to update
    water_to_update: Vec<DynamicWaterEntry>,
    /// Map of named water handles
    water_handles: HashMap<AsciiString, WaterHandle>,
    /// Loaded terrain data (heightmap and bridges)
    terrain_data: Option<TerrainData>,
    /// Polygon trigger areas for scripts
    /// Matches C++ ThePolygonTriggerListPtr from PolygonTrigger.h
    trigger_areas: PolygonTriggerList,
}

impl TerrainLogic {
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
            water_grid_enabled: false,
            grid_water_handle: WaterHandle::new(
                "GridWater".to_string().into(),
                0.0,
                Region3D::default(),
            ),
            water_to_update: Vec::new(),
            water_handles: HashMap::new(),
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

        // Set water height if specified
        if let Some(water_height) = map_data.water_height {
            self.grid_water_handle.set_height(water_height);
        }

        // Load polygon trigger areas from map data
        self.trigger_areas.clear();
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

    /// Initialize for new map
    pub fn new_map(&mut self, _save_game: bool) {
        self.reset();
        // Initialize defaults for new map
        self.water_grid_enabled = true;
    }

    /// Get ground height at position
    pub fn get_ground_height(&self, x: f32, y: f32, normal: Option<&mut Coord3D>) -> f32 {
        // Convert world coordinates to map coordinates
        let map_x = x / MAP_XY_FACTOR;
        let map_y = y / MAP_XY_FACTOR;

        // Bounds check
        if map_x < 0.0
            || map_y < 0.0
            || map_x > (self.map_dx - 1).max(0) as f32
            || map_y > (self.map_dy - 1).max(0) as f32
        {
            return 0.0;
        }

        // Bilinear interpolation between height samples
        let x0 = map_x.floor() as i32;
        let y0 = map_y.floor() as i32;
        let x1 = (x0 + 1).min(self.map_dx - 1);
        let y1 = (y0 + 1).min(self.map_dy - 1);
        let fx = map_x - x0 as f32;
        let fy = map_y - y0 as f32;

        let idx00 = (y0 * self.map_dx + x0) as usize;
        let idx10 = (y0 * self.map_dx + x1) as usize;
        let idx01 = (y1 * self.map_dx + x0) as usize;
        let idx11 = (y1 * self.map_dx + x1) as usize;
        if idx00 < self.map_data.len()
            && idx10 < self.map_data.len()
            && idx01 < self.map_data.len()
            && idx11 < self.map_data.len()
        {
            let h00 = self.map_data[idx00] as f32 * MAP_HEIGHT_SCALE;
            let h10 = self.map_data[idx10] as f32 * MAP_HEIGHT_SCALE;
            let h01 = self.map_data[idx01] as f32 * MAP_HEIGHT_SCALE;
            let h11 = self.map_data[idx11] as f32 * MAP_HEIGHT_SCALE;

            let h0 = h00 * (1.0 - fx) + h10 * fx;
            let h1 = h01 * (1.0 - fx) + h11 * fx;
            let world_height = h0 * (1.0 - fy) + h1 * fy;

            if let Some(n) = normal {
                let dx = (h10 - h00) / MAP_XY_FACTOR;
                let dy = (h01 - h00) / MAP_XY_FACTOR;
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

            world_height
        } else {
            0.0
        }
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

    /// Variant that can optionally ignore broken bridges.
    pub fn get_highest_layer_for_destination_with_health(
        &self,
        pos: &Coord3D,
        only_healthy_bridges: bool,
    ) -> PathfindLayerEnum {
        let ground_z = self.get_ground_height(pos.x, pos.y, None);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = pos.z - ground_z;

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

        // Check water handles
        let pos = Coord2D::new(x, y);
        for water in self.water_handles.values() {
            let bounds_2d = Region2D::new(
                Coord2D::new(water.bounds.lo.x, water.bounds.lo.y),
                Coord2D::new(water.bounds.hi.x, water.bounds.hi.y),
            );

            if pos.x >= bounds_2d.lo.x
                && pos.x <= bounds_2d.hi.x
                && pos.y >= bounds_2d.lo.y
                && pos.y <= bounds_2d.hi.y
            {
                if let Some(wz) = water_z {
                    *wz = water.current_height;
                }
                return terrain_height < water.current_height;
            }
        }

        false
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
        let pos = Coord2D::new(x, y);

        for water in self.water_handles.values() {
            let bounds_2d = Region2D::new(
                Coord2D::new(water.bounds.lo.x, water.bounds.lo.y),
                Coord2D::new(water.bounds.hi.x, water.bounds.hi.y),
            );

            if pos.x >= bounds_2d.lo.x
                && pos.x <= bounds_2d.hi.x
                && pos.y >= bounds_2d.lo.y
                && pos.y <= bounds_2d.hi.y
            {
                return Some(water);
            }
        }

        None
    }

    /// Get water handle by name
    pub fn get_water_handle_by_name(&self, name: &AsciiString) -> Option<&WaterHandle> {
        self.water_handles.get(name)
    }

    /// Get water height
    pub fn get_water_height(&self, water: &WaterHandle) -> f32 {
        water.get_current_height()
    }

    /// Set water height
    pub fn set_water_height(
        &mut self,
        water_name: &AsciiString,
        height: f32,
        _damage_amount: f32,
        _force_pathfind_update: bool,
    ) {
        if let Some(water) = self.water_handles.get_mut(water_name) {
            water.set_height(height);
        }
    }

    /// Change water height over time
    pub fn change_water_height_over_time(
        &mut self,
        water_name: &AsciiString,
        final_height: f32,
        transition_time_seconds: f32,
        damage_amount: f32,
    ) {
        if let Some(water) = self.water_handles.get(water_name) {
            let frames_to_complete =
                (transition_time_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
            if frames_to_complete > 0 {
                let change_per_frame =
                    (final_height - water.current_height) / frames_to_complete as f32;

                let entry = DynamicWaterEntry {
                    water_handle: DynamicWaterHandle::new(Arc::new(RwLock::new(water.clone()))),
                    change_per_frame,
                    target_height: final_height,
                    damage_amount,
                    current_height: water.current_height,
                };

                self.water_to_update.push(entry);
            }
        }
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
        let trigger_name = trigger.get_trigger_name().to_string();
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
        let mut current = &mut self.bridge_list_head;
        while let Some(_) = current {
            if current.as_ref().unwrap().is_point_on_bridge(location) {
                let node = current.take().unwrap();
                *current = node.next;
                self.bridge_damage_states_changed = true;
                return true;
            }
            current = &mut current.as_mut().unwrap().next;
        }
        false
    }

    /// Find bridge at layer
    pub fn find_bridge_layer_at(
        &self,
        location: &Coord3D,
        layer: PathfindLayerEnum,
        _clip: bool,
    ) -> Option<&Bridge> {
        let mut current = self.bridge_list_head.as_deref();
        while let Some(bridge) = current {
            if bridge.get_layer() == layer && bridge.is_point_on_bridge(location) {
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
            return matches!(obj.get_layer(), crate::common::PathfindLayerEnum::Wall);
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
        new_bridge.next = self.bridge_list_head.take();
        self.bridge_list_head = Some(new_bridge);
    }

    /// Enable/disable water grid
    pub fn enable_water_grid(&mut self, enable: bool) {
        self.water_grid_enabled = enable;
    }

    /// Get active boundary
    pub fn get_active_boundary(&self) -> i32 {
        self.active_boundary
    }

    /// Set active boundary
    pub fn set_active_boundary(&mut self, new_active_boundary: i32) {
        self.active_boundary = new_active_boundary;
    }

    /// Flatten terrain under object
    pub fn flatten_terrain(&mut self, _obj: &Arc<RwLock<Object>>) {
        // Would flatten terrain under the object
        // This affects the height map data
    }

    /// Create crater in terrain
    pub fn create_crater_in_terrain(&mut self, _obj: &Arc<RwLock<Object>>) {
        // Would create a crater effect in the terrain
    }

    // Private helper methods

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
        self.water_to_update.retain_mut(|entry| {
            entry.current_height += entry.change_per_frame;

            let reached_target = if entry.change_per_frame > 0.0 {
                entry.current_height >= entry.target_height
            } else {
                entry.current_height <= entry.target_height
            };

            if reached_target {
                entry.current_height = entry.target_height;
                if let Ok(mut guard) = entry.water_handle.get().write() {
                    guard.set_height(entry.current_height);
                }
                false // Remove this entry
            } else {
                true // Keep updating
            }
        });
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
        let h_center = self.get_ground_height(x, y, None);
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
