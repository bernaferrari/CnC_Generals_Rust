use super::Vector3;

/// Axis-aligned plane normal direction
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisEnum {
    XNormal = 0,
    YNormal = 1,
    ZNormal = 2,
}

/// An axis-aligned plane where the normal is one of the coordinate axes
///
/// This is a simplified plane representation for cases where the normal
/// is guaranteed to be aligned with one of the coordinate axes (X, Y, or Z).
/// This allows for more efficient computations compared to general planes.
///
/// This exactly matches the C++ AAPlaneClass structure.
#[derive(Debug, Clone, PartialEq)]
pub struct AAPlane {
    /// The normal axis (XNORMAL, YNORMAL, or ZNORMAL) - matches C++ Normal field
    pub normal: AxisEnum,
    /// The distance from origin along the normal axis - matches C++ Dist field
    pub dist: f32,
}

impl Default for AAPlane {
    fn default() -> Self {
        Self {
            normal: AxisEnum::ZNormal,
            dist: 0.0,
        }
    }
}

impl AAPlane {
    /// Create a new axis-aligned plane
    /// Matches C++: AAPlaneClass(AxisEnum normal,float dist) : Normal(normal),Dist(dist)
    pub fn new(normal: AxisEnum, dist: f32) -> Self {
        Self { normal, dist }
    }

    /// Set the plane parameters
    /// Matches C++: void Set(AxisEnum normal,float dist);
    pub fn set(&mut self, normal: AxisEnum, dist: f32) {
        self.normal = normal;
        self.dist = dist;
    }

    /// Get the normal vector of the plane
    /// Matches C++: void Get_Normal(Vector3 * normal) const;
    pub fn get_normal(&self) -> Vector3 {
        match self.normal {
            AxisEnum::XNormal => Vector3::new(1.0, 0.0, 0.0),
            AxisEnum::YNormal => Vector3::new(0.0, 1.0, 0.0),
            AxisEnum::ZNormal => Vector3::new(0.0, 0.0, 1.0),
        }
    }

    /// Set the normal vector (alternative method for C++ compatibility)
    /// Matches C++: void Get_Normal(Vector3 * normal) const;
    pub fn get_normal_into(&self, normal: &mut Vector3) {
        normal.x = 0.0;
        normal.y = 0.0;
        normal.z = 0.0;
        match self.normal {
            AxisEnum::XNormal => normal.x = 1.0,
            AxisEnum::YNormal => normal.y = 1.0,
            AxisEnum::ZNormal => normal.z = 1.0,
        }
    }

    /// Get the signed distance from a point to the plane
    pub fn distance_to_point(&self, point: Vector3) -> f32 {
        let coordinate = match self.normal {
            AxisEnum::XNormal => point.x,
            AxisEnum::YNormal => point.y,
            AxisEnum::ZNormal => point.z,
        };
        coordinate - self.dist
    }

    /// Test if a point is in front of the plane (positive side)
    pub fn is_point_in_front(&self, point: Vector3) -> bool {
        self.distance_to_point(point) > 0.0
    }

    /// Project a point onto the plane
    pub fn project_point(&self, point: Vector3) -> Vector3 {
        let mut result = point;
        match self.normal {
            AxisEnum::XNormal => result.x = self.dist,
            AxisEnum::YNormal => result.y = self.dist,
            AxisEnum::ZNormal => result.z = self.dist,
        }
        result
    }

    /// Test if a sphere is entirely in front of the plane
    pub fn is_sphere_in_front(&self, center: Vector3, radius: f32) -> bool {
        self.distance_to_point(center) >= radius
    }

    /// Test if any part of a sphere is in front of or intersecting the plane
    pub fn is_sphere_in_front_or_intersecting(&self, center: Vector3, radius: f32) -> bool {
        self.distance_to_point(center) > -radius
    }

    /// Compute intersection of a line segment with the plane
    /// Returns (intersection_found, parameter_t)
    /// If intersection_found is true, the intersection point is at p0 + t * (p1 - p0)
    pub fn compute_intersection(&self, p0: Vector3, p1: Vector3) -> (bool, f32) {
        let (start_coord, end_coord) = match self.normal {
            AxisEnum::XNormal => (p0.x, p1.x),
            AxisEnum::YNormal => (p0.y, p1.y),
            AxisEnum::ZNormal => (p0.z, p1.z),
        };

        let direction = end_coord - start_coord;

        // Check if line is parallel to plane
        if direction.abs() < f32::EPSILON {
            return (false, 0.0);
        }

        let t = (self.dist - start_coord) / direction;

        // Check if intersection is within the line segment
        if !(0.0..=1.0).contains(&t) {
            (false, t)
        } else {
            (true, t)
        }
    }
}

