// Command & Conquer Generals Zero Hour
// Copyright 2025 Electronic Arts Inc.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Axis-Aligned Bounding Box Tree culling system implementation.
//!
//! This culling system uses a hierarchical spatial partitioning structure
//! that adapts to the geometry placed in it and can therefore cull objects
//! more efficiently than uniform grid systems.

use super::{CollisionMath, CullCollection, CullStats, CullSystem, Cullable, OverlapType};
use crate::{AABox, AAPlane, AxisEnum, Frustum, MinMaxAABox, Vector3};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;

/// Split choice information for tree partitioning
#[derive(Debug)]
struct SplitChoice {
    cost: f32,
    front_count: usize,
    back_count: usize,
    front_box: MinMaxAABox,
    back_box: MinMaxAABox,
    plane: AAPlane,
}

impl SplitChoice {
    fn new() -> Self {
        Self {
            cost: f32::MAX,
            front_count: 0,
            back_count: 0,
            front_box: MinMaxAABox::empty(),
            back_box: MinMaxAABox::empty(),
            plane: AAPlane::new(AxisEnum::XNormal, 0.0),
        }
    }
}

/// Node in the AAB tree structure
pub struct AABTreeNode {
    /// Unique index for this node
    pub index: u32,
    /// Bounding box of the node
    pub bbox: AABox,
    /// Parent node
    pub parent: Option<Rc<RefCell<AABTreeNode>>>,
    /// Front child node
    pub front: Option<Rc<RefCell<AABTreeNode>>>,
    /// Back child node  
    pub back: Option<Rc<RefCell<AABTreeNode>>>,
    /// Objects contained in this node
    pub objects: Vec<Arc<dyn Cullable>>,
    /// User data field
    pub user_data: u32,
}

impl AABTreeNode {
    fn new() -> Self {
        Self {
            index: 0,
            bbox: AABox::new(Vector3::ZERO, Vector3::ZERO),
            parent: None,
            front: None,
            back: None,
            objects: Vec::new(),
            user_data: 0,
        }
    }

    fn add_object(&mut self, obj: Arc<dyn Cullable>, update_bounds: bool) {
        self.objects.push(obj.clone());

        if update_bounds {
            if self.objects.len() == 1 && self.front.is_none() && self.back.is_none() {
                // First object and no children - use object's bbox
                self.bbox = obj.get_cull_box();
            } else {
                // Expand to include new object
                self.bbox.add_box(&obj.get_cull_box());
            }
        }
    }

    fn remove_object(&mut self, obj_id: u64) -> bool {
        let initial_len = self.objects.len();
        self.objects.retain(|o| o.get_id() != obj_id);
        self.objects.len() != initial_len
    }

    fn object_count(&self) -> usize {
        self.objects.len()
    }

    fn compute_volume(&self) -> f32 {
        self.bbox.volume()
    }

    fn compute_local_bounding_box(&mut self) {
        let mut min_max_box = MinMaxAABox::empty();

        // Include child nodes
        if let Some(ref front) = self.front {
            min_max_box.add_aabox(&front.borrow().bbox);
        }
        if let Some(ref back) = self.back {
            min_max_box.add_aabox(&back.borrow().bbox);
        }

        // Include objects
        for obj in &self.objects {
            min_max_box.add_aabox(&obj.get_cull_box());
        }

        self.bbox = AABox::from(min_max_box);
    }

    fn transfer_objects(&mut self, target_node: &mut AABTreeNode) {
        // Move all objects to target node
        for obj in self.objects.drain(..) {
            target_node.add_object(obj, true);
        }

        // Recursively transfer from children
        if let Some(ref front) = self.front {
            front.borrow_mut().transfer_objects(target_node);
        }
        if let Some(ref back) = self.back {
            back.borrow_mut().transfer_objects(target_node);
        }
    }

