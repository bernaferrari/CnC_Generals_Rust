//! Path implementation matching C++ Path class
//!
//! This class encapsulates a "path" returned by the Pathfinder.
//! Includes path optimization, caching, and serialization support.
#![allow(missing_docs, unused_variables, unused_mut)]

use super::{PathNode, PathfindLayerEnum, PATHFIND_CELL_SIZE_F};
use crate::common::{Coord3D, CoordOrigin, ObjectID};
use crate::path::{LocomotorSet, LocomotorSurfaceTypeMask};
use std::ptr::{self, NonNull};

/// Query interface used to validate line-of-sight and collision checks during path optimization.
pub trait PassabilityQuery {
    fn is_line_passable(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        from: &Coord3D,
        to: &Coord3D,
        blocked: bool,
    ) -> bool;

    fn is_ground_line_passable(
        &self,
        _crusher: bool,
        _diameter: i32,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        self.is_line_passable(u32::MAX, from, to, false)
    }
}

/// Information about the closest point on path
#[derive(Debug, Clone)]
pub struct ClosestPointOnPathInfo {
    pub dist_along_path: f32,
    pub pos_on_path: Coord3D,
    pub layer: PathfindLayerEnum,
}

impl Default for ClosestPointOnPathInfo {
    fn default() -> Self {
        Self {
            dist_along_path: 0.0,
            pos_on_path: Coord3D::origin(),
            layer: PathfindLayerEnum::Invalid,
        }
    }
}

/// Path class matching C++ Path class
#[derive(Debug)]
pub struct Path {
    // Path structure
    path: Option<Box<PathNode>>, // The list of PathNode objects that define the path
    path_tail: *mut PathNode,    // Tail of the path for efficient appending

    // Path state
    is_optimized: bool,    // True if the path has been optimized
    blocked_by_ally: bool, // An ally needs to move off of this path

    // Caching info for compute_point_on_path (matching C++ implementation)
    cpop_valid: bool,                   // Is cached info valid?
    cpop_countdown: i32,                // We only return the same cpop MAX_CPOP times
    cpop_in: Coord3D,                   // Input position for cached lookup
    cpop_out: ClosestPointOnPathInfo,   // Cached output
    cpop_recent_start: *const PathNode, // Recent start node for optimization
}

const MAX_CPOP: i32 = 20; // Max times we will return the cached cpop

impl Path {
    /// Create a new empty path
    pub fn new() -> Self {
        Self {
            path: None,
            path_tail: ptr::null_mut(),
            is_optimized: false,
            blocked_by_ally: false,
            cpop_valid: false,
            cpop_countdown: 0,
            cpop_in: Coord3D::origin(),
            cpop_out: ClosestPointOnPathInfo::default(),
            cpop_recent_start: ptr::null(),
        }
    }

    /// Get first node in path
    pub fn get_first_node(&self) -> Option<&PathNode> {
        self.path.as_ref().map(|node| node.as_ref())
    }

    /// Get mutable first node in path
    pub fn get_first_node_mut(&mut self) -> Option<&mut PathNode> {
        self.path.as_mut().map(|node| node.as_mut())
    }

    /// Get last node in path (unsafe due to raw pointer)
    pub fn get_last_node(&self) -> Option<&PathNode> {
        if self.path_tail.is_null() {
            None
        } else {
            Some(unsafe { &*self.path_tail })
        }
    }

    /// Update the position of the last node
    pub fn update_last_node(&mut self, pos: &Coord3D) {
        if !self.path_tail.is_null() {
            unsafe {
                (*self.path_tail).set_position(pos);
            }
        }
    }

    /// Create a new node at the head of the path
    pub fn prepend_node(&mut self, pos: &Coord3D, layer: PathfindLayerEnum) {
        let mut new_node = Box::new(PathNode::new());
        new_node.set_position(pos);
        new_node.set_layer(layer);

        // Update tail if this is the first node
        if self.path.is_none() {
            self.path_tail = new_node.as_mut() as *mut PathNode;
        }

        self.path = Some(new_node.prepend_to_list(self.path.take()));
        self.cpop_valid = false; // Invalidate cache
    }

