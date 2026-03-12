use super::{Vector3, EPSILON};
use crate::sphere::Sphere;

/// Side of a plane that a point or object lies on
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaneSide {
    Front = 0,
    Back = 1,
    On = 2,
}

/// A 3D plane defined by a normal vector and distance from origin
///
/// The plane equation is: N·P = D
/// Where N is the normal vector, P is any point on the plane, and D is the distance.
///
/// Note: This uses the N·P = D form, not the Ax + By + Cz + D = 0 form.
/// If you're used to the latter, the sign of D is inverted.
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Plane {
    pub normal: Vector3,
    pub distance: f32,
    // Legacy field alias for backward compatibility with collision code
    pub dist: f32,
}

impl Default for Plane {
    fn default() -> Self {
        Self {
            normal: Vector3::new(0.0, 0.0, 1.0),
            distance: 0.0,
            dist: 0.0,
        }
    }
}

impl Plane {
    /// Create a new plane with explicit normal and distance
    pub fn new(normal: Vector3, distance: f32) -> Self {
        Self {
            normal,
            distance,
            dist: distance,
        }
    }

    /// Create a plane from coefficients (nx, ny, nz, d)
    pub fn from_coefficients(nx: f32, ny: f32, nz: f32, d: f32) -> Self {
        Self {
            normal: Vector3::new(nx, ny, nz),
            distance: d,
            dist: d,
        }
    }

    /// Create a plane from a normal vector and a point on the plane
    pub fn from_normal_and_point(normal: Vector3, point: Vector3) -> Self {
        let distance = normal.dot(point);
        Self {
            normal,
            distance,
            dist: distance,
        }
    }

    /// Create a plane from three points
    pub fn from_three_points(p1: Vector3, p2: Vector3, p3: Vector3) -> Self {
        let v1 = p2 - p1;
        let v2 = p3 - p1;
        let cross = v1.cross(v2);

        if cross.length_squared() < EPSILON * EPSILON {
            // Points are collinear - return default plane
            Self::default()
        } else {
            let normal = cross.normalize();
            let distance = normal.dot(p1);
            Self {
                normal,
                distance,
                dist: distance,
            }
        }
    }

    /// Set plane from coefficients
    pub fn set_coefficients(&mut self, nx: f32, ny: f32, nz: f32, d: f32) {
        self.normal.x = nx;
        self.normal.y = ny;
        self.normal.z = nz;
        self.distance = d;
    }

    /// Set plane from normal and distance
    pub fn set_normal_distance(&mut self, normal: Vector3, distance: f32) {
        self.normal = normal;
        self.distance = distance;
    }

    /// Set plane from normal and point
    pub fn set_normal_point(&mut self, normal: Vector3, point: Vector3) {
        self.normal = normal;
        self.distance = normal.dot(point);
    }

    /// Set plane from three points
    pub fn set_three_points(&mut self, p1: Vector3, p2: Vector3, p3: Vector3) {
        let v1 = p2 - p1;
        let v2 = p3 - p1;
        let cross = v1.cross(v2);

        if cross.length_squared() < EPSILON * EPSILON {
            // Points are collinear - set to default
            self.normal = Vector3::new(0.0, 0.0, 1.0);
            self.distance = 0.0;
        } else {
            self.normal = cross.normalize();
            self.distance = self.normal.dot(p1);
        }
    }

    /// Compute intersection of a line segment with the plane
    /// Returns (intersection_found, parameter_t)
    /// If intersection_found is true, the intersection point is at p0 + t * (p1 - p0)
    pub fn compute_intersection(&self, p0: Vector3, p1: Vector3) -> (bool, f32) {
        let direction = p1 - p0;
        let denominator = self.normal.dot(direction);

        // Check if ray is parallel to plane
        if denominator.abs() < EPSILON {
            return (false, 0.0);
        }

        let numerator = -(self.normal.dot(p0) - self.distance);
        let t = numerator / denominator;

        // Check if intersection is within the line segment
        if !(0.0..=1.0).contains(&t) {
            (false, t)
        } else {
            (true, t)
        }
    }

    /// Test if a point is in front of the plane
    pub fn is_point_in_front(&self, point: Vector3) -> bool {
        let dist = point.dot(self.normal);
        dist > self.distance
    }

    /// Test if a sphere is entirely in front of the plane
    pub fn is_sphere_in_front(&self, sphere: &Sphere) -> bool {
        let dist = sphere.center.dot(self.normal);
        (dist - self.distance) >= sphere.radius
    }

    /// Test if any part of a sphere is in front of or intersecting the plane
    pub fn is_sphere_in_front_or_intersecting(&self, sphere: &Sphere) -> bool {
        let dist = sphere.center.dot(self.normal);
        (self.distance - dist) < sphere.radius
    }

    /// Get the signed distance from a point to the plane
    pub fn distance_to_point(&self, point: Vector3) -> f32 {
        self.normal.dot(point) - self.distance
    }

    /// Project a point onto the plane
    pub fn project_point(&self, point: Vector3) -> Vector3 {
        let dist = self.distance_to_point(point);
        point - self.normal * dist
    }

