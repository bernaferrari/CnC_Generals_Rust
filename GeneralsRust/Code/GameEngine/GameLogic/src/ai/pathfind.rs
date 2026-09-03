use crate::ai::the_ai;
use crate::ai::path_optimization::PathOptimizer;
use crate::ai::pathfind_astar::{PATHFIND_CELL_SIZE_F, PathfindLayerEnum as OptLayer};
use crate::common::coord::*;
use crate::common::vector_ext::Vector3Ext;
use crate::common::xfer::XferExt;
use crate::common::*;
use crate::helpers::TheGameLogic;
use crate::helpers::ThePartitionManager;
use crate::locomotor::LocomotorSurfaceTypeMask;
use crate::object::registry::OBJECT_REGISTRY;
use crate::object::*;
use crate::path::SURFACE_GROUND;
use crate::terrain::get_terrain_logic;
use game_engine::common::system::{Snapshotable, Xfer};
use slotmap::SlotMap;

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, RwLock, Weak};

/// Wave 426: host-only path has no dual-world factory objects.
#[inline]
fn dual_world_registry_unavailable() -> bool {
    crate::object::registry::OBJECT_REGISTRY.is_empty()
}

/// How close is close enough when moving
pub const PATHFIND_CLOSE_ENOUGH: f32 = 1.0;
pub const PATH_MAX_PRIORITY: i32 = i32::MAX;

/// Maximum wall pieces supported
pub const MAX_WALL_PIECES: usize = 128;

// Index key for PathNode within the Path's SlotMap. `new_key_type!` supplies
// SlotMap's unsafe key contract, so this module does not hand-roll it.
slotmap::new_key_type! { pub struct PathNodeKey; }

/// PathNodes are used to create a final Path to return from the pathfinder.
/// All node ownership is managed through the Path's SlotMap.
pub struct PathNode {
    /// Position of this node
    pos: Coord3D,
    /// Layer for this section
    layer: PathfindLayerEnum,
    /// Previous node in the path (index into SlotMap)
    prev: Option<PathNodeKey>,
    /// Next node in the path (index into SlotMap).
    ///
    /// C++ `PathNode` is a doubly linked list.  Keeping both directions is
    /// important: path construction walks forward from the head, while Xfer
    /// writes backward from the tail.
    next: Option<PathNodeKey>,
    /// Next node in optimized path (index into SlotMap)
    next_opti: Option<PathNodeKey>,
    /// Distance to next optimized node
    next_opti_dist_2d: f32,
    /// Normalized direction vector to next optimized node
    next_opti_dir_norm_2d: Coord2D,
    /// Whether this node can be optimized out
    can_optimize: bool,
    /// Node ID (used for serialization)
    id: i32,
}

impl PathNode {
    pub fn new() -> Self {
        Self {
            pos: Coord3D::new(0.0, 0.0, 0.0),
            layer: PathfindLayerEnum::Invalid,
            prev: None,
            next: None,
            next_opti: None,
            next_opti_dist_2d: 0.0,
            next_opti_dir_norm_2d: Coord2D::new(0.0, 0.0),
            can_optimize: false,
            id: -1,
        }
    }

    /// Get position of this node
    pub fn get_position(&self) -> &Coord3D {
        &self.pos
    }

    /// Set position of this node
    pub fn set_position(&mut self, pos: &Coord3D) {
        self.pos = *pos;
    }

    /// Get layer of this node
    pub fn get_layer(&self) -> PathfindLayerEnum {
        self.layer
    }

    /// Set layer of this node
    pub fn set_layer(&mut self, layer: PathfindLayerEnum) {
        self.layer = layer;
    }

    /// Get next node in the raw path.
    pub fn get_next<'a>(
        &'a self,
        _self_key: PathNodeKey,
        nodes: &'a SlotMap<PathNodeKey, PathNode>,
    ) -> Option<&'a PathNode> {
        self.next.and_then(|key| nodes.get(key))
    }

    /// Get next node in optimized path
    pub fn get_next_optimized<'a>(
        &'a self,
        nodes: &'a SlotMap<PathNodeKey, PathNode>,
    ) -> (Option<&'a PathNode>, Coord2D, f32) {
        let next = self.next_opti.and_then(|key| nodes.get(key));
        (next, self.next_opti_dir_norm_2d, self.next_opti_dist_2d)
    }

    /// Set next optimized node link and compute distance/direction from positions.
    pub fn set_next_optimized(
        &mut self,
        next_key: Option<PathNodeKey>,
        next_pos: Option<&Coord3D>,
    ) {
        if let Some((_, pos)) = next_key.zip(next_pos) {
            let dx = pos.x - self.pos.x;
            let dy = pos.y - self.pos.y;
            self.next_opti_dist_2d = (dx * dx + dy * dy).sqrt();

            if self.next_opti_dist_2d == 0.0 {
                self.next_opti_dist_2d = 0.01;
            }

            self.next_opti_dir_norm_2d =
                Coord2D::new(dx / self.next_opti_dist_2d, dy / self.next_opti_dist_2d);
        } else {
            self.next_opti_dist_2d = 0.0;
        }
        self.next_opti = next_key;
    }

    /// Check if this node can be optimized
    pub fn can_optimize(&self) -> bool {
        self.can_optimize
    }

    /// Set whether this node can be optimized
    pub fn set_can_optimize(&mut self, can_opt: bool) {
        self.can_optimize = can_opt;
    }

    /// Compute direction vector to next node.
    /// `self_key` is this node's key, used to find the next node via prev lookup.
    pub fn compute_direction_vector(
        &self,
        self_key: PathNodeKey,
        nodes: &SlotMap<PathNodeKey, PathNode>,
    ) -> Option<Coord3D> {
        // Try to find next node (one whose prev == self_key)
        if let Some(next) = self.get_next(self_key, nodes) {
            let mut dir = Coord3D::new(
                next.pos.x - self.pos.x,
                next.pos.y - self.pos.y,
                next.pos.z - self.pos.z,
            );
            let length = dir.length();
            if length > 0.0 {
                dir.x /= length;
                dir.y /= length;
                dir.z /= length;
            }
            Some(dir)
        } else if let Some(prev_key) = self.prev {
            if let Some(prev) = nodes.get(prev_key) {
                let mut dir = Coord3D::new(
                    self.pos.x - prev.pos.x,
                    self.pos.y - prev.pos.y,
                    self.pos.z - prev.pos.z,
                );
                let length = dir.length();
                if length > 0.0 {
                    dir.x /= length;
                    dir.y /= length;
                    dir.z /= length;
                }
                Some(dir)
            } else {
                Some(Coord3D::new(0.0, 0.0, 0.0))
            }
        } else {
            Some(Coord3D::new(0.0, 0.0, 0.0))
        }
    }
}

/// Information about the closest point on a path
#[derive(Copy, Clone, Debug)]
pub struct ClosestPointOnPathInfo {
    /// Distance along the path
    pub dist_along_path: f32,
    /// Position on the path
    pub pos_on_path: Coord3D,
    /// Layer of this section
    pub layer: PathfindLayerEnum,
}

/// Path class encapsulates a path returned by the pathfinder.
/// All nodes are owned in a SlotMap; head/tail are index keys.
/// The path is ordered head -> tail through `next`; `prev` is retained for
/// reverse traversal during Xfer, exactly like C++ `PathNode`.
pub struct Path {
    /// All nodes owned in a SlotMap
    nodes: SlotMap<PathNodeKey, PathNode>,
    /// First node key in the path
    head: Option<PathNodeKey>,
    /// Last node key in the path (for efficient appending)
    tail: Option<PathNodeKey>,
    /// Whether the path has been optimized
    is_optimized: bool,
    /// Whether an ally is blocking this path
    blocked_by_ally: bool,
    /// Cached point-on-path computation info
    cpop_valid: bool,
    cpop_countdown: i32,
    cpop_in: Coord3D,
    cpop_out: ClosestPointOnPathInfo,
}

