//! Oriented Bounding Box (OBBox) implementation.
//!
//! This module provides oriented bounding box functionality,
//! converted from the original C++ OBBoxClass.
//!
//! An oriented bounding box represents a collision box in world space with:
//! - Center: position of the center of the box
//! - Extent: size of the box (half-widths along each axis)
//! - Basis: rotation matrix defining the orientation of the box
//!
//! To find the world space coordinates of the "+x,+y,+z" corner of 
//! the bounding box you could use this equation:
//! Vector3 corner = center + basis * extent;

use crate::{Vector3, Matrix3, Matrix3D, WWMath, EPSILON};
// Note: operator traits may be used in the future for OBBox operations

/// Oriented Bounding Box
#[derive(Debug, Copy, Clone)]
pub struct OBBox {
    /// Rotation matrix defining the orientation of the box
    pub basis: Matrix3,
    /// Position of the center of the box
    pub center: Vector3,
    /// Size of the box (half-widths along each axis)
    pub extent: Vector3,
}

impl OBBox {
    /// Create a new OBBox with default values
    pub fn new() -> Self {
        Self {
            basis: Matrix3::IDENTITY,
            center: Vector3::ZERO,
            extent: Vector3::ZERO,
        }
    }

    /// Create an OBBox from center and extent (identity orientation)
    pub fn from_center_extent(center: Vector3, extent: Vector3) -> Self {
        Self {
            basis: Matrix3::IDENTITY,
            center,
            extent,
        }
    }

    /// Create an OBBox from center, extent, and orientation
    pub fn from_center_extent_basis(center: Vector3, extent: Vector3, basis: Matrix3) -> Self {
        Self {
            basis,
            center,
            extent,
        }
    }

    /// Create an OBBox from a set of points (unimplemented - placeholder for future)
    pub fn from_points(_points: &[Vector3]) -> Self {
        // TODO: Implement PCA-based OBBox fitting
        // For now, return a default box
        Self::new()
    }

    /// Initialize from 8 corner points of a box
    pub fn init_from_box_points(&mut self, points: &[Vector3]) {
        assert_eq!(points.len(), 8, "Must provide exactly 8 corner points");

        // Compute vectors from first point to all others
        let mut dp = Vec::with_capacity(7);
        for i in 1..8 {
            dp.push(points[i] - points[0]);
        }

        // Sort by length to find shortest two candidate axes
        dp.sort_by(|a, b| a.length_squared().partial_cmp(&b.length_squared()).unwrap());

        // Use the two shortest vectors as basis for first two axes
        let axis0 = dp[0].normalize();
        let axis1 = dp[1].normalize();
        let axis2 = axis0.cross(axis1);

        self.basis = Matrix3::from_rows(axis0, axis1, axis2);

        // Center is the average of all points
        self.center = Vector3::ZERO;
        for point in points {
            self.center += *point;
        }
        self.center = self.center / points.len() as f32;

        // Compute extents along the computed axes
        self.extent = Vector3::ZERO;

        for point in points {
            let delta = *point - self.center;

            let x_proj = WWMath::fabs(axis0.dot(delta));
            if x_proj > self.extent.x {
                self.extent.x = x_proj;
            }

            let y_proj = WWMath::fabs(axis1.dot(delta));
            if y_proj > self.extent.y {
                self.extent.y = y_proj;
            }

            let z_proj = WWMath::fabs(axis2.dot(delta));
            if z_proj > self.extent.z {
                self.extent.z = z_proj;
            }
        }
    }

    /// Initialize a random oriented box
    pub fn init_random(&mut self, min_extent: f32, max_extent: f32) {
        self.center = Vector3::ZERO;
        
        self.extent.x = min_extent + WWMath::random_float() * (max_extent - min_extent);
        self.extent.y = min_extent + WWMath::random_float() * (max_extent - min_extent);
        self.extent.z = min_extent + WWMath::random_float() * (max_extent - min_extent);

        // Create random orientation using quaternion
        let mut orient = crate::wwmath::Quaternion::from_components(
            WWMath::random_float(),
            WWMath::random_float(),
            WWMath::random_float(),
            WWMath::random_float(),
        );
        orient.normalize();

        self.basis = Matrix3::from_quaternion(&orient);
    }

