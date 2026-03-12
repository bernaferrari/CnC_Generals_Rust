//! AABTree (Axis-Aligned Bounding Box Tree) Implementation
//!
//! This module provides spatial partitioning functionality for efficient
//! collision detection and culling, matching the C++ WW3D AABTree system.

use crate::*;
// use crate::mesh_geometry::MeshTriangle;

/// AABTree node structure for spatial partitioning
#[derive(Debug, Clone)]
pub struct AABTreeNode {
    /// Minimum bounds of this node
    pub min: Vec3,
    /// Maximum bounds of this node
    pub max: Vec3,
    /// Front child index or polygon start index (if leaf)
    pub front_or_poly0: u32,
    /// Back child index or polygon count (if leaf)
    pub back_or_poly_count: u32,
}

impl Default for AABTreeNode {
    fn default() -> Self {
        Self {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            front_or_poly0: 0,
            back_or_poly_count: 0,
        }
    }
}

impl AABTreeNode {
    /// Check if this node is a leaf (contains polygons)
    pub fn is_leaf(&self) -> bool {
        (self.front_or_poly0 & AABTREE_LEAF_FLAG) != 0
    }

    /// Get front child index (for non-leaf nodes)
    pub fn get_front_child(&self) -> u32 {
        self.front_or_poly0 & !AABTREE_LEAF_FLAG
    }

    /// Get back child index (for non-leaf nodes)
    pub fn get_back_child(&self) -> u32 {
        self.back_or_poly_count
    }

    /// Get polygon start index (for leaf nodes)
    pub fn get_poly0(&self) -> u32 {
        self.front_or_poly0 & !AABTREE_LEAF_FLAG
    }

    /// Get polygon count (for leaf nodes)
    pub fn get_poly_count(&self) -> u32 {
        self.back_or_poly_count
    }

    /// Set front child index
    pub fn set_front_child(&mut self, index: u32) {
        self.front_or_poly0 = index & !AABTREE_LEAF_FLAG;
    }

    /// Set back child index
    pub fn set_back_child(&mut self, index: u32) {
        self.back_or_poly_count = index;
    }

    /// Set polygon start index (marks as leaf)
    pub fn set_poly0(&mut self, index: u32) {
        self.front_or_poly0 = (index & !AABTREE_LEAF_FLAG) | AABTREE_LEAF_FLAG;
    }

    /// Set polygon count
    pub fn set_poly_count(&mut self, count: u32) {
        self.back_or_poly_count = count;
    }
}

/// AABTree for spatial partitioning and collision detection
#[derive(Debug, Clone)]
pub struct AABTree {
    /// Array of tree nodes
    pub nodes: Vec<AABTreeNode>,
    /// Array of polygon indices referenced by leaf nodes
    pub poly_indices: Vec<u32>,
    /// Total node count
    pub node_count: usize,
    /// Total polygon count
    pub poly_count: usize,
}

