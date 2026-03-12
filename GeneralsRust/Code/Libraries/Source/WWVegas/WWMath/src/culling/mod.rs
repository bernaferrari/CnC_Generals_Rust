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

//! Culling system module for spatial partitioning and object culling.

mod aab_tree_cull;
mod grid_cull;
#[cfg(test)]
mod tests;

pub use aab_tree_cull::{AABTreeCullSystem, AABTreeNode};
pub use grid_cull::GridCullSystem;

use crate::{AABox, Frustum, Sphere, Vector3};
use std::sync::Arc;

/// Result of a culling operation indicating the relationship between
/// an object and the culling volume.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullType {
    /// The object is completely outside the culling volume
    Outside = 0,
    /// The object intersects an edge of the culling volume
    Intersecting,
    /// The object is completely inside the culling volume
    Inside,
}

/// Overlap test results for more detailed collision detection
/// This enum provides compatibility with the existing OverlapResult in AABox
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlapType {
    /// No overlap between objects
    Outside = 0,
    /// Partial overlap between objects  
    Intersecting,
    /// One object is completely inside the other
    Inside,
}

impl From<CullType> for OverlapType {
    fn from(cull_type: CullType) -> Self {
        match cull_type {
            CullType::Outside => OverlapType::Outside,
            CullType::Intersecting => OverlapType::Intersecting,
            CullType::Inside => OverlapType::Inside,
        }
    }
}

impl From<OverlapType> for CullType {
    fn from(overlap_type: OverlapType) -> Self {
        match overlap_type {
            OverlapType::Outside => CullType::Outside,
            OverlapType::Intersecting => CullType::Intersecting,
            OverlapType::Inside => CullType::Inside,
        }
    }
}

/// Statistics for culling operations
#[derive(Debug, Clone, Copy, Default)]
pub struct CullStats {
    /// Total number of nodes in the culling structure
    pub node_count: usize,
    /// Number of nodes accepted during culling
    pub nodes_accepted: usize,
    /// Number of nodes trivially accepted (fully inside)
    pub nodes_trivially_accepted: usize,
    /// Number of nodes rejected during culling
    pub nodes_rejected: usize,
}

impl CullStats {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn reset(&mut self) {
        self.nodes_accepted = 0;
        self.nodes_trivially_accepted = 0;
        self.nodes_rejected = 0;
    }
}

/// Trait for objects that can be inserted into culling systems
pub trait Cullable: Send + Sync {
    /// Get the axis-aligned bounding box for this object
    fn get_cull_box(&self) -> AABox;

    /// Set the bounding box for this object (triggers culling system update)
    fn set_cull_box(&mut self, box_: AABox, just_loaded: bool);

    /// Get a unique identifier for this object (for debugging)
    fn get_id(&self) -> u64 {
        // Default implementation using pointer address
        (self as *const Self as *const ()).addr() as u64
    }
}

/// Collection of cullable objects returned by culling operations
pub struct CullCollection {
    objects: Vec<Arc<dyn Cullable>>,
    current_index: usize,
}

impl CullCollection {
    pub fn new() -> Self {
        Self {
            objects: Vec::new(),
            current_index: 0,
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            objects: Vec::with_capacity(capacity),
            current_index: 0,
        }
    }

    pub fn clear(&mut self) {
        self.objects.clear();
        self.current_index = 0;
    }

    pub fn add(&mut self, object: Arc<dyn Cullable>) {
        self.objects.push(object);
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Get first object and reset iterator
    pub fn first(&mut self) -> Option<Arc<dyn Cullable>> {
        self.current_index = 0;
        self.next()
    }

    /// Peek at first object without modifying iterator
    pub fn peek_first(&self) -> Option<Arc<dyn Cullable>> {
        self.objects.first().cloned()
    }

    /// Peek at next object without advancing iterator
    pub fn peek_next(&self) -> Option<Arc<dyn Cullable>> {
        if self.current_index < self.objects.len() {
            Some(self.objects[self.current_index].clone())
        } else {
            None
        }
    }

    /// Get iterator over all objects
    pub fn iter(&self) -> impl Iterator<Item = &Arc<dyn Cullable>> {
        self.objects.iter()
    }
}

impl Iterator for CullCollection {
    type Item = Arc<dyn Cullable>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.current_index < self.objects.len() {
            let result = self.objects[self.current_index].clone();
            self.current_index += 1;
            Some(result)
        } else {
            None
        }
    }
}

impl Default for CullCollection {
    fn default() -> Self {
        Self::new()
    }
}

/// Base trait for all culling systems
pub trait CullSystem {
    /// Reset the current collection of culled objects
    fn reset_collection(&mut self);

    /// Collect objects that overlap with a point
    fn collect_objects_point(&mut self, point: Vector3);

    /// Collect objects that overlap with an axis-aligned box
    fn collect_objects_box(&mut self, box_: &AABox);

    /// Collect objects that overlap with a frustum
    fn collect_objects_frustum(&mut self, frustum: &Frustum);

    /// Update an object's position in the culling system
    fn update_culling(&mut self, object: &Arc<dyn Cullable>);

    /// Add an object to the culling system
    fn add_object(&mut self, object: Arc<dyn Cullable>);

