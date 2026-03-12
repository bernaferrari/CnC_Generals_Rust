use super::{Vector2, Vector3, Vector4};
use crate::EPSILON;

/// Flags for triangle raycast operations
pub const TRI_RAYCAST_FLAG_NONE: u8 = 0x00;
pub const TRI_RAYCAST_FLAG_HIT_EDGE: u8 = 0x01;
pub const TRI_RAYCAST_FLAG_START_IN_TRI: u8 = 0x02;

/// A 3D triangle defined by three vertices and a normal vector
///
/// This triangle representation is used for collision detection and geometric operations.
/// The triangle can hold references to vertices or own them directly.
#[derive(Debug, Clone, PartialEq)]
pub struct Triangle {
    pub vertices: [Vector3; 3],
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
        self.normal = edge1.cross(edge2).normalize();
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

        if abs_normal.x > abs_normal.y {
            if abs_normal.x > abs_normal.z {
                // X is dominant - use Y and Z axes
                (1, 2)
            } else {
                // Z is dominant - use X and Y axes
                (0, 1)
            }
        } else if abs_normal.y > abs_normal.z {
            // Y is dominant - use X and Z axes
            (0, 2)
        } else {
            // Z is dominant - use X and Y axes
            (0, 1)
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

/// Test if a point is inside a triangle in 2D
/// This is a utility function used by triangle and raycast operations
pub fn point_in_triangle_2d(
    tri_p0: Vector3,
    tri_p1: Vector3,
    tri_p2: Vector3,
    test_point: Vector3,
    axis1: usize,
    axis2: usize,
    flags: &mut u8,
) -> bool {
    // Based on checking signs of determinants - checking which side of each line a point lies
    let p0p1 = Vector2::new(tri_p1[axis1] - tri_p0[axis1], tri_p1[axis2] - tri_p0[axis2]);
    let p1p2 = Vector2::new(tri_p2[axis1] - tri_p1[axis1], tri_p2[axis2] - tri_p1[axis2]);
    let p2p0 = Vector2::new(tri_p0[axis1] - tri_p2[axis1], tri_p0[axis2] - tri_p2[axis2]);

    // Check which side P2 is relative to P0P1 to determine triangle winding
    let p0p2 = Vector2::new(tri_p2[axis1] - tri_p0[axis1], tri_p2[axis2] - tri_p0[axis2]);
    let p0p1p2 = Vector2::perp_dot(p0p1, p0p2);

    if p0p1p2.abs() < EPSILON {
        // Triangle is degenerate - handle as line segment
        return handle_degenerate_triangle(tri_p0, tri_p1, tri_p2, test_point, axis1, axis2, flags);
    }

    // Triangle is not degenerate - test three sides
    let side_factor = if p0p1p2 > 0.0 { 1.0 } else { -1.0 };
    let mut factors = [0.0; 3];

    // Test against each edge
    let p0p_test = Vector2::new(
        test_point[axis1] - tri_p0[axis1],
        test_point[axis2] - tri_p0[axis2],
    );
    factors[0] = Vector2::perp_dot(p0p1, p0p_test);
    if factors[0] * side_factor < 0.0 {
        return false;
    }

    let p1p_test = Vector2::new(
        test_point[axis1] - tri_p1[axis1],
        test_point[axis2] - tri_p1[axis2],
    );
    factors[1] = Vector2::perp_dot(p1p2, p1p_test);
    if factors[1] * side_factor < 0.0 {
        return false;
    }

    let p2p_test = Vector2::new(
        test_point[axis1] - tri_p2[axis1],
        test_point[axis2] - tri_p2[axis2],
    );
    factors[2] = Vector2::perp_dot(p2p0, p2p_test);
    if factors[2] * side_factor < 0.0 {
        return false;
    }

    // Check if point is exactly on an edge
    if factors[0].abs() < EPSILON || factors[1].abs() < EPSILON || factors[2].abs() < EPSILON {
        *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
    }

    true
}

/// Handle degenerate triangle case (collinear points)
fn handle_degenerate_triangle(
    tri_p0: Vector3,
    tri_p1: Vector3,
    tri_p2: Vector3,
    test_point: Vector3,
    axis1: usize,
    axis2: usize,
    flags: &mut u8,
) -> bool {
    // Find the two outer points along the triangle's line
    let p0p1 = Vector2::new(tri_p1[axis1] - tri_p0[axis1], tri_p1[axis2] - tri_p0[axis2]);
    let p1p2 = Vector2::new(tri_p2[axis1] - tri_p1[axis1], tri_p2[axis2] - tri_p1[axis2]);
    let p2p0 = Vector2::new(tri_p0[axis1] - tri_p2[axis1], tri_p0[axis2] - tri_p2[axis2]);

    let p0p1_dist2 = p0p1.length_squared();
    let p1p2_dist2 = p1p2.length_squared();
    let p2p0_dist2 = p2p0.length_squared();

    let (start_point, end_vec, max_dist2) = if p0p1_dist2 >= p1p2_dist2 && p0p1_dist2 >= p2p0_dist2
    {
        (tri_p0, p0p1, p0p1_dist2)
    } else if p1p2_dist2 >= p2p0_dist2 {
        (tri_p1, p1p2, p1p2_dist2)
    } else {
        (tri_p2, p2p0, p2p0_dist2)
    };

    if max_dist2 < EPSILON * EPSILON {
        // All points coincide
        let test_vec = Vector2::new(
            test_point[axis1] - start_point[axis1],
            test_point[axis2] - start_point[axis2],
        );
        if test_vec.length_squared() < EPSILON * EPSILON {
            *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
            return true;
        }
        return false;
    }

    // Triangle is a line segment - check if test point is collinear and within bounds
    let start_to_test = Vector2::new(
        test_point[axis1] - start_point[axis1],
        test_point[axis2] - start_point[axis2],
    );

    if Vector2::perp_dot(end_vec, start_to_test).abs() > EPSILON {
        // Not collinear
        return false;
    }

    // Collinear - check if within segment bounds
    let end_to_test = start_to_test - end_vec;
    if start_to_test.length_squared() <= max_dist2 && end_to_test.length_squared() <= max_dist2 {
        *flags |= TRI_RAYCAST_FLAG_HIT_EDGE;
        true
    } else {
        false
    }
}

/// Test a semi-infinite axis-aligned ray against a triangle
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
        if tri_plane[axis_r].abs() > EPSILON {
            // Start point is embedded in triangle plane
            *flags |= flags_2d & TRI_RAYCAST_FLAG_HIT_EDGE;
            *flags |= TRI_RAYCAST_FLAG_START_IN_TRI;
            true
        } else {
            // Ray is parallel to plane - check if start is in triangle
            let triangle = Triangle::new(tri_p0, tri_p1, tri_p2);
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
