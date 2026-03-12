#![allow(dead_code)]
//! WW3D Collision Detection System
//!
//! This crate provides collision detection and spatial partitioning for the WW3D engine.
//! It includes ray casting, bounding volume hierarchies, and collision queries.

use glam::Vec3;

pub mod aabtree;
pub mod aabtree_builder;
pub mod aabtree_extended;
pub mod bounding_volumes;
pub mod collision_math;
pub mod collision_system;
pub mod collision_tests;
pub mod intersection;
pub mod physics_integration;
pub mod screen_picking;
pub mod spatial_hash;
pub mod w3d_io;

pub mod tree {
    pub use crate::aabtree::*;
}

pub mod volumes {
    pub use crate::bounding_volumes::*;
}

pub mod system {
    pub use crate::collision_system::*;
}

pub mod test_support {
    pub use crate::collision_tests::*;
}

pub mod intersection_api {
    pub use crate::intersection::*;
}

pub mod spatial {
    pub use crate::spatial_hash::*;
}

pub use crate::bounding_volumes::{
    aabox::AABoxClass,
    obbox::OBBoxClass,
    plane::{PlaneClass, PlaneClassification},
    sphere::SphereClass,
    AABox, OBBox, Plane, Sphere,
};

/// Ray-AABB intersection test
/// Returns true if the ray intersects the AABB
pub fn ray_aabb_intersect(ray_origin: Vec3, ray_dir: Vec3, aabb: &AABox) -> bool {
    let mut tmin: f32 = 0.0;
    let mut tmax: f32 = f32::MAX;

    // For AABox with center/extent representation, compute min/max
    let min = aabb.center - aabb.extent;
    let max = aabb.center + aabb.extent;

    // X axis
    if ray_dir.x != 0.0 {
        let inv_dx = 1.0 / ray_dir.x;
        let tx1 = (min.x - ray_origin.x) * inv_dx;
        let tx2 = (max.x - ray_origin.x) * inv_dx;
        let (txnear, txfar) = if tx1 < tx2 { (tx1, tx2) } else { (tx2, tx1) };
        tmin = tmin.max(txnear);
        tmax = tmax.min(txfar);
        if tmax <= tmin {
            return false;
        }
    }

    // Y axis
    if ray_dir.y != 0.0 {
        let inv_dy = 1.0 / ray_dir.y;
        let ty1 = (min.y - ray_origin.y) * inv_dy;
        let ty2 = (max.y - ray_origin.y) * inv_dy;
        let (tynear, tyfar) = if ty1 < ty2 { (ty1, ty2) } else { (ty2, ty1) };
        tmin = tmin.max(tynear);
        tmax = tmax.min(tyfar);
        if tmax <= tmin {
            return false;
        }
    }

    // Z axis
    if ray_dir.z != 0.0 {
        let inv_dz = 1.0 / ray_dir.z;
        let tz1 = (min.z - ray_origin.z) * inv_dz;
        let tz2 = (max.z - ray_origin.z) * inv_dz;
        let (tznear, tzfar) = if tz1 < tz2 { (tz1, tz2) } else { (tz2, tz1) };
        tmin = tmin.max(tznear);
        tmax = tmax.min(tzfar);
        if tmax <= tmin {
            return false;
        }
    }

    tmin < tmax && tmax > 0.0
}

/// AABB-AABB intersection test
pub fn aabb_aabb_intersect(a: &AABox, b: &AABox) -> bool {
    let a_min = a.center - a.extent;
    let a_max = a.center + a.extent;
    let b_min = b.center - b.extent;
    let b_max = b.center + b.extent;

    a_min.x <= b_max.x
        && a_max.x >= b_min.x
        && a_min.y <= b_max.y
        && a_max.y >= b_min.y
        && a_min.z <= b_max.z
        && a_max.z >= b_min.z
}

/// Point-AABB containment test
pub fn point_in_aabb(point: Vec3, aabb: &AABox) -> bool {
    let min = aabb.center - aabb.extent;
    let max = aabb.center + aabb.extent;

    point.x >= min.x
        && point.x <= max.x
        && point.y >= min.y
        && point.y <= max.y
        && point.z >= min.z
        && point.z <= max.z
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_aabb_intersect() {
        let aabb = AABox {
            center: Vec3::new(0.0, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        // Ray from origin towards positive x (hits)
        assert!(ray_aabb_intersect(
            Vec3::ZERO,
            Vec3::new(1.0, 0.0, 0.0),
            &aabb
        ));
        // Ray parallel to y-axis but outside and away (misses)
        assert!(!ray_aabb_intersect(
            Vec3::new(0.0, 2.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
            &aabb
        ));
    }

    #[test]
    fn test_aabb_aabb_intersect() {
        let a = AABox {
            center: Vec3::new(0.0, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        let b = AABox {
            center: Vec3::new(1.5, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        // Should intersect (touching edges)
        assert!(aabb_aabb_intersect(&a, &b));

        let c = AABox {
            center: Vec3::new(3.0, 0.0, 0.0),
            extent: Vec3::new(1.0, 1.0, 1.0),
        };
        // Should not intersect
        assert!(!aabb_aabb_intersect(&a, &c));
    }

    #[test]
    fn test_sphere_ray_intersect() {
        let sphere = Sphere::new(Vec3::ZERO, 1.0);
        // Ray through center should hit
        assert!(sphere.ray_intersects(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0)));
        // Ray missing sphere should not hit
        assert!(!sphere.ray_intersects(Vec3::new(0.0, 2.0, 0.0), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_sphere_intersect() {
        let a = Sphere::new(Vec3::ZERO, 1.0);
        let b = Sphere::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
        // Should intersect (touching)
        assert!(a.intersects_sphere(&b));

        let c = Sphere::new(Vec3::new(3.0, 0.0, 0.0), 1.0);
        // Should not intersect
        assert!(!a.intersects_sphere(&c));
    }

    #[test]
    fn test_plane_classification() {
        let plane = Plane::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(
            plane.classify_point(Vec3::new(0.0, 1.0, 0.0)),
            PlaneClassification::Front
        );
        assert_eq!(
            plane.classify_point(Vec3::new(0.0, -1.0, 0.0)),
            PlaneClassification::Back
        );
        assert_eq!(
            plane.classify_point(Vec3::ZERO),
            PlaneClassification::OnPlane
        );
    }
}