    /// Project the box onto the given axis
    pub fn project_to_axis(&self, axis: Vector3) -> f32 {
        let x = self.extent.x * axis.dot(self.basis.get_x_vector());
        let y = self.extent.y * axis.dot(self.basis.get_y_vector());
        let z = self.extent.z * axis.dot(self.basis.get_z_vector());

        // Projection is the sum of the absolute values of the projections of the three extents
        WWMath::fabs(x) + WWMath::fabs(y) + WWMath::fabs(z)
    }

    /// Calculate the volume of the box
    pub fn volume(&self) -> f32 {
        8.0 * self.extent.x * self.extent.y * self.extent.z
    }

    /// Compute position of a parametrically defined point
    /// point = center + params[0]*axis0*extent.x + params[1]*axis1*extent.y + params[2]*axis2*extent.z
    /// where -1 <= params[i] <= 1
    pub fn compute_point(&self, params: [f32; 3]) -> Vector3 {
        let mut point = Vector3::new(
            self.extent.x * params[0],
            self.extent.y * params[1],
            self.extent.z * params[2],
        );

        point = self.basis * point;
        point + self.center
    }

    /// Compute extent of an axis-aligned box that encloses this oriented box
    pub fn compute_axis_aligned_extent(&self) -> Vector3 {
        Vector3::new(
            // X extent is the box projected onto the X axis
            WWMath::fabs(self.extent.x * self.basis[0].x) +
            WWMath::fabs(self.extent.y * self.basis[0].y) +
            WWMath::fabs(self.extent.z * self.basis[0].z),
            
            // Y extent
            WWMath::fabs(self.extent.x * self.basis[1].x) +
            WWMath::fabs(self.extent.y * self.basis[1].y) +
            WWMath::fabs(self.extent.z * self.basis[1].z),
            
            // Z extent
            WWMath::fabs(self.extent.x * self.basis[2].x) +
            WWMath::fabs(self.extent.y * self.basis[2].y) +
            WWMath::fabs(self.extent.z * self.basis[2].z),
        )
    }

    /// Transform this OBBox by a transformation matrix
    pub fn transform(&self, transform: &Matrix3D) -> Self {
        let mut result_basis = Matrix3::IDENTITY;
        Matrix3::multiply_matrix3d_matrix3(transform, &self.basis, &mut result_basis);
        
        Self {
            extent: self.extent, // Extents don't change
            center: transform.transform_vector(self.center),
            basis: result_basis,
        }
    }

    /// Transform an OBBox into a result OBBox
    pub fn transform_into(transform: &Matrix3D, input: &Self, output: &mut Self) {
        output.extent = input.extent;
        output.center = transform.transform_vector(input.center);
        Matrix3::multiply_matrix3d_matrix3(transform, &input.basis, &mut output.basis);
    }

    /// Get the 8 corner points of the box
    pub fn get_corners(&self) -> [Vector3; 8] {
        let params = [
            [-1.0, -1.0, -1.0], [1.0, -1.0, -1.0], [-1.0, 1.0, -1.0], [1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],  [1.0, -1.0, 1.0],  [-1.0, 1.0, 1.0],  [1.0, 1.0, 1.0],
        ];

        let mut corners = [Vector3::ZERO; 8];
        for (i, param_set) in params.iter().enumerate() {
            corners[i] = self.compute_point(*param_set);
        }
        corners
    }

    /// Check if this OBBox contains a point
    pub fn contains_point(&self, point: Vector3) -> bool {
        let local_point = self.world_to_local(point);
        local_point.x.abs() <= self.extent.x &&
        local_point.y.abs() <= self.extent.y &&
        local_point.z.abs() <= self.extent.z
    }

    /// Transform a point from world space to local box space
    pub fn world_to_local(&self, world_point: Vector3) -> Vector3 {
        let delta = world_point - self.center;
        self.basis.transpose() * delta
    }

    /// Transform a point from local box space to world space
    pub fn local_to_world(&self, local_point: Vector3) -> Vector3 {
        self.center + self.basis * local_point
    }