impl AABTree {
    /// Leaf flag constant
    pub const LEAF_FLAG: u32 = 0x80000000;

    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            poly_indices: Vec::new(),
            node_count: 0,
            poly_count: 0,
        }
    }

    /// Build AABTree from mesh geometry
    pub fn build_from_mesh(&mut self, mesh: &MeshGeometry) {
        let mut builder = AABTreeBuilder::new();
        builder.build_aabtree(mesh);
        *self = builder.export();
    }

    /// Scale the AABTree
    pub fn scale(&mut self, scale: f32) {
        for node in &mut self.nodes {
            node.min *= scale;
            node.max *= scale;
        }
    }

    /// Get node count
    pub fn get_node_count(&self) -> usize {
        self.node_count
    }

    /// Get polygon count
    pub fn get_poly_count(&self) -> usize {
        self.poly_count
    }

    /// Generate Affected Polygon Table (APT) for an oriented bounding box
    pub fn generate_apt(&self, obbox: &OBBox, apt: &mut Vec<u32>) {
        apt.clear();
        if self.nodes.is_empty() {
            return;
        }

        let mut context = OBBoxAPTContext {
            obbox: obbox.clone(),
            apt,
        };

        self.generate_obbox_apt_recursive(0, &mut context);
    }

    /// Generate APT with view direction culling
    pub fn generate_apt_with_view(
        &self,
        obbox: &OBBox,
        view_dir: Vec3,
        mesh: &MeshGeometry,
        apt: &mut Vec<u32>,
    ) {
        apt.clear();
        if self.nodes.is_empty() {
            return;
        }

        let mut context = OBBoxRayAPTContext {
            obbox: obbox.clone(),
            view_vector: view_dir,
            apt,
            mesh,
        };

        self.generate_obbox_ray_apt_recursive(0, &mut context);
    }

    /// Cast ray against the AABTree
    pub fn cast_ray(&self, ray_test: &mut RayCollisionTest) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.cast_ray_recursive(0, ray_test)
    }

    /// Cast AABox against the AABTree
    pub fn cast_aabox(&self, box_test: &mut AABoxCollisionTest) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.cast_aabox_recursive(0, box_test)
    }

    /// Cast OBBox against the AABTree
    pub fn cast_obbox(&self, box_test: &mut OBBoxCollisionTest) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.cast_obbox_recursive(0, box_test)
    }

    /// Test intersection with OBBox
    pub fn intersect_obbox(&self, box_test: &mut OBBoxIntersectionTest) -> bool {
        if self.nodes.is_empty() {
            return false;
        }
        self.intersect_obbox_recursive(0, box_test)
    }

    // Recursive helper methods
    fn generate_obbox_apt_recursive(&self, node_index: usize, context: &mut OBBoxAPTContext) {
        if node_index >= self.nodes.len() {
            return;
        }

        let node = &self.nodes[node_index];

        // Test if this node's AABB intersects with the OBBox
        if !self.node_intersects_obbox(node, &context.obbox) {
            return;
        }

        if node.is_leaf() {
            // Add all polygons in this leaf to APT
            let poly0 = node.get_poly0() as usize;
            let poly_count = node.get_poly_count() as usize;

            for i in 0..poly_count {
                if poly0 + i < self.poly_indices.len() {
                    context.apt.push(self.poly_indices[poly0 + i]);
                }
            }
        } else {
            // Recurse to children
            let front_child = node.get_front_child() as usize;
            let back_child = node.get_back_child() as usize;

            self.generate_obbox_apt_recursive(front_child, context);
            self.generate_obbox_apt_recursive(back_child, context);
        }
    }

    fn generate_obbox_ray_apt_recursive(
        &self,
        node_index: usize,
        context: &mut OBBoxRayAPTContext,
    ) {
        if node_index >= self.nodes.len() {
            return;
        }

        let node = &self.nodes[node_index];

        // Test if this node's AABB intersects with the OBBox
        if !self.node_intersects_obbox(node, &context.obbox) {
            return;
        }

        if node.is_leaf() {
            // Add polygons that are front-facing to the view direction
            let poly0 = node.get_poly0() as usize;
            let poly_count = node.get_poly_count() as usize;

            for i in 0..poly_count {
                if poly0 + i < self.poly_indices.len() {
                    let poly_index = self.poly_indices[poly0 + i] as usize;
                    // Implement backface culling based on view direction
                    if let Some(triangle) = context.mesh.triangles.get(poly_index) {
                        let v0 = context.mesh.vertices[triangle.indices[0] as usize].position;
                        let v1 = context.mesh.vertices[triangle.indices[1] as usize].position;
                        let v2 = context.mesh.vertices[triangle.indices[2] as usize].position;

                        // Compute triangle normal; skip degenerate tris to match C++ guard
                        let edge1 = v1 - v0;
                        let edge2 = v2 - v0;
                        let normal = edge1.cross(edge2);
                        let normal_len_sq = normal.length_squared();
                        if normal_len_sq <= EPSILON * EPSILON {
                            continue;
                        }

                        // Backface cull against the supplied view vector (same sign test as C++)
                        if normal.dot(context.view_vector) < 0.0 {
                            context.apt.push(self.poly_indices[poly0 + i]);
                        }
                    }
                }
            }
        } else {
            // Recurse to children
            let front_child = node.get_front_child() as usize;
            let back_child = node.get_back_child() as usize;

            self.generate_obbox_ray_apt_recursive(front_child, context);
            self.generate_obbox_ray_apt_recursive(back_child, context);
        }
    }

    fn cast_ray_recursive(&self, node_index: usize, ray_test: &mut RayCollisionTest) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }

        let node = &self.nodes[node_index];

        // Test ray against this node's AABB
        if !self.ray_intersects_aabox(ray_test, node) {
            return false;
        }

        if node.is_leaf() {
            // Test ray against individual polygons in this leaf
            let poly0 = node.get_poly0() as usize;
            let poly_count = node.get_poly_count() as usize;
            let mut hit = false;

            for i in 0..poly_count {
                if poly0 + i >= self.poly_indices.len() {
                    continue;
                }

                let poly_index = self.poly_indices[poly0 + i] as usize;

                if let Some(triangle) = ray_test.mesh.triangles.get(poly_index) {
                    let v0 = ray_test.mesh.vertices[triangle.indices[0] as usize].position;
                    let v1 = ray_test.mesh.vertices[triangle.indices[1] as usize].position;
                    let v2 = ray_test.mesh.vertices[triangle.indices[2] as usize].position;
                    let ray = ray_test.ray;

                    if self.ray_triangle_intersection(&ray, v0, v1, v2, ray_test) {
                        ray_test.hit_polygon_index = Some(poly_index);
                        hit = true;
                    }
                }
            }

            hit
        } else {
            // Test children; process both to ensure closest hit is found
            let front_child = node.get_front_child() as usize;
            let back_child = node.get_back_child() as usize;

            let mut hit = false;
            if front_child < self.nodes.len() {
                hit |= self.cast_ray_recursive(front_child, ray_test);
            }
            if back_child < self.nodes.len() {
                hit |= self.cast_ray_recursive(back_child, ray_test);
            }
            hit
        }
    }

    fn cast_aabox_recursive(&self, node_index: usize, box_test: &mut AABoxCollisionTest) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }

        let node = &self.nodes[node_index];

        // Test AABox against this node's AABB
        let node_aabox = AABox::new((node.min + node.max) / 2.0, (node.max - node.min) / 2.0);

        if !node_aabox.intersects_aabox(&box_test.aabox) {
            return false;
        }

        if node.is_leaf() {
            // Test AABox against individual polygons in this leaf
            let poly0 = node.get_poly0() as usize;
            let poly_count = node.get_poly_count() as usize;

            for i in 0..poly_count {
                if poly0 + i < self.poly_indices.len() {
                    let poly_index = self.poly_indices[poly0 + i] as usize;
                    // Get triangle vertices from mesh geometry
                    if let Some(triangle) = box_test.mesh.triangles.get(poly_index) {
                        let v0 = box_test.mesh.vertices[triangle.indices[0] as usize].position;
                        let v1 = box_test.mesh.vertices[triangle.indices[1] as usize].position;
                        let v2 = box_test.mesh.vertices[triangle.indices[2] as usize].position;

                        if self.aabox_triangle_intersection(&box_test.aabox, v0, v1, v2) {
                            box_test.hit_polygon_index = Some(poly_index);
                            return true;
                        }
                    }
                }
            }
            false
        } else {
            // Test children
            let front_child = node.get_front_child() as usize;
            let back_child = node.get_back_child() as usize;

            if self.cast_aabox_recursive(front_child, box_test) {
                return true;
            }
            if self.cast_aabox_recursive(back_child, box_test) {
                return true;
            }
            false
        }
    }

    fn cast_obbox_recursive(&self, node_index: usize, box_test: &mut OBBoxCollisionTest) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }

        let node = &self.nodes[node_index];

        // Test OBBox against this node's AABB
        if !self.node_intersects_obbox(node, &box_test.obbox) {
            return false;
        }

        if node.is_leaf() {
            // Test OBBox against individual polygons in this leaf
            let poly0 = node.get_poly0() as usize;
            let poly_count = node.get_poly_count() as usize;

            for i in 0..poly_count {
                if poly0 + i < self.poly_indices.len() {
                    let poly_index = self.poly_indices[poly0 + i] as usize;
                    // Get triangle vertices from mesh geometry
                    if let Some(triangle) = box_test.mesh.triangles.get(poly_index) {
                        let v0 = box_test.mesh.vertices[triangle.indices[0] as usize].position;
                        let v1 = box_test.mesh.vertices[triangle.indices[1] as usize].position;
                        let v2 = box_test.mesh.vertices[triangle.indices[2] as usize].position;

                        if self.obbox_triangle_intersection(&box_test.obbox, v0, v1, v2) {
                            box_test.hit_polygon_index = Some(poly_index);
                            return true;
                        }
                    }
                }
            }
            false
        } else {
            // Test children
            let front_child = node.get_front_child() as usize;
            let back_child = node.get_back_child() as usize;

            if self.cast_obbox_recursive(front_child, box_test) {
                return true;
            }
            if self.cast_obbox_recursive(back_child, box_test) {
                return true;
            }
            false
        }
    }

    fn intersect_obbox_recursive(
        &self,
        node_index: usize,
        box_test: &mut OBBoxIntersectionTest,
    ) -> bool {
        if node_index >= self.nodes.len() {
            return false;
        }

        let node = &self.nodes[node_index];

        // Test OBBox against this node's AABB
        if !self.node_intersects_obbox(node, &box_test.obbox) {
            return false;
        }

        if node.is_leaf() {
            // Test intersection with individual polygons in this leaf
            let poly0 = node.get_poly0() as usize;
            let poly_count = node.get_poly_count() as usize;

            for i in 0..poly_count {
                if poly0 + i < self.poly_indices.len() {
                    let poly_index = self.poly_indices[poly0 + i] as usize;
                    // Get triangle vertices from mesh geometry
                    if let Some(triangle) = box_test.mesh.triangles.get(poly_index) {
                        let v0 = box_test.mesh.vertices[triangle.indices[0] as usize].position;
                        let v1 = box_test.mesh.vertices[triangle.indices[1] as usize].position;
                        let v2 = box_test.mesh.vertices[triangle.indices[2] as usize].position;

                        if self.obbox_triangle_intersection(&box_test.obbox, v0, v1, v2) {
                            box_test.hit_polygon_index = Some(poly_index);
                            return true;
                        }
                    }
                }
            }
            false
        } else {
            // Test children
            let front_child = node.get_front_child() as usize;
            let back_child = node.get_back_child() as usize;

            if self.intersect_obbox_recursive(front_child, box_test) {
                return true;
            }
            if self.intersect_obbox_recursive(back_child, box_test) {
                return true;
            }
            false
        }
    }

    // Helper methods for intersection tests
    fn node_intersects_obbox(&self, node: &AABTreeNode, obbox: &OBBox) -> bool {
        // Convert node AABB to AABox for intersection test
        let node_aabox = AABox::new((node.min + node.max) / 2.0, (node.max - node.min) / 2.0);

        // Implement proper OBBox-AABox intersection using separating axis theorem
        self.obbox_aabox_intersection(obbox, &node_aabox)
    }

    fn ray_intersects_aabox(&self, ray_test: &RayCollisionTest, node: &AABTreeNode) -> bool {
        let node_aabox = AABox::new((node.min + node.max) / 2.0, (node.max - node.min) / 2.0);

        // Implement ray-AABox intersection using slab method
        let ray_origin = ray_test.ray.origin;
        let ray_dir = ray_test.ray.direction;

        // Check for ray direction components being zero (parallel to axis)
        let inv_dir = Vec3::new(
            if ray_dir.x.abs() < 1e-8 {
                f32::INFINITY
            } else {
                1.0 / ray_dir.x
            },
            if ray_dir.y.abs() < 1e-8 {
                f32::INFINITY
            } else {
                1.0 / ray_dir.y
            },
            if ray_dir.z.abs() < 1e-8 {
                f32::INFINITY
            } else {
                1.0 / ray_dir.z
            },
        );

        // Calculate t values for intersection with box planes
        let t1 = (node_aabox.min() - ray_origin) * inv_dir;
        let t2 = (node_aabox.max() - ray_origin) * inv_dir;

        // Get min and max t values for each axis
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        // Find the largest tmin and smallest tmax
        let tmin_max = tmin.x.max(tmin.y).max(tmin.z);
        let tmax_min = tmax.x.min(tmax.y).min(tmax.z);

        // Ray intersects AABB if tmin_max <= tmax_min and tmax_min >= 0
        tmax_min >= 0.0 && tmin_max <= tmax_min
    }

    /// Ray-triangle intersection using Möller-Trumbore algorithm
    fn ray_triangle_intersection(
        &self,
        ray: &Ray,
        v0: Vec3,
        v1: Vec3,
        v2: Vec3,
        ray_test: &mut RayCollisionTest,
    ) -> bool {
        const EPSILON: f32 = 0.0000001;

        let edge1 = v1 - v0;
        let edge2 = v2 - v0;
        let h = ray.direction.cross(edge2);
        let a = edge1.dot(h);

        if a > -EPSILON && a < EPSILON {
            return false; // Ray is parallel to triangle
        }

        let f = 1.0 / a;
        let s = ray.origin - v0;
        let u = f * s.dot(h);

        if u < 0.0 || u > 1.0 {
            return false;
        }

        let q = s.cross(edge1);
        let v = f * ray.direction.dot(q);

        if v < 0.0 || u + v > 1.0 {
            return false;
        }

        let t = f * edge2.dot(q);

        if t > EPSILON {
            // Check if this is closer than previous hits
            if ray_test.closest_distance < 0.0 || t < ray_test.closest_distance {
                ray_test.closest_distance = t;
                ray_test.hit_point = ray.origin + ray.direction * t;
                ray_test.hit_normal = edge1.cross(edge2).normalize();
                return true;
            }
        }

        false
    }

    /// AABox-triangle intersection test
    fn aabox_triangle_intersection(&self, aabox: &AABox, v0: Vec3, v1: Vec3, v2: Vec3) -> bool {
        // Transform triangle vertices to box local space
        let center = aabox.center;
        let extents = aabox.extent;

        let v0_local = v0 - center;
        let v1_local = v1 - center;
        let v2_local = v2 - center;

        // Test all 13 separating axes
        // 3 face normals of AABB
        for i in 0..3 {
            let mut min_val = v0_local[i];
            let mut max_val = v0_local[i];
            min_val = min_val.min(v1_local[i]).min(v2_local[i]);
            max_val = max_val.max(v1_local[i]).max(v2_local[i]);

            if max_val < -extents[i] || min_val > extents[i] {
                return false;
            }
        }

        // 1 triangle normal
        let tri_normal = (v1_local - v0_local).cross(v2_local - v0_local);
        let d = tri_normal.dot(v0_local);
        let r = extents.x * tri_normal.x.abs()
            + extents.y * tri_normal.y.abs()
            + extents.z * tri_normal.z.abs();
        if d.abs() > r {
            return false;
        }

        // 9 edge-edge cross products
        let edges = [
            v1_local - v0_local,
            v2_local - v1_local,
            v0_local - v2_local,
        ];
        let box_axes = [Vec3::X, Vec3::Y, Vec3::Z];

        for edge in &edges {
            for &box_axis in &box_axes {
                let axis = edge.cross(box_axis);
                if axis.length_squared() < 1e-6 {
                    continue; // Skip near-zero axes
                }

                let p0 = v0_local.dot(axis);
                let p1 = v1_local.dot(axis);
                let p2 = v2_local.dot(axis);

                let min_p = p0.min(p1).min(p2);
                let max_p = p0.max(p1).max(p2);

                let r = extents.x * (box_axis.cross(axis)).x.abs()
                    + extents.y * (box_axis.cross(axis)).y.abs()
                    + extents.z * (box_axis.cross(axis)).z.abs();

                if max_p < -r || min_p > r {
                    return false;
                }
            }
        }

        true
    }

    /// OBBox-triangle intersection test
    fn obbox_triangle_intersection(&self, obbox: &OBBox, v0: Vec3, v1: Vec3, v2: Vec3) -> bool {
        // Transform triangle vertices to OBBox local space
        let inv_transform = obbox.basis.inverse();
        let v0_local = inv_transform.transform_point3(v0 - obbox.center);
        let v1_local = inv_transform.transform_point3(v1 - obbox.center);
        let v2_local = inv_transform.transform_point3(v2 - obbox.center);

        // Now test as AABB vs triangle in local space
        let local_aabox = AABox::new(Vec3::ZERO, obbox.extent);
        self.aabox_triangle_intersection(&local_aabox, v0_local, v1_local, v2_local)
    }

    /// OBBox-AABox intersection test using separating axis theorem
    fn obbox_aabox_intersection(&self, obbox: &OBBox, aabox: &AABox) -> bool {
        // Convert AABB to OBBox for consistent testing
        let aabox_as_obbox = OBBox {
            center: aabox.center,
            extent: aabox.extent,
            basis: Mat4::IDENTITY,
        };

        self.obbox_obbox_intersection(obbox, &aabox_as_obbox)
    }

    /// OBBox-OBBox intersection test using separating axis theorem
    fn obbox_obbox_intersection(&self, obb1: &OBBox, obb2: &OBBox) -> bool {
        // Get the axes of both OBBoxes
        let axes1 = [
            obb1.basis.x_axis.truncate().normalize(),
            obb1.basis.y_axis.truncate().normalize(),
            obb1.basis.z_axis.truncate().normalize(),
        ];

        let axes2 = [
            obb2.basis.x_axis.truncate().normalize(),
            obb2.basis.y_axis.truncate().normalize(),
            obb2.basis.z_axis.truncate().normalize(),
        ];

        // Vector between centers
        let center_diff = obb2.center - obb1.center;

        // Test 15 potential separating axes
        let mut test_axes = Vec::with_capacity(15);

        // 6 face normals (3 from each OBBox)
        test_axes.extend_from_slice(&axes1);
        test_axes.extend_from_slice(&axes2);

        // 9 edge cross products
        for &axis1 in &axes1 {
            for &axis2 in &axes2 {
                let cross = axis1.cross(axis2);
                if cross.length_squared() > 1e-6 {
                    test_axes.push(cross.normalize());
                }
            }
        }

        // Test each axis
        for axis in test_axes {
            let projection1 = self.project_obbox_onto_axis(obb1, axis);
            let projection2 = self.project_obbox_onto_axis(obb2, axis);
            let center_projection = center_diff.dot(axis).abs();

            if center_projection > projection1 + projection2 {
                return false; // Separating axis found
            }
        }

        true // No separating axis found
    }

    /// Project OBBox onto an axis
    fn project_obbox_onto_axis(&self, obbox: &OBBox, axis: Vec3) -> f32 {
        let axes = [
            obbox.basis.x_axis.truncate().normalize(),
            obbox.basis.y_axis.truncate().normalize(),
            obbox.basis.z_axis.truncate().normalize(),
        ];

        obbox.extent.x * axes[0].dot(axis).abs()
            + obbox.extent.y * axes[1].dot(axis).abs()
            + obbox.extent.z * axes[2].dot(axis).abs()
    }
}

