/// AABTree Builder - Constructs collision trees from geometry (ported from aabtreebuilder.cpp)
///
/// This module builds AABTrees using a recursive partitioning algorithm with:
/// - Surface Area Heuristic (SAH) for plane selection
/// - Axis-aligned splitting planes
/// - Cost-based tree optimization
use crate::aabtree::{AABTree, CullNode};
use glam::Vec3;
use std::f32;

const MIN_POLYS_PER_NODE: usize = 4;
const SMALL_VERTEX: f32 = -100000.0;
const BIG_VERTEX: f32 = 100000.0;
/// Matches C++ colmath.cpp:40 - for near-coincident sphere/feature detection
const COINCIDENCE_EPSILON: f32 = 0.000001;
const NUM_PLANE_CANDIDATES: usize = 50;

/// Triangle index (matches C++ TriIndex)
#[derive(Debug, Clone, Copy)]
pub struct TriIndex {
    pub i: u32,
    pub j: u32,
    pub k: u32,
}

impl TriIndex {
    pub fn new(i: u32, j: u32, k: u32) -> Self {
        Self { i, j, k }
    }

    pub fn get(&self, index: usize) -> u32 {
        match index {
            0 => self.i,
            1 => self.j,
            2 => self.k,
            _ => panic!("Triangle index out of bounds"),
        }
    }
}

/// Axis-aligned plane
#[derive(Debug, Clone, Copy)]
pub enum AAPlaneAxis {
    X = 0,
    Y = 1,
    Z = 2,
}

#[derive(Debug, Clone)]
pub struct AAPlane {
    pub axis: AAPlaneAxis,
    pub dist: f32,
}

impl AAPlane {
    pub fn new(axis: AAPlaneAxis, dist: f32) -> Self {
        Self { axis, dist }
    }

    #[allow(dead_code)] // Used in tree building algorithm (WIP)
    fn axis_index(&self) -> usize {
        self.axis as usize
    }
}

/// Polygon side classification
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlapType {
    Front,
    Back,
    On,
    Both,
}

/// Builder node structure (more memory-intensive than runtime structure)
struct BuilderNode {
    index: usize,
    #[allow(dead_code)] // Used in tree building algorithm (WIP)
    min: Vec3,
    #[allow(dead_code)] // Used in tree building algorithm (WIP)
    max: Vec3,
    front: Option<Box<BuilderNode>>,
    back: Option<Box<BuilderNode>>,
    poly_indices: Vec<usize>,
}

impl BuilderNode {
    fn new() -> Self {
        Self {
            index: 0,
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            front: None,
            back: None,
            poly_indices: Vec::new(),
        }
    }
}

/// Splitting plane evaluation result
struct SplitChoice {
    cost: f32,
    front_count: usize,
    back_count: usize,
    b_min: Vec3,
    b_max: Vec3,
    f_min: Vec3,
    f_max: Vec3,
    plane: AAPlane,
}

impl SplitChoice {
    fn new() -> Self {
        Self {
            cost: f32::MAX,
            front_count: 0,
            back_count: 0,
            b_min: Vec3::splat(BIG_VERTEX),
            b_max: Vec3::splat(SMALL_VERTEX),
            f_min: Vec3::splat(BIG_VERTEX),
            f_max: Vec3::splat(SMALL_VERTEX),
            plane: AAPlane::new(AAPlaneAxis::X, 0.0),
        }
    }
}

/// Split polygon arrays
struct SplitArrays {
    front_polys: Vec<usize>,
    back_polys: Vec<usize>,
}

/// AABTree Builder
pub struct AABTreeBuilder {
    root: Option<Box<BuilderNode>>,
    polys: Vec<TriIndex>,
    verts: Vec<Vec3>,
}

impl Default for AABTreeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl AABTreeBuilder {
    pub fn new() -> Self {
        Self {
            root: None,
            polys: Vec::new(),
            verts: Vec::new(),
        }
    }

    /// Build AABTree from triangle mesh
    pub fn build_aabtree(&mut self, polys: Vec<TriIndex>, verts: Vec<Vec3>) -> AABTree {
        assert!(!polys.is_empty(), "Polygon count must be > 0");
        assert!(!verts.is_empty(), "Vertex count must be > 0");

        // Store mesh data
        self.polys = polys;
        self.verts = verts;

        // Create initial polygon index list
        let poly_indices: Vec<usize> = (0..self.polys.len()).collect();

        // Build the tree recursively
        self.root = Some(Box::new(BuilderNode::new()));

        // Build tree (take root temporarily to avoid borrow issues)
        if let Some(mut root) = self.root.take() {
            Self::build_tree_internal(&mut root, poly_indices, &self.polys, &self.verts);
            self.root = Some(root);
        }

        // Compute bounding boxes for all nodes
        if let Some(root) = self.root.as_ref() {
            self.compute_bounding_box(root);
        }

        // Assign sequential indices (take root temporarily to avoid borrow issues)
        if let Some(mut root) = self.root.take() {
            self.assign_index(&mut root, 0);
            self.root = Some(root);
        }

        // Convert to runtime AABTree format
        self.export_to_aabtree()
    }

