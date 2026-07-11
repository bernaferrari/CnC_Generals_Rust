//! Advanced Geometric Intersection Tests
//!
//! This module provides comprehensive intersection testing between various
//! geometric primitives, including rays, lines, planes, and complex shapes.

use crate::*;
use glam::Vec3;

/// Ray intersection result
#[derive(Debug, Clone, Copy)]
pub struct IntersectionResult {
    pub hit: bool,
    pub distance: f32,
    pub point: Vec3,
    pub normal: Vec3,
}

/// Advanced intersection testing functions

/// Test ray-sphere intersection
pub fn ray_sphere_intersection(ray: &Ray, sphere: &Sphere) -> Option<IntersectionResult> {
    let oc = ray.origin - sphere.center;
    let a = ray.direction.dot(ray.direction);
    let b = 2.0 * oc.dot(ray.direction);
    let c = oc.dot(oc) - sphere.radius * sphere.radius;

    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }

    let sqrt_discriminant = discriminant.sqrt();
    let t1 = (-b - sqrt_discriminant) / (2.0 * a);
    let t2 = (-b + sqrt_discriminant) / (2.0 * a);

    let t = if t1 >= 0.0 { t1 } else { t2 };
    if t < 0.0 {
        return None;
    }

    let point = ray.origin + ray.direction * t;
    let normal = (point - sphere.center).normalize();

    Some(IntersectionResult {
        hit: true,
        distance: t,
        point,
        normal,
    })
}

/// Test ray-AABB intersection
pub fn ray_aabb_intersection(ray: &Ray, aabb: &AABox) -> Option<IntersectionResult> {
    let min = aabb.center - aabb.extent;
    let max = aabb.center + aabb.extent;

    let mut tmin: f32 = 0.0;
    let mut tmax: f32 = f32::INFINITY;

    for i in 0..3 {
        let origin = ray.origin[i];
        let direction = ray.direction[i];

        if direction.abs() < EPSILON {
            // Ray is parallel to this axis
            if origin < min[i] || origin > max[i] {
                return None;
            }
        } else {
            let inv_direction = 1.0 / direction;
            let t1 = (min[i] - origin) * inv_direction;
            let t2 = (max[i] - origin) * inv_direction;

            let (t_near, t_far) = if t1 < t2 { (t1, t2) } else { (t2, t1) };

            tmin = tmin.max(t_near);
            tmax = tmax.min(t_far);

            if tmin > tmax {
                return None;
            }
        }
    }

    if tmin < 0.0 {
        return None;
    }

    let point = ray.origin + ray.direction * tmin;
    let normal = calculate_aabb_normal(point, aabb);

    Some(IntersectionResult {
        hit: true,
        distance: tmin,
        point,
        normal,
    })
}

/// Calculate normal at intersection point on AABB
fn calculate_aabb_normal(point: Vec3, aabb: &AABox) -> Vec3 {
    let min = aabb.center - aabb.extent;
    let max = aabb.center + aabb.extent;

    let mut normal = Vec3::ZERO;
    let mut max_distance = f32::NEG_INFINITY;

    let faces = [
        (Vec3::new(1.0, 0.0, 0.0), point.x - min.x),
        (Vec3::new(-1.0, 0.0, 0.0), max.x - point.x),
        (Vec3::new(0.0, 1.0, 0.0), point.y - min.y),
        (Vec3::new(0.0, -1.0, 0.0), max.y - point.y),
        (Vec3::new(0.0, 0.0, 1.0), point.z - min.z),
        (Vec3::new(0.0, 0.0, -1.0), max.z - point.z),
    ];

    for (face_normal, distance) in faces {
        if distance > max_distance {
            max_distance = distance;
            normal = face_normal;
        }
    }

    normal
}

/// Test ray-OBB intersection
pub fn ray_obb_intersection(ray: &Ray, obb: &OBBox) -> Option<IntersectionResult> {
    // Transform ray to OBB local space
    let inv_basis = obb.basis.inverse();
    let local_ray_origin = inv_basis.transform_point3(ray.origin - obb.center);
    let local_ray_direction = inv_basis.transform_vector3(ray.direction);

    let local_aabb = AABox::new(Vec3::ZERO, obb.extent);
    let local_ray = Ray::new(local_ray_origin, local_ray_direction.normalize());

    if let Some(local_result) = ray_aabb_intersection(&local_ray, &local_aabb) {
        // Transform result back to world space
        let world_point = obb.basis.transform_point3(local_result.point) + obb.center;
        let world_normal = obb.basis.transform_vector3(local_result.normal);

        Some(IntersectionResult {
            hit: true,
            distance: local_result.distance,
            point: world_point,
            normal: world_normal.normalize(),
        })
    } else {
        None
    }
}

