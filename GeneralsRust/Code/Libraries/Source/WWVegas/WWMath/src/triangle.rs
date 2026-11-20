//! WWMath triangle geometry from `tri.h` and `tri.cpp`.
//!
//! The Rust triangle owns initialized vertices and a normal. The point and ray
//! tests retain the original ordered floating-point and flag behavior.

use super::{Vector2, Vector3, Vector4};
use crate::EPSILON;
use crate::vector2::perp_dot_product;

/// No point or ray observation flags.
pub const TRI_RAYCAST_FLAG_NONE: u8 = 0x00;
/// The ray meets a projected triangle edge.
pub const TRI_RAYCAST_FLAG_HIT_EDGE: u8 = 0x01;
/// The ray origin lies on the triangle.
pub const TRI_RAYCAST_FLAG_START_IN_TRI: u8 = 0x02;

/// A 3D triangle defined by three vertices and a normal vector
///
/// This triangle representation is used for collision detection and geometric operations.
#[derive(Debug, Clone, PartialEq)]
pub struct Triangle {
    /// Ordered triangle corners.
    pub vertices: [Vector3; 3],
    /// Triangle normal, computed from the corners or provided by the caller.
    pub normal: Vector3,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            vertices: [Vector3::ZERO; 3],
            normal: Vector3::new(0.0, 0.0, 1.0),
        }
    }
}

impl Triangle {
    /// Create a new triangle from three vertices
    pub fn new(v0: Vector3, v1: Vector3, v2: Vector3) -> Self {
        let mut triangle = Self {
            vertices: [v0, v1, v2],
            normal: Vector3::ZERO,
        };
        triangle.compute_normal();
        triangle
    }

    /// Create a triangle with explicit normal
    pub fn with_normal(v0: Vector3, v1: Vector3, v2: Vector3, normal: Vector3) -> Self {
        Self {
            vertices: [v0, v1, v2],
            normal,
        }
    }

    /// Compute and update the normal vector from the vertices
    pub fn compute_normal(&mut self) {
        let edge1 = self.vertices[1] - self.vertices[0];
        let edge2 = self.vertices[2] - self.vertices[0];
        let normal = edge1.cross(edge2);
        let squared = (normal.x * normal.x + normal.y * normal.y) + normal.z * normal.z;
        // Vector3::Normalize leaves a zero normal unchanged. This is the
        // original portable Inv_Sqrt branch, not the unrelated Quake helper.
        self.normal = if squared != 0.0 {
            normal * (1.0 / (f64::from(squared).sqrt() as f32))
        } else {
            normal
        };
    }

    /// Test if a point is contained within the triangle
    /// Assumes the point is in the plane of the triangle
    pub fn contains_point(&self, point: Vector3) -> bool {
        // Find the dominant plane to project onto for 2D test
        let (axis1, axis2) = self.find_dominant_plane();

        // Use optimized 2D point-in-triangle test
        self.point_in_triangle_2d_optimized(point, axis1, axis2)
    }

    /// Find the dominant plane (the plane most perpendicular to the normal)
    /// Returns the two axes to use for 2D projection
    pub fn find_dominant_plane(&self) -> (usize, usize) {
        let abs_normal = Vector3::new(
            self.normal.x.abs(),
            self.normal.y.abs(),
            self.normal.z.abs(),
        );

        // tri.cpp uses strict greater-than updates starting with X, so
        // equal magnitudes preserve X before Y before Z.
        let mut dominant = 0;
        let mut value = abs_normal.x;
        if abs_normal.y > value {
            dominant = 1;
            value = abs_normal.y;
        }
        if abs_normal.z > value {
            dominant = 2;
        }
        match dominant {
            0 => (1, 2),
            1 => (0, 2),
            _ => (0, 1),
        }
    }

    /// Optimized 2D point-in-triangle test using cross products
    fn point_in_triangle_2d_optimized(&self, point: Vector3, axis1: usize, axis2: usize) -> bool {
        // Compute 2D cross products to determine which side of each edge the point is on
        let mut sides = [false; 3];

        for (i, side) in sides.iter_mut().enumerate() {
            let va = i;
            let vb = (i + 1) % 3;

            let edge_x = self.vertices[vb][axis1] - self.vertices[va][axis1];
            let edge_y = self.vertices[vb][axis2] - self.vertices[va][axis2];
            let dp_x = point[axis1] - self.vertices[va][axis1];
            let dp_y = point[axis2] - self.vertices[va][axis2];

            let cross = edge_x * dp_y - edge_y * dp_x;
            *side = cross >= 0.0;
        }

        // Point is inside if it's on the same side of all three edges
        sides[0] == sides[1] && sides[1] == sides[2]
    }