    /// Recursively build the tree
    fn build_tree_internal(
        node: &mut BuilderNode,
        poly_indices: Vec<usize>,
        polys: &[TriIndex],
        verts: &[Vec3],
    ) {
        // Terminate if few enough polygons
        if poly_indices.len() <= MIN_POLYS_PER_NODE {
            node.poly_indices = poly_indices;
            return;
        }

        // Try to find a suitable partitioning plane
        let split_choice = Self::select_splitting_plane_static(&poly_indices, polys, verts);

        // If we couldn't separate any polys, just store them here
        if split_choice.front_count + split_choice.back_count != poly_indices.len() {
            node.poly_indices = poly_indices;
            return;
        }

        // Split the polygons
        let split_arrays = Self::split_polys_static(&poly_indices, &split_choice, polys, verts);

        // Build front tree if necessary
        if !split_arrays.front_polys.is_empty() {
            let mut front_node = Box::new(BuilderNode::new());
            Self::build_tree_internal(&mut front_node, split_arrays.front_polys, polys, verts);
            node.front = Some(front_node);
        }

        // Build back tree if necessary
        if !split_arrays.back_polys.is_empty() {
            let mut back_node = Box::new(BuilderNode::new());
            Self::build_tree_internal(&mut back_node, split_arrays.back_polys, polys, verts);
            node.back = Some(back_node);
        }
    }

    /// Wrapper to call build_tree_internal
    #[allow(dead_code)] // Tree building not yet integrated
    fn build_tree(&mut self, node: &mut BuilderNode, poly_indices: Vec<usize>) {
        Self::build_tree_internal(node, poly_indices, &self.polys, &self.verts);
    }

    /// Select best splitting plane using SAH
    fn select_splitting_plane_static(
        poly_indices: &[usize],
        polys: &[TriIndex],
        verts: &[Vec3],
    ) -> SplitChoice {
        let mut best_split = SplitChoice::new();

        let num_tries = NUM_PLANE_CANDIDATES.min(poly_indices.len());

        for _ in 0..num_tries {
            // Select random polygon and vertex
            let poly_idx = poly_indices[fastrand::usize(..poly_indices.len())];
            let vert_idx = fastrand::usize(0..3);
            let poly = &polys[poly_idx];
            let vert = verts[poly.get(vert_idx) as usize];

            // Try each axis
            for axis_val in 0..3 {
                let axis = match axis_val {
                    0 => AAPlaneAxis::X,
                    1 => AAPlaneAxis::Y,
                    _ => AAPlaneAxis::Z,
                };

                let dist = match axis {
                    AAPlaneAxis::X => vert.x,
                    AAPlaneAxis::Y => vert.y,
                    AAPlaneAxis::Z => vert.z,
                };

                let plane = AAPlane::new(axis, dist);
                let split = Self::compute_plane_score_static(poly_indices, &plane, polys, verts);

                if split.cost < best_split.cost {
                    best_split = split;
                }
            }
        }

        best_split
    }

    /// Evaluate splitting plane quality using surface area heuristic
    fn compute_plane_score_static(
        poly_indices: &[usize],
        plane: &AAPlane,
        polys: &[TriIndex],
        verts: &[Vec3],
    ) -> SplitChoice {
        let mut split = SplitChoice::new();
        split.plane = plane.clone();

        for &poly_idx in poly_indices {
            match Self::which_side_static(plane, poly_idx, polys, verts) {
                OverlapType::Front | OverlapType::On | OverlapType::Both => {
                    split.front_count += 1;
                    Self::update_min_max_static(
                        poly_idx,
                        &mut split.f_min,
                        &mut split.f_max,
                        polys,
                        verts,
                    );
                }
                OverlapType::Back => {
                    split.back_count += 1;
                    Self::update_min_max_static(
                        poly_idx,
                        &mut split.b_min,
                        &mut split.b_max,
                        polys,
                        verts,
                    );
                }
            }
        }

        // Inflate boxes slightly to avoid zero volume
        const EPSILON: f32 = 0.00001;
        split.b_min -= Vec3::splat(EPSILON);
        split.b_max += Vec3::splat(EPSILON);
        split.f_min -= Vec3::splat(EPSILON);
        split.f_max += Vec3::splat(EPSILON);

        // Compute cost = sum of (volume * poly_count) for each child
        let back_volume = (split.b_max.x - split.b_min.x)
            * (split.b_max.y - split.b_min.y)
            * (split.b_max.z - split.b_min.z);
        let front_volume = (split.f_max.x - split.f_min.x)
            * (split.f_max.y - split.f_min.y)
            * (split.f_max.z - split.f_min.z);

        split.cost =
            back_volume * split.back_count as f32 + front_volume * split.front_count as f32;

        // Penalize splits that don't separate anything
        if split.front_count == 0 || split.back_count == 0 {
            split.cost = f32::MAX;
        }

        split
    }

