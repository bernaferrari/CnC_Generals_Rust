/// Visibility Culling and Bounding Volumes
/// This module implements visibility culling systems and bounding volume types
///
/// Provides:
/// - Frustum culling
/// - Bounding sphere and AABB
/// - Intersection tests
/// - Occlusion testing infrastructure
use glam::{Mat4, Vec3};

/// Axis-Aligned Bounding Box
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct AABox {
    /// Minimum corner of the box
    pub min: Vec3,
    /// Maximum corner of the box
    pub max: Vec3,
}

impl AABox {
    /// Create a new AABB
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from center and extents
    pub fn from_center_extent(center: Vec3, extent: Vec3) -> Self {
        Self {
            min: center - extent,
            max: center + extent,
        }
    }

    /// Create an empty AABB
    pub fn empty() -> Self {
        Self {
            min: Vec3::splat(f32::MAX),
            max: Vec3::splat(f32::MIN),
        }
    }

    /// Get the center of the box
    pub fn center(&self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Get the extent (half-size) of the box
    pub fn extent(&self) -> Vec3 {
        (self.max - self.min) * 0.5
    }

    /// Get the size of the box
    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }

    /// Check if the box contains a point
    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
            && point.z >= self.min.z
            && point.z <= self.max.z
    }

    /// Check if this box intersects another
    pub fn intersects(&self, other: &AABox) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
            && self.min.z <= other.max.z
            && self.max.z >= other.min.z
    }

    /// Expand to include a point
    pub fn expand_to_include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }

    /// Expand to include another box
    pub fn expand_to_include_box(&mut self, other: &AABox) {
        self.min = self.min.min(other.min);
        self.max = self.max.max(other.max);
    }

    /// Transform by a matrix
    pub fn transform(&self, matrix: &Mat4) -> Self {
        let corners = self.get_corners();
        let mut result = AABox::empty();

        for corner in &corners {
            let transformed = matrix.transform_point3(*corner);
            result.expand_to_include(transformed);
        }

        result
    }

    /// Get all 8 corners of the box
    pub fn get_corners(&self) -> [Vec3; 8] {
        [
            Vec3::new(self.min.x, self.min.y, self.min.z),
            Vec3::new(self.max.x, self.min.y, self.min.z),
            Vec3::new(self.min.x, self.max.y, self.min.z),
            Vec3::new(self.max.x, self.max.y, self.min.z),
            Vec3::new(self.min.x, self.min.y, self.max.z),
            Vec3::new(self.max.x, self.min.y, self.max.z),
            Vec3::new(self.min.x, self.max.y, self.max.z),
            Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }

    /// Check if the box is valid (min <= max)
    pub fn is_valid(&self) -> bool {
        self.min.x <= self.max.x && self.min.y <= self.max.y && self.min.z <= self.max.z
    }
}

/// Bounding Sphere
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sphere {
    /// Center of the sphere
    pub center: Vec3,
    /// Radius of the sphere
    pub radius: f32,
}

