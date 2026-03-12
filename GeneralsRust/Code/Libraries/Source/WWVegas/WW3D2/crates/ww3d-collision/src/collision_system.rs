//! Complete Collision Detection System
//!
//! This module demonstrates how all the collision components work together

use crate::{
    aabtree::AABTree,
    bounding_volumes::{AABox, OBBox, Sphere},
    collision_tests::{AABoxCollisionTest, SphereCollisionTest},
    intersection::{CastResult, CollisionMath, IntersectionClass, RayCollisionTest},
    spatial_hash::BroadPhase,
};
use glam::Vec3;

/// Complete collision detection system that integrates all components
pub struct CollisionSystem {
    pub spatial_hash: BroadPhase,
    pub aabtrees: Vec<AABTree>,
    pub static_objects: Vec<StaticCollisionObject>,
    pub dynamic_objects: Vec<DynamicCollisionObject>,
}

/// Static collision object (terrain, buildings, etc.)
#[derive(Debug)]
pub struct StaticCollisionObject {
    pub id: usize,
    pub aabb: AABox,
    pub collision_type: u32,
    pub mesh_index: Option<usize>, // Index into AABTree array
}

/// Dynamic collision object (units, projectiles, etc.)
#[derive(Debug)]
pub struct DynamicCollisionObject {
    pub id: usize,
    pub aabb: AABox,
    pub velocity: Vec3,
    pub collision_type: u32,
    pub shape: CollisionShape,
}

/// Collision shape variants
#[derive(Debug, Clone)]
pub enum CollisionShape {
    Box(AABox),
    OrientedBox(OBBox),
    Sphere(Sphere),
    Line { start: Vec3, end: Vec3 },
}

/// Collision query result
#[derive(Debug)]
pub struct CollisionQueryResult {
    pub hit: bool,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
    pub object_id: Option<usize>,
    pub surface_type: u32,
}

impl CollisionSystem {
    pub fn new(cell_size: f32) -> Self {
        Self {
            spatial_hash: BroadPhase::new(cell_size),
            aabtrees: Vec::new(),
            static_objects: Vec::new(),
            dynamic_objects: Vec::new(),
        }
    }

    /// Add a static mesh with AABTree for detailed collision
    pub fn add_static_mesh(&mut self, aabtree: AABTree, aabb: AABox, collision_type: u32) -> usize {
        let mesh_index = self.aabtrees.len();
        self.aabtrees.push(aabtree);

        let object_id = self.static_objects.len();
        self.static_objects.push(StaticCollisionObject {
            id: object_id,
            aabb,
            collision_type,
            mesh_index: Some(mesh_index),
        });

        object_id
    }

    /// Add a dynamic object
    pub fn add_dynamic_object(
        &mut self,
        shape: CollisionShape,
        velocity: Vec3,
        collision_type: u32,
    ) -> usize {
        let aabb = match &shape {
            CollisionShape::Box(box_shape) => *box_shape,
            CollisionShape::OrientedBox(obb) => AABox {
                center: obb.center,
                extent: obb.extent, // Simplified - should compute proper AABB from OBB
            },
            CollisionShape::Sphere(sphere) => AABox {
                center: sphere.center,
                extent: Vec3::splat(sphere.radius),
            },
            CollisionShape::Line { start, end } => {
                let min = start.min(*end);
                let max = start.max(*end);
                let center = (min + max) * 0.5;
                let extent = (max - min) * 0.5;
                AABox { center, extent }
            }
        };

        let object_id = self.dynamic_objects.len();
        self.dynamic_objects.push(DynamicCollisionObject {
            id: object_id,
            aabb,
            velocity,
            collision_type,
            shape,
        });

        object_id
    }

    /// Update the collision system (call each frame)
    pub fn update(&mut self, dt: f32) {
        // Update dynamic object positions
        for obj in &mut self.dynamic_objects {
            obj.aabb.center += obj.velocity * dt;
        }

        // Update broad phase
        let mut all_objects = Vec::new();

        // Add static objects
        for obj in &self.static_objects {
            all_objects.push((obj.id, obj.aabb));
        }

        // Add dynamic objects (with offset IDs to avoid conflicts)
        for obj in &self.dynamic_objects {
            all_objects.push((obj.id + 1000, obj.aabb));
        }

        self.spatial_hash.update(&all_objects);
    }