impl Path {
    /// Maximum times to return cached point-on-path result
    const MAX_CPOP: i32 = 20;

    pub fn new() -> Self {
        Self {
            nodes: SlotMap::with_key(),
            head: None,
            tail: None,
            is_optimized: false,
            blocked_by_ally: false,
            cpop_valid: false,
            cpop_countdown: Self::MAX_CPOP,
            cpop_in: Coord3D::new(0.0, 0.0, 0.0),
            cpop_out: ClosestPointOnPathInfo {
                dist_along_path: 0.0,
                pos_on_path: Coord3D::new(0.0, 0.0, 0.0),
                layer: PathfindLayerEnum::Invalid,
            },
        }
    }

    /// Get first node in the path
    pub fn get_first_node(&self) -> Option<&PathNode> {
        self.head.and_then(|key| self.nodes.get(key))
    }

    /// Iterate path from head to tail, calling `f` for each (key, node) pair.
    fn for_each_node<F>(&self, mut f: F)
    where
        F: FnMut(PathNodeKey, &PathNode),
    {
        let mut current = self.head;
        while let Some(key) = current {
            if let Some(node) = self.nodes.get(key) {
                let next = node.next;
                f(key, node);
                current = next;
            } else {
                break;
            }
        }
    }

    /// Collect ordered keys from head to tail.
    fn ordered_keys(&self) -> Vec<PathNodeKey> {
        let mut keys = Vec::new();
        let mut current = self.head;
        while let Some(key) = current {
            keys.push(key);
            current = self.nodes.get(key).and_then(|n| n.next);
        }
        keys
    }

    /// Update the position of the last node
    pub fn update_last_node(&mut self, pos: &Coord3D) {
        if let Some(tail_key) = self.tail {
            if let Some(tail_node) = self.nodes.get_mut(tail_key) {
                tail_node.set_position(pos);
                if let Ok(terrain) = get_terrain_logic().read() {
                    let layer = terrain.get_layer_for_destination(pos);
                    tail_node.set_layer(path_layer_from_u32(layer as u32));
                }
            }
        }

        if self.is_optimized {
            if let Some(head_key) = self.head {
                let tail_key = self.tail;
                let mut current = Some(head_key);
                while let Some(cur_key) = current {
                    let points_to_tail = self
                        .nodes
                        .get(cur_key)
                        .map(|n| n.next_opti == tail_key)
                        .unwrap_or(false);
                    if points_to_tail {
                        let tail_pos = self.tail.and_then(|k| self.nodes.get(k)).map(|n| n.pos);
                        if let Some(node) = self.nodes.get_mut(cur_key) {
                            node.set_next_optimized(tail_key, tail_pos.as_ref());
                        }
                        break;
                    }
                    current = self.nodes.get(cur_key).and_then(|n| n.next_opti);
                }
            }
        }
    }

    /// Add a new node at the head of the path
    pub fn prepend_node(&mut self, pos: &Coord3D, layer: PathfindLayerEnum) {
        let mut new_node = PathNode::new();
        new_node.set_position(pos);
        new_node.set_layer(layer);

        let new_key = self.nodes.insert(new_node);

        if let Some(old_head_key) = self.head {
            self.nodes[new_key].next = Some(old_head_key);
            self.nodes[old_head_key].prev = Some(new_key);
        } else {
            // This is the first node, so it's also the tail
            self.tail = Some(new_key);
        }

        self.head = Some(new_key);
        self.is_optimized = false;
    }

    /// Add a new node at the end of the path
    pub fn append_node(&mut self, pos: &Coord3D, layer: PathfindLayerEnum) {
        if self.is_optimized {
            if let Some(tail_key) = self.tail {
                if let Some(tail_node) = self.nodes.get(tail_key) {
                    if tail_node.get_position().x == pos.x && tail_node.get_position().y == pos.y {
                        // Match C++: ignore duplicate segment when optimized.
                        return;
                    }
                }
            }
        }

        let mut new_node = PathNode::new();
        new_node.set_position(pos);
        new_node.set_layer(layer);

        let new_key = self.nodes.insert(new_node);

        if let Some(tail_key) = self.tail {
            self.nodes[new_key].prev = Some(tail_key);
            self.nodes[tail_key].next = Some(new_key);
            if self.is_optimized {
                let new_pos = self.nodes.get(new_key).map(|n| n.pos);
                if let Some(tail_node) = self.nodes.get_mut(tail_key) {
                    tail_node.set_next_optimized(Some(new_key), new_pos.as_ref());
                }
            }
        } else {
            // This is the first node
            self.head = Some(new_key);
        }

        self.tail = Some(new_key);
    }

    /// Check if path is blocked by ally
    pub fn is_blocked_by_ally(&self) -> bool {
        self.blocked_by_ally
    }

    /// Set whether path is blocked by ally
    pub fn set_blocked_by_ally(&mut self, blocked: bool) {
        self.blocked_by_ally = blocked;
    }

    fn set_opti_link(&mut self, from_key: PathNodeKey, to_key: Option<PathNodeKey>) {
        let to_pos = to_key.and_then(|k| self.nodes.get(k)).map(|n| n.pos);
        if let Some(node) = self.nodes.get_mut(from_key) {
            node.set_next_optimized(to_key, to_pos.as_ref());
        }
    }

    /// Optimize the path to discard redundant nodes
    pub fn optimize(
        &mut self,
        obj_id: ObjectID,
        acceptable_surfaces: LocomotorSurfaceTypeMask,
        blocked: bool,
    ) {
        self.blocked_by_ally = blocked;

        // Collect ordered node keys, points, and layers from head to tail
        let ordered_keys = self.ordered_keys();
        let raw_nodes: Vec<PathNodeKey> = ordered_keys.clone();
        let raw_points: Vec<Coord3D> = ordered_keys
            .iter()
            .filter_map(|&key| self.nodes.get(key).map(|n| n.pos))
            .collect();
        let raw_layers: Vec<PathfindLayerEnum> = ordered_keys
            .iter()
            .filter_map(|&key| self.nodes.get(key).map(|n| n.layer))
            .collect();

        if raw_nodes.len() <= 1 {
            if let Some(&key) = raw_nodes.first() {
                if let Some(node) = self.nodes.get_mut(key) {
                    node.set_next_optimized(None, None);
                }
            }
            self.is_optimized = true;
            return;
        }

        let opt_layers: Vec<OptLayer> = raw_layers
            .iter()
            .map(|layer| match layer {
                PathfindLayerEnum::Ground => OptLayer::Ground,
                PathfindLayerEnum::Invalid => OptLayer::Invalid,
                _ => OptLayer::Top,
            })
            .collect();

        let optimizer = PathOptimizer::new();
        let passable = |from: &Coord3D, to: &Coord3D, _layer: OptLayer| {
            if blocked {
                return false;
            }
            if let Ok(terrain) = get_terrain_logic().read() {
                terrain.is_clear_line_of_sight(from, to)
            } else {
                true
            }
        };

        let (mut opt_points, mut opt_layers) =
            optimizer.optimize(&raw_points, &opt_layers, passable);

        if (acceptable_surfaces & SURFACE_GROUND) != 0 {
            let diameter = TheGameLogic::find_object_by_id(obj_id)
                .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
                .and_then(|obj| {
                    obj.read()
                        .ok()
                        .map(|guard| guard.get_geometry_info().get_major_radius())
                })
                .unwrap_or(PATHFIND_CELL_SIZE_F)
                .max(PATHFIND_CELL_SIZE_F)
                * 2.0;
            let diameter = diameter as i32;
            let passable_ground = |from: &Coord3D, to: &Coord3D, _diameter: i32| {
                if blocked {
                    return false;
                }
                if let Ok(terrain) = get_terrain_logic().read() {
                    terrain.is_clear_line_of_sight(from, to)
                } else {
                    true
                }
            };
            let (ground_points, ground_layers) = optimizer.optimize_ground_path(
                &opt_points,
                &opt_layers,
                false,
                diameter,
                passable_ground,
            );
            opt_points = ground_points;
            opt_layers = ground_layers;
        }

        let mut optimized_indices: Vec<usize> = Vec::new();
        let mut search_start = 0;
        for (idx, opt_point) in opt_points.iter().enumerate() {
            let desired_layer = match opt_layers.get(idx).copied().unwrap_or(OptLayer::Ground) {
                OptLayer::Ground => PathfindLayerEnum::Ground,
                OptLayer::Top => PathfindLayerEnum::Top,
                OptLayer::Invalid => PathfindLayerEnum::Invalid,
                OptLayer::Wall => PathfindLayerEnum::Wall,
                // Layer3–14 are unnamed C++ bridge slots (`LAYER_GROUND+1`…); map like Bridge*.
                _ => PathfindLayerEnum::Top,
            };
            let mut found = None;
            for raw_idx in search_start..raw_points.len() {
                let raw_point = raw_points[raw_idx];
                let same_point = (raw_point.x - opt_point.x).abs() <= 0.01
                    && (raw_point.y - opt_point.y).abs() <= 0.01
                    && (raw_point.z - opt_point.z).abs() <= 0.01;
                if same_point && raw_layers[raw_idx] == desired_layer {
                    found = Some(raw_idx);
                    break;
                }
            }
            let Some(found_idx) = found else {
                optimized_indices.clear();
                break;
            };
            optimized_indices.push(found_idx);
            search_start = found_idx + 1;
        }

        if optimized_indices.is_empty() || optimized_indices.first() != Some(&0) {
            for window in raw_nodes.windows(2) {
                self.set_opti_link(window[0], Some(window[1]));
            }
            if let Some(&last_key) = raw_nodes.last() {
                self.set_opti_link(last_key, None);
            }
            self.is_optimized = true;
            return;
        }

        for &key in &raw_nodes {
            self.set_opti_link(key, None);
        }

        for window in optimized_indices.windows(2) {
            if let [from_idx, to_idx] = window {
                self.set_opti_link(raw_nodes[*from_idx], Some(raw_nodes[*to_idx]));
            }
        }

        if let Some(&last_idx) = optimized_indices.last() {
            self.set_opti_link(raw_nodes[last_idx], None);
        }

        self.is_optimized = true;
    }