    /// Get the area of the triangle
    pub fn area(&self) -> f32 {
        let edge1 = self.vertices[1] - self.vertices[0];
        let edge2 = self.vertices[2] - self.vertices[0];
        edge1.cross(edge2).length() * 0.5
    }

    /// Get the centroid (center point) of the triangle
    pub fn centroid(&self) -> Vector3 {
        (self.vertices[0] + self.vertices[1] + self.vertices[2]) / 3.0
    }

    /// Get the perimeter of the triangle
    pub fn perimeter(&self) -> f32 {
        let edge1 = (self.vertices[1] - self.vertices[0]).length();
        let edge2 = (self.vertices[2] - self.vertices[1]).length();
        let edge3 = (self.vertices[0] - self.vertices[2]).length();
        edge1 + edge2 + edge3
    }

    /// Transform the triangle by a matrix
    pub fn transform(&mut self, matrix: &crate::matrix3d::Matrix3D) {
        for vertex in &mut self.vertices {
            *vertex = matrix.transform_vector(*vertex);
        }
        self.compute_normal();
    }

    /// Get a transformed copy of the triangle
    pub fn transformed(&self, matrix: &crate::matrix3d::Matrix3D) -> Self {
        let mut result = self.clone();
        result.transform(matrix);
        result
    }

    /// Get the closest point on the triangle to a given point
    pub fn closest_point_to(&self, point: Vector3) -> Vector3 {
        // Project point onto triangle plane
        let to_point = point - self.vertices[0];
        let dist_to_plane = to_point.dot(self.normal);
        let projected = point - self.normal * dist_to_plane;

        // If projected point is inside triangle, return it
        if self.contains_point(projected) {
            return projected;
        }

        // Otherwise, find closest point on triangle edges
        let mut closest = self.vertices[0];
        let mut min_dist_sq = (point - closest).length_squared();

        // Check all three edges
        for i in 0..3 {
            let v1 = self.vertices[i];
            let v2 = self.vertices[(i + 1) % 3];
            let edge_closest = closest_point_on_line_segment(point, v1, v2);
            let dist_sq = (point - edge_closest).length_squared();

            if dist_sq < min_dist_sq {
                min_dist_sq = dist_sq;
                closest = edge_closest;
            }
        }

        closest
    }

    /// Compute barycentric coordinates of a point with respect to the triangle
    /// Returns (u, v, w) where point = u*v0 + v*v1 + w*v2 and u+v+w=1
    pub fn barycentric_coordinates(&self, point: Vector3) -> (f32, f32, f32) {
        let v0 = self.vertices[1] - self.vertices[0];
        let v1 = self.vertices[2] - self.vertices[0];
        let v2 = point - self.vertices[0];

        let dot00 = v0.dot(v0);
        let dot01 = v0.dot(v1);
        let dot02 = v0.dot(v2);
        let dot11 = v1.dot(v1);
        let dot12 = v1.dot(v2);

        let inv_denom = 1.0 / (dot00 * dot11 - dot01 * dot01);
        let u = (dot11 * dot02 - dot01 * dot12) * inv_denom;
        let v = (dot00 * dot12 - dot01 * dot02) * inv_denom;
        let w = 1.0 - u - v;

        (w, u, v) // Reorder to match vertex order
    }
}

/// Find the closest point on a line segment to a given point
fn closest_point_on_line_segment(
    point: Vector3,
    line_start: Vector3,
    line_end: Vector3,
) -> Vector3 {
    let line_vec = line_end - line_start;
    let line_len_sq = line_vec.length_squared();

    if line_len_sq < EPSILON * EPSILON {
        // Line segment is degenerate
        return line_start;
    }

    let t = (point - line_start).dot(line_vec) / line_len_sq;
    let clamped_t = t.clamp(0.0, 1.0);

    line_start + line_vec * clamped_t
}

