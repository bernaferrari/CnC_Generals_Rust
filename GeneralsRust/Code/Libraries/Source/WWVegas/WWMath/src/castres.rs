//! Cast result structure - matches C++ CastResultStruct
//!
//! Used to store the results of ray casting operations

use crate::Vector3;

/// Ray casting flags - matches C++ TRI_RAYCAST_FLAG_* constants
pub mod raycast_flags {
    pub const NONE: u8 = 0x00;
    pub const HIT_EDGE: u8 = 0x01;
    pub const START_IN_TRI: u8 = 0x02;
}

/// Cast result structure - matches C++ CastResultStruct
/// Note: This is separate from the collision CastResult to avoid naming conflicts
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CastResultStruct {
    /// Whether the start position is considered "bad" (inside the geometry)
    pub start_bad: bool,
    /// The fraction along the ray where intersection occurred (0.0 to 1.0)
    pub fraction: f32,
    /// Surface normal at the intersection point
    pub normal: Vector3,
    /// Surface type identifier
    pub surface_type: u32,
    /// Whether to compute the contact point
    pub compute_contact_point: bool,
    /// Contact point (computed if compute_contact_point is true)
    pub contact_point: Vector3,
}

impl CastResultStruct {
    /// Create a new cast result with default values
    pub fn new() -> Self {
        Self {
            start_bad: false,
            fraction: 1.0,
            normal: Vector3::new(0.0, 0.0, 0.0),
            surface_type: 0,
            compute_contact_point: false,
            contact_point: Vector3::new(0.0, 0.0, 0.0),
        }
    }

    /// Reset the cast result to default values - matches C++ Reset()
    pub fn reset(&mut self) {
        self.start_bad = false;
        self.fraction = 1.0;
        self.normal = Vector3::new(0.0, 0.0, 0.0);
        self.surface_type = 0;
        self.compute_contact_point = false;
        self.contact_point = Vector3::new(0.0, 0.0, 0.0);
    }

    /// Set the result as a successful intersection
    pub fn set_intersection(&mut self, fraction: f32, normal: Vector3) {
        self.fraction = fraction;
        self.normal = normal;
        self.start_bad = false;
    }

    /// Set the result as starting inside geometry
    pub fn set_start_bad(&mut self) {
        self.start_bad = true;
        self.fraction = 0.0;
    }

    /// Check if this result represents a valid intersection
    pub fn is_valid_intersection(&self) -> bool {
        !self.start_bad && self.fraction >= 0.0 && self.fraction <= 1.0
    }

    /// Compute contact point if requested
    pub fn compute_contact_point_if_needed(&mut self, ray_start: Vector3, ray_end: Vector3) {
        if self.compute_contact_point {
            self.contact_point = ray_start + (ray_end - ray_start) * self.fraction;
        }
    }
}

impl Default for CastResultStruct {
    fn default() -> Self {
        Self::new()
    }
}
