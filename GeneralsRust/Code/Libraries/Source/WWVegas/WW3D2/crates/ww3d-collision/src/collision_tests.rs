//! Collision Test Classes
//!
//! This module contains collision test classes ported from coltest.cpp

use crate::bounding_volumes::{AABox, OBBox};
use crate::intersection::CastResult;
use glam::Vec3;

/// Base collision test class
pub trait CollisionTest {
    fn cull(&self, min: Vec3, max: Vec3) -> bool;
    fn get_collision_type(&self) -> u32;
    fn get_result(&self) -> &CastResult;
    fn get_result_mut(&mut self) -> &mut CastResult;
}

/// Axis-aligned box collision test
#[derive(Debug, Clone)]
pub struct AABoxCollisionTest {
    pub aabb: AABox,
    pub movement: Vec3,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub result: CastResult,
    pub collision_type: u32,
}

impl AABoxCollisionTest {
    pub fn new(aabb: AABox, movement: Vec3, collision_type: u32) -> Self {
        let mut test = Self {
            aabb,
            movement,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
            result: CastResult::default(),
            collision_type,
        };

        // Calculate sweep bounds
        test.calculate_sweep_bounds();
        test
    }

    fn calculate_sweep_bounds(&mut self) {
        self.sweep_min = self.aabb.center - self.aabb.extent;
        self.sweep_max = self.aabb.center + self.aabb.extent;

        let end_min = self.aabb.center + self.movement - self.aabb.extent;
        let end_max = self.aabb.center + self.movement + self.aabb.extent;

        // Expand sweep to include end position
        self.sweep_max = self.sweep_max.max(end_max);
        self.sweep_min = self.sweep_min.min(end_min);
    }

    /// Rotate the test by specified rotation (around Z-axis)
    pub fn rotate(&mut self, rotation: RotationType) {
        match rotation {
            RotationType::None => {}
            RotationType::Z90 => {
                // Rotate center and movement
                let temp = self.aabb.center.x;
                self.aabb.center.x = -self.aabb.center.y;
                self.aabb.center.y = temp;

                let temp = self.movement.x;
                self.movement.x = -self.movement.y;
                self.movement.y = temp;

                // Swap x and y extents
                std::mem::swap(&mut self.aabb.extent.x, &mut self.aabb.extent.y);

                // Update sweep bounds
                let min_x = self.sweep_min.x;
                let min_y = self.sweep_min.y;
                let max_x = self.sweep_max.x;
                let max_y = self.sweep_max.y;

                self.sweep_min.x = -max_y;
                self.sweep_min.y = min_x;
                self.sweep_max.x = -min_y;
                self.sweep_max.y = max_x;
            }
            RotationType::Z180 => {
                // Rotate center and movement 180 degrees
                self.aabb.center.x = -self.aabb.center.x;
                self.aabb.center.y = -self.aabb.center.y;
                self.movement.x = -self.movement.x;
                self.movement.y = -self.movement.y;

                // Update sweep bounds
                let min_x = self.sweep_min.x;
                let min_y = self.sweep_min.y;
                let max_x = self.sweep_max.x;
                let max_y = self.sweep_max.y;

                self.sweep_min.x = -max_x;
                self.sweep_min.y = -max_y;
                self.sweep_max.x = -min_x;
                self.sweep_max.y = -min_y;
            }
            RotationType::Z270 => {
                // Rotate center and movement 270 degrees
                let temp = self.aabb.center.x;
                self.aabb.center.x = self.aabb.center.y;
                self.aabb.center.y = -temp;

                let temp = self.movement.x;
                self.movement.x = self.movement.y;
                self.movement.y = -temp;

                // Swap x and y extents
                std::mem::swap(&mut self.aabb.extent.x, &mut self.aabb.extent.y);

                // Update sweep bounds
                let min_x = self.sweep_min.x;
                let min_y = self.sweep_min.y;
                let max_x = self.sweep_max.x;
                let max_y = self.sweep_max.y;

                self.sweep_min.x = min_y;
                self.sweep_min.y = -max_x;
                self.sweep_max.x = max_y;
                self.sweep_max.y = -min_x;
            }
        }
    }
}