    /// Partition this node based on contained objects
    fn partition(&mut self) {
        if self.object_count() <= 2 {
            return;
        }

        // Create array of bounding boxes
        let boxes: Vec<AABox> = self.objects.iter().map(|obj| obj.get_cull_box()).collect();

        // Select splitting plane
        let split_choice = self.select_splitting_plane(&boxes);

        if split_choice.cost == f32::MAX {
            return; // No good split found
        }

        // Create child nodes
        let mut front_node = AABTreeNode::new();
        let mut back_node = AABTreeNode::new();

        // Split objects
        self.split_objects(&split_choice, &mut front_node, &mut back_node);

        // Create children if they have objects
        if front_node.object_count() > 0 {
            front_node.bbox = AABox::from(split_choice.front_box);
            let front_rc = Rc::new(RefCell::new(front_node));
            self.front = Some(front_rc.clone());
            front_rc.borrow_mut().partition();
        }

        if back_node.object_count() > 0 {
            back_node.bbox = AABox::from(split_choice.back_box);
            let back_rc = Rc::new(RefCell::new(back_node));
            self.back = Some(back_rc.clone());
            back_rc.borrow_mut().partition();
        }
    }

    fn select_splitting_plane(&self, boxes: &[AABox]) -> SplitChoice {
        const MAX_TRIES: usize = 300;
        let mut best_choice = SplitChoice::new();

        let obj_count = boxes.len();
        let tries = std::cmp::min(MAX_TRIES, obj_count);

        for _ in 0..tries {
            let obj_index = fastrand::usize(..obj_count);
            let bbox = boxes[obj_index];

            // Try each face of the bounding box
            for face in 0..6 {
                let plane = match face {
                    0 => AAPlane::new(AxisEnum::XNormal, bbox.center.x + bbox.extent.x),
                    1 => AAPlane::new(AxisEnum::XNormal, bbox.center.x - bbox.extent.x),
                    2 => AAPlane::new(AxisEnum::YNormal, bbox.center.y + bbox.extent.y),
                    3 => AAPlane::new(AxisEnum::YNormal, bbox.center.y - bbox.extent.y),
                    4 => AAPlane::new(AxisEnum::ZNormal, bbox.center.z + bbox.extent.z),
                    5 => AAPlane::new(AxisEnum::ZNormal, bbox.center.z - bbox.extent.z),
                    _ => continue,
                };

                let choice = self.compute_score(&plane, boxes);
                if choice.cost < best_choice.cost {
                    best_choice = choice;
                }
            }
        }

        best_choice
    }

    fn compute_score(&self, plane: &AAPlane, boxes: &[AABox]) -> SplitChoice {
        let mut choice = SplitChoice::new();
        choice.plane = plane.clone();

        for bbox in boxes {
            if self.point_in_front_of_plane(plane, bbox.center) {
                choice.front_count += 1;
                choice.front_box.add_aabox(bbox);
            } else {
                choice.back_count += 1;
                choice.back_box.add_aabox(bbox);
            }
        }

        // Compute cost
        let back_cost = choice.back_box.volume() * choice.back_count as f32;
        let front_cost = choice.front_box.volume() * choice.front_count as f32;
        choice.cost = front_cost + back_cost;

        if choice.front_count == 0 || choice.back_count == 0 {
            choice.cost = f32::MAX;
        }

        choice
    }

    fn point_in_front_of_plane(&self, plane: &AAPlane, point: Vector3) -> bool {
        let distance = match plane.normal {
            AxisEnum::XNormal => point.x - plane.dist,
            AxisEnum::YNormal => point.y - plane.dist,
            AxisEnum::ZNormal => point.z - plane.dist,
        };
        distance >= 0.0
    }

    fn split_objects(
        &mut self,
        split_choice: &SplitChoice,
        front_node: &mut AABTreeNode,
        back_node: &mut AABTreeNode,
    ) {
        // Extract the plane information to avoid borrowing self
        let plane_normal = split_choice.plane.normal;
        let plane_dist = split_choice.plane.dist;

        for obj in self.objects.drain(..) {
            let center = obj.get_cull_box().center;
            let distance = match plane_normal {
                AxisEnum::XNormal => center.x - plane_dist,
                AxisEnum::YNormal => center.y - plane_dist,
                AxisEnum::ZNormal => center.z - plane_dist,
            };

            if distance >= 0.0 {
                front_node.add_object(obj, true);
            } else {
                back_node.add_object(obj, true);
            }
        }
    }
}

