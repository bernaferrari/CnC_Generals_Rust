//! Collision Geometry Types and Utilities
//!
//! This module provides geometric primitives and collision detection algorithms
//! for unit-to-unit, unit-to-terrain, and projectile collision detection.
//!
//! Matches C++ PartitionManager.cpp collision geometry handling

use super::Coord3D;
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

/// Geometry type for collision shapes
/// Matches C++ GeometryType enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GeometryType {
    /// Box/Rectangle collision shape
    Box,
    /// Spherical collision shape
    Sphere,
    /// Cylindrical collision shape
    Cylinder,
}

/// Geometry information for collision detection
/// Matches C++ GeometryInfo struct in PartitionManager.cpp
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct GeometryInfo {
    geom_type: GeometryType,
    /// Major radius (width/2 for box, radius for sphere/cylinder)
    major_radius: f32,
    /// Minor radius (height/2 for box, unused for sphere, height for cylinder)
    minor_radius: f32,
    /// Whether this is a small object (affects collision priority)
    is_small: bool,
}

impl GeometryInfo {
    /// Create a new box geometry
    pub fn new_box(width: f32, height: f32, is_small: bool) -> Self {
        Self {
            geom_type: GeometryType::Box,
            major_radius: width / 2.0,
            minor_radius: height / 2.0,
            is_small,
        }
    }

    /// Create a new sphere geometry
    pub fn new_sphere(radius: f32, is_small: bool) -> Self {
        Self {
            geom_type: GeometryType::Sphere,
            major_radius: radius,
            minor_radius: radius,
            is_small,
        }
    }

    /// Create a new cylinder geometry
    pub fn new_cylinder(radius: f32, height: f32, is_small: bool) -> Self {
        Self {
            geom_type: GeometryType::Cylinder,
            major_radius: radius,
            minor_radius: height,
            is_small,
        }
    }

    pub fn get_geom_type(&self) -> GeometryType {
        self.geom_type
    }

    pub fn get_major_radius(&self) -> f32 {
        self.major_radius
    }

    pub fn get_minor_radius(&self) -> f32 {
        self.minor_radius
    }

    pub fn is_small(&self) -> bool {
        self.is_small
    }

    pub fn set_major_radius(&mut self, radius: f32) {
        self.major_radius = radius;
    }

    pub fn set_minor_radius(&mut self, radius: f32) {
        self.minor_radius = radius;
    }
}

/// Collision information structure
/// Matches C++ CollideInfo struct in PartitionManager.cpp:103
#[derive(Debug, Clone, Copy)]
pub struct CollideInfo {
    pub position: Coord3D,
    pub geom: GeometryInfo,
    pub angle: f32,
}

impl CollideInfo {
    pub fn new(position: Coord3D, geom: GeometryInfo, angle: f32) -> Self {
        Self {
            position,
            geom,
            angle,
        }
    }
}

/// Collision location and normal information
/// Matches C++ CollideLocAndNormal struct in PartitionManager.cpp
#[derive(Debug, Clone, Copy)]
pub struct CollideLocAndNormal {
    /// Location of collision point
    pub loc: Coord3D,
    /// Normal vector at collision point (unit vector)
    pub normal: Coord3D,
}

impl CollideLocAndNormal {
    pub fn new(loc: Coord3D, normal: Coord3D) -> Self {
        Self { loc, normal }
    }
}

/// 2D coordinate for XY plane operations
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coord2D {
    pub x: f32,
    pub y: f32,
}

impl Coord2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    pub fn zero() -> Self {
        Self { x: 0.0, y: 0.0 }
    }
}

/// Vector difference in 2D (XY plane only)
/// Matches C++ vecDiff_2D in PartitionManager.cpp:244
#[inline]
pub fn vec_diff_2d(pos_a: &Coord3D, pos_b: &Coord3D) -> Coord3D {
    Coord3D::new(pos_a.x - pos_b.x, pos_a.y - pos_b.y, 0.0)
}

/// Vector difference in 3D
/// Matches C++ vecDiff_3D in PartitionManager.cpp:253
#[inline]
pub fn vec_diff_3d(pos_a: &Coord3D, pos_b: &Coord3D) -> Coord3D {
    Coord3D::new(pos_a.x - pos_b.x, pos_a.y - pos_b.y, pos_a.z - pos_b.z)
}

