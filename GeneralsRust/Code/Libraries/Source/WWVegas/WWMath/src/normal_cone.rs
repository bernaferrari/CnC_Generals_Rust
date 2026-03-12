//! Normal cone utilities for hierarchical backface culling.
//!
//! The NormalCone represents a cone of unit-length normals and can be used to
//! loosely represent a collection of other normals, allowing backface culling
//! to be performed at a hierarchical level rather than at a triangle level.
//!
//! The term 'NormalCone' is a bit of a misnomer; it is really a circular portion
//! of a sphere representing the angular spread of normal vectors.

use crate::{Matrix3, Vector3};
use std::f32::consts::PI;

/// Small epsilon value for floating-point comparisons.
const EPSILON: f32 = 1e-6;

/// A normal cone representing a collection of normal vectors.
///
/// The cone is defined by a center direction (inherited from Vector3) and
/// an angle parameter that represents the cosine of the half-angle of the cone.
///
/// - angle = 1.0: degenerate cone (single direction)
/// - angle = 0.0: hemisphere
/// - angle = -1.0: complete sphere (all directions)
#[derive(Debug, Clone, PartialEq)]
pub struct NormalCone {
    /// Center direction of the cone
    pub center: Vector3,
    /// Cosine of the half-angle of the cone
    /// - 1.0: single direction (degenerate cone)
    /// - 0.0: hemisphere
    /// - -1.0: complete sphere
    pub angle: f32,
}

impl NormalCone {
    /// Create a new normal cone.
    ///
    /// # Arguments
    /// * `center` - Center direction of the cone (should be normalized)
    /// * `angle` - Cosine of the half-angle of the cone (default: 1.0)
    pub fn new(center: Vector3, angle: f32) -> Self {
        Self { center, angle }
    }

    /// Create a degenerate cone (single direction).
    ///
    /// # Arguments
    /// * `direction` - The single direction (should be normalized)
    pub fn from_direction(direction: Vector3) -> Self {
        Self::new(direction, 1.0)
    }

    /// Create a hemisphere cone.
    ///
    /// # Arguments
    /// * `center` - Center direction of the hemisphere
    pub fn hemisphere(center: Vector3) -> Self {
        Self::new(center, 0.0)
    }

    /// Create a complete sphere cone.
    pub fn complete_sphere() -> Self {
        Self::new(Vector3::new(0.0, 0.0, 1.0), -1.0)
    }

    /// Set the cone parameters.
    ///
    /// # Arguments
    /// * `center` - Center direction of the cone
    /// * `angle` - Cosine of the half-angle of the cone
    pub fn set(&mut self, center: Vector3, angle: f32) {
        self.center = center;
        self.angle = angle;
    }

    /// Copy from another normal cone.
    ///
    /// # Arguments
    /// * `other` - The cone to copy from
    pub fn copy_from(&mut self, other: &NormalCone) {
        self.center = other.center;
        self.angle = other.angle;
    }

    /// Check if this cone has degenerated into a complete sphere.
    ///
    /// # Returns
    /// `true` if the cone represents all possible directions
    pub fn is_complete_sphere(&self) -> bool {
        (self.angle + EPSILON) <= -1.0
    }

    /// Find the two vectors on the edge of the cone residing on the same plane as the input vector.
    ///
    /// # Arguments
    /// * `input` - Input vector to find coplanar normals for
    ///
    /// # Returns
    /// Tuple of (length of cross product, output1, output2) or None if vectors are parallel
    pub fn get_coplanar_normals(&self, input: &Vector3) -> Option<(f32, Vector3, Vector3)> {
        // Get the cross product of the existing normal and the new one
        let cross = input.cross(self.center);
        let length_squared = cross.length_squared();

        if length_squared < EPSILON {
            return None;
        }

        let length = length_squared.sqrt();
        let cross_normalized = cross / length;

        // Make rotation matrices which use the cross product as an axis of rotation
        // and rotate the center about that axis twice, once +angle, once -angle.
        let radians = (1.0 - self.angle) * PI * 0.5;

        let m1 = Matrix3::from_axis_angle(cross_normalized, radians);
        let m2 = Matrix3::from_axis_angle(cross_normalized, -radians);

        let output1 = m1 * self.center;
        let output2 = m2 * self.center;

        Some((length, output1, output2))
    }