    /// Cast a ray through the world
    pub fn cast_ray(
        &self,
        origin: Vec3,
        direction: Vec3,
        max_distance: f32,
        collision_type: u32,
    ) -> CollisionQueryResult {
        let ray_aabb = AABox {
            center: origin + direction * max_distance * 0.5,
            extent: Vec3::splat(max_distance * 0.5).abs() + Vec3::splat(0.1),
        };

        let mut closest_result = CollisionQueryResult {
            hit: false,
            distance: max_distance,
            point: Vec3::ZERO,
            normal: Vec3::ZERO,
            object_id: None,
            surface_type: 0,
        };

        // Query broad phase for potential hits
        let candidates = self.spatial_hash.query_near(&ray_aabb);

        // Test against static objects with detailed meshes
        for obj in &self.static_objects {
            if (obj.collision_type & collision_type) == 0 {
                continue;
            }

            if candidates.contains(&obj.id) {
                if let Some(mesh_index) = obj.mesh_index {
                    // Use AABTree for detailed collision
                    if mesh_index < self.aabtrees.len() {
                        let _ray_test = RayCollisionTest::new(origin, direction);
                        // Note: Would need mesh geometry implementation
                        // Note: AABTree ray casting would provide precise triangle-level collision
                        // but requires full AABTree implementation with traversal and triangle tests.
                        // For now, use AABB intersection as fallback (matches C++ when AABTree unavailable).
                        // C++ equivalent: AABTreeClass::CastRay (aabtree.cpp)

                        let mut intersection = IntersectionClass::new();
                        intersection.set_ray(origin, direction);
                        let box_min = obj.aabb.center - obj.aabb.extent;
                        let box_max = obj.aabb.center + obj.aabb.extent;
                        let result = intersection.intersect_box(box_min, box_max);

                        if result.intersects && result.range < closest_result.distance {
                            closest_result.hit = true;
                            closest_result.distance = result.range;
                            closest_result.point = result.intersection;
                            closest_result.normal = result.normal;
                            closest_result.object_id = Some(obj.id);
                        }
                    }
                } else {
                    // Simple AABB test
                    let mut intersection = IntersectionClass::new();
                    intersection.set_ray(origin, direction);
                    let box_min = obj.aabb.center - obj.aabb.extent;
                    let box_max = obj.aabb.center + obj.aabb.extent;
                    let result = intersection.intersect_box(box_min, box_max);

                    if result.intersects && result.range < closest_result.distance {
                        closest_result.hit = true;
                        closest_result.distance = result.range;
                        closest_result.point = result.intersection;
                        closest_result.normal = result.normal;
                        closest_result.object_id = Some(obj.id);
                    }
                }
            }
        }

        // Test against dynamic objects
        for obj in &self.dynamic_objects {
            if (obj.collision_type & collision_type) == 0 {
                continue;
            }

            let dynamic_id = obj.id + 1000;
            if candidates.contains(&dynamic_id) {
                match &obj.shape {
                    CollisionShape::Sphere(sphere) => {
                        let mut intersection = IntersectionClass::new();
                        intersection.set_ray(origin, direction);
                        let result = intersection.intersect_sphere(sphere.center, sphere.radius);

                        if result.intersects && result.range < closest_result.distance {
                            closest_result.hit = true;
                            closest_result.distance = result.range;
                            closest_result.point = result.intersection;
                            closest_result.normal = result.normal;
                            closest_result.object_id = Some(obj.id);
                        }
                    }
                    _ => {
                        // Fallback to AABB test for other shapes
                        let mut intersection = IntersectionClass::new();
                        intersection.set_ray(origin, direction);
                        let result = intersection.intersect_box(
                            obj.aabb.center - obj.aabb.extent,
                            obj.aabb.center + obj.aabb.extent,
                        );

                        if result.intersects && result.range < closest_result.distance {
                            closest_result.hit = true;
                            closest_result.distance = result.range;
                            closest_result.point = result.intersection;
                            closest_result.normal = result.normal;
                            closest_result.object_id = Some(obj.id);
                        }
                    }
                }
            }
        }

        closest_result.point = origin + direction * closest_result.distance;
        closest_result
    }