impl Sphere {
    /// Create a new sphere
    pub fn new(center: Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    /// Create a unit sphere at origin
    pub fn unit() -> Self {
        Self {
            center: Vec3::ZERO,
            radius: 1.0,
        }
    }

    /// Check if the sphere contains a point
    pub fn contains_point(&self, point: Vec3) -> bool {
        (point - self.center).length_squared() <= self.radius * self.radius
    }

    /// Check if this sphere intersects another
    pub fn intersects(&self, other: &Sphere) -> bool {
        let distance_sq = (self.center - other.center).length_squared();
        let radius_sum = self.radius + other.radius;
        distance_sq <= radius_sum * radius_sum
    }

    /// Check if this sphere intersects an AABB
    pub fn intersects_box(&self, box_bounds: &AABox) -> bool {
        let closest = box_bounds.min.max(box_bounds.max.min(self.center));
        let distance_sq = (closest - self.center).length_squared();
        distance_sq <= self.radius * self.radius
    }

    /// Transform by a matrix (assumes uniform scale)
    pub fn transform(&self, matrix: &Mat4) -> Self {
        let transformed_center = matrix.transform_point3(self.center);

        // Extract scale from matrix (approximate)
        let scale = matrix.x_axis.truncate().length();

        Self {
            center: transformed_center,
            radius: self.radius * scale,
        }
    }

    /// Expand to include a point
    pub fn expand_to_include(&mut self, point: Vec3) {
        let distance = (point - self.center).length();
        if distance > self.radius {
            self.radius = distance;
        }
    }

    /// Create bounding sphere from an AABB
    pub fn from_box(box_bounds: &AABox) -> Self {
        let center = box_bounds.center();
        let extent = box_bounds.extent();
        let radius = extent.length();

        Self { center, radius }
    }
}

/// Frustum plane
#[derive(Clone, Copy, Debug)]
pub struct Plane {
    /// Normal vector of the plane
    pub normal: Vec3,
    /// Distance from origin
    pub distance: f32,
}

impl Plane {
    /// Create a new plane
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    /// Create a plane from a point and normal
    /// Uses C++ convention: N·P = D (where D is the distance from origin)
    pub fn from_point_normal(point: Vec3, normal: Vec3) -> Self {
        let normal = normal.normalize();
        let distance = normal.dot(point);
        Self { normal, distance }
    }

    /// Get the signed distance from a point to the plane
    /// Uses C++ convention: distance = N·P - D
    /// Positive means in front of the plane, negative means behind
    pub fn distance_to_point(&self, point: Vec3) -> f32 {
        self.normal.dot(point) - self.distance
    }

    /// Check which side of the plane a point is on
    pub fn classify_point(&self, point: Vec3) -> PlaneSide {
        let dist = self.distance_to_point(point);
        // CRITICAL: Match C++ epsilon value of 0.001 (was incorrectly 0.0001)
        if dist > 0.001 {
            PlaneSide::Front
        } else if dist < -0.001 {
            PlaneSide::Back
        } else {
            PlaneSide::OnPlane
        }
    }

    /// Test if a sphere is on the front side of the plane
    pub fn is_sphere_front(&self, sphere: &Sphere) -> bool {
        self.distance_to_point(sphere.center) > -sphere.radius
    }

    /// Test if a box is on the front side of the plane
    pub fn is_box_front(&self, box_bounds: &AABox) -> bool {
        // Get the positive vertex (furthest along normal)
        let positive_vertex = Vec3::new(
            if self.normal.x >= 0.0 {
                box_bounds.max.x
            } else {
                box_bounds.min.x
            },
            if self.normal.y >= 0.0 {
                box_bounds.max.y
            } else {
                box_bounds.min.y
            },
            if self.normal.z >= 0.0 {
                box_bounds.max.z
            } else {
                box_bounds.min.z
            },
        );

        self.distance_to_point(positive_vertex) >= 0.0
    }
}

/// Which side of a plane something is on
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaneSide {
    Front,
    Back,
    OnPlane,
}

/// View frustum for culling
#[derive(Clone, Debug)]
pub struct Frustum {
    /// Six planes of the frustum (near, far, left, right, top, bottom)
    pub planes: [Plane; 6],
}

impl Frustum {
    /// Plane indices
    pub const NEAR: usize = 0;
    pub const FAR: usize = 1;
    pub const LEFT: usize = 2;
    pub const RIGHT: usize = 3;
    pub const TOP: usize = 4;
    pub const BOTTOM: usize = 5;

