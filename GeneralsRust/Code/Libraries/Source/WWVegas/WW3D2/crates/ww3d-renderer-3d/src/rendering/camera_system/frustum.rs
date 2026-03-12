//! Camera Frustum System
//!
//! This module provides camera frustum culling and management,
//! equivalent to the original DirectX8 frustum functionality.

use glam::{Mat4, Vec3};

/// 3D plane for frustum culling
#[derive(Debug, Clone, Copy)]
pub struct Plane3 {
    pub normal: Vec3,
    pub distance: f32,
}

impl Plane3 {
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    pub fn default() -> Self {
        Self {
            normal: Vec3::ZERO,
            distance: 0.0,
        }
    }

    pub fn normalize(&mut self) {
        let length = self.normal.length();
        if length > 0.0 {
            self.normal /= length;
            self.distance /= length;
        }
    }

    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }
}

/// Camera frustum planes
#[derive(Debug, Clone)]
pub struct Frustum {
    planes: [Plane3; 6],
}

impl Frustum {
    /// Create new frustum
    pub fn new() -> Self {
        Self::default()
    }

    /// Update frustum from view-projection matrix
    pub fn update_from_matrix(&mut self, vp_matrix: Mat4) {
        *self = Self::from_matrix(&vp_matrix);
    }

    /// Create frustum from view-projection matrix
    pub fn from_matrix(vp_matrix: &Mat4) -> Self {
        let matrix = *vp_matrix;
        let mut planes = [Plane3::default(); 6];

        // Left plane
        planes[0] = Plane3::new(
            Vec3::new(
                matrix.row(3).x + matrix.row(0).x,
                matrix.row(3).y + matrix.row(0).y,
                matrix.row(3).z + matrix.row(0).z,
            ),
            matrix.row(3).w + matrix.row(0).w,
        );

        // Right plane
        planes[1] = Plane3::new(
            Vec3::new(
                matrix.row(3).x - matrix.row(0).x,
                matrix.row(3).y - matrix.row(0).y,
                matrix.row(3).z - matrix.row(0).z,
            ),
            matrix.row(3).w - matrix.row(0).w,
        );

        // Top plane
        planes[2] = Plane3::new(
            Vec3::new(
                matrix.row(3).x - matrix.row(1).x,
                matrix.row(3).y - matrix.row(1).y,
                matrix.row(3).z - matrix.row(1).z,
            ),
            matrix.row(3).w - matrix.row(1).w,
        );

        // Bottom plane
        planes[3] = Plane3::new(
            Vec3::new(
                matrix.row(3).x + matrix.row(1).x,
                matrix.row(3).y + matrix.row(1).y,
                matrix.row(3).z + matrix.row(1).z,
            ),
            matrix.row(3).w + matrix.row(1).w,
        );

        // Near plane
        planes[4] = Plane3::new(
            Vec3::new(
                matrix.row(3).x + matrix.row(2).x,
                matrix.row(3).y + matrix.row(2).y,
                matrix.row(3).z + matrix.row(2).z,
            ),
            matrix.row(3).w + matrix.row(2).w,
        );

        // Far plane
        planes[5] = Plane3::new(
            Vec3::new(
                matrix.row(3).x - matrix.row(2).x,
                matrix.row(3).y - matrix.row(2).y,
                matrix.row(3).z - matrix.row(2).z,
            ),
            matrix.row(3).w - matrix.row(2).w,
        );

        Self { planes }
    }

    /// Test if point is inside frustum
    pub fn contains_point(&self, point: &Vec3) -> bool {
        let p = Vec3::new(point.x, point.y, point.z);
        for plane in &self.planes {
            if plane.distance_to_point(p) < 0.0 {
                return false;
            }
        }
        true
    }

    /// Test if sphere intersects frustum
    pub fn intersects_sphere(&self, center: &Vec3, radius: f32) -> bool {
        let c = Vec3::new(center.x, center.y, center.z);
        for plane in &self.planes {
            if plane.distance_to_point(c) < -radius {
                return false;
            }
        }
        true
    }

    /// Check if frustum contains a bounding box
    pub fn contains_box(&self, bbox: &crate::bounding_volumes::aabox::AABoxClass) -> bool {
        let min = bbox.get_min();
        let max = bbox.get_max();
        self.intersects_aabb(&min, &max)
    }

    /// Test if AABB intersects frustum
    pub fn intersects_aabb(&self, min: &Vec3, max: &Vec3) -> bool {
        let min_p = Vec3::new(min.x, min.y, min.z);
        let max_p = Vec3::new(max.x, max.y, max.z);

        for plane in &self.planes {
            // Test all 8 corners of the AABB
            let mut all_outside = true;

            for x in 0..2 {
                for y in 0..2 {
                    for z in 0..2 {
                        let corner = Vec3::new(
                            if x == 0 { min_p.x } else { max_p.x },
                            if y == 0 { min_p.y } else { max_p.y },
                            if z == 0 { min_p.z } else { max_p.z },
                        );

                        if plane.distance_to_point(corner) >= 0.0 {
                            all_outside = false;
                            break;
                        }
                    }
                    if !all_outside {
                        break;
                    }
                }
                if !all_outside {
                    break;
                }
            }

            if all_outside {
                return false;
            }
        }

        true
    }

    /// Get frustum planes
    pub fn planes(&self) -> &[Plane3; 6] {
        &self.planes
    }
}

impl Default for Frustum {
    fn default() -> Self {
        Self {
            planes: [Plane3::default(); 6],
        }
    }
}

/// Type alias for backward compatibility with C++ naming convention
pub type FrustumClass = Frustum;
