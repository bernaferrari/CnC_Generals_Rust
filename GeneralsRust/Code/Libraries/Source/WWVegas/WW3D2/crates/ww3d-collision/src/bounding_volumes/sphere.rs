//! Sphere - Spherical bounding volume
//!
//! This module implements spherical bounding volumes for collision detection
//! and spatial partitioning, converted from the original SphereClass.

use glam::{Mat4, Vec3};

/// Spherical bounding volume
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct SphereClass {
    /// Center point of the sphere
    pub center: Vec3,
    /// Radius of the sphere
    pub radius: f32,
}

impl SphereClass {
    /// Create a new sphere
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    /// Create an empty sphere (zero radius)
    pub fn empty() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 0.0,
        }
    }

    /// Create a sphere that bounds a single point
    pub fn point(center: Vec3) -> Self {
        Self {
            center,
            radius: 0.0,
        }
    }

    /// Get the center of the sphere
    pub fn center(&self) -> Vec3 {
        self.center
    }

    /// Get the radius of the sphere
    pub fn radius(&self) -> f32 {
        self.radius
    }

    /// Set the center of the sphere
    pub fn set_center(&mut self, center: Vec3) {
        self.center = center;
    }

    /// Set the radius of the sphere
    pub fn set_radius(&mut self, radius: f32) {
        self.radius = radius;
    }

    /// Initialize sphere from center and radius
    pub fn init(&mut self, center: Vec3, radius: f32) {
        self.center = center;
        self.radius = radius;
    }

    /// Initialize sphere to bound a single point
    pub fn init_from_point(&mut self, point: Vec3) {
        self.center = point;
        self.radius = 0.0;
    }

    /// Initialize sphere to bound multiple points
    pub fn init_from_points(&mut self, points: &[Vec3]) {
        if points.is_empty() {
            *self = Self::empty();
            return;
        }

        // Find the center as the average of all points
        let mut center_sum = Vec3::ZERO;
        for &point in points {
            center_sum += point;
        }
        self.center = center_sum / points.len() as f32;

        // Find the maximum distance from center to any point
        let mut max_distance_squared: f32 = 0.0;
        for &point in points {
            let distance_squared = (point - self.center).length_squared();
            max_distance_squared = max_distance_squared.max(distance_squared);
        }
        self.radius = max_distance_squared.sqrt();
    }

    /// Check if the sphere contains a point
    pub fn contains_point(&self, point: Vec3) -> bool {
        (point - self.center).length_squared() <= self.radius * self.radius
    }

    /// Check if the sphere contains another sphere
    pub fn contains_sphere(&self, other: &SphereClass) -> bool {
        let distance = (self.center - other.center).length();
        distance + other.radius <= self.radius
    }

    /// Check if two spheres intersect
    pub fn intersects_sphere(&self, other: &SphereClass) -> bool {
        let distance_squared = (self.center - other.center).length_squared();
        let radius_sum = self.radius + other.radius;
        distance_squared <= radius_sum * radius_sum
    }

    /// Calculate the distance from the sphere to a point
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        let distance = (point - self.center).length();
        (distance - self.radius).max(0.0)
    }

    /// Calculate the squared distance from the sphere to a point
    pub fn distance_squared_to_point(&self, point: Vec3) -> f32 {
        let distance = (point - self.center).length();
        let diff = distance - self.radius;
        if diff > 0.0 {
            diff * diff
        } else {
            0.0
        }
    }

    /// Find the closest point on the sphere to a given point
    pub fn closest_point(&self, point: Vec3) -> Vec3 {
        let direction = point - self.center;
        let distance = direction.length();

        if distance <= self.radius {
            // Point is inside sphere, return the point itself
            point
        } else {
            // Point is outside, return point on sphere surface
            self.center + direction.normalize() * self.radius
        }
    }

    /// Add a point to the sphere, expanding it if necessary
    pub fn add_point(&mut self, point: Vec3) {
        if self.contains_point(point) {
            return;
        }

        // Calculate new sphere that contains both the old sphere and the new point
        let direction = point - self.center;
        let distance = direction.length();

        if distance > self.radius {
            let new_radius = (self.radius + distance) * 0.5;
            let center_shift = direction.normalize() * (new_radius - self.radius);
            self.center += center_shift;
            self.radius = new_radius;
        }
    }

    /// Add multiple points to the sphere
    pub fn add_points(&mut self, points: &[Vec3]) {
        for &point in points {
            self.add_point(point);
        }
    }

    /// Add another sphere to this sphere
    pub fn add_sphere(&mut self, other: &SphereClass) {
        if self.contains_sphere(other) {
            return;
        }

        let direction = other.center - self.center;
        let distance = direction.length();

        if distance + other.radius > self.radius {
            if distance + self.radius <= other.radius {
                // Other sphere completely contains this sphere
                *self = *other;
            } else {
                // Calculate new sphere that contains both
                let new_radius = (distance + self.radius + other.radius) * 0.5;
                let center_shift = direction.normalize() * (new_radius - self.radius);
                self.center += center_shift;
                self.radius = new_radius;
            }
        }
    }

    /// Translate the sphere by an offset
    pub fn translate(&mut self, offset: Vec3) {
        self.center += offset;
    }

    /// Scale the sphere
    pub fn scale(&mut self, scale: f32) {
        self.center *= scale;
        self.radius *= scale;
    }

    /// Transform the sphere by a matrix (approximation for affine transforms)
    pub fn transform(&mut self, transform: &Mat4) {
        // Transform the center
        self.center = transform.transform_point3(self.center);

        // For radius, we need to consider the maximum scaling factor
        // This is an approximation that works for most cases
        let scale_x =
            (transform.x_axis.length() + transform.y_axis.length() + transform.z_axis.length())
                / 3.0;
        let scale_y =
            (transform.x_axis.length() + transform.y_axis.length() + transform.z_axis.length())
                / 3.0;
        let scale_z =
            (transform.x_axis.length() + transform.y_axis.length() + transform.z_axis.length())
                / 3.0;

        let max_scale = scale_x.max(scale_y).max(scale_z);
        self.radius *= max_scale;
    }

    /// Get the volume of the sphere
    pub fn volume(&self) -> f32 {
        (4.0 / 3.0) * std::f32::consts::PI * self.radius.powi(3)
    }

    /// Get the surface area of the sphere
    pub fn surface_area(&self) -> f32 {
        4.0 * std::f32::consts::PI * self.radius.powi(2)
    }

    /// Check if the sphere is valid (non-negative radius)
    pub fn is_valid(&self) -> bool {
        self.radius >= 0.0
    }

    /// Make the sphere valid
    pub fn make_valid(&mut self) {
        if self.radius < 0.0 {
            self.radius = 0.0;
        }
    }

    /// Check if the sphere is degenerate (zero radius)
    pub fn is_degenerate(&self) -> bool {
        self.radius <= 0.0
    }

    /// Check intersection with AABox
    pub fn intersects_aabox(&self, aabox: &super::AABoxClass) -> bool {
        let closest_point = aabox.closest_point(&self.center);
        (closest_point - self.center).length_squared() <= self.radius * self.radius
    }

    /// Check if sphere completely contains AABox
    pub fn contains_aabox(&self, aabox: &super::AABoxClass) -> bool {
        let corners = aabox.get_corners();
        for corner in &corners {
            if !self.contains_point(*corner) {
                return false;
            }
        }
        true
    }

    /// Calculate intersection with a ray
    pub fn ray_intersection(&self, ray_origin: Vec3, ray_direction: Vec3) -> Option<(f32, f32)> {
        let oc = ray_origin - self.center;
        let a = ray_direction.dot(ray_direction);
        let b = 2.0 * oc.dot(ray_direction);
        let c = oc.dot(oc) - self.radius * self.radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            return None; // No intersection
        }

        let sqrt_discriminant = discriminant.sqrt();
        let t1 = (-b - sqrt_discriminant) / (2.0 * a);
        let t2 = (-b + sqrt_discriminant) / (2.0 * a);

        Some((t1.min(t2), t1.max(t2)))
    }

    /// Check if ray intersects sphere
    pub fn ray_intersects(&self, ray_origin: Vec3, ray_direction: Vec3) -> bool {
        self.ray_intersection(ray_origin, ray_direction).is_some()
    }

    /// Get the bounding AABox of the sphere
    pub fn bounding_aabox(&self) -> super::AABoxClass {
        let extent = Vec3::new(self.radius, self.radius, self.radius);
        super::AABoxClass::from_center_and_extent(self.center, extent)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sphere_creation() {
        let sphere = SphereClass::new(Vec3::new(1.0, 2.0, 3.0), 5.0);
        assert_eq!(sphere.center, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(sphere.radius, 5.0);
    }

    #[test]
    fn test_sphere_contains_point() {
        let sphere = SphereClass::new(Vec3::ZERO, 1.0);
        assert!(sphere.contains_point(Vec3::ZERO));
        assert!(sphere.contains_point(Vec3::new(0.5, 0.0, 0.0)));
        assert!(!sphere.contains_point(Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_intersection() {
        let a = SphereClass::new(Vec3::ZERO, 1.0);
        let b = SphereClass::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
        let c = SphereClass::new(Vec3::new(3.0, 0.0, 0.0), 1.0);

        assert!(a.intersects_sphere(&b)); // Touching
        assert!(!a.intersects_sphere(&c)); // Not touching
    }

    #[test]
    fn test_sphere_add_point() {
        let mut sphere = SphereClass::point(Vec3::ZERO);
        sphere.add_point(Vec3::new(2.0, 0.0, 0.0));

        assert_eq!(sphere.center, Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(sphere.radius, 1.0);
    }

    #[test]
    fn test_sphere_volume() {
        let sphere = SphereClass::new(Vec3::ZERO, 1.0);
        let expected_volume = (4.0 / 3.0) * std::f32::consts::PI;
        assert!((sphere.volume() - expected_volume).abs() < 0.001);
    }

    #[test]
    fn test_sphere_ray_intersection() {
        let sphere = SphereClass::new(Vec3::ZERO, 1.0);

        // Ray through center should intersect
        let (t1, t2) = sphere
            .ray_intersection(Vec3::new(-2.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .unwrap();

        assert!(t1 > 0.0); // Entry point after ray origin
        assert!(t2 > t1); // Exit point after entry point

        // Ray missing sphere should not intersect
        assert!(sphere
            .ray_intersection(Vec3::new(0.0, 2.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
            .is_none());
    }
}