/// Context for OBBox APT generation
struct OBBoxAPTContext<'a> {
    obbox: OBBox,
    apt: &'a mut Vec<u32>,
}

/// Context for OBBox+Ray APT generation
struct OBBoxRayAPTContext<'a> {
    obbox: OBBox,
    view_vector: Vec3,
    apt: &'a mut Vec<u32>,
    mesh: &'a MeshGeometry,
}

#[derive(Clone, Debug)]
struct SplitChoice {
    axis: usize,
    dist: f32,
    cost: f32,
    front_count: usize,
    back_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaneSide {
    Front,
    Back,
    On,
    Both,
}

/// AABTreeBuilder for constructing AABTrees
#[derive(Debug)]
pub struct AABTreeBuilder {
    root: Option<Box<AABTreeBuilderNode>>,
    node_count: usize,
    poly_count: usize,
    rng_state: u64,
}

impl AABTreeBuilder {
    pub fn new() -> Self {
        Self {
            root: None,
            node_count: 0,
            poly_count: 0,
            rng_state: 0x1_2E_3D_4C_5B_6A_7980,
        }
    }

    /// Build AABTree from mesh geometry
    pub fn build_aabtree(&mut self, mesh: &MeshGeometry) {
        self.root = None;
        self.node_count = 0;
        self.poly_count = 0;
        self.rng_state = 0x1_2E_3D_4C_5B_6A_7980;

        if mesh.triangles.is_empty() {
            return;
        }

        // Collect all polygon indices
        let poly_indices: Vec<usize> = (0..mesh.triangles.len()).collect();

        // Build the tree recursively
        self.root = Some(self.build_tree_recursive(mesh, &poly_indices));
        self.node_count = self.count_nodes(&self.root);
        self.poly_count = poly_indices.len();
    }

