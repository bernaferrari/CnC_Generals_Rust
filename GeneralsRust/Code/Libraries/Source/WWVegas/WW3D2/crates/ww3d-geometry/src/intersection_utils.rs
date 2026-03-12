//! Intersection and collision detection utilities.
//!
//! Provides various geometric intersection tests ported from the C++ WW3D codebase.

use glam::Vec3;

/// Represents a ray for intersection testing.
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

/// Represents a plane in 3D space.
#[derive(Debug, Clone, Copy)]
pub struct Plane {
    pub normal: Vec3,
    pub distance: f32, // Distance from origin along normal
}

impl Plane {
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self {
            normal: normal.normalize(),
            distance,
        }
    }

    pub fn from_point_and_normal(point: Vec3, normal: Vec3) -> Self {
        let normal = normal.normalize();
        let distance = point.dot(normal);
        Self { normal, distance }
    }

    pub fn signed_distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }
}

/// Represents a triangle for intersection testing.
#[derive(Debug, Clone, Copy)]
pub struct Triangle {
    pub v0: Vec3,
    pub v1: Vec3,
    pub v2: Vec3,
}

impl Triangle {
    pub fn new(v0: Vec3, v1: Vec3, v2: Vec3) -> Self {
        Self { v0, v1, v2 }
    }

    pub fn normal(&self) -> Vec3 {
        let edge1 = self.v1 - self.v0;
        let edge2 = self.v2 - self.v0;
        edge1.cross(edge2).normalize()
    }
}

/// Represents a sphere for intersection testing.
#[derive(Debug, Clone, Copy)]
pub struct Sphere {
    pub center: Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }
}

/// Represents an oriented bounding box (OBB).
#[derive(Debug, Clone, Copy)]
pub struct OBBox {
    pub center: Vec3,
    pub extents: Vec3,   // Half-sizes along each axis
    pub axes: [Vec3; 3], // Orthonormal axes
}

impl OBBox {
    pub fn new(center: Vec3, extents: Vec3, axes: [Vec3; 3]) -> Self {
        Self {
            center,
            extents,
            axes,
        }
    }
}

/// Ray-plane intersection test.
///
/// Returns Some(t) if intersection occurs, where t is the distance along the ray.
/// Returns None if ray is parallel to plane or intersection is behind ray origin.
///
/// # C++ Reference
/// Common pattern in `intersec.cpp`
pub fn ray_plane_intersection(ray: Ray, plane: Plane) -> Option<f32> {
    let denom = plane.normal.dot(ray.direction);

    // Check if ray is parallel to plane
    if denom.abs() < 1e-6 {
        return None;
    }

    let t = (plane.distance - plane.normal.dot(ray.origin)) / denom;

    // Check if intersection is behind ray origin
    if t < 0.0 {
        return None;
    }

    Some(t)
}

