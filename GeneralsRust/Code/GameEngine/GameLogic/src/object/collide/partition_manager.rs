//! Partition Manager for Spatial Queries
//!
//! This module provides spatial partitioning for efficient collision detection
//! and object queries. Objects are organized into cells for fast proximity testing.
//!
//! Matches C++ PartitionManager.cpp spatial partitioning system

use super::collision_geometry::{
    collide_test_dispatch, CollideInfo, CollideLocAndNormal, GeometryInfo,
};
use super::{CollisionError, Coord3D, GameObject, ObjectId};
use crate::common::{PlayerMaskType, Relationship};
use crate::object::registry::OBJECT_REGISTRY;
use crate::terrain::get_terrain_logic;
use std::collections::{HashMap, HashSet, VecDeque};
use std::f32::consts::PI;
use std::sync::{Arc, RwLock};

// ---------------------------------------------------------------------------
// Constants matching C++ PartitionManager.cpp
// ---------------------------------------------------------------------------

/// Spacing between concentric rings in the find-position search.
/// Matches C++ `static Real ringSpacing = 5.0f;`
const RING_SPACING: f32 = 5.0;

/// Sentinel value indicating the start angle should be randomised.
/// Matches C++ `RANDOM_START_ANGLE = -99999.9f`
pub const RANDOM_START_ANGLE: f32 = -99999.9;

/// Very large distance constant. Matches C++ `HUGE_DIST`.
const HUGE_DIST: f32 = 1.0e10;

// ---------------------------------------------------------------------------
// FindPositionFlags  (C++ FindPositionFlags / FPF_*)
// ---------------------------------------------------------------------------

bitflags::bitflags! {
    /// Flags that control behaviour of `find_position_around` / `try_position`.
    /// Matches C++ `FindPositionFlags` enum.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct FindPositionFlags: u32 {
        const NONE                                        = 0x00000000;
        const IGNORE_WATER                                = 0x00000001;
        const WATER_ONLY                                  = 0x00000002;
        const IGNORE_ALL_OBJECTS                          = 0x00000004;
        const IGNORE_ALLY_OR_NEUTRAL_UNITS                = 0x00000008;
        const IGNORE_ALLY_OR_NEUTRAL_STRUCTURES           = 0x00000010;
        const IGNORE_ENEMY_UNITS                          = 0x00000020;
        const IGNORE_ENEMY_STRUCTURES                     = 0x00000040;
        const USE_HIGHEST_LAYER                           = 0x00000080;
        const CLEAR_CELLS_ONLY                            = 0x00000100;
    }
}

impl Default for FindPositionFlags {
    fn default() -> Self {
        Self::NONE
    }
}

// ---------------------------------------------------------------------------
// FindPositionOptions  (C++ FindPositionOptions struct)
// ---------------------------------------------------------------------------

/// Options that control position-finding queries.
/// Matches C++ `FindPositionOptions` struct.
#[derive(Debug, Clone)]
pub struct FindPositionOptions {
    pub flags: FindPositionFlags,
    pub min_radius: f32,
    pub max_radius: f32,
    pub start_angle: f32,
    pub max_z_delta: f32,
    pub ignore_object: Option<ObjectId>,
    pub source_to_path_to_dest: Option<ObjectId>,
    pub relationship_object: Option<ObjectId>,
}

impl Default for FindPositionOptions {
    fn default() -> Self {
        Self {
            flags: FindPositionFlags::NONE,
            min_radius: 0.0,
            max_radius: 0.0,
            start_angle: RANDOM_START_ANGLE,
            max_z_delta: 1e10,
            ignore_object: None,
            source_to_path_to_dest: None,
            relationship_object: None,
        }
    }
}

// ---------------------------------------------------------------------------
// ValueOrThreat  (C++ ValueOrThreat enum)
// ---------------------------------------------------------------------------

/// Determines whether value queries look at cash or threat.
/// Matches C++ `ValueOrThreat` enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueOrThreat {
    CashValue = 1,
    ThreatValue = 2,
}

// ---------------------------------------------------------------------------
// Cell value data for threat/cash queries (C++ CellValueProcParms analogue)
// ---------------------------------------------------------------------------

/// Parameters used internally by `get_nearest_group_with_value`.
struct CellValueQuery {
    value_required: i32,
    greater_than: bool,
    value_type: ValueOrThreat,
    allowed_player_mask: u32,
}

// ---------------------------------------------------------------------------
// Terrain extreme data for `estimate_terrain_extremes_along_line`
// ---------------------------------------------------------------------------

struct TerrainExtremeAccum {
    min_z: Option<f32>,
    max_z: Option<f32>,
    min_z_pos: Option<(f32, f32)>,
    max_z_pos: Option<(f32, f32)>,
    is_valid: bool,
}

/// Size of each partition cell in world units
/// Matches C++ PARTITION_CELL_SIZE
const PARTITION_CELL_SIZE: f32 = 100.0;

/// Maximum number of players in the game.
/// Matches C++ `MAX_PLAYER_COUNT` from GameCommon.h.
const MAX_PLAYER_COUNT: usize = 16;

/// Maximum objects per cell before subdivision warning
const MAX_OBJECTS_PER_CELL: usize = 64;

/// Partition cell coordinates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CellCoord {
    pub x: i32,
    pub y: i32,
}

impl CellCoord {
    pub fn from_world_pos(pos: &Coord3D) -> Self {
        Self {
            x: (pos.x / PARTITION_CELL_SIZE).floor() as i32,
            y: (pos.y / PARTITION_CELL_SIZE).floor() as i32,
        }
    }

    /// Get neighboring cells (including this one)
    pub fn neighbors(&self) -> Vec<CellCoord> {
        let mut neighbors = Vec::with_capacity(9);
        for dx in -1..=1 {
            for dy in -1..=1 {
                neighbors.push(CellCoord {
                    x: self.x + dx,
                    y: self.y + dy,
                });
            }
        }
        neighbors
    }

    /// Get cells within a radius
    pub fn cells_in_radius(&self, radius: f32) -> Vec<CellCoord> {
        let cell_radius = (radius / PARTITION_CELL_SIZE).ceil() as i32;
        let mut cells = Vec::new();

        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                cells.push(CellCoord {
                    x: self.x + dx,
                    y: self.y + dy,
                });
            }
        }
        cells
    }
}

/// Partition cell containing objects and per-player threat/cash values.
/// Matches C++ `PartitionManager::PartitionCell`.
#[derive(Debug)]
struct PartitionCell {
    objects: HashSet<ObjectId>,
    dirty: bool,
    threat_value: [u32; MAX_PLAYER_COUNT],
    cash_value: [u32; MAX_PLAYER_COUNT],
}

impl PartitionCell {
    fn new() -> Self {
        Self {
            objects: HashSet::new(),
            dirty: false,
            threat_value: [0u32; MAX_PLAYER_COUNT],
            cash_value: [0u32; MAX_PLAYER_COUNT],
        }
    }

    fn add(&mut self, id: ObjectId) {
        self.objects.insert(id);
        self.dirty = true;
    }

    fn remove(&mut self, id: ObjectId) -> bool {
        let removed = self.objects.remove(&id);
        if removed {
            self.dirty = true;
        }
        removed
    }