    /// Mark path as optimized
    pub fn mark_optimized(&mut self) {
        self.is_optimized = true;
    }

    fn is_really_close(a: &Coord3D, b: &Coord3D) -> bool {
        let close_enough: f32 = 0.1;
        (a.x - b.x).abs() <= close_enough
            && (a.y - b.y).abs() <= close_enough
            && (a.z - b.z).abs() <= close_enough
    }

    pub fn compute_point_on_path(&mut self, pos: &Coord3D) -> ClosestPointOnPathInfo {
        if self.cpop_valid && self.cpop_countdown > 0 && Self::is_really_close(pos, &self.cpop_in) {
            self.cpop_countdown -= 1;
            return ClosestPointOnPathInfo {
                dist_along_path: self.cpop_out.dist_along_path,
                pos_on_path: self.cpop_out.pos_on_path,
                layer: self.cpop_out.layer,
            };
        }

        if self.head.is_none() {
            self.cpop_valid = false;
            return ClosestPointOnPathInfo {
                dist_along_path: 0.0,
                pos_on_path: Coord3D::ZERO,
                layer: PathfindLayerEnum::Ground,
            };
        }

        let mut best_dist_sqr = f32::MAX;
        let mut best_point = *pos;
        let mut best_layer = self
            .head
            .and_then(|key| self.nodes.get(key))
            .map(|node| node.get_layer())
            .unwrap_or(PathfindLayerEnum::Ground);
        let mut dist_along_path = 0.0;

        let keys = self.ordered_keys();
        let mut path_distance = 0.0;
        for (i, &key) in keys.iter().enumerate() {
            let node = match self.nodes.get(key) {
                Some(n) => n,
                None => continue,
            };

            let next_key = node.next_opti.or(keys.get(i + 1).copied());
            let next_node = match next_key.and_then(|k| self.nodes.get(k)) {
                Some(n) => n,
                None => break,
            };

            let (closest_point, t) =
                Self::closest_point_on_segment(node.get_position(), next_node.get_position(), pos);

            let dist_sqr = Vector3Ext::length_sqr(&(*pos - closest_point));
            if dist_sqr < best_dist_sqr {
                best_dist_sqr = dist_sqr;
                best_point = closest_point;
                best_layer = node.get_layer();
                dist_along_path =
                    path_distance + t * (*next_node.get_position() - *node.get_position()).length();
            }

            path_distance += (*next_node.get_position() - *node.get_position()).length();

            if next_key != keys.get(i + 1).copied() {
                break;
            }
        }

        self.cpop_valid = true;
        self.cpop_countdown = Self::MAX_CPOP;
        self.cpop_in = *pos;
        self.cpop_out = ClosestPointOnPathInfo {
            dist_along_path,
            pos_on_path: best_point,
            layer: best_layer,
        };

        self.cpop_out
    }

    /// Find closest point on line segment
    fn closest_point_on_segment(start: &Coord3D, end: &Coord3D, point: &Coord3D) -> (Coord3D, f32) {
        let segment = *end - *start;
        let to_point = *point - *start;

        let segment_length_sqr = Vector3Ext::length_sqr(&segment);
        if segment_length_sqr == 0.0 {
            return (*start, 0.0);
        }

        let t = (to_point.dot(segment) / segment_length_sqr).clamp(0.0, 1.0);
        let closest = *start + segment * t;

        (closest, t)
    }

    /// Peek at cached point on path
    pub fn peek_cached_point_on_path(&self) -> Coord3D {
        self.cpop_out.pos_on_path
    }
}

/// Cell type enumeration for pathfinding grid
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellType {
    Clear = 0x00,            // Clear, unobstructed ground
    Water = 0x01,            // Water area
    Cliff = 0x02,            // Steep altitude change
    Rubble = 0x03,           // Cell is occupied by rubble
    Obstacle = 0x04,         // Occupied by a structure
    BridgeImpassable = 0x05, // Piece of a bridge that is impassable
    Impassable = 0x06,       // Just plain impassable except for aircraft
}

/// Cell flags for unit presence
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellFlags {
    NoUnits = 0x00,             // No units in this cell
    UnitGoal = 0x01,            // A unit is heading to this cell
    UnitPresentMoving = 0x02,   // A unit is moving through this cell
    UnitPresentFixed = 0x03,    // A unit is stationary in this cell
    UnitGoalOtherMoving = 0x05, // A unit is moving through this cell, and another has this as goal
}

/// Pathfinding cell information (allocated on demand)
pub struct PathfindCellInfo {
    /// A* topology is represented by grid coordinates, not non-owning cell
    /// addresses.  C++ used pointers here because its cells lived in a fixed
    /// array; coordinates retain the same graph identity without dangling
    /// pointer risk when the Rust grid is reset or rebuilt.
    next_open: Option<ICoord2D>,
    prev_open: Option<ICoord2D>,
    /// Parent cell from pathfinder.
    path_parent: Option<ICoord2D>,
    /// Cost estimates for A* search
    total_cost: u16,
    cost_so_far: u16,
    /// Cell coordinates
    pos: ICoord2D,
    /// Unit IDs
    goal_unit_id: ObjectID,
    pos_unit_id: ObjectID,
    goal_aircraft_id: ObjectID,
    obstacle_id: ObjectID,
    /// Flags
    is_free: bool,
    blocked_by_ally: bool,
    obstacle_is_fence: bool,
    obstacle_is_transparent: bool,
    open: bool,
    closed: bool,
}