/// Link information for objects in the AAB tree
struct AABTreeLink {
    /// Node containing this object
    node: Option<Weak<RefCell<AABTreeNode>>>,
}

/// Axis-Aligned Bounding Box Tree culling system
pub struct AABTreeCullSystem {
    /// Root node of the tree
    root_node: Rc<RefCell<AABTreeNode>>,
    /// Total number of objects in the system
    object_count: usize,
    /// Number of nodes in the tree
    node_count: usize,
    /// Indexed access to nodes
    indexed_nodes: Vec<Rc<RefCell<AABTreeNode>>>,
    /// Object link information
    object_links: HashMap<u64, AABTreeLink>,
    /// Current collection of culled objects
    collection: CullCollection,
    /// Statistics
    stats: CullStats,
}

impl AABTreeCullSystem {
    pub fn new() -> Self {
        let root = Rc::new(RefCell::new(AABTreeNode::new()));
        let mut system = Self {
            root_node: root,
            object_count: 0,
            node_count: 0,
            indexed_nodes: Vec::new(),
            object_links: HashMap::new(),
            collection: CullCollection::new(),
            stats: CullStats::new(),
        };

        system.re_index_nodes();
        system
    }

    /// Re-partition the tree based on contained objects
    pub fn re_partition(&mut self) {
        // Transfer all objects to a dummy node
        let mut dummy_node = AABTreeNode::new();
        self.root_node
            .borrow_mut()
            .transfer_objects(&mut dummy_node);

        // Create new root
        self.root_node = Rc::new(RefCell::new(dummy_node));

        // Partition the objects
        self.root_node.borrow_mut().partition();

        // Re-index nodes
        self.re_index_nodes();
        self.reset_statistics();
    }

    /// Update bounding boxes throughout the tree
    pub fn update_bounding_boxes(&mut self) {
        Self::update_bounding_boxes_recursive(&self.root_node.clone());
    }

    fn update_bounding_boxes_recursive(node: &Rc<RefCell<AABTreeNode>>) {
        // First update children
        {
            let node_ref = node.borrow();
            if let Some(ref front) = node_ref.front {
                Self::update_bounding_boxes_recursive(front);
            }
            if let Some(ref back) = node_ref.back {
                Self::update_bounding_boxes_recursive(back);
            }
        }

        // Then update this node's bounding box
        node.borrow_mut().compute_local_bounding_box();
    }

    /// Get the bounding box of the entire tree
    pub fn get_bounding_box(&self) -> AABox {
        self.root_node.borrow().bbox
    }

    /// Get node bounds by index
    pub fn get_node_bounds(&self, node_id: usize) -> Option<AABox> {
        self.indexed_nodes
            .get(node_id)
            .map(|node| node.borrow().bbox)
    }

    /// Get number of partition nodes
    pub fn partition_node_count(&self) -> usize {
        Self::partition_node_count_recursive(&self.root_node)
    }

    fn partition_node_count_recursive(node: &Rc<RefCell<AABTreeNode>>) -> usize {
        let mut count = 1;
        let node_ref = node.borrow();

        if let Some(ref front) = node_ref.front {
            count += Self::partition_node_count_recursive(front);
        }
        if let Some(ref back) = node_ref.back {
            count += Self::partition_node_count_recursive(back);
        }

        count
    }

    /// Get maximum tree depth
    pub fn partition_tree_depth(&self) -> usize {
        let mut max_depth = 0;
        Self::partition_tree_depth_recursive(&self.root_node, 0, &mut max_depth);
        max_depth
    }

