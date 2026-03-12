//! Collision Detection - Main collision detection system
//!
//! This module provides the main collision detection functionality,
//! integrating all bounding volume types for comprehensive collision queries.

use super::*;
use glam::Vec3;

/// Main collision detection result
#[derive(Debug, Clone)]
pub struct CollisionResult {
    pub has_collision: bool,
    pub penetration_depth: f32,
    pub contact_point: Vec3,
    pub contact_normal: Vec3,
}

/// Ray collision query
#[derive(Debug, Clone)]
pub struct RayCollisionQuery {
    pub origin: Vec3,
    pub direction: Vec3,
    pub max_distance: f32,
}

impl RayCollisionQuery {
    pub fn new(origin: Vec3, direction: Vec3, max_distance: f32) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
            max_distance,
        }
    }

    /// Test ray intersection with AABox
    pub fn test_aabox(&self, aabox: &AABoxClass) -> Option<CollisionResult> {
        // For AABox with center/extent representation, compute min/max
        let min = aabox.center - aabox.extent;
        let max = aabox.center + aabox.extent;

        let mut tmin = 0.0f32;
        let mut tmax = self.max_distance;

        // X axis
        if self.direction.x.abs() > 0.0001 {
            let inv_dx = 1.0 / self.direction.x;
            let tx1 = (min.x - self.origin.x) * inv_dx;
            let tx2 = (max.x - self.origin.x) * inv_dx;
            let (txnear, txfar) = if tx1 < tx2 { (tx1, tx2) } else { (tx2, tx1) };
            tmin = tmin.max(txnear);
            tmax = tmax.min(txfar);
            if tmax <= tmin {
                return None;
            }
        } else if self.origin.x < min.x || self.origin.x > max.x {
            return None;
        }

        // Y axis
        if self.direction.y.abs() > 0.0001 {
            let inv_dy = 1.0 / self.direction.y;
            let ty1 = (min.y - self.origin.y) * inv_dy;
            let ty2 = (max.y - self.origin.y) * inv_dy;
            let (tynear, tyfar) = if ty1 < ty2 { (ty1, ty2) } else { (ty2, ty1) };
            tmin = tmin.max(tynear);
            tmax = tmax.min(tyfar);
            if tmax <= tmin {
                return None;
            }
        } else if self.origin.y < min.y || self.origin.y > max.y {
            return None;
        }

        // Z axis
        if self.direction.z.abs() > 0.0001 {
            let inv_dz = 1.0 / self.direction.z;
            let tz1 = (min.z - self.origin.z) * inv_dz;
            let tz2 = (max.z - self.origin.z) * inv_dz;
            let (tznear, tzfar) = if tz1 < tz2 { (tz1, tz2) } else { (tz2, tz1) };
            tmin = tmin.max(tznear);
            tmax = tmax.min(tzfar);
            if tmax <= tmin {
                return None;
            }
        } else if self.origin.z < min.z || self.origin.z > max.z {
            return None;
        }

        if tmin < 0.0 {
            tmin = 0.0;
        }

        if tmin > self.max_distance {
            return None;
        }

        let contact_point = self.origin + self.direction * tmin;
        let contact_normal = self.compute_normal_at_point(aabox, contact_point);

        Some(CollisionResult {
            has_collision: true,
            penetration_depth: 0.0, // Ray doesn't have penetration depth
            contact_point,
            contact_normal,
        })
    }

    /// Test ray intersection with OBBox
    pub fn test_obbox(&self, obbox: &OBBoxClass) -> Option<CollisionResult> {
        // For simplified OBBox (axis-aligned), just use AABox intersection
        let aabox = AABoxClass::from_center_and_extent(obbox.center, obbox.extent);
        self.test_aabox(&aabox)
    }

    /// Test ray intersection with Sphere
    pub fn test_sphere(&self, sphere: &SphereClass) -> Option<CollisionResult> {
        let oc = self.origin - sphere.center;
        let a = self.direction.dot(self.direction);
        let b = 2.0 * oc.dot(self.direction);
        let c = oc.dot(oc) - sphere.radius * sphere.radius;
        let discriminant: f32 = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None;
        }

        let sqrt_discriminant = discriminant.sqrt();
        let t1 = (-b - sqrt_discriminant) / (2.0 * a);
        let t2 = (-b + sqrt_discriminant) / (2.0 * a);

        let t = if t1 >= 0.0 { t1 } else { t2 };

        if t < 0.0 || t > self.max_distance {
            return None;
        }

        let contact_point = self.origin + self.direction * t;
        let contact_normal = (contact_point - sphere.center).normalize();

        Some(CollisionResult {
            has_collision: true,
            penetration_depth: 0.0,
            contact_point,
            contact_normal,
        })
    }

    /// Test ray intersection with Plane
    pub fn test_plane(&self, plane: &PlaneClass) -> Option<CollisionResult> {
        let denom = plane.normal.dot(self.direction);

        if denom.abs() < 0.0001 {
            return None; // Ray is parallel to plane
        }

        let t = -(plane.normal.dot(self.origin) + plane.distance) / denom;

        if t < 0.0 || t > self.max_distance {
            return None;
        }

        let contact_point = self.origin + self.direction * t;

        Some(CollisionResult {
            has_collision: true,
            penetration_depth: 0.0,
            contact_point,
            contact_normal: plane.normal,
        })
    }

    /// Compute normal at contact point for AABox
    fn compute_normal_at_point(&self, aabox: &AABoxClass, point: Vec3) -> Vec3 {
        let local_point = point - aabox.center;
        let extent = aabox.extent;

        let mut _normal = Vec3::ZERO;

        // Check each face and determine which one the point is closest to
        let dist_x = (local_point.x.abs() - extent.x).abs();
        let dist_y = (local_point.y.abs() - extent.y).abs();
        let dist_z = (local_point.z.abs() - extent.z).abs();

        if dist_x >= dist_y && dist_x >= dist_z {
            _normal = Vec3::new(local_point.x.signum(), 0.0, 0.0);
        } else if dist_y >= dist_z {
            _normal = Vec3::new(0.0, local_point.y.signum(), 0.0);
        } else {
            _normal = Vec3::new(0.0, 0.0, local_point.z.signum());
        }

        _normal
    }
}

