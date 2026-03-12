/*
 * Overlap Tests
 *
 * Categorized spatial relationship tests between geometric primitives.
 * These functions classify the second operand with respect to the first.
 */

use super::*;
use crate::EPSILON;

impl CollisionMath {
    // ========================================================================================
    // AAPlane Overlap Tests
    // ========================================================================================

    /// Test overlap between AAPlane and point
    pub fn overlap_test_aaplane_point(plane: &AAPlane, point: &Vector3) -> OverlapType {
        let delta = match plane.normal {
            AxisEnum::XNormal => point.x,
            AxisEnum::YNormal => point.y,
            AxisEnum::ZNormal => point.z,
        } - plane.dist;
        if delta > COINCIDENCE_EPSILON {
            OverlapType::Positive
        } else if delta < -COINCIDENCE_EPSILON {
            OverlapType::Negative
        } else {
            OverlapType::On
        }
    }

    /// Test overlap between AAPlane and line segment
    pub fn overlap_test_aaplane_line(plane: &AAPlane, line: &LineSegment) -> OverlapType {
        let mut mask = 0;
        mask |= Self::overlap_test_aaplane_point(plane, &line.start()) as i32;
        mask |= Self::overlap_test_aaplane_point(plane, &line.end()) as i32;
        Self::eval_overlap_mask(mask)
    }

    /// Test overlap between AAPlane and triangle
    pub fn overlap_test_aaplane_triangle(plane: &AAPlane, tri: &Triangle) -> OverlapType {
        let mut mask = 0;
        mask |= Self::overlap_test_aaplane_point(plane, &tri.vertices[0]) as i32;
        mask |= Self::overlap_test_aaplane_point(plane, &tri.vertices[1]) as i32;
        mask |= Self::overlap_test_aaplane_point(plane, &tri.vertices[2]) as i32;
        Self::eval_overlap_mask(mask)
    }

    /// Test overlap between AAPlane and sphere
    pub fn overlap_test_aaplane_sphere(plane: &AAPlane, sphere: &Sphere) -> OverlapType {
        let delta = match plane.normal {
            AxisEnum::XNormal => sphere.center.x,
            AxisEnum::YNormal => sphere.center.y,
            AxisEnum::ZNormal => sphere.center.z,
        } - plane.dist;
        if delta > sphere.radius {
            OverlapType::Positive
        } else if delta < -sphere.radius {
            OverlapType::Negative
        } else {
            OverlapType::Both
        }
    }

    /// Test overlap between AAPlane and AABox
    pub fn overlap_test_aaplane_aabox(plane: &AAPlane, box_ref: &AABox) -> OverlapType {
        let mut mask = 0;
        // Check min side of box
        let (center_val, extent_val) = match plane.normal {
            AxisEnum::XNormal => (box_ref.center.x, box_ref.extent.x),
            AxisEnum::YNormal => (box_ref.center.y, box_ref.extent.y),
            AxisEnum::ZNormal => (box_ref.center.z, box_ref.extent.z),
        };

        let delta_min = (center_val - extent_val) - plane.dist;
        if delta_min > EPSILON {
            mask |= OverlapType::Positive as i32;
        } else if delta_min < -EPSILON {
            mask |= OverlapType::Negative as i32;
        } else {
            mask |= OverlapType::On as i32;
        }

        // Check max side of box
        let delta_max = (center_val + extent_val) - plane.dist;
        if delta_max > EPSILON {
            mask |= OverlapType::Positive as i32;
        } else if delta_max < -EPSILON {
            mask |= OverlapType::Negative as i32;
        } else {
            mask |= OverlapType::On as i32;
        }

        Self::eval_overlap_mask(mask)
    }

    // ========================================================================================
    // Plane Overlap Tests
    // ========================================================================================

    /// Test overlap between plane and point
    pub fn overlap_test_plane_point(plane: &Plane, point: &Vector3) -> OverlapType {
        let delta = point.dot(plane.normal) - plane.dist;
        if delta > COINCIDENCE_EPSILON {
            OverlapType::Positive
        } else if delta < -COINCIDENCE_EPSILON {
            OverlapType::Negative
        } else {
            OverlapType::On
        }
    }