    fn partition_tree_depth_recursive(
        node: &Rc<RefCell<AABTreeNode>>,
        cur_depth: usize,
        max_depth: &mut usize,
    ) {
        let depth = cur_depth + 1;
        if depth > *max_depth {
            *max_depth = depth;
        }

        let node_ref = node.borrow();
        if let Some(ref front) = node_ref.front {
            Self::partition_tree_depth_recursive(front, depth, max_depth);
        }
        if let Some(ref back) = node_ref.back {
            Self::partition_tree_depth_recursive(back, depth, max_depth);
        }
    }

    fn add_object_recursive(&mut self, node: &Rc<RefCell<AABTreeNode>>, obj: Arc<dyn Cullable>) {
        let obj_bbox = obj.get_cull_box();

        // Check children to see if object fits better there
        let node_ref = node.borrow();
        let mut best_child: Option<Rc<RefCell<AABTreeNode>>> = None;

        // Order children by volume (prefer smaller child first for better fit)
        if let (Some(ref front), Some(ref back)) = (&node_ref.front, &node_ref.back) {
            let front_vol = front.borrow().compute_volume();
            let back_vol = back.borrow().compute_volume();

            let (small_child, big_child) = if front_vol < back_vol {
                (front.clone(), back.clone())
            } else {
                (back.clone(), front.clone())
            };

            // Try smaller child first
            if small_child.borrow().bbox.contains_box(&obj_bbox) {
                best_child = Some(small_child);
            } else if big_child.borrow().bbox.contains_box(&obj_bbox) {
                best_child = Some(big_child);
            }
        } else if let Some(child) = node_ref.front.as_ref().or(node_ref.back.as_ref()) {
            if child.borrow().bbox.contains_box(&obj_bbox) {
                best_child = Some(child.clone());
            }
        }

        drop(node_ref);

        if let Some(child) = best_child {
            self.add_object_recursive(&child, obj);
        } else {
            // Add to this node
            node.borrow_mut().add_object(obj.clone(), true);

            // Update object link
            let obj_id = obj.get_id();
            self.object_links.insert(
                obj_id,
                AABTreeLink {
                    node: Some(Rc::downgrade(node)),
                },
            );

            self.object_count += 1;
        }
    }

    fn collect_objects_recursive_point(&mut self, node: &Rc<RefCell<AABTreeNode>>, point: Vector3) {
        let node_ref = node.borrow();

        // Test if point is inside this node's bounding box
        if !node_ref.bbox.contains_point(point) {
            self.stats.nodes_rejected += 1;
            return;
        }

        self.stats.nodes_accepted += 1;

        // Collect objects in this node
        for obj in &node_ref.objects {
            if obj.get_cull_box().contains_point(point) {
                self.collection.add(obj.clone());
            }
        }

        // Recurse into children
        let front_child = node_ref.front.clone();
        let back_child = node_ref.back.clone();
        drop(node_ref);

        if let Some(ref front) = front_child {
            self.collect_objects_recursive_point(front, point);
        }
        if let Some(ref back) = back_child {
            self.collect_objects_recursive_point(back, point);
        }
    }

    fn collect_objects_recursive_box(
        &mut self,
        node: &Rc<RefCell<AABTreeNode>>,
        query_box: &AABox,
    ) {
        let node_ref = node.borrow();

        // Test overlap with node's bounding box
        let overlap = CollisionMath::overlap_test_box_box(query_box, &node_ref.bbox);
        match overlap {
            OverlapType::Outside => {
                self.stats.nodes_rejected += 1;
                return;
            }
            OverlapType::Inside => {
                // Query box completely contains this node - collect everything
                self.collect_objects_recursive_all(node);
                return;
            }
            OverlapType::Intersecting => {
                self.stats.nodes_accepted += 1;
            }
        }

        // Test objects in this node
        for obj in &node_ref.objects {
            if CollisionMath::overlap_test_box_box(query_box, &obj.get_cull_box())
                != OverlapType::Outside
            {
                self.collection.add(obj.clone());
            }
        }

        // Recurse into children
        let front_child = node_ref.front.clone();
        let back_child = node_ref.back.clone();
        drop(node_ref);

        if let Some(ref front) = front_child {
            self.collect_objects_recursive_box(front, query_box);
        }
        if let Some(ref back) = back_child {
            self.collect_objects_recursive_box(back, query_box);
        }
    }