/// Bounding volume collision tests
pub fn test_aabox_aabox(a: &AABoxClass, b: &AABoxClass) -> bool {
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

pub fn test_aabox_sphere(aabox: &AABoxClass, sphere: &SphereClass) -> bool {
    let closest_point = aabox.closest_point(&sphere.center);
    (closest_point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

pub fn test_aabox_obbox(aabox: &AABoxClass, obbox: &OBBoxClass) -> bool {
    let obbox_aabox = AABoxClass::from_center_and_extent(obbox.center, obbox.extent);
    test_aabox_aabox(aabox, &obbox_aabox)
}

pub fn test_sphere_sphere(a: &SphereClass, b: &SphereClass) -> bool {
    let distance_squared = (a.center - b.center).length_squared();
    let radius_sum = a.radius + b.radius;
    distance_squared <= radius_sum * radius_sum
}

pub fn test_sphere_obbox(sphere: &SphereClass, obbox: &OBBoxClass) -> bool {
    let closest_point = obbox.closest_point(sphere.center);
    (closest_point - sphere.center).length_squared() <= sphere.radius * sphere.radius
}

pub fn test_plane_point(plane: &PlaneClass, point: Vec3) -> PlaneClassification {
    plane.classify_point(point)
}

pub fn test_plane_sphere(plane: &PlaneClass, sphere: &SphereClass) -> bool {
    let distance = plane.distance_to_point(sphere.center).abs();
    distance <= sphere.radius
}

/// Frustum culling test
pub fn test_frustum_culling(frustum_planes: &[PlaneClass], bounds: &AABoxClass) -> bool {
    let corners = bounds.get_corners();

    for plane in frustum_planes {
        let mut all_outside = true;

        for corner in &corners {
            if plane.classify_point(*corner) != PlaneClassification::Back {
                all_outside = false;
                break;
            }
        }

        if all_outside {
            return false; // Completely outside this plane
        }
    }

    true // At least partially visible
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ray_aabox_intersection() {
        let aabox =
            AABoxClass::from_center_extent(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        let ray = RayCollisionQuery::new(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 10.0);

        let result = ray.test_aabox(&aabox);
        assert!(result.is_some());
        assert!(result.unwrap().has_collision);
    }

    #[test]
    fn test_ray_sphere_intersection() {
        let sphere = SphereClass::new(Vec3::ZERO, 1.0);

        let ray = RayCollisionQuery::new(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), 10.0);

        let result = ray.test_sphere(&sphere);
        assert!(result.is_some());
        assert!(result.unwrap().has_collision);
    }

    #[test]
    fn test_aabox_aabox_collision() {
        let a = AABoxClass::from_center_extent(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0));
        let b = AABoxClass::from_center_extent(Vec3::new(1.5, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        assert!(test_aabox_aabox(&a, &b)); // Touching

        let c = AABoxClass::from_center_extent(Vec3::new(3.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 1.0));

        assert!(!test_aabox_aabox(&a, &c)); // Not touching
    }

    #[test]
    fn test_sphere_sphere_collision() {
        let a = SphereClass::new(Vec3::ZERO, 1.0);
        let b = SphereClass::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
        let c = SphereClass::new(Vec3::new(3.0, 0.0, 0.0), 1.0);

        assert!(test_sphere_sphere(&a, &b)); // Touching
        assert!(!test_sphere_sphere(&a, &c)); // Not touching
    }

    #[test]
    fn test_plane_sphere_collision() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let sphere = SphereClass::new(Vec3::new(0.0, 0.5, 0.0), 0.6);

        assert!(test_plane_sphere(&plane, &sphere)); // Intersecting

        let distant_sphere = SphereClass::new(Vec3::new(0.0, 2.0, 0.0), 0.5);
        assert!(!test_plane_sphere(&plane, &distant_sphere)); // Not intersecting
    }
}