/// Calculate squared distance in 2D (faster than full distance)
/// Matches C++ calcSqrDist_2D in PartitionManager.cpp:262
#[inline]
pub fn calc_sqr_dist_2d(dist: &Coord3D) -> f32 {
    dist.x * dist.x + dist.y * dist.y
}

/// Calculate squared distance in 3D (faster than full distance)
/// Matches C++ calcSqrDist_3D in PartitionManager.cpp:268
#[inline]
pub fn calc_sqr_dist_3d(dist: &Coord3D) -> f32 {
    dist.x * dist.x + dist.y * dist.y + dist.z * dist.z
}

/// Project coordinate along a unit direction vector
/// Matches C++ projectCoord3D in PartitionManager.cpp:344
#[inline]
pub fn project_coord_3d(coord: &mut Coord3D, unit_dir: &Coord3D, dist: f32) {
    coord.x += unit_dir.x * dist;
    coord.y += unit_dir.y * dist;
    coord.z += unit_dir.z * dist;
}

/// Flip/negate a coordinate
/// Matches C++ flipCoord3D in PartitionManager.cpp:352
#[inline]
pub fn flip_coord_3d(coord: &mut Coord3D) {
    coord.x = -coord.x;
    coord.y = -coord.y;
    coord.z = -coord.z;
}

/// Normalize a 3D vector to unit length
pub fn normalize_coord_3d(coord: &Coord3D) -> Coord3D {
    let length = (coord.x * coord.x + coord.y * coord.y + coord.z * coord.z).sqrt();
    if length > 0.0 {
        Coord3D::new(coord.x / length, coord.y / length, coord.z / length)
    } else {
        Coord3D::zero()
    }
}

/// Convert a rectangle to four corner points in 2D
/// Matches C++ rectToFourPoints in PartitionManager.cpp:400
pub fn rect_to_four_points(collide_info: &CollideInfo) -> [Coord2D; 4] {
    let c = collide_info.angle.cos();
    let s = collide_info.angle.sin();

    let exc = collide_info.geom.get_major_radius() * c;
    let eyc = collide_info.geom.get_minor_radius() * c;
    let exs = collide_info.geom.get_major_radius() * s;
    let eys = collide_info.geom.get_minor_radius() * s;

    [
        // top-left
        Coord2D::new(
            collide_info.position.x - exc - eys,
            collide_info.position.y + eyc - exs,
        ),
        // top-right
        Coord2D::new(
            collide_info.position.x + exc - eys,
            collide_info.position.y + eyc + exs,
        ),
        // bottom-left
        Coord2D::new(
            collide_info.position.x - exc + eys,
            collide_info.position.y - eyc - exs,
        ),
        // bottom-right
        Coord2D::new(
            collide_info.position.x + exc + eys,
            collide_info.position.y - eyc + exs,
        ),
    ]
}

/// Test rotated points against a rectangle
/// Matches C++ testRotatedPointsAgainstRect in PartitionManager.cpp:360
pub fn test_rotated_points_against_rect(pts: &[Coord2D; 4], rect: &CollideInfo) -> (Coord2D, i32) {
    let major = rect.geom.get_major_radius();
    let minor = if rect.geom.get_geom_type() == GeometryType::Sphere {
        rect.geom.get_major_radius()
    } else {
        rect.geom.get_minor_radius()
    };

    let c = (-rect.angle).cos();
    let s = (-rect.angle).sin();

    let mut avg = Coord2D::zero();
    let mut avg_tot = 0;

    for pt in pts {
        // Convert to delta relative to rect center
        let ptx = pt.x - rect.position.x;
        let pty = pt.y - rect.position.y;

        // Inverse-rotate to the right coordinate system
        let ptx_new = (ptx * c - pty * s).abs();
        let pty_new = (ptx * s + pty * c).abs();

        if ptx_new <= major && pty_new <= minor {
            avg.x += pt.x;
            avg.y += pt.y;
            avg_tot += 1;
        }
    }

    (avg, avg_tot)
}

