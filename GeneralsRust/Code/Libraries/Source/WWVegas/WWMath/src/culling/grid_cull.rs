// Command & Conquer Generals Zero Hour
// Copyright 2025 Electronic Arts Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Grid-based culling system implementation.
//!
//! This culling system is designed for dynamic objects (objects which are moving or
//! changing bounding box size). It features O(1) insertion as opposed to
//! the AAB-tree insertion times which are O(logn). Its disadvantages
//! compared to tree-based systems are that it must uniformly divide space.

use super::{CollisionMath, CullCollection, CullStats, CullSystem, Cullable, OverlapType};
use crate::{AABox, Frustum, Vector3};
use std::collections::HashMap;
use std::sync::Arc;

const TERMINATION_CELL_COUNT: usize = 16384;
const UNGRIDDED_ADDRESS: u32 = 0xFFFFFFFF;

/// Volume structure defining a range of grid cells
#[derive(Debug, Clone, Copy)]
struct GridVolume {
    /// Minimum cell indices [x, y, z]
    min: [i32; 3],
    /// Maximum cell indices [x, y, z] (exclusive)
    max: [i32; 3],
}

impl GridVolume {
    fn new() -> Self {
        Self {
            min: [0; 3],
            max: [0; 3],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn new_with_bounds(i0: i32, j0: i32, k0: i32, i1: i32, j1: i32, k1: i32) -> Self {
        Self {
            min: [i0, j0, k0],
            max: [i1, j1, k1],
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn is_leaf(&self) -> bool {
        (self.max[0] - self.min[0] == 1)
            && (self.max[1] - self.min[1] == 1)
            && (self.max[2] - self.min[2] == 1)
    }

    fn is_empty(&self) -> bool {
        (self.max[0] - self.min[0] <= 0)
            || (self.max[1] - self.min[1] <= 0)
            || (self.max[2] - self.min[2] <= 0)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn split(&self) -> (GridVolume, GridVolume) {
        // Find the longest dimension
        let delta = [
            self.max[0] - self.min[0],
            self.max[1] - self.min[1],
            self.max[2] - self.min[2],
        ];

        let mut split_axis = 0;
        if delta[1] > delta[split_axis] {
            split_axis = 1;
        }
        if delta[2] > delta[split_axis] {
            split_axis = 2;
        }

        // Split along that dimension
        let mut v0 = *self;
        let mut v1 = *self;

        let split_point = self.min[split_axis] + (delta[split_axis] >> 1);
        v0.max[split_axis] = split_point;
        v1.min[split_axis] = split_point;

        (v0, v1)
    }
}

/// Link information for objects in the grid culling system
struct GridLink {
    /// Address in the grid
    grid_address: u32,
}

/// Grid-based culling system for dynamic objects
pub struct GridCullSystem {
    // Grid parameters
    min_cell_size: Vector3,
    max_obj_extent: f32,
    termination_cell_count: usize,

    // Grid structure
    origin: Vector3,
    cell_dim: Vector3,
    oo_cell_dim: Vector3, // One over cell dimensions (for fast division)
    cell_count: [usize; 3],

    // Storage
    cells: Vec<Vec<Arc<dyn Cullable>>>,
    no_grid_list: Vec<Arc<dyn Cullable>>,

    // Object management
    object_count: usize,
    object_links: HashMap<u64, GridLink>,

    // Collection and statistics
    collection: CullCollection,
    stats: CullStats,
}

impl GridCullSystem {
    /// Create a new grid culling system
    pub fn new() -> Self {
        let mut system = Self {
            min_cell_size: Vector3::new(10.0, 10.0, 10.0),
            max_obj_extent: 15.0,
            termination_cell_count: TERMINATION_CELL_COUNT,
            origin: Vector3::new(-100.0, -100.0, -100.0),
            cell_dim: Vector3::new(10.0, 10.0, 10.0),
            oo_cell_dim: Vector3::new(0.1, 0.1, 0.1),
            cell_count: [0; 3],
            cells: Vec::new(),
            no_grid_list: Vec::new(),
            object_count: 0,
            object_links: HashMap::new(),
            collection: CullCollection::new(),
            stats: CullStats::new(),
        };

        system.re_partition(
            Vector3::new(-100.0, -100.0, -100.0),
            Vector3::new(100.0, 100.0, 100.0),
            15.0,
        );

        system.reset_statistics();
        system
    }

    /// Re-partition the grid for the given volume and maximum object dimension
    pub fn re_partition(&mut self, mut input_min: Vector3, mut input_max: Vector3, obj_dim: f32) {
        // Collect and unlink all objects
        self.reset_collection();
        self.collect_and_unlink_all();

        // Sanity check input parameters
        if input_max.x - input_min.x < 1.0 {
            input_max.x += self.min_cell_size.x;
            input_min.x -= self.min_cell_size.x;
        }
        if input_max.y - input_min.y < 1.0 {
            input_max.y += self.min_cell_size.y;
            input_min.y -= self.min_cell_size.y;
        }
        if input_max.z - input_min.z < 1.0 {
            input_max.z += self.min_cell_size.z;
            input_min.z -= self.min_cell_size.z;
        }

        // Compute grid parameters
        self.origin = input_min;
        let world_dim = input_max - input_min;
        self.max_obj_extent = obj_dim;

        // Determine cell count for each dimension
        self.cell_count = [1, 1, 1];
        self.cell_dim = world_dim;

        let mut done = false;
        while !done {
            // Find biggest dimension relative to minimum cell size
            let mut big_dim = 0;
            if self.cell_dim.y / self.min_cell_size.y
                > self.cell_dim[big_dim] / self.min_cell_size[big_dim]
            {
                big_dim = 1;
            }
            if self.cell_dim.z / self.min_cell_size.z
                > self.cell_dim[big_dim] / self.min_cell_size[big_dim]
            {
                big_dim = 2;
            }

            // Split dimension in two if possible
            if self.cell_dim[big_dim] >= 2.0 * self.min_cell_size[big_dim] {
                self.cell_dim[big_dim] /= 2.0;
                self.cell_count[big_dim] *= 2;
            }

            // Check termination conditions
            if self.total_cell_count() >= self.termination_cell_count {
                done = true;
            }

            if self.cell_dim.x < 2.0 * self.min_cell_size.x
                && self.cell_dim.y < 2.0 * self.min_cell_size.y
                && self.cell_dim.z < 2.0 * self.min_cell_size.z
            {
                done = true;
            }
        }

        self.oo_cell_dim = Vector3::new(
            1.0 / self.cell_dim.x,
            1.0 / self.cell_dim.y,
            1.0 / self.cell_dim.z,
        );

        // Allocate cell storage
        let total_cells = self.total_cell_count();
        self.cells = vec![Vec::new(); total_cells];

        // Re-insert all collected objects
        let objects_to_reinsert: Vec<_> = self.collection.iter().cloned().collect();
        for obj in objects_to_reinsert {
            self.link_object(obj.clone());
        }

        self.reset_statistics();
    }

    /// Get minimum cell size
    pub fn get_min_cell_size(&self) -> Vector3 {
        self.min_cell_size
    }

    /// Set minimum cell size
    pub fn set_min_cell_size(&mut self, size: Vector3) {
        self.min_cell_size = size;
    }

    /// Get termination count
    pub fn get_termination_count(&self) -> usize {
        self.termination_cell_count
    }

    /// Set termination count
    pub fn set_termination_count(&mut self, count: usize) {
        self.termination_cell_count = count;
    }

    fn total_cell_count(&self) -> usize {
        self.cell_count[0] * self.cell_count[1] * self.cell_count[2]
    }

    fn map_point_to_cell(&self, pt: Vector3) -> Option<(i32, i32, i32)> {
        let dp = pt - self.origin;
        let i = (dp.x * self.oo_cell_dim.x).floor() as i32;
        let j = (dp.y * self.oo_cell_dim.y).floor() as i32;
        let k = (dp.z * self.oo_cell_dim.z).floor() as i32;

        if i >= 0
            && j >= 0
            && k >= 0
            && i < self.cell_count[0] as i32
            && j < self.cell_count[1] as i32
            && k < self.cell_count[2] as i32
        {
            Some((i, j, k))
        } else {
            None
        }
    }

    fn map_point_to_address(&self, pt: Vector3) -> u32 {
        if let Some((i, j, k)) = self.map_point_to_cell(pt) {
            self.map_indices_to_address(i, j, k)
        } else {
            UNGRIDDED_ADDRESS
        }
    }

    fn map_indices_to_address(&self, i: i32, j: i32, k: i32) -> u32 {
        (i + j * self.cell_count[0] as i32 + k * (self.cell_count[0] * self.cell_count[1]) as i32)
            as u32
    }

    fn clamp_indices_to_grid(&self, i: &mut i32, j: &mut i32, k: &mut i32) {
        *i = (*i).max(0).min(self.cell_count[0] as i32 - 1);
        *j = (*j).max(0).min(self.cell_count[1] as i32 - 1);
        *k = (*k).max(0).min(self.cell_count[2] as i32 - 1);
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn compute_box_for_cell(&self, i: i32, j: i32, k: i32) -> AABox {
        let min = Vector3::new(
            self.origin.x + i as f32 * self.cell_dim.x - self.max_obj_extent,
            self.origin.y + j as f32 * self.cell_dim.y - self.max_obj_extent,
            self.origin.z + k as f32 * self.cell_dim.z - self.max_obj_extent,
        );

        let max = Vector3::new(
            min.x + self.cell_dim.x + 2.0 * self.max_obj_extent,
            min.y + self.cell_dim.y + 2.0 * self.max_obj_extent,
            min.z + self.cell_dim.z + 2.0 * self.max_obj_extent,
        );

        AABox::from_min_max(min, max)
    }

    #[cfg_attr(not(test), allow(dead_code))]
    fn compute_box_for_volume(&self, vol: &GridVolume) -> AABox {
        let min = Vector3::new(
            self.origin.x + vol.min[0] as f32 * self.cell_dim.x - self.max_obj_extent,
            self.origin.y + vol.min[1] as f32 * self.cell_dim.y - self.max_obj_extent,
            self.origin.z + vol.min[2] as f32 * self.cell_dim.z - self.max_obj_extent,
        );

        let max = Vector3::new(
            self.origin.x + vol.max[0] as f32 * self.cell_dim.x + self.max_obj_extent,
            self.origin.y + vol.max[1] as f32 * self.cell_dim.y + self.max_obj_extent,
            self.origin.z + vol.max[2] as f32 * self.cell_dim.z + self.max_obj_extent,
        );

        AABox::from_min_max(min, max)
    }

    fn init_volume_from_bounds(&self, bound_min: Vector3, bound_max: Vector3) -> GridVolume {
        // Expand the box by the maximum size of any object
        let grid_min = bound_min
            - Vector3::new(
                self.max_obj_extent,
                self.max_obj_extent,
                self.max_obj_extent,
            );
        let grid_max = bound_max
            + Vector3::new(
                self.max_obj_extent,
                self.max_obj_extent,
                self.max_obj_extent,
            );

        // Compute grid coordinates
        let mut vol = GridVolume::new();
        if let Some((i_min, j_min, k_min)) = self.map_point_to_cell(grid_min) {
            vol.min = [i_min, j_min, k_min];
        }
        if let Some((i_max, j_max, k_max)) = self.map_point_to_cell(grid_max) {
            vol.max = [i_max, j_max, k_max];
        }

        // Clamp to grid and increment max for traversal
        let mut min_i = vol.min[0];
        let mut min_j = vol.min[1];
        let mut min_k = vol.min[2];
        self.clamp_indices_to_grid(&mut min_i, &mut min_j, &mut min_k);
        vol.min = [min_i, min_j, min_k];

        let mut max_i = vol.max[0];
        let mut max_j = vol.max[1];
        let mut max_k = vol.max[2];
        self.clamp_indices_to_grid(&mut max_i, &mut max_j, &mut max_k);
        vol.max = [max_i, max_j, max_k];

        vol.max[0] += 1;
        vol.max[1] += 1;
        vol.max[2] += 1;

        vol
    }

    fn init_volume_from_box(&self, box_: &AABox) -> GridVolume {
        self.init_volume_from_bounds(box_.min_corner(), box_.max_corner())
    }

    fn init_volume_from_frustum(&self, frustum: &Frustum) -> GridVolume {
        self.init_volume_from_bounds(frustum.get_bound_min(), frustum.get_bound_max())
    }

    fn link_object(&mut self, obj: Arc<dyn Cullable>) {
        let address = self.map_point_to_address(obj.get_cull_box().center);
        self.link_object_with_address(obj, address);
    }

    fn link_object_with_address(&mut self, obj: Arc<dyn Cullable>, address: u32) {
        let obj_id = obj.get_id();
        let obj_box = obj.get_cull_box();

        // Check if object fits in grid
        if obj_box.extent.x > self.max_obj_extent
            || obj_box.extent.y > self.max_obj_extent
            || obj_box.extent.z > self.max_obj_extent
            || address == UNGRIDDED_ADDRESS
        {
            // Add to no-grid list
            self.object_links.insert(
                obj_id,
                GridLink {
                    grid_address: UNGRIDDED_ADDRESS,
                },
            );
            self.no_grid_list.push(obj);
        } else {
            // Add to grid cell
            self.object_links.insert(
                obj_id,
                GridLink {
                    grid_address: address,
                },
            );
            self.cells[address as usize].push(obj);
        }
    }

    fn unlink_object(&mut self, obj: &Arc<dyn Cullable>) {
        let obj_id = obj.get_id();
        if let Some(link) = self.object_links.remove(&obj_id) {
            if link.grid_address == UNGRIDDED_ADDRESS {
                // Remove from no-grid list
                self.no_grid_list.retain(|o| o.get_id() != obj_id);
            } else {
                // Remove from grid cell
                self.cells[link.grid_address as usize].retain(|o| o.get_id() != obj_id);
            }
        }
    }

    fn collect_and_unlink_all(&mut self) {
        self.collection.clear();

        // Collect from grid cells
        for cell in &mut self.cells {
            for obj in cell.drain(..) {
                self.collection.add(obj);
            }
        }

        // Collect from no-grid list
        for obj in self.no_grid_list.drain(..) {
            self.collection.add(obj);
        }

        // Clear links
        self.object_links.clear();
    }

    fn collect_objects_in_cell_point(&mut self, point: Vector3, objects: &[Arc<dyn Cullable>]) {
        let mut objects_to_add = Vec::new();
        for obj in objects {
            if obj.get_cull_box().contains_point(point) {
                objects_to_add.push(obj.clone());
            }
        }
        for obj in objects_to_add {
            self.collection.add(obj);
        }
    }

    fn collect_objects_in_cell_box(&mut self, box_: &AABox, objects: &[Arc<dyn Cullable>]) {
        let mut objects_to_add = Vec::new();
        for obj in objects {
            if CollisionMath::overlap_test_box_box(box_, &obj.get_cull_box())
                != OverlapType::Outside
            {
                objects_to_add.push(obj.clone());
            }
        }
        for obj in objects_to_add {
            self.collection.add(obj);
        }
    }

    fn collect_objects_in_cell_frustum(
        &mut self,
        frustum: &Frustum,
        objects: &[Arc<dyn Cullable>],
    ) {
        let mut objects_to_add = Vec::new();
        for obj in objects {
            if CollisionMath::overlap_test_frustum_box(frustum, &obj.get_cull_box())
                != OverlapType::Outside
            {
                objects_to_add.push(obj.clone());
            }
        }
        for obj in objects_to_add {
            self.collection.add(obj);
        }
    }

    fn reset_statistics(&mut self) {
        self.stats = CullStats::new();
        self.stats.node_count = 2 * self.total_cell_count() - 1;
    }
}

impl Default for GridCullSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CullSystem for GridCullSystem {
    fn reset_collection(&mut self) {
        self.collection.clear();
    }

    fn collect_objects_point(&mut self, point: Vector3) {
        let vol = self.init_volume_from_bounds(point, point);
        if !vol.is_empty() {
            // Collect all cell references first to avoid borrowing conflicts
            let mut cells_to_check = Vec::new();
            for k in vol.min[2]..vol.max[2] {
                for j in vol.min[1]..vol.max[1] {
                    let mut address = self.map_indices_to_address(vol.min[0], j, k);
                    for _ in vol.min[0]..vol.max[0] {
                        self.stats.nodes_trivially_accepted += 1;
                        cells_to_check.push(self.cells[address as usize].clone());
                        address += 1;
                    }
                }
            }

            // Now process all collected cells
            for cell in cells_to_check {
                self.collect_objects_in_cell_point(point, &cell);
            }
        }

        // Check no-grid list
        let no_grid_list = self.no_grid_list.clone();
        self.collect_objects_in_cell_point(point, &no_grid_list);
    }

    fn collect_objects_box(&mut self, box_: &AABox) {
        let vol = self.init_volume_from_box(box_);
        if !vol.is_empty() {
            // Collect all cell references first to avoid borrowing conflicts
            let mut cells_to_check = Vec::new();
            for k in vol.min[2]..vol.max[2] {
                for j in vol.min[1]..vol.max[1] {
                    let mut address = self.map_indices_to_address(vol.min[0], j, k);
                    for _ in vol.min[0]..vol.max[0] {
                        self.stats.nodes_trivially_accepted += 1;
                        cells_to_check.push(self.cells[address as usize].clone());
                        address += 1;
                    }
                }
            }

            // Now process all collected cells
            for cell in cells_to_check {
                self.collect_objects_in_cell_box(box_, &cell);
            }
        }

        // Check no-grid list
        let no_grid_list = self.no_grid_list.clone();
        self.collect_objects_in_cell_box(box_, &no_grid_list);
    }

    fn collect_objects_frustum(&mut self, frustum: &Frustum) {
        let vol = self.init_volume_from_frustum(frustum);
        if !vol.is_empty() {
            // Collect all cell references first to avoid borrowing conflicts
            let mut cells_to_check = Vec::new();
            for k in vol.min[2]..vol.max[2] {
                for j in vol.min[1]..vol.max[1] {
                    let mut address = self.map_indices_to_address(vol.min[0], j, k);
                    for _ in vol.min[0]..vol.max[0] {
                        self.stats.nodes_trivially_accepted += 1;
                        cells_to_check.push(self.cells[address as usize].clone());
                        address += 1;
                    }
                }
            }

            // Now process all collected cells
            for cell in cells_to_check {
                self.collect_objects_in_cell_frustum(frustum, &cell);
            }
        }

        // Check no-grid list
        let no_grid_list = self.no_grid_list.clone();
        self.collect_objects_in_cell_frustum(frustum, &no_grid_list);
    }

    fn update_culling(&mut self, object: &Arc<dyn Cullable>) {
        let obj_id = object.get_id();
        let new_address = self.map_point_to_address(object.get_cull_box().center);

        if let Some(link) = self.object_links.get(&obj_id) {
            if link.grid_address != new_address {
                // Object moved to different cell, need to relocate
                self.unlink_object(object);
                self.link_object_with_address(object.clone(), new_address);
            }
        }
    }

    fn add_object(&mut self, object: Arc<dyn Cullable>) {
        self.link_object(object);
        self.object_count += 1;
    }

    fn remove_object(&mut self, object: &Arc<dyn Cullable>) {
        self.unlink_object(object);
        self.object_count -= 1;
    }

    fn get_collection(&mut self) -> &mut CullCollection {
        &mut self.collection
    }

    fn get_stats(&self) -> CullStats {
        self.stats
    }

    fn reset_stats(&mut self) {
        self.reset_statistics();
    }

    fn get_object_count(&self) -> usize {
        self.object_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug)]
    struct TestObject {
        id: u64,
        bbox: AABox,
    }

    impl TestObject {
        fn new(id: u64, center: Vector3, extent: Vector3) -> Self {
            Self {
                id,
                bbox: AABox::new(center, extent),
            }
        }
    }

    impl Cullable for TestObject {
        fn get_cull_box(&self) -> AABox {
            self.bbox
        }

        fn set_cull_box(&mut self, box_: AABox, _just_loaded: bool) {
            self.bbox = box_;
        }

        fn get_id(&self) -> u64 {
            self.id
        }
    }

    #[test]
    fn test_grid_volume() {
        let vol = GridVolume::new_with_bounds(0, 0, 0, 2, 2, 2);
        assert!(!vol.is_leaf());
        assert!(!vol.is_empty());

        let leaf = GridVolume::new_with_bounds(0, 0, 0, 1, 1, 1);
        assert!(leaf.is_leaf());

        let empty = GridVolume::new_with_bounds(0, 0, 0, 0, 0, 0);
        assert!(empty.is_empty());

        let (v0, v1) = vol.split();
        assert!(v0.max[0] <= v1.min[0] || v0.max[1] <= v1.min[1] || v0.max[2] <= v1.min[2]);
    }

    #[test]
    fn test_grid_cull_system_creation() {
        let grid = GridCullSystem::new();
        assert_eq!(grid.get_object_count(), 0);
        assert!(grid.total_cell_count() > 0);
    }

    #[test]
    fn test_grid_point_mapping() {
        let grid = GridCullSystem::new();

        // Test point inside grid
        let center_point = Vector3::ZERO;
        if let Some((i, j, k)) = grid.map_point_to_cell(center_point) {
            assert!(i >= 0 && j >= 0 && k >= 0);
            assert!(i < grid.cell_count[0] as i32);
            assert!(j < grid.cell_count[1] as i32);
            assert!(k < grid.cell_count[2] as i32);
        }

        // Test point outside grid
        let far_point = Vector3::new(1000.0, 1000.0, 1000.0);
        assert!(grid.map_point_to_cell(far_point).is_none());
    }

    #[test]
    fn test_grid_add_remove_objects() {
        let mut grid = GridCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        grid.add_object(obj1.clone());
        grid.add_object(obj2.clone());

        assert_eq!(grid.get_object_count(), 2);

        grid.remove_object(&obj1);
        assert_eq!(grid.get_object_count(), 1);

        grid.remove_object(&obj2);
        assert_eq!(grid.get_object_count(), 0);
    }

    #[test]
    fn test_grid_collect_objects_point() {
        let mut grid = GridCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(2.0, 2.0, 2.0),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        grid.add_object(obj1);
        grid.add_object(obj2);

        grid.reset_collection();
        grid.collect_objects_point(Vector3::new(1.0, 1.0, 1.0)); // Inside obj1

        assert!(grid.get_collection().len() >= 1);
        if let Some(first_obj) = grid.get_collection().peek_first() {
            assert_eq!(first_obj.get_id(), 1);
        }
    }

    #[test]
    fn test_grid_collect_objects_box() {
        let mut grid = GridCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        grid.add_object(obj1);
        grid.add_object(obj2);

        let query_box = AABox::new(Vector3::new(0.5, 0.5, 0.5), Vector3::new(2.0, 2.0, 2.0));

        grid.reset_collection();
        grid.collect_objects_box(&query_box);

        assert!(grid.get_collection().len() >= 1);
    }

    #[test]
    fn test_grid_repartition() {
        let mut grid = GridCullSystem::new();
        let initial_cells = grid.total_cell_count();

        // Add some objects first
        let obj = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        grid.add_object(obj);

        // Re-partition with different bounds
        grid.re_partition(
            Vector3::new(-50.0, -50.0, -50.0),
            Vector3::new(50.0, 50.0, 50.0),
            10.0,
        );

        // Object should still be in the system
        assert_eq!(grid.get_object_count(), 1);

        // Cell count might have changed
        let new_cells = grid.total_cell_count();
        assert!(new_cells > 0);
    }

    #[test]
    fn test_grid_update_culling() {
        let mut grid = GridCullSystem::new();

        let obj = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        grid.add_object(obj.clone());

        // Update object's position (simulate movement)
        grid.update_culling(&obj);

        // Object should still be in the system
        assert_eq!(grid.get_object_count(), 1);
    }

    #[test]
    fn test_grid_stats() {
        let grid = GridCullSystem::new();
        let stats = grid.get_stats();

        assert!(stats.node_count > 0);
        assert_eq!(stats.nodes_accepted, 0);
        assert_eq!(stats.nodes_rejected, 0);
    }
}
