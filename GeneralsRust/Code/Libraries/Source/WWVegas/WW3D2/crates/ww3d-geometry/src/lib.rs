//! Core geometry primitives and utilities for WW3D.
//!
//! This module mirrors the layout of the original WW3D C++ geometry
//! layer, exposing the bounding volumes and primitive types expected by
//! higher level systems (collision, rendering, spatial partitioning).
//! Keeping these definitions central avoids each submodule re-defining
//! incompatible representations.

pub mod aabtree;
pub mod bounding_volumes;
pub mod collision;
pub mod decal_mesh;
pub mod dynamic_mesh;
pub mod hlod;
pub mod intersection;
pub mod intersection_utils;
pub mod math_utils;
pub mod mesh_builder;
pub mod mesh_damage;
pub mod mesh_geometry;
pub mod mesh_loader;
pub mod mesh_mat_desc;
pub mod mesh_optimizer;
pub mod primitive_animation;
pub mod render_info;
pub mod simd_math;
pub mod spatial_partitioning;
pub mod texture_mapper;

pub mod meshgeometry;
pub mod meshmatdesc;
pub use aabtree::*;
pub use bounding_volumes::*;
pub use collision::*;
pub use decal_mesh::*;
pub use dynamic_mesh::*;
pub use hlod::*;
// Both modules export similar functions - intersection_utils provides optimized versions
#[allow(ambiguous_glob_reexports)]
pub use intersection::*;
#[allow(ambiguous_glob_reexports)]
pub use intersection_utils::*;
pub use math_utils::*;
pub use mesh_builder::*;
pub use mesh_damage::*;
pub use mesh_geometry::*;
pub use mesh_loader::*;
pub use mesh_mat_desc::*;
pub use mesh_optimizer::*;
pub use primitive_animation::*;
pub use render_info::*;
pub use simd_math::*;
pub use spatial_partitioning::*;
pub use texture_mapper::*;

use glam::Vec4;
pub use glam::{Mat4, Vec3};

/// Convenience alias matching legacy naming.
pub type Vector3 = Vec3;

/// Small epsilon used across intersection tests.
pub const EPSILON: f32 = 0.0001;
pub const INFINITY: f32 = f32::INFINITY;
pub const PI: f32 = std::f32::consts::PI;
pub const TWO_PI: f32 = 2.0 * PI;
pub const HALF_PI: f32 = PI / 2.0;

/// Axis-aligned bounding box expressed with centre and half extent,
/// matching the original `AABoxClass`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct AABox {
    pub center: Vector3,
    pub extent: Vector3,
}

impl AABox {
    pub const ZERO: Self = Self {
        center: Vector3::ZERO,
        extent: Vector3::ZERO,
    };

    #[must_use]
    pub fn new(center: Vector3, extent: Vector3) -> Self {
        Self { center, extent }
    }

    #[must_use]
    pub fn from_min_max(min: Vector3, max: Vector3) -> Self {
        Self {
            center: (min + max) * 0.5,
            extent: (max - min) * 0.5,
        }
    }

    #[inline]
    #[must_use]
    pub fn min(&self) -> Vector3 {
        self.center - self.extent
    }

    #[inline]
    #[must_use]
    pub fn max(&self) -> Vector3 {
        self.center + self.extent
    }

    #[inline]
    #[must_use]
    pub fn volume(&self) -> f32 {
        let size = self.max() - self.min();
        size.x * size.y * size.z
    }

    #[must_use]
    pub fn contains_point(&self, point: Vector3) -> bool {
        let min = self.min();
        let max = self.max();
        point.x >= min.x
            && point.x <= max.x
            && point.y >= min.y
            && point.y <= max.y
            && point.z >= min.z
            && point.z <= max.z
    }

    #[must_use]
    pub fn merge_aabbs(&self, other: &AABox) -> AABox {
        let min = self.min().min(other.min());
        let max = self.max().max(other.max());
        AABox::from_min_max(min, max)
    }