    fn collect_objects_recursive_frustum(
        &mut self,
        node: &Rc<RefCell<AABTreeNode>>,
        frustum: &Frustum,
    ) {
        let node_ref = node.borrow();

        // Test overlap with node's bounding box
        let overlap = CollisionMath::overlap_test_frustum_box(frustum, &node_ref.bbox);
        match overlap {
            OverlapType::Outside => {
                self.stats.nodes_rejected += 1;
                return;
            }
            OverlapType::Inside => {
                // Node is completely inside frustum - collect everything
                self.collect_objects_recursive_all(node);
                return;
            }
            OverlapType::Intersecting => {
                self.stats.nodes_accepted += 1;
            }
        }

        // Test objects in this node
        for obj in &node_ref.objects {
            if CollisionMath::overlap_test_frustum_box(frustum, &obj.get_cull_box())
                != OverlapType::Outside
            {
                self.collection.add(obj.clone());
            }
        }

        // Recurse into children
        let front_child = node_ref.front.clone();
        let back_child = node_ref.back.clone();
        drop(node_ref);

        if let Some(ref front) = front_child {
            self.collect_objects_recursive_frustum(front, frustum);
        }
        if let Some(ref back) = back_child {
            self.collect_objects_recursive_frustum(back, frustum);
        }
    }

    fn collect_objects_recursive_all(&mut self, node: &Rc<RefCell<AABTreeNode>>) {
        let node_ref = node.borrow();

        // Collect all objects in this node
        for obj in &node_ref.objects {
            self.collection.add(obj.clone());
        }

        self.stats.nodes_trivially_accepted += 1;

        // Recurse into children
        let front_child = node_ref.front.clone();
        let back_child = node_ref.back.clone();
        drop(node_ref);

        if let Some(ref front) = front_child {
            self.collect_objects_recursive_all(front);
        }
        if let Some(ref back) = back_child {
            self.collect_objects_recursive_all(back);
        }
    }

    fn re_index_nodes(&mut self) {
        self.indexed_nodes.clear();
        self.node_count = self.partition_node_count();
        self.indexed_nodes.reserve(self.node_count);

        let mut counter = 0;
        Self::re_index_nodes_recursive(&self.root_node, &mut self.indexed_nodes, &mut counter);
    }

    fn re_index_nodes_recursive(
        node: &Rc<RefCell<AABTreeNode>>,
        indexed_nodes: &mut Vec<Rc<RefCell<AABTreeNode>>>,
        counter: &mut u32,
    ) {
        node.borrow_mut().index = *counter;
        indexed_nodes.push(node.clone());
        *counter += 1;

        let front_child = node.borrow().front.clone();
        let back_child = node.borrow().back.clone();

        if let Some(ref front) = front_child {
            Self::re_index_nodes_recursive(front, indexed_nodes, counter);
        }
        if let Some(ref back) = back_child {
            Self::re_index_nodes_recursive(back, indexed_nodes, counter);
        }
    }

    fn reset_statistics(&mut self) {
        self.stats = CullStats::new();
        self.stats.node_count = self.node_count;
    }
}

impl Default for AABTreeCullSystem {
    fn default() -> Self {
        Self::new()
    }
}

impl CullSystem for AABTreeCullSystem {
    fn reset_collection(&mut self) {
        self.collection.clear();
    }

    fn collect_objects_point(&mut self, point: Vector3) {
        self.collect_objects_recursive_point(&self.root_node.clone(), point);
    }

    fn collect_objects_box(&mut self, box_: &AABox) {
        self.collect_objects_recursive_box(&self.root_node.clone(), box_);
    }