    /// Determine which side of plane the polygon is on
    fn which_side_static(
        plane: &AAPlane,
        poly_idx: usize,
        polys: &[TriIndex],
        verts: &[Vec3],
    ) -> OverlapType {
        let poly = &polys[poly_idx];
        let mut mask = 0u8;
        const POS: u8 = 0x01;
        const NEG: u8 = 0x02;
        const ON: u8 = 0x04;

        for i in 0..3 {
            let point = verts[poly.get(i) as usize];
            let point_coord = match plane.axis {
                AAPlaneAxis::X => point.x,
                AAPlaneAxis::Y => point.y,
                AAPlaneAxis::Z => point.z,
            };
            let delta = point_coord - plane.dist;

            if delta > COINCIDENCE_EPSILON {
                mask |= POS;
            }
            if delta < -COINCIDENCE_EPSILON {
                mask |= NEG;
            }
            mask |= ON;
        }

        // All verts on plane
        if mask == ON {
            return OverlapType::On;
        }

        // All verts POS or ON
        if (mask & !(POS | ON)) == 0 {
            return OverlapType::Front;
        }

        // All verts NEG or ON
        if (mask & !(NEG | ON)) == 0 {
            return OverlapType::Back;
        }

        // Triangle spans plane
        OverlapType::Both
    }

    /// Instance method wrapper for which_side
    #[allow(dead_code)] // Used by tree building algorithm (WIP)
    fn which_side(&self, plane: &AAPlane, poly_idx: usize) -> OverlapType {
        Self::which_side_static(plane, poly_idx, &self.polys, &self.verts)
    }

    /// Split polygons into front and back lists
    fn split_polys_static(
        poly_indices: &[usize],
        split_choice: &SplitChoice,
        polys: &[TriIndex],
        verts: &[Vec3],
    ) -> SplitArrays {
        let mut front_polys = Vec::with_capacity(split_choice.front_count);
        let mut back_polys = Vec::with_capacity(split_choice.back_count);

        for &poly_idx in poly_indices {
            match Self::which_side_static(&split_choice.plane, poly_idx, polys, verts) {
                OverlapType::Front | OverlapType::On | OverlapType::Both => {
                    front_polys.push(poly_idx);
                }
                OverlapType::Back => {
                    back_polys.push(poly_idx);
                }
            }
        }

        SplitArrays {
            front_polys,
            back_polys,
        }
    }

    /// Update min/max bounds to include polygon (static version)
    fn update_min_max_static(
        poly_idx: usize,
        min: &mut Vec3,
        max: &mut Vec3,
        polys: &[TriIndex],
        verts: &[Vec3],
    ) {
        let poly = &polys[poly_idx];
        for i in 0..3 {
            let point = verts[poly.get(i) as usize];
            *min = min.min(point);
            *max = max.max(point);
        }
    }

    /// Update min/max bounds to include polygon (instance wrapper)
    fn update_min_max(&self, poly_idx: usize, min: &mut Vec3, max: &mut Vec3) {
        Self::update_min_max_static(poly_idx, min, max, &self.polys, &self.verts);
    }

    /// Recursively compute bounding boxes
    fn compute_bounding_box(&self, node: &BuilderNode) -> (Vec3, Vec3) {
        let mut min = Vec3::splat(BIG_VERTEX);
        let mut max = Vec3::splat(SMALL_VERTEX);

        // Compute children bounds first
        if let Some(ref front) = node.front {
            let (f_min, f_max) = self.compute_bounding_box(front);
            min = min.min(f_min);
            max = max.max(f_max);
        }

        if let Some(ref back) = node.back {
            let (b_min, b_max) = self.compute_bounding_box(back);
            min = min.min(b_min);
            max = max.max(b_max);
        }

        // Compute polygon bounds
        for &poly_idx in &node.poly_indices {
            self.update_min_max(poly_idx, &mut min, &mut max);
        }

        (min, max)
    }