    /// Export to AABTree
    pub fn export(mut self) -> AABTree {
        let mut aabtree = AABTree::new();

        if let Some(root) = self.root.take() {
            self.flatten_tree(&root, &mut aabtree.nodes, &mut aabtree.poly_indices);
        }

        aabtree.node_count = aabtree.nodes.len();
        aabtree.poly_count = aabtree.poly_indices.len();
        aabtree
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.node_count
    }

    /// Get polygon count
    pub fn poly_count(&self) -> usize {
        self.poly_count
    }

    // Recursive tree building
    fn build_tree_recursive(
        &mut self,
        mesh: &MeshGeometry,
        poly_indices: &[usize],
    ) -> Box<AABTreeBuilderNode> {
        let mut node = Box::new(AABTreeBuilderNode::new());

        // Compute bounding box for this set of polygons
        node.compute_bounds(mesh, poly_indices);

        // Check if this should be a leaf node
        if poly_indices.len() <= MIN_POLYS_PER_NODE {
            node.poly_indices = poly_indices.to_vec();
            return node;
        }

        if let Some(split_choice) = self.select_splitting_plane(mesh, poly_indices) {
            if split_choice.front_count + split_choice.back_count == poly_indices.len()
                && split_choice.front_count > 0
                && split_choice.back_count > 0
            {
                let (front_polys, back_polys) =
                    self.split_polygons(mesh, poly_indices, &split_choice);

                if !front_polys.is_empty() && !back_polys.is_empty() {
                    node.front = Some(self.build_tree_recursive(mesh, &front_polys));
                    node.back = Some(self.build_tree_recursive(mesh, &back_polys));
                    return node;
                }
            }
        }

        node.poly_indices = poly_indices.to_vec();
        node
    }

