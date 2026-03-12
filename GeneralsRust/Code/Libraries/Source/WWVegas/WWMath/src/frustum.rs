use super::{Matrix3D, Vector2, Vector3};
use crate::plane::Plane;

/// A 3D view frustum defined by six clipping planes and eight corner points
///
/// The frustum represents the visible volume of a camera, bounded by near and far planes,
/// and four side planes forming a truncated pyramid.
#[derive(Debug, Clone)]
pub struct Frustum {
    pub camera_transform: Matrix3D,
    pub planes: [Plane; 6],
    pub corners: [Vector3; 8],
    pub bound_min: Vector3,
    pub bound_max: Vector3,
}

impl Default for Frustum {
    fn default() -> Self {
        Self {
            camera_transform: Matrix3D::new(),
            planes: [Plane::default(); 6],
            corners: [Vector3::ZERO; 8],
            bound_min: Vector3::ZERO,
            bound_max: Vector3::ZERO,
        }
    }
}

impl Frustum {
    /// Initialize the frustum from camera parameters
    ///
    /// # Parameters
    /// * `camera` - Camera transform matrix (camera looks down -Z axis)
    /// * `viewport_min` - Minimum corner of the z=-1.0 view plane
    /// * `viewport_max` - Maximum corner of the z=-1.0 view plane  
    /// * `znear` - Near clip plane distance (should be negative, negated if positive)
    /// * `zfar` - Far clip plane distance (should be negative, negated if positive)
    pub fn init(
        &mut self,
        camera: Matrix3D,
        viewport_min: Vector2,
        viewport_max: Vector2,
        mut znear: f32,
        mut zfar: f32,
    ) {
        // Store camera transform
        self.camera_transform = camera;

        // Forward is negative Z in viewspace - flip sign if user passed positive values
        if znear > 0.0 && zfar > 0.0 {
            znear = -znear;
            zfar = -zfar;
        }

        // Calculate camera-space frustum corners by extrapolating viewplane to near/far planes
        // Corner numbering: near plane 0-3 (upper-left, upper-right, lower-left, lower-right)
        //                   far plane 4-7 (analogous)
        // Camera space: x right, y up, z backwards (right-handed)

        // Check if we have a reflected camera matrix by computing correct z-vector
        let correct_z = self
            .camera_transform
            .get_x_vector()
            .cross(self.camera_transform.get_y_vector());
        let is_reflected = self.camera_transform.get_z_vector().dot(correct_z) < 0.0;

        if is_reflected {
            // Flip frustum corners horizontally for reflected matrix
            self.corners[1] = Vector3::new(viewport_min.x, viewport_max.y, 1.0);
            self.corners[5] = self.corners[1];
            self.corners[1] *= znear;
            self.corners[5] *= zfar;

            self.corners[0] = Vector3::new(viewport_max.x, viewport_max.y, 1.0);
            self.corners[4] = self.corners[0];
            self.corners[0] *= znear;
            self.corners[4] *= zfar;

            self.corners[3] = Vector3::new(viewport_min.x, viewport_min.y, 1.0);
            self.corners[7] = self.corners[3];
            self.corners[3] *= znear;
            self.corners[7] *= zfar;

            self.corners[2] = Vector3::new(viewport_max.x, viewport_min.y, 1.0);
            self.corners[6] = self.corners[2];
            self.corners[2] *= znear;
            self.corners[6] *= zfar;
        } else {
            // Normal camera
            self.corners[0] = Vector3::new(viewport_min.x, viewport_max.y, 1.0);
            self.corners[4] = self.corners[0];
            self.corners[0] *= znear;
            self.corners[4] *= zfar;

            self.corners[1] = Vector3::new(viewport_max.x, viewport_max.y, 1.0);
            self.corners[5] = self.corners[1];
            self.corners[1] *= znear;
            self.corners[5] *= zfar;

            self.corners[2] = Vector3::new(viewport_min.x, viewport_min.y, 1.0);
            self.corners[6] = self.corners[2];
            self.corners[2] *= znear;
            self.corners[6] *= zfar;

            self.corners[3] = Vector3::new(viewport_max.x, viewport_min.y, 1.0);
            self.corners[7] = self.corners[3];
            self.corners[3] *= znear;
            self.corners[7] *= zfar;
        }

        // Transform corners from camera space to world space
        for corner in &mut self.corners {
            *corner = self.camera_transform.transform_vector(*corner);
        }

        // Create six frustum bounding planes with normals pointing outward
        self.planes[0] =
            Plane::from_three_points(self.corners[0], self.corners[3], self.corners[1]); // near
        self.planes[1] =
            Plane::from_three_points(self.corners[0], self.corners[5], self.corners[4]); // bottom
        self.planes[2] =
            Plane::from_three_points(self.corners[0], self.corners[6], self.corners[2]); // right
        self.planes[3] =
            Plane::from_three_points(self.corners[2], self.corners[7], self.corners[3]); // top
        self.planes[4] =
            Plane::from_three_points(self.corners[1], self.corners[7], self.corners[5]); // left
        self.planes[5] =
            Plane::from_three_points(self.corners[4], self.corners[7], self.corners[6]); // far

        // Calculate bounding box of entire frustum for quick rejection
        self.bound_min = self.corners[0];
        self.bound_max = self.corners[0];

        for &corner in &self.corners[1..] {
            if corner.x < self.bound_min.x {
                self.bound_min.x = corner.x;
            }
            if corner.x > self.bound_max.x {
                self.bound_max.x = corner.x;
            }
            if corner.y < self.bound_min.y {
                self.bound_min.y = corner.y;
            }
            if corner.y > self.bound_max.y {
                self.bound_max.y = corner.y;
            }
            if corner.z < self.bound_min.z {
                self.bound_min.z = corner.z;
            }
            if corner.z > self.bound_max.z {
                self.bound_max.z = corner.z;
            }
        }
    }