    /// Remove an object from the culling system
    fn remove_object(&mut self, object: &Arc<dyn Cullable>);

    /// Get the current collection of culled objects
    fn get_collection(&mut self) -> &mut CullCollection;

    /// Get culling statistics
    fn get_stats(&self) -> CullStats;

    /// Reset statistics
    fn reset_stats(&mut self);

    /// Get total number of objects in the system
    fn get_object_count(&self) -> usize;
}

/// Collision math utilities for culling operations
pub struct CollisionMath;

impl CollisionMath {
    /// Test overlap between a frustum and an axis-aligned box
    pub fn overlap_test_frustum_box(frustum: &Frustum, box_: &AABox) -> OverlapType {
        // Test against bounding box first for quick rejection
        let box_min = box_.min_corner();
        let box_max = box_.max_corner();

        if box_max.x < frustum.get_bound_min().x
            || box_min.x > frustum.get_bound_max().x
            || box_max.y < frustum.get_bound_min().y
            || box_min.y > frustum.get_bound_max().y
            || box_max.z < frustum.get_bound_min().z
            || box_min.z > frustum.get_bound_max().z
        {
            return OverlapType::Outside;
        }

        // Test against all frustum planes
        let mut inside_count = 0;
        for i in 0..6 {
            if let Some(plane) = frustum.get_plane(i) {
                let mut corners_inside = 0;
                let corners = [
                    Vector3::new(box_min.x, box_min.y, box_min.z),
                    Vector3::new(box_max.x, box_min.y, box_min.z),
                    Vector3::new(box_min.x, box_max.y, box_min.z),
                    Vector3::new(box_max.x, box_max.y, box_min.z),
                    Vector3::new(box_min.x, box_min.y, box_max.z),
                    Vector3::new(box_max.x, box_min.y, box_max.z),
                    Vector3::new(box_min.x, box_max.y, box_max.z),
                    Vector3::new(box_max.x, box_max.y, box_max.z),
                ];

                for corner in &corners {
                    if !plane.is_point_in_front(*corner) {
                        corners_inside += 1;
                    }
                }

                if corners_inside == 0 {
                    // All corners are outside this plane
                    return OverlapType::Outside;
                } else if corners_inside == 8 {
                    // All corners are inside this plane
                    inside_count += 1;
                }
            }
        }

        if inside_count == 6 {
            OverlapType::Inside
        } else {
            OverlapType::Intersecting
        }
    }

    /// Test overlap between two axis-aligned boxes
    pub fn overlap_test_box_box(box1: &AABox, box2: &AABox) -> OverlapType {
        use crate::OverlapResult;
        match box1.overlap_test_box(box2) {
            OverlapResult::Outside => OverlapType::Outside,
            OverlapResult::Intersecting => OverlapType::Intersecting,
            OverlapResult::Inside => OverlapType::Inside,
        }
    }

    /// Test overlap between a box and a point
    pub fn overlap_test_box_point(box_: &AABox, point: Vector3) -> OverlapType {
        use crate::OverlapResult;
        match box_.overlap_test_point(point) {
            OverlapResult::Outside => OverlapType::Outside,
            OverlapResult::Intersecting => OverlapType::Intersecting,
            OverlapResult::Inside => OverlapType::Inside,
        }
    }

    /// Test overlap between a box and a sphere
    pub fn overlap_test_box_sphere(box_: &AABox, sphere: &Sphere) -> OverlapType {
        // Find closest point on box to sphere center
        let box_min = box_.min_corner();
        let box_max = box_.max_corner();

        let closest = Vector3::new(
            sphere.center.x.max(box_min.x).min(box_max.x),
            sphere.center.y.max(box_min.y).min(box_max.y),
            sphere.center.z.max(box_min.z).min(box_max.z),
        );

        let distance_squared = (closest - sphere.center).length_squared();
        let radius_squared = sphere.radius * sphere.radius;

        if distance_squared > radius_squared {
            OverlapType::Outside
        } else if Self::sphere_contains_box(sphere, box_) {
            OverlapType::Inside
        } else {
            OverlapType::Intersecting
        }
    }

    /// Check if a sphere completely contains a box
    fn sphere_contains_box(sphere: &Sphere, box_: &AABox) -> bool {
        // Check if all corners of the box are within the sphere
        let box_min = box_.min_corner();
        let box_max = box_.max_corner();
        let radius_squared = sphere.radius * sphere.radius;

        let corners = [
            Vector3::new(box_min.x, box_min.y, box_min.z),
            Vector3::new(box_max.x, box_min.y, box_min.z),
            Vector3::new(box_min.x, box_max.y, box_min.z),
            Vector3::new(box_max.x, box_max.y, box_min.z),
            Vector3::new(box_min.x, box_min.y, box_max.z),
            Vector3::new(box_max.x, box_min.y, box_max.z),
            Vector3::new(box_min.x, box_max.y, box_max.z),
            Vector3::new(box_max.x, box_max.y, box_max.z),
        ];

        for corner in &corners {
            if (*corner - sphere.center).length_squared() > radius_squared {
                return false;
            }
        }

        true
    }
}