    fn collect_objects_frustum(&mut self, frustum: &Frustum) {
        self.collect_objects_recursive_frustum(&self.root_node.clone(), frustum);
    }

    fn update_culling(&mut self, object: &Arc<dyn Cullable>) {
        let obj_id = object.get_id();

        // Remove from current location
        if let Some(link) = self.object_links.get(&obj_id) {
            if let Some(ref weak_node) = link.node {
                if let Some(node) = weak_node.upgrade() {
                    if node.borrow_mut().remove_object(obj_id) {
                        self.object_count -= 1;
                    }
                }
            }
        }

        // Re-add to tree
        self.add_object_recursive(&self.root_node.clone(), object.clone());
    }

    fn add_object(&mut self, object: Arc<dyn Cullable>) {
        self.add_object_recursive(&self.root_node.clone(), object);
    }

    fn remove_object(&mut self, object: &Arc<dyn Cullable>) {
        let obj_id = object.get_id();

        if let Some(link) = self.object_links.remove(&obj_id) {
            if let Some(ref weak_node) = link.node {
                if let Some(node) = weak_node.upgrade() {
                    if node.borrow_mut().remove_object(obj_id) {
                        self.object_count -= 1;
                    }
                }
            }
        }
    }

    fn get_collection(&mut self) -> &mut CullCollection {
        &mut self.collection
    }

    fn get_stats(&self) -> CullStats {
        self.stats
    }

    fn reset_stats(&mut self) {
        self.reset_statistics();
    }

    fn get_object_count(&self) -> usize {
        self.object_count
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Matrix3D, Vector2};

    #[derive(Debug)]
    struct TestObject {
        id: u64,
        bbox: AABox,
    }

    impl TestObject {
        fn new(id: u64, center: Vector3, extent: Vector3) -> Self {
            Self {
                id,
                bbox: AABox::new(center, extent),
            }
        }
    }

    impl Cullable for TestObject {
        fn get_cull_box(&self) -> AABox {
            self.bbox
        }

        fn set_cull_box(&mut self, box_: AABox, _just_loaded: bool) {
            self.bbox = box_;
        }

        fn get_id(&self) -> u64 {
            self.id
        }
    }

