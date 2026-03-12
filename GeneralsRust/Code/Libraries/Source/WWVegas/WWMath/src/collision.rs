/*
 * Command & Conquer Generals Zero Hour - Rust Edition
 *
 * Collision Detection System
 *
 * This module provides comprehensive collision detection between various geometric primitives
 * including points, lines, planes, spheres, boxes, triangles, and frustums.
 *
 * The system supports:
 * - Simple intersection tests (boolean results)
 * - Overlap tests (categorized spatial relationships)
 * - Moving collision detection (swept volumes)
 * - Ray/line segment casting
 *
 * Based on the original C++ WWMath collision detection system.
 */

use super::*;

/// Collision detection epsilon for floating point comparisons
pub const COLLISION_EPSILON: f32 = 0.001;

/// Very small epsilon for coincidence tests
pub const COINCIDENCE_EPSILON: f32 = 0.000001;

/// Result of an overlap test between two geometric primitives
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OverlapType {
    /// Object is in positive space (front/outside)
    Positive = 0x01,
    /// Object is in negative space (back/inside)
    Negative = 0x02,
    /// Object is on the boundary
    On = 0x04,
    /// Object spans both positive and negative space (overlapping)
    Both = 0x08,
}

impl OverlapType {
    pub const OUTSIDE: Self = Self::Positive;
    pub const INSIDE: Self = Self::Negative;
    pub const OVERLAPPED: Self = Self::Both;
    pub const FRONT: Self = Self::Positive;
    pub const BACK: Self = Self::Negative;
}

/// Result of a volume or ray casting operation
#[derive(Debug, Clone)]
pub struct CastResult {
    /// Was the initial configuration interpenetrating something?
    pub start_bad: bool,
    /// Fraction of the move up until collision (0.0 to 1.0)
    pub fraction: f32,
    /// Surface normal at the collision point
    pub normal: Vector3,
    /// Surface type identifier at collision point
    pub surface_type: u32,
    /// Should the collision code compute the contact point?
    pub compute_contact_point: bool,
    /// Point of collision (only valid if compute_contact_point is true)
    pub contact_point: Vector3,
}

impl Default for CastResult {
    fn default() -> Self {
        Self {
            start_bad: false,
            fraction: 1.0,
            normal: Vector3::ZERO,
            surface_type: 0,
            compute_contact_point: false,
            contact_point: Vector3::ZERO,
        }
    }
}

impl CastResult {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

/// Optional statistics tracking for collision detection performance
#[derive(Debug, Default, Clone)]
pub struct CollisionStats {
    pub total_collision_count: i32,
    pub total_collision_hit_count: i32,
    pub ray_triangle_count: i32,
    pub ray_triangle_hit_count: i32,
    pub aabox_triangle_count: i32,
    pub aabox_triangle_hit_count: i32,
    pub aabox_aabox_count: i32,
    pub aabox_aabox_hit_count: i32,
    pub obbox_triangle_count: i32,
    pub obbox_triangle_hit_count: i32,
    pub obbox_aabox_count: i32,
    pub obbox_aabox_hit_count: i32,
    pub obbox_obbox_count: i32,
    pub obbox_obbox_hit_count: i32,
}

impl CollisionStats {
    pub fn reset(&mut self) {
        *self = Default::default();
    }
}

/// Main collision detection system providing intersection, overlap, and collision tests
#[derive(Default)]
pub struct CollisionMath {
    #[cfg(feature = "collision-stats")]
    stats: CollisionStats,
}

impl CollisionMath {
    pub fn new() -> Self {
        Default::default()
    }

    #[cfg(feature = "collision-stats")]
    pub fn get_stats(&self) -> &CollisionStats {
        &self.stats
    }

    #[cfg(feature = "collision-stats")]
    pub fn reset_stats(&mut self) {
        self.stats.reset();
    }

    /// Evaluate overlap mask from multiple point tests
    fn eval_overlap_mask(mask: i32) -> OverlapType {
        // Check if all vertices are "ON"
        if mask == OverlapType::On as i32 {
            return OverlapType::On;
        }

        // Check if all vertices are either "ON" or "POS"
        if (mask & !((OverlapType::Positive as i32) | (OverlapType::On as i32))) == 0 {
            return OverlapType::Positive;
        }

        // Check if all vertices are either "ON" or "NEG"
        if (mask & !((OverlapType::Negative as i32) | (OverlapType::On as i32))) == 0 {
            return OverlapType::Negative;
        }

        // Otherwise, object spans the boundary
        OverlapType::Both
    }

    /// Evaluate overlap from collision result
    #[allow(dead_code)]
    fn eval_overlap_collision(res: &CastResult) -> OverlapType {
        if res.fraction < 1.0 {
            OverlapType::Both
        } else if res.start_bad {
            OverlapType::Negative
        } else {
            OverlapType::Positive
        }
    }
}

// ========================================================================================
// Sub-modules
// ========================================================================================

mod intersection;
mod overlap;
mod sweep;
#[cfg(test)]
#[cfg(test)]
mod tests;

#[allow(unused_imports)]
pub use intersection::*;
#[allow(unused_imports)]
pub use overlap::*;
#[allow(unused_imports)]
pub use sweep::*;

// ========================================================================================
// Helper Functions
// ========================================================================================

// Helper function to get far extent of a box along an axis
fn get_far_extent(normal: &Vector3, extent: &Vector3) -> Vector3 {
    Vector3::new(
        if normal.x >= 0.0 { extent.x } else { -extent.x },
        if normal.y >= 0.0 { extent.y } else { -extent.y },
        if normal.z >= 0.0 { extent.z } else { -extent.z },
    )
}