    /// Find the two vectors on the edge of the cone residing on the same plane as the input vector
    /// and compute their dot products with the input.
    ///
    /// # Arguments
    /// * `input` - Input vector to find coplanar normals for
    ///
    /// # Returns
    /// Tuple of (length, output1, output2, dot1, dot2) or None if vectors are parallel
    pub fn get_coplanar_normals_and_dots(
        &self,
        input: &Vector3,
    ) -> Option<(f32, Vector3, Vector3, f32, f32)> {
        if let Some((length, output1, output2)) = self.get_coplanar_normals(input) {
            let dot1 = input.dot(output1);
            let dot2 = input.dot(output2);
            Some((length, output1, output2, dot1, dot2))
        } else {
            None
        }
    }

    /// Merge a normal vector into this cone, expanding the angle as needed.
    ///
    /// # Arguments
    /// * `input` - Normal vector to merge into the cone
    pub fn merge_normal(&mut self, input: &Vector3) {
        // Early exit if this normal cone has already turned into a complete sphere
        if self.is_complete_sphere() {
            return;
        }

        // Get the dot of the new vector with the current center vector
        let dot0 = input.dot(self.center) + EPSILON;

        // If the dot value is greater than the existing cone angle, then the new vector fits
        // within the cone, so return.
        if dot0 >= self.angle {
            return;
        }

        // Get the two normals found in the cone which are coplanar to the input
        if let Some((length, normal1, normal2, dot1, dot2)) =
            self.get_coplanar_normals_and_dots(input)
        {
            if length <= EPSILON {
                return;
            }

            // Test the case where the current center has a lower dot than either of the coplanar normals.
            // If true, this means that the object now represents a complete sphere with normals facing every
            // direction.
            if (dot0 < dot1) && (dot0 < dot2) {
                self.angle = -1.0;
                return;
            }

            // The smaller of the dot values indicates which of the two coplanar normals to use
            // for averaging into the new center normal.
            let new_center = if dot1 < dot2 {
                *input + normal1
            } else {
                *input + normal2
            };

            let new_angle = dot1.min(dot2);

            // If the angle is < 0, reverse the direction of the averaged normal since we have constructed
            // something more like a sphere with a cone shape taken out of it (a negative cone).
            if new_angle < EPSILON {
                self.center = -new_center.normalize();
            } else {
                self.center = new_center.normalize();
            }
            self.angle = new_angle;
        }
    }

    /// Merge another normal cone into this cone.
    ///
    /// # Arguments
    /// * `other` - The other normal cone to merge
    pub fn merge_cone(&mut self, other: &NormalCone) {
        if let Some((_, n1, n2)) = other.get_coplanar_normals(&self.center) {
            self.merge_normal(&n1);
            self.merge_normal(&n2);
        }
    }

    /// Find the smallest dot product between the input vector and any normal contained by the cone.
    ///
    /// If the input vector is also contained by the cone, the result is always 1.0.
    /// In the case of a complete sphere, the nearest coplanar normal will be pointing in
    /// the opposite direction of the input vector, so the result is -1.0.
    ///
    /// # Arguments
    /// * `input` - Input vector to test against
    ///
    /// # Returns
    /// The smallest possible dot product with any normal in the cone
    pub fn smallest_dot_product(&self, input: &Vector3) -> f32 {
        if self.is_complete_sphere() {
            return -1.0;
        }

        // Get the dot of the input vector with the current center vector
        let dot0 = input.dot(self.center);

        // If the negative dot value is greater than the existing cone angle, then the input vector is
        // parallel to one of the vectors contained in the cone but in the negative direction
        if -dot0 + EPSILON >= self.angle {
            return -1.0;
        }

        // If the dot value is greater than the existing cone angle, then the input vector is
        // parallel to one of the vectors contained in the cone
        if dot0 + EPSILON >= self.angle {
            return 1.0;
        }

        // Get the two normals found in the cone which are coplanar to the input
        if let Some((_, _, _, dot1, dot2)) = self.get_coplanar_normals_and_dots(input) {
            dot1.min(dot2)
        } else {
            dot0
        }
    }

