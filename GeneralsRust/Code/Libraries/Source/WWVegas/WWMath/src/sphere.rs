use super::{Matrix3D, Vector3};
use std::ops::{Add, AddAssign, Mul, MulAssign};

/// A 3D sphere defined by a center point and radius
#[derive(Debug, Clone, PartialEq)]
pub struct Sphere {
    pub center: Vector3,
    pub radius: f32,
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            center: Vector3::ZERO,
            radius: 0.0,
        }
    }
}

impl Sphere {
    /// Create a new sphere with given center and radius
    pub fn new(center: Vector3, radius: f32) -> Self {
        Self { center, radius }
    }

    /// Create a sphere from a transformed sphere
    pub fn from_transformed(matrix: &Matrix3D, center: Vector3, radius: f32) -> Self {
        Self {
            center: matrix.transform_vector(center),
            radius,
        }
    }

    /// Create a bounding sphere from a set of points using the algorithm from Graphics Gems I
    /// This generates a sphere within 5% of optimal but is much faster than exact algorithms
    pub fn from_points(points: &[Vector3]) -> Self {
        if points.is_empty() {
            return Self::default();
        }

        if points.len() == 1 {
            return Self::new(points[0], 0.0);
        }

        // Find the 6 minima and maxima points
        let mut x_min = points[0];
        let mut x_max = points[0];
        let mut y_min = points[0];
        let mut y_max = points[0];
        let mut z_min = points[0];
        let mut z_max = points[0];

        for &point in points.iter().skip(1) {
            if point.x < x_min.x {
                x_min = point;
            }
            if point.x > x_max.x {
                x_max = point;
            }
            if point.y < y_min.y {
                y_min = point;
            }
            if point.y > y_max.y {
                y_max = point;
            }
            if point.z < z_min.z {
                z_min = point;
            }
            if point.z > z_max.z {
                z_max = point;
            }
        }

        // Calculate squared distances between extreme points
        let x_span = (x_max - x_min).length_squared();
        let y_span = (y_max - y_min).length_squared();
        let z_span = (z_max - z_min).length_squared();

        // Set points to maximally separated pair
        let (dia1, dia2, _max_span) = if x_span >= y_span && x_span >= z_span {
            (x_min, x_max, x_span)
        } else if y_span >= z_span {
            (y_min, y_max, y_span)
        } else {
            (z_min, z_max, z_span)
        };

        // Compute initial center and radius
        let mut center = (dia1 + dia2) * 0.5;
        let mut radius_squared = (dia2 - center).length_squared();
        let mut radius = radius_squared.sqrt();

        // Second pass: expand sphere to include any points outside
        for &point in points {
            let dist_squared = (point - center).length_squared();

            if dist_squared > radius_squared {
                let dist = dist_squared.sqrt();

                // Adjust radius and center
                radius = (radius + dist) * 0.5;
                radius_squared = radius * radius;

                let old_to_new = dist - radius;
                center = (center * radius + point * old_to_new) / dist;
            }
        }

        Self::new(center, radius)
    }

    /// Create a sphere that encloses another sphere when moved to a new center
    pub fn from_center_and_sphere(new_center: Vector3, other: &Sphere) -> Self {
        let dist = (other.center - new_center).length();
        Self::new(new_center, other.radius + dist)
    }

    /// Initialize with new center and radius
    pub fn init(&mut self, center: Vector3, radius: f32) {
        self.center = center;
        self.radius = radius;
    }

    /// Initialize with transformed center and radius
    pub fn init_transformed(&mut self, matrix: &Matrix3D, center: Vector3, radius: f32) {
        self.center = matrix.transform_vector(center);
        self.radius = radius;
    }

    /// Move the center and update radius to enclose the old sphere
    pub fn re_center(&mut self, new_center: Vector3) {
        let dist = (self.center - new_center).length();
        self.center = new_center;
        self.radius += dist;
    }

    /// Expand this sphere to enclose another sphere
    pub fn add_sphere(&mut self, other: &Sphere) {
        if other.radius == 0.0 {
            return;
        }

        let dist = (other.center - self.center).length();
        if dist == 0.0 {
            self.radius = self.radius.max(other.radius);
            return;
        }

        let new_radius = (dist + self.radius + other.radius) * 0.5;

        if new_radius < self.radius {
            // The existing sphere is the result - do nothing
        } else if new_radius < other.radius {
            // The new sphere is the result
            self.init(other.center, other.radius);
        } else {
            // Neither sphere is completely inside the other
            let lerp = (new_radius - self.radius) / dist;
            let new_center = (other.center - self.center) * lerp + self.center;
            self.init(new_center, new_radius);
        }
    }

    /// Transform the sphere by a matrix (assumes orthogonal matrix)
    pub fn transform(&mut self, matrix: &Matrix3D) {
        self.center = matrix.transform_vector(self.center);
    }