    /// Create a new node at the end of the path
    pub fn append_node(&mut self, pos: &Coord3D, layer: PathfindLayerEnum) {
        let mut new_node = Box::new(PathNode::new());
        new_node.set_position(pos);
        new_node.set_layer(layer);

        if self.path.is_none() {
            // First node
            self.path_tail = new_node.as_mut() as *mut PathNode;
            self.path = Some(new_node);
        } else {
            // Append to existing tail
            if !self.path_tail.is_null() {
                let new_ptr = new_node.as_mut() as *mut PathNode;
                unsafe {
                    (*self.path_tail).append(new_node);
                }
                self.path_tail = new_ptr;
            }
        }
        self.cpop_valid = false; // Invalidate cache
    }

    /// Set blocked by ally status
    pub fn set_blocked_by_ally(&mut self, blocked: bool) {
        self.blocked_by_ally = blocked;
    }

    /// Get blocked by ally status
    pub fn get_blocked_by_ally(&self) -> bool {
        self.blocked_by_ally
    }

    /// Mark path as optimized
    pub fn mark_optimized(&mut self) {
        self.is_optimized = true;
    }

    /// Check if path is optimized
    pub fn is_optimized(&self) -> bool {
        self.is_optimized
    }

    /// Optimize the path to discard redundant nodes
    pub fn optimize(
        &mut self,
        _obj: ObjectID,
        acceptable_surfaces: LocomotorSurfaceTypeMask,
        blocked: bool,
        passability: Option<&dyn PassabilityQuery>,
    ) {
        if self.is_optimized {
            return; // Already optimized
        }

        self.optimize_internal(acceptable_surfaces, blocked, passability);
        self.mark_optimized();
    }

    /// Internal optimization implementation
    pub(crate) fn optimize_internal(
        &mut self,
        acceptable_surfaces: LocomotorSurfaceTypeMask,
        blocked: bool,
        passability: Option<&dyn PassabilityQuery>,
    ) {
        // Surfaces influence how aggressive we can be: fewer supported surfaces means
        // we keep more intermediate samples to avoid skimming over required waypoints.
        let surface_span = acceptable_surfaces.count_ones().max(1) as f32;
        let tolerance_cells =
            (0.65_f32 - 0.06_f32 * (4.0 - surface_span).max(0.0)).clamp(0.35, 0.75);

        let passability_adapter = passability.map(|query| {
            let surfaces = acceptable_surfaces;
            Box::new(move |from: &Coord3D, to: &Coord3D| {
                query.is_line_passable(surfaces, from, to, blocked)
            }) as Box<dyn Fn(&Coord3D, &Coord3D) -> bool>
        });

        self.apply_optimization(
            tolerance_cells,
            3.9,
            blocked,
            passability_adapter.as_deref(),
        );
    }

    /// Optimize ground path specifically for ground units
    pub fn optimize_ground_path(
        &mut self,
        crusher: bool,
        diameter: i32,
        passability: Option<&dyn PassabilityQuery>,
    ) {
        if self.is_optimized {
            return;
        }

        // Ground-specific optimization
        self.optimize_ground_internal(crusher, diameter, passability);
        self.mark_optimized();
    }

    /// Internal ground path optimization
    pub(crate) fn optimize_ground_internal(
        &mut self,
        crusher: bool,
        diameter: i32,
        passability: Option<&dyn PassabilityQuery>,
    ) {
        let diameter_cells = (diameter.max(1) as f32) / PATHFIND_CELL_SIZE_F.max(1.0);
        let base_tolerance = if crusher { 0.72 } else { 0.58 };
        let tolerance_cells = (base_tolerance - diameter_cells * 0.05).clamp(0.3, base_tolerance);

        let passability_adapter = passability.map(|query| {
            Box::new(move |from: &Coord3D, to: &Coord3D| {
                query.is_ground_line_passable(crusher, diameter, from, to)
            }) as Box<dyn Fn(&Coord3D, &Coord3D) -> bool>
        });

        self.apply_optimization(tolerance_cells, 3.9, false, passability_adapter.as_deref());
    }

    fn head_ptr(&self) -> Option<NonNull<PathNode>> {
        self.path.as_ref().map(|node| unsafe {
            NonNull::new_unchecked(node.as_ref() as *const PathNode as *mut PathNode)
        })
    }