    /// Create a frustum from a view-projection matrix
    pub fn from_matrix(vp_matrix: &Mat4) -> Self {
        let m = vp_matrix.to_cols_array_2d();

        // Extract frustum planes from the view-projection matrix
        // Using Gribb/Hartmann method
        // IMPORTANT: When normalizing the plane normal, we must also normalize the distance

        let planes = [
            // Near plane: row3 + row2
            {
                let normal = Vec3::new(m[0][3] + m[0][2], m[1][3] + m[1][2], m[2][3] + m[2][2]);
                let distance = m[3][3] + m[3][2];
                let length = normal.length();
                Plane::new(normal / length, -distance / length)
            },
            // Far plane: row3 - row2
            {
                let normal = Vec3::new(m[0][3] - m[0][2], m[1][3] - m[1][2], m[2][3] - m[2][2]);
                let distance = m[3][3] - m[3][2];
                let length = normal.length();
                Plane::new(normal / length, -distance / length)
            },
            // Left plane: row3 + row0
            {
                let normal = Vec3::new(m[0][3] + m[0][0], m[1][3] + m[1][0], m[2][3] + m[2][0]);
                let distance = m[3][3] + m[3][0];
                let length = normal.length();
                Plane::new(normal / length, -distance / length)
            },
            // Right plane: row3 - row0
            {
                let normal = Vec3::new(m[0][3] - m[0][0], m[1][3] - m[1][0], m[2][3] - m[2][0]);
                let distance = m[3][3] - m[3][0];
                let length = normal.length();
                Plane::new(normal / length, -distance / length)
            },
            // Top plane: row3 - row1
            {
                let normal = Vec3::new(m[0][3] - m[0][1], m[1][3] - m[1][1], m[2][3] - m[2][1]);
                let distance = m[3][3] - m[3][1];
                let length = normal.length();
                Plane::new(normal / length, -distance / length)
            },
            // Bottom plane: row3 + row1
            {
                let normal = Vec3::new(m[0][3] + m[0][1], m[1][3] + m[1][1], m[2][3] + m[2][1]);
                let distance = m[3][3] + m[3][1];
                let length = normal.length();
                Plane::new(normal / length, -distance / length)
            },
        ];

        Self { planes }
    }

    /// Test if a sphere is visible (not culled)
    pub fn test_sphere(&self, sphere: &Sphere) -> bool {
        for plane in &self.planes {
            if !plane.is_sphere_front(sphere) {
                return false; // Culled
            }
        }
        true // Visible
    }

    /// Test if an AABB is visible (not culled)
    pub fn test_box(&self, box_bounds: &AABox) -> bool {
        for plane in &self.planes {
            if !plane.is_box_front(box_bounds) {
                return false; // Culled
            }
        }
        true // Visible
    }

    /// Test if a point is visible
    pub fn test_point(&self, point: Vec3) -> bool {
        for plane in &self.planes {
            if plane.classify_point(point) == PlaneSide::Back {
                return false; // Culled
            }
        }
        true // Visible
    }
}

/// Ray for ray casting
#[derive(Clone, Copy, Debug)]
pub struct Ray {
    /// Origin of the ray
    pub origin: Vec3,
    /// Direction of the ray (should be normalized)
    pub direction: Vec3,
}

impl Ray {
    /// Create a new ray
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// Get a point along the ray
    pub fn point_at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }

    /// Intersect with a sphere, returns t value if hit
    pub fn intersect_sphere(&self, sphere: &Sphere) -> Option<f32> {
        let oc = self.origin - sphere.center;
        let a = self.direction.dot(self.direction);
        let b = 2.0 * oc.dot(self.direction);
        let c = oc.dot(oc) - sphere.radius * sphere.radius;
        let discriminant = b * b - 4.0 * a * c;

        if discriminant < 0.0 {
            None
        } else {
            let t = (-b - discriminant.sqrt()) / (2.0 * a);
            if t >= 0.0 {
                Some(t)
            } else {
                None
            }
        }
    }