    /// Assign sequential indices to nodes
    fn assign_index(&mut self, node: &mut BuilderNode, index: usize) -> usize {
        node.index = index;
        let mut next_index = index + 1;

        if let Some(ref mut front) = node.front {
            next_index = self.assign_index(front, next_index);
        }

        if let Some(ref mut back) = node.back {
            next_index = self.assign_index(back, next_index);
        }

        next_index
    }

    /// Count total nodes in tree
    fn count_nodes(&self, node: &BuilderNode) -> usize {
        let mut count = 1;
        if let Some(ref front) = node.front {
            count += self.count_nodes(front);
        }
        if let Some(ref back) = node.back {
            count += self.count_nodes(back);
        }
        count
    }

    /// Export to runtime AABTree format
    fn export_to_aabtree(&self) -> AABTree {
        if self.root.is_none() {
            return AABTree::new();
        }

        let root = self.root.as_ref().unwrap();

        let node_count = self.count_nodes(root);
        let poly_count = self.polys.len();

        let mut nodes = vec![CullNode::new(); node_count];
        let mut poly_indices = Vec::new();

        self.build_runtime_tree_recursive(root, &mut nodes, &mut poly_indices, 0);

        AABTree {
            nodes,
            poly_indices,
            node_count,
            poly_count,
        }
    }

    /// Convert builder tree to runtime format recursively
    fn build_runtime_tree_recursive(
        &self,
        node: &BuilderNode,
        nodes: &mut [CullNode],
        poly_indices: &mut Vec<u32>,
        cur_poly: usize,
    ) -> usize {
        let (min, max) = self.compute_bounding_box(node);

        // Set up the runtime node
        nodes[node.index].min = min;
        nodes[node.index].max = max;

        let mut next_poly = cur_poly;

        // Set up children or polygon indices
        if node.front.is_some() || node.back.is_some() {
            // Non-leaf node
            if let Some(ref front) = node.front {
                nodes[node.index].set_front_child(front.index);
            }
            if let Some(ref back) = node.back {
                nodes[node.index].set_back_child(back.index);
            }
        } else {
            // Leaf node
            nodes[node.index].set_poly0(poly_indices.len());
            nodes[node.index].set_poly_count(node.poly_indices.len());

            // Add polygon indices
            for &poly_idx in &node.poly_indices {
                poly_indices.push(poly_idx as u32);
            }
            next_poly += node.poly_indices.len();
        }

        // Recurse to children
        if let Some(ref front) = node.front {
            next_poly = self.build_runtime_tree_recursive(front, nodes, poly_indices, next_poly);
        }
        if let Some(ref back) = node.back {
            next_poly = self.build_runtime_tree_recursive(back, nodes, poly_indices, next_poly);
        }

        next_poly
    }

    /// Get node count
    pub fn node_count(&self) -> usize {
        self.root.as_ref().map_or(0, |r| self.count_nodes(r))
    }

    /// Get polygon count
    pub fn poly_count(&self) -> usize {
        self.polys.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_builder_simple() {
        let mut builder = AABTreeBuilder::new();

        // Create a simple box mesh (2 triangles)
        let verts = vec![
            Vec3::new(-1.0, -1.0, 0.0),
            Vec3::new(1.0, -1.0, 0.0),
            Vec3::new(1.0, 1.0, 0.0),
            Vec3::new(-1.0, 1.0, 0.0),
        ];

        let polys = vec![TriIndex::new(0, 1, 2), TriIndex::new(0, 2, 3)];

        let tree = builder.build_aabtree(polys, verts);

        assert!(tree.node_count > 0);
        assert_eq!(tree.poly_count, 2);
    }

    #[test]
    fn test_which_side() {
        let builder = AABTreeBuilder {
            root: None,
            polys: vec![TriIndex::new(0, 1, 2)],
            verts: vec![
                Vec3::new(-1.0, 0.0, 0.0),
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
            ],
        };

        let plane = AAPlane::new(AAPlaneAxis::Z, 0.5);
        let side = builder.which_side(&plane, 0);
        assert_eq!(side, OverlapType::Back);

        let plane2 = AAPlane::new(AAPlaneAxis::X, 0.0);
        let side2 = builder.which_side(&plane2, 0);
        assert_eq!(side2, OverlapType::Both);
    }
}