    #[must_use]
    pub fn union_cost(&self, other: &AABox) -> f32 {
        self.merge_aabbs(other).volume()
    }

    pub fn get_min(&self) -> Vector3 {
        self.min()
    }

    pub fn get_max(&self) -> Vector3 {
        self.max()
    }

    #[must_use]
    pub fn transform(&self, matrix: &Mat4) -> AABox {
        let extent = self.extent;
        let center = self.center;
        let corners = [
            center + Vector3::new(-extent.x, -extent.y, -extent.z),
            center + Vector3::new(extent.x, -extent.y, -extent.z),
            center + Vector3::new(-extent.x, extent.y, -extent.z),
            center + Vector3::new(extent.x, extent.y, -extent.z),
            center + Vector3::new(-extent.x, -extent.y, extent.z),
            center + Vector3::new(extent.x, -extent.y, extent.z),
            center + Vector3::new(-extent.x, extent.y, extent.z),
            center + Vector3::new(extent.x, extent.y, extent.z),
        ];

        let mut transformed_min = Vector3::splat(f32::INFINITY);
        let mut transformed_max = Vector3::splat(f32::NEG_INFINITY);

        for corner in corners {
            let transformed = matrix.transform_point3(corner);
            transformed_min = transformed_min.min(transformed);
            transformed_max = transformed_max.max(transformed);
        }

        AABox::from_min_max(transformed_min, transformed_max)
    }

    #[inline]
    #[must_use]
    pub fn intersects_aabox(&self, other: &AABox) -> bool {
        self.min().x <= other.max().x
            && self.max().x >= other.min().x
            && self.min().y <= other.max().y
            && self.max().y >= other.min().y
            && self.min().z <= other.max().z
            && self.max().z >= other.min().z
    }

    #[must_use]
    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        let clamped = Vector3::new(
            sphere.center.x.clamp(self.min().x, self.max().x),
            sphere.center.y.clamp(self.min().y, self.max().y),
            sphere.center.z.clamp(self.min().z, self.max().z),
        );

        (clamped - sphere.center).length_squared() <= sphere.radius * sphere.radius
    }
}

/// Oriented bounding box mirroring `OBBoxClass`.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct OBBox {
    pub center: Vector3,
    pub extent: Vector3,
    pub basis: Mat4,
}

impl OBBox {
    #[must_use]
    pub fn new(center: Vector3, extent: Vector3, basis: Mat4) -> Self {
        Self {
            center,
            extent,
            basis,
        }
    }
}

/// Plane equation in normal-distance form.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub normal: Vector3,
    pub distance: f32,
}

impl Plane {
    #[must_use]
    pub fn new(normal: Vector3, distance: f32) -> Self {
        Self { normal, distance }
    }

    #[must_use]
    pub fn from_points(a: Vector3, b: Vector3, c: Vector3) -> Self {
        let normal = (b - a).cross(c - a).normalize();
        let distance = -normal.dot(a);
        Self { normal, distance }
    }

    #[must_use]
    pub fn from_point_normal(point: Vector3, normal: Vector3) -> Self {
        let normalized = normal.normalize();
        let distance = -normalized.dot(point);
        Self {
            normal: normalized,
            distance,
        }
    }

    #[inline]
    #[must_use]
    pub fn distance_to_point(&self, point: Vector3) -> f32 {
        self.normal.dot(point) + self.distance
    }

    #[must_use]
    pub fn classify_point(&self, point: Vector3) -> PlaneClassification {
        let dist = self.distance_to_point(point);
        if dist > EPSILON {
            PlaneClassification::Front
        } else if dist < -EPSILON {
            PlaneClassification::Back
        } else {
            PlaneClassification::OnPlane
        }
    }
}

/// Classification result relative to a plane.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneClassification {
    Front,
    Back,
    OnPlane,
}

/// Bounding sphere primitive.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
}

