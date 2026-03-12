//! Collision System - Main collision management
//!
//! This module provides the main collision system that integrates
//! all bounding volume types and manages collision queries.

use super::*;
use glam::Vec3;
use std::collections::HashMap;

/// Collision system that manages collision queries and spatial partitioning
#[derive(Debug)]
pub struct CollisionSystem {
    aaboxes: HashMap<String, AABoxClass>,
    obboxes: HashMap<String, OBBoxClass>,
    spheres: HashMap<String, SphereClass>,
    planes: HashMap<String, PlaneClass>,
}

impl CollisionSystem {
    /// Create a new collision system
    pub fn new() -> Self {
        Self {
            aaboxes: HashMap::new(),
            obboxes: HashMap::new(),
            spheres: HashMap::new(),
            planes: HashMap::new(),
        }
    }

    /// Add an AABox to the system
    pub fn add_aabox(&mut self, id: String, aabox: AABoxClass) {
        self.aaboxes.insert(id, aabox);
    }

    /// Add an OBBox to the system
    pub fn add_obbox(&mut self, id: String, obbox: OBBoxClass) {
        self.obboxes.insert(id, obbox);
    }

    /// Add a sphere to the system
    pub fn add_sphere(&mut self, id: String, sphere: SphereClass) {
        self.spheres.insert(id, sphere);
    }

    /// Add a plane to the system
    pub fn add_plane(&mut self, id: String, plane: PlaneClass) {
        self.planes.insert(id, plane);
    }

    /// Remove a collision object by ID
    pub fn remove(&mut self, id: &str) {
        self.aaboxes.remove(id);
        self.obboxes.remove(id);
        self.spheres.remove(id);
        self.planes.remove(id);
    }

    /// Get AABox by ID
    pub fn get_aabox(&self, id: &str) -> Option<&AABoxClass> {
        self.aaboxes.get(id)
    }

    /// Get OBBox by ID
    pub fn get_obbox(&self, id: &str) -> Option<&OBBoxClass> {
        self.obboxes.get(id)
    }

    /// Get sphere by ID
    pub fn get_sphere(&self, id: &str) -> Option<&SphereClass> {
        self.spheres.get(id)
    }

    /// Get plane by ID
    pub fn get_plane(&self, id: &str) -> Option<&PlaneClass> {
        self.planes.get(id)
    }

    /// Test ray intersection with all objects
    pub fn ray_cast(&self, ray: &RayCollisionQuery) -> Vec<(String, CollisionResult)> {
        let mut results = Vec::new();

        // Test against all AABoxes
        for (id, aabox) in &self.aaboxes {
            if let Some(result) = ray.test_aabox(aabox) {
                results.push((id.clone(), result));
            }
        }

        // Test against all OBBoxes
        for (id, obbox) in &self.obboxes {
            if let Some(result) = ray.test_obbox(obbox) {
                results.push((id.clone(), result));
            }
        }

        // Test against all spheres
        for (id, sphere) in &self.spheres {
            if let Some(result) = ray.test_sphere(sphere) {
                results.push((id.clone(), result));
            }
        }

        // Test against all planes
        for (id, plane) in &self.planes {
            if let Some(result) = ray.test_plane(plane) {
                results.push((id.clone(), result));
            }
        }

        results
    }

    /// Test intersection between two objects by ID
    pub fn test_intersection(&self, id1: &str, id2: &str) -> bool {
        // Try all combinations of object types

        // AABox vs AABox
        if let (Some(a), Some(b)) = (self.get_aabox(id1), self.get_aabox(id2)) {
            return test_aabox_aabox(a, b);
        }

        // AABox vs Sphere
        if let Some(aabox) = self.get_aabox(id1) {
            if let Some(sphere) = self.get_sphere(id2) {
                return test_aabox_sphere(aabox, sphere);
            }
        }
        if let Some(aabox) = self.get_aabox(id2) {
            if let Some(sphere) = self.get_sphere(id1) {
                return test_aabox_sphere(aabox, sphere);
            }
        }

        // AABox vs OBBox
        if let Some(aabox) = self.get_aabox(id1) {
            if let Some(obbox) = self.get_obbox(id2) {
                return test_aabox_obbox(aabox, obbox);
            }
        }
        if let Some(aabox) = self.get_aabox(id2) {
            if let Some(obbox) = self.get_obbox(id1) {
                return test_aabox_obbox(aabox, obbox);
            }
        }

        // Sphere vs Sphere
        if let (Some(a), Some(b)) = (self.get_sphere(id1), self.get_sphere(id2)) {
            return test_sphere_sphere(a, b);
        }

        // Sphere vs OBBox
        if let Some(sphere) = self.get_sphere(id1) {
            if let Some(obbox) = self.get_obbox(id2) {
                return test_sphere_obbox(sphere, obbox);
            }
        }
        if let Some(sphere) = self.get_sphere(id2) {
            if let Some(obbox) = self.get_obbox(id1) {
                return test_sphere_obbox(sphere, obbox);
            }
        }

        // OBBox vs OBBox (simplified as AABox intersection)
        if let (Some(a), Some(b)) = (self.get_obbox(id1), self.get_obbox(id2)) {
            let a_aabox = AABoxClass::from_center_and_extent(a.center, a.extent);
            let b_aabox = AABoxClass::from_center_and_extent(b.center, b.extent);
            return test_aabox_aabox(&a_aabox, &b_aabox);
        }

        false // No intersection or unsupported types
    }

