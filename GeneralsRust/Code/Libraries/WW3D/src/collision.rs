// Collision Detection System
// Ported from coltest.h, AABTree

use crate::math::*;

// AABTree for hierarchical polygon culling
pub struct AABTree {
    pub nodes: Vec<AABTreeNode>,
    pub poly_indices: Vec<u32>,
}

pub struct AABTreeNode {
    pub min: Vec3,
    pub max: Vec3,
    pub front_child: i32,
    pub back_child: i32,
    pub poly_begin: u32,
    pub poly_count: u32,
}

impl AABTree {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            poly_indices: Vec::new(),
        }
    }
}

impl Default for AABTree {
    fn default() -> Self {
        Self::new()
    }
}