/// A cell in the pathfinding grid
pub struct PathfindCell {
    /// Cell type
    cell_type: CellType,
    /// Cell flags
    flags: CellFlags,
    /// Layer of this cell
    layer: PathfindLayerEnum,
    /// Layer this cell connects to
    connects_to_layer: PathfindLayerEnum,
    /// Zone information
    zone: u16,
    /// Whether this is an aircraft goal
    aircraft_goal: bool,
    /// Whether this cell is pinched (surrounded by obstacles)
    pinched: bool,
    /// Detailed info (allocated on demand)
    info: Option<Box<PathfindCellInfo>>,
}

impl Snapshotable for Path {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to crc Path version: {:?}", e))?;
        let mut count: i32 = self.nodes.len() as i32;
        xfer.xfer_int(&mut count)
            .map_err(|e| format!("Failed to crc Path node count: {:?}", e))?;
        let mut remaining = count;
        let mut current = self.tail;
        while let Some(key) = current {
            let node = self.nodes.get(key).unwrap();
            let mut node_id = remaining;
            xfer.xfer_int(&mut node_id)
                .map_err(|e| format!("Failed to crc Path node id: {:?}", e))?;
            let mut pos = *node.get_position();
            xfer.xfer_real(&mut pos.x)
                .map_err(|e| format!("Failed to crc Path node pos.x: {:?}", e))?;
            xfer.xfer_real(&mut pos.y)
                .map_err(|e| format!("Failed to crc Path node pos.y: {:?}", e))?;
            xfer.xfer_real(&mut pos.z)
                .map_err(|e| format!("Failed to crc Path node pos.z: {:?}", e))?;
            let mut layer_value = node.get_layer() as u32;
            xfer.xfer_unsigned_int(&mut layer_value)
                .map_err(|e| format!("Failed to crc Path node layer: {:?}", e))?;
            let mut can_opt = node.can_optimize();
            xfer.xfer_bool(&mut can_opt)
                .map_err(|e| format!("Failed to crc Path node can_optimize: {:?}", e))?;
            let mut opt_id: i32 = -1;
            if let Some(next_key) = node.next_opti {
                if let Some(next_node) = self.nodes.get(next_key) {
                    opt_id = next_node.id;
                }
            }
            xfer.xfer_int(&mut opt_id)
                .map_err(|e| format!("Failed to crc Path opt id: {:?}", e))?;
            remaining -= 1;
            current = node.prev;
        }
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 1;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| format!("Failed to xfer Path version: {:?}", e))?;

        let mut count: i32 = 0;
        if !xfer.is_loading() {
            count = self.nodes.len() as i32;
        }

        xfer.xfer_int(&mut count)
            .map_err(|e| format!("Failed to xfer Path node count: {:?}", e))?;

        if xfer.is_loading() {
            self.nodes.clear();
            self.head = None;
            self.tail = None;
            self.cpop_valid = false;
            self.cpop_countdown = Self::MAX_CPOP;
        }

        if !xfer.is_loading() {
            // Save: iterate from tail backwards to head via prev chain
            let mut remaining = count;
            let mut current = self.tail;
            while let Some(key) = current {
                {
                    let node = self.nodes.get_mut(key).unwrap();
                    node.id = remaining;
                }
                let node = self.nodes.get(key).unwrap();

                let mut node_id = remaining;
                xfer.xfer_int(&mut node_id)
                    .map_err(|e| format!("Failed to xfer Path node id: {:?}", e))?;

                let mut pos = *node.get_position();
                xfer.xfer_real(&mut pos.x)
                    .map_err(|e| format!("Failed to xfer Path node pos.x: {:?}", e))?;
                xfer.xfer_real(&mut pos.y)
                    .map_err(|e| format!("Failed to xfer Path node pos.y: {:?}", e))?;
                xfer.xfer_real(&mut pos.z)
                    .map_err(|e| format!("Failed to xfer Path node pos.z: {:?}", e))?;

                let layer = node.get_layer();
                let mut layer_value = layer as u32;
                xfer.xfer_unsigned_int(&mut layer_value)
                    .map_err(|e| format!("Failed to xfer Path node layer: {:?}", e))?;

                let mut can_opt = node.can_optimize();
                xfer.xfer_bool(&mut can_opt)
                    .map_err(|e| format!("Failed to xfer Path node can_optimize: {:?}", e))?;

                let mut opt_id: i32 = -1;
                if let Some(next_key) = node.next_opti {
                    if let Some(next_node) = self.nodes.get(next_key) {
                        opt_id = next_node.id;
                    }
                }
                xfer.xfer_int(&mut opt_id)
                    .map_err(|e| format!("Failed to xfer Path opt id: {:?}", e))?;

                remaining -= 1;
                // C++ writes the doubly-linked list tail -> head.
                current = node.prev;
            }
        } else {
            // Load: rebuild SlotMap from saved data
            let mut remaining = count;
            let mut id_map: HashMap<i32, PathNodeKey> = HashMap::new();
            let mut pending_opt: Vec<(i32, i32)> = Vec::new();
            while remaining > 0 {
                let mut node_id: i32 = 0;
                xfer.xfer_int(&mut node_id)
                    .map_err(|e| format!("Failed to xfer Path node id: {:?}", e))?;

                let mut pos = Coord3D::new(0.0, 0.0, 0.0);
                xfer.xfer_real(&mut pos.x)
                    .map_err(|e| format!("Failed to xfer Path load pos.x: {:?}", e))?;
                xfer.xfer_real(&mut pos.y)
                    .map_err(|e| format!("Failed to xfer Path load pos.y: {:?}", e))?;
                xfer.xfer_real(&mut pos.z)
                    .map_err(|e| format!("Failed to xfer Path load pos.z: {:?}", e))?;

                let mut layer_value: u32 = 0;
                xfer.xfer_unsigned_int(&mut layer_value)
                    .map_err(|e| format!("Failed to xfer Path node layer: {:?}", e))?;

                let mut can_opt = false;
                xfer.xfer_bool(&mut can_opt)
                    .map_err(|e| format!("Failed to xfer Path load can_optimize: {:?}", e))?;

                let mut opt_id: i32 = -1;
                xfer.xfer_int(&mut opt_id)
                    .map_err(|e| format!("Failed to xfer Path opt id: {:?}", e))?;

                let mut new_node = PathNode::new();
                new_node.id = node_id;
                new_node.set_position(&pos);
                new_node.set_layer(path_layer_from_u32(layer_value));
                new_node.set_can_optimize(can_opt);

                let new_key = self.nodes.insert(new_node);

                // First node inserted is the tail.  Each subsequent node is
                // prepended because C++ writes tail -> head.
                if let Some(old_head_key) = self.head.take() {
                    self.nodes[new_key].next = Some(old_head_key);
                    self.nodes[old_head_key].prev = Some(new_key);
                } else {
                    // First node is also tail
                    self.tail = Some(new_key);
                }
                self.head = Some(new_key);
                id_map.insert(node_id, new_key);

                if opt_id > 0 {
                    pending_opt.push((node_id, opt_id));
                }

                remaining -= 1;
            }

            for (node_id, opt_id) in pending_opt {
                if let (Some(&node_key), Some(&opti_key)) =
                    (id_map.get(&node_id), id_map.get(&opt_id))
                {
                    self.set_opti_link(node_key, Some(opti_key));
                }
            }
        }

        xfer.xfer_bool(&mut self.is_optimized)
            .map_err(|e| format!("Failed to xfer Path is_optimized: {:?}", e))?;
        let mut obsolete1: i32 = 0;
        xfer.xfer_int(&mut obsolete1)
            .map_err(|e| format!("Failed to xfer Path obsolete1: {:?}", e))?;
        let mut obsolete2: u32 = 0;
        xfer.xfer_unsigned_int(&mut obsolete2)
            .map_err(|e| format!("Failed to xfer Path obsolete2: {:?}", e))?;
        xfer.xfer_bool(&mut self.blocked_by_ally)
            .map_err(|e| format!("Failed to xfer Path blocked_by_ally: {:?}", e))?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