    fn collect_node_ptrs(&self) -> Vec<NonNull<PathNode>> {
        let mut nodes = Vec::new();
        let mut cursor = self.head_ptr();
        while let Some(ptr) = cursor {
            nodes.push(ptr);
            unsafe {
                cursor = ptr.as_ref().next_ptr();
            }
        }
        nodes
    }

    fn apply_optimization(
        &mut self,
        tolerance_cells: f32,
        jiggle_threshold_multiplier: f32,
        blocked: bool,
        passability: Option<&dyn Fn(&Coord3D, &Coord3D) -> bool>,
    ) {
        let mut node_ptrs = self.collect_node_ptrs();
        let node_count = node_ptrs.len();
        if node_count <= 1 {
            return;
        }

        self.cpop_valid = false;
        self.cpop_countdown = 0;
        self.cpop_recent_start = ptr::null();

        // Reset optimized chain
        for mut ptr in node_ptrs.iter().copied() {
            unsafe {
                ptr.as_mut().set_next_optimized(None);
            }
        }

        if blocked {
            for idx in 0..(node_count - 1) {
                let node = unsafe { node_ptrs[idx].as_mut() };
                node.set_next_optimized(Some(node_ptrs[idx + 1]));
            }
            return;
        }

        let tolerance = (PATHFIND_CELL_SIZE_F * tolerance_cells.max(0.05)).max(f32::EPSILON.sqrt());

        let mut anchor_idx = 0;
        while anchor_idx < node_count - 1 {
            let mut anchor_ptr = node_ptrs[anchor_idx];
            let mut best: Option<(usize, NonNull<PathNode>)> = None;

            let mut candidate_idx = node_count - 1;
            while candidate_idx > anchor_idx {
                if self.segment_within_tolerance(
                    &node_ptrs,
                    anchor_idx,
                    candidate_idx,
                    tolerance,
                    passability,
                ) {
                    best = Some((candidate_idx, node_ptrs[candidate_idx]));
                    break;
                }
                candidate_idx -= 1;
            }

            if let Some((next_idx, next_ptr)) = best {
                unsafe {
                    anchor_ptr.as_mut().set_next_optimized(Some(next_ptr));
                }
                anchor_idx = next_idx;
            } else if let Some(next_ptr) = unsafe { anchor_ptr.as_ref().next_ptr() } {
                unsafe {
                    anchor_ptr.as_mut().set_next_optimized(Some(next_ptr));
                }
                anchor_idx += 1;
            } else {
                unsafe {
                    anchor_ptr.as_mut().set_next_optimized(None);
                }
                break;
            }
        }

        self.prune_small_jogs(jiggle_threshold_multiplier);
    }

    fn segment_within_tolerance(
        &self,
        nodes: &[NonNull<PathNode>],
        anchor_idx: usize,
        candidate_idx: usize,
        tolerance: f32,
        passability: Option<&dyn Fn(&Coord3D, &Coord3D) -> bool>,
    ) -> bool {
        if candidate_idx <= anchor_idx {
            return false;
        }

        let anchor = unsafe { nodes[anchor_idx].as_ref() };
        let candidate = unsafe { nodes[candidate_idx].as_ref() };
        let a_pos = anchor.get_position();
        let b_pos = candidate.get_position();

        let dx = b_pos.x - a_pos.x;
        let dy = b_pos.y - a_pos.y;
        let segment_len_sq = dx * dx + dy * dy;
        if segment_len_sq < f32::EPSILON {
            return false;
        }

        for idx in (anchor_idx + 1)..candidate_idx {
            let node = unsafe { nodes[idx].as_ref() };
            let p_pos = node.get_position();

            let distance = Self::distance_point_to_segment(
                p_pos.x, p_pos.y, a_pos.x, a_pos.y, b_pos.x, b_pos.y,
            );
            if distance > tolerance {
                return false;
            }

            let proj = ((p_pos.x - a_pos.x) * dx + (p_pos.y - a_pos.y) * dy) / segment_len_sq;
            if proj < -0.05 || proj > 1.05 {
                return false;
            }
        }

        if let Some(check) = passability {
            return check(a_pos, b_pos);
        }

        true
    }