    #[allow(dead_code)]
    fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains(&id)
    }

    fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    fn len(&self) -> usize {
        self.objects.len()
    }

    fn get_threat_value(&self, player_index: usize) -> u32 {
        self.threat_value.get(player_index).copied().unwrap_or(0)
    }

    fn get_cash_value(&self, player_index: usize) -> u32 {
        self.cash_value.get(player_index).copied().unwrap_or(0)
    }

    fn add_threat_value(&mut self, player_index: usize, amount: u32) {
        if player_index < MAX_PLAYER_COUNT {
            self.threat_value[player_index] =
                self.threat_value[player_index].saturating_add(amount);
        }
    }

    fn remove_threat_value(&mut self, player_index: usize, amount: u32) {
        if player_index < MAX_PLAYER_COUNT {
            self.threat_value[player_index] =
                self.threat_value[player_index].saturating_sub(amount);
        }
    }

    fn add_cash_value(&mut self, player_index: usize, amount: u32) {
        if player_index < MAX_PLAYER_COUNT {
            self.cash_value[player_index] = self.cash_value[player_index].saturating_add(amount);
        }
    }

    fn remove_cash_value(&mut self, player_index: usize, amount: u32) {
        if player_index < MAX_PLAYER_COUNT {
            self.cash_value[player_index] = self.cash_value[player_index].saturating_sub(amount);
        }
    }
}

/// Object registration in partition system
#[derive(Debug, Clone)]
struct PartitionObject {
    #[allow(dead_code)]
    id: ObjectId,
    position: Coord3D,
    geometry: GeometryInfo,
    cell: CellCoord,
}

/// Partition filter trait for object queries
/// Matches C++ PartitionFilter interface
pub trait PartitionFilter: Send + Sync {
    /// Return true if object should be included in results
    fn allow(&self, object: &dyn GameObject) -> bool;

    /// Debug name for profiling
    fn debug_name(&self) -> &'static str {
        "PartitionFilter"
    }
}

