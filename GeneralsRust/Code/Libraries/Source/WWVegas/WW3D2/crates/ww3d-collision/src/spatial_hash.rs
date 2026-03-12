//! Spatial Hashing for Collision Detection
//!
//! This module provides spatial hashing for efficient broad-phase collision detection

use crate::bounding_volumes::AABox;
use glam::Vec3;
use std::collections::HashMap;

/// 3D spatial hash grid for efficient collision detection
#[derive(Debug)]
pub struct SpatialHashGrid {
    pub cell_size: f32,
    pub grid: HashMap<(i32, i32, i32), Vec<usize>>,
    pub object_bounds: HashMap<usize, AABox>,
}

impl SpatialHashGrid {
    pub fn new(cell_size: f32) -> Self {
        Self {
            cell_size,
            grid: HashMap::new(),
            object_bounds: HashMap::new(),
        }
    }

    /// Insert an object with given AABB
    pub fn insert(&mut self, object_id: usize, aabb: AABox) {
        self.object_bounds.insert(object_id, aabb);

        let min_cell = self.world_to_cell(aabb.center - aabb.extent);
        let max_cell = self.world_to_cell(aabb.center + aabb.extent);

        for x in min_cell.0..=max_cell.0 {
            for y in min_cell.1..=max_cell.1 {
                for z in min_cell.2..=max_cell.2 {
                    self.grid
                        .entry((x, y, z))
                        .or_insert_with(Vec::new)
                        .push(object_id);
                }
            }
        }
    }

    /// Remove an object from the grid
    pub fn remove(&mut self, object_id: usize) {
        if let Some(aabb) = self.object_bounds.remove(&object_id) {
            let min_cell = self.world_to_cell(aabb.center - aabb.extent);
            let max_cell = self.world_to_cell(aabb.center + aabb.extent);

            for x in min_cell.0..=max_cell.0 {
                for y in min_cell.1..=max_cell.1 {
                    for z in min_cell.2..=max_cell.2 {
                        if let Some(cell) = self.grid.get_mut(&(x, y, z)) {
                            cell.retain(|&id| id != object_id);
                            if cell.is_empty() {
                                self.grid.remove(&(x, y, z));
                            }
                        }
                    }
                }
            }
        }
    }

    /// Update an object's position
    pub fn update(&mut self, object_id: usize, new_aabb: AABox) {
        self.remove(object_id);
        self.insert(object_id, new_aabb);
    }

    /// Query objects that might intersect with the given AABB
    pub fn query(&self, query_aabb: &AABox) -> Vec<usize> {
        // Pre-allocate with estimate: typical queries return 10-50 candidates
        // Prevents repeated allocations during broad-phase collision detection
        let mut candidates = Vec::with_capacity(32);
        let min_cell = self.world_to_cell(query_aabb.center - query_aabb.extent);
        let max_cell = self.world_to_cell(query_aabb.center + query_aabb.extent);

        for x in min_cell.0..=max_cell.0 {
            for y in min_cell.1..=max_cell.1 {
                for z in min_cell.2..=max_cell.2 {
                    if let Some(cell) = self.grid.get(&(x, y, z)) {
                        for &object_id in cell {
                            if !candidates.contains(&object_id) {
                                candidates.push(object_id);
                            }
                        }
                    }
                }
            }
        }

        candidates
    }

    /// Get all potential collision pairs (broad phase)
    pub fn get_collision_pairs(&self) -> Vec<(usize, usize)> {
        // Pre-allocate with estimate: N objects typically generate O(N) collision pairs
        // Assumes typical game with ~256 dynamic objects → ~256-512 pairs
        let estimated_pairs = self.object_bounds.len().saturating_mul(2);
        let mut pairs = Vec::with_capacity(estimated_pairs);
        let mut seen_pairs = std::collections::HashSet::new();

        for cell in self.grid.values() {
            for i in 0..cell.len() {
                for j in (i + 1)..cell.len() {
                    let pair = if cell[i] < cell[j] {
                        (cell[i], cell[j])
                    } else {
                        (cell[j], cell[i])
                    };

                    if seen_pairs.insert(pair) {
                        pairs.push(pair);
                    }
                }
            }
        }

        pairs
    }

    /// Convert world position to cell coordinates
    fn world_to_cell(&self, position: Vec3) -> (i32, i32, i32) {
        (
            (position.x / self.cell_size).floor() as i32,
            (position.y / self.cell_size).floor() as i32,
            (position.z / self.cell_size).floor() as i32,
        )
    }

    /// Clear the grid
    pub fn clear(&mut self) {
        self.grid.clear();
        self.object_bounds.clear();
    }

    /// Get statistics about the grid
    pub fn get_stats(&self) -> SpatialHashStats {
        let total_cells = self.grid.len();
        let total_objects = self.object_bounds.len();
        let mut max_objects_per_cell = 0;
        let mut total_cell_occupancy = 0;

        for cell in self.grid.values() {
            max_objects_per_cell = max_objects_per_cell.max(cell.len());
            total_cell_occupancy += cell.len();
        }

        let avg_objects_per_cell = if total_cells > 0 {
            total_cell_occupancy as f32 / total_cells as f32
        } else {
            0.0
        };

        SpatialHashStats {
            total_cells,
            total_objects,
            max_objects_per_cell,
            avg_objects_per_cell,
            cell_size: self.cell_size,
        }
    }
}

