//! Sphere - Spherical bounding volume
//!
//! Legacy operations follow `WWMath/sphere.h`: the two-pass point-cloud
//! constructor, sphere union, strict sphere intersection, and center-only
//! orthogonal transform. Inputs are finite and radii nonnegative.
//! Empty point slices produce a zero sphere (C++ requires a nonempty array).
//! Point growth, distance, box queries, scaling, validity and line-root helpers
//! extend that API; they own no state outside the sphere.

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

    /// Bound points using C++'s extremal-pair seed and ordered expansion pass.
    pub fn init_from_points(&mut self, points: &[Vec3]) {
        if points.is_empty() {
            *self = Self::empty();
            return;
        }

        let mut minima = [points[0]; 3];
        let mut maxima = minima;
        for &point in &points[1..] {
            for axis in 0..3 {
                if point[axis] < minima[axis][axis] {
                    minima[axis] = point;
                }
                if point[axis] > maxima[axis][axis] {
                    maxima[axis] = point;
                }
            }
        }

        // C++ subtracts float coordinates before promoting each difference to
        // double. Equal spans retain the earlier axis (X, then Y, then Z).
        let mut diameter_axis = 0;
        let mut max_span = (maxima[0] - minima[0]).as_dvec3().length_squared();
        for axis in 1..3 {
            let span = (maxima[axis] - minima[axis]).as_dvec3().length_squared();
            if span > max_span {
                diameter_axis = axis;
                max_span = span;
            }
        }
        let diameter_end = maxima[diameter_axis];
        let mut center = (minima[diameter_axis] + diameter_end) * 0.5;
        let mut radius_squared = (diameter_end - center).as_dvec3().length_squared();
        let mut radius = radius_squared.sqrt();
        for &point in points {
            let distance_squared = (point - center).as_dvec3().length_squared();
            if distance_squared > radius_squared {
                let distance = distance_squared.sqrt();
                radius = (radius + distance) / 2.0;
                radius_squared = radius * radius;
                let old_to_new = distance - radius;
                center = ((center.as_dvec3() * radius + point.as_dvec3() * old_to_new)
                    / distance)
                    .as_vec3();
            }
        }
        self.center = center;
        self.radius = radius as f32;
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

    /// Check strict overlap, excluding external tangency (`Spheres_Intersect`).
    pub fn intersects_sphere(&self, other: &SphereClass) -> bool {
        let distance_squared = (self.center - other.center).length_squared();
        let radius_sum = self.radius + other.radius;
        distance_squared < radius_sum * radius_sum
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
        if diff > 0.0 { diff * diff } else { 0.0 }
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

    /// Enclose another sphere, ignoring zero-radius inputs as C++ does.
    pub fn add_sphere(&mut self, other: &SphereClass) {
        if other.radius == 0.0 {
            return;
        }
        let delta = other.center - self.center;
        let distance = delta.length();
        if distance == 0.0 {
            self.radius = self.radius.max(other.radius);
            return;
        }

        let new_radius = (distance + self.radius + other.radius) / 2.0;
        if new_radius >= self.radius {
            if new_radius < other.radius {
                *self = *other;
            } else {
                let lerp = (new_radius - self.radius) / distance;
                self.center += delta * lerp;
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

    /// Transform the center, retaining the radius exactly.
    ///
    /// Matches `SphereClass::Transform`: the caller supplies an orthogonal
    /// affine matrix. Scale the radius explicitly with [`Self::scale`] if needed.
    pub fn transform(&mut self, transform: &Mat4) {
        self.center = transform.transform_point3(self.center);
    }

    /// Get the volume of the sphere
    pub fn volume(&self) -> f32 {
        // sphere.h computes the cube in float, then multiplies in double.
        let radius_cubed = self.radius * self.radius * self.radius;
        ((4.0 / 3.0) * f64::from(std::f32::consts::PI) * f64::from(radius_cubed)) as f32
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

    /// Return ordered parameters where the infinite line meets the sphere.
    ///
    /// The direction must be nonzero and need not be normalized. Negative
    /// parameters are retained; this Rust extension does not clip to a ray.
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

    /// Whether the infinite line has roots; negative parameters also count.
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
    fn sphere_extensions_preserve_line_roots_and_box_distance_contracts() {
        let sphere = SphereClass::new(Vec3::ZERO, 2.0);
        assert_eq!(
            sphere.ray_intersection(Vec3::new(6.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0)),
            Some((-4.0, -2.0))
        );
        assert_eq!(sphere.distance_to_point(Vec3::new(5.0, 0.0, 0.0)), 3.0);
        assert_eq!(sphere.distance_squared_to_point(Vec3::new(5.0, 0.0, 0.0)), 9.0);
        assert_eq!(sphere.closest_point(Vec3::new(5.0, 0.0, 0.0)), Vec3::new(2.0, 0.0, 0.0));
        assert_eq!(sphere.closest_point(Vec3::X), Vec3::X);
        assert!(sphere.contains_aabox(&super::super::AABoxClass::from_center_extent(
            Vec3::ZERO, Vec3::ONE,
        )));
        let tangent_box = super::super::AABoxClass::from_center_extent(
            Vec3::new(3.0, 0.0, 0.0), Vec3::ONE,
        );
        assert!(sphere.intersects_aabox(&tangent_box));
        assert!(!sphere.contains_aabox(&tangent_box));
        let mut scaled = sphere;
        scaled.translate(Vec3::X);
        scaled.scale(2.0);
        assert_eq!(scaled, SphereClass::new(Vec3::new(2.0, 0.0, 0.0), 4.0));
        scaled.set_radius(-1.0);
        assert!(!scaled.is_valid());
        scaled.make_valid();
        assert!(scaled.is_valid());
        assert!(scaled.is_degenerate());
    }

    #[test]
    fn cpp_point_cloud_uses_extremal_diameter_not_centroid() {
        let mut sphere = SphereClass::empty();
        sphere.init_from_points(&[
            Vec3::ZERO,
            Vec3::new(10.0, 0.0, 0.0),
            Vec3::new(1.0, 0.0, 0.0),
        ]);
        assert_eq!(sphere.center, Vec3::new(5.0, 0.0, 0.0));
        assert_eq!(sphere.radius, 5.0);
        sphere.init_from_points(&[]);
        assert_eq!(sphere, SphereClass::empty());
        sphere.init_from_points(&[Vec3::new(3.0, 4.0, 5.0)]);
        assert_eq!(sphere, SphereClass::point(Vec3::new(3.0, 4.0, 5.0)));
    }

    #[test]
    fn cpp_point_cloud_expands_seed_in_input_order() {
        let mut sphere = SphereClass::empty();
        sphere.init_from_points(&[
            Vec3::new(-2.0, 0.0, 0.0),
            Vec3::new(2.0, 0.0, 0.0),
            Vec3::new(0.0, 3.0, 0.0),
        ]);
        assert_eq!(sphere.center, Vec3::new(0.0, 0.5, 0.0));
        assert_eq!(sphere.radius, 2.5);
    }

    #[test]
    fn cpp_sphere_union_handles_zero_concentric_contained_and_disjoint_inputs() {
        let original = SphereClass::new(Vec3::ZERO, 2.0);
        let mut sphere = original;
        sphere.add_sphere(&SphereClass::point(Vec3::splat(100.0)));
        assert_eq!(sphere, original);
        sphere.add_sphere(&SphereClass::new(Vec3::ZERO, 3.0));
        assert_eq!(sphere, SphereClass::new(Vec3::ZERO, 3.0));
        sphere.add_sphere(&SphereClass::new(Vec3::X, 1.0));
        assert_eq!(sphere, SphereClass::new(Vec3::ZERO, 3.0));
        sphere.add_sphere(&SphereClass::new(Vec3::X, 8.0));
        assert_eq!(sphere, SphereClass::new(Vec3::X, 8.0));
        sphere = original;
        sphere.add_sphere(&SphereClass::new(Vec3::new(6.0, 0.0, 0.0), 2.0));
        assert_eq!(sphere, SphereClass::new(Vec3::new(3.0, 0.0, 0.0), 5.0));
    }

    #[test]
    fn cpp_sphere_tangency_is_not_overlap() {
        let sphere = SphereClass::new(Vec3::ZERO, 2.0);
        assert!(!sphere.intersects_sphere(&SphereClass::new(Vec3::new(3.0, 0.0, 0.0), 1.0)));
        assert!(sphere.intersects_sphere(&SphereClass::new(Vec3::new(2.5, 0.0, 0.0), 1.0)));
        assert!(!SphereClass::empty().intersects_sphere(&SphereClass::empty()));
    }

    #[test]
    fn cpp_sphere_transform_preserves_radius_and_moves_center_once() {
        let mut sphere = SphereClass::new(Vec3::new(1.0, 2.0, 3.0), 7.0);
        let transform = Mat4::from_cols(
            Vec3::Y.extend(0.0),
            (-Vec3::X).extend(0.0),
            Vec3::Z.extend(0.0),
            Vec3::new(10.0, 20.0, 30.0).extend(1.0),
        );
        sphere.transform(&transform);
        assert_eq!(sphere, SphereClass::new(Vec3::new(8.0, 21.0, 33.0), 7.0));
        assert_eq!(sphere.bounding_aabox().extent, Vec3::splat(7.0));
    }

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
        assert!(
            sphere
                .ray_intersection(Vec3::new(0.0, 2.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
                .is_none()
        );
    }
}