    pub fn smooth(&mut self, smoothing_factor: f32) {
        self.smooth_path_internal(smoothing_factor);
        self.cpop_valid = false;
    }

    fn smooth_path_internal(&mut self, smoothing_factor: f32) {
        let factor = smoothing_factor.clamp(0.0, 1.0);
        if factor <= f32::EPSILON {
            return;
        }

        let node_ptrs = self.collect_node_ptrs();
        if node_ptrs.len() <= 2 {
            return;
        }

        let original_positions: Vec<Coord3D> = node_ptrs
            .iter()
            .map(|ptr| unsafe { ptr.as_ref().get_position().clone() })
            .collect();

        for idx in 1..(node_ptrs.len() - 1) {
            let prev = original_positions[idx - 1];
            let current = original_positions[idx];
            let next = original_positions[idx + 1];

            let blended = Coord3D::new(
                current.x + (prev.x + next.x - 2.0 * current.x) * factor * 0.5,
                current.y + (prev.y + next.y - 2.0 * current.y) * factor * 0.5,
                current.z + (prev.z + next.z - 2.0 * current.z) * factor * 0.25,
            );

            let mut ptr = node_ptrs[idx];
            unsafe {
                ptr.as_mut().set_position(&blended);
            }
        }
    }

    fn distance_point_to_segment(px: f32, py: f32, ax: f32, ay: f32, bx: f32, by: f32) -> f32 {
        let abx = bx - ax;
        let aby = by - ay;
        let ab_len_sq = abx * abx + aby * aby;
        if ab_len_sq <= f32::EPSILON {
            let dx = px - ax;
            let dy = py - ay;
            return (dx * dx + dy * dy).sqrt();
        }

        let t = ((px - ax) * abx + (py - ay) * aby) / ab_len_sq;
        let t_clamped = t.clamp(0.0, 1.0);
        let cx = ax + abx * t_clamped;
        let cy = ay + aby * t_clamped;
        let dx = px - cx;
        let dy = py - cy;
        (dx * dx + dy * dy).sqrt()
    }

    fn prune_small_jogs(&mut self, jiggle_threshold_multiplier: f32) {
        let threshold_sq =
            PATHFIND_CELL_SIZE_F * PATHFIND_CELL_SIZE_F * jiggle_threshold_multiplier;
        let mut anchor_ptr = self.head_ptr();

        while let Some(mut anchor) = anchor_ptr {
            let next_ptr = unsafe { anchor.as_ref().next_optimized_ptr() };
            let Some(next_ptr) = next_ptr else {
                break;
            };

            if let Some(skip_ptr) = unsafe { next_ptr.as_ref().next_optimized_ptr() } {
                let anchor_pos = unsafe { anchor.as_ref().get_position() };
                let skip_pos = unsafe { skip_ptr.as_ref().get_position() };
                let dx = skip_pos.x - anchor_pos.x;
                let dy = skip_pos.y - anchor_pos.y;
                if dx * dx + dy * dy <= threshold_sq {
                    unsafe {
                        anchor.as_mut().set_next_optimized(Some(skip_ptr));
                    }
                    continue;
                }
            }

            anchor_ptr = Some(next_ptr);
        }
    }

    /// Compute point on path closest to given position
    pub fn compute_point_on_path(
        &mut self,
        obj: ObjectID,
        locomotor_set: &LocomotorSet,
        pos: &Coord3D,
    ) -> ClosestPointOnPathInfo {
        // Check cache first
        if self.cpop_valid && self.cpop_countdown > 0 {
            let dist_squared = (pos.x - self.cpop_in.x) * (pos.x - self.cpop_in.x)
                + (pos.y - self.cpop_in.y) * (pos.y - self.cpop_in.y)
                + (pos.z - self.cpop_in.z) * (pos.z - self.cpop_in.z);

            // If position hasn't changed much, return cached result
            if dist_squared < 1.0 {
                self.cpop_countdown -= 1;
                return self.cpop_out.clone();
            }
        }

        // Compute new closest point
        let result = self.compute_closest_point_internal(pos, locomotor_set);

        // Cache the result
        self.cpop_valid = true;
        self.cpop_countdown = MAX_CPOP;
        self.cpop_in = *pos;
        self.cpop_out = result.clone();

        result
    }

