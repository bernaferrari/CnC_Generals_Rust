#![allow(dead_code)]
//! WW3D Collision Detection System
//!
//! This crate provides collision detection and spatial partitioning for the WW3D engine.
//! It includes ray casting, bounding volume hierarchies, and collision queries.
//!
//! Mesh collision dispatch belongs to renderer mesh/model owners, mirroring
//! C++ `MeshClass` and `MeshGeometryClass`; this crate does not expose a separate
//! world collision dispatcher.

use glam::Vec3;

pub mod aabtree;
pub mod aabtree_builder;
pub mod aabtree_extended;
pub mod bounding_volumes;
pub mod collision_math;
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
    AABox, OBBox, Plane, Sphere,
    aabox::AABoxClass,
    obbox::OBBoxClass,
    plane::{PlaneClass, PlaneClassification},
    sphere::SphereClass,
};

/// Test an infinite forward ray against a closed AABB.
///
/// Uses the `IntersectionClass::Intersect_Box` algorithm from C++ `intersec.cpp`.
/// Boundary origins and tangencies count as hits. The direction need not be
/// normalized; a zero direction hits only when the origin is inside the box.
pub fn ray_aabb_intersect(ray_origin: Vec3, ray_dir: Vec3, aabb: &AABox) -> bool {
    let intersection = intersection::IntersectionClass {
        ray_location: ray_origin,
        ray_direction: ray_dir,
        ..Default::default()
    };
    intersection
        .intersect_box(aabb.center - aabb.extent, aabb.center + aabb.extent)
        .intersects
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
    fn test_ray_aabb_intersect_parallel_outside_slabs() {
        let aabb = AABox {
            center: Vec3::ZERO,
            extent: Vec3::ONE,
        };
        for axis in 0..3 {
            for side in [-1.0, 1.0] {
                for parallel in [0.0, -0.0] {
                    let mut origin = Vec3::ZERO;
                    let mut direction = Vec3::ZERO;
                    origin[axis] = side * 10.0;
                    origin[(axis + 1) % 3] = -5.0;
                    direction[axis] = parallel;
                    direction[(axis + 1) % 3] = 1.0;
                    assert!(!ray_aabb_intersect(origin, direction, &aabb));
                }
            }
        }
    }

    #[test]
    fn test_ray_aabb_intersect_closed_boundaries() {
        let aabb = AABox {
            center: Vec3::ZERO,
            extent: Vec3::ONE,
        };
        // C++ intersec.cpp:175 accepts origins inside or on the boundary,
        // including stationary rays and rays pointing away from that boundary.
        assert!(ray_aabb_intersect(Vec3::ZERO, Vec3::ZERO, &aabb));
        assert!(ray_aabb_intersect(Vec3::X, Vec3::X, &aabb));
        assert!(ray_aabb_intersect(Vec3::ONE, Vec3::ZERO, &aabb));
        assert!(!ray_aabb_intersect(Vec3::splat(2.0), Vec3::ZERO, &aabb));
        // C++ intersec.cpp:202 rejects only strictly outside coordinates:
        // this ray touches (-1, 1, 0) without entering the box interior.
        assert!(ray_aabb_intersect(
            Vec3::new(-2.0, 0.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            &aabb,
        ));
        assert!(ray_aabb_intersect(
            Vec3::new(-2.0, 1.0, 0.0),
            Vec3::X,
            &aabb,
        ));
        assert!(!ray_aabb_intersect(
            Vec3::new(-2.0, 0.0, 0.0),
            -Vec3::X,
            &aabb,
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