/// Test ray-plane intersection
pub fn ray_plane_intersection(ray: &Ray, plane: &Plane) -> Option<IntersectionResult> {
    let denom = plane.normal.dot(ray.direction);

    if denom.abs() < EPSILON {
        return None; // Ray is parallel to plane
    }

    let t = -(plane.normal.dot(ray.origin) + plane.distance) / denom;

    if t < 0.0 {
        return None; // Intersection is behind ray origin
    }

    let point = ray.origin + ray.direction * t;

    Some(IntersectionResult {
        hit: true,
        distance: t,
        point,
        normal: plane.normal,
    })
}

/// Test ray-triangle intersection
pub fn ray_triangle_intersection(
    ray: &Ray,
    triangle: &Triangle,
    cull_backface: bool,
) -> Option<IntersectionResult> {
    let edge1 = triangle.v1 - triangle.v0;
    let edge2 = triangle.v2 - triangle.v0;

    let h = ray.direction.cross(edge2);
    let a = edge1.dot(h);

    if cull_backface && a < EPSILON {
        return None;
    }

    if a.abs() < EPSILON {
        return None;
    }

    let f = 1.0 / a;
    let s = ray.origin - triangle.v0;
    let u = f * s.dot(h);

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = s.cross(edge1);
    let v = f * ray.direction.dot(q);

    if v < 0.0 || u + v > 1.0 {
        return None;
    }

    let t = f * edge2.dot(q);

    if t < 0.0 {
        return None;
    }

    let point = ray.origin + ray.direction * t;
    let normal = edge1.cross(edge2).normalize();

    Some(IntersectionResult {
        hit: true,
        distance: t,
        point,
        normal,
    })
}

/// Test line segment-plane intersection
pub fn line_segment_plane_intersection(segment: &LineSegment, plane: &Plane) -> Option<Vec3> {
    let direction = segment.end - segment.start;
    let ray = Ray::new(segment.start, direction.normalize());

    if let Some(result) = ray_plane_intersection(&ray, plane) {
        if result.distance <= direction.length() {
            Some(result.point)
        } else {
            None
        }
    } else {
        None
    }
}

/// Test line segment-triangle intersection
pub fn line_segment_triangle_intersection(
    segment: &LineSegment,
    triangle: &Triangle,
) -> Option<IntersectionResult> {
    let direction = segment.end - segment.start;
    let length = direction.length();
    let ray = Ray::new(segment.start, direction.normalize());

    ray_triangle_intersection(&ray, triangle, false).filter(|&result| result.distance <= length)
}

/// Test sphere-sphere intersection
pub fn sphere_sphere_intersection(sphere1: &Sphere, sphere2: &Sphere) -> bool {
    let distance_squared = (sphere1.center - sphere2.center).length_squared();
    let radius_sum = sphere1.radius + sphere2.radius;
    distance_squared <= radius_sum * radius_sum
}