    /// Choose a partitioning plane using the original WW3D heuristic.
    fn select_splitting_plane(
        &mut self,
        mesh: &MeshGeometry,
        poly_indices: &[usize],
    ) -> Option<SplitChoice> {
        if poly_indices.is_empty() {
            return None;
        }

        let tries = poly_indices.len().min(NUM_PLANE_TRIES);
        let mut best: Option<SplitChoice> = None;

        for _ in 0..tries {
            let candidate_poly = poly_indices[(self.next_random() as usize) % poly_indices.len()];
            let vertex_choice = (self.next_random() % 3) as usize;
            let axis = (self.next_random() % 3) as usize;

            if let Some(triangle) = mesh.triangles.get(candidate_poly) {
                let vertex_index = triangle.indices[vertex_choice] as usize;
                if let Some(vertex) = mesh.vertices.get(vertex_index) {
                    let dist = vertex.position[axis];
                    if let Some(choice) = self.compute_split_choice(mesh, poly_indices, axis, dist)
                    {
                        if best
                            .as_ref()
                            .map_or(true, |current| choice.cost < current.cost)
                        {
                            best = Some(choice);
                        }
                    }
                }
            }
        }

        if best.is_none() {
            // Deterministic fallback: sample the midpoint along each axis
            for axis in 0..3 {
                if let Some((axis_min, axis_max)) = self.axis_range(mesh, poly_indices, axis) {
                    let dist = 0.5 * (axis_min + axis_max);
                    if let Some(choice) = self.compute_split_choice(mesh, poly_indices, axis, dist)
                    {
                        if best
                            .as_ref()
                            .map_or(true, |current| choice.cost < current.cost)
                        {
                            best = Some(choice);
                        }
                    }
                }
            }
        }

        best
    }