impl CollisionTest for AABoxCollisionTest {
    fn cull(&self, min: Vec3, max: Vec3) -> bool {
        // AABB vs AABB culling test
        self.sweep_min.x > max.x
            || self.sweep_max.x < min.x
            || self.sweep_min.y > max.y
            || self.sweep_max.y < min.y
            || self.sweep_min.z > max.z
            || self.sweep_max.z < min.z
    }

    fn get_collision_type(&self) -> u32 {
        self.collision_type
    }

    fn get_result(&self) -> &CastResult {
        &self.result
    }

    fn get_result_mut(&mut self) -> &mut CastResult {
        &mut self.result
    }
}

/// Oriented box collision test
#[derive(Debug, Clone)]
pub struct OBBoxCollisionTest {
    pub obbox: OBBox,
    pub movement: Vec3,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub result: CastResult,
    pub collision_type: u32,
}

impl OBBoxCollisionTest {
    pub fn new(obbox: OBBox, movement: Vec3, collision_type: u32) -> Self {
        let mut test = Self {
            obbox,
            movement,
            sweep_min: Vec3::ZERO,
            sweep_max: Vec3::ZERO,
            result: CastResult::default(),
            collision_type,
        };

        test.calculate_sweep_bounds();
        test
    }

    fn calculate_sweep_bounds(&mut self) {
        // Calculate maximum extent in world space
        let max_extent = Vec3::new(
            (self.obbox.basis[0].x * self.obbox.extent.x).abs()
                + (self.obbox.basis[0].y * self.obbox.extent.y).abs()
                + (self.obbox.basis[0].z * self.obbox.extent.z).abs()
                + 0.01,
            (self.obbox.basis[1].x * self.obbox.extent.x).abs()
                + (self.obbox.basis[1].y * self.obbox.extent.y).abs()
                + (self.obbox.basis[1].z * self.obbox.extent.z).abs()
                + 0.01,
            (self.obbox.basis[2].x * self.obbox.extent.x).abs()
                + (self.obbox.basis[2].y * self.obbox.extent.y).abs()
                + (self.obbox.basis[2].z * self.obbox.extent.z).abs()
                + 0.01,
        );

        self.sweep_min = self.obbox.center - max_extent;
        self.sweep_max = self.obbox.center + max_extent;

        let end_min = self.obbox.center + self.movement - max_extent;
        let end_max = self.obbox.center + self.movement + max_extent;

        // Expand sweep to include end position
        self.sweep_max = self.sweep_max.max(end_max);
        self.sweep_min = self.sweep_min.min(end_min);
    }
}

impl CollisionTest for OBBoxCollisionTest {
    fn cull(&self, min: Vec3, max: Vec3) -> bool {
        // AABB vs swept OBB culling test
        self.sweep_min.x > max.x
            || self.sweep_max.x < min.x
            || self.sweep_min.y > max.y
            || self.sweep_max.y < min.y
            || self.sweep_min.z > max.z
            || self.sweep_max.z < min.z
    }

    fn get_collision_type(&self) -> u32 {
        self.collision_type
    }

    fn get_result(&self) -> &CastResult {
        &self.result
    }

    fn get_result_mut(&mut self) -> &mut CastResult {
        &mut self.result
    }
}

/// Rotation types for collision tests
#[derive(Debug, Clone, Copy)]
pub enum RotationType {
    None,
    Z90,
    Z180,
    Z270,
}

/// Swept sphere collision test
#[derive(Debug, Clone)]
pub struct SphereCollisionTest {
    pub center: Vec3,
    pub radius: f32,
    pub movement: Vec3,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub result: CastResult,
    pub collision_type: u32,
}

impl SphereCollisionTest {
    pub fn new(center: Vec3, radius: f32, movement: Vec3, collision_type: u32) -> Self {
        let extent = Vec3::splat(radius);
        let sweep_min = (center - extent).min(center + movement - extent);
        let sweep_max = (center + extent).max(center + movement + extent);

        Self {
            center,
            radius,
            movement,
            sweep_min,
            sweep_max,
            result: CastResult::default(),
            collision_type,
        }
    }
}