    /// Get the bounding box minimum corner
    pub fn get_bound_min(&self) -> Vector3 {
        self.bound_min
    }

    /// Get the bounding box maximum corner
    pub fn get_bound_max(&self) -> Vector3 {
        self.bound_max
    }

    /// Test if a point is inside the frustum
    pub fn contains_point(&self, point: Vector3) -> bool {
        for plane in &self.planes {
            if plane.is_point_in_front(point) {
                return false; // Point is outside this plane
            }
        }
        true
    }

    /// Test if a sphere intersects or is inside the frustum
    pub fn intersects_sphere(&self, center: Vector3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.distance_to_point(center) > radius {
                return false; // Sphere is entirely outside this plane
            }
        }
        true
    }

    /// Test if a sphere is entirely inside the frustum
    pub fn contains_sphere(&self, center: Vector3, radius: f32) -> bool {
        for plane in &self.planes {
            if plane.distance_to_point(center) > -radius {
                return false; // Sphere extends outside this plane
            }
        }
        true
    }

    /// Get a specific plane of the frustum
    /// Planes are ordered: [0] near, [1] bottom, [2] right, [3] top, [4] left, [5] far
    pub fn get_plane(&self, index: usize) -> Option<&Plane> {
        self.planes.get(index)
    }

    /// Get a specific corner of the frustum
    /// Corners are ordered: [0-3] near plane (UL, UR, LL, LR), [4-7] far plane (UL, UR, LL, LR)
    pub fn get_corner(&self, index: usize) -> Option<Vector3> {
        self.corners.get(index).copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frustum_default() {
        let frustum = Frustum::default();
        assert_eq!(frustum.bound_min, Vector3::ZERO);
        assert_eq!(frustum.bound_max, Vector3::ZERO);
    }

    #[test]
    fn test_frustum_init() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        // Check that corners were computed
        assert_ne!(frustum.corners[0], Vector3::ZERO);

        // Check that bounding box was computed
        assert!(frustum.bound_min.x <= frustum.bound_max.x);
        assert!(frustum.bound_min.y <= frustum.bound_max.y);
        assert!(frustum.bound_min.z <= frustum.bound_max.z);
    }

    #[test]
    fn test_frustum_contains_point() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        // Point at origin should be inside frustum
        assert!(frustum.contains_point(Vector3::new(0.0, 0.0, -5.0)));

        // Point far outside should not be inside
        assert!(!frustum.contains_point(Vector3::new(100.0, 0.0, -5.0)));
    }

    #[test]
    fn test_frustum_get_bounds() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        let bound_min = frustum.get_bound_min();
        let bound_max = frustum.get_bound_max();

        assert_eq!(bound_min, frustum.bound_min);
        assert_eq!(bound_max, frustum.bound_max);
    }

    #[test]
    fn test_frustum_get_plane() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        // Should have 6 planes
        for i in 0..6 {
            assert!(frustum.get_plane(i).is_some());
        }
        assert!(frustum.get_plane(6).is_none());
    }

    #[test]
    fn test_frustum_get_corner() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        // Should have 8 corners
        for i in 0..8 {
            assert!(frustum.get_corner(i).is_some());
        }
        assert!(frustum.get_corner(8).is_none());
    }

    #[test]
    fn test_frustum_sphere_intersection() {
        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);

        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        // Small sphere at center should intersect
        assert!(frustum.intersects_sphere(Vector3::new(0.0, 0.0, -5.0), 0.1));

        // Large sphere far away should not intersect
        assert!(!frustum.intersects_sphere(Vector3::new(1000.0, 0.0, -5.0), 1.0));
    }
}