    fn compute_split_choice(
        &self,
        mesh: &MeshGeometry,
        poly_indices: &[usize],
        axis: usize,
        dist: f32,
    ) -> Option<SplitChoice> {
        let mut front_count = 0usize;
        let mut back_count = 0usize;
        let mut front_min = Vec3::splat(f32::INFINITY);
        let mut front_max = Vec3::splat(f32::NEG_INFINITY);
        let mut back_min = Vec3::splat(f32::INFINITY);
        let mut back_max = Vec3::splat(f32::NEG_INFINITY);

        for &poly_idx in poly_indices {
            if poly_idx >= mesh.triangles.len() {
                return None;
            }

            match self.which_side(mesh, poly_idx, axis, dist) {
                PlaneSide::Front | PlaneSide::On | PlaneSide::Both => {
                    if !self.update_bounds_for_poly(mesh, poly_idx, &mut front_min, &mut front_max)
                    {
                        return None;
                    }
                    front_count += 1;
                }
                PlaneSide::Back => {
                    if !self.update_bounds_for_poly(mesh, poly_idx, &mut back_min, &mut back_max) {
                        return None;
                    }
                    back_count += 1;
                }
            }
        }

        if front_count == 0 || back_count == 0 {
            return None;
        }

        front_min -= Vec3::splat(WWMATH_EPSILON);
        front_max += Vec3::splat(WWMATH_EPSILON);
        back_min -= Vec3::splat(WWMATH_EPSILON);
        back_max += Vec3::splat(WWMATH_EPSILON);

        let front_extent = front_max - front_min;
        let back_extent = back_max - back_min;

        let front_volume = (front_extent.x.abs()) * (front_extent.y.abs()) * (front_extent.z.abs());
        let back_volume = (back_extent.x.abs()) * (back_extent.y.abs()) * (back_extent.z.abs());

        let cost = front_volume * front_count as f32 + back_volume * back_count as f32;

        Some(SplitChoice {
            axis,
            dist,
            cost,
            front_count,
            back_count,
        })
    }

    fn axis_range(
        &self,
        mesh: &MeshGeometry,
        poly_indices: &[usize],
        axis: usize,
    ) -> Option<(f32, f32)> {
        let mut axis_min = f32::INFINITY;
        let mut axis_max = f32::NEG_INFINITY;
        let mut found = false;

        for &poly_idx in poly_indices {
            if poly_idx >= mesh.triangles.len() {
                continue;
            }

            let triangle = &mesh.triangles[poly_idx];
            for &vertex_idx in &triangle.indices {
                if let Some(vertex) = mesh.vertices.get(vertex_idx as usize) {
                    let coord = vertex.position[axis];
                    axis_min = axis_min.min(coord);
                    axis_max = axis_max.max(coord);
                    found = true;
                }
            }
        }

        if found {
            Some((axis_min, axis_max))
        } else {
            None
        }
    }