    /// Get the closest point on this OBBox to another point
    pub fn closest_point_to(&self, point: Vector3) -> Vector3 {
        let local_point = self.world_to_local(point);
        let clamped = Vector3::new(
            WWMath::clamp(local_point.x, -self.extent.x, self.extent.x),
            WWMath::clamp(local_point.y, -self.extent.y, self.extent.y),
            WWMath::clamp(local_point.z, -self.extent.z, self.extent.z),
        );
        self.local_to_world(clamped)
    }

    /// Get the distance from this OBBox to a point (0 if point is inside)
    pub fn distance_to_point(&self, point: Vector3) -> f32 {
        let local_point = self.world_to_local(point);
        let dx = WWMath::max(0.0, local_point.x.abs() - self.extent.x);
        let dy = WWMath::max(0.0, local_point.y.abs() - self.extent.y);
        let dz = WWMath::max(0.0, local_point.z.abs() - self.extent.z);
        (dx * dx + dy * dy + dz * dz).sqrt()
    }

    /// Expand the box by a margin in all directions
    pub fn expand(&mut self, margin: f32) {
        self.extent.x += margin;
        self.extent.y += margin;
        self.extent.z += margin;
    }

    /// Create an expanded version of this box
    pub fn expanded(&self, margin: f32) -> Self {
        Self {
            basis: self.basis,
            center: self.center,
            extent: Vector3::new(
                self.extent.x + margin,
                self.extent.y + margin,
                self.extent.z + margin,
            ),
        }
    }

    /// Merge this box with another box to create a bounding box that contains both
    pub fn merge_with(&self, other: &Self) -> Self {
        // Simple approach: get all corners and create AABB, then convert to OBBox
        // This is not optimal but works correctly
        let mut all_corners = Vec::with_capacity(16);
        all_corners.extend_from_slice(&self.get_corners());
        all_corners.extend_from_slice(&other.get_corners());

        let mut min_point = all_corners[0];
        let mut max_point = all_corners[0];

        for corner in &all_corners {
            min_point.x = WWMath::min(min_point.x, corner.x);
            min_point.y = WWMath::min(min_point.y, corner.y);
            min_point.z = WWMath::min(min_point.z, corner.z);
            max_point.x = WWMath::max(max_point.x, corner.x);
            max_point.y = WWMath::max(max_point.y, corner.y);
            max_point.z = WWMath::max(max_point.z, corner.z);
        }

        let center = (min_point + max_point) * 0.5;
        let extent = (max_point - min_point) * 0.5;

        Self::from_center_extent(center, extent)
    }
}

impl Default for OBBox {
    fn default() -> Self {
        Self::new()
    }
}

impl PartialEq for OBBox {
    fn eq(&self, other: &Self) -> bool {
        self.center == other.center &&
        self.extent == other.extent &&
        self.basis == other.basis
    }
}

// Intersection and collision detection functions

/// Test if two boxes intersect on a given axis
pub fn oriented_boxes_intersect_on_axis(box0: &OBBox, box1: &OBBox, axis: Vector3) -> bool {
    if axis.length_squared() < EPSILON {
        return true;
    }

    let ra = box0.project_to_axis(axis);
    let rb = box1.project_to_axis(axis);
    let rsum = ra + rb;

    // Project the center distance onto the axis
    let center_diff = box1.center - box0.center;
    let cdist = axis.dot(center_diff);

    cdist.abs() <= rsum
}

/// Test if two oriented boxes intersect (using Separating Axis Theorem)
pub fn oriented_boxes_intersect(box0: &OBBox, box1: &OBBox) -> bool {
    // Extract basis vectors
    let a = [
        box0.basis.get_x_vector(),
        box0.basis.get_y_vector(),
        box0.basis.get_z_vector(),
    ];
    let b = [
        box1.basis.get_x_vector(),
        box1.basis.get_y_vector(),
        box1.basis.get_z_vector(),
    ];

    // Test the 6 axes from both boxes
    for axis in &a {
        if !oriented_boxes_intersect_on_axis(box0, box1, *axis) {
            return false;
        }
    }

    for axis in &b {
        if !oriented_boxes_intersect_on_axis(box0, box1, *axis) {
            return false;
        }
    }

    // Test the 9 cross product axes
    for axis_a in &a {
        for axis_b in &b {
            let cross_axis = axis_a.cross(*axis_b);
            if !oriented_boxes_intersect_on_axis(box0, box1, cross_axis) {
                return false;
            }
        }
    }

    // None of the above tests separated the two boxes, so they are intersecting
    true
}