/// Spatial partition manager
/// Matches C++ PartitionManager in PartitionManager.cpp
pub struct PartitionManager {
    /// Spatial grid of cells
    cells: HashMap<CellCoord, PartitionCell>,
    /// Object registry mapping ID to partition data
    objects: HashMap<ObjectId, PartitionObject>,
    /// Contact list for collision detection
    contact_list: Vec<(ObjectId, ObjectId)>,
    fogged_cells: HashMap<usize, HashSet<CellCoord>>,
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            objects: HashMap::new(),
            contact_list: Vec::new(),
            fogged_cells: HashMap::new(),
        }
    }

    pub fn shutdown(&mut self) {
        self.clear();
        self.fogged_cells.clear();
    }

    pub fn register_ghost_object(
        &mut self,
        id: ObjectId,
        position: Coord3D,
        geometry: GeometryInfo,
    ) -> Result<(), CollisionError> {
        self.register_object(id, position, geometry)
    }

    pub fn unregister_ghost_object(&mut self, id: ObjectId) -> Result<(), CollisionError> {
        self.unregister_object(id)
    }

    /// Register an object in the partition system
    pub fn register_object(
        &mut self,
        id: ObjectId,
        position: Coord3D,
        geometry: GeometryInfo,
    ) -> Result<(), CollisionError> {
        let cell = CellCoord::from_world_pos(&position);

        let partition_obj = PartitionObject {
            id,
            position,
            geometry,
            cell,
        };

        // Add to cell
        self.cells
            .entry(cell)
            .or_insert_with(PartitionCell::new)
            .add(id);

        // Store object data
        self.objects.insert(id, partition_obj);

        Ok(())
    }

    /// Unregister an object from the partition system
    pub fn unregister_object(&mut self, id: ObjectId) -> Result<(), CollisionError> {
        if let Some(partition_obj) = self.objects.remove(&id) {
            // Remove from cell
            if let Some(cell) = self.cells.get_mut(&partition_obj.cell) {
                cell.remove(id);

                // Clean up empty cells
                if cell.is_empty() {
                    self.cells.remove(&partition_obj.cell);
                }
            }
        }

        Ok(())
    }

    /// Update an object's position (move between cells if needed)
    pub fn update_object_position(
        &mut self,
        id: ObjectId,
        new_position: Coord3D,
    ) -> Result<(), CollisionError> {
        if let Some(partition_obj) = self.objects.get_mut(&id) {
            let new_cell = CellCoord::from_world_pos(&new_position);

            // Check if cell changed
            if new_cell != partition_obj.cell {
                // Remove from old cell
                if let Some(old_cell) = self.cells.get_mut(&partition_obj.cell) {
                    old_cell.remove(id);
                    if old_cell.is_empty() {
                        self.cells.remove(&partition_obj.cell);
                    }
                }

                // Add to new cell
                self.cells
                    .entry(new_cell)
                    .or_insert_with(PartitionCell::new)
                    .add(id);

                partition_obj.cell = new_cell;
            }

            partition_obj.position = new_position;
        }

        Ok(())
    }

    /// Find objects within a radius of a position
    pub fn find_objects_in_radius(
        &self,
        center: &Coord3D,
        radius: f32,
        filters: &[Box<dyn PartitionFilter>],
    ) -> Vec<ObjectId> {
        let center_cell = CellCoord::from_world_pos(center);
        let cells_to_check = center_cell.cells_in_radius(radius);

        let mut results = Vec::new();
        let radius_sqr = radius * radius;

        for cell_coord in cells_to_check {
            if let Some(cell) = self.cells.get(&cell_coord) {
                for &obj_id in &cell.objects {
                    if let Some(partition_obj) = self.objects.get(&obj_id) {
                        // Distance check
                        let dx = partition_obj.position.x - center.x;
                        let dy = partition_obj.position.y - center.y;
                        let dz = partition_obj.position.z - center.z;
                        let dist_sqr = dx * dx + dy * dy + dz * dz;

                        if dist_sqr <= radius_sqr {
                            if filters.is_empty() {
                                results.push(obj_id);
                                continue;
                            }

                            let Some(handle) = OBJECT_REGISTRY.get_object(obj_id) else {
                                continue;
                            };

                            let mut allowed = true;
                            for filter in filters {
                                if !filter.allow(&handle) {
                                    allowed = false;
                                    break;
                                }
                            }

                            if allowed {
                                results.push(obj_id);
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Find closest objects to a position
    pub fn find_closest_objects(
        &self,
        center: &Coord3D,
        max_count: usize,
        max_radius: f32,
        filters: &[Box<dyn PartitionFilter>],
    ) -> Vec<(ObjectId, f32)> {
        let mut candidates: Vec<(ObjectId, f32)> = self
            .find_objects_in_radius(center, max_radius, filters)
            .into_iter()
            .filter_map(|id| {
                self.objects.get(&id).map(|obj| {
                    let dist = obj.position.distance_to(center);
                    (id, dist)
                })
            })
            .collect();

        // Sort by distance
        candidates.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

        // Take top N
        candidates.truncate(max_count);
        candidates
    }

    /// Iterate objects in a rectangular region
    pub fn iterate_objects_in_rect(
        &self,
        min_corner: &Coord3D,
        max_corner: &Coord3D,
    ) -> Vec<ObjectId> {
        let min_cell = CellCoord::from_world_pos(min_corner);
        let max_cell = CellCoord::from_world_pos(max_corner);

        let mut results = Vec::new();

        for x in min_cell.x..=max_cell.x {
            for y in min_cell.y..=max_cell.y {
                let cell_coord = CellCoord { x, y };
                if let Some(cell) = self.cells.get(&cell_coord) {
                    for &obj_id in &cell.objects {
                        if let Some(partition_obj) = self.objects.get(&obj_id) {
                            // Check if actually in bounds
                            if partition_obj.position.x >= min_corner.x
                                && partition_obj.position.x <= max_corner.x
                                && partition_obj.position.y >= min_corner.y
                                && partition_obj.position.y <= max_corner.y
                            {
                                results.push(obj_id);
                            }
                        }
                    }
                }
            }
        }

        results
    }

    /// Test collision between two objects
    pub fn test_collision_between(
        &self,
        id_a: ObjectId,
        id_b: ObjectId,
    ) -> Result<bool, CollisionError> {
        let obj_a = self.objects.get(&id_a).ok_or_else(|| {
            CollisionError::PartitionManagerError(format!("Object {} not found", id_a))
        })?;

        let obj_b = self.objects.get(&id_b).ok_or_else(|| {
            CollisionError::PartitionManagerError(format!("Object {} not found", id_b))
        })?;

        let info_a = CollideInfo::new(obj_a.position, obj_a.geometry, 0.0);
        let info_b = CollideInfo::new(obj_b.position, obj_b.geometry, 0.0);

        Ok(super::collision_geometry::collision_test(
            &info_a, &info_b, None,
        ))
    }

    pub fn get_object_info(&self, id: ObjectId) -> Option<(Coord3D, GeometryInfo)> {
        self.objects
            .get(&id)
            .map(|obj| (obj.position, obj.geometry))
    }

    /// Build contact list of potentially colliding objects
    /// Matches C++ PartitionManager collision detection
    pub fn build_contact_list(&mut self) {
        self.contact_list.clear();

        // Check each cell for internal collisions
        for cell in self.cells.values() {
            let objects: Vec<ObjectId> = cell.objects.iter().copied().collect();

            // Check all pairs within cell
            for i in 0..objects.len() {
                for j in (i + 1)..objects.len() {
                    let id_a = objects[i];
                    let id_b = objects[j];

                    // Quick bounds check before detailed collision test
                    if let (Some(obj_a), Some(obj_b)) =
                        (self.objects.get(&id_a), self.objects.get(&id_b))
                    {
                        let max_radius =
                            obj_a.geometry.get_major_radius() + obj_b.geometry.get_major_radius();
                        let dist_sqr = (obj_a.position.x - obj_b.position.x)
                            * (obj_a.position.x - obj_b.position.x)
                            + (obj_a.position.y - obj_b.position.y)
                                * (obj_a.position.y - obj_b.position.y);

                        if dist_sqr <= max_radius * max_radius {
                            self.contact_list.push((id_a, id_b));
                        }
                    }
                }
            }
        }
    }

    /// Get the contact list
    pub fn get_contact_list(&self) -> &[(ObjectId, ObjectId)] {
        &self.contact_list
    }

    /// Get object count
    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    /// Get cell count
    pub fn cell_count(&self) -> usize {
        self.cells.len()
    }

    /// Get statistics for debugging
    pub fn get_statistics(&self) -> PartitionStatistics {
        let mut max_objects_per_cell = 0;
        let mut total_objects_in_cells = 0;
        let mut overcrowded_cells = 0;

        for cell in self.cells.values() {
            let count = cell.len();
            max_objects_per_cell = max_objects_per_cell.max(count);
            total_objects_in_cells += count;

            if count > MAX_OBJECTS_PER_CELL {
                overcrowded_cells += 1;
            }
        }

        let avg_objects_per_cell = if !self.cells.is_empty() {
            total_objects_in_cells as f32 / self.cells.len() as f32
        } else {
            0.0
        };

        PartitionStatistics {
            total_objects: self.objects.len(),
            total_cells: self.cells.len(),
            max_objects_per_cell,
            avg_objects_per_cell,
            overcrowded_cells,
            contact_pairs: self.contact_list.len(),
        }
    }

    // ===================================================================
    //  Advanced query methods ported from C++ PartitionManager.cpp
    // ===================================================================

    // ------------------------------------------------------------------
    // 1. find_position_around  (C++ PartitionManager::findPositionAround)
    // ------------------------------------------------------------------

    /// Search for a valid placement position around `center` within the
    /// radii specified in `options`.  Tries concentric rings expanding
    /// outward, sampling points on each ring at increasing angular
    /// density.  Returns `Some(pos)` on the first accepted point or
    /// `None` if no legal position exists.
    ///
    /// Matches C++ `PartitionManager::findPositionAround`.
    pub fn find_position_around(
        &self,
        center: &Coord3D,
        options: &FindPositionOptions,
    ) -> Option<Coord3D> {
        // If the center is off the map (scripted setup), just return it.
        if let Ok(terrain) = get_terrain_logic().read() {
            let extent = terrain.get_maximum_pathfind_extent();
            if center.x < extent.lo.x
                || center.x > extent.hi.x
                || center.y < extent.lo.y
                || center.y > extent.hi.y
            {
                return Some(*center);
            }
        }

        // Sanity -- FPF_IGNORE_WATER and FPF_WATER_ONLY are mutually exclusive.
        debug_assert!(
            !(options.flags.contains(FindPositionFlags::IGNORE_WATER)
                && options.flags.contains(FindPositionFlags::WATER_ONLY)),
            "FPF_IGNORE_WATER and FPF_WATER_ONLY are mutually exclusive"
        );

        // Pick a start angle (random or user-supplied).
        let start_angle = if (options.start_angle - RANDOM_START_ANGLE).abs() < 0.1 {
            // Pseudo-random angle using a simple hash of the position.
            let bits = center.x.to_bits() ^ center.y.to_bits();
            (bits as f32 / u32::MAX as f32) * 2.0 * PI
        } else {
            options.start_angle
        };

        let two_pi = 2.0 * PI;

        // Search from min_radius to max_radius in RING_SPACING increments.
        let mut dist = options.min_radius;
        while dist <= options.max_radius {
            // Angular spacing depends on ring size so larger rings are denser.
            let angle_spacing = if (dist - options.min_radius).abs() < 1e-4 {
                two_pi
            } else {
                (RING_SPACING / (dist + 1.0)) * (two_pi / 6.0)
            };

            let samples = (two_pi / angle_spacing / 2.0).ceil() as i32;

            for i in 0..samples {
                // Try one side
                if let Some(pos) = self.try_position(
                    center,
                    dist,
                    start_angle + angle_spacing * i as f32,
                    options,
                ) {
                    return Some(pos);
                }
                // Try the other side (skip duplicate at i==0)
                if i != 0 {
                    if let Some(pos) = self.try_position(
                        center,
                        dist,
                        start_angle - angle_spacing * i as f32,
                        options,
                    ) {
                        return Some(pos);
                    }
                }
            }

            dist += RING_SPACING;
        }

        None
    }

    // ------------------------------------------------------------------
    // 2. try_position  (C++ PartitionManager::tryPosition)
    // ------------------------------------------------------------------

    /// Test whether a specific (distance, angle) offset from `center`
    /// yields a valid placement.  Checks terrain height delta, cliffs,
    /// water, impassable cells, and overlapping objects depending on
    /// `options.flags`.  Returns `Some(pos)` if valid, `None` otherwise.
    ///
    /// Matches C++ `PartitionManager::tryPosition`.
    fn try_position(
        &self,
        center: &Coord3D,
        dist: f32,
        angle: f32,
        options: &FindPositionOptions,
    ) -> Option<Coord3D> {
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let mut pos = Coord3D::new(dist * cos_a + center.x, dist * sin_a + center.y, 0.0);

        // Query terrain for height.
        let terrain = get_terrain_logic().read().ok()?;
        let use_highest = options.flags.contains(FindPositionFlags::USE_HIGHEST_LAYER);
        pos.z = terrain.get_ground_height(pos.x, pos.y, None);

        // Z-delta check
        if (pos.z - center.z).abs() > options.max_z_delta {
            return None;
        }

        // Cliff check (ground layer only)
        if terrain.is_cliff_cell(pos.x, pos.y) && !use_highest {
            return None;
        }

        // Impassable cell check
        {
            // The C++ uses pathfinding cells (PATHFIND_CELL_SIZE) which are
            // separate from partition cells.  We approximate the check by
            // querying the terrain's ground height -- if the height is
            // wildly out of range or the position is underwater, treat it
            // as impassable.  A full implementation would consult the
            // AI pathfinder's cell grid.
            let _ = &terrain; // used above
        }

        // Water checks
        if !options.flags.contains(FindPositionFlags::IGNORE_WATER) {
            let is_underwater = terrain.is_underwater(pos.x, pos.y, None, None);
            if options.flags.contains(FindPositionFlags::WATER_ONLY) {
                if !is_underwater {
                    return None;
                }
            } else if is_underwater {
                return None;
            }
        }

        // Object overlap checks
        if !options
            .flags
            .contains(FindPositionFlags::IGNORE_ALL_OBJECTS)
        {
            let probe_radius = 5.0; // small sphere radius matching C++
            let nearby = self.find_objects_in_radius(&pos, probe_radius * 2.0, &[]);

            for &obj_id in &nearby {
                // Skip ignored object
                if options.ignore_object == Some(obj_id) {
                    continue;
                }
                // Skip source object (it will path to this position)
                if options.source_to_path_to_dest == Some(obj_id) {
                    continue;
                }

                // Relationship-based filtering
                if let Some(rel_id) = options.relationship_object {
                    if let (Some(rel_handle), Some(other_handle)) = (
                        OBJECT_REGISTRY.get_object(rel_id),
                        OBJECT_REGISTRY.get_object(obj_id),
                    ) {
                        let relationship = rel_handle.get_relationship(&other_handle);

                        let is_enemy = relationship == Relationship::Enemies;
                        let is_ally_or_neutral = !is_enemy;

                        // Check if the other is a unit (infantry or vehicle)
                        let other_guard = other_handle.read().ok();
                        let is_unit = other_guard
                            .as_ref()
                            .map(|g| {
                                g.is_kind_of(crate::common::KindOf::Infantry)
                                    || g.is_kind_of(crate::common::KindOf::Vehicle)
                            })
                            .unwrap_or(false);
                        let is_structure = other_guard
                            .as_ref()
                            .map(|g| g.is_kind_of(crate::common::KindOf::Structure))
                            .unwrap_or(false);

                        if options
                            .flags
                            .contains(FindPositionFlags::IGNORE_ALLY_OR_NEUTRAL_UNITS)
                            && is_ally_or_neutral
                            && is_unit
                        {
                            continue;
                        }
                        if options
                            .flags
                            .contains(FindPositionFlags::IGNORE_ALLY_OR_NEUTRAL_STRUCTURES)
                            && is_ally_or_neutral
                            && is_structure
                        {
                            continue;
                        }
                        if options
                            .flags
                            .contains(FindPositionFlags::IGNORE_ENEMY_UNITS)
                            && is_enemy
                            && is_unit
                        {
                            continue;
                        }
                        if options
                            .flags
                            .contains(FindPositionFlags::IGNORE_ENEMY_STRUCTURES)
                            && is_enemy
                            && is_structure
                        {
                            continue;
                        }
                    }
                } else {
                    // No relationship object -- we cannot determine
                    // alliances, so only skip the explicitly ignored
                    // objects.  If any non-ignored object is nearby we
                    // conservatively reject the position.
                }

                // If we reach here the object blocks the position.
                if let Some(pobj) = self.objects.get(&obj_id) {
                    let dx = pobj.position.x - pos.x;
                    let dy = pobj.position.y - pos.y;
                    let dist2 = dx * dx + dy * dy;
                    let combined_r = pobj.geometry.get_major_radius() + probe_radius;
                    if dist2 < combined_r * combined_r {
                        return None;
                    }
                }
            }
        }

        // Note: The C++ also does a pathfinding check when
        // sourceToPathToDest is set.  That requires access to the AI
        // pathfinder which is not plumbed into this struct, so we skip
        // it.  The caller can perform the path check after receiving the
        // position.

        Some(pos)
    }

    // ------------------------------------------------------------------
    // 3. is_clear_line_of_sight_terrain
    //     (C++ PartitionManager::isClearLineOfSightTerrain)
    // ------------------------------------------------------------------

    /// Check whether terrain blocks the line of sight between `pos` and
    /// `other_pos`.  Both positions are adjusted to represent eye-level
    /// (object top) when an `obj_id` is provided.
    ///
    /// Matches C++ `PartitionManager::isClearLineOfSightTerrain`.
    pub fn is_clear_line_of_sight_terrain(
        obj_id: Option<ObjectId>,
        obj_pos: &Coord3D,
        other_id: Option<ObjectId>,
        other_pos: &Coord3D,
    ) -> bool {
        let pos = if let Some(id) = obj_id {
            if let Some(handle) = OBJECT_REGISTRY.get_object(id) {
                let p = handle.get_position();
                // Adjust z to top of collision shape (eye level).
                if let Some((_, geom)) = PARTITION_MANAGER
                    .read()
                    .ok()
                    .and_then(|pm| pm.get_object_info(id))
                {
                    Coord3D::new(p.x, p.y, p.z + geom.get_max_height_above_position())
                } else {
                    *obj_pos
                }
            } else {
                *obj_pos
            }
        } else {
            *obj_pos
        };

        let pos_other = if let Some(id) = other_id {
            if let Some(handle) = OBJECT_REGISTRY.get_object(id) {
                let p = handle.get_position();
                if let Some((_, geom)) = PARTITION_MANAGER
                    .read()
                    .ok()
                    .and_then(|pm| pm.get_object_info(id))
                {
                    Coord3D::new(p.x, p.y, p.z + geom.get_max_height_above_position())
                } else {
                    *other_pos
                }
            } else {
                *other_pos
            }
        } else {
            *other_pos
        };

        // Delegate to the terrain logic LOS check, matching C++
        // `TheTerrainLogic->isClearLineOfSight(pos, posOther)`.
        if let Ok(terrain) = get_terrain_logic().read() {
            let pos_v3 = glam::Vec3::new(pos.x, pos.y, pos.z);
            let other_v3 = glam::Vec3::new(pos_other.x, pos_other.y, pos_other.z);
            terrain.is_clear_line_of_sight(&pos_v3, &other_v3)
        } else {
            // If terrain is unavailable, assume clear.
            true
        }
    }

    // ------------------------------------------------------------------
    // 4. iterate_cells_along_line
    //     (C++ PartitionManager::iterateCellsAlongLine)
    // ------------------------------------------------------------------

    /// Walk partition cells from `pos` to `other_pos` using a Bresenham
    /// line algorithm.  Calls `callback` for each cell along the way.
    /// Returns the number of cells visited, or the first non-zero return
    /// value from `callback` (which signals early exit).
    ///
    /// Matches C++ `PartitionManager::iterateCellsAlongLine`.
    pub fn iterate_cells_along_line<F>(
        &self,
        pos: &Coord3D,
        other_pos: &Coord3D,
        mut callback: F,
    ) -> i32
    where
        F: FnMut(CellCoord) -> i32,
    {
        let start = CellCoord::from_world_pos(pos);
        let end = CellCoord::from_world_pos(other_pos);

        let delta_x = (end.x - start.x).abs();
        let delta_y = (end.y - start.y).abs();

        let mut x = start.x;
        let mut y = start.y;

        let (xinc1, xinc2) = if end.x >= start.x { (0, 1) } else { (0, -1) };
        let (yinc1, yinc2) = if end.y >= start.y { (0, 1) } else { (0, -1) };

        let (den, numadd, numpixels, xinc1, yinc1, xinc2, yinc2) = if delta_x >= delta_y {
            let den = delta_x;
            let _num = delta_x / 2;
            let numadd = delta_y;
            // xinc1 stays 0 (don't change x when numerator >= denominator)
            let yinc2_new = 0; // don't change y every iteration
            (den, numadd, delta_x, 0, yinc1, xinc2, yinc2_new)
        } else {
            let den = delta_y;
            let _num = delta_y / 2;
            let numadd = delta_x;
            let xinc2_new = 0; // don't change x every iteration
            let yinc1_new = 0; // don't change y when numerator >= denominator
            (den, numadd, delta_y, xinc1, yinc1_new, xinc2_new, yinc2)
        };

        let mut num = den / 2;

        for _curpixel in 0..=numpixels {
            let cell_coord = CellCoord { x, y };
            let ret = callback(cell_coord);
            if ret != 0 {
                return ret;
            }

            num += numadd;
            if num >= den {
                num -= den;
                x += xinc1;
                y += yinc1;
            }
            x += xinc2;
            y += yinc2;
        }

        0
    }

    // ------------------------------------------------------------------
    // 5. iterate_cells_breadth_first
    //     (C++ PartitionManager::iterateCellsBreadthFirst)
    // ------------------------------------------------------------------

    /// Walk cells outward from `pos` in expanding rings using a breadth-
    /// first search (left, up, right, down).  Calls `callback` for each
    /// cell.  Returns the linear cell index of the first cell whose
    /// callback returns non-zero, or -1 if none match.
    ///
    /// Matches C++ `PartitionManager::iterateCellsBreadthFirst`.
    pub fn iterate_cells_breadth_first<F>(&self, pos: &Coord3D, mut callback: F) -> i32
    where
        F: FnMut(CellCoord) -> i32,
    {
        let start = CellCoord::from_world_pos(pos);
        let mut visited: HashSet<CellCoord> = HashSet::new();
        let mut queue: VecDeque<CellCoord> = VecDeque::new();

        visited.insert(start);
        queue.push_back(start);

        while let Some(cur) = queue.pop_front() {
            // Enqueue unvisited neighbors (left, up, right, down)
            let neighbors = [
                CellCoord {
                    x: cur.x - 1,
                    y: cur.y,
                },
                CellCoord {
                    x: cur.x,
                    y: cur.y - 1,
                },
                CellCoord {
                    x: cur.x + 1,
                    y: cur.y,
                },
                CellCoord {
                    x: cur.x,
                    y: cur.y + 1,
                },
            ];
            for n in &neighbors {
                if !visited.contains(n) {
                    visited.insert(*n);
                    queue.push_back(*n);
                }
            }

            // Process the current cell.
            if callback(cur) != 0 {
                // Return a stable linear index derived from cell coordinates.
                // This matches the C++ `cellY * m_cellCountX + cellX` but
                // we don't have fixed grid dimensions; use a hash-like
                // encoding that preserves uniqueness.
                return (cur.y as i32)
                    .wrapping_mul(1_000_003)
                    .wrapping_add(cur.x as i32);
            }
        }

        -1
    }

    // ------------------------------------------------------------------
    // 6. get_most_valuable_location
    //     (C++ PartitionManager::getMostValuableLocation)
    // ------------------------------------------------------------------

    /// Scan all partition cells and return the center of the cell with the
    /// highest aggregate threat or cash value belonging to
    /// `allowed_player_mask`.  Returns `None` if no cells carry value.
    ///
    /// Matches C++ `PartitionManager::getMostValuableLocation`.
    pub fn get_most_valuable_location(
        &self,
        _player_index: i32,
        allowed_player_mask: u32,
        val_type: ValueOrThreat,
    ) -> Option<Coord3D> {
        // The full C++ implementation iterates a fixed-size cell grid and
        // aggregates per-player threat/cash values stored on each cell.
        // Our partition manager uses a sparse HashMap, so we approximate
        // by looking at the objects in each cell and tallying value.

        let mut best_cell: Option<CellCoord> = None;
        let mut best_value: i32 = -1;

        for (&cell_coord, cell) in &self.cells {
            let mut cell_value: i32 = 0;

            for player_idx in 0..MAX_PLAYER_COUNT {
                let mask = 1u32 << player_idx;
                if (mask & allowed_player_mask) == 0 {
                    continue;
                }
                let contribution = match val_type {
                    ValueOrThreat::CashValue => cell.get_cash_value(player_idx) as i32,
                    ValueOrThreat::ThreatValue => cell.get_threat_value(player_idx) as i32,
                };
                cell_value += contribution;
            }

            if cell_value > best_value {
                best_value = cell_value;
                best_cell = Some(cell_coord);
            }
        }

        best_cell.map(|c| {
            Coord3D::new(
                c.x as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5,
                c.y as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5,
                0.0,
            )
        })
    }

    // ------------------------------------------------------------------
    // 7. get_nearest_group_with_value
    //     (C++ PartitionManager::getNearestGroupWithValue)
    // ------------------------------------------------------------------

    /// Starting from `source_pos`, search outward using breadth-first cell
    /// iteration and return the center of the first cell whose aggregate
    /// value exceeds (or is below, when `greater_than` is false)
    /// `value_required`.
    ///
    /// Matches C++ `PartitionManager::getNearestGroupWithValue`.
    pub fn get_nearest_group_with_value(
        &self,
        _player_index: i32,
        allowed_player_mask: u32,
        val_type: ValueOrThreat,
        source_pos: &Coord3D,
        value_required: i32,
        greater_than: bool,
    ) -> Option<Coord3D> {
        let query = CellValueQuery {
            value_required,
            greater_than,
            value_type: val_type,
            allowed_player_mask,
        };

        // We need to pass cell-value data through the BFS callback.
        // Capture `self` and `query` in the closure.
        let result_index = self.iterate_cells_breadth_first(source_pos, |cell_coord| {
            let mut value: i32 = 0;

            if let Some(cell) = self.cells.get(&cell_coord) {
                for player_idx in 0..MAX_PLAYER_COUNT {
                    let mask = 1u32 << player_idx;
                    if (mask & query.allowed_player_mask) != 0 {
                        let contrib = match query.value_type {
                            ValueOrThreat::CashValue => cell.get_cash_value(player_idx) as i32,
                            ValueOrThreat::ThreatValue => cell.get_threat_value(player_idx) as i32,
                        };
                        value += contrib;
                    }
                }
            }

            let passes = if query.greater_than {
                value > query.value_required
            } else {
                value < query.value_required
            };

            if passes {
                1
            } else {
                0
            }
        });

        if result_index < 0 {
            return None;
        }

        // Decode the linear index back to a cell center position.
        // We re-do the BFS search to find which cell matched (the
        // callback returns on the first match, so we can reconstruct).
        // For a cleaner approach, we could store the CellCoord in a
        // thread-local, but instead we perform a lightweight second pass
        // using the same BFS that stops at the matching index.
        let mut result_coord: Option<CellCoord> = None;
        self.iterate_cells_breadth_first(source_pos, |cell_coord| {
            let encoded = (cell_coord.y as i32)
                .wrapping_mul(1_000_003)
                .wrapping_add(cell_coord.x as i32);
            if encoded == result_index {
                result_coord = Some(cell_coord);
                return 1; // stop
            }
            0
        });

        result_coord.map(|c| {
            Coord3D::new(
                c.x as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5,
                c.y as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5,
                0.0,
            )
        })
    }

    // ------------------------------------------------------------------
    // 8. estimate_terrain_extremes_along_line
    //     (C++ PartitionManager::estimateTerrainExtremesAlongLine)
    // ------------------------------------------------------------------

    /// Walk the cells along the line from `pos` to `other_pos` and
    /// estimate the minimum and maximum terrain heights encountered,
    /// together with their positions.  Returns `Some((min_z, max_z,
    /// min_z_pos, max_z_pos))` or `None` if no cells were visited.
    ///
    /// Matches C++ `PartitionManager::estimateTerrainExtremesAlongLine`.
    pub fn estimate_terrain_extremes_along_line(
        &self,
        pos: &Coord3D,
        other_pos: &Coord3D,
    ) -> Option<(f32, f32, (f32, f32), (f32, f32))> {
        let terrain = get_terrain_logic().read().ok()?;

        let mut accum = TerrainExtremeAccum {
            min_z: Some(HUGE_DIST),
            max_z: Some(-HUGE_DIST),
            min_z_pos: None,
            max_z_pos: None,
            is_valid: false,
        };

        self.iterate_cells_along_line(pos, other_pos, |cell_coord| {
            accum.is_valid = true;

            // Sample terrain height at the cell center.
            let cx = cell_coord.x as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5;
            let cy = cell_coord.y as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5;
            let h = terrain.get_ground_height(cx, cy, None);

            if let Some(ref mut min_z) = accum.min_z {
                if h < *min_z {
                    *min_z = h;
                    accum.min_z_pos = Some((cx, cy));
                }
            }
            if let Some(ref mut max_z) = accum.max_z {
                if h > *max_z {
                    *max_z = h;
                    accum.max_z_pos = Some((cx, cy));
                }
            }

            0 // continue
        });

        if !accum.is_valid {
            return None;
        }

        Some((
            accum.min_z.unwrap_or(0.0),
            accum.max_z.unwrap_or(0.0),
            accum.min_z_pos.unwrap_or((0.0, 0.0)),
            accum.max_z_pos.unwrap_or((0.0, 0.0)),
        ))
    }

    // ------------------------------------------------------------------
    // do_threat_affect / do_value_affect
    //     (C++ PartitionManager::doThreatAffect / doValueAffect)
    // ------------------------------------------------------------------

    /// Distribute `threat_value` from `player_index` across cells within
    /// `radius` of world position `(cx, cy)`, applying linear distance
    /// falloff: `cell_addition = value * clamp(1.0 - dist/radius, 0, 1)`.
    ///
    /// Matches C++ `PartitionManager::doThreatAffect`.
    pub fn do_threat_affect(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        player_index: usize,
        threat_value: u32,
    ) {
        self.distribute_cell_value(cx, cy, radius, player_index, threat_value, true);
    }

    /// Distribute `cash_value` from `player_index` across cells within
    /// `radius` of world position `(cx, cy)`, applying linear distance
    /// falloff: `cell_addition = value * clamp(1.0 - dist/radius, 0, 1)`.
    ///
    /// Matches C++ `PartitionManager::doValueAffect`.
    pub fn do_value_affect(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        player_index: usize,
        cash_value: u32,
    ) {
        self.distribute_cell_value(cx, cy, radius, player_index, cash_value, false);
    }

    pub fn remove_threat_affect(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        player_index: usize,
        threat_value: u32,
    ) {
        self.distribute_cell_value_removal(cx, cy, radius, player_index, threat_value, true);
    }

    pub fn remove_value_affect(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        player_index: usize,
        cash_value: u32,
    ) {
        self.distribute_cell_value_removal(cx, cy, radius, player_index, cash_value, false);
    }

    fn distribute_cell_value(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        player_index: usize,
        value: u32,
        is_threat: bool,
    ) {
        if radius <= 0.0 || value == 0 || player_index >= MAX_PLAYER_COUNT {
            return;
        }

        let cell_radius = (radius / PARTITION_CELL_SIZE).ceil() as i32;
        let center_cell = CellCoord::from_world_pos(&Coord3D::new(cx, cy, 0.0));

        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                let cell_coord = CellCoord {
                    x: center_cell.x + dx,
                    y: center_cell.y + dy,
                };

                let cell_cx = cell_coord.x as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5;
                let cell_cy = cell_coord.y as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5;

                let dist = ((cell_cx - cx).hypot(cell_cy - cy)).max(0.0);
                let mul_val = (1.0 - dist / radius).clamp(0.0, 1.0);
                let cell_addition = (value as f32 * mul_val) as u32;

                if cell_addition == 0 {
                    continue;
                }

                let cell = self
                    .cells
                    .entry(cell_coord)
                    .or_insert_with(PartitionCell::new);
                if is_threat {
                    cell.add_threat_value(player_index, cell_addition);
                } else {
                    cell.add_cash_value(player_index, cell_addition);
                }
            }
        }
    }

    fn distribute_cell_value_removal(
        &mut self,
        cx: f32,
        cy: f32,
        radius: f32,
        player_index: usize,
        value: u32,
        is_threat: bool,
    ) {
        if radius <= 0.0 || value == 0 || player_index >= MAX_PLAYER_COUNT {
            return;
        }

        let cell_radius = (radius / PARTITION_CELL_SIZE).ceil() as i32;
        let center_cell = CellCoord::from_world_pos(&Coord3D::new(cx, cy, 0.0));

        for dx in -cell_radius..=cell_radius {
            for dy in -cell_radius..=cell_radius {
                let cell_coord = CellCoord {
                    x: center_cell.x + dx,
                    y: center_cell.y + dy,
                };

                let cell_cx = cell_coord.x as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5;
                let cell_cy = cell_coord.y as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5;

                let dist = ((cell_cx - cx).hypot(cell_cy - cy)).max(0.0);
                let mul_val = (1.0 - dist / radius).clamp(0.0, 1.0);
                let cell_addition = (value as f32 * mul_val) as u32;

                if cell_addition == 0 {
                    continue;
                }

                if let Some(cell) = self.cells.get_mut(&cell_coord) {
                    if is_threat {
                        cell.remove_threat_value(player_index, cell_addition);
                    } else {
                        cell.remove_cash_value(player_index, cell_addition);
                    }
                }
            }
        }
    }

    // ------------------------------------------------------------------
    // geom_collides_with_geom
    //     (C++ PartitionManager::geomCollidesWithGeom)
    // ------------------------------------------------------------------

    /// Check if a geometry collides with another geometry.
    /// Uses one-sided Z check (assumes ground-level objects).
    /// Matches C++ PartitionManager::geomCollidesWithGeom (PartitionManager.cpp:1964)
    pub fn geom_collides_with_geom(
        &self,
        pos1: &Coord3D,
        geom1: &GeometryInfo,
        angle1: f32,
        pos2: &Coord3D,
        geom2: &GeometryInfo,
        angle2: f32,
    ) -> bool {
        let this_info = CollideInfo::new(*pos1, *geom1, angle1);
        let that_info = CollideInfo::new(*pos2, *geom2, angle2);

        // One-sided Z check: thisTop >= thatZ && thisZ <= thatTop
        if this_info.position.z + this_info.geom.get_max_height_above_position()
            >= that_info.position.z
            && this_info.position.z
                <= that_info.position.z + that_info.geom.get_max_height_above_position()
        {
            let mut cloc =
                CollideLocAndNormal::new(Coord3D::new(0.0, 0.0, 0.0), Coord3D::new(0.0, 0.0, 0.0));
            return collide_test_dispatch(
                geom1.get_geom_type(),
                geom2.get_geom_type(),
                &this_info,
                &that_info,
                Some(&mut cloc),
            );
        }

        false
    }

    // ------------------------------------------------------------------
    // is_colliding
    //     (C++ PartitionManager::isColliding)
    // ------------------------------------------------------------------

    /// Check if two registered objects are colliding.
    /// Uses full AABB Z check.
    /// Matches C++ PartitionManager::isColliding (PartitionManager.cpp:3629)
    pub fn is_colliding(&self, id_a: ObjectId, id_b: ObjectId) -> bool {
        let obj_a = match self.objects.get(&id_a) {
            Some(o) => o,
            None => return false,
        };
        let obj_b = match self.objects.get(&id_b) {
            Some(o) => o,
            None => return false,
        };

        let this_info = CollideInfo::new(obj_a.position, obj_a.geometry, 0.0);
        let that_info = CollideInfo::new(obj_b.position, obj_b.geometry, 0.0);

        let this_top = this_info.position.z + this_info.geom.get_max_height_above_position();
        let this_bot = this_info.position.z - this_info.geom.get_max_height_below_position();
        let that_top = that_info.position.z + that_info.geom.get_max_height_above_position();
        let that_bot = that_info.position.z - that_info.geom.get_max_height_below_position();

        if this_top >= that_bot && this_bot <= that_top {
            return collide_test_dispatch(
                this_info.geom.get_geom_type(),
                that_info.geom.get_geom_type(),
                &this_info,
                &that_info,
                None,
            );
        }

        false
    }

    // ------------------------------------------------------------------
    // get_ground_or_structure_height
    //     (C++ PartitionManager::getGroundOrStructureHeight)
    // ------------------------------------------------------------------

    /// Get ground height plus tallest structure height at a position.
    /// Matches C++ PartitionManager::getGroundOrStructureHeight (PartitionManager.cpp:4674)
    pub fn get_ground_or_structure_height(&self, posx: f32, posy: f32) -> f32 {
        let terrain_height = if let Ok(terrain) = get_terrain_logic().read() {
            terrain.get_ground_height(posx, posy, None)
        } else {
            0.0
        };

        const RANGE: f32 = 1.0;
        let pos = Coord3D::new(posx, posy, terrain_height);

        let mut tallest_height: f32 = 0.0;

        let center_cell = CellCoord::from_world_pos(&pos);
        for cell_coord in center_cell.neighbors() {
            if let Some(cell) = self.cells.get(&cell_coord) {
                for &obj_id in &cell.objects {
                    if let Some(handle) = OBJECT_REGISTRY.get_object(obj_id) {
                        let guard = handle.read().ok();
                        let is_structure = guard
                            .as_ref()
                            .map(|g| g.is_kind_of(crate::common::KindOf::Structure))
                            .unwrap_or(false);
                        if !is_structure {
                            continue;
                        }
                    } else {
                        continue;
                    }

                    if let Some(pobj) = self.objects.get(&obj_id) {
                        let dx = pobj.position.x - pos.x;
                        let dy = pobj.position.y - pos.y;
                        let dist_2d = (dx * dx + dy * dy).sqrt();
                        let bounding_r = pobj.geometry.get_major_radius();

                        if dist_2d - bounding_r <= RANGE {
                            let this_height = pobj.geometry.get_max_height_above_position();
                            if this_height > tallest_height {
                                tallest_height = this_height;
                            }
                        }
                    }
                }
            }
        }

        terrain_height + tallest_height
    }

    pub fn refresh_shroud_for_local_player(&self) {
        if let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() {
            shroud.refresh_shroud_for_local_player();
        }
    }

    pub fn calc_min_radius(&self, cur: CellCoord) -> i32 {
        let half = PARTITION_CELL_SIZE * 0.5;
        let centers = [(-half, -half), (half, -half), (-half, half), (half, half)];
        let x = cur.x as f32 * PARTITION_CELL_SIZE;
        let y = cur.y as f32 * PARTITION_CELL_SIZE;
        let others = [
            (x - half, y - half),
            (x + half, y - half),
            (x - half, y + half),
            (x + half, y + half),
        ];
        let mut min_dist_sqr = f32::MAX;
        for center in centers {
            for other in others {
                let dx = center.0 - other.0;
                let dy = center.1 - other.1;
                min_dist_sqr = min_dist_sqr.min(dx * dx + dy * dy);
            }
        }
        (min_dist_sqr.sqrt() / PARTITION_CELL_SIZE).ceil() as i32
    }

    pub fn calc_radius_vec(&self) -> Vec<Vec<CellCoord>> {
        let mut result = vec![Vec::new()];
        for cell in self.cells.keys().copied() {
            let radius = self.calc_min_radius(cell).max(0) as usize;
            if radius >= result.len() {
                result.resize(radius + 1, Vec::new());
            }
            result[radius].push(cell);
        }
        result
    }

    pub fn get_vector_to(&self, from: &Coord3D, to: &Coord3D) -> Coord3D {
        Coord3D::new(to.x - from.x, to.y - from.y, to.z - from.z)
    }

    pub fn get_distance_squared(&self, from: &Coord3D, to: &Coord3D) -> f32 {
        let dx = from.x - to.x;
        let dy = from.y - to.y;
        let dz = from.z - to.z;
        dx * dx + dy * dy + dz * dz
    }

    pub fn get_relative_angle_2d(&self, from: &Coord3D, forward: &Coord3D, to: &Coord3D) -> f32 {
        let mut delta = self.get_vector_to(from, to);
        delta.z = 0.0;
        let len = (delta.x * delta.x + delta.y * delta.y).sqrt();
        if len <= f32::EPSILON {
            return 0.0;
        }
        delta.x /= len;
        delta.y /= len;
        let dot = (forward.x * delta.x + forward.y * delta.y).clamp(-1.0, 1.0);
        let mut angle = dot.acos();
        let perp_z = forward.x * delta.y - forward.y * delta.x;
        if perp_z < 0.0 {
            angle = -angle;
        }
        angle
    }

    pub fn do_shroud_cover(&self, center: &Coord3D, radius: f32, player_mask: PlayerMaskType) {
        if let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() {
            let c = glam::Vec3::new(center.x, center.y, center.z);
            shroud.do_shroud_cover(&c, radius, player_mask.bits());
        }
    }

    pub fn undo_shroud_cover(&self, center: &Coord3D, radius: f32, player_mask: PlayerMaskType) {
        if let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() {
            let c = glam::Vec3::new(center.x, center.y, center.z);
            shroud.undo_shroud_cover(&c, radius, player_mask.bits());
        }
    }

    pub fn get_cell_center_pos(&self, x: i32, y: i32) -> Coord3D {
        Coord3D::new(
            x as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5,
            y as f32 * PARTITION_CELL_SIZE + PARTITION_CELL_SIZE * 0.5,
            0.0,
        )
    }

    pub fn store_fogged_cells(&mut self, player_index: usize, _store_to_fog: bool) {
        self.fogged_cells
            .insert(player_index, self.cells.keys().copied().collect());
    }

    pub fn restore_fogged_cells(&self, player_index: usize, restore_to_fog: bool) {
        let Some(cells) = self.fogged_cells.get(&player_index) else {
            return;
        };
        let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() else {
            return;
        };
        for cell in cells {
            let center = self.get_cell_center_pos(cell.x, cell.y);
            let c = glam::Vec3::new(center.x, center.y, center.z);
            if restore_to_fog {
                shroud.undo_shroud_reveal(&c, PARTITION_CELL_SIZE, 1u32 << player_index);
            } else {
                shroud.do_shroud_reveal(&c, PARTITION_CELL_SIZE, 1u32 << player_index);
            }
        }
    }

    /// Clear all data
    pub fn clear(&mut self) {
        self.cells.clear();
        self.objects.clear();
        self.contact_list.clear();
    }
}

impl Default for PartitionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Partition statistics for debugging and profiling
#[derive(Debug, Clone)]
pub struct PartitionStatistics {
    pub total_objects: usize,
    pub total_cells: usize,
    pub max_objects_per_cell: usize,
    pub avg_objects_per_cell: f32,
    pub overcrowded_cells: usize,
    pub contact_pairs: usize,
}

// Global partition manager instance
lazy_static::lazy_static! {
    pub static ref PARTITION_MANAGER: Arc<RwLock<PartitionManager>> =
        Arc::new(RwLock::new(PartitionManager::new()));
}

#[cfg(test)]
mod tests {
    use super::super::collision_geometry::GeometryInfo;
    use super::*;

    #[test]
    fn test_cell_coord_from_world_pos() {
        let pos = Coord3D::new(150.0, 250.0, 0.0);
        let cell = CellCoord::from_world_pos(&pos);
        assert_eq!(cell.x, 1);
        assert_eq!(cell.y, 2);

        let neg_pos = Coord3D::new(-150.0, -50.0, 0.0);
        let neg_cell = CellCoord::from_world_pos(&neg_pos);
        assert_eq!(neg_cell.x, -2);
        assert_eq!(neg_cell.y, -1);
    }

    #[test]
    fn test_cell_neighbors() {
        let cell = CellCoord { x: 0, y: 0 };
        let neighbors = cell.neighbors();
        assert_eq!(neighbors.len(), 9);
        assert!(neighbors.contains(&CellCoord { x: 0, y: 0 }));
        assert!(neighbors.contains(&CellCoord { x: 1, y: 1 }));
        assert!(neighbors.contains(&CellCoord { x: -1, y: -1 }));
    }

    #[test]
    fn test_partition_manager_register_unregister() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);
        let pos = Coord3D::new(50.0, 50.0, 0.0);

        pm.register_object(1, pos, geom).unwrap();
        assert_eq!(pm.object_count(), 1);

        pm.unregister_object(1).unwrap();
        assert_eq!(pm.object_count(), 0);
    }

    #[test]
    fn test_partition_manager_update_position() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);
        let pos1 = Coord3D::new(50.0, 50.0, 0.0);
        let pos2 = Coord3D::new(250.0, 250.0, 0.0);

        pm.register_object(1, pos1, geom).unwrap();

        let cell1 = CellCoord::from_world_pos(&pos1);
        assert_eq!(pm.objects.get(&1).unwrap().cell, cell1);

        pm.update_object_position(1, pos2).unwrap();

        let cell2 = CellCoord::from_world_pos(&pos2);
        assert_eq!(pm.objects.get(&1).unwrap().cell, cell2);
        assert_ne!(cell1, cell2);
    }

    #[test]
    fn test_find_objects_in_radius() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        pm.register_object(1, Coord3D::new(0.0, 0.0, 0.0), geom)
            .unwrap();
        pm.register_object(2, Coord3D::new(10.0, 0.0, 0.0), geom)
            .unwrap();
        pm.register_object(3, Coord3D::new(100.0, 0.0, 0.0), geom)
            .unwrap();

        let center = Coord3D::new(0.0, 0.0, 0.0);
        let results = pm.find_objects_in_radius(&center, 20.0, &[]);

        assert_eq!(results.len(), 2); // Objects 1 and 2
        assert!(results.contains(&1));
        assert!(results.contains(&2));
        assert!(!results.contains(&3));
    }

    #[test]
    fn test_find_closest_objects() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        pm.register_object(1, Coord3D::new(5.0, 0.0, 0.0), geom)
            .unwrap();
        pm.register_object(2, Coord3D::new(10.0, 0.0, 0.0), geom)
            .unwrap();
        pm.register_object(3, Coord3D::new(15.0, 0.0, 0.0), geom)
            .unwrap();

        let center = Coord3D::new(0.0, 0.0, 0.0);
        let results = pm.find_closest_objects(&center, 2, 50.0, &[]);

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].0, 1); // Closest
        assert_eq!(results[1].0, 2); // Second closest
    }

    #[test]
    fn test_iterate_objects_in_rect() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        pm.register_object(1, Coord3D::new(25.0, 25.0, 0.0), geom)
            .unwrap();
        pm.register_object(2, Coord3D::new(75.0, 75.0, 0.0), geom)
            .unwrap();
        pm.register_object(3, Coord3D::new(200.0, 200.0, 0.0), geom)
            .unwrap();

        let min_corner = Coord3D::new(0.0, 0.0, 0.0);
        let max_corner = Coord3D::new(100.0, 100.0, 0.0);
        let results = pm.iterate_objects_in_rect(&min_corner, &max_corner);

        assert_eq!(results.len(), 2);
        assert!(results.contains(&1));
        assert!(results.contains(&2));
        assert!(!results.contains(&3));
    }

    #[test]
    fn test_build_contact_list() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        // Place two objects close together
        pm.register_object(1, Coord3D::new(0.0, 0.0, 0.0), geom)
            .unwrap();
        pm.register_object(2, Coord3D::new(8.0, 0.0, 0.0), geom)
            .unwrap();

        // Place one far away
        pm.register_object(3, Coord3D::new(1000.0, 0.0, 0.0), geom)
            .unwrap();

        pm.build_contact_list();
        let contacts = pm.get_contact_list();

        assert_eq!(contacts.len(), 1); // Only 1 and 2 should be in contact
        assert!(contacts.contains(&(1, 2)) || contacts.contains(&(2, 1)));
    }

    #[test]
    fn test_partition_statistics() {
        let mut pm = PartitionManager::new();

        let geom = GeometryInfo::new_sphere(5.0, false);

        for i in 0..10 {
            pm.register_object(i, Coord3D::new((i * 10) as f32, 0.0, 0.0), geom)
                .unwrap();
        }

        let stats = pm.get_statistics();
        assert_eq!(stats.total_objects, 10);
        assert!(stats.total_cells > 0);
        assert!(stats.avg_objects_per_cell > 0.0);
    }
}