    fn which_side(
        &self,
        mesh: &MeshGeometry,
        poly_idx: usize,
        axis: usize,
        dist: f32,
    ) -> PlaneSide {
        if let Some(triangle) = mesh.triangles.get(poly_idx) {
            let mut pos = false;
            let mut neg = false;

            for &vertex_idx in &triangle.indices {
                if let Some(vertex) = mesh.vertices.get(vertex_idx as usize) {
                    let delta = vertex.position[axis] - dist;
                    if delta > COINCIDENCE_EPSILON {
                        pos = true;
                    }
                    if delta < -COINCIDENCE_EPSILON {
                        neg = true;
                    }
                }
            }

            return match (pos, neg) {
                (false, false) => PlaneSide::On,
                (true, false) => PlaneSide::Front,
                (false, true) => PlaneSide::Back,
                (true, true) => PlaneSide::Both,
            };
        }

        PlaneSide::On
    }

    fn update_bounds_for_poly(
        &self,
        mesh: &MeshGeometry,
        poly_idx: usize,
        min: &mut Vec3,
        max: &mut Vec3,
    ) -> bool {
        let triangle = match mesh.triangles.get(poly_idx) {
            Some(tri) => tri,
            None => return false,
        };

        let mut updated = false;
        for &vertex_idx in &triangle.indices {
            let vertex = match mesh.vertices.get(vertex_idx as usize) {
                Some(v) => v,
                None => return false,
            };
            *min = (*min).min(vertex.position);
            *max = (*max).max(vertex.position);
            updated = true;
        }

        updated
    }

    fn split_polygons(
        &self,
        mesh: &MeshGeometry,
        poly_indices: &[usize],
        split: &SplitChoice,
    ) -> (Vec<usize>, Vec<usize>) {
        let mut front = Vec::with_capacity(split.front_count);
        let mut back = Vec::with_capacity(split.back_count);

        for &poly_idx in poly_indices {
            if poly_idx >= mesh.triangles.len() {
                front.push(poly_idx);
                continue;
            }

            match self.which_side(mesh, poly_idx, split.axis, split.dist) {
                PlaneSide::Back => back.push(poly_idx),
                _ => front.push(poly_idx),
            }
        }

        (front, back)
    }

    fn next_random(&mut self) -> u32 {
        const MULT: u64 = 6364136223846793005;
        const INC: u64 = 1442695040888963407;
        self.rng_state = self.rng_state.wrapping_mul(MULT).wrapping_add(INC);
        (self.rng_state >> 16) as u32
    }

    // Count total nodes in tree
    fn count_nodes(&self, node: &Option<Box<AABTreeBuilderNode>>) -> usize {
        match node {
            Some(n) => 1 + self.count_nodes(&n.front) + self.count_nodes(&n.back),
            None => 0,
        }
    }

    // Flatten tree into array representation
    fn flatten_tree(
        &self,
        node: &AABTreeBuilderNode,
        nodes: &mut Vec<AABTreeNode>,
        poly_indices: &mut Vec<u32>,
    ) -> u32 {
        let index = nodes.len() as u32;
        nodes.push(AABTreeNode {
            min: node.min,
            max: node.max,
            ..AABTreeNode::default()
        });

        match (node.front.as_ref(), node.back.as_ref()) {
            (Some(front), Some(back)) => {
                let front_index = self.flatten_tree(front, nodes, poly_indices);
                let back_index = self.flatten_tree(back, nodes, poly_indices);
                if let Some(entry) = nodes.get_mut(index as usize) {
                    entry.set_front_child(front_index);
                    entry.set_back_child(back_index);
                }
            }
            _ => {
                let start = poly_indices.len() as u32;
                let count = node.poly_indices.len() as u32;
                if let Some(entry) = nodes.get_mut(index as usize) {
                    entry.set_poly0(start);
                    entry.set_poly_count(count);
                }
                poly_indices.extend(node.poly_indices.iter().map(|&idx| idx as u32));
            }
        }

        index
    }
}

/// Builder node for AABTree construction
#[derive(Debug)]
struct AABTreeBuilderNode {
    min: Vec3,
    max: Vec3,
    front: Option<Box<AABTreeBuilderNode>>,
    back: Option<Box<AABTreeBuilderNode>>,
    poly_indices: Vec<usize>,
}

impl AABTreeBuilderNode {
    fn new() -> Self {
        Self {
            min: Vec3::new(f32::INFINITY, f32::INFINITY, f32::INFINITY),
            max: Vec3::new(f32::NEG_INFINITY, f32::NEG_INFINITY, f32::NEG_INFINITY),
            front: None,
            back: None,
            poly_indices: Vec::new(),
        }
    }

    fn compute_bounds(&mut self, mesh: &MeshGeometry, poly_indices: &[usize]) {
        for &poly_idx in poly_indices {
            if poly_idx < mesh.triangles.len() {
                let triangle = &mesh.triangles[poly_idx];

                for &vertex_idx in &triangle.indices {
                    if (vertex_idx as usize) < mesh.vertices.len() {
                        let vertex = &mesh.vertices[vertex_idx as usize];
                        self.min = self.min.min(vertex.position);
                        self.max = self.max.max(vertex.position);
                    }
                }
            }
        }
    }
}

// Constants matching C++
pub const AABTREE_LEAF_FLAG: u32 = 0x8000_0000;
const MIN_POLYS_PER_NODE: usize = 4;
const NUM_PLANE_TRIES: usize = 50;
const COINCIDENCE_EPSILON: f32 = 0.001;
const WWMATH_EPSILON: f32 = 1e-5;

// Collision test structures
#[derive(Debug)]
pub struct RayCollisionTest<'a> {
    pub ray: Ray,
    pub mesh: &'a MeshGeometry,
    pub closest_distance: f32,
    pub hit_point: Vec3,
    pub hit_normal: Vec3,
    pub hit_polygon_index: Option<usize>,
}