/// Statistics for spatial hash grid
#[derive(Debug)]
pub struct SpatialHashStats {
    pub total_cells: usize,
    pub total_objects: usize,
    pub max_objects_per_cell: usize,
    pub avg_objects_per_cell: f32,
    pub cell_size: f32,
}

/// Broad-phase collision detection system
#[derive(Debug)]
pub struct BroadPhase {
    pub spatial_hash: SpatialHashGrid,
    pub collision_pairs: Vec<(usize, usize)>,
}

impl BroadPhase {
    pub fn new(cell_size: f32) -> Self {
        Self {
            spatial_hash: SpatialHashGrid::new(cell_size),
            collision_pairs: Vec::new(),
        }
    }

    /// Update the broad phase with new object positions
    pub fn update(&mut self, objects: &[(usize, AABox)]) {
        self.spatial_hash.clear();

        for &(object_id, aabb) in objects {
            self.spatial_hash.insert(object_id, aabb);
        }

        // Get potential collision pairs from spatial hash and filter with AABB intersection
        let potential_pairs = self.spatial_hash.get_collision_pairs();
        self.collision_pairs.clear();

        for &(id1, id2) in &potential_pairs {
            if let (Some(&aabb1), Some(&aabb2)) = (
                self.spatial_hash.object_bounds.get(&id1),
                self.spatial_hash.object_bounds.get(&id2),
            ) {
                if NarrowPhase::aabb_intersect(&aabb1, &aabb2) {
                    self.collision_pairs.push((id1, id2));
                }
            }
        }
    }

    /// Get collision pairs from the last update
    pub fn get_collision_pairs(&self) -> &[(usize, usize)] {
        &self.collision_pairs
    }

    /// Query objects near a specific AABB
    pub fn query_near(&self, query_aabb: &AABox) -> Vec<usize> {
        self.spatial_hash.query(query_aabb)
    }
}

/// Narrow-phase collision detection
pub struct NarrowPhase;

impl NarrowPhase {
    /// Perform exact collision tests on potential pairs
    pub fn test_collision_pairs(
        pairs: &[(usize, usize)],
        get_aabb: impl Fn(usize) -> Option<AABox>,
    ) -> Vec<(usize, usize)> {
        let mut actual_collisions = Vec::new();

        for &(id1, id2) in pairs {
            if let (Some(aabb1), Some(aabb2)) = (get_aabb(id1), get_aabb(id2)) {
                if Self::aabb_intersect(&aabb1, &aabb2) {
                    actual_collisions.push((id1, id2));
                }
            }
        }

        actual_collisions
    }

    /// AABB intersection test
    fn aabb_intersect(a: &AABox, b: &AABox) -> bool {
        let a_min = a.center - a.extent;
        let a_max = a.center + a.extent;
        let b_min = b.center - b.extent;
        let b_max = b.center + b.extent;

        a_min.x <= b_max.x
            && a_max.x >= b_min.x
            && a_min.y <= b_max.y
            && a_max.y >= b_min.y
            && a_min.z <= b_max.z
            && a_max.z >= b_min.z
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_spatial_hash_insertion() {
        let mut grid = SpatialHashGrid::new(10.0);

        let aabb = AABox {
            center: Vec3::new(5.0, 5.0, 5.0),
            extent: Vec3::new(2.0, 2.0, 2.0),
        };

        grid.insert(1, aabb);

        let candidates = grid.query(&aabb);
        assert!(candidates.contains(&1));
    }

    #[test]
    fn test_broad_phase() {
        let mut broad_phase = BroadPhase::new(10.0);

        let objects = vec![
            (
                1,
                AABox {
                    center: Vec3::new(0.0, 0.0, 0.0),
                    extent: Vec3::new(1.0, 1.0, 1.0),
                },
            ),
            (
                2,
                AABox {
                    center: Vec3::new(1.5, 0.0, 0.0),
                    extent: Vec3::new(1.0, 1.0, 1.0),
                },
            ),
            (
                3,
                AABox {
                    center: Vec3::new(10.0, 10.0, 10.0),
                    extent: Vec3::new(1.0, 1.0, 1.0),
                },
            ),
        ];

        broad_phase.update(&objects);

        let pairs = broad_phase.get_collision_pairs();
        assert!(pairs.contains(&(1, 2)) || pairs.contains(&(2, 1)));
        assert!(!pairs.contains(&(1, 3)) && !pairs.contains(&(3, 1)));
    }

    #[test]
    fn test_narrow_phase() {
        let aabb1 = AABox {
            center: Vec3::new(0.0, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        let aabb2 = AABox {
            center: Vec3::new(1.5, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        let aabb3 = AABox {
            center: Vec3::new(10.0, 10.0, 10.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };

        assert!(NarrowPhase::aabb_intersect(&aabb1, &aabb2));
        assert!(!NarrowPhase::aabb_intersect(&aabb1, &aabb3));
    }
}
