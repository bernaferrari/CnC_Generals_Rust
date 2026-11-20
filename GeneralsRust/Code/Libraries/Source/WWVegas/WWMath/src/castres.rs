//! Results of a volume or ray cast from `WWMath/castres.h`.

use crate::Vector3;

/// Triangle ray-cast result bits from `WWMath/tri.h`.
pub mod raycast_flags {
    /// No additional intersection flags.
    pub const NONE: u8 = 0x00;
    /// The intersection lies on a triangle edge.
    pub const HIT_EDGE: u8 = 0x01;
    /// The ray starts within the triangle.
    pub const START_IN_TRI: u8 = 0x02;
}

/// Cast output with the field layout of the original 32-bit game.
#[repr(C)]
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
    /// Create the default result with no contact point requested.
    pub const fn new() -> Self {
        Self {
            start_bad: false,
            fraction: 1.0,
            normal: Vector3::ZERO,
            surface_type: 0,
            compute_contact_point: false,
            contact_point: Vector3::ZERO,
        }
    }

    /// Reset every field, including the optional contact point request.
    pub fn reset(&mut self) {
        *self = Self::new();
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