impl<'a> RayCollisionTest<'a> {
    pub fn new(ray: Ray, mesh: &'a MeshGeometry) -> Self {
        Self {
            ray,
            mesh,
            closest_distance: -1.0,
            hit_point: Vec3::ZERO,
            hit_normal: Vec3::ZERO,
            hit_polygon_index: None,
        }
    }
}

#[derive(Debug)]
pub struct AABoxCollisionTest<'a> {
    pub aabox: AABox,
    pub mesh: &'a MeshGeometry,
    pub hit_polygon_index: Option<usize>,
}

impl<'a> AABoxCollisionTest<'a> {
    pub fn new(aabox: AABox, mesh: &'a MeshGeometry) -> Self {
        Self {
            aabox,
            mesh,
            hit_polygon_index: None,
        }
    }
}

#[derive(Debug)]
pub struct OBBoxCollisionTest<'a> {
    pub obbox: OBBox,
    pub mesh: &'a MeshGeometry,
    pub hit_polygon_index: Option<usize>,
}

impl<'a> OBBoxCollisionTest<'a> {
    pub fn new(obbox: OBBox, mesh: &'a MeshGeometry) -> Self {
        Self {
            obbox,
            mesh,
            hit_polygon_index: None,
        }
    }
}

#[derive(Debug)]
pub struct OBBoxIntersectionTest<'a> {
    pub obbox: OBBox,
    pub mesh: &'a MeshGeometry,
    pub hit_polygon_index: Option<usize>,
}

impl<'a> OBBoxIntersectionTest<'a> {
    pub fn new(obbox: OBBox, mesh: &'a MeshGeometry) -> Self {
        Self {
            obbox,
            mesh,
            hit_polygon_index: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh_geometry::{MeshTriangle, MeshVertex};
    use glam::Vec2;

    #[test]
    fn test_aabtree_creation() {
        let mut mesh = MeshGeometry::new();

        // Add a simple triangle
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(1.0, 0.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::ZERO,
        ));
        mesh.add_vertex(MeshVertex::new(
            Vec3::new(0.0, 1.0, 0.0),
            Vec3::new(0.0, 0.0, 1.0),
            Vec2::ZERO,
        ));

        mesh.add_triangle(MeshTriangle::new(0, 1, 2, 0));

        let mut aabtree = AABTree::new();
        aabtree.build_from_mesh(&mesh);

        assert!(aabtree.get_node_count() > 0);
        assert_eq!(aabtree.get_poly_count(), 1);
    }

    #[test]
    fn test_aabtree_node_leaf() {
        let mut node = AABTreeNode {
            min: Vec3::ZERO,
            max: Vec3::ONE,
            front_or_poly0: 0,
            back_or_poly_count: 0,
        };

        // Test leaf operations
        node.set_poly0(5);
        node.set_poly_count(10);

        assert!(node.is_leaf());
        assert_eq!(node.get_poly0(), 5);
        assert_eq!(node.get_poly_count(), 10);
    }

    #[test]
    fn test_aabtree_node_internal() {
        let mut node = AABTreeNode {
            min: Vec3::ZERO,
            max: Vec3::ONE,
            front_or_poly0: 0,
            back_or_poly_count: 0,
        };

        // Test internal node operations
        node.set_front_child(3);
        node.set_back_child(7);

        assert!(!node.is_leaf());
        assert_eq!(node.get_front_child(), 3);
        assert_eq!(node.get_back_child(), 7);
    }

    #[test]
    fn test_aabtree_indices_are_well_formed() {
        let positions = [
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ];
        let cube_tris = [
            [0, 1, 2],
            [0, 2, 3],
            [4, 6, 5],
            [4, 7, 6],
            [0, 4, 5],
            [0, 5, 1],
            [1, 5, 6],
            [1, 6, 2],
            [2, 6, 7],
            [2, 7, 3],
            [3, 7, 4],
            [3, 4, 0],
        ];

        let mut mesh = MeshGeometry::new();
        for pos in positions {
            mesh.add_vertex(MeshVertex::new(pos, Vec3::Z, Vec2::ZERO));
        }
        for tri in cube_tris {
            mesh.add_triangle(MeshTriangle::new(tri[0], tri[1], tri[2], 0));
        }

        let mut aabtree = AABTree::new();
        aabtree.build_from_mesh(&mesh);
        assert!(!aabtree.nodes.is_empty());
        assert_eq!(aabtree.node_count, aabtree.nodes.len());
        assert_eq!(aabtree.poly_count, aabtree.poly_indices.len());

        for (index, node) in aabtree.nodes.iter().enumerate() {
            if node.is_leaf() {
                let start = node.get_poly0() as usize;
                let count = node.get_poly_count() as usize;
                assert!(
                    start + count <= aabtree.poly_indices.len(),
                    "leaf node {index} references out-of-range polygons"
                );
            } else {
                let front = node.get_front_child() as usize;
                let back = node.get_back_child() as usize;
                assert!(
                    front < aabtree.nodes.len(),
                    "front child {front} of node {index} out of range"
                );
                assert!(
                    back < aabtree.nodes.len(),
                    "back child {back} of node {index} out of range"
                );
            }
        }
    }
}