/// 2D circle-circle collision test (XY plane only)
/// Matches C++ xy_collideTest_Circle_Circle in PartitionManager.cpp:491
pub fn xy_collide_test_circle_circle(
    pos_a: &Coord3D,
    pos_b: &Coord3D,
    radius_a: f32,
    radius_b: f32,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let diff = vec_diff_2d(pos_b, pos_a);
    let dist_sqr = calc_sqr_dist_2d(&diff);
    let touching_dist_sqr = (radius_a + radius_b) * (radius_a + radius_b);

    if dist_sqr <= touching_dist_sqr {
        if let Some(info) = cinfo {
            // Calculate approximate location and normal
            let dist = dist_sqr.sqrt();
            if dist > 0.0 {
                // Normalize the difference vector for the normal
                info.normal = Coord3D::new(diff.x / dist, diff.y / dist, 0.0);
            } else {
                // Overlapping at same position - arbitrary normal
                info.normal = Coord3D::new(1.0, 0.0, 0.0);
            }

            // Collision point is between the two centers, weighted by radii
            let ratio = radius_a / (radius_a + radius_b);
            info.loc = Coord3D::new(
                pos_a.x + diff.x * ratio,
                pos_a.y + diff.y * ratio,
                (pos_a.z + pos_b.z) * 0.5,
            );
        }
        true
    } else {
        false
    }
}

/// 2D rectangle-circle collision test
/// Matches C++ xy_collideTest_Rect_Circle in PartitionManager.cpp:446
pub fn xy_collide_test_rect_circle(
    rect: &CollideInfo,
    circle: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // Convert circle to a box for collision testing
    let mut circle_as_box = *circle;
    circle_as_box
        .geom
        .set_minor_radius(circle_as_box.geom.get_major_radius());
    xy_collide_test_rect_rect(rect, &circle_as_box, cinfo)
}

/// 2D circle-rectangle collision test (reversed order)
/// Matches C++ xy_collideTest_Circle_Rect in PartitionManager.cpp:434
pub fn xy_collide_test_circle_rect(
    circle: &CollideInfo,
    rect: &CollideInfo,
    mut cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // Reborrow cinfo to allow usage after the call
    let result = xy_collide_test_rect_circle(rect, circle, cinfo.as_mut().map(|r| &mut **r));
    if result {
        if let Some(info) = cinfo {
            flip_coord_3d(&mut info.normal);
        }
    }
    result
}

/// 2D rectangle-rectangle collision test
/// Matches C++ xy_collideTest_Rect_Rect in PartitionManager.cpp
pub fn xy_collide_test_rect_rect(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // Get four corner points for both rectangles
    let pts_a = rect_to_four_points(a);
    let pts_b = rect_to_four_points(b);

    // Test points of A against rect B
    let (mut avg, mut avg_tot) = test_rotated_points_against_rect(&pts_a, b);

    // Test points of B against rect A
    let (avg_b, avg_tot_b) = test_rotated_points_against_rect(&pts_b, a);
    avg.x += avg_b.x;
    avg.y += avg_b.y;
    avg_tot += avg_tot_b;

    if avg_tot > 0 {
        if let Some(info) = cinfo {
            // Average collision location
            info.loc = Coord3D::new(
                avg.x / avg_tot as f32,
                avg.y / avg_tot as f32,
                (a.position.z + b.position.z) * 0.5,
            );

            // Normal points from A to B
            let diff = vec_diff_2d(&b.position, &a.position);
            let dist = calc_sqr_dist_2d(&diff).sqrt();
            if dist > 0.0 {
                info.normal = Coord3D::new(diff.x / dist, diff.y / dist, 0.0);
            } else {
                info.normal = Coord3D::new(1.0, 0.0, 0.0);
            }
        }
        true
    } else {
        false
    }
}

/// 3D sphere-sphere collision test
/// Matches C++ collideTest_Sphere_Sphere in PartitionManager.cpp:328
pub fn collide_test_sphere_sphere(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    let diff = vec_diff_3d(&b.position, &a.position);
    let dist_sqr = calc_sqr_dist_3d(&diff);
    let touching_dist_sqr = (a.geom.get_major_radius() + b.geom.get_major_radius())
        * (a.geom.get_major_radius() + b.geom.get_major_radius());

    if dist_sqr <= touching_dist_sqr {
        if let Some(info) = cinfo {
            let dist = dist_sqr.sqrt();
            if dist > 0.0 {
                info.normal = Coord3D::new(diff.x / dist, diff.y / dist, diff.z / dist);
            } else {
                info.normal = Coord3D::new(1.0, 0.0, 0.0);
            }

            let ratio =
                a.geom.get_major_radius() / (a.geom.get_major_radius() + b.geom.get_major_radius());
            info.loc = Coord3D::new(
                a.position.x + diff.x * ratio,
                a.position.y + diff.y * ratio,
                a.position.z + diff.z * ratio,
            );
        }
        true
    } else {
        false
    }
}