    #[test]
    fn test_aab_tree_node() {
        let mut node = AABTreeNode::new();
        let obj = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));

        node.add_object(obj, true);
        assert_eq!(node.object_count(), 1);
        assert_ne!(node.bbox.extent, Vector3::ZERO);

        assert!(node.remove_object(1));
        assert_eq!(node.object_count(), 0);
    }

    #[test]
    fn test_split_choice() {
        let choice = SplitChoice::new();
        assert_eq!(choice.cost, f32::MAX);
        assert_eq!(choice.front_count, 0);
        assert_eq!(choice.back_count, 0);
    }

    #[test]
    fn test_aab_tree_cull_system_creation() {
        let tree = AABTreeCullSystem::new();
        assert_eq!(tree.get_object_count(), 0);
        assert!(tree.partition_node_count() > 0);
    }

    #[test]
    fn test_aab_tree_add_remove_objects() {
        let mut tree = AABTreeCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        tree.add_object(obj1.clone());
        tree.add_object(obj2.clone());

        assert_eq!(tree.get_object_count(), 2);

        tree.remove_object(&obj1);
        assert_eq!(tree.get_object_count(), 1);

        tree.remove_object(&obj2);
        assert_eq!(tree.get_object_count(), 0);
    }

    #[test]
    fn test_aab_tree_collect_objects_point() {
        let mut tree = AABTreeCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(2.0, 2.0, 2.0),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        tree.add_object(obj1);
        tree.add_object(obj2);

        tree.reset_collection();
        tree.collect_objects_point(Vector3::new(1.0, 1.0, 1.0)); // Inside obj1

        assert!(tree.get_collection().len() >= 1);
        if let Some(first_obj) = tree.get_collection().peek_first() {
            assert_eq!(first_obj.get_id(), 1);
        }
    }

    #[test]
    fn test_aab_tree_collect_objects_box() {
        let mut tree = AABTreeCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(10.0, 10.0, 10.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        tree.add_object(obj1);
        tree.add_object(obj2);

        let query_box = AABox::new(Vector3::new(0.5, 0.5, 0.5), Vector3::new(2.0, 2.0, 2.0));

        tree.reset_collection();
        tree.collect_objects_box(&query_box);

        assert!(tree.get_collection().len() >= 1);
    }

    #[test]
    fn test_aab_tree_collect_objects_frustum() {
        let mut tree = AABTreeCullSystem::new();

        let obj1 = Arc::new(TestObject::new(
            1,
            Vector3::new(0.0, 0.0, -5.0),
            Vector3::new(0.5, 0.5, 0.5),
        ));
        let obj2 = Arc::new(TestObject::new(
            2,
            Vector3::new(100.0, 100.0, -5.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));

        tree.add_object(obj1);
        tree.add_object(obj2);

        let mut frustum = Frustum::default();
        let camera = Matrix3D::IDENTITY;
        let vp_min = Vector2::new(-1.0, -1.0);
        let vp_max = Vector2::new(1.0, 1.0);
        frustum.init(camera, vp_min, vp_max, -1.0, -10.0);

        tree.reset_collection();
        tree.collect_objects_frustum(&frustum);

        // Should collect at least the object inside the frustum
        assert!(tree.get_collection().len() >= 1);
    }

    #[test]
    fn test_aab_tree_repartition() {
        let mut tree = AABTreeCullSystem::new();

        // Add several objects
        for i in 0..10 {
            let obj = Arc::new(TestObject::new(
                i as u64,
                Vector3::new(i as f32 * 2.0, i as f32 * 2.0, 0.0),
                Vector3::new(1.0, 1.0, 1.0),
            ));
            tree.add_object(obj);
        }

        assert_eq!(tree.get_object_count(), 10);

        // Re-partition
        tree.re_partition();

        // All objects should still be present
        assert_eq!(tree.get_object_count(), 10);
    }

    #[test]
    fn test_aab_tree_bounding_box() {
        let mut tree = AABTreeCullSystem::new();

        let obj = Arc::new(TestObject::new(
            1,
            Vector3::new(5.0, 5.0, 5.0),
            Vector3::new(1.0, 1.0, 1.0),
        ));
        tree.add_object(obj);

        let bbox = tree.get_bounding_box();
        assert!(bbox.contains_point(Vector3::new(5.0, 5.0, 5.0)));
    }

    #[test]
    fn test_aab_tree_update_culling() {
        let mut tree = AABTreeCullSystem::new();

        let obj = Arc::new(TestObject::new(
            1,
            Vector3::ZERO,
            Vector3::new(1.0, 1.0, 1.0),
        ));
        tree.add_object(obj.clone());

        // Update object's position
        tree.update_culling(&obj);

        // Object should still be in the system
        assert_eq!(tree.get_object_count(), 1);
    }

    #[test]
    fn test_aab_tree_stats() {
        let tree = AABTreeCullSystem::new();
        let stats = tree.get_stats();

        assert!(stats.node_count > 0);
        assert_eq!(stats.nodes_accepted, 0);
        assert_eq!(stats.nodes_rejected, 0);
    }

    #[test]
    fn test_aab_tree_depth_and_node_count() {
        let mut tree = AABTreeCullSystem::new();

        // Add objects to create a tree structure
        for i in 0..8 {
            let obj = Arc::new(TestObject::new(
                i as u64,
                Vector3::new(i as f32 * 10.0, 0.0, 0.0),
                Vector3::new(1.0, 1.0, 1.0),
            ));
            tree.add_object(obj);
        }

        tree.re_partition();

        let depth = tree.partition_tree_depth();
        let node_count = tree.partition_node_count();

        assert!(depth > 0);
        assert!(node_count > 0);
    }
}