impl Sphere {
    #[must_use]
    pub fn new(center: Vector3, radius: f32) -> Self {
        Self { center, radius }
    }

    #[must_use]
    pub fn contains_point(&self, point: Vector3) -> bool {
        (point - self.center).length_squared() <= self.radius * self.radius
    }
}

/// Simple triangle primitive used for intersection tests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Triangle {
    pub v0: Vector3,
    pub v1: Vector3,
    pub v2: Vector3,
}

impl Triangle {
    #[must_use]
    pub fn new(v0: Vector3, v1: Vector3, v2: Vector3) -> Self {
        Self { v0, v1, v2 }
    }

    #[must_use]
    pub fn normal(&self) -> Vector3 {
        (self.v1 - self.v0).cross(self.v2 - self.v0).normalize()
    }

    #[must_use]
    pub fn centroid(&self) -> Vector3 {
        (self.v0 + self.v1 + self.v2) / 3.0
    }
}

/// Ray primitive used across intersection routines.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vector3,
    pub direction: Vector3,
}

impl Ray {
    #[must_use]
    pub fn new(origin: Vector3, direction: Vector3) -> Self {
        Self { origin, direction }
    }

    #[must_use]
    pub fn at(&self, distance: f32) -> Vector3 {
        self.origin + self.direction * distance
    }
}

/// Finite line segment.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LineSegment {
    pub start: Vector3,
    pub end: Vector3,
}

impl LineSegment {
    #[must_use]
    pub fn new(start: Vector3, end: Vector3) -> Self {
        Self { start, end }
    }

    #[must_use]
    pub fn direction(&self) -> Vector3 {
        self.end - self.start
    }

    #[must_use]
    pub fn length(&self) -> f32 {
        self.direction().length()
    }
}

/// View frustum represented by six clipping planes.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    pub planes: [Plane; 6],
}

impl Frustum {
    #[must_use]
    pub fn from_matrix(matrix: Mat4) -> Self {
        let row = |i: usize| -> Vec4 {
            let cols = matrix.to_cols_array_2d();
            Vec4::new(cols[0][i], cols[1][i], cols[2][i], cols[3][i])
        };

        let r0 = row(0);
        let r1 = row(1);
        let r2 = row(2);
        let r3 = row(3);

        let make_plane = |sum: Vec4| -> Plane {
            let normal = Vector3::new(sum.x, sum.y, sum.z);
            let length = normal.length();
            if length > 0.0 {
                Plane::new(normal / length, sum.w / length)
            } else {
                Plane::new(normal, sum.w)
            }
        };

        Self {
            planes: [
                make_plane(r3 + r0), // Left
                make_plane(r3 - r0), // Right
                make_plane(r3 + r1), // Bottom
                make_plane(r3 - r1), // Top
                make_plane(r3 + r2), // Near
                make_plane(r3 - r2), // Far
            ],
        }
    }

    #[must_use]
    pub fn contains_point(&self, point: Vector3) -> bool {
        self.planes
            .iter()
            .all(|plane| plane.distance_to_point(point) >= 0.0)
    }

    #[must_use]
    pub fn intersects_aabox(&self, aabb: &AABox) -> bool {
        for plane in &self.planes {
            let positive = Vector3::new(
                if plane.normal.x >= 0.0 {
                    aabb.max().x
                } else {
                    aabb.min().x
                },
                if plane.normal.y >= 0.0 {
                    aabb.max().y
                } else {
                    aabb.min().y
                },
                if plane.normal.z >= 0.0 {
                    aabb.max().z
                } else {
                    aabb.min().z
                },
            );

            if plane.distance_to_point(positive) < 0.0 {
                return false;
            }
        }
        true
    }

    #[must_use]
    pub fn intersects_sphere(&self, sphere: &Sphere) -> bool {
        self.planes
            .iter()
            .all(|plane| plane.distance_to_point(sphere.center) >= -sphere.radius)
    }
}