    /// Internal closest point computation
    fn compute_closest_point_internal(
        &self,
        pos: &Coord3D,
        _locomotor_set: &LocomotorSet,
    ) -> ClosestPointOnPathInfo {
        let mut closest_info = ClosestPointOnPathInfo::default();
        let mut closest_dist_sq = f32::MAX;
        let mut dist_along_path = 0.0;

        if let Some(ref first_node) = self.path {
            let mut current = first_node.as_ref();

            loop {
                // Check distance to this node
                let dx = pos.x - current.get_position().x;
                let dy = pos.y - current.get_position().y;
                let dz = pos.z - current.get_position().z;
                let dist_sq = dx * dx + dy * dy + dz * dz;

                if dist_sq < closest_dist_sq {
                    closest_dist_sq = dist_sq;
                    closest_info.pos_on_path = *current.get_position();
                    closest_info.layer = current.get_layer();
                    closest_info.dist_along_path = dist_along_path;
                }

                // Move to next node
                if let Some(next) = current.get_next() {
                    let segment_length = {
                        let dx = next.get_position().x - current.get_position().x;
                        let dy = next.get_position().y - current.get_position().y;
                        let dz = next.get_position().z - current.get_position().z;
                        (dx * dx + dy * dy + dz * dz).sqrt()
                    };
                    dist_along_path += segment_length;
                    current = next;
                } else {
                    break;
                }
            }
        }

        closest_info
    }

    /// Peek cached point on path
    pub fn peek_cached_point_on_path(&self) -> Coord3D {
        if self.cpop_valid {
            self.cpop_out.pos_on_path
        } else {
            Coord3D::origin()
        }
    }

    /// Compute flight distance to goal for aircraft
    pub fn compute_flight_dist_to_goal(&self, pos: &Coord3D) -> (f32, Coord3D) {
        if let Some(ref path) = self.path {
            if !self.path_tail.is_null() {
                unsafe {
                    let goal_pos = *(*self.path_tail).get_position();
                    let dx = goal_pos.x - pos.x;
                    let dy = goal_pos.y - pos.y;
                    let dz = goal_pos.z - pos.z;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    (dist.max(0.0), goal_pos)
                }
            } else {
                (0.0, Coord3D::origin())
            }
        } else {
            (0.0, Coord3D::origin())
        }
    }

    /// Check if path is empty
    pub fn is_empty(&self) -> bool {
        self.path.is_none()
    }

    /// Get path length (number of nodes)
    pub fn get_length(&self) -> usize {
        let mut count = 0;
        if let Some(ref first) = self.path {
            let mut current = first.as_ref();
            loop {
                count += 1;
                if let Some(next) = current.get_next() {
                    current = next;
                } else {
                    break;
                }
            }
        }
        count
    }

    /// Get total path distance
    pub fn get_total_distance(&self) -> f32 {
        let mut total = 0.0;
        if let Some(ref first) = self.path {
            let mut current = first.as_ref();

            while let Some(next) = current.get_next() {
                let dx = next.get_position().x - current.get_position().x;
                let dy = next.get_position().y - current.get_position().y;
                let dz = next.get_position().z - current.get_position().z;
                total += (dx * dx + dy * dy + dz * dz).sqrt();
                current = next;
            }
        }
        total
    }

    /// Collect the world positions for every node in the path.
    pub fn positions(&self) -> Vec<Coord3D> {
        let mut points = Vec::new();
        if let Some(ref first) = self.path {
            let mut current = first.as_ref();
            loop {
                points.push(*current.get_position());
                if let Some(next) = current.get_next() {
                    current = next;
                } else {
                    break;
                }
            }
        }
        points
    }