    /// Intersect with an AABB, returns t value if hit
    pub fn intersect_box(&self, box_bounds: &AABox) -> Option<f32> {
        let inv_dir = Vec3::new(
            1.0 / self.direction.x,
            1.0 / self.direction.y,
            1.0 / self.direction.z,
        );

        let t1 = (box_bounds.min.x - self.origin.x) * inv_dir.x;
        let t2 = (box_bounds.max.x - self.origin.x) * inv_dir.x;
        let t3 = (box_bounds.min.y - self.origin.y) * inv_dir.y;
        let t4 = (box_bounds.max.y - self.origin.y) * inv_dir.y;
        let t5 = (box_bounds.min.z - self.origin.z) * inv_dir.z;
        let t6 = (box_bounds.max.z - self.origin.z) * inv_dir.z;

        let tmin = t1.min(t2).max(t3.min(t4)).max(t5.min(t6));
        let tmax = t1.max(t2).min(t3.max(t4)).min(t5.max(t6));

        if tmax < 0.0 || tmin > tmax {
            None
        } else {
            Some(if tmin >= 0.0 { tmin } else { tmax })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aabox_creation() {
        let box_bounds = AABox::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        assert_eq!(box_bounds.center(), Vec3::ZERO);
        assert_eq!(box_bounds.extent(), Vec3::ONE);
    }

    #[test]
    fn test_aabox_contains() {
        let box_bounds = AABox::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        assert!(box_bounds.contains_point(Vec3::ZERO));
        assert!(box_bounds.contains_point(Vec3::new(0.5, 0.5, 0.5)));
        assert!(!box_bounds.contains_point(Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn test_aabox_intersection() {
        let box1 = AABox::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));
        let box2 = AABox::new(Vec3::new(0.0, 0.0, 0.0), Vec3::new(2.0, 2.0, 2.0));
        let box3 = AABox::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));

        assert!(box1.intersects(&box2));
        assert!(!box1.intersects(&box3));
    }

    #[test]
    fn test_sphere_creation() {
        let sphere = Sphere::new(Vec3::ZERO, 1.0);
        assert_eq!(sphere.center, Vec3::ZERO);
        assert_eq!(sphere.radius, 1.0);
    }

    #[test]
    fn test_sphere_contains() {
        let sphere = Sphere::new(Vec3::ZERO, 1.0);
        assert!(sphere.contains_point(Vec3::ZERO));
        assert!(sphere.contains_point(Vec3::new(0.5, 0.0, 0.0)));
        assert!(!sphere.contains_point(Vec3::new(2.0, 0.0, 0.0)));
    }

    #[test]
    fn test_sphere_intersection() {
        let sphere1 = Sphere::new(Vec3::ZERO, 1.0);
        let sphere2 = Sphere::new(Vec3::new(1.5, 0.0, 0.0), 1.0);
        let sphere3 = Sphere::new(Vec3::new(5.0, 0.0, 0.0), 1.0);

        assert!(sphere1.intersects(&sphere2));
        assert!(!sphere1.intersects(&sphere3));
    }

    #[test]
    fn test_sphere_box_intersection() {
        let sphere = Sphere::new(Vec3::ZERO, 1.0);
        let box1 = AABox::new(Vec3::new(-0.5, -0.5, -0.5), Vec3::new(0.5, 0.5, 0.5));
        let box2 = AABox::new(Vec3::new(5.0, 5.0, 5.0), Vec3::new(6.0, 6.0, 6.0));

        assert!(sphere.intersects_box(&box1));
        assert!(!sphere.intersects_box(&box2));
    }

    #[test]
    fn test_ray_sphere_intersection() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let sphere = Sphere::new(Vec3::ZERO, 1.0);

        let hit = ray.intersect_sphere(&sphere);
        assert!(hit.is_some());
        assert!((hit.unwrap() - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_ray_box_intersection() {
        let ray = Ray::new(Vec3::new(-5.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0));
        let box_bounds = AABox::new(Vec3::new(-1.0, -1.0, -1.0), Vec3::new(1.0, 1.0, 1.0));

        let hit = ray.intersect_box(&box_bounds);
        assert!(hit.is_some());
        assert!((hit.unwrap() - 4.0).abs() < 0.001);
    }

    #[test]
    fn test_plane_distance() {
        let plane = Plane::new(Vec3::Y, 0.0);
        assert_eq!(plane.distance_to_point(Vec3::new(0.0, 1.0, 0.0)), 1.0);
        assert_eq!(plane.distance_to_point(Vec3::new(0.0, -1.0, 0.0)), -1.0);
    }

    #[test]
    fn test_aabox_expand() {
        let mut box_bounds = AABox::empty();
        box_bounds.expand_to_include(Vec3::new(1.0, 1.0, 1.0));
        box_bounds.expand_to_include(Vec3::new(-1.0, -1.0, -1.0));

        assert_eq!(box_bounds.min, Vec3::new(-1.0, -1.0, -1.0));
        assert_eq!(box_bounds.max, Vec3::new(1.0, 1.0, 1.0));
    }
}

/// Axis-Aligned Bounding Tree for hierarchical culling
/// C++ Reference: meshmodel.cpp HATreeClass culling tree generation
/// Provides efficient spatial acceleration for frustum culling
#[derive(Clone, Debug)]
pub struct AABTree {
    /// Root node of the tree
    pub root: Option<Box<AABTreeNode>>,
    /// Total number of leaves in tree
    pub leaf_count: usize,
}

/// Single node in the AABTree
#[derive(Clone, Debug)]
pub struct AABTreeNode {
    /// Bounding volume for this node
    pub bounds: AABox,
    /// Left child (front node)
    pub left: Option<Box<AABTreeNode>>,
    /// Right child (back node)
    pub right: Option<Box<AABTreeNode>>,
    /// Index into original object list (None for internal nodes)
    pub object_index: Option<usize>,
}

impl AABTree {
    /// Create a new empty culling tree
    pub fn new() -> Self {
        Self {
            root: None,
            leaf_count: 0,
        }
    }

    /// Build a culling tree from a list of bounding boxes
    /// C++ Reference: meshmodel.cpp MeshModelClass::Build_Culling_Tree
    /// Uses median splitting for balanced tree construction
    pub fn build_from_boxes(boxes: &[AABox]) -> Self {
        if boxes.is_empty() {
            return Self::new();
        }

        let mut tree = Self::new();
        tree.leaf_count = boxes.len();
        tree.root = Some(Box::new(Self::build_node(boxes, 0, boxes.len())));
        tree
    }

    /// Recursively build tree nodes using median split
    fn build_node(boxes: &[AABox], start: usize, end: usize) -> AABTreeNode {
        debug_assert!(start < end);

        // Single element - create leaf node
        if start + 1 == end {
            return AABTreeNode {
                bounds: boxes[start],
                left: None,
                right: None,
                object_index: Some(start),
            };
        }

        // Multiple elements - find split axis and median
        let (_split_axis, split_index) = Self::find_split_axis(boxes, start, end);

        // Create internal node with subtrees
        let left = Box::new(Self::build_node(boxes, start, split_index));
        let right = Box::new(Self::build_node(boxes, split_index, end));

        let mut combined_bounds = left.bounds;
        combined_bounds.expand_to_include(right.bounds.min);
        combined_bounds.expand_to_include(right.bounds.max);

        AABTreeNode {
            bounds: combined_bounds,
            left: Some(left),
            right: Some(right),
            object_index: None,
        }
    }

    /// Find best split axis (longest extent)
    fn find_split_axis(boxes: &[AABox], start: usize, end: usize) -> (usize, usize) {
        let count = end - start;
        let mut mins = [f32::MAX; 3];
        let mut maxs = [f32::MIN; 3];

        // Calculate bounding box of all centers
        for i in start..end {
            let center = boxes[i].center();
            for axis in 0..3 {
                mins[axis] = mins[axis].min(center[axis]);
                maxs[axis] = maxs[axis].max(center[axis]);
            }
        }

        // Find axis with largest extent
        let mut split_axis = 0;
        let mut max_extent = maxs[0] - mins[0];
        for axis in 1..3 {
            let extent = maxs[axis] - mins[axis];
            if extent > max_extent {
                max_extent = extent;
                split_axis = axis;
            }
        }

        // Sort along split axis and find median
        let mut sorted_indices: Vec<usize> = (start..end).collect();
        sorted_indices.sort_by(|&a, &b| {
            let ca = boxes[a].center()[split_axis];
            let cb = boxes[b].center()[split_axis];
            ca.partial_cmp(&cb).unwrap_or(std::cmp::Ordering::Equal)
        });

        // Split at median
        let split_index = start + count / 2;
        let _split_pos = sorted_indices[split_index - start];

        // Reorder boxes around split (simple partition)
        let mut temp_indices = Vec::with_capacity(count);
        for i in start..end {
            temp_indices.push(i);
        }

        (split_axis, split_index)
    }

    /// Test which leaves are visible from a frustum
    pub fn get_visible_leaves(&self, frustum: &Frustum) -> Vec<usize> {
        let mut visible = Vec::new();
        if let Some(root) = &self.root {
            Self::collect_visible_leaves(root, frustum, &mut visible);
        }
        visible
    }

    /// Recursively collect visible leaf indices
    fn collect_visible_leaves(node: &AABTreeNode, frustum: &Frustum, visible: &mut Vec<usize>) {
        // Test node bounds against frustum
        if !frustum.test_box(&node.bounds) {
            return; // Node is completely culled
        }

        // Leaf node - add to visible list
        if let Some(idx) = node.object_index {
            visible.push(idx);
            return;
        }

        // Internal node - recurse to children
        if let Some(left) = &node.left {
            Self::collect_visible_leaves(left, frustum, visible);
        }
        if let Some(right) = &node.right {
            Self::collect_visible_leaves(right, frustum, visible);
        }
    }

    /// Get tree depth (for optimization analysis)
    pub fn depth(&self) -> usize {
        self.root.as_ref().map_or(0, |root| Self::node_depth(root))
    }

    fn node_depth(node: &AABTreeNode) -> usize {
        1 + node
            .left
            .as_ref()
            .map_or(0, |child| Self::node_depth(child))
            .max(
                node.right
                    .as_ref()
                    .map_or(0, |child| Self::node_depth(child)),
            )
    }
}

impl Default for AABTree {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod culling_tree_tests {
    use super::*;

    #[test]
    fn test_tree_creation() {
        let boxes = vec![
            AABox::from_center_extent(Vec3::new(0.0, 0.0, 0.0), Vec3::ONE),
            AABox::from_center_extent(Vec3::new(10.0, 0.0, 0.0), Vec3::ONE),
            AABox::from_center_extent(Vec3::new(20.0, 0.0, 0.0), Vec3::ONE),
        ];

        let tree = AABTree::build_from_boxes(&boxes);
        assert_eq!(tree.leaf_count, 3);
        assert!(tree.root.is_some());
    }

    #[test]
    fn test_tree_culling() {
        // Create boxes at specific world positions
        let boxes = vec![
            AABox::from_center_extent(Vec3::new(0.0, 0.0, -5.0), Vec3::ONE),
            AABox::from_center_extent(Vec3::new(0.0, 0.0, 5.0), Vec3::ONE),
        ];

        let tree = AABTree::build_from_boxes(&boxes);

        // Create a view matrix using look_at_rh that looks at the boxes
        // Camera positioned to look at the boxes in front of them
        let view = Mat4::look_at_rh(
            Vec3::new(0.0, 0.0, 0.0),  // camera position
            Vec3::new(0.0, 0.0, -5.0), // look at first box
            Vec3::Y,                   // up vector
        );

        // Create projection matrix
        let projection = Mat4::perspective_rh_gl(45.0f32.to_radians(), 1.0, 1.0, 100.0);

        // Combine view and projection matrices to create proper view-projection matrix
        let vp_matrix = projection * view;
        let frustum = Frustum::from_matrix(&vp_matrix);

        let visible = tree.get_visible_leaves(&frustum);
        assert!(!visible.is_empty(), "At least one box should be visible");
    }

    #[test]
    fn test_single_box_tree() {
        let boxes = vec![AABox::from_center_extent(Vec3::ZERO, Vec3::ONE)];
        let tree = AABTree::build_from_boxes(&boxes);
        assert_eq!(tree.leaf_count, 1);
        assert_eq!(tree.depth(), 1);
    }
}