    /// Test if a vector is contained within the cone.
    ///
    /// # Arguments
    /// * `vector` - Vector to test
    ///
    /// # Returns
    /// `true` if the vector is within the cone
    pub fn contains(&self, vector: &Vector3) -> bool {
        if self.is_complete_sphere() {
            return true;
        }

        let dot = vector.dot(self.center);
        dot + EPSILON >= self.angle
    }

    /// Get the half-angle of the cone in radians.
    ///
    /// # Returns
    /// Half-angle in radians, or None for degenerate cases
    pub fn half_angle_radians(&self) -> Option<f32> {
        if self.angle >= 1.0 - EPSILON {
            Some(0.0) // Degenerate cone
        } else if self.angle <= -1.0 + EPSILON {
            Some(PI) // Complete sphere
        } else {
            Some(self.angle.acos())
        }
    }

    /// Get the full opening angle of the cone in radians.
    ///
    /// # Returns
    /// Full opening angle in radians, or None for degenerate cases
    pub fn opening_angle_radians(&self) -> Option<f32> {
        self.half_angle_radians().map(|half| 2.0 * half)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_approx_eq(a: f32, b: f32, tolerance: f32) {
        assert!(
            (a - b).abs() < tolerance,
            "Expected {} ≈ {}, difference: {}",
            a,
            b,
            (a - b).abs()
        );
    }

    fn assert_vector_approx_eq(a: &Vector3, b: &Vector3, tolerance: f32) {
        assert_approx_eq(a.x, b.x, tolerance);
        assert_approx_eq(a.y, b.y, tolerance);
        assert_approx_eq(a.z, b.z, tolerance);
    }

    #[test]
    fn test_cone_creation() {
        let center = Vector3::new(0.0, 0.0, 1.0);
        let cone = NormalCone::new(center, 0.5);

        assert_vector_approx_eq(&cone.center, &center, EPSILON);
        assert_approx_eq(cone.angle, 0.5, EPSILON);
    }

    #[test]
    fn test_degenerate_cone() {
        let direction = Vector3::new(1.0, 0.0, 0.0);
        let cone = NormalCone::from_direction(direction);

        assert_vector_approx_eq(&cone.center, &direction, EPSILON);
        assert_approx_eq(cone.angle, 1.0, EPSILON);
        assert!(!cone.is_complete_sphere());
    }

    #[test]
    fn test_hemisphere() {
        let center = Vector3::new(0.0, 1.0, 0.0);
        let cone = NormalCone::hemisphere(center);

        assert_vector_approx_eq(&cone.center, &center, EPSILON);
        assert_approx_eq(cone.angle, 0.0, EPSILON);
        assert!(!cone.is_complete_sphere());
    }

    #[test]
    fn test_complete_sphere() {
        let cone = NormalCone::complete_sphere();

        assert_approx_eq(cone.angle, -1.0, EPSILON);
        assert!(cone.is_complete_sphere());
    }

    #[test]
    fn test_contains() {
        // Test degenerate cone (single direction)
        let cone = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));
        assert!(cone.contains(&Vector3::new(0.0, 0.0, 1.0)));
        assert!(!cone.contains(&Vector3::new(1.0, 0.0, 0.0)));

        // Test hemisphere
        let hemisphere = NormalCone::hemisphere(Vector3::new(0.0, 0.0, 1.0));
        assert!(hemisphere.contains(&Vector3::new(0.0, 0.0, 1.0)));
        assert!(hemisphere.contains(&Vector3::new(1.0, 0.0, 0.0)));
        assert!(!hemisphere.contains(&Vector3::new(0.0, 0.0, -1.0)));

