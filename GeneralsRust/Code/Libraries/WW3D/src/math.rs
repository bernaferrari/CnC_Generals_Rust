// Math utilities for W3D system
// Ported from vector.h, matrix3d.h, quat.h

use nalgebra::{Matrix4, Vector3, Vector4, Quaternion, UnitQuaternion};
use bytemuck::{Pod, Zeroable};

pub type Vec3 = Vector3<f32>;
pub type Vec4 = Vector4<f32>;
pub type Mat4 = Matrix4<f32>;
pub type Quat = Quaternion<f32>;
pub type UnitQuat = UnitQuaternion<f32>;

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vector3i16 {
    pub x: i16,
    pub y: i16,
    pub z: i16,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vector2 {
    pub x: f32,
    pub y: f32,
}

// AABox - Axis-Aligned Bounding Box
#[derive(Debug, Clone, Copy)]
pub struct AABox {
    pub min: Vec3,
    pub max: Vec3,
}

impl AABox {
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self {
                min: Vec3::zeros(),
                max: Vec3::zeros(),
            };
        }

        let mut min = points[0];
        let mut max = points[0];

        for point in points.iter().skip(1) {
            min = Vec3::new(
                min.x.min(point.x),
                min.y.min(point.y),
                min.z.min(point.z),
            );
            max = Vec3::new(
                max.x.max(point.x),
                max.y.max(point.y),
                max.z.max(point.z),
            );
        }

        Self { min, max }
    }

    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    pub fn extents(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    pub fn contains_point(&self, point: &Vec3) -> bool {
        point.x >= self.min.x && point.x <= self.max.x &&
        point.y >= self.min.y && point.y <= self.max.y &&
        point.z >= self.min.z && point.z <= self.max.z
    }

    pub fn intersects(&self, other: &AABox) -> bool {
        self.min.x <= other.max.x && self.max.x >= other.min.x &&
        self.min.y <= other.max.y && self.max.y >= other.min.y &&
        self.min.z <= other.max.z && self.max.z >= other.min.z
    }

    pub fn transform(&self, transform: &Mat4) -> Self {
        let corners = [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ];

        let transformed: Vec<Vec3> = corners.iter()
            .map(|c| transform_point(transform, c))
            .collect();

        Self::from_points(&transformed)
    }
}

// Sphere - Bounding Sphere
#[derive(Debug, Clone, Copy)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self {
                center: Vec3::zeros(),
                radius: 0.0,
            };
        }

        // Simple center-based approach
        let mut center = Vec3::zeros();
        for point in points {
            center += point;
        }
        center /= points.len() as f32;

        let mut radius = 0.0;
        for point in points {
            let dist = (point - center).norm();
            if dist > radius {
                radius = dist;
            }
        }

        Self { center, radius }
    }

    pub fn contains_point(&self, point: &Vec3) -> bool {
        (point - self.center).norm_squared() <= self.radius * self.radius
    }

    pub fn intersects(&self, other: &Sphere) -> bool {
        let dist = (self.center - other.center).norm();
        dist <= (self.radius + other.radius)
    }

    pub fn transform(&self, transform: &Mat4) -> Self {
        let center = transform_point(transform, &self.center);

        // Extract scale from transform
        let scale_x = transform.column(0).xyz().norm();
        let scale_y = transform.column(1).xyz().norm();
        let scale_z = transform.column(2).xyz().norm();
        let max_scale = scale_x.max(scale_y).max(scale_z);

        Self {
            center,
            radius: self.radius * max_scale,
        }
    }
}

// OBBox - Oriented Bounding Box
#[derive(Debug, Clone, Copy)]
pub struct OBBox {
    pub center: Vec3,
    pub extents: Vec3,
    pub axes: [Vec3; 3],
}

impl OBBox {
    pub fn new(center: Vec3, extents: Vec3, axes: [Vec3; 3]) -> Self {
        Self { center, extents, axes }
    }

    pub fn from_aabox(aabox: &AABox) -> Self {
        Self {
            center: aabox.center(),
            extents: aabox.extents(),
            axes: [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
        }
    }

    pub fn transform(&self, transform: &Mat4) -> Self {
        let center = transform_point(transform, &self.center);
        let axes = [
            transform_vector(transform, &self.axes[0]).normalize(),
            transform_vector(transform, &self.axes[1]).normalize(),
            transform_vector(transform, &self.axes[2]).normalize(),
        ];

        // Calculate new extents based on transform scale
        let scale_0 = transform_vector(transform, &self.axes[0]).norm();
        let scale_1 = transform_vector(transform, &self.axes[1]).norm();
        let scale_2 = transform_vector(transform, &self.axes[2]).norm();

        let extents = Vec3::new(
            self.extents.x * scale_0,
            self.extents.y * scale_1,
            self.extents.z * scale_2,
        );

        Self { center, extents, axes }
    }

    pub fn contains_point(&self, point: &Vec3) -> bool {
        let p = point - self.center;
        for i in 0..3 {
            let dist = p.dot(&self.axes[i]);
            if dist.abs() > self.extents[i] {
                return false;
            }
        }
        true
    }
}

// Plane
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub distance: f32,
}