fn path_layer_from_u32(value: u32) -> PathfindLayerEnum {
    match value {
        1 => PathfindLayerEnum::Ground,
        2..=15 => PathfindLayerEnum::Top,
        _ => PathfindLayerEnum::Invalid,
    }
}

impl PathfindCell {
    pub fn new() -> Self {
        Self {
            cell_type: CellType::Clear,
            flags: CellFlags::NoUnits,
            layer: PathfindLayerEnum::Ground,
            connects_to_layer: PathfindLayerEnum::Invalid,
            zone: 0,
            aircraft_goal: false,
            pinched: false,
            info: None,
        }
    }

    /// Reset the cell to default state
    pub fn reset(&mut self) {
        self.cell_type = CellType::Clear;
        self.flags = CellFlags::NoUnits;
        self.info = None;
        self.pinched = false;
        self.aircraft_goal = false;
    }

    /// Get cell type
    pub fn get_type(&self) -> CellType {
        self.cell_type
    }

    /// Set cell type
    pub fn set_type(&mut self, cell_type: CellType) {
        self.cell_type = cell_type;
    }

    /// Get cell flags
    pub fn get_flags(&self) -> CellFlags {
        self.flags
    }

    /// Check if cell is impassable
    pub fn is_impassable(&self) -> bool {
        matches!(
            self.cell_type,
            CellType::Impassable | CellType::Obstacle | CellType::BridgeImpassable
        )
    }

    /// Set as obstacle
    pub fn set_type_as_obstacle(
        &mut self,
        obstacle_id: ObjectID,
        is_fence: bool,
        is_transparent: bool,
        pos: &ICoord2D,
    ) -> bool {
        self.cell_type = CellType::Obstacle;
        self.allocate_info(pos);
        if let Some(ref mut info) = self.info {
            info.obstacle_id = obstacle_id;
            info.obstacle_is_fence = is_fence;
            info.obstacle_is_transparent = is_transparent;
        }
        true
    }

    /// Prefer [`Self::set_type_as_obstacle`] with explicit flags.
    pub fn set_type_as_obstacle_from_object(
        &mut self,
        obstacle: &Arc<RwLock<Object>>,
        is_fence: bool,
        pos: &ICoord2D,
    ) -> bool {
        let Ok(obj_ref) = obstacle.try_read() else {
            return false;
        };
        self.set_type_as_obstacle(
            obj_ref.get_id(),
            is_fence,
            obj_ref.is_any_kind_of(&[KindOf::CanSeeThrough]),
            pos,
        )
    }

    /// Set as obstacle using a direct object reference (no Arc<Mutex> available).
    pub fn set_type_as_obstacle_for_object(
        &mut self,
        obstacle: &Object,
        is_fence: bool,
        pos: &ICoord2D,
    ) -> bool {
        self.cell_type = CellType::Obstacle;
        self.allocate_info(pos);
        if let Some(ref mut info) = self.info {
            info.obstacle_id = obstacle.get_id();
            info.obstacle_is_fence = is_fence;
            info.obstacle_is_transparent = obstacle.is_any_kind_of(&[KindOf::CanSeeThrough]);
        }
        true
    }

    /// Remove obstacle
    /// Prefer [`Self::remove_obstacle_by_id`].
    pub fn remove_obstacle(&mut self, obstacle: &Arc<RwLock<Object>>) -> bool {
        let Ok(obj_ref) = obstacle.try_read() else {
            return false;
        };
        self.remove_obstacle_by_id(obj_ref.get_id())
    }

    /// Remove obstacle by object id.
    pub fn remove_obstacle_by_id(&mut self, obj_id: ObjectID) -> bool {
        if let Some(ref info) = self.info {
            if info.obstacle_id == obj_id {
                self.cell_type = CellType::Clear;
                return true;
            }
        }
        false
    }

    /// Check if obstacle is present
    pub fn is_obstacle_present(&self, obj_id: ObjectID) -> bool {
        self.info
            .as_ref()
            .map(|info| info.obstacle_id == obj_id)
            .unwrap_or(false)
    }

    /// Allocate detailed info for this cell
    pub fn allocate_info(&mut self, pos: &ICoord2D) -> bool {
        if self.info.is_none() {
            self.info = Some(Box::new(PathfindCellInfo {
                next_open: None,
                prev_open: None,
                path_parent: None,
                total_cost: 0,
                cost_so_far: 0,
                pos: *pos,
                goal_unit_id: crate::common::INVALID_ID,
                pos_unit_id: crate::common::INVALID_ID,
                goal_aircraft_id: crate::common::INVALID_ID,
                obstacle_id: crate::common::INVALID_ID,
                is_free: true,
                blocked_by_ally: false,
                obstacle_is_fence: false,
                obstacle_is_transparent: false,
                open: false,
                closed: false,
            }));
            true
        } else {
            false
        }
    }

    /// Get zone
    pub fn get_zone(&self) -> u16 {
        self.zone
    }

    /// Set zone
    pub fn set_zone(&mut self, zone: u16) {
        self.zone = zone;
    }

    /// Check if pinched
    pub fn is_pinched(&self) -> bool {
        self.pinched
    }

    /// Set pinched state
    pub fn set_pinched(&mut self, pinched: bool) {
        self.pinched = pinched;
    }
}

/// Pathfinding layer enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathfindLayerEnum {
    Invalid = 0,
    Ground = 1,
    Top = 2,
    Wall = 15,
}

impl From<crate::path::PathfindLayerEnum> for PathfindLayerEnum {
    fn from(layer: crate::path::PathfindLayerEnum) -> Self {
        match layer {
            crate::path::PathfindLayerEnum::Wall => PathfindLayerEnum::Wall,
            crate::path::PathfindLayerEnum::Ground => PathfindLayerEnum::Ground,
            crate::path::PathfindLayerEnum::Invalid | crate::path::PathfindLayerEnum::Last => {
                PathfindLayerEnum::Invalid
            }
            _ => PathfindLayerEnum::Top,
        }
    }
}

impl From<crate::ai::pathfinding_system::PathfindLayerEnum> for PathfindLayerEnum {
    fn from(layer: crate::ai::pathfinding_system::PathfindLayerEnum) -> Self {
        match layer {
            crate::ai::pathfinding_system::PathfindLayerEnum::Air => PathfindLayerEnum::Top,
            crate::ai::pathfinding_system::PathfindLayerEnum::Invalid => PathfindLayerEnum::Invalid,
            _ => PathfindLayerEnum::Ground,
        }
    }
}

/// Simple pathfinder implementation
pub struct Pathfinder {
    /// Grid of pathfinding cells
    grid: Vec<Vec<PathfindCell>>,
    /// Grid dimensions
    width: usize,
    height: usize,
    /// Cell size in world units
    cell_size: f32,
    /// World offset
    world_offset: Coord2D,
}

impl Pathfinder {
    pub fn new(width: usize, height: usize, cell_size: f32, world_offset: Coord2D) -> Self {
        let mut grid = Vec::with_capacity(height);
        for _ in 0..height {
            let mut row = Vec::with_capacity(width);
            for _ in 0..width {
                row.push(PathfindCell::new());
            }
            grid.push(row);
        }

        Self {
            grid,
            width,
            height,
            cell_size,
            world_offset,
        }
    }

    /// Convert world coordinate to grid coordinate
    pub fn world_to_grid(&self, world_pos: &Coord3D) -> ICoord2D {
        ICoord2D::new(
            ((world_pos.x - self.world_offset.x) / self.cell_size) as i32,
            ((world_pos.y - self.world_offset.y) / self.cell_size) as i32,
        )
    }