/// Test if two boxes collide on a given axis (considering motion)
pub fn oriented_boxes_collide_on_axis(
    box0: &OBBox,
    v0: Vector3,
    box1: &OBBox,
    v1: Vector3,
    axis: Vector3,
    dt: f32,
) -> bool {
    if axis.length_squared() < EPSILON {
        return true;
    }

    let ra = box0.project_to_axis(axis);
    let rb = box1.project_to_axis(axis);
    let rsum = ra + rb;

    // Project the center distance and velocity onto the axis
    let center_diff = box1.center - box0.center;
    let velocity_diff = v1 - v0;

    let cdist = axis.dot(center_diff);
    let vdist = cdist + dt * axis.dot(velocity_diff);

    !((cdist > rsum && vdist > rsum) || (cdist < -rsum && vdist < -rsum))
}

/// Test if two oriented boxes collide (considering motion)
pub fn oriented_boxes_collide(
    box0: &OBBox,
    v0: Vector3,
    box1: &OBBox,
    v1: Vector3,
    dt: f32,
) -> bool {
    // Extract basis vectors
    let a = [
        box0.basis.get_x_vector(),
        box0.basis.get_y_vector(),
        box0.basis.get_z_vector(),
    ];
    let b = [
        box1.basis.get_x_vector(),
        box1.basis.get_y_vector(),
        box1.basis.get_z_vector(),
    ];

    // Test the 6 axes from both boxes
    for axis in &a {
        if !oriented_boxes_collide_on_axis(box0, v0, box1, v1, *axis, dt) {
            return false;
        }
    }

    for axis in &b {
        if !oriented_boxes_collide_on_axis(box0, v0, box1, v1, *axis, dt) {
            return false;
        }
    }

    // Test the 9 cross product axes
    for axis_a in &a {
        for axis_b in &b {
            let cross_axis = axis_a.cross(*axis_b);
            if !oriented_boxes_collide_on_axis(box0, v0, box1, v1, cross_axis, dt) {
                return false;
            }
        }
    }

    true
}

/// Triangle struct for intersection testing (simplified version)
#[derive(Debug, Copy, Clone)]
pub struct Triangle {
    pub vertices: [Vector3; 3],
    pub normal: Vector3,
}

impl Triangle {
    pub fn new(v0: Vector3, v1: Vector3, v2: Vector3) -> Self {
        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let normal = edge1.cross(edge2).normalize();
        
        Self {
            vertices: [v0, v1, v2],
            normal,
        }
    }
}

/// Test if a box intersects with a triangle on a given axis
pub fn oriented_box_intersects_tri_on_axis(obbox: &OBBox, tri: &Triangle, mut axis: Vector3) -> bool {
    if axis.length_squared() < EPSILON {
        return true;
    }

    let delta = tri.vertices[0] - obbox.center;
    let r1 = tri.vertices[1] - tri.vertices[0];
    let r2 = tri.vertices[2] - tri.vertices[0];

    // Make axis point from box center to tri.v0
    let dist = delta.dot(axis);
    if dist < 0.0 {
        axis = -axis;
    }
    let dist = dist.abs();

    // Compute leading edge of the box
    let box_projection = obbox.project_to_axis(axis);

    // Compute the leading edge of the triangle
    let mut tri_projection = 0.0;
    let tmp1 = r1.dot(axis);
    if tmp1 < tri_projection {
        tri_projection = tmp1;
    }
    let tmp2 = r2.dot(axis);
    if tmp2 < tri_projection {
        tri_projection = tmp2;
    }
    tri_projection += dist;

    tri_projection < box_projection
}

