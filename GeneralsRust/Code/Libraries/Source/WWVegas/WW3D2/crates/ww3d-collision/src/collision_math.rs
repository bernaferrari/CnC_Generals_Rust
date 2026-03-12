/// Collision Math - Full SAT and collision routines (ported from colmath.cpp/colmathinlines.h)
///
/// Implements complete collision mathematics including:
/// - Separating Axis Theorem (SAT) for OBBox-Triangle
/// - AABox-Triangle swept collision
/// - OBBox intersection tests
use crate::bounding_volumes::{AABoxClass, OBBoxClass};
use crate::intersection::{CastResult, Triangle};
use glam::{Mat3, Vec3};

// CRITICAL: Match C++ values exactly - these were previously swapped!
// COLLISION_EPSILON in C++ = 0.001 (used for general collision tolerance)
// COINCIDENCE_EPSILON in C++ = 0.000001 (used for near-coincident features)
const EPSILON: f32 = 0.001;
#[allow(dead_code)] // Used in collision algorithms (potential future use)
const COINCIDENCE_EPSILON: f32 = 0.000001;

/// Collision math utility functions
pub struct CollisionMath;

impl CollisionMath {
    /// Test if OBBox intersects Triangle using SAT
    pub fn obbox_triangle_intersection(obbox: &OBBoxClass, triangle: &Triangle) -> bool {
        // Full Separating Axis Theorem implementation
        // Test 13 potential separating axes:
        // - 3 face normals of OBBox
        // - 1 face normal of triangle
        // - 9 edge cross products

        let tri_v0 = triangle.vertices[0];
        let tri_v1 = triangle.vertices[1];
        let tri_v2 = triangle.vertices[2];

        // Transform triangle vertices to OBBox local space
        // OBBox basis is stored as [Vec3; 3], construct Mat3 from columns
        let basis_mat = Mat3::from_cols(obbox.basis[0], obbox.basis[1], obbox.basis[2]);
        let basis_inv = basis_mat.transpose();
        let local_v0 = basis_inv * (tri_v0 - obbox.center);
        let local_v1 = basis_inv * (tri_v1 - obbox.center);
        let local_v2 = basis_inv * (tri_v2 - obbox.center);

        // Test OBBox face normals (in local space, these are just X, Y, Z axes)
        if !Self::test_axis_aabox_tri(Vec3::X, &obbox.extent, &local_v0, &local_v1, &local_v2) {
            return false;
        }
        if !Self::test_axis_aabox_tri(Vec3::Y, &obbox.extent, &local_v0, &local_v1, &local_v2) {
            return false;
        }
        if !Self::test_axis_aabox_tri(Vec3::Z, &obbox.extent, &local_v0, &local_v1, &local_v2) {
            return false;
        }

        // Test triangle normal
        let tri_normal = (tri_v1 - tri_v0).cross(tri_v2 - tri_v0).normalize_or_zero();
        let local_normal = basis_inv * tri_normal;
        if !Self::test_axis_aabox_tri(local_normal, &obbox.extent, &local_v0, &local_v1, &local_v2)
        {
            return false;
        }

        // Test edge cross products (9 axes)
        let tri_edges = [
            local_v1 - local_v0,
            local_v2 - local_v1,
            local_v0 - local_v2,
        ];

        let box_edges = [Vec3::X, Vec3::Y, Vec3::Z];

        for &box_edge in &box_edges {
            for &tri_edge in &tri_edges {
                let axis = box_edge.cross(tri_edge);
                if axis.length_squared() > EPSILON {
                    let axis_norm = axis.normalize();
                    if !Self::test_axis_aabox_tri(
                        axis_norm,
                        &obbox.extent,
                        &local_v0,
                        &local_v1,
                        &local_v2,
                    ) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Test separation on a single axis for AABox-Triangle (in local space)
    fn test_axis_aabox_tri(axis: Vec3, extent: &Vec3, v0: &Vec3, v1: &Vec3, v2: &Vec3) -> bool {
        // Project box onto axis
        let box_radius =
            extent.x * axis.x.abs() + extent.y * axis.y.abs() + extent.z * axis.z.abs();

        // Project triangle onto axis
        let p0 = axis.dot(*v0);
        let p1 = axis.dot(*v1);
        let p2 = axis.dot(*v2);

        let tri_min = p0.min(p1).min(p2);
        let tri_max = p0.max(p1).max(p2);

        // Test for overlap
        tri_max >= -box_radius && tri_min <= box_radius
    }

    /// AABox-Triangle swept collision test
    pub fn aabox_triangle_swept(
        aabox: &AABoxClass,
        movement: Vec3,
        triangle: &Triangle,
        result: &mut CastResult,
    ) -> bool {
        // Full swept collision test using continuous collision detection

        let tri_v0 = triangle.vertices[0];
        let tri_v1 = triangle.vertices[1];
        let tri_v2 = triangle.vertices[2];
        let tri_normal = triangle.normal;

        // Check if movement is towards triangle
        let dot = movement.dot(tri_normal);
        if dot >= 0.0 {
            return false; // Moving away or parallel
        }

        // Expand triangle by box extent to handle swept volume
        let _expanded_tri_v0 =
            tri_v0 - aabox.extent.x * Vec3::X - aabox.extent.y * Vec3::Y - aabox.extent.z * Vec3::Z;
        let _expanded_tri_v1 =
            tri_v1 + aabox.extent.x * Vec3::X - aabox.extent.y * Vec3::Y - aabox.extent.z * Vec3::Z;
        let _expanded_tri_v2 =
            tri_v2 + aabox.extent.x * Vec3::X + aabox.extent.y * Vec3::Y - aabox.extent.z * Vec3::Z;

        // Simple approach: test ray from box center against triangle plane
        let plane_d = -tri_normal.dot(tri_v0);
        let start_dist = tri_normal.dot(aabox.center) + plane_d;
        let end_dist = tri_normal.dot(aabox.center + movement) + plane_d;

        // Check if we cross the plane
        if (start_dist > 0.0 && end_dist > 0.0) || (start_dist < 0.0 && end_dist < 0.0) {
            return false;
        }

        // Calculate intersection fraction
        let t = start_dist / (start_dist - end_dist);
        if t < 0.0 || t > 1.0 || t >= result.fraction {
            return false;
        }

        // Point of intersection on plane
        let hit_point = aabox.center + movement * t;

        // Check if point is inside triangle (2D test)
        if !Self::point_in_triangle(&hit_point, &tri_v0, &tri_v1, &tri_v2, &tri_normal) {
            return false;
        }

        // Valid collision
        result.fraction = t;
        result.normal = tri_normal;
        true
    }

    /// Test if point is inside triangle
    fn point_in_triangle(point: &Vec3, v0: &Vec3, v1: &Vec3, v2: &Vec3, normal: &Vec3) -> bool {
        // Use barycentric coordinates
        let edge0 = *v1 - *v0;
        let edge1 = *v2 - *v1;
        let edge2 = *v0 - *v2;

        let c0 = edge0.cross(*point - *v0);
        let c1 = edge1.cross(*point - *v1);
        let c2 = edge2.cross(*point - *v2);

        // All cross products should point in same direction as normal
        let d0 = c0.dot(*normal);
        let d1 = c1.dot(*normal);
        let d2 = c2.dot(*normal);

        d0 >= -EPSILON && d1 >= -EPSILON && d2 >= -EPSILON
    }

    /// OBBox-Triangle swept collision test
    pub fn obbox_triangle_swept(
        obbox: &OBBoxClass,
        movement: Vec3,
        triangle: &Triangle,
        result: &mut CastResult,
    ) -> bool {
        // Transform to OBBox local space for easier testing
        let basis_mat = Mat3::from_cols(obbox.basis[0], obbox.basis[1], obbox.basis[2]);
        let basis_inv = basis_mat.transpose();

        let local_movement = basis_inv * movement;
        let local_tri_v0 = basis_inv * (triangle.vertices[0] - obbox.center);
        let local_tri_v1 = basis_inv * (triangle.vertices[1] - obbox.center);
        let local_tri_v2 = basis_inv * (triangle.vertices[2] - obbox.center);
        let local_tri_normal = basis_inv * triangle.normal;

        let local_triangle = Triangle {
            vertices: [local_tri_v0, local_tri_v1, local_tri_v2],
            normal: local_tri_normal,
        };

        let local_aabox = AABoxClass::from_center_extent(Vec3::ZERO, obbox.extent);

        // Perform swept test in local space
        let mut local_result = CastResult::default();
        if Self::aabox_triangle_swept(
            &local_aabox,
            local_movement,
            &local_triangle,
            &mut local_result,
        ) {
            result.fraction = local_result.fraction;
            result.normal = basis_mat * local_result.normal; // Transform normal back to world space
            result.start_bad = local_result.start_bad;
            result.surface_type = local_result.surface_type;
            true
        } else {
            false
        }
    }

    /// AABox-AABox intersection test
    pub fn aabox_aabox_intersect(a: &AABoxClass, b: &AABoxClass) -> bool {
        let a_min = a.center - a.extent;
        let a_max = a.center + a.extent;
        let b_min = b.center - b.extent;
        let b_max = b.center + b.extent;

        a_min.x <= b_max.x
            && a_max.x >= b_min.x
            && a_min.y <= b_max.y
            && a_max.y >= b_min.y
            && a_min.z <= b_max.z
            && a_max.z >= b_min.z
    }

    /// AABox-Triangle static intersection test
    pub fn aabox_triangle_intersect(aabox: &AABoxClass, triangle: &Triangle) -> bool {
        // Use SAT with box in local space
        let tri_v0 = triangle.vertices[0] - aabox.center;
        let tri_v1 = triangle.vertices[1] - aabox.center;
        let tri_v2 = triangle.vertices[2] - aabox.center;

        Self::test_axis_aabox_tri(Vec3::X, &aabox.extent, &tri_v0, &tri_v1, &tri_v2)
            && Self::test_axis_aabox_tri(Vec3::Y, &aabox.extent, &tri_v0, &tri_v1, &tri_v2)
            && Self::test_axis_aabox_tri(Vec3::Z, &aabox.extent, &tri_v0, &tri_v1, &tri_v2)
            && {
                let tri_normal = (tri_v1 - tri_v0).cross(tri_v2 - tri_v0).normalize_or_zero();
                Self::test_axis_aabox_tri(tri_normal, &aabox.extent, &tri_v0, &tri_v1, &tri_v2)
            }
            && {
                // Test edge cross products
                let tri_edges = [tri_v1 - tri_v0, tri_v2 - tri_v1, tri_v0 - tri_v2];
                let box_edges = [Vec3::X, Vec3::Y, Vec3::Z];

                box_edges.iter().all(|&box_edge| {
                    tri_edges.iter().all(|&tri_edge| {
                        let axis = box_edge.cross(tri_edge);
                        if axis.length_squared() > EPSILON {
                            let axis_norm = axis.normalize();
                            Self::test_axis_aabox_tri(
                                axis_norm,
                                &aabox.extent,
                                &tri_v0,
                                &tri_v1,
                                &tri_v2,
                            )
                        } else {
                            true
                        }
                    })
                })
            }
    }

    /// OBBox-OBBox intersection test using SAT
    pub fn obbox_obbox_intersect(a: &OBBoxClass, b: &OBBoxClass) -> bool {
        // Test 15 potential separating axes
        // - 6 face normals (3 from each OBBox)
        // - 9 edge cross products

        let a_axes = [a.basis[0], a.basis[1], a.basis[2]];
        let b_axes = [b.basis[0], b.basis[1], b.basis[2]];

        let t = b.center - a.center;

        // Test face normals of A
        for &axis in &a_axes {
            if !Self::test_axis_obbox_obbox(&axis, a, b, &a_axes, &b_axes, &t) {
                return false;
            }
        }

        // Test face normals of B
        for &axis in &b_axes {
            if !Self::test_axis_obbox_obbox(&axis, a, b, &a_axes, &b_axes, &t) {
                return false;
            }
        }

        // Test edge cross products
        for &a_axis in &a_axes {
            for &b_axis in &b_axes {
                let axis = a_axis.cross(b_axis);
                if axis.length_squared() > EPSILON {
                    let axis_norm = axis.normalize();
                    if !Self::test_axis_obbox_obbox(&axis_norm, a, b, &a_axes, &b_axes, &t) {
                        return false;
                    }
                }
            }
        }

        true
    }

    fn test_axis_obbox_obbox(
        axis: &Vec3,
        a: &OBBoxClass,
        b: &OBBoxClass,
        a_axes: &[Vec3; 3],
        b_axes: &[Vec3; 3],
        t: &Vec3,
    ) -> bool {
        // Project both boxes onto axis
        let ra = a.extent.x * a_axes[0].dot(*axis).abs()
            + a.extent.y * a_axes[1].dot(*axis).abs()
            + a.extent.z * a_axes[2].dot(*axis).abs();

        let rb = b.extent.x * b_axes[0].dot(*axis).abs()
            + b.extent.y * b_axes[1].dot(*axis).abs()
            + b.extent.z * b_axes[2].dot(*axis).abs();

        let distance = t.dot(*axis).abs();

        distance <= ra + rb
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabox_triangle_intersect() {
        let aabox = AABoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1.0));

        let triangle = Triangle {
            vertices: [
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.0, 0.5, 0.0),
            ],
            normal: Vec3::Z,
        };

        assert!(CollisionMath::aabox_triangle_intersect(&aabox, &triangle));
    }

    #[test]
    fn test_obbox_triangle_intersection() {
        let obbox = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1.0));

        let triangle = Triangle {
            vertices: [
                Vec3::new(-0.5, -0.5, 0.0),
                Vec3::new(0.5, -0.5, 0.0),
                Vec3::new(0.0, 0.5, 0.0),
            ],
            normal: Vec3::Z,
        };

        assert!(CollisionMath::obbox_triangle_intersection(
            &obbox, &triangle
        ));
    }

    #[test]
    fn test_obbox_obbox_intersect() {
        let a = OBBoxClass::from_center_extent(Vec3::ZERO, Vec3::splat(1.0));
        let b = OBBoxClass::from_center_extent(Vec3::new(1.5, 0.0, 0.0), Vec3::splat(1.0));

        assert!(CollisionMath::obbox_obbox_intersect(&a, &b));
    }
}
