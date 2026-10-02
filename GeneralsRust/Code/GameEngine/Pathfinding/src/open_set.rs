// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// Open-list machinery: the search node record, its cost comparator and the
// binary-heap open set that reproduces C++ putOnSortedOpenList()'s FIFO
// tie-break for equal costs.

use std::cmp::Ordering;
use std::collections::BinaryHeap;

use crate::cell::{GridCoord, PathfindLayerEnum};

/// Internal A* key: C++ keeps a distinct PathfindCell per (x, y, layer).
/// Public GridCoord stays (x, y); layer is search-only.
pub(crate) type SearchKey = (GridCoord, PathfindLayerEnum);

/// A* node for priority queue
/// Matches C++ PathfindCell structure at AIPathfind.cpp:6137-6357
#[derive(Debug, Clone)]
pub(crate) struct AStarNode {
    pub(crate) coord: GridCoord,
    pub(crate) layer: PathfindLayerEnum,
    pub(crate) g_score: u32, // Cost from start
    pub(crate) f_score: u32, // g_score + h_score
    pub(crate) parent: Option<SearchKey>,
    /// FIFO position assigned at enqueue time. CPP inserts after existing
    /// nodes with the same total cost.
    pub(crate) enqueue_order: u64,
}

impl PartialEq for AStarNode {
    fn eq(&self, other: &Self) -> bool {
        self.f_score == other.f_score && self.enqueue_order == other.enqueue_order
    }
}

impl Eq for AStarNode {}

impl PartialOrd for AStarNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for AStarNode {
    /// Min-heap by total cost, then stable enqueue order.
    /// Matches CPP PathfindCell::putOnSortedOpenList()'s insertion-after-equals.
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f_score
            .cmp(&self.f_score)
            .then_with(|| other.enqueue_order.cmp(&self.enqueue_order))
    }
}

/// Open-list storage preserving CPP's stable insertion order for equal costs.
/// Decreased nodes are enqueued as new generations; callers reject stale
/// generations against their current membership and g-score tables.
pub(crate) struct OpenSet {
    nodes: BinaryHeap<AStarNode>,
    /// Rebased directly by the comparator fixtures when pinning overflow.
    pub(crate) next_enqueue_order: u64,
}

impl OpenSet {
    pub(crate) fn new() -> Self {
        Self {
            nodes: BinaryHeap::new(),
            next_enqueue_order: 0,
        }
    }

    pub(crate) fn push(&mut self, mut node: AStarNode) {
        if self.next_enqueue_order == u64::MAX {
            self.rebase_enqueue_order();
        }
        node.enqueue_order = self.next_enqueue_order;
        self.next_enqueue_order += 1;
        self.nodes.push(node);
    }

    /// Rebase before counter overflow while preserving the chronological order
    /// of every queued generation, including stale entries not yet drained.
    fn rebase_enqueue_order(&mut self) {
        let mut nodes = self.nodes.drain().collect::<Vec<_>>();
        nodes.sort_by_key(|node| node.enqueue_order);
        for (order, node) in nodes.iter_mut().enumerate() {
            node.enqueue_order = u64::try_from(order)
                .expect("resident A* open set must fit the enqueue-order counter");
        }
        self.next_enqueue_order = u64::try_from(nodes.len())
            .expect("resident A* open set must fit the enqueue-order counter");
        self.nodes = BinaryHeap::from(nodes);
    }

    pub(crate) fn pop_live(&mut self, mut is_live: impl FnMut(&AStarNode) -> bool) -> Option<AStarNode> {
        while let Some(node) = self.nodes.pop() {
            if is_live(&node) {
                return Some(node);
            }
        }
        None
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.nodes.len()
    }
}