/// Test whether the point falls within the triangle projected onto two axes.
/// A hit on the boundary ORs the edge bit into the caller’s flags.
pub fn point_in_triangle_2d(
    tri_p0: Vector3,
    tri_p1: Vector3,
    tri_p2: Vector3,
    test_point: Vector3,
    axis1: usize,
    axis2: usize,
    flags: &mut u8,
) -> bool {
    let projected = |point: Vector3| Vector2::new(point[axis1], point[axis2]);
    let [a, b, c, point] = [tri_p0, tri_p1, tri_p2, test_point].map(projected);
    let ab = b - a;
    let bc = c - b;
    let ca = a - c;
    let winding = perp_dot_product(ab, c - a);

    if winding != 0.0 {
        let side = if winding > 0.0 { 1.0 } else { -1.0 };
        let mut edge_hit = false;
        for (edge, origin) in [(ab, a), (bc, b), (ca, c)] {
            let cross = perp_dot_product(edge, point - origin);
            if cross * side < 0.0 {
                return false;
            }
            edge_hit |= cross == 0.0;
        }
        if edge_hit {
            *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
        }
        return true;
    }

    // The original computes the C→A distance from B→C's components. Keep
    // that distance and its strict tie behavior for collinear triangles.
    let ab_len2 = ab.length_squared();
    let bc_len2 = bc.length_squared();
    let (edge, offset, len2) = if ab_len2 > bc_len2 {
        (ab, point - a, ab_len2)
    } else {
        (ca, point - c, bc_len2)
    };
    let hit = if len2 != 0.0 {
        perp_dot_product(edge, offset) == 0.0
            && offset.length_squared() <= len2
            && (offset - edge).length_squared() <= len2
    } else {
        offset.length_squared() == 0.0
    };
    if hit {
        *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
    }
    hit
}