impl From<AxisEnum> for usize {
    fn from(axis: AxisEnum) -> Self {
        axis as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::EPSILON;

    #[test]
    fn test_aaplane_new() {
        let plane = AAPlane::new(AxisEnum::YNormal, 5.0);
        assert_eq!(plane.normal, AxisEnum::YNormal);
        assert_eq!(plane.dist, 5.0);
    }

    #[test]
    fn test_aaplane_get_normal() {
        let plane_x = AAPlane::new(AxisEnum::XNormal, 0.0);
        let plane_y = AAPlane::new(AxisEnum::YNormal, 0.0);
        let plane_z = AAPlane::new(AxisEnum::ZNormal, 0.0);

        assert_eq!(plane_x.get_normal(), Vector3::new(1.0, 0.0, 0.0));
        assert_eq!(plane_y.get_normal(), Vector3::new(0.0, 1.0, 0.0));
        assert_eq!(plane_z.get_normal(), Vector3::new(0.0, 0.0, 1.0));

        // Test get_normal_into method
        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        plane_x.get_normal_into(&mut normal);
        assert_eq!(normal, Vector3::new(1.0, 0.0, 0.0));

        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        plane_y.get_normal_into(&mut normal);
        assert_eq!(normal, Vector3::new(0.0, 1.0, 0.0));

        let mut normal = Vector3::new(0.0, 0.0, 0.0);
        plane_z.get_normal_into(&mut normal);
        assert_eq!(normal, Vector3::new(0.0, 0.0, 1.0));
    }

    #[test]
    fn test_aaplane_distance_to_point() {
        let plane = AAPlane::new(AxisEnum::YNormal, 3.0);

        let point1 = Vector3::new(0.0, 5.0, 0.0);
        let point2 = Vector3::new(0.0, 1.0, 0.0);

        assert!((plane.distance_to_point(point1) - 2.0).abs() < EPSILON);
        assert!((plane.distance_to_point(point2) + 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_aaplane_is_point_in_front() {
        let plane = AAPlane::new(AxisEnum::ZNormal, 0.0);

        assert!(plane.is_point_in_front(Vector3::new(0.0, 0.0, 1.0)));
        assert!(!plane.is_point_in_front(Vector3::new(0.0, 0.0, -1.0)));
    }

    #[test]
    fn test_aaplane_project_point() {
        let plane = AAPlane::new(AxisEnum::XNormal, 2.0);
        let point = Vector3::new(10.0, 5.0, 3.0);
        let projected = plane.project_point(point);

        assert_eq!(projected, Vector3::new(2.0, 5.0, 3.0));
    }

    #[test]
    fn test_aaplane_sphere_tests() {
        let plane = AAPlane::new(AxisEnum::YNormal, 0.0);

        // Sphere entirely in front
        assert!(plane.is_sphere_in_front(Vector3::new(0.0, 2.0, 0.0), 1.0));

        // Sphere entirely behind
        assert!(!plane.is_sphere_in_front(Vector3::new(0.0, -2.0, 0.0), 1.0));

        // Sphere intersecting
        assert!(!plane.is_sphere_in_front(Vector3::new(0.0, 0.0, 0.0), 2.0));

        // Test in_front_or_intersecting
        assert!(plane.is_sphere_in_front_or_intersecting(Vector3::new(0.0, 2.0, 0.0), 1.0));
        assert!(!plane.is_sphere_in_front_or_intersecting(Vector3::new(0.0, -2.0, 0.0), 1.0));
        assert!(plane.is_sphere_in_front_or_intersecting(Vector3::new(0.0, 0.0, 0.0), 2.0));
    }

    #[test]
    fn test_aaplane_compute_intersection() {
        let plane = AAPlane::new(AxisEnum::ZNormal, 0.0);
        let p0 = Vector3::new(0.0, 0.0, -1.0);
        let p1 = Vector3::new(0.0, 0.0, 1.0);

        let (intersects, t) = plane.compute_intersection(p0, p1);
        assert!(intersects);
        assert!((t - 0.5).abs() < EPSILON);
    }

    #[test]
    fn test_aaplane_set() {
        let mut plane = AAPlane::default();
        plane.set(AxisEnum::XNormal, 10.0);

        assert_eq!(plane.normal, AxisEnum::XNormal);
        assert_eq!(plane.dist, 10.0);
    }
}
