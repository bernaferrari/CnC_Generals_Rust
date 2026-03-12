// Culling System

use crate::math::*;

pub struct Frustum {
    pub planes: [Plane; 6],
}

impl Frustum {
    pub fn from_view_projection(vp_matrix: &Mat4) -> Self {
        // Extract frustum planes from view-projection matrix
        let planes = [
            // Left
            Plane {
                normal: Vec3::new(vp_matrix[(0, 3)] + vp_matrix[(0, 0)],
                                 vp_matrix[(1, 3)] + vp_matrix[(1, 0)],
                                 vp_matrix[(2, 3)] + vp_matrix[(2, 0)]).normalize(),
                distance: vp_matrix[(3, 3)] + vp_matrix[(3, 0)],
            },
            // Right
            Plane {
                normal: Vec3::new(vp_matrix[(0, 3)] - vp_matrix[(0, 0)],
                                 vp_matrix[(1, 3)] - vp_matrix[(1, 0)],
                                 vp_matrix[(2, 3)] - vp_matrix[(2, 0)]).normalize(),
                distance: vp_matrix[(3, 3)] - vp_matrix[(3, 0)],
            },
            // Bottom
            Plane {
                normal: Vec3::new(vp_matrix[(0, 3)] + vp_matrix[(0, 1)],
                                 vp_matrix[(1, 3)] + vp_matrix[(1, 1)],
                                 vp_matrix[(2, 3)] + vp_matrix[(2, 1)]).normalize(),
                distance: vp_matrix[(3, 3)] + vp_matrix[(3, 1)],
            },
            // Top
            Plane {
                normal: Vec3::new(vp_matrix[(0, 3)] - vp_matrix[(0, 1)],
                                 vp_matrix[(1, 3)] - vp_matrix[(1, 1)],
                                 vp_matrix[(2, 3)] - vp_matrix[(2, 1)]).normalize(),
                distance: vp_matrix[(3, 3)] - vp_matrix[(3, 1)],
            },
            // Near
            Plane {
                normal: Vec3::new(vp_matrix[(0, 3)] + vp_matrix[(0, 2)],
                                 vp_matrix[(1, 3)] + vp_matrix[(1, 2)],
                                 vp_matrix[(2, 3)] + vp_matrix[(2, 2)]).normalize(),
                distance: vp_matrix[(3, 3)] + vp_matrix[(3, 2)],
            },
            // Far
            Plane {
                normal: Vec3::new(vp_matrix[(0, 3)] - vp_matrix[(0, 2)],
                                 vp_matrix[(1, 3)] - vp_matrix[(1, 2)],
                                 vp_matrix[(2, 3)] - vp_matrix[(2, 2)]).normalize(),
                distance: vp_matrix[(3, 3)] - vp_matrix[(3, 2)],
            },
        ];

        Self { planes }
    }

    pub fn contains_sphere(&self, sphere: &Sphere) -> bool {
        for plane in &self.planes {
            if plane.distance_to_point(&sphere.center) < -sphere.radius {
                return false;
            }
        }
        true
    }

    pub fn contains_box(&self, bbox: &AABox) -> bool {
        for plane in &self.planes {
            let mut out = 0;
            out += if plane.distance_to_point(&Vec3::new(bbox.min.x, bbox.min.y, bbox.min.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.max.x, bbox.min.y, bbox.min.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.min.x, bbox.max.y, bbox.min.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.max.x, bbox.max.y, bbox.min.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.min.x, bbox.min.y, bbox.max.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.max.x, bbox.min.y, bbox.max.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.min.x, bbox.max.y, bbox.max.z)) < 0.0 { 1 } else { 0 };
            out += if plane.distance_to_point(&Vec3::new(bbox.max.x, bbox.max.y, bbox.max.z)) < 0.0 { 1 } else { 0 };

            if out == 8 {
                return false;
            }
        }
        true
    }
}