    /// Test if a moving object collides with the world
    pub fn test_movement(
        &self,
        object_id: usize,
        movement: Vec3,
        collision_type: u32,
    ) -> Option<CastResult> {
        if object_id >= self.dynamic_objects.len() {
            return None;
        }

        let obj = &self.dynamic_objects[object_id];

        match &obj.shape {
            CollisionShape::Box(box_shape) => {
                let _box_test = AABoxCollisionTest::new(*box_shape, movement, collision_type);

                // Test against static meshes
                for static_obj in &self.static_objects {
                    if (static_obj.collision_type & collision_type) == 0 {
                        continue;
                    }

                    if let Some(mesh_index) = static_obj.mesh_index {
                        if mesh_index < self.aabtrees.len() {
                            // Would need mesh geometry implementation
                            // if self.aabtrees[mesh_index].cast_aabox(&mut box_test, mesh) {
                            //     return Some(box_test.result);
                            // }
                        }
                    }
                }
            }
            CollisionShape::Sphere(sphere) => {
                let sphere_test = SphereCollisionTest::new(
                    sphere.center,
                    sphere.radius,
                    movement,
                    collision_type,
                );

                // Simplified sphere collision test
                let query_aabb = AABox {
                    center: sphere.center + movement * 0.5,
                    extent: Vec3::splat(sphere.radius) + movement.abs() * 0.5,
                };

                let candidates = self.spatial_hash.query_near(&query_aabb);

                for static_obj in &self.static_objects {
                    if candidates.contains(&static_obj.id) {
                        if CollisionMath::aabb_intersect(&query_aabb, &static_obj.aabb) {
                            return Some(sphere_test.result);
                        }
                    }
                }
            }
            _ => {}
        }

        None
    }

    /// Get all current collision pairs
    pub fn get_collision_pairs(&self) -> Vec<(usize, usize)> {
        let pairs = self.spatial_hash.get_collision_pairs();

        // Filter and resolve ID mapping
        let mut resolved_pairs = Vec::new();
        for &(id1, id2) in pairs {
            let obj1_id = if id1 >= 1000 { id1 - 1000 } else { id1 };
            let obj2_id = if id2 >= 1000 { id2 - 1000 } else { id2 };

            // Only include dynamic-dynamic or dynamic-static pairs
            if id1 >= 1000 || id2 >= 1000 {
                resolved_pairs.push((obj1_id, obj2_id));
            }
        }

        resolved_pairs
    }

    /// Get system statistics
    pub fn get_stats(&self) -> CollisionSystemStats {
        let spatial_stats = self.spatial_hash.spatial_hash.get_stats();

        CollisionSystemStats {
            static_objects: self.static_objects.len(),
            dynamic_objects: self.dynamic_objects.len(),
            aabtrees: self.aabtrees.len(),
            spatial_hash_cells: spatial_stats.total_cells,
            spatial_hash_objects: spatial_stats.total_objects,
        }
    }
}

/// Statistics for the collision system
#[derive(Debug)]
pub struct CollisionSystemStats {
    pub static_objects: usize,
    pub dynamic_objects: usize,
    pub aabtrees: usize,
    pub spatial_hash_cells: usize,
    pub spatial_hash_objects: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collision_system() {
        let mut system = CollisionSystem::new(10.0);

        // Add a static box
        let static_aabb = AABox {
            center: Vec3::new(0.0, 0.0, 0.0),
            extent: Vec3::new(5.0, 5.0, 5.0),
        };
        system.add_static_mesh(AABTree::new(), static_aabb, 1);

        // Add a dynamic sphere
        let sphere = Sphere {
            center: Vec3::new(10.0, 0.0, 0.0),
            radius: 2.0,
        };
        system.add_dynamic_object(CollisionShape::Sphere(sphere), Vec3::new(-5.0, 0.0, 0.0), 1);

        system.update(1.0);

        let stats = system.get_stats();
        assert_eq!(stats.static_objects, 1);
        assert_eq!(stats.dynamic_objects, 1);
    }

    #[test]
    fn test_ray_casting() {
        let mut system = CollisionSystem::new(10.0);

        let static_aabb = AABox {
            center: Vec3::new(5.0, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        system.add_static_mesh(AABTree::new(), static_aabb, 1);

        system.update(0.0);

        let result = system.cast_ray(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 10.0, 1);

        // Should hit the static object
        assert!(result.hit);
        assert!(result.distance > 0.0);
    }
}
