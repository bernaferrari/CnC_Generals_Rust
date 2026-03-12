//! Partition Manager for Spatial Queries
//!
//! This module provides spatial partitioning for efficient collision detection
//! and object queries. Objects are organized into cells for fast proximity testing.
//!
//! Matches C++ PartitionManager.cpp spatial partitioning system

use super::collision_geometry::{CollideInfo, GeometryInfo};
use super::{CollisionError, Coord3D, GameObject, ObjectId};
use crate::object::registry::OBJECT_REGISTRY;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, RwLock};

/// Size of each partition cell in world units
/// Matches C++ PARTITION_CELL_SIZE
const PARTITION_CELL_SIZE: f32 = 100.0;

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

/// Partition cell containing objects
#[derive(Debug)]
struct PartitionCell {
    objects: HashSet<ObjectId>,
    dirty: bool,
}

impl PartitionCell {
    fn new() -> Self {
        Self {
            objects: HashSet::new(),
            dirty: false,
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

    fn contains(&self, id: ObjectId) -> bool {
        self.objects.contains(&id)
    }

    fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    fn len(&self) -> usize {
        self.objects.len()
    }
}

/// Object registration in partition system
#[derive(Debug, Clone)]
struct PartitionObject {
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
}

impl PartitionManager {
    pub fn new() -> Self {
        Self {
            cells: HashMap::new(),
            objects: HashMap::new(),
            contact_list: Vec::new(),
        }
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

/// Global partition manager instance
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
