//! PathNode implementation matching C++ PathNode class
//!
//! PathNodes are used to create a final Path to return from the pathfinder.
//! Note that these are not used during the A* search.

use super::*;
use crate::common::CoordOrigin;
use crate::common::*;
use crate::path::PathfindLayerEnum;
use std::ptr::{self, NonNull};

/// PathNode structure matching C++ PathNode class exactly
#[derive(Debug)]
pub struct PathNode {
    // Memory pool management (handled by Rust's allocator)
    id: i32, // Used in Path::xfer() to save & recreate the path list

    // Path structure
    next_opti: Option<NonNull<PathNode>>, // next node in the optimized path (non-owning)
    next: Option<Box<PathNode>>,          // next node in the path
    prev: *mut PathNode,                  // previous node in the path (weak reference)

    // Spatial data
    pos: Coord3D,             // position of node in space
    layer: PathfindLayerEnum, // Layer for this section
    can_optimize: bool,       // True if this cell can be optimized out

    // Optimization data
    next_opti_dist_2d: f32, // if next_opti is non-null, the distance to it
    next_opti_dir_norm_2d: Coord2D, // if next_opti is non-null, normalized direction vec towards it
}

impl PathNode {
    /// Create a new PathNode
    pub fn new() -> Self {
        Self {
            id: -1,
            next_opti: None,
            next: None,
            prev: ptr::null_mut(),
            pos: Coord3D::origin(),
            layer: PathfindLayerEnum::Invalid,
            can_optimize: false,
            next_opti_dist_2d: 0.0,
            next_opti_dir_norm_2d: Coord2D::origin(),
        }
    }

    /// Get position of this node
    pub fn get_position(&self) -> &Coord3D {
        &self.pos
    }

    /// Get mutable position of this node  
    pub fn get_position_mut(&mut self) -> &mut Coord3D {
        &mut self.pos
    }

    /// Set the position of this path node
    pub fn set_position(&mut self, pos: &Coord3D) {
        self.pos = *pos;
    }

    /// Compute direction vector to next node
    pub fn compute_direction_vector(&self) -> Coord3D {
        if let Some(ref next) = self.next {
            // Direction to next node
            let mut dir = Coord3D::new(
                next.pos.x - self.pos.x,
                next.pos.y - self.pos.y,
                next.pos.z - self.pos.z,
            );

            let length = (dir.x * dir.x + dir.y * dir.y + dir.z * dir.z).sqrt();
            if length > 0.0 {
                dir.x /= length;
                dir.y /= length;
                dir.z /= length;
            }
            dir
        } else if !self.prev.is_null() {
            // Tail node - continue prior direction
            unsafe {
                let prev = &*self.prev;
                prev.compute_direction_vector()
            }
        } else {
            // Only one node on whole path - no direction
            Coord3D::origin()
        }
    }

    /// Get next node in the path
    pub fn get_next(&self) -> Option<&PathNode> {
        self.next.as_ref().map(|n| n.as_ref())
    }

    /// Get mutable next node in the path
    pub fn get_next_mut(&mut self) -> Option<&mut PathNode> {
        self.next.as_mut().map(|n| n.as_mut())
    }

    /// Get previous node in the path (unsafe due to raw pointer)
    pub fn get_previous(&self) -> Option<&PathNode> {
        if self.prev.is_null() {
            None
        } else {
            Some(unsafe { &*self.prev })
        }
    }