/// Test a ray on a principal axis against a triangle with plane equation `Ax + By + Cz + D = 0`.
/// Direction 0 follows the negative axis and direction 1 follows the positive axis.
/// The caller’s unrelated flags are retained.
#[allow(clippy::too_many_arguments)]
pub fn cast_semi_infinite_axis_aligned_ray_to_triangle(
    tri_p0: Vector3,
    tri_p1: Vector3,
    tri_p2: Vector3,
    tri_plane: Vector4,
    ray_start: Vector3,
    axis_r: usize,
    axis1: usize,
    axis2: usize,
    direction: usize, // 0 for negative direction, 1 for positive
    flags: &mut u8,
) -> bool {
    // First check infinite ray vs triangle (2D check)
    let mut flags_2d = TRI_RAYCAST_FLAG_NONE;
    if !point_in_triangle_2d(
        tri_p0,
        tri_p1,
        tri_p2,
        ray_start,
        axis1,
        axis2,
        &mut flags_2d,
    ) {
        return false;
    }

    // Ray projection intersects triangle - now check 3D intersection
    let sign = if direction == 0 { -1.0 } else { 1.0 };
    let plane_dist = tri_plane.x * ray_start.x
        + tri_plane.y * ray_start.y
        + tri_plane.z * ray_start.z
        + tri_plane.w;
    let result = tri_plane[axis_r] * sign * plane_dist;

    if result < 0.0 {
        // Intersection!
        *flags |= flags_2d & TRI_RAYCAST_FLAG_HIT_EDGE;
        true
    } else if result == 0.0 {
        // Ray start is on plane or ray is parallel to plane
        if tri_plane[axis_r] != 0.0 {
            // Start point is embedded in triangle plane
            *flags |= flags_2d & TRI_RAYCAST_FLAG_HIT_EDGE;
            *flags |= TRI_RAYCAST_FLAG_START_IN_TRI;
            true
        } else {
            // Ray is parallel to plane - check if start is in triangle
            let triangle = Triangle::with_normal(tri_p0, tri_p1, tri_p2, tri_plane.truncate());
            if triangle.contains_point(ray_start) {
                *flags |= TRI_RAYCAST_FLAG_START_IN_TRI;
            }
            false
        }
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_triangle_new() {
        let v0 = Vector3::new(0.0, 0.0, 0.0);
        let v1 = Vector3::new(1.0, 0.0, 0.0);
        let v2 = Vector3::new(0.0, 1.0, 0.0);
        let triangle = Triangle::new(v0, v1, v2);

        assert_eq!(triangle.vertices[0], v0);
        assert_eq!(triangle.vertices[1], v1);
        assert_eq!(triangle.vertices[2], v2);

        // Normal should point in +Z direction
        assert!((triangle.normal - Vector3::new(0.0, 0.0, 1.0)).length() < EPSILON);
    }

    #[test]
    fn test_triangle_area() {
        let triangle = Triangle::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
        );

        assert!((triangle.area() - 2.0).abs() < EPSILON);
    }

    #[test]
    fn test_triangle_centroid() {
        let triangle = Triangle::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(3.0, 0.0, 0.0),
            Vector3::new(0.0, 3.0, 0.0),
        );

        let expected_centroid = Vector3::new(1.0, 1.0, 0.0);
        assert!((triangle.centroid() - expected_centroid).length() < EPSILON);
    }

    #[test]
    fn test_triangle_contains_point() {
        let triangle = Triangle::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
        );

        // Point inside triangle
        assert!(triangle.contains_point(Vector3::new(0.5, 0.5, 0.0)));

        // Point outside triangle
        assert!(!triangle.contains_point(Vector3::new(2.0, 2.0, 0.0)));

        // Point on edge
        assert!(triangle.contains_point(Vector3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn test_triangle_find_dominant_plane() {
        // Triangle in XY plane - Z should be dominant
        let triangle = Triangle::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );

        let (axis1, axis2) = triangle.find_dominant_plane();
        assert_eq!(axis1, 0); // X axis
        assert_eq!(axis2, 1); // Y axis
    }

    #[test]
    fn test_triangle_barycentric_coordinates() {
        let triangle = Triangle::new(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.0, 1.0, 0.0),
        );

        // Test center point
        let (u, v, w) = triangle.barycentric_coordinates(Vector3::new(1.0 / 3.0, 1.0 / 3.0, 0.0));
        assert!((u - 1.0 / 3.0).abs() < EPSILON);
        assert!((v - 1.0 / 3.0).abs() < EPSILON);
        assert!((w - 1.0 / 3.0).abs() < EPSILON);

        // Test vertex positions
        let (u, v, w) = triangle.barycentric_coordinates(triangle.vertices[0]);
        assert!((u - 1.0).abs() < EPSILON);
        assert!(v.abs() < EPSILON);
        assert!(w.abs() < EPSILON);
    }

    #[test]
    fn test_point_in_triangle_2d() {
        let tri_p0 = Vector3::new(0.0, 0.0, 0.0);
        let tri_p1 = Vector3::new(1.0, 0.0, 0.0);
        let tri_p2 = Vector3::new(0.0, 1.0, 0.0);

        let mut flags = TRI_RAYCAST_FLAG_NONE;

        // Point inside triangle
        assert!(point_in_triangle_2d(
            tri_p0,
            tri_p1,
            tri_p2,
            Vector3::new(0.25, 0.25, 0.0),
            0,
            1,
            &mut flags
        ));

        // Point outside triangle
        assert!(!point_in_triangle_2d(
            tri_p0,
            tri_p1,
            tri_p2,
            Vector3::new(1.0, 1.0, 0.0),
            0,
            1,
            &mut flags
        ));
    }

    #[test]
    fn test_closest_point_on_line_segment() {
        let start = Vector3::new(0.0, 0.0, 0.0);
        let end = Vector3::new(10.0, 0.0, 0.0);

        // Point on line
        let closest = closest_point_on_line_segment(Vector3::new(5.0, 0.0, 0.0), start, end);
        assert_eq!(closest, Vector3::new(5.0, 0.0, 0.0));

        // Point off line, middle
        let closest = closest_point_on_line_segment(Vector3::new(5.0, 2.0, 0.0), start, end);
        assert_eq!(closest, Vector3::new(5.0, 0.0, 0.0));

        // Point beyond start
        let closest = closest_point_on_line_segment(Vector3::new(-5.0, 0.0, 0.0), start, end);
        assert_eq!(closest, start);

        // Point beyond end
        let closest = closest_point_on_line_segment(Vector3::new(15.0, 0.0, 0.0), start, end);
        assert_eq!(closest, end);
    }
}