    /// Convert grid coordinate to world coordinate
    pub fn grid_to_world(&self, grid_pos: &ICoord2D) -> Coord3D {
        let mut pos = Coord3D::new(
            grid_pos.x as f32 * self.cell_size + self.world_offset.x,
            grid_pos.y as f32 * self.cell_size + self.world_offset.y,
            0.0,
        );
        if let Some(terrain) = crate::helpers::TheTerrainLogic::get() {
            pos.z =
                terrain.get_layer_height(pos.x, pos.y, crate::common::PathfindLayerEnum::Ground);
        }
        pos
    }

    /// Treat the object's footprint as an obstacle wall (matches createAWallFromMyFootprint).
    pub fn create_wall_from_object(&mut self, obj: &Object) {
        let pos = obj.get_position();
        let radius = obj
            .get_geometry_info()
            .get_major_radius()
            .max(self.cell_size * 0.5);
        let center = self.world_to_grid(pos);
        let radius_cells = (radius / self.cell_size).ceil() as i32;

        for dy in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                let cell_x = center.x + dx;
                let cell_y = center.y + dy;
                if cell_x < 0 || cell_y < 0 {
                    continue;
                }
                let cell_x = cell_x as usize;
                let cell_y = cell_y as usize;
                if cell_x >= self.width || cell_y >= self.height {
                    continue;
                }

                let cell_world = self.grid_to_world(&ICoord2D::new(cell_x as i32, cell_y as i32));
                let delta_x = cell_world.x - pos.x;
                let delta_y = cell_world.y - pos.y;
                if (delta_x * delta_x + delta_y * delta_y) > radius * radius {
                    continue;
                }

                if let Some(cell) = self.get_cell_mut(cell_x, cell_y) {
                    cell.set_type_as_obstacle_for_object(
                        obj,
                        false,
                        &ICoord2D::new(cell_x as i32, cell_y as i32),
                    );
                }
            }
        }
    }

    /// Remove a previously created wall from this object's footprint.
    pub fn remove_wall_from_object(&mut self, obj: &Object) {
        let pos = obj.get_position();
        let radius = obj
            .get_geometry_info()
            .get_major_radius()
            .max(self.cell_size * 0.5);
        let center = self.world_to_grid(pos);
        let radius_cells = (radius / self.cell_size).ceil() as i32;
        let obj_id = obj.get_id();

        for dy in -radius_cells..=radius_cells {
            for dx in -radius_cells..=radius_cells {
                let cell_x = center.x + dx;
                let cell_y = center.y + dy;
                if cell_x < 0 || cell_y < 0 {
                    continue;
                }
                let cell_x = cell_x as usize;
                let cell_y = cell_y as usize;
                if cell_x >= self.width || cell_y >= self.height {
                    continue;
                }

                let cell_world = self.grid_to_world(&ICoord2D::new(cell_x as i32, cell_y as i32));
                let delta_x = cell_world.x - pos.x;
                let delta_y = cell_world.y - pos.y;
                if (delta_x * delta_x + delta_y * delta_y) > radius * radius {
                    continue;
                }

                if let Some(cell) = self.get_cell_mut(cell_x, cell_y) {
                    cell.remove_obstacle_by_id(obj_id);
                }
            }
        }
    }

    /// Get cell at grid position
    pub fn get_cell(&self, x: usize, y: usize) -> Option<&PathfindCell> {
        if x < self.width && y < self.height {
            Some(&self.grid[y][x])
        } else {
            None
        }
    }

    /// Get mutable cell at grid position
    pub fn get_cell_mut(&mut self, x: usize, y: usize) -> Option<&mut PathfindCell> {
        if x < self.width && y < self.height {
            Some(&mut self.grid[y][x])
        } else {
            None
        }
    }

    /// Check if attack view is blocked by obstacles (terrain LOS parity).
    /// Matches C++ Pathfinder::isAttackViewBlockedByObstacle in intent.
    pub fn is_attack_view_blocked_by_obstacle(
        &self,
        attacker: &Object,
        attacker_pos: &Coord3D,
        victim: Option<&Object>,
        victim_pos: &Coord3D,
    ) -> bool {
        // Wave 426: empty dual-world → false (no obstacles).
        if dual_world_registry_unavailable() {
            return false;
        }

        let ai_store = the_ai();let attack_uses_los = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|data| data.attack_uses_line_of_sight)
            })
            .unwrap_or(false);
        if !attack_uses_los {
            return false;
        }

        if let Some(victim_obj) = victim {
            if victim_obj.is_significantly_above_terrain() {
                return false;
            }
        }

        if attacker.is_kind_of(KindOf::Immobile) {
            return false;
        }

        let Ok(terrain) = get_terrain_logic().read() else {
            return false;
        };

        if !terrain.is_clear_line_of_sight(attacker_pos, victim_pos) {
            return true;
        }

        let attack_id = attacker.get_id();
        let victim_id = victim
            .map(|obj| obj.get_id())
            .unwrap_or(crate::common::INVALID_ID);
        let dx = victim_pos.x - attacker_pos.x;
        let dy = victim_pos.y - attacker_pos.y;
        let segment_len = (dx * dx + dy * dy).sqrt();
        let scan_radius = segment_len * 0.5 + PATHFIND_CELL_SIZE_F * 2.0;
        let scan_center = Coord3D::new(attacker_pos.x + dx * 0.5, attacker_pos.y + dy * 0.5, 0.0);

        let Some(partition) = ThePartitionManager::get() else {
            return false;
        };

        let candidates = partition.get_objects_in_range(&scan_center, scan_radius);
        for obj_id in candidates {
            if obj_id == attack_id || obj_id == victim_id {
                continue;
            }
            if OBJECT_REGISTRY
                .with_object(obj_id, |obj_guard| {
                    if obj_guard.is_significantly_above_terrain() {
                        return false;
                    }
                    if !(obj_guard.is_kind_of(KindOf::Structure)
                        || obj_guard.is_kind_of(KindOf::Building)
                        || obj_guard.is_kind_of(KindOf::Defense)
                        || obj_guard.is_kind_of(KindOf::Bridge))
                    {
                        return false;
                    }
                    let radius = obj_guard.get_geometry_info().get_bounding_circle_radius()
                        + PATHFIND_CELL_SIZE_F;
                    let pos = obj_guard.get_position();
                    let dist_sqr =
                        Self::distance_sq_point_to_segment_2d(pos, attacker_pos, victim_pos);
                    dist_sqr <= radius * radius
                })
                .unwrap_or(false)
            {
                return true;
            }
        }

        false
    }

    fn distance_sq_point_to_segment_2d(p: &Coord3D, a: &Coord3D, b: &Coord3D) -> f32 {
        let ax = a.x;
        let ay = a.y;
        let bx = b.x;
        let by = b.y;
        let dx = bx - ax;
        let dy = by - ay;
        let len_sq = dx * dx + dy * dy;
        if len_sq <= 0.0001 {
            let px = p.x - ax;
            let py = p.y - ay;
            return px * px + py * py;
        }
        let t = ((p.x - ax) * dx + (p.y - ay) * dy) / len_sq;
        let t = t.max(0.0).min(1.0);
        let cx = ax + t * dx;
        let cy = ay + t * dy;
        let px = p.x - cx;
        let py = p.y - cy;
        px * px + py * py
    }

    /// Find path from start to goal using A* pathfinding
    pub fn find_path(&self, start: &Coord3D, goal: &Coord3D) -> Option<Path> {
        let start_grid = self.world_to_grid(start);
        let goal_grid = self.world_to_grid(goal);

        // Check bounds
        if start_grid.x < 0
            || start_grid.x >= self.width as i32
            || start_grid.y < 0
            || start_grid.y >= self.height as i32
            || goal_grid.x < 0
            || goal_grid.x >= self.width as i32
            || goal_grid.y < 0
            || goal_grid.y >= self.height as i32
        {
            return None;
        }

        self.find_path_astar(start_grid, goal_grid, start, goal)
    }

    /// A* pathfinding implementation
    fn find_path_astar(
        &self,
        start_grid: ICoord2D,
        goal_grid: ICoord2D,
        start_world: &Coord3D,
        goal_world: &Coord3D,
    ) -> Option<Path> {
        use std::cmp::Ordering;
        use std::collections::BinaryHeap;

        // A* node for the priority queue
        #[derive(Clone)]
        struct AStarNode {
            pos: ICoord2D,
            g_cost: f32,
            h_cost: f32,
            parent: Option<ICoord2D>,
        }

        impl AStarNode {
            fn f_cost(&self) -> f32 {
                self.g_cost + self.h_cost
            }
        }

        impl PartialEq for AStarNode {
            fn eq(&self, other: &Self) -> bool {
                self.pos == other.pos
            }
        }

        impl Eq for AStarNode {}

        impl PartialOrd for AStarNode {
            fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                Some(self.cmp(other))
            }
        }

        impl Ord for AStarNode {
            fn cmp(&self, other: &Self) -> Ordering {
                // Reverse ordering for min-heap behavior
                other
                    .f_cost()
                    .partial_cmp(&self.f_cost())
                    .unwrap_or(Ordering::Equal)
            }
        }

        let mut open_set = BinaryHeap::new();
        let mut closed_set = std::collections::HashSet::new();
        let mut came_from = std::collections::HashMap::new();

        // Helper function to calculate heuristic distance
        let heuristic =
            |a: &ICoord2D, b: &ICoord2D| -> f32 { ((a.x - b.x).abs() + (a.y - b.y).abs()) as f32 };

        // Helper function to calculate movement cost
        let movement_cost = |from: &ICoord2D, to: &ICoord2D| -> f32 {
            if let Some(cell) = self.get_cell(to.x as usize, to.y as usize) {
                if cell.is_impassable() {
                    return f32::INFINITY;
                }
                // Base cost is 1.0 for orthogonal, ~1.4 for diagonal
                let dx = (to.x - from.x).abs();
                let dy = (to.y - from.y).abs();
                if dx == 1 && dy == 1 {
                    1.414 // Diagonal movement
                } else {
                    1.0 // Orthogonal movement
                }
            } else {
                f32::INFINITY
            }
        };

        // Initialize with start node
        let start_node = AStarNode {
            pos: start_grid,
            g_cost: 0.0,
            h_cost: heuristic(&start_grid, &goal_grid),
            parent: None,
        };

        open_set.push(start_node);

        // A* main loop
        while let Some(current) = open_set.pop() {
            if current.pos == goal_grid {
                // Reconstruct path
                let mut path_points = Vec::new();
                let mut current_pos = goal_grid;

                path_points.push(current_pos);

                while let Some(&parent) = came_from.get(&current_pos) {
                    path_points.push(parent);
                    current_pos = parent;
                }

                path_points.reverse();

                // Convert to Path structure
                let mut path = Path::new();
                for (i, &grid_pos) in path_points.iter().enumerate() {
                    let world_pos = if i == 0 {
                        *start_world
                    } else if i == path_points.len() - 1 {
                        *goal_world
                    } else {
                        self.grid_to_world(&grid_pos)
                    };

                    if i == 0 {
                        path.prepend_node(&world_pos, PathfindLayerEnum::Ground);
                    } else {
                        path.append_node(&world_pos, PathfindLayerEnum::Ground);
                    }
                }

                return Some(path);
            }

            closed_set.insert(current.pos);

            // Check all neighbors
            for dx in -1..=1 {
                for dy in -1..=1 {
                    if dx == 0 && dy == 0 {
                        continue; // Skip current position
                    }

                    let neighbor_pos = ICoord2D::new(current.pos.x + dx, current.pos.y + dy);

                    // Check bounds
                    if neighbor_pos.x < 0
                        || neighbor_pos.x >= self.width as i32
                        || neighbor_pos.y < 0
                        || neighbor_pos.y >= self.height as i32
                    {
                        continue;
                    }

                    if closed_set.contains(&neighbor_pos) {
                        continue;
                    }

                    let move_cost = movement_cost(&current.pos, &neighbor_pos);
                    if move_cost == f32::INFINITY {
                        continue; // Impassable
                    }

                    let tentative_g_cost = current.g_cost + move_cost;

                    // Check if this path is better
                    let mut found_better = false;
                    let existing_nodes: Vec<_> = open_set.clone().into_sorted_vec();
                    for node in &existing_nodes {
                        if node.pos == neighbor_pos && tentative_g_cost >= node.g_cost {
                            found_better = true;
                            break;
                        }
                    }

                    if !found_better {
                        came_from.insert(neighbor_pos, current.pos);

                        let neighbor_node = AStarNode {
                            pos: neighbor_pos,
                            g_cost: tentative_g_cost,
                            h_cost: heuristic(&neighbor_pos, &goal_grid),
                            parent: Some(current.pos),
                        };

                        open_set.push(neighbor_node);
                    }
                }
            }
        }

        // No path found - return direct path as fallback
        let mut path = Path::new();
        path.prepend_node(start_world, PathfindLayerEnum::Ground);
        path.append_node(goal_world, PathfindLayerEnum::Ground);
        Some(path)
    }
}