    /// Get all objects within a frustum
    pub fn frustum_cull(&self, frustum_planes: &[PlaneClass]) -> Vec<String> {
        let mut visible_objects = Vec::new();

        // Test AABoxes
        for (id, aabox) in &self.aaboxes {
            if test_frustum_culling(frustum_planes, aabox) {
                visible_objects.push(id.clone());
            }
        }

        // Test OBBoxes (convert to AABox for culling)
        for (id, obbox) in &self.obboxes {
            let aabox = obbox.bounding_aabox();
            if test_frustum_culling(frustum_planes, &aabox) {
                visible_objects.push(id.clone());
            }
        }

        // Test spheres (convert to AABox for culling)
        for (id, sphere) in &self.spheres {
            let aabox = sphere.bounding_aabox();
            if test_frustum_culling(frustum_planes, &aabox) {
                visible_objects.push(id.clone());
            }
        }

        visible_objects
    }

    /// Get statistics about the collision system
    pub fn get_stats(&self) -> CollisionStats {
        CollisionStats {
            num_aaboxes: self.aaboxes.len(),
            num_obboxes: self.obboxes.len(),
            num_spheres: self.spheres.len(),
            num_planes: self.planes.len(),
            total_objects: self.aaboxes.len()
                + self.obboxes.len()
                + self.spheres.len()
                + self.planes.len(),
        }
    }

    /// Clear all collision objects
    pub fn clear(&mut self) {
        self.aaboxes.clear();
        self.obboxes.clear();
        self.spheres.clear();
        self.planes.clear();
    }
}

/// Statistics about the collision system
#[derive(Debug, Clone)]
pub struct CollisionStats {
    pub num_aaboxes: usize,
    pub num_obboxes: usize,
    pub num_spheres: usize,
    pub num_planes: usize,
    pub total_objects: usize,
}

/// Main collision management class (equivalent to CollisionTestClass)
#[derive(Debug)]
pub struct CollisionMath;

impl CollisionMath {
    /// Test ray intersection with AABox
    pub fn ray_aabb_test(
        ray_origin: Vec3,
        ray_dir: Vec3,
        aabox: &AABoxClass,
    ) -> Option<CollisionResult> {
        let query = RayCollisionQuery::new(ray_origin, ray_dir, f32::INFINITY);
        query.test_aabox(aabox)
    }

    /// Test ray intersection with OBBox
    pub fn ray_obbox_test(
        ray_origin: Vec3,
        ray_dir: Vec3,
        obbox: &OBBoxClass,
    ) -> Option<CollisionResult> {
        let query = RayCollisionQuery::new(ray_origin, ray_dir, f32::INFINITY);
        query.test_obbox(obbox)
    }

    /// Test ray intersection with Sphere
    pub fn ray_sphere_test(
        ray_origin: Vec3,
        ray_dir: Vec3,
        sphere: &SphereClass,
    ) -> Option<CollisionResult> {
        let query = RayCollisionQuery::new(ray_origin, ray_dir, f32::INFINITY);
        query.test_sphere(sphere)
    }

    /// Test point in AABox
    pub fn point_in_aabox(point: Vec3, aabox: &AABoxClass) -> bool {
        aabox.contains_point(&point)
    }

    /// Test point in OBBox
    pub fn point_in_obbox(point: Vec3, obbox: &OBBoxClass) -> bool {
        obbox.contains_point(point)
    }

    /// Test point in sphere
    pub fn point_in_sphere(point: Vec3, sphere: &SphereClass) -> bool {
        sphere.contains_point(point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collision_system_basic() {
        let mut system = CollisionSystem::new();

        let aabox =
            AABoxClass::from_center_extent(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        system.add_aabox("test_aabox".to_string(), aabox);

        assert!(system.get_aabox("test_aabox").is_some());
        assert!(system.get_sphere("nonexistent").is_none());

        let stats = system.get_stats();
        assert_eq!(stats.num_aaboxes, 1);
        assert_eq!(stats.total_objects, 1);
    }

    #[test]
    fn test_collision_system_ray_cast() {
        let mut system = CollisionSystem::new();

        let aabox =
            AABoxClass::from_center_extent(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        system.add_aabox("test_aabox".to_string(), aabox);

        let ray = RayCollisionQuery::new(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 10.0);

        let results = system.ray_cast(&ray);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, "test_aabox");
        assert!(results[0].1.has_collision);
    }

    #[test]
    fn test_collision_system_intersection() {
        let mut system = CollisionSystem::new();

        let aabox1 = AABoxClass::from_center_extent(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let aabox2 =
            AABoxClass::from_center_extent(Vec3::new(1.5, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        system.add_aabox("aabox1".to_string(), aabox1);
        system.add_aabox("aabox2".to_string(), aabox2);

        assert!(system.test_intersection("aabox1", "aabox2"));

        let aabox3 =
            AABoxClass::from_center_extent(Vec3::new(3.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        system.add_aabox("aabox3".to_string(), aabox3);

        assert!(!system.test_intersection("aabox1", "aabox3"));
    }

    #[test]
    fn test_collision_math() {
        let aabox = AABoxClass::from_center_extent(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));

        let result = CollisionMath::ray_aabb_test(
            Vec3::new(-2.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            &aabox,
        );

        assert!(result.is_some());
        assert!(result.unwrap().has_collision);
    }
}
