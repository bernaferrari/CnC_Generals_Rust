//! WWMath `plane.h` coefficients: `normal.dot(point) = distance`.
//!
//! Constructors and setters retain raw coefficients; only the three-point
//! constructor normalizes, with the original collinear fallback to Z = 0.
//! Classification uses `colmathplane.h`'s coefficient-space coincidence tolerance.
//! Projection, affine transformation, rays, and plane-distance/angle queries are
//! Rust extensions. Metric queries require a nonzero normal; transformation
//! requires an invertible affine matrix. No implicit normalization is performed.

use glam::{Mat4, Vec3};

/// Plane coefficients in the original `N.dot(point) = D` convention.
#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(C)]
pub struct PlaneClass {
    /// Raw normal, not necessarily unit length.
    pub normal: Vec3,
    /// Right-hand coefficient D; a metric distance only for a unit normal.
    pub distance: f32,
}

impl Default for PlaneClass {
    fn default() -> Self {
        Self::new(Vec3::Z, 0.0)
    }
}

impl PlaneClass {
    /// Set the raw coefficients, exactly as C++ `PlaneClass::Set(normal, dist)`.
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    /// Retain the supplied normal and compute D = N.dot(point).
    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Self {
        Self::new(normal, normal.dot(point))
    }

    /// Create plane from three points
    pub fn from_points(p0: Vec3, p1: Vec3, p2: Vec3) -> Self {
        let v1 = p1 - p0;
        let v2 = p2 - p0;
        let mut normal = v1.cross(v2);
        if normal == Vec3::ZERO {
            Self::default()
        } else {
            // Vector3::Normalize leaves a vector unchanged if Length2 is zero,
            // including a nonzero cross product whose squared length underflows.
            let length_squared = normal.length_squared();
            if length_squared != 0.0 {
                normal *= length_squared.sqrt().recip();
            }
            Self::from_point_normal(p0, normal)
        }
    }

    /// Create plane from normal and a point on the plane
    pub fn from_normal_point(normal: Vec3, point: Vec3) -> Self {
        Self::from_point_normal(point, normal)
    }

    /// Get the normal vector
    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    /// Get the raw D coefficient.
    pub fn distance(&self) -> f32 {
        self.distance
    }

    /// Set the raw normal without changing D.
    pub fn set_normal(&mut self, normal: Vec3) {
        self.normal = normal;
    }

    /// Set the raw D coefficient.
    pub fn set_distance(&mut self, distance: f32) {
        self.distance = distance;
    }

    /// Initialize plane
    pub fn init(&mut self, normal: Vec3, distance: f32) {
        self.normal = normal;
        self.distance = distance;
    }

    /// Initialize from point and normal
    pub fn init_from_point_normal(&mut self, point: Vec3, normal: Vec3) {
        *self = Self::from_point_normal(point, normal);
    }

    /// Initialize from three points
    pub fn init_from_points(&mut self, p0: Vec3, p1: Vec3, p2: Vec3) {
        *self = Self::from_points(p0, p1, p2);
    }