/// 3D cylinder-cylinder collision test
/// Matches C++ collideTest_Cylinder_Cylinder in PartitionManager.cpp:332
pub fn collide_test_cylinder_cylinder(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    // Test XY circle collision first
    if !xy_collide_test_circle_circle(
        &a.position,
        &b.position,
        a.geom.get_major_radius(),
        b.geom.get_major_radius(),
        None,
    ) {
        return false;
    }

    // Check Z overlap
    let a_bottom = a.position.z;
    let a_top = a.position.z + a.geom.get_minor_radius();
    let b_bottom = b.position.z;
    let b_top = b.position.z + b.geom.get_minor_radius();

    if a_top < b_bottom || b_top < a_bottom {
        return false;
    }

    // Collision detected, compute info if requested
    if let Some(info) = cinfo {
        xy_collide_test_circle_circle(
            &a.position,
            &b.position,
            a.geom.get_major_radius(),
            b.geom.get_major_radius(),
            Some(info),
        );
        // Z coordinate is average of overlapping region
        info.loc.z = (a_top.min(b_top) + a_bottom.max(b_bottom)) * 0.5;
    }

    true
}

/// Master collision test dispatcher
/// Matches C++ collision testing logic in PartitionManager.cpp
pub fn collision_test(
    a: &CollideInfo,
    b: &CollideInfo,
    cinfo: Option<&mut CollideLocAndNormal>,
) -> bool {
    match (a.geom.get_geom_type(), b.geom.get_geom_type()) {
        (GeometryType::Sphere, GeometryType::Sphere) => collide_test_sphere_sphere(a, b, cinfo),
        (GeometryType::Cylinder, GeometryType::Cylinder) => {
            collide_test_cylinder_cylinder(a, b, cinfo)
        }
        (GeometryType::Box, GeometryType::Box) => xy_collide_test_rect_rect(a, b, cinfo),
        (GeometryType::Sphere, GeometryType::Cylinder)
        | (GeometryType::Cylinder, GeometryType::Sphere) => {
            // Approximate with circle test in XY plane
            xy_collide_test_circle_circle(
                &a.position,
                &b.position,
                a.geom.get_major_radius(),
                b.geom.get_major_radius(),
                cinfo,
            )
        }
        (GeometryType::Box, GeometryType::Sphere) => xy_collide_test_rect_circle(a, b, cinfo),
        (GeometryType::Sphere, GeometryType::Box) => xy_collide_test_circle_rect(a, b, cinfo),
        (GeometryType::Box, GeometryType::Cylinder) => xy_collide_test_rect_circle(a, b, cinfo),
        (GeometryType::Cylinder, GeometryType::Box) => xy_collide_test_circle_rect(a, b, cinfo),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_geometry_info_creation() {
        let box_geom = GeometryInfo::new_box(10.0, 20.0, false);
        assert_eq!(box_geom.get_geom_type(), GeometryType::Box);
        assert_eq!(box_geom.get_major_radius(), 5.0);
        assert_eq!(box_geom.get_minor_radius(), 10.0);

        let sphere_geom = GeometryInfo::new_sphere(5.0, true);
        assert_eq!(sphere_geom.get_geom_type(), GeometryType::Sphere);
        assert_eq!(sphere_geom.get_major_radius(), 5.0);
        assert!(sphere_geom.is_small());
    }

    #[test]
    fn test_circle_circle_collision() {
        let pos_a = Coord3D::new(0.0, 0.0, 0.0);
        let pos_b = Coord3D::new(3.0, 0.0, 0.0);

        // Touching circles
        assert!(xy_collide_test_circle_circle(
            &pos_a, &pos_b, 2.0, 1.0, None
        ));

        // Non-touching circles
        assert!(!xy_collide_test_circle_circle(
            &pos_a, &pos_b, 1.0, 1.0, None
        ));

        // Overlapping circles with collision info
        let mut cinfo = CollideLocAndNormal::new(Coord3D::zero(), Coord3D::zero());
        assert!(xy_collide_test_circle_circle(
            &pos_a,
            &pos_b,
            2.0,
            2.0,
            Some(&mut cinfo)
        ));
        assert!(cinfo.normal.x > 0.0); // Normal points in +X direction
    }

    #[test]
    fn test_sphere_sphere_collision_3d() {
        let geom_a = GeometryInfo::new_sphere(2.0, false);
        let geom_b = GeometryInfo::new_sphere(2.0, false);

        let info_a = CollideInfo::new(Coord3D::new(0.0, 0.0, 0.0), geom_a, 0.0);
        let info_b = CollideInfo::new(Coord3D::new(3.0, 0.0, 1.0), geom_b, 0.0);

        // Should collide (distance = sqrt(9+1) = 3.16, radii sum = 4.0)
        assert!(collide_test_sphere_sphere(&info_a, &info_b, None));

        let info_c = CollideInfo::new(Coord3D::new(5.0, 0.0, 0.0), geom_b, 0.0);
        // Should not collide (distance = 5.0, radii sum = 4.0)
        assert!(!collide_test_sphere_sphere(&info_a, &info_c, None));
    }

    #[test]
    fn test_cylinder_cylinder_collision() {
        let geom_a = GeometryInfo::new_cylinder(1.0, 2.0, false);
        let geom_b = GeometryInfo::new_cylinder(1.0, 2.0, false);

        let info_a = CollideInfo::new(Coord3D::new(0.0, 0.0, 0.0), geom_a, 0.0);
        let info_b = CollideInfo::new(Coord3D::new(1.5, 0.0, 0.5), geom_b, 0.0);

        // Should collide (XY overlaps, Z overlaps)
        assert!(collide_test_cylinder_cylinder(&info_a, &info_b, None));

        let info_c = CollideInfo::new(Coord3D::new(1.5, 0.0, 5.0), geom_b, 0.0);
        // Should not collide (XY overlaps but Z doesn't)
        assert!(!collide_test_cylinder_cylinder(&info_a, &info_c, None));
    }

    #[test]
    fn test_rect_to_four_points() {
        let geom = GeometryInfo::new_box(4.0, 2.0, false);
        let info = CollideInfo::new(Coord3D::new(0.0, 0.0, 0.0), geom, 0.0);

        let points = rect_to_four_points(&info);
        assert_eq!(points.len(), 4);

        // With no rotation, points should form axis-aligned rectangle
        assert!((points[0].x - (-2.0)).abs() < 0.01); // top-left X
        assert!((points[1].x - 2.0).abs() < 0.01); // top-right X
    }

    #[test]
    fn test_vector_utilities() {
        let pos_a = Coord3D::new(5.0, 3.0, 1.0);
        let pos_b = Coord3D::new(2.0, 1.0, 0.0);

        let diff_2d = vec_diff_2d(&pos_a, &pos_b);
        assert_eq!(diff_2d.x, 3.0);
        assert_eq!(diff_2d.y, 2.0);
        assert_eq!(diff_2d.z, 0.0);

        let diff_3d = vec_diff_3d(&pos_a, &pos_b);
        assert_eq!(diff_3d.z, 1.0);

        let dist_sqr_2d = calc_sqr_dist_2d(&diff_2d);
        assert_eq!(dist_sqr_2d, 13.0);
    }

    #[test]
    fn test_collision_dispatcher() {
        let sphere_a = CollideInfo::new(
            Coord3D::new(0.0, 0.0, 0.0),
            GeometryInfo::new_sphere(2.0, false),
            0.0,
        );
        let sphere_b = CollideInfo::new(
            Coord3D::new(3.0, 0.0, 0.0),
            GeometryInfo::new_sphere(2.0, false),
            0.0,
        );

        assert!(collision_test(&sphere_a, &sphere_b, None));

        let box_a = CollideInfo::new(
            Coord3D::new(0.0, 0.0, 0.0),
            GeometryInfo::new_box(4.0, 4.0, false),
            0.0,
        );
        let box_b = CollideInfo::new(
            Coord3D::new(3.0, 0.0, 0.0),
            GeometryInfo::new_box(4.0, 4.0, false),
            0.0,
        );

        assert!(collision_test(&box_a, &box_b, None));
    }
}