/// Ray-triangle intersection using Moller-Trumbore algorithm.
///
/// Returns Some(t) if intersection occurs, where t is the distance along the ray.
///
/// # C++ Reference
/// `intersec.cpp`: Triangle intersection tests
pub fn ray_triangle_intersection(ray: Ray, triangle: Triangle) -> Option<f32> {
    const EPSILON: f32 = 1e-6;

    let edge1 = triangle.v1 - triangle.v0;
    let edge2 = triangle.v2 - triangle.v0;

    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    // Ray is parallel to triangle
    if a.abs() < EPSILON {
        return None;
    }

    let f = 1.0 / a;
    let s = ray.origin - triangle.v0;
    let u = f * s.dot(h);

    if u < 0.0 || u > 1.0 {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    // Compute t to find out where intersection point is on the line
    let t = f * edge2.dot(q);

    if t > EPSILON {
        Some(t)
    } else {
        None
    }
}

/// Ray-sphere intersection test.
///
/// Returns Some((t_near, t_far)) if intersection occurs.
///
/// # C++ Reference
/// `intersec.cpp`: Sphere intersection tests
pub fn ray_sphere_intersection(ray: Ray, sphere: Sphere) -> Option<(f32, f32)> {
    let oc = ray.origin - sphere.center;
    let a = ray.direction.length_squared();
    let half_b = oc.dot(ray.direction);
    let c = oc.length_squared() - sphere.radius * sphere.radius;
    let discriminant = half_b * half_b - a * c;

    if discriminant < 0.0 {
        return None;
    }

    let sqrt_d = discriminant.sqrt();
    let t_near = (-half_b - sqrt_d) / a;
    let t_far = (-half_b + sqrt_d) / a;

    Some((t_near, t_far))
}

/// Sphere-sphere intersection test.
///
/// # C++ Reference
/// Common collision detection pattern
pub fn sphere_sphere_intersection(s1: Sphere, s2: Sphere) -> bool {
    let distance_sq = (s1.center - s2.center).length_squared();
    let radius_sum = s1.radius + s2.radius;
    distance_sq <= radius_sum * radius_sum
}

/// Point-in-sphere test.
pub fn point_in_sphere(point: Vec3, sphere: Sphere) -> bool {
    (point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

/// Point-in-triangle test (2D, assumes points are coplanar).
///
/// Uses barycentric coordinates.
pub fn point_in_triangle(point: Vec3, triangle: Triangle) -> bool {
    let v0 = triangle.v2 - triangle.v0;
    let v1 = triangle.v1 - triangle.v0;
    let v2 = point - triangle.v0;

    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);

    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;

    (u >= 0.0) && (v >= 0.0) && (u + v <= 1.0)
}

/// Computes barycentric coordinates of a point on a triangle.
///
/// Returns (u, v, w) where point = u*v0 + v*v1 + w*v2 and u+v+w=1
pub fn barycentric_coords(point: Vec3, triangle: Triangle) -> (f32, f32, f32) {
    let v0 = triangle.v1 - triangle.v0;
    let v1 = triangle.v2 - triangle.v0;
    let v2 = point - triangle.v0;

    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);

    let denom = d00 * d11 - d01 * d01;
    let v = (d11 * d20 - d01 * d21) / denom;
    let w = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - w;

    (u, v, w)
}

/// OBB-OBB intersection test using Separating Axis Theorem (SAT).
///
/// # C++ Reference
/// `intersec.cpp`: Oriented bounding box collision
pub fn obb_obb_intersection(obb1: OBBox, obb2: OBBox) -> bool {
    // Compute rotation matrix from obb1 to obb2
    let mut r = [[0.0f32; 3]; 3];
    let mut abs_r = [[0.0f32; 3]; 3];

    for i in 0..3 {
        for j in 0..3 {
            r[i][j] = obb1.axes[i].dot(obb2.axes[j]);
            abs_r[i][j] = r[i][j].abs() + 1e-6;
        }
    }

    // Compute translation vector
    let t_raw = obb2.center - obb1.center;
    let t = [
        t_raw.dot(obb1.axes[0]),
        t_raw.dot(obb1.axes[1]),
        t_raw.dot(obb1.axes[2]),
    ];

    // Test axes from obb1
    for i in 0..3 {
        let ra = obb1.extents[i];
        let rb = obb2.extents.x * abs_r[i][0]
            + obb2.extents.y * abs_r[i][1]
            + obb2.extents.z * abs_r[i][2];

        if t[i].abs() > ra + rb {
            return false;
        }
    }

    // Test axes from obb2
    for i in 0..3 {
        let ra = obb1.extents.x * abs_r[0][i]
            + obb1.extents.y * abs_r[1][i]
            + obb1.extents.z * abs_r[2][i];
        let rb = obb2.extents[i];

        let test = t[0] * r[0][i] + t[1] * r[1][i] + t[2] * r[2][i];
        if test.abs() > ra + rb {
            return false;
        }
    }

    // Test cross products of axes
    // L = A0 x B0
    let ra = obb1.extents.y * abs_r[2][0] + obb1.extents.z * abs_r[1][0];
    let rb = obb2.extents.y * abs_r[0][2] + obb2.extents.z * abs_r[0][1];
    if (t[2] * r[1][0] - t[1] * r[2][0]).abs() > ra + rb {
        return false;
    }

    // L = A0 x B1
    let ra = obb1.extents.y * abs_r[2][1] + obb1.extents.z * abs_r[1][1];
    let rb = obb2.extents.x * abs_r[0][2] + obb2.extents.z * abs_r[0][0];
    if (t[2] * r[1][1] - t[1] * r[2][1]).abs() > ra + rb {
        return false;
    }

    // L = A0 x B2
    let ra = obb1.extents.y * abs_r[2][2] + obb1.extents.z * abs_r[1][2];
    let rb = obb2.extents.x * abs_r[0][1] + obb2.extents.y * abs_r[0][0];
    if (t[2] * r[1][2] - t[1] * r[2][2]).abs() > ra + rb {
        return false;
    }

    // L = A1 x B0
    let ra = obb1.extents.x * abs_r[2][0] + obb1.extents.z * abs_r[0][0];
    let rb = obb2.extents.y * abs_r[1][2] + obb2.extents.z * abs_r[1][1];
    if (t[0] * r[2][0] - t[2] * r[0][0]).abs() > ra + rb {
        return false;
    }

    // L = A1 x B1
    let ra = obb1.extents.x * abs_r[2][1] + obb1.extents.z * abs_r[0][1];
    let rb = obb2.extents.x * abs_r[1][2] + obb2.extents.z * abs_r[1][0];
    if (t[0] * r[2][1] - t[2] * r[0][1]).abs() > ra + rb {
        return false;
    }

    // L = A1 x B2
    let ra = obb1.extents.x * abs_r[2][2] + obb1.extents.z * abs_r[0][2];
    let rb = obb2.extents.x * abs_r[1][1] + obb2.extents.y * abs_r[1][0];
    if (t[0] * r[2][2] - t[2] * r[0][2]).abs() > ra + rb {
        return false;
    }

    // L = A2 x B0
    let ra = obb1.extents.x * abs_r[1][0] + obb1.extents.y * abs_r[0][0];
    let rb = obb2.extents.y * abs_r[2][2] + obb2.extents.z * abs_r[2][1];
    if (t[1] * r[0][0] - t[0] * r[1][0]).abs() > ra + rb {
        return false;
    }

    // L = A2 x B1
    let ra = obb1.extents.x * abs_r[1][1] + obb1.extents.y * abs_r[0][1];
    let rb = obb2.extents.x * abs_r[2][2] + obb2.extents.z * abs_r[2][0];
    if (t[1] * r[0][1] - t[0] * r[1][1]).abs() > ra + rb {
        return false;
    }

    // L = A2 x B2
    let ra = obb1.extents.x * abs_r[1][2] + obb1.extents.y * abs_r[0][2];
    let rb = obb2.extents.x * abs_r[2][1] + obb2.extents.y * abs_r[2][0];
    if (t[1] * r[0][2] - t[0] * r[1][2]).abs() > ra + rb {
        return false;
    }

    // No separating axis found
    true
}

/// Closest point on a line segment to a given point.
pub fn closest_point_on_segment(point: Vec3, segment_start: Vec3, segment_end: Vec3) -> Vec3 {
    let segment = segment_end - segment_start;
    let t = (point - segment_start).dot(segment) / segment.length_squared();
    let t_clamped = t.clamp(0.0, 1.0);
    segment_start + segment * t_clamped
}

/// Distance from a point to a line segment.
pub fn point_segment_distance(point: Vec3, segment_start: Vec3, segment_end: Vec3) -> f32 {
    let closest = closest_point_on_segment(point, segment_start, segment_end);
    (point - closest).length()
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1e-5;

    #[test]
    fn test_ray_plane_intersection() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let plane =
            Plane::from_point_and_normal(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0));

        let t = ray_plane_intersection(ray, plane).unwrap();
        assert!((t - 5.0).abs() < EPSILON);
    }

    #[test]
    fn test_ray_sphere_intersection() {
        let ray = Ray::new(Vec3::new(0.0, 0.0, -10.0), Vec3::new(0.0, 0.0, 1.0));
        let sphere = Sphere::new(Vec3::ZERO, 1.0);

        let (t_near, t_far) = ray_sphere_intersection(ray, sphere).unwrap();
        assert!((t_near - 9.0).abs() < EPSILON);
        assert!((t_far - 11.0).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_sphere_intersection() {
        let s1 = Sphere::new(Vec3::new(0.0, 0.0, 0.0), 1.0);
        let s2 = Sphere::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
        let s3 = Sphere::new(Vec3::new(3.0, 0.0, 0.0), 1.0);

        assert!(sphere_sphere_intersection(s1, s2));
        assert!(!sphere_sphere_intersection(s1, s3));
    }

    #[test]
    fn test_point_in_sphere() {
        let sphere = Sphere::new(Vec3::ZERO, 5.0);

        assert!(point_in_sphere(Vec3::new(3.0, 0.0, 0.0), sphere));
        assert!(!point_in_sphere(Vec3::new(6.0, 0.0, 0.0), sphere));
    }

    #[test]
    fn test_ray_triangle_intersection() {
        let triangle = Triangle::new(
            Vec3::new(-1.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        // Ray hits triangle
        let ray1 = Ray::new(Vec3::new(0.0, 0.3, -1.0), Vec3::new(0.0, 0.0, 1.0));
        assert!(ray_triangle_intersection(ray1, triangle).is_some());

        // Ray misses triangle
        let ray2 = Ray::new(Vec3::new(2.0, 0.3, -1.0), Vec3::new(0.0, 0.0, 1.0));
        assert!(ray_triangle_intersection(ray2, triangle).is_none());
    }

    #[test]
    fn test_barycentric_coords() {
        let triangle = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        let point = Vec3::new(0.25, 0.25, 0.0);
        let (u, v, w) = barycentric_coords(point, triangle);

        // Coordinates should sum to 1
        assert!((u + v + w - 1.0).abs() < EPSILON);

        // Reconstruct point from barycentric coords
        let reconstructed = triangle.v0 * u + triangle.v1 * v + triangle.v2 * w;
        assert!((reconstructed - point).length() < EPSILON);
    }

    #[test]
    fn test_closest_point_on_segment() {
        let start = Vec3::new(0.0, 0.0, 0.0);
        let end = Vec3::new(10.0, 0.0, 0.0);

        // Point on segment
        let p1 = Vec3::new(5.0, 5.0, 0.0);
        let closest1 = closest_point_on_segment(p1, start, end);
        assert!((closest1 - Vec3::new(5.0, 0.0, 0.0)).length() < EPSILON);

        // Point beyond end
        let p2 = Vec3::new(15.0, 0.0, 0.0);
        let closest2 = closest_point_on_segment(p2, start, end);
        assert!((closest2 - end).length() < EPSILON);
    }
}