    /// Signed coefficient residual N.dot(point) - D (metric for unit normals).
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }

    /// Rust extension: orthogonal projection; requires a nonzero normal.
    pub fn project_point(&self, point: Vec3) -> Vec3 {
        let distance = self.distance_to_point(point) / self.normal.length_squared();
        point - self.normal * distance
    }

    /// C++ `CollisionMath::Overlap_Test(Plane, point)` (COINCIDENCE_EPSILON).
    pub fn classify_point(&self, point: Vec3) -> PlaneClassification {
        self.classify_point_epsilon(point, 0.000001)
    }

    /// Classify point with epsilon
    pub fn classify_point_epsilon(&self, point: Vec3, epsilon: f32) -> PlaneClassification {
        let dist = self.distance_to_point(point);
        if dist > epsilon {
            PlaneClassification::Front
        } else if dist < -epsilon {
            PlaneClassification::Back
        } else {
            PlaneClassification::OnPlane
        }
    }

    /// Check if point is in front of plane
    pub fn is_in_front(&self, point: Vec3) -> bool {
        self.distance_to_point(point) > 0.0
    }

    /// Check if point is behind plane
    pub fn is_behind(&self, point: Vec3) -> bool {
        self.distance_to_point(point) < 0.0
    }

    /// Whether the point is ON under the source coincidence tolerance.
    pub fn is_on_plane(&self, point: Vec3) -> bool {
        self.classify_point(point) == PlaneClassification::OnPlane
    }

    /// Rust extension: inclusive coefficient-space tolerance, epsilon >= 0.
    pub fn is_on_plane_epsilon(&self, point: Vec3, epsilon: f32) -> bool {
        self.distance_to_point(point).abs() <= epsilon
    }

    /// C++ `Compute_Intersection`: exact parallel rejection and closed [0, 1].
    /// A coplanar segment has no unique intersection and returns None.
    pub fn intersect_line_segment(&self, start: Vec3, end: Vec3) -> Option<Vec3> {
        let direction = end - start;
        let denominator = self.normal.dot(direction);
        if denominator == 0.0 {
            return None;
        }
        let t = -self.distance_to_point(start) / denominator;
        if !(0.0..=1.0).contains(&t) {
            return None;
        }
        Some(start + direction * t)
    }

    /// Rust extension: forward ray intersection, with exact parallel rejection.
    pub fn intersect_ray(&self, origin: Vec3, direction: Vec3) -> Option<f32> {
        let denom = self.normal.dot(direction);

        if denom == 0.0 {
            return None; // Ray is parallel to plane
        }

        let t = -self.distance_to_point(origin) / denom;

        if t >= 0.0 { Some(t) } else { None }
    }

    /// Flip the plane (reverse normal and distance)
    pub fn flip(&mut self) {
        self.normal = -self.normal;
        self.distance = -self.distance;
    }

    /// Get flipped plane
    pub fn flipped(&self) -> Self {
        Self {
            normal: -self.normal,
            distance: -self.distance,
        }
    }

    /// Normalize the plane (ensure normal is unit length)
    pub fn normalize(&mut self) {
        let length = self.normal.length();
        if length > 0.0 {
            self.normal /= length;
            self.distance /= length;
        }
    }

    /// Get normalized plane
    pub fn normalized(&self) -> Self {
        let mut result = *self;
        result.normalize();
        result
    }

    /// Rust extension: exact unit-length check (no geometric tolerance).
    pub fn is_normalized(&self) -> bool {
        self.normal.length_squared() == 1.0
    }

    /// Rust extension: transform raw coefficients by an invertible affine matrix.
    pub fn transform(&mut self, transform: &Mat4) {
        let coefficients = transform.inverse().transpose() * self.normal.extend(-self.distance);
        self.normal = coefficients.truncate();
        self.distance = -coefficients.w;
    }

    /// Get transformed plane
    pub fn transformed(&self, transform: &Mat4) -> Self {
        let mut result = *self;
        result.transform(transform);
        result
    }

    /// Rust extension: metric separation of parallel, nonzero-normal planes.
    pub fn distance_to_plane(&self, other: &PlaneClass) -> Option<f32> {
        if !self.is_parallel_to(other) {
            return None;
        }
        let orientation = if self.normal.dot(other.normal) < 0.0 { -1.0 } else { 1.0 };
        Some((self.distance / self.normal.length()
            - orientation * other.distance / other.normal.length()).abs())
    }

    /// Rust extension: exact parallelism; requires nonzero normals.
    pub fn is_parallel_to(&self, other: &PlaneClass) -> bool {
        self.normal.cross(other.normal) == Vec3::ZERO
    }

    /// Rust extension: exact coincidence independent of coefficient scale/sign.
    pub fn is_coincident_with(&self, other: &PlaneClass) -> bool {
        self.is_parallel_to(other)
            && self.normal * other.distance == other.normal * self.distance
    }

    /// Rust extension: oriented normal angle; requires nonzero normals.
    pub fn angle_with(&self, other: &PlaneClass) -> f32 {
        let cosine = self.normal.dot(other.normal)
            / (self.normal.length() * other.normal.length());
        cosine.clamp(-1.0, 1.0).acos()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlaneClassification {
    Front,
    Back,
    OnPlane,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_coefficients_project_and_classify_with_cpp_sign() {
        let plane = PlaneClass::new(Vec3::Y * 2.0, 10.0);
        assert_eq!(plane.normal, Vec3::Y * 2.0);
        assert_eq!(plane.distance, 10.0);
        assert_eq!(plane.distance_to_point(Vec3::Y * 6.0), 2.0);
        assert_eq!(plane.project_point(Vec3::new(3.0, 9.0, -2.0)), Vec3::new(3.0, 5.0, -2.0));
        let mut initialized = PlaneClass::default();
        initialized.init(plane.normal, plane.distance);
        assert_eq!(initialized, plane);
        initialized.set_normal(Vec3::Y * 4.0);
        assert_eq!(initialized.normal, Vec3::Y * 4.0);
        assert_eq!(initialized.distance, 10.0);
        assert_eq!(PlaneClass::from_point_normal(Vec3::Y * 5.0, Vec3::Y * 2.0), plane);
    }

    #[test]
    fn collinear_points_use_cpp_default_plane() {
        let plane = PlaneClass::from_points(Vec3::X, Vec3::X * 2.0, Vec3::X * 3.0);
        assert_eq!(plane, PlaneClass::default());
        assert_eq!(plane.normal, Vec3::Z);
        assert_eq!(plane.distance, 0.0);
        assert_eq!(PlaneClass::new(Vec3::ZERO, 7.0).normal, Vec3::ZERO);
        let wound = PlaneClass::from_points(Vec3::Z * 3.0, Vec3::new(1.0, 0.0, 3.0), Vec3::new(0.0, 1.0, 3.0));
        assert_eq!(wound, PlaneClass::new(Vec3::Z, 3.0));
        let tiny = PlaneClass::from_points(Vec3::ZERO, Vec3::X * 1e-10, Vec3::Y * 1e-15);
        assert_eq!(tiny.normal, (Vec3::X * 1e-10).cross(Vec3::Y * 1e-15));
        assert_ne!(tiny.normal, Vec3::ZERO);
        assert_eq!(tiny.normal.length_squared(), 0.0);
    }

    #[test]
    fn segment_uses_exact_parallel_and_closed_endpoint_contract() {
        let plane = PlaneClass::new(Vec3::Y, 0.0);
        assert_eq!(plane.intersect_line_segment(Vec3::ZERO, Vec3::X), None);
        assert_eq!(plane.intersect_line_segment(Vec3::Y * 0.001, Vec3::Y * 0.002), None);
        assert_eq!(plane.intersect_line_segment(Vec3::Y * 1e-8, -Vec3::Y * 1e-8), Some(Vec3::ZERO));
        assert_eq!(plane.intersect_line_segment(Vec3::ZERO, Vec3::Y), Some(Vec3::ZERO));
        assert_eq!(plane.intersect_line_segment(Vec3::Y, Vec3::ZERO), Some(Vec3::ZERO));
        assert_eq!(plane.intersect_ray(Vec3::Y, -Vec3::Y * 1e-8), Some(1e8));
    }

    #[test]
    fn classification_uses_source_coincidence_epsilon() {
        let plane = PlaneClass::new(Vec3::Y, 0.0);
        for sign in [-1.0, 1.0] {
            assert_eq!(plane.classify_point(Vec3::Y * (sign * 1e-6)), PlaneClassification::OnPlane);
            assert!(plane.is_on_plane(Vec3::Y * (sign * 1e-6)));
        }
        assert_eq!(plane.classify_point(Vec3::Y * 2e-6), PlaneClassification::Front);
        assert_eq!(plane.classify_point(-Vec3::Y * 2e-6), PlaneClassification::Back);
        assert!(plane.is_in_front(Vec3::Y * 1e-8));
        assert!(!plane.is_in_front(Vec3::ZERO));
    }

    #[test]
    fn affine_transform_preserves_raw_plane_equation() {
        let plane = PlaneClass::new(Vec3::Y * 2.0, 10.0);
        let transform = Mat4::from_cols(
            glam::Vec4::new(-2.0, 0.0, 0.0, 0.0),
            glam::Vec4::new(1.0, 4.0, 0.0, 0.0),
            glam::Vec4::new(0.0, 0.0, 3.0, 0.0),
            glam::Vec4::new(7.0, 8.0, -1.0, 1.0),
        );
        let transformed = plane.transformed(&transform);
        assert_eq!(transformed.normal, Vec3::Y * 0.5);
        assert_eq!(transformed.distance, 14.0);
        for point in [Vec3::new(1.0, 5.0, 2.0), Vec3::new(-3.0, 5.0, 4.0)] {
            assert_eq!(transformed.distance_to_point(transform.transform_point3(point)), 0.0);
        }
        assert_eq!(transformed.project_point(Vec3::ZERO), Vec3::Y * 28.0);
        let tilted = PlaneClass::new(Vec3::X * 2.0, 6.0).transformed(&transform);
        assert_eq!(tilted.normal, Vec3::new(-1.0, 0.25, 0.0));
        assert_eq!(tilted.distance, 1.0);
        assert_eq!(tilted.distance_to_point(transform.transform_point3(Vec3::new(3.0, 2.0, 1.0))), 0.0);
    }

    #[test]
    fn parallel_plane_extensions_account_for_scale_and_orientation() {
        let plane = PlaneClass::new(Vec3::Y * 2.0, 10.0);
        assert!(plane.is_coincident_with(&PlaneClass::new(-Vec3::Y * 4.0, -20.0)));
        assert_eq!(plane.distance_to_plane(&PlaneClass::new(-Vec3::Y, -7.0)), Some(2.0));
        assert_eq!(plane.angle_with(&plane), 0.0);
        assert!((plane.angle_with(&plane.flipped()) - std::f32::consts::PI).abs() < 1e-6);
        assert!(!plane.is_parallel_to(&PlaneClass::new(Vec3::new(1e-5, 1.0, 0.0), 0.0)));
    }

    #[test]
    fn test_plane_creation() {
        let plane = PlaneClass::new(Vec3::new(0.0, 1.0, 0.0), 5.0);
        assert_eq!(plane.normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_from_point_normal() {
        let point = Vec3::new(0.0, 5.0, 0.0);
        let normal = Vec3::new(0.0, 1.0, 0.0);
        let plane = PlaneClass::from_point_normal(point, normal);

        assert_eq!(plane.normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_distance_to_point() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        assert_eq!(plane.distance_to_point(Vec3::new(0.0, 5.0, 0.0)), 5.0);
        assert_eq!(plane.distance_to_point(Vec3::new(0.0, -3.0, 0.0)), -3.0);
        assert_eq!(plane.distance_to_point(Vec3::ZERO), 0.0);
    }

    #[test]
    fn test_plane_classification() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        assert_eq!(
            plane.classify_point(Vec3::new(0.0, 1.0, 0.0)),
            PlaneClassification::Front
        );
        assert_eq!(
            plane.classify_point(Vec3::new(0.0, -1.0, 0.0)),
            PlaneClassification::Back
        );
        assert_eq!(
            plane.classify_point(Vec3::ZERO),
            PlaneClassification::OnPlane
        );
    }

    #[test]
    fn test_plane_intersect_ray() {
        let plane = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));

        // Ray hitting plane
        let t = plane
            .intersect_ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, -1.0, 0.0))
            .unwrap();

        assert_eq!(t, 5.0);

        // Ray parallel to plane
        assert!(
            plane
                .intersect_ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(1.0, 0.0, 0.0))
                .is_none()
        );

        // Ray away from plane
        assert!(
            plane
                .intersect_ray(Vec3::new(0.0, 5.0, 0.0), Vec3::new(0.0, 1.0, 0.0))
                .is_none()
        );
    }

    #[test]
    fn test_plane_normalize() {
        let mut plane = PlaneClass::new(Vec3::new(0.0, 2.0, 0.0), 10.0);
        plane.normalize();

        assert_eq!(plane.normal, Vec3::new(0.0, 1.0, 0.0));
        assert_eq!(plane.distance, 5.0);
    }

    #[test]
    fn test_plane_parallel() {
        let plane1 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let plane2 =
            PlaneClass::from_point_normal(Vec3::new(0.0, 2.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
        let plane3 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(1.0, 0.0, 0.0));

        assert!(plane1.is_parallel_to(&plane2));
        assert!(!plane1.is_parallel_to(&plane3));
    }

    #[test]
    fn test_plane_coincident() {
        let plane1 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let plane2 = PlaneClass::from_point_normal(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0));
        let plane3 =
            PlaneClass::from_point_normal(Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0));

        assert!(plane1.is_coincident_with(&plane2));
        assert!(!plane1.is_coincident_with(&plane3));
    }
}