    /// Find the intersection line between two planes
    /// Returns (direction, point_on_line)
    pub fn intersect_with_plane(&self, other: &Plane) -> (Vector3, Vector3) {
        // Method from "plane-to-plane intersection", Graphics Gems III, pp. 233-235

        // Find direction vector of intersection line
        let line_dir = self.normal.cross(other.normal);

        // Find a point on the line based on the largest coordinate of the direction vector
        let abs_dir = Vector3::new(line_dir.x.abs(), line_dir.y.abs(), line_dir.z.abs());

        let line_point = if abs_dir.x > abs_dir.y {
            if abs_dir.x > abs_dir.z {
                // X is largest
                let ool = 1.0 / line_dir.x;
                Vector3::new(
                    0.0,
                    (other.normal.z * self.distance - self.normal.z * other.distance) * ool,
                    (self.normal.y * other.distance - other.normal.y * self.distance) * ool,
                )
            } else {
                // Z is largest
                let ool = 1.0 / line_dir.z;
                Vector3::new(
                    (other.normal.y * self.distance - self.normal.y * other.distance) * ool,
                    (self.normal.x * other.distance - other.normal.x * self.distance) * ool,
                    0.0,
                )
            }
        } else if abs_dir.y > abs_dir.z {
            // Y is largest
            let ool = 1.0 / line_dir.y;
            Vector3::new(
                (self.normal.z * other.distance - other.normal.z * self.distance) * ool,
                0.0,
                (other.normal.x * self.distance - self.normal.x * other.distance) * ool,
            )
        } else {
            // Z is largest
            let ool = 1.0 / line_dir.z;
            Vector3::new(
                (other.normal.y * self.distance - self.normal.y * other.distance) * ool,
                (self.normal.x * other.distance - other.normal.x * self.distance) * ool,
                0.0,
            )
        };

        // Normalize direction vector
        let normalized_dir = line_dir.normalize();

        (normalized_dir, line_point)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plane_new() {
        let normal = Vector3::new(0.0, 1.0, 0.0);
        let distance = 5.0;
        let plane = Plane::new(normal, distance);

        assert_eq!(plane.normal, normal);
        assert_eq!(plane.distance, distance);
    }

    #[test]
    fn test_plane_from_coefficients() {
        let plane = Plane::from_coefficients(1.0, 0.0, 0.0, 5.0);
        assert_eq!(plane.normal, Vector3::new(1.0, 0.0, 0.0));
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_from_normal_and_point() {
        let normal = Vector3::new(0.0, 1.0, 0.0);
        let point = Vector3::new(0.0, 5.0, 0.0);
        let plane = Plane::from_normal_and_point(normal, point);

        assert_eq!(plane.normal, normal);
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_from_three_points() {
        let p1 = Vector3::new(0.0, 0.0, 0.0);
        let p2 = Vector3::new(1.0, 0.0, 0.0);
        let p3 = Vector3::new(0.0, 1.0, 0.0);
        let plane = Plane::from_three_points(p1, p2, p3);

        // Should create a plane with normal pointing in +Z direction
        assert!((plane.normal - Vector3::new(0.0, 0.0, 1.0)).length() < EPSILON);
        assert!(plane.distance.abs() < EPSILON);
    }

    #[test]
    fn test_plane_is_point_in_front() {
        let plane = Plane::new(Vector3::new(0.0, 1.0, 0.0), 0.0);

        assert!(plane.is_point_in_front(Vector3::new(0.0, 1.0, 0.0)));
        assert!(!plane.is_point_in_front(Vector3::new(0.0, -1.0, 0.0)));
    }

    #[test]
    fn test_plane_distance_to_point() {
        let plane = Plane::new(Vector3::new(0.0, 1.0, 0.0), 0.0);

        assert!((plane.distance_to_point(Vector3::new(0.0, 5.0, 0.0)) - 5.0).abs() < EPSILON);
        assert!((plane.distance_to_point(Vector3::new(0.0, -3.0, 0.0)) + 3.0).abs() < EPSILON);
    }

    #[test]
    fn test_plane_compute_intersection() {
        let plane = Plane::new(Vector3::new(0.0, 1.0, 0.0), 0.0);
        let p0 = Vector3::new(0.0, -1.0, 0.0);
        let p1 = Vector3::new(0.0, 1.0, 0.0);

        let (intersects, t) = plane.compute_intersection(p0, p1);
        assert!(intersects);
        assert!((t - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_plane_project_point() {
        let plane = Plane::new(Vector3::new(0.0, 1.0, 0.0), 0.0);
        let point = Vector3::new(5.0, 10.0, 3.0);
        let projected = plane.project_point(point);

        assert_eq!(projected, Vector3::new(5.0, 0.0, 3.0));
    }

    #[test]
    fn test_plane_sphere_tests() {
        let plane = Plane::new(Vector3::new(0.0, 1.0, 0.0), 0.0);
        let sphere_in_front = Sphere::new(Vector3::new(0.0, 2.0, 0.0), 1.0);
        let sphere_behind = Sphere::new(Vector3::new(0.0, -2.0, 0.0), 1.0);
        let sphere_intersecting = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 2.0);

        assert!(plane.is_sphere_in_front(&sphere_in_front));
        assert!(!plane.is_sphere_in_front(&sphere_behind));
        assert!(!plane.is_sphere_in_front(&sphere_intersecting));

        assert!(plane.is_sphere_in_front_or_intersecting(&sphere_in_front));
        assert!(!plane.is_sphere_in_front_or_intersecting(&sphere_behind));
        assert!(plane.is_sphere_in_front_or_intersecting(&sphere_intersecting));
    }
}