    /// Test overlap between plane and line segment
    pub fn overlap_test_plane_line(plane: &Plane, line: &LineSegment) -> OverlapType {
        let mut mask = 0;
        mask |= Self::overlap_test_plane_point(plane, &line.start()) as i32;
        mask |= Self::overlap_test_plane_point(plane, &line.end()) as i32;
        Self::eval_overlap_mask(mask)
    }

    /// Test overlap between plane and triangle
    pub fn overlap_test_plane_triangle(plane: &Plane, tri: &Triangle) -> OverlapType {
        let mut mask = 0;
        mask |= Self::overlap_test_plane_point(plane, &tri.vertices[0]) as i32;
        mask |= Self::overlap_test_plane_point(plane, &tri.vertices[1]) as i32;
        mask |= Self::overlap_test_plane_point(plane, &tri.vertices[2]) as i32;
        Self::eval_overlap_mask(mask)
    }

    /// Test overlap between plane and sphere
    pub fn overlap_test_plane_sphere(plane: &Plane, sphere: &Sphere) -> OverlapType {
        let dist = sphere.center.dot(plane.normal) - plane.dist;
        if dist > sphere.radius {
            OverlapType::Positive
        } else if dist < -sphere.radius {
            OverlapType::Negative
        } else {
            OverlapType::Both
        }
    }

    /// Test overlap between plane and AABox
    pub fn overlap_test_plane_aabox(plane: &Plane, box_ref: &AABox) -> OverlapType {
        let pos_far_pt = get_far_extent(&plane.normal, &box_ref.extent);
        let neg_far_pt = -pos_far_pt;
        let pos_point = pos_far_pt + box_ref.center;
        let neg_point = neg_far_pt + box_ref.center;

        if Self::overlap_test_plane_point(plane, &neg_point) == OverlapType::Positive {
            OverlapType::Positive
        } else if Self::overlap_test_plane_point(plane, &pos_point) == OverlapType::Negative {
            OverlapType::Negative
        } else {
            OverlapType::Both
        }
    }

    // ========================================================================================
    // Sphere Overlap Tests
    // ========================================================================================

    /// Test overlap between sphere and point
    pub fn overlap_test_sphere_point(sphere: &Sphere, point: &Vector3) -> OverlapType {
        let r2 = (*point - sphere.center).length_squared();
        let radius_sq = sphere.radius * sphere.radius;
        if r2 < radius_sq - COINCIDENCE_EPSILON {
            OverlapType::Negative
        } else if r2 > radius_sq + COINCIDENCE_EPSILON {
            OverlapType::Positive
        } else {
            OverlapType::On
        }
    }

    /// Test overlap between two spheres
    pub fn overlap_test_sphere_sphere(sphere1: &Sphere, sphere2: &Sphere) -> OverlapType {
        let radius_sum = sphere1.radius + sphere2.radius;
        let dist_sq = (sphere2.center - sphere1.center).length_squared();

        if dist_sq == 0.0 && (sphere1.radius - sphere2.radius).abs() < COINCIDENCE_EPSILON {
            OverlapType::Both
        } else if dist_sq <= radius_sum * radius_sum - COINCIDENCE_EPSILON {
            OverlapType::Negative
        } else {
            OverlapType::Positive
        }
    }

    /// Test overlap between sphere and AABox
    pub fn overlap_test_sphere_aabox(sphere: &Sphere, box_ref: &AABox) -> OverlapType {
        if Self::intersection_test_sphere_aabox(sphere, box_ref) {
            OverlapType::Both
        } else {
            OverlapType::Positive
        }
    }

    // ========================================================================================
    // AABox Overlap Tests
    // ========================================================================================