/// Test if an oriented box intersects with a triangle
pub fn oriented_box_intersects_tri(obbox: &OBBox, tri: &Triangle) -> bool {
    // Extract box axes
    let box_axes = [
        obbox.basis.get_x_vector(),
        obbox.basis.get_y_vector(),
        obbox.basis.get_z_vector(),
    ];

    // Extract triangle edges
    let tri_edges = [
        tri.vertices[1] - tri.vertices[0],
        tri.vertices[2] - tri.vertices[1],
        tri.vertices[0] - tri.vertices[2],
    ];

    // Test triangle normal
    if !oriented_box_intersects_tri_on_axis(obbox, tri, tri.normal) {
        return false;
    }

    // Test box axes
    for axis in &box_axes {
        if !oriented_box_intersects_tri_on_axis(obbox, tri, *axis) {
            return false;
        }
    }

    // Test cross products of box axes and triangle edges
    for box_axis in &box_axes {
        for tri_edge in &tri_edges {
            let cross_axis = box_axis.cross(*tri_edge);
            if !oriented_box_intersects_tri_on_axis(obbox, tri, cross_axis) {
                return false;
            }
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_obbox_creation() {
        let center = Vector3::new(1.0, 2.0, 3.0);
        let extent = Vector3::new(0.5, 1.0, 1.5);
        let obbox = OBBox::from_center_extent(center, extent);

        assert_eq!(obbox.center, center);
        assert_eq!(obbox.extent, extent);
        assert_eq!(obbox.basis, Matrix3::IDENTITY);
    }

    #[test]
    fn test_volume() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 2.0, 3.0)
        );
        assert_eq!(obbox.volume(), 48.0); // 8 * 1 * 2 * 3
    }

    #[test]
    fn test_compute_point() {
        let obbox = OBBox::from_center_extent(
            Vector3::new(5.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0)
        );

        // Test corner points
        let corner = obbox.compute_point([1.0, 1.0, 1.0]);
        assert_eq!(corner, Vector3::new(6.0, 1.0, 1.0));

        let opposite_corner = obbox.compute_point([-1.0, -1.0, -1.0]);
        assert_eq!(opposite_corner, Vector3::new(4.0, -1.0, -1.0));
    }

    #[test]
    fn test_axis_aligned_extent() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 2.0, 3.0)
        );

        let aa_extent = obbox.compute_axis_aligned_extent();
        assert_eq!(aa_extent, Vector3::new(1.0, 2.0, 3.0));
    }

    #[test]
    fn test_contains_point() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        );

        assert!(obbox.contains_point(Vector3::new(0.5, 0.5, 0.5)));
        assert!(obbox.contains_point(Vector3::new(1.0, 1.0, 1.0)));
        assert!(!obbox.contains_point(Vector3::new(1.5, 0.0, 0.0)));
    }

    #[test]
    fn test_world_local_transform() {
        let obbox = OBBox::from_center_extent(
            Vector3::new(5.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0)
        );

        let world_point = Vector3::new(6.0, 0.0, 0.0);
        let local_point = obbox.world_to_local(world_point);
        assert!((local_point - Vector3::new(1.0, 0.0, 0.0)).length() < 1e-6);

        let back_to_world = obbox.local_to_world(local_point);
        assert!((back_to_world - world_point).length() < 1e-6);
    }

    #[test]
    fn test_distance_to_point() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        );

        // Point inside
        assert_eq!(obbox.distance_to_point(Vector3::new(0.5, 0.5, 0.5)), 0.0);

        // Point outside
        let distance = obbox.distance_to_point(Vector3::new(3.0, 0.0, 0.0));
        assert!((distance - 2.0).abs() < 1e-6);
    }

    #[test]
    fn test_closest_point_to() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        );

        let outside_point = Vector3::new(3.0, 0.0, 0.0);
        let closest = obbox.closest_point_to(outside_point);
        assert!((closest - Vector3::new(1.0, 0.0, 0.0)).length() < 1e-6);
    }

    #[test]
    fn test_expand() {
        let mut obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        );

        obbox.expand(0.5);
        assert_eq!(obbox.extent, Vector3::new(1.5, 1.5, 1.5));

        let expanded = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        ).expanded(0.5);
        assert_eq!(expanded.extent, Vector3::new(1.5, 1.5, 1.5));
    }

    #[test]
    fn test_project_to_axis() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 2.0, 3.0)
        );

        let projection = obbox.project_to_axis(Vector3::new(1.0, 0.0, 0.0));
        assert_eq!(projection, 1.0);

        let projection_y = obbox.project_to_axis(Vector3::new(0.0, 1.0, 0.0));
        assert_eq!(projection_y, 2.0);
    }

    #[test]
    fn test_oriented_boxes_intersect() {
        let box1 = OBBox::from_center_extent(
            Vector3::new(0.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0)
        );

        let box2 = OBBox::from_center_extent(
            Vector3::new(1.5, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0)
        );

        // Boxes should intersect
        assert!(oriented_boxes_intersect(&box1, &box2));

        let box3 = OBBox::from_center_extent(
            Vector3::new(3.0, 0.0, 0.0),
            Vector3::new(1.0, 1.0, 1.0)
        );

        // Boxes should not intersect
        assert!(!oriented_boxes_intersect(&box1, &box3));
    }

    #[test]
    fn test_transform() {
        let obbox = OBBox::from_center_extent(
            Vector3::new(1.0, 0.0, 0.0),
            Vector3::new(0.5, 0.5, 0.5)
        );

        let mut transform = Matrix3D::IDENTITY;
        transform.translate(2.0, 0.0, 0.0);

        let transformed = obbox.transform(&transform);
        assert!((transformed.center - Vector3::new(3.0, 0.0, 0.0)).length() < 1e-6);
        assert_eq!(transformed.extent, obbox.extent);
    }

    #[test]
    fn test_get_corners() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        );

        let corners = obbox.get_corners();
        
        // Check that we have 8 corners
        assert_eq!(corners.len(), 8);
        
        // Check a few specific corners
        assert!(corners.contains(&Vector3::new(-1.0, -1.0, -1.0)));
        assert!(corners.contains(&Vector3::new(1.0, 1.0, 1.0)));
    }

    #[test]
    fn test_triangle_intersection() {
        let obbox = OBBox::from_center_extent(
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0)
        );

        // Triangle intersecting the box
        let tri = Triangle::new(
            Vector3::new(-2.0, 0.0, 0.0),
            Vector3::new(2.0, 0.0, 0.0),
            Vector3::new(0.0, 2.0, 0.0),
        );

        assert!(oriented_box_intersects_tri(&obbox, &tri));

        // Triangle not intersecting the box
        let tri_far = Triangle::new(
            Vector3::new(5.0, 5.0, 5.0),
            Vector3::new(6.0, 5.0, 5.0),
            Vector3::new(5.0, 6.0, 5.0),
        );

        assert!(!oriented_box_intersects_tri(&obbox, &tri_far));
    }

    #[test]
    fn test_equality() {
        let box1 = OBBox::from_center_extent(
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(0.5, 1.0, 1.5)
        );

        let box2 = OBBox::from_center_extent(
            Vector3::new(1.0, 2.0, 3.0),
            Vector3::new(0.5, 1.0, 1.5)
        );

        let box3 = OBBox::from_center_extent(
            Vector3::new(2.0, 2.0, 3.0),
            Vector3::new(0.5, 1.0, 1.5)
        );

        assert_eq!(box1, box2);
        assert_ne!(box1, box3);
    }

    #[test]
    fn test_init_from_box_points() {
        // Create 8 points forming a box
        let points = [
            Vector3::new(0.0, 0.0, 0.0), // origin
            Vector3::new(2.0, 0.0, 0.0), // +x
            Vector3::new(0.0, 4.0, 0.0), // +y
            Vector3::new(2.0, 4.0, 0.0), // +x+y
            Vector3::new(0.0, 0.0, 6.0), // +z
            Vector3::new(2.0, 0.0, 6.0), // +x+z
            Vector3::new(0.0, 4.0, 6.0), // +y+z
            Vector3::new(2.0, 4.0, 6.0), // +x+y+z
        ];

        let mut obbox = OBBox::new();
        obbox.init_from_box_points(&points);

        // Check that center is correct
        let expected_center = Vector3::new(1.0, 2.0, 3.0);
        assert!((obbox.center - expected_center).length() < 1e-6);

        // Check that extents are reasonable (should be half of dimensions)
        assert!(obbox.extent.x > 0.9 && obbox.extent.x < 1.1);
        assert!(obbox.extent.y > 1.9 && obbox.extent.y < 2.1);
        assert!(obbox.extent.z > 2.9 && obbox.extent.z < 3.1);
    }
}