        // Test complete sphere
        let sphere = NormalCone::complete_sphere();
        assert!(sphere.contains(&Vector3::new(1.0, 0.0, 0.0)));
        assert!(sphere.contains(&Vector3::new(0.0, 1.0, 0.0)));
        assert!(sphere.contains(&Vector3::new(0.0, 0.0, -1.0)));
    }

    #[test]
    fn test_smallest_dot_product() {
        // Test with degenerate cone
        let cone = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));

        // Same direction should give 1.0
        assert_approx_eq(
            cone.smallest_dot_product(&Vector3::new(0.0, 0.0, 1.0)),
            1.0,
            EPSILON,
        );

        // Opposite direction should give -1.0
        assert_approx_eq(
            cone.smallest_dot_product(&Vector3::new(0.0, 0.0, -1.0)),
            -1.0,
            EPSILON,
        );

        // Test with complete sphere
        let sphere = NormalCone::complete_sphere();
        assert_approx_eq(
            sphere.smallest_dot_product(&Vector3::new(1.0, 0.0, 0.0)),
            -1.0,
            EPSILON,
        );
    }

    #[test]
    fn test_get_coplanar_normals() {
        let cone = NormalCone::new(Vector3::new(0.0, 0.0, 1.0), 0.5);
        let input = Vector3::new(1.0, 0.0, 0.0);

        if let Some((length, n1, n2)) = cone.get_coplanar_normals(&input) {
            assert!(length > 0.0);
            assert!((n1.length() - 1.0).abs() < 0.01);
            assert!((n2.length() - 1.0).abs() < 0.01);

            // Both normals should be on the cone edge (dot with center = angle)
            assert_approx_eq(n1.dot(cone.center), cone.angle, 0.1);
            assert_approx_eq(n2.dot(cone.center), cone.angle, 0.1);
        } else {
            panic!("Should have found coplanar normals");
        }
    }

    #[test]
    fn test_merge_normal() {
        let mut cone = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));

        // Merging the same direction should not change the cone
        let original_center = cone.center;
        let original_angle = cone.angle;
        cone.merge_normal(&Vector3::new(0.0, 0.0, 1.0));
        assert_vector_approx_eq(&cone.center, &original_center, EPSILON);
        assert_approx_eq(cone.angle, original_angle, EPSILON);

        // Merging a different direction should expand the cone
        cone.merge_normal(&Vector3::new(1.0, 0.0, 0.0));
        assert!(cone.angle < original_angle); // Angle should decrease (cone widens)
    }

    #[test]
    fn test_merge_opposite_normals() {
        let mut cone = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));

        // Merging opposite direction should create complete sphere
        cone.merge_normal(&Vector3::new(0.0, 0.0, -1.0));
        assert!(cone.is_complete_sphere() || cone.angle < -0.9);
    }

    #[test]
    fn test_half_angle() {
        // Test degenerate cone
        let degenerate = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));
        assert_approx_eq(degenerate.half_angle_radians().unwrap(), 0.0, EPSILON);

        // Test hemisphere
        let hemisphere = NormalCone::hemisphere(Vector3::new(0.0, 0.0, 1.0));
        assert_approx_eq(hemisphere.half_angle_radians().unwrap(), PI / 2.0, EPSILON);

        // Test complete sphere
        let sphere = NormalCone::complete_sphere();
        assert_approx_eq(sphere.half_angle_radians().unwrap(), PI, EPSILON);
    }

    #[test]
    fn test_opening_angle() {
        // Test degenerate cone
        let degenerate = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));
        assert_approx_eq(degenerate.opening_angle_radians().unwrap(), 0.0, EPSILON);

        // Test hemisphere
        let hemisphere = NormalCone::hemisphere(Vector3::new(0.0, 0.0, 1.0));
        assert_approx_eq(hemisphere.opening_angle_radians().unwrap(), PI, EPSILON);

        // Test complete sphere
        let sphere = NormalCone::complete_sphere();
        assert_approx_eq(sphere.opening_angle_radians().unwrap(), 2.0 * PI, EPSILON);
    }

    #[test]
    fn test_cone_merge_cone() {
        let mut cone1 = NormalCone::from_direction(Vector3::new(0.0, 0.0, 1.0));
        let cone2 = NormalCone::from_direction(Vector3::new(1.0, 0.0, 0.0));

        let original_angle = cone1.angle;
        cone1.merge_cone(&cone2);

        // The cone should have expanded
        assert!(cone1.angle < original_angle);
    }
}