    /// Test overlap between AABox and point
    pub fn overlap_test_aabox_point(box_ref: &AABox, point: &Vector3) -> OverlapType {
        let diff = *point - box_ref.center;
        if diff.x.abs() > box_ref.extent.x {
            return OverlapType::Positive;
        }
        if diff.y.abs() > box_ref.extent.y {
            return OverlapType::Positive;
        }
        if diff.z.abs() > box_ref.extent.z {
            return OverlapType::Positive;
        }
        OverlapType::Negative
    }

    /// Test overlap between two AABoxes
    pub fn overlap_test_aabox_aabox(box1: &AABox, box2: &AABox) -> OverlapType {
        let dc = box2.center - box1.center;

        // Check for separation
        if box1.extent.x + box2.extent.x < dc.x.abs() {
            return OverlapType::Positive;
        }
        if box1.extent.y + box2.extent.y < dc.y.abs() {
            return OverlapType::Positive;
        }
        if box1.extent.z + box2.extent.z < dc.z.abs() {
            return OverlapType::Positive;
        }

        // Check for complete containment of box2 inside box1
        if (dc.x + box2.extent.x <= box1.extent.x)
            && (dc.y + box2.extent.y <= box1.extent.y)
            && (dc.z + box2.extent.z <= box1.extent.z)
            && (dc.x - box2.extent.x >= -box1.extent.x)
            && (dc.y - box2.extent.y >= -box1.extent.y)
            && (dc.z - box2.extent.z >= -box1.extent.z)
        {
            OverlapType::Negative
        } else {
            OverlapType::Both
        }
    }

    /// Test overlap between AABox and line segment using separating axis theorem
    pub fn overlap_test_aabox_line(box_ref: &AABox, line: &LineSegment) -> OverlapType {
        // If both endpoints are inside, return INSIDE
        let start_inside =
            Self::overlap_test_aabox_point(box_ref, &line.start()) == OverlapType::Negative;
        let end_inside =
            Self::overlap_test_aabox_point(box_ref, &line.end()) == OverlapType::Negative;

        if start_inside && end_inside {
            return OverlapType::Negative;
        }

        if start_inside || end_inside {
            return OverlapType::Both;
        }

        // Use separating axis theorem for line segment vs box
        let dp0 = line.start() - box_ref.center;
        let dir = line.end() - line.start();

        // Test against box face normals (X, Y, Z axes)
        for axis in 0..3 {
            let extent = box_ref.extent[axis];
            let p0_proj = dp0[axis];
            let dp_proj = dir[axis];

            if p0_proj > 0.0 {
                if p0_proj > extent - dp_proj.min(0.0) {
                    return OverlapType::Positive;
                }
            } else if -p0_proj > extent + dp_proj.max(0.0) {
                return OverlapType::Positive;
            }
        }

        // Test against cross product axes (line direction x box axes)
        for axis in 0..3 {
            let mut cross_axis = Vector3::ZERO;
            cross_axis[axis] = 1.0;
            let test_axis = cross_axis.cross(dir);

            if test_axis.length_squared() > EPSILON * EPSILON {
                let box_proj = box_ref.extent.x * (test_axis.x * 1.0).abs()
                    + box_ref.extent.y * (test_axis.y * 1.0).abs()
                    + box_ref.extent.z * (test_axis.z * 1.0).abs();
                let p0_proj = dp0.dot(test_axis);

                if p0_proj.abs() > box_proj {
                    return OverlapType::Positive;
                }
            }
        }

        OverlapType::Both
    }

    /// Test overlap between AABox and triangle
    pub fn overlap_test_aabox_triangle(box_ref: &AABox, tri: &Triangle) -> OverlapType {
        // For now, use intersection test as a simplified approach
        if Self::intersection_test_aabox_triangle(box_ref, tri) {
            OverlapType::Both
        } else {
            OverlapType::Positive
        }
    }

    /// Test overlap between AABox and sphere
    pub fn overlap_test_aabox_sphere(box_ref: &AABox, sphere: &Sphere) -> OverlapType {
        // Use the sphere-box test but reverse the interpretation
        Self::overlap_test_sphere_aabox(sphere, box_ref)
    }
}