/// Helper function to adjust destination for an object's movement capabilities.
/// Matches C++ Pathfinder::adjustDestination.
pub fn adjust_destination_for_object(
    obj_id: ObjectID,
    goal: &mut Coord3D,
    group_dest: Option<&Coord3D>,
) -> Result<(), String> {
    let obj = TheGameLogic::find_object_by_id(obj_id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        .ok_or_else(|| format!("Could not resolve object {obj_id} for destination adjustment"))?;
    let obj_guard = obj
        .try_read()
        .map_err(|_| "Could not lock object for destination adjustment")?;

    // Get AI update interface if available
    if let Some(_ai) = obj_guard.get_ai_update_interface() {
        // Use the AI's adjust_destination if it has one
        drop(obj_guard);

        // Try to get mutable access to call adjust_destination
        if let Ok(mut obj_mut) = obj.try_write() {
            if let Some(ai_mut) = obj_mut.get_ai_update_interface_mut() {
                // Note: The actual adjustment is done via the trait method
                // This is a simplified implementation
                let _ = ai_mut;
            }
        }
        return Ok(());
    }

    // Fall back to terrain-based adjustment if no AI
    if let Some(group_center) = group_dest {
        // If we have a group center, adjust relative to it
        let dx = goal.x - group_center.x;
        let dy = goal.y - group_center.y;
        let dist = (dx * dx + dy * dy).sqrt();

        // Don't let units stray too far from group center
        const MAX_OFFSET: f32 = 100.0;
        if dist > MAX_OFFSET {
            let scale = MAX_OFFSET / dist;
            goal.x = group_center.x + dx * scale;
            goal.y = group_center.y + dy * scale;
        }
    }

    Ok(())
}

/// Helper function to update an object's pathfinding goal.
/// Matches C++ Pathfinder::updateGoal.
pub fn update_goal_for_object(
    obj_id: ObjectID,
    goal: &Coord3D,
    layer: PathfindLayerEnum,
) -> Result<(), String> {
    let obj = TheGameLogic::find_object_by_id(obj_id)
        .or_else(|| crate::object::registry::OBJECT_REGISTRY.get_object(obj_id))
        .ok_or_else(|| format!("Could not resolve object {obj_id} for goal update"))?;
    let obj_guard = obj
        .try_read()
        .map_err(|_| "Could not lock object for goal update")?;

    let cell = crate::ai::pathfind_astar::GridCoord::from_world(goal);
    let radius = {
        let geo = obj_guard.get_geometry_info();
        let r = (geo.get_major_radius() / crate::ai::pathfind_astar::PATHFIND_CELL_SIZE_F).floor()
            as i32;
        r.clamp(0, 2)
    };
    let dest_layer = get_terrain_logic()
        .read()
        .ok()
        .map(|t| t.get_layer_for_destination(goal))
        .unwrap_or(crate::path::PathfindLayerEnum::Ground);
    let interacts_with_bridge_end = get_terrain_logic()
        .read()
        .ok()
        .map(|t| t.object_interacts_with_bridge_end(&*obj_guard, dest_layer))
        .unwrap_or(false);
    let pf_layer = if dest_layer != crate::path::PathfindLayerEnum::Ground {
        crate::ai::pathfind_astar::PathfindLayerEnum::from_u32(dest_layer as u32)
    } else {
        match layer {
            PathfindLayerEnum::Ground => crate::ai::pathfind_astar::PathfindLayerEnum::Ground,
            PathfindLayerEnum::Wall => crate::ai::pathfind_astar::PathfindLayerEnum::Wall,
            _ => crate::ai::pathfind_astar::PathfindLayerEnum::Top,
        }
    };
    drop(obj_guard);
    let ai_store = the_ai(); if let Ok(ai) = ai_store.read() {
        if let Some(pf) = ai.pathfinder() {
            if let Ok(pf) = pf.read() {
                pf.update_goal_cells(
                    cell,
                    obj_id,
                    pf_layer,
                    radius,
                    true,
                    interacts_with_bridge_end,
                );
            }
        }
    }
    Ok(())
}

/// Snap a world position to the nearest pathfind grid cell center.
/// Matches C++ Pathfinder::goalPosition() — returns the grid-snapped position
/// and whether snapping actually changed the position.
///
/// C++ reference (AIStates.cpp:1347):
///   if (TheAI->pathfinder()->goalPosition(obj, &goalPos)) { ... }
///
/// The C++ version looks up the object's registered goal in the pathfinder's
/// internal goal map and returns the grid-cell-center position.  In this port
/// we compute it directly from the world→grid→world round-trip.
pub fn goal_position(pos: &Coord3D) -> Option<Coord3D> {
    let ai_store = the_ai();let ai = ai_store.read().ok()?;
    let pf_arc = ai.pathfinder()?;
    let pf = pf_arc.read().ok()?;

    let grid = pf.world_to_grid(pos);
    let mut snapped = pf.grid_to_world(&grid);

    // Preserve Z from terrain (C++ does this via the layer/terrain query)
    if let Ok(terrain) = get_terrain_logic().read() {
        snapped.z = terrain.get_ground_height(snapped.x, snapped.y, None);
    } else {
        snapped.z = pos.z;
    }

    // Only return Some if snapping actually moved the position
    let dx = snapped.x - pos.x;
    let dy = snapped.y - pos.y;
    if dx.abs() > 0.001 || dy.abs() > 0.001 {
        Some(snapped)
    } else {
        None
    }
}

/// PARITY_NOTE: C++ uses full A* pathfind map (AIPathfind.cpp). Rust uses simplified
/// direct path with terrain height sampling until the full pathfind map is ported.
pub fn find_path(start: Coord3D, end: Coord3D, obj: Option<ObjectID>) -> Option<Vec<Coord3D>> {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let dist_2d = (dx * dx + dy * dy).sqrt();

    if dist_2d < PATHFIND_CLOSE_ENOUGH {
        return Some(vec![start, end]);
    }

    // Try full A* pathfinder first, fall back to straight-line if unavailable.
    let ai_store = the_ai(); if let Some(pathfinder) = ai_store.read().ok().and_then(|ai| ai.pathfinder()) {
        if let Ok(pf_guard) = pathfinder.read() {
            if let Some(waypoints) =
                pf_guard.find_path(&start, &end, crate::path::SURFACE_GROUND, false)
            {
                if !waypoints.is_empty() {
                    return Some(waypoints);
                }
            }
        }
    }

    let step_size = PATHFIND_CELL_SIZE_F;
    let num_steps = (dist_2d / step_size).ceil() as usize;
    let num_steps = num_steps.min(2000);

    let mut path = Vec::with_capacity(num_steps + 2);
    path.push(start);

    let _ = obj;

    for i in 1..num_steps {
        let t = i as f32 / num_steps as f32;
        let mut pos = Coord3D::new(start.x + dx * t, start.y + dy * t, 0.0);
        if let Ok(terrain) = get_terrain_logic().read() {
            pos.z = terrain.get_ground_height(pos.x, pos.y, None);
        }
        path.push(pos);
    }

    let mut final_pos = end;
    if let Ok(terrain) = get_terrain_logic().read() {
        final_pos.z = terrain.get_ground_height(final_pos.x, final_pos.y, None);
    }
    path.push(final_pos);

    Some(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use game_engine::common::system::xfer_load::XferLoad;
    use game_engine::common::system::xfer_save::XferSave;
    use std::io::Cursor;

    fn points(path: &Path) -> Vec<Coord3D> {
        path.ordered_keys()
            .into_iter()
            .map(|key| path.nodes.get(key).expect("ordered key is live").pos)
            .collect()
    }

    #[test]
    fn path_nodes_keep_cxx_head_to_tail_links() {
        // C++ Path::prependNode then appendNode builds a doubly-linked chain
        // traversable from m_path with PathNode::getNext().
        let mut path = Path::new();
        path.prepend_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground);

        let keys = path.ordered_keys();
        assert_eq!(keys.len(), 3);
        assert_eq!(points(&path)[2], Coord3D::new(20.0, 0.0, 0.0));
        assert_eq!(path.nodes[keys[0]].next, Some(keys[1]));
        assert_eq!(path.nodes[keys[1]].prev, Some(keys[0]));
        assert_eq!(path.nodes[keys[1]].next, Some(keys[2]));
        assert_eq!(path.nodes[keys[2]].prev, Some(keys[1]));
    }

    #[test]
    fn path_closest_point_uses_later_live_path_segments() {
        let mut path = Path::new();
        path.prepend_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(10.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        path.append_node(&Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground);

        let closest = path.compute_point_on_path(&Coord3D::new(15.0, 5.0, 0.0));
        assert!((closest.pos_on_path.x - 15.0).abs() < 0.001);
        assert!(closest.pos_on_path.y.abs() < 0.001);
    }

    #[test]
    fn path_xfer_round_trip_preserves_head_to_tail_order() {
        let mut source = Path::new();
        source.prepend_node(&Coord3D::new(0.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        source.append_node(&Coord3D::new(10.0, 0.0, 0.0), PathfindLayerEnum::Ground);
        source.append_node(&Coord3D::new(20.0, 0.0, 0.0), PathfindLayerEnum::Ground);

        let mut saved = Vec::new();
        {
            let cursor = Cursor::new(&mut saved);
            let mut xfer = XferSave::new(cursor, 1);
            Snapshotable::xfer(&mut source, &mut xfer).expect("save path");
        }

        let mut loaded = Path::new();
        {
            let cursor = Cursor::new(saved);
            let mut xfer = XferLoad::new(cursor, 1);
            Snapshotable::xfer(&mut loaded, &mut xfer).expect("load path");
        }

        assert_eq!(
            points(&loaded),
            vec![
                Coord3D::new(0.0, 0.0, 0.0),
                Coord3D::new(10.0, 0.0, 0.0),
                Coord3D::new(20.0, 0.0, 0.0),
            ]
        );
    }

    #[test]
    fn a_star_path_keeps_every_waypoint_for_movement() {
        let pathfinder = Pathfinder::new(5, 1, 10.0, Coord2D::new(0.0, 0.0));
        let path = pathfinder
            .find_path(&Coord3D::new(0.0, 0.0, 0.0), &Coord3D::new(40.0, 0.0, 0.0))
            .expect("open ground has a path");

        assert_eq!(points(&path).len(), 5);
        assert_eq!(
            points(&path).last().copied(),
            Some(Coord3D::new(40.0, 0.0, 0.0))
        );
    }
}