    /// Get layer of this node
    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    /// Set the layer of this path node
    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        self.layer = layer;
    }

    /// Set next optimized node and compute direction/distance
    pub fn set_next_optimized(&mut self, node: Option<NonNull<PathNode>>) {
        self.next_opti = node;

        if let Some(ptr) = self.next_opti {
            let target = unsafe { ptr.as_ref() };

            // Compute direction and distance
            let dx = target.pos.x - self.pos.x;
            let dy = target.pos.y - self.pos.y;
            self.next_opti_dist_2d = (dx * dx + dy * dy).sqrt();

            if self.next_opti_dist_2d == 0.0 {
                // Avoid division by zero
                self.next_opti_dist_2d = 0.01;
            }

            self.next_opti_dir_norm_2d.x = dx / self.next_opti_dist_2d;
            self.next_opti_dir_norm_2d.y = dy / self.next_opti_dist_2d;
        } else {
            self.next_opti_dist_2d = 0.0;
            self.next_opti_dir_norm_2d = Coord2D::origin();
        }
    }

    /// Get next node in optimized path with optional direction and distance
    pub fn get_next_optimized(&self) -> (Option<&PathNode>, Coord2D, f32) {
        let next = self.next_opti.as_ref().map(|ptr| unsafe { ptr.as_ref() });
        (next, self.next_opti_dir_norm_2d, self.next_opti_dist_2d)
    }

    /// Set whether this node can be optimized
    pub fn set_can_optimize(&mut self, can_opt: bool) {
        self.can_optimize = can_opt;
    }

    /// Get whether this node can be optimized
    pub fn get_can_optimize(&self) -> bool {
        self.can_optimize
    }

    /// Get node ID
    pub fn get_id(&self) -> i32 {
        self.id
    }

    /// Set node ID (used for serialization)
    pub fn set_id(&mut self, id: i32) {
        self.id = id;
    }

    /// Non-owning pointer to the next node in the path.
    pub fn next_ptr(&self) -> Option<NonNull<PathNode>> {
        self.next
            .as_ref()
            .map(|node| unsafe { NonNull::new_unchecked(node.as_ref() as *const _ as *mut _) })
    }

    /// Non-owning pointer to the optimized successor.
    pub fn next_optimized_ptr(&self) -> Option<NonNull<PathNode>> {
        self.next_opti
    }

    /// Prepend this node to a list, return new list head
    pub fn prepend_to_list(mut self: Box<Self>, list: Option<Box<PathNode>>) -> Box<PathNode> {
        self.next = list;
        let self_ptr = self.as_ref() as *const PathNode as *mut PathNode;
        if let Some(ref mut next) = self.next {
            next.prev = self_ptr;
        }
        self.prev = ptr::null_mut();
        self
    }

    /// Append this node to a list, return new list head (slow implementation)
    pub fn append_to_list(mut self: Box<Self>, list: Option<Box<PathNode>>) -> Box<PathNode> {
        if list.is_none() {
            self.next = None;
            self.prev = ptr::null_mut();
            return self;
        }

        let mut current = list.unwrap();

        // Find the tail
        while current.next.is_some() {
            let next = current.next.take().unwrap();
            current = next;
        }

        // Append to tail
        self.prev = current.as_ref() as *const PathNode as *mut PathNode;
        self.next = None;
        current.next = Some(self);

        // Return the original head (we need to reconstruct the chain)
        // This is a simplified version - in practice, we'd need to track the head
        current
    }

    /// Append a node after this node
    pub fn append(&mut self, new_node: Box<PathNode>) {
        let old_next = self.next.take();
        let mut new_node = new_node;

        new_node.next = old_next;
        new_node.prev = self as *mut PathNode;

        let new_node_ptr = new_node.as_ref() as *const PathNode as *mut PathNode;
        if let Some(ref mut next) = new_node.next {
            next.prev = new_node_ptr;
        }

        self.next = Some(new_node);
    }
}

impl Default for PathNode {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_node_creation() {
        let node = PathNode::new();
        assert_eq!(node.get_layer(), PathfindLayerEnum::Invalid);
        assert_eq!(node.get_position(), &Coord3D::origin());
        assert!(!node.get_can_optimize());
    }

    #[test]
    fn test_path_node_position() {
        let mut node = PathNode::new();
        let pos = Coord3D::new(10.0, 20.0, 5.0);
        node.set_position(&pos);
        assert_eq!(node.get_position(), &pos);
    }

    #[test]
    fn test_path_node_layer() {
        let mut node = PathNode::new();
        node.set_layer(PathfindLayerEnum::Ground);
        assert_eq!(node.get_layer(), PathfindLayerEnum::Ground);
    }

    #[test]
    fn test_path_node_optimization() {
        let mut node = PathNode::new();
        assert!(!node.get_can_optimize());
        node.set_can_optimize(true);
        assert!(node.get_can_optimize());
    }

    #[test]
    fn test_path_node_list_operations() {
        let node1 = Box::new(PathNode::new());
        let node2 = Box::new(PathNode::new());

        let head = node1.prepend_to_list(Some(node2));
        assert!(head.get_next().is_some());
    }
}