impl Plane {
    pub fn from_points(p0: &Vec3, p1: &Vec3, p2: &Vec3) -> Self {
        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let normal = v1.cross(&v2).normalize();
        let distance = -normal.dot(p0);
        Self { normal, distance }
    }

    pub fn from_normal_and_point(normal: Vec3, point: &Vec3) -> Self {
        let distance = -normal.dot(point);
        Self { normal, distance }
    }

    pub fn distance_to_point(&self, point: &Vec3) -> f32 {
        self.normal.dot(point) + self.distance
    }

    pub fn classify_point(&self, point: &Vec3, epsilon: f32) -> i32 {
        let dist = self.distance_to_point(point);
        if dist > epsilon {
            1  // Front
        } else if dist < -epsilon {
            -1 // Back
        } else {
            0  // On plane
        }
    }
}

// Ray
#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

// Utility functions for transform operations
pub fn transform_point(transform: &Mat4, point: &Vec3) -> Vec3 {
    let v4 = transform * Vec4::new(point.x, point.y, point.z, 1.0);
    Vec3::new(v4.x, v4.y, v4.z)
}

pub fn transform_vector(transform: &Mat4, vector: &Vec3) -> Vec3 {
    let v4 = transform * Vec4::new(vector.x, vector.y, vector.z, 0.0);
    Vec3::new(v4.x, v4.y, v4.z)
}

pub fn inverse_transform_point(transform: &Mat4, point: &Vec3) -> Vec3 {
    if let Some(inv) = transform.try_inverse() {
        transform_point(&inv, point)
    } else {
        *point
    }
}

pub fn inverse_transform_vector(transform: &Mat4, vector: &Vec3) -> Vec3 {
    if let Some(inv) = transform.try_inverse() {
        transform_vector(&inv, vector)
    } else {
        *vector
    }
}

// Matrix utilities
pub fn matrix_from_rotation_translation(rotation: &UnitQuat, translation: &Vec3) -> Mat4 {
    let mut mat = rotation.to_homogeneous();
    mat[(0, 3)] = translation.x;
    mat[(1, 3)] = translation.y;
    mat[(2, 3)] = translation.z;
    mat
}

pub fn decompose_matrix(matrix: &Mat4) -> (Vec3, UnitQuat, Vec3) {
    // Extract translation
    let translation = Vec3::new(matrix[(0, 3)], matrix[(1, 3)], matrix[(2, 3)]);

    // Extract scale
    let scale_x = Vec3::new(matrix[(0, 0)], matrix[(1, 0)], matrix[(2, 0)]).norm();
    let scale_y = Vec3::new(matrix[(0, 1)], matrix[(1, 1)], matrix[(2, 1)]).norm();
    let scale_z = Vec3::new(matrix[(0, 2)], matrix[(1, 2)], matrix[(2, 2)]).norm();
    let scale = Vec3::new(scale_x, scale_y, scale_z);

    // Extract rotation
    let rot_matrix = Matrix4::new(
        matrix[(0, 0)] / scale_x, matrix[(0, 1)] / scale_y, matrix[(0, 2)] / scale_z, 0.0,
        matrix[(1, 0)] / scale_x, matrix[(1, 1)] / scale_y, matrix[(1, 2)] / scale_z, 0.0,
        matrix[(2, 0)] / scale_x, matrix[(2, 1)] / scale_y, matrix[(2, 2)] / scale_z, 0.0,
        0.0, 0.0, 0.0, 1.0,
    );

    let rotation = UnitQuat::from_matrix(&rot_matrix.fixed_slice::<3, 3>(0, 0).into());

    (translation, rotation, scale)
}

// Interpolation
pub fn lerp_vec3(a: &Vec3, b: &Vec3, t: f32) -> Vec3 {
    a + (b - a) * t
}

pub fn slerp_quat(a: &UnitQuat, b: &UnitQuat, t: f32) -> UnitQuat {
    a.slerp(b, t)
}