    /// Clear the entire path
    pub fn clear(&mut self) {
        self.path = None;
        self.path_tail = ptr::null_mut();
        self.is_optimized = false;
        self.blocked_by_ally = false;
        self.cpop_valid = false;
        self.cpop_countdown = 0;
        self.cpop_recent_start = ptr::null();
    }
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for Path {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::path::{CollisionMap, ICoord2D, IRegion2D, SURFACE_GROUND};

    #[test]
    fn test_path_creation() {
        let path = Path::new();
        assert!(path.is_empty());
        assert!(!path.is_optimized());
        assert!(!path.get_blocked_by_ally());
        assert_eq!(path.get_length(), 0);
    }

    #[test]
    fn test_path_node_operations() {
        let mut path = Path::new();

        // Test prepend
        path.prepend_node(&Coord3D::new(10.0, 10.0, 0.0), PathfindLayerEnum::Ground);
        assert!(!path.is_empty());
        assert_eq!(path.get_length(), 1);

        // Test append
        path.append_node(&Coord3D::new(20.0, 20.0, 0.0), PathfindLayerEnum::Ground);
        assert_eq!(path.get_length(), 2);

        // Test first node
        if let Some(first) = path.get_first_node() {
            assert_eq!(first.get_position().x, 10.0);
        } else {
            panic!("Should have first node");
        }
    }

    #[test]
    fn test_path_optimization() {
        let mut path = Path::new();
        path.append_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 10.0, 0.0), PathfindLayerEnum::Ground);

        assert!(!path.is_optimized());
        path.optimize(1, SURFACE_GROUND, false, None);
        assert!(path.is_optimized());
    }

    #[test]
    fn test_path_optimization_skips_colinear_nodes() {
        let mut path = Path::new();
        path.append_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground);

        path.optimize(1, SURFACE_GROUND, false, None);

        let first = path.get_first_node().expect("first node");
        let (next_opt, _, _) = first.get_next_optimized();
        let next = next_opt.expect("optimized successor");
        assert_eq!(next.get_position().x, 20.0);
    }

    #[test]
    fn test_path_optimization_respects_blocked_flag() {
        let mut path = Path::new();
        path.append_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground);

        path.optimize(1, SURFACE_GROUND, true, None);

        let first = path.get_first_node().expect("first node");
        let (next_opt, _, _) = first.get_next_optimized();
        let next = next_opt.expect("blocked successor");
        assert_eq!(next.get_position().x, 10.0);
    }

    #[test]
    fn test_path_optimization_respects_collision_map() {
        let mut collision = CollisionMap::new();
        let bounds = IRegion2D {
            lo: ICoord2D::new(0, 0),
            hi: ICoord2D::new(4, 4),
        };
        collision.initialize(&bounds);

        let obstacle_bounds = IRegion2D {
            lo: ICoord2D::new(1, 0),
            hi: ICoord2D::new(1, 0),
        };
        collision.add_static_obstacle(99, &obstacle_bounds);

        let mut path = Path::new();
        path.append_node(&Coord3D::new(5.0, 5.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(15.0, 5.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(25.0, 5.0, 0.0), PathfindLayerEnum::Ground);

        path.optimize(1, SURFACE_GROUND, false, Some(&collision));

        let first = path.get_first_node().expect("first node");
        let (next_opt, _, _) = first.get_next_optimized();
        let next = next_opt.expect("collision-aware successor");
        assert_eq!(next.get_position().x, 15.0);
    }

    #[test]
    fn test_path_distance() {
        let mut path = Path::new();
        path.append_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 10.0, 0.0), PathfindLayerEnum::Ground);

        let total_dist = path.get_total_distance();
        assert!((total_dist - 20.0).abs() < 0.001); // 10 + 10 = 20
    }

    #[test]
    fn test_path_blocked_by_ally() {
        let mut path = Path::new();
        assert!(!path.get_blocked_by_ally());

        path.set_blocked_by_ally(true);
        assert!(path.get_blocked_by_ally());

        path.set_blocked_by_ally(false);
        assert!(!path.get_blocked_by_ally());
    }

    #[test]
    fn test_path_clear() {
        let mut path = Path::new();
        path.append_node(&Coord3D::new(10.0, 10.0, 0.0), PathfindLayerEnum::Ground);
        assert!(!path.is_empty());

        path.clear();
        assert!(path.is_empty());
        assert_eq!(path.get_length(), 0);
        assert!(!path.is_optimized());
    }
}