    /// Get the volume of the sphere
    pub fn volume(&self) -> f32 {
        (4.0 / 3.0) * std::f32::consts::PI * self.radius.powi(3)
    }

    /// Test if two spheres intersect
    pub fn intersects(&self, other: &Sphere) -> bool {
        let delta = self.center - other.center;
        let dist_squared = delta.dot(delta);
        let radius_sum = self.radius + other.radius;
        dist_squared < radius_sum * radius_sum
    }

    /// Test if this sphere contains a point
    pub fn contains_point(&self, point: Vector3) -> bool {
        (point - self.center).length_squared() <= self.radius * self.radius
    }

    /// Test if this sphere is entirely in front of a plane
    pub fn is_in_front_of_plane(&self, plane_normal: Vector3, plane_distance: f32) -> bool {
        let dist = self.center.dot(plane_normal);
        (dist - plane_distance) >= self.radius
    }

    /// Test if any part of this sphere is in front of or intersecting a plane
    pub fn is_in_front_or_intersecting_plane(
        &self,
        plane_normal: Vector3,
        plane_distance: f32,
    ) -> bool {
        let dist = self.center.dot(plane_normal);
        (plane_distance - dist) < self.radius
    }
}

impl Add for Sphere {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        if self.radius == 0.0 {
            other
        } else {
            let mut result = self;
            result.add_sphere(&other);
            result
        }
    }
}

impl AddAssign for Sphere {
    fn add_assign(&mut self, other: Self) {
        self.add_sphere(&other);
    }
}

impl Mul<Sphere> for &Matrix3D {
    type Output = Sphere;

    fn mul(self, sphere: Sphere) -> Sphere {
        Sphere {
            center: self.transform_vector(sphere.center),
            radius: sphere.radius,
        }
    }
}

impl MulAssign<&Matrix3D> for Sphere {
    fn mul_assign(&mut self, matrix: &Matrix3D) {
        self.transform(matrix);
    }
}

/// Test if two spheres intersect
pub fn spheres_intersect(s1: &Sphere, s2: &Sphere) -> bool {
    s1.intersects(s2)
}

/// Add two spheres together, creating a sphere that encloses both
pub fn add_spheres(s1: &Sphere, s2: &Sphere) -> Sphere {
    s1.clone() + s2.clone()
}

/// Transform a sphere by a matrix
pub fn transform_sphere(matrix: &Matrix3D, sphere: &Sphere) -> Sphere {
    matrix * sphere.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EPSILON;

    #[test]
    fn test_sphere_new() {
        let center = Vector3::new(1.0, 2.0, 3.0);
        let radius = 5.0;
        let sphere = Sphere::new(center, radius);

        assert_eq!(sphere.center, center);
        assert_eq!(sphere.radius, radius);
    }

    #[test]
    fn test_sphere_default() {
        let sphere = Sphere::default();
        assert_eq!(sphere.center, Vector3::ZERO);
        assert_eq!(sphere.radius, 0.0);
    }

    #[test]
    fn test_sphere_volume() {
        let sphere = Sphere::new(Vector3::ZERO, 1.0);
        let expected_volume = (4.0 / 3.0) * std::f32::consts::PI;
        assert!((sphere.volume() - expected_volume).abs() < EPSILON);
    }

    #[test]
    fn test_sphere_intersects() {
        let s1 = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let s2 = Sphere::new(Vector3::new(1.5, 0.0, 0.0), 1.0);
        let s3 = Sphere::new(Vector3::new(3.0, 0.0, 0.0), 1.0);

        assert!(s1.intersects(&s2));
        assert!(!s1.intersects(&s3));
    }

    #[test]
    fn test_sphere_contains_point() {
        let sphere = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 2.0);

        assert!(sphere.contains_point(Vector3::new(0.0, 0.0, 0.0)));
        assert!(sphere.contains_point(Vector3::new(1.0, 0.0, 0.0)));
        assert!(!sphere.contains_point(Vector3::new(3.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_from_points() {
        let points = vec![
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
            Vector3::new(0.0, 0.0, 2.0),
        ];

        let sphere = Sphere::from_points(&points);

        // All points should be contained within the sphere
        for point in &points {
            assert!(sphere.contains_point(*point));
        }
    }

    #[test]
    fn test_sphere_add() {
        let s1 = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let s2 = Sphere::new(Vector3::new(3.0, 0.0, 0.0), 1.0);
        let combined = s1 + s2;

        // Combined sphere should contain both original spheres
        assert!(combined.contains_point(Vector3::new(0.0, 0.0, 0.0)));
        assert!(combined.contains_point(Vector3::new(3.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_re_center() {
        let mut sphere = Sphere::new(Vector3::new(0.0, 0.0, 0.0), 1.0);
        let new_center = Vector3::new(2.0, 0.0, 0.0);
        sphere.re_center(new_center);

        assert_eq!(sphere.center, new_center);
        assert_eq!(sphere.radius, 3.0); // Original radius + distance moved
    }
}
