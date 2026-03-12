//! Frustum culling implementation

use glam::{Mat4, Vec3};
use ww3d_collision::{AABox, Plane};

/// Frustum class for culling operations
#[derive(Debug, Clone)]
pub struct FrustumClass {
    pub planes: [Plane; 6],
    pub corners: [Vec3; 8],
    pub valid: bool,
}

impl FrustumClass {
    /// Create a new frustum
    pub fn new() -> Self {
        Self {
            planes: [
                Plane::new(Vec3::ZERO, 0.0),
                Plane::new(Vec3::ZERO, 0.0),
                Plane::new(Vec3::ZERO, 0.0),
                Plane::new(Vec3::ZERO, 0.0),
                Plane::new(Vec3::ZERO, 0.0),
                Plane::new(Vec3::ZERO, 0.0),
            ],
            corners: [
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
                Vec3::ZERO,
            ],
            valid: false,
        }
    }

    /// Update frustum from view-projection matrix
    pub fn update_from_matrix(&mut self, view_proj_matrix: &Mat4) {
        let m = view_proj_matrix.to_cols_array_2d();

        // Extract frustum planes from the matrix
        // Right plane
        self.planes[0] = Plane::new(
            Vec3::new(m[0][3] - m[0][0], m[1][3] - m[1][0], m[2][3] - m[2][0]),
            m[3][3] - m[3][0],
        );

        // Left plane
        self.planes[1] = Plane::new(
            Vec3::new(m[0][3] + m[0][0], m[1][3] + m[1][0], m[2][3] + m[2][0]),
            m[3][3] + m[3][0],
        );

        // Bottom plane
        self.planes[2] = Plane::new(
            Vec3::new(m[0][3] + m[0][1], m[1][3] + m[1][1], m[2][3] + m[2][1]),
            m[3][3] + m[3][1],
        );

        // Top plane
        self.planes[3] = Plane::new(
            Vec3::new(m[0][3] - m[0][1], m[1][3] - m[1][1], m[2][3] - m[2][1]),
            m[3][3] - m[3][1],
        );

        // Far plane
        self.planes[4] = Plane::new(
            Vec3::new(m[0][3] - m[0][2], m[1][3] - m[1][2], m[2][3] - m[2][2]),
            m[3][3] - m[3][2],
        );

        // Near plane
        self.planes[5] = Plane::new(
            Vec3::new(m[0][3] + m[0][2], m[1][3] + m[1][2], m[2][3] + m[2][2]),
            m[3][3] + m[3][2],
        );

        // Normalize all planes
        for plane in &mut self.planes {
            let length = plane.normal.length();
            if length > 0.0 {
                plane.normal /= length;
                plane.distance /= length;
            }
        }

        // Calculate frustum corners (optional, for more advanced culling)
        // This would require the inverse view-projection matrix

        self.valid = true;
    }

    /// Check if frustum intersects with AABox
    /// CRITICAL: Uses optimized p-vertex algorithm instead of testing all 8 corners (C++ parity)
    pub fn intersects_aabox(&self, aabox: &AABox) -> bool {
        if !self.valid {
            return true; // If frustum is invalid, assume intersection
        }

        // Convert center-extent representation to min-max for p-vertex algorithm
        let box_min = aabox.center - aabox.extent;
        let box_max = aabox.center + aabox.extent;

        // Test AABox against all frustum planes using optimized p-vertex test
        for plane in &self.planes {
            // Get the positive vertex (furthest along plane normal) - p-vertex optimization
            // This replaces the inefficient 8-corner loop with a single vertex test
            let positive_vertex = Vec3::new(
                if plane.normal.x >= 0.0 {
                    box_max.x
                } else {
                    box_min.x
                },
                if plane.normal.y >= 0.0 {
                    box_max.y
                } else {
                    box_min.y
                },
                if plane.normal.z >= 0.0 {
                    box_max.z
                } else {
                    box_min.z
                },
            );

            // If the positive vertex is behind the plane, the entire box is outside
            let distance = plane.normal.dot(positive_vertex) + plane.distance;
            if distance < 0.0 {
                return false;
            }
        }

        true
    }

    /// Check if frustum intersects with a sphere
    pub fn intersects_sphere(&self, center: Vec3, radius: f32) -> bool {
        if !self.valid {
            return true; // If frustum is invalid, assume intersection
        }

        // Test sphere against all frustum planes
        for plane in &self.planes {
            let distance = plane.normal.dot(center) + plane.distance;
            if distance < -radius {
                return false; // Sphere is completely outside this plane
            }
        }

        true
    }
}