/// Test sphere-AABB intersection
pub fn sphere_aabb_intersection(sphere: &Sphere, aabb: &AABox) -> bool {
    let closest_point = Vec3::new(
        sphere
            .center
            .x
            .clamp(aabb.center.x - aabb.extent.x, aabb.center.x + aabb.extent.x),
        sphere
            .center
            .y
            .clamp(aabb.center.y - aabb.extent.y, aabb.center.y + aabb.extent.y),
        sphere
            .center
            .z
            .clamp(aabb.center.z - aabb.extent.z, aabb.center.z + aabb.extent.z),
    );

    (closest_point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

/// Test AABB-AABB intersection
pub fn aabb_aabb_intersection(aabb1: &AABox, aabb2: &AABox) -> bool {
    let min1 = aabb1.center - aabb1.extent;
    let max1 = aabb1.center + aabb1.extent;
    let min2 = aabb2.center - aabb2.extent;
    let max2 = aabb2.center + aabb2.extent;

    min1.x <= max2.x
        && max1.x >= min2.x
        && min1.y <= max2.y
        && max1.y >= min2.y
        && min1.z <= max2.z
        && max1.z >= min2.z
}

/// Test frustum-AABB intersection
pub fn frustum_aabb_intersection(frustum: &Frustum, aabb: &AABox) -> bool {
    // Test if AABB is completely outside any frustum plane
    for plane in &frustum.planes {
        // Find the vertex of the AABB farthest in the direction of the plane normal
        let mut max_distance = f32::NEG_INFINITY;

        for x in [-1.0, 1.0].iter() {
            for y in [-1.0, 1.0].iter() {
                for z in [-1.0, 1.0].iter() {
                    let vertex = aabb.center
                        + Vec3::new(aabb.extent.x * x, aabb.extent.y * y, aabb.extent.z * z);
                    let distance = plane.distance_to_point(vertex);
                    max_distance = max_distance.max(distance);
                }
            }
        }

        if max_distance < 0.0 {
            return false;
        }
    }

    true
}

/// Test frustum-sphere intersection
pub fn frustum_sphere_intersection(frustum: &Frustum, sphere: &Sphere) -> bool {
    for plane in &frustum.planes {
        let distance = plane.distance_to_point(sphere.center);
        if distance < -sphere.radius {
            return false;
        }
    }
    true
}

/// Find closest point between two line segments
pub fn closest_points_between_lines(
    segment1: &LineSegment,
    segment2: &LineSegment,
) -> (Vec3, Vec3) {
    let p1 = segment1.start;
    let q1 = segment1.end;
    let p2 = segment2.start;
    let q2 = segment2.end;

    let d1 = q1 - p1;
    let d2 = q2 - p2;
    let r = p1 - p2;

    let a = d1.dot(d1);
    let e = d2.dot(d2);
    let f = d2.dot(r);

    let mut s: f32;
    let mut t: f32;

    if a <= EPSILON && e <= EPSILON {
        // Both segments are points
        s = 0.0;
        t = 0.0;
    } else if a <= EPSILON {
        // First segment is a point
        s = 0.0;
        t = f / e;
        t = t.clamp(0.0, 1.0);
    } else if e <= EPSILON {
        // Second segment is a point
        t = 0.0;
        s = (-d1.dot(r)) / a;
        s = s.clamp(0.0, 1.0);
    } else {
        let c = d1.dot(r);
        let b = d1.dot(d2);
        let denom = a * e - b * b;

        if denom != 0.0 {
            s = (b * f - c * e) / denom;
            s = s.clamp(0.0, 1.0);
        } else {
            s = 0.0;
        }

        t = (b * s + f) / e;
        t = t.clamp(0.0, 1.0);

        // Recalculate s with clamped t
        if b != 0.0 {
            s = (b * t - c) / a;
            s = s.clamp(0.0, 1.0);
        }
    }

    let point1 = p1 + d1 * s;
    let point2 = p2 + d2 * t;

    (point1, point2)
}

/// Test line segment-AABB intersection
pub fn line_segment_aabb_intersection(
    segment: &LineSegment,
    aabb: &AABox,
) -> Option<IntersectionResult> {
    let ray = Ray::new(segment.start, (segment.end - segment.start).normalize());
    let length = (segment.end - segment.start).length();

    ray_aabb_intersection(&ray, aabb).filter(|&result| result.distance <= length)
}

/// Test point-in-triangle
pub fn point_in_triangle(point: Vec3, triangle: &Triangle) -> bool {
    let (u, v, w) = barycentric_coordinates(point, triangle);
    u >= 0.0 && v >= 0.0 && w >= 0.0 && u <= 1.0 && v <= 1.0 && w <= 1.0
}

/// Test point-in-AABB
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

/// Test point-in-sphere
pub fn point_in_sphere(point: Vec3, sphere: &Sphere) -> bool {
    (point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

/// Compute barycentric coordinates of point in triangle
pub fn barycentric_coordinates(point: Vec3, triangle: &Triangle) -> (f32, f32, f32) {
    let v0 = triangle.v1 - triangle.v0;
    let v1 = triangle.v2 - triangle.v0;
    let v2 = point - triangle.v0;

    let dot00 = v0.dot(v0);
    let dot01 = v0.dot(v1);
    let dot02 = v0.dot(v2);
    let dot11 = v1.dot(v1);
    let dot12 = v1.dot(v2);

    let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
    let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
    let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
    let w = 1.0 - u - v;

    (u, v, w)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_sphere_intersection() {
        let sphere = Sphere::new(Vec3::ZERO, 1.0);
        let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0));

        let result = ray_sphere_intersection(&ray, &sphere);
        assert!(result.is_some());

        let intersection = result.unwrap();
        assert!(intersection.hit);
        assert!((intersection.point - Vec3::new(0.0, 0.0, -1.0)).length() < EPSILON);
    }

    #[test]
    fn test_ray_aabb_intersection() {
        let aabb = AABox::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let ray = Ray::new(Vec3::new(0.0, 0.0, -2.0), Vec3::new(0.0, 0.0, 1.0));

        let result = ray_aabb_intersection(&ray, &aabb);
        assert!(result.is_some());

        let intersection = result.unwrap();
        assert!(intersection.hit);
        assert!((intersection.point - Vec3::new(0.0, 0.0, -1.0)).length() < EPSILON);
    }

    #[test]
    fn test_aabb_aabb_intersection() {
        let aabb1 = AABox::new(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let aabb2 = AABox::new(Vec3::new(0.5, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        assert!(aabb_aabb_intersection(&aabb1, &aabb2));

        let aabb3 = AABox::new(Vec3::new(3.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(!aabb_aabb_intersection(&aabb1, &aabb3));
    }

    #[test]
    fn test_point_in_triangle() {
        let triangle = Triangle::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 1.0, 0.0),
        );

        assert!(point_in_triangle(Vec3::new(0.2, 0.2, 0.0), &triangle));
        assert!(!point_in_triangle(Vec3::new(1.0, 1.0, 0.0), &triangle));
    }
}