impl CollisionTest for SphereCollisionTest {
    fn cull(&self, min: Vec3, max: Vec3) -> bool {
        // AABB vs swept sphere culling test
        self.sweep_min.x > max.x
            || self.sweep_max.x < min.x
            || self.sweep_min.y > max.y
            || self.sweep_max.y < min.y
            || self.sweep_min.z > max.z
            || self.sweep_max.z < min.z
    }

    fn get_collision_type(&self) -> u32 {
        self.collision_type
    }

    fn get_result(&self) -> &CastResult {
        &self.result
    }

    fn get_result_mut(&mut self) -> &mut CastResult {
        &mut self.result
    }
}

/// Line/Ray collision test
#[derive(Debug, Clone)]
pub struct LineCollisionTest {
    pub start: Vec3,
    pub end: Vec3,
    pub direction: Vec3,
    pub length: f32,
    pub sweep_min: Vec3,
    pub sweep_max: Vec3,
    pub result: CastResult,
    pub collision_type: u32,
}

impl LineCollisionTest {
    pub fn new(start: Vec3, end: Vec3, collision_type: u32) -> Self {
        let direction = end - start;
        let length = direction.length();
        let normalized_direction = if length > 0.0 {
            direction / length
        } else {
            Vec3::ZERO
        };

        let sweep_min = start.min(end);
        let sweep_max = start.max(end);

        Self {
            start,
            end,
            direction: normalized_direction,
            length,
            sweep_min,
            sweep_max,
            result: CastResult::default(),
            collision_type,
        }
    }
}

impl CollisionTest for LineCollisionTest {
    fn cull(&self, min: Vec3, max: Vec3) -> bool {
        // AABB vs line culling test
        self.sweep_min.x > max.x
            || self.sweep_max.x < min.x
            || self.sweep_min.y > max.y
            || self.sweep_max.y < min.y
            || self.sweep_min.z > max.z
            || self.sweep_max.z < min.z
    }

    fn get_collision_type(&self) -> u32 {
        self.collision_type
    }

    fn get_result(&self) -> &CastResult {
        &self.result
    }

    fn get_result_mut(&mut self) -> &mut CastResult {
        &mut self.result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabox_collision_test() {
        let aabb = AABox {
            center: Vec3::ZERO,
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        let movement = Vec3::new(2.0, 0.0, 0.0);
        let test = AABoxCollisionTest::new(aabb, movement, 1);

        // Should not cull against a box that intersects the sweep
        assert!(!test.cull(Vec3::new(0.5, -0.5, -0.5), Vec3::new(1.5, 0.5, 0.5)));

        // Should cull against a box that doesn't intersect the sweep
        assert!(test.cull(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0)));
    }

    #[test]
    fn test_sphere_collision_test() {
        let center = Vec3::ZERO;
        let radius = 1.0;
        let movement = Vec3::new(2.0, 0.0, 0.0);
        let test = SphereCollisionTest::new(center, radius, movement, 1);

        // Should not cull against a box that intersects the sweep
        assert!(!test.cull(Vec3::new(0.5, -0.5, -0.5), Vec3::new(1.5, 0.5, 0.5)));

        // Should cull against a box that doesn't intersect the sweep
        assert!(test.cull(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0)));
    }

    #[test]
    fn test_rotation() {
        let aabb = AABox {
            center: Vec3::new(1.0, 0.0, 0.0),
            extent: Vec3::new(0.5, 1.0, 0.5),
        };
        let movement = Vec3::new(1.0, 0.0, 0.0);
        let mut test = AABoxCollisionTest::new(aabb, movement, 1);

        test.rotate(RotationType::Z90);

        // After 90-degree rotation, center should be at (0, 1, 0)
        assert!((test.aabb.center.x - 0.0).abs() < 0.001);
        assert!((test.aabb.center.y - 1.0).abs() < 0.001);

        // Extents should be swapped
        assert!((test.aabb.extent.x - 1.0).abs() < 0.001);
        assert!((test.aabb.extent.y - 0.5).abs() < 0.001);
    }
}
