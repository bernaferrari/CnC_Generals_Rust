//! Pathfinding request/result types, occupancy cells, bridges, and queues.
//! Extracted from the former monolithic `pathfind_complete.rs`.

use super::*;

/// Maximum pathfind queue length
/// Matches C++ PATHFIND_QUEUE_LEN at AIPathfind.h:418
pub const PATHFIND_QUEUE_LEN: usize = 512;

/// C++ PATHFIND_CELLS_PER_FRAME — max cells examined per processPathfindQueue call.
/// C++ LAYER_Z_CLOSE_ENOUGH_F (AIPathfind.h).
pub const LAYER_Z_CLOSE_ENOUGH_F: f32 = 10.0;
pub const PATHFIND_CELLS_PER_FRAME: usize = 500;
/// C++ MAX_WALL_PIECES (AIPathfind.h).
pub const MAX_WALL_PIECES: usize = 128;
/// C++ PathfindZoneManager::ZONE_BLOCK_SIZE (AIPathfind.h:479).
pub const ZONE_BLOCK_SIZE: i32 = 10;
/// C++ PathfindZoneManager::UNINITIALIZED_ZONE.
pub const UNINITIALIZED_ZONE: u16 = 0xFFFF;

/// Maximum iterations for A* to prevent infinite loops
pub const MAX_PATH_ITERATIONS: usize = 10000;

/// Locomotor surface type mask matching C++ LocomotorSurfaceTypeMask
pub type LocomotorSurfaceTypeMask = u32;

pub const SURFACE_GROUND: u32 = 0x01;
pub const SURFACE_WATER: u32 = 0x02;
pub const SURFACE_CLIFF: u32 = 0x04;
pub const SURFACE_AIR: u32 = 0x08;
pub const SURFACE_RUBBLE: u32 = 0x10;

/// Wave 262: host-only path has no dual-world factory objects.
#[inline]
pub(crate) fn dual_world_registry_unavailable() -> bool {
    OBJECT_REGISTRY.is_empty()
}

/// Pathfinding request
#[derive(Debug, Clone)]
pub struct PathRequest {
    pub object_id: ObjectID,
    pub from: Coord3D,
    pub to: Coord3D,
    pub surfaces: LocomotorSurfaceTypeMask,
    pub is_crusher: bool,
    pub unit_radius: f32,
    pub allow_partial: bool,
    pub move_allies: bool,
    pub ignore_obstacle_id: Option<ObjectID>,
    /// C++ human player pathing clamps to m_logicalExtent (AI may leave map).
    pub is_human: bool,
}

impl PathRequest {
    pub fn new(from: Coord3D, to: Coord3D, surfaces: LocomotorSurfaceTypeMask) -> Self {
        Self {
            object_id: INVALID_ID,
            from,
            to,
            surfaces,
            is_crusher: false,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human: false,
        }
    }
}

/// Pathfinding result
#[derive(Debug, Clone)]
pub struct PathResult {
    pub success: bool,
    pub waypoints: Vec<Coord3D>,
    pub layers: Vec<PathfindLayerEnum>,
    /// Per-waypoint canOptimize (C++ PathNode::setCanOptimize from prependCells).
    pub can_optimize: Vec<bool>,
    pub total_cost: u32,
    pub blocked_by_ally: bool,
}

impl PathResult {
    pub fn none() -> Self {
        Self {
            success: false,
            waypoints: Vec::new(),
            layers: Vec::new(),
            can_optimize: Vec::new(),
            total_cost: u32::MAX,
            blocked_by_ally: false,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GoalCell {
    pub(crate) goal_unit_ground: ObjectID,
    pub(crate) goal_unit_top: ObjectID,
    pub(crate) goal_aircraft: ObjectID,
    /// C++ PathfindCell::getPosUnit / setPosUnit (UNIT_PRESENT_FIXED occupancy).
    pub(crate) pos_unit_ground: ObjectID,
    pub(crate) pos_unit_top: ObjectID,
}

impl GoalCell {
    pub(crate) fn new() -> Self {
        Self {
            goal_unit_ground: INVALID_ID,
            goal_unit_top: INVALID_ID,
            goal_aircraft: INVALID_ID,
            pos_unit_ground: INVALID_ID,
            pos_unit_top: INVALID_ID,
        }
    }

    pub(crate) fn get_goal_unit(&self, layer: PathfindLayerEnum) -> ObjectID {
        match layer {
            PathfindLayerEnum::Ground => self.goal_unit_ground,
            _ => self.goal_unit_top,
        }
    }

    pub(crate) fn set_goal_unit(&mut self, layer: PathfindLayerEnum, unit: ObjectID) {
        match layer {
            PathfindLayerEnum::Ground => self.goal_unit_ground = unit,
            _ => self.goal_unit_top = unit,
        }
    }

    pub(crate) fn clear_goal_unit(&mut self, layer: PathfindLayerEnum, unit: ObjectID) {
        match layer {
            PathfindLayerEnum::Ground => {
                if self.goal_unit_ground == unit {
                    self.goal_unit_ground = INVALID_ID;
                }
            }
            _ => {
                if self.goal_unit_top == unit {
                    self.goal_unit_top = INVALID_ID;
                }
            }
        }
    }

    pub(crate) fn get_pos_unit(&self, layer: PathfindLayerEnum) -> ObjectID {
        match layer {
            PathfindLayerEnum::Ground => self.pos_unit_ground,
            _ => self.pos_unit_top,
        }
    }

    pub(crate) fn set_pos_unit(&mut self, layer: PathfindLayerEnum, unit: ObjectID) {
        match layer {
            PathfindLayerEnum::Ground => self.pos_unit_ground = unit,
            _ => self.pos_unit_top = unit,
        }
    }

    pub(crate) fn clear_pos_unit(&mut self, layer: PathfindLayerEnum, unit: ObjectID) {
        match layer {
            PathfindLayerEnum::Ground => {
                if self.pos_unit_ground == unit {
                    self.pos_unit_ground = INVALID_ID;
                }
            }
            _ => {
                if self.pos_unit_top == unit {
                    self.pos_unit_top = INVALID_ID;
                }
            }
        }
    }

    pub(crate) fn set_goal_aircraft(&mut self, unit: ObjectID) {
        self.goal_aircraft = unit;
    }

    pub(crate) fn clear_goal_aircraft(&mut self, unit: ObjectID) {
        if self.goal_aircraft == unit {
            self.goal_aircraft = INVALID_ID;
        }
    }

    pub(crate) fn has_aircraft_goal(&self) -> bool {
        self.goal_aircraft != INVALID_ID
    }
}

/// Bridge/layer information for pathfinding
/// Matches C++ PathfindLayer at AIPathfind.h:363-412
#[derive(Debug, Clone)]
pub struct BridgeLayer {
    pub layer_id: u32,
    pub bounds: (GridCoord, GridCoord),
    pub destroyed: bool,
    pub zone: u16,
    /// C++ PathfindLayer bridge object id (INVALID_ID if landmark-only).
    pub bridge_object_id: ObjectID,
    /// C++ m_startCell / m_endCell (from bridge from/to attach).
    pub start_cell: GridCoord,
    pub end_cell: GridCoord,
    /// C++ layer cells with getConnectLayer()==LAYER_GROUND (entry points).
    /// Populated at classify/add time; scanned by `connectsZones`.
    pub ground_connect_cells: Vec<GridCoord>,
}

impl BridgeLayer {
    pub fn new(layer_id: u32, bounds: (GridCoord, GridCoord)) -> Self {
        Self::with_meta(layer_id, bounds, INVALID_ID, bounds.0, bounds.1)
    }

    pub fn with_meta(
        layer_id: u32,
        bounds: (GridCoord, GridCoord),
        bridge_object_id: ObjectID,
        start_cell: GridCoord,
        end_cell: GridCoord,
    ) -> Self {
        // Default entry points: attach cells (C++ isCellEntryPoint marks ends).
        // Full classifyCells can replace this via set_ground_connect_cells.
        let mut ground_connect_cells = Vec::new();
        if start_cell != end_cell {
            ground_connect_cells.push(start_cell);
            ground_connect_cells.push(end_cell);
        } else {
            ground_connect_cells.push(start_cell);
        }
        Self {
            layer_id,
            bounds,
            destroyed: false,
            zone: 0,
            bridge_object_id,
            start_cell,
            end_cell,
            ground_connect_cells,
        }
    }

    pub fn contains(&self, coord: GridCoord) -> bool {
        coord.x >= self.bounds.0.x
            && coord.x <= self.bounds.1.x
            && coord.y >= self.bounds.0.y
            && coord.y <= self.bounds.1.y
    }

    /// Replace entry-point cells after C++-style classifyCells.
    pub fn set_ground_connect_cells(&mut self, cells: Vec<GridCoord>) {
        self.ground_connect_cells = cells;
    }

    /// C++ `PathfindLayer::connectsZones` (AIPathfind.cpp).
    ///
    /// Scans layer cells with connectLayer==GROUND; reads ground-cell zones
    /// via `zone_at` (effective terrain zone already applied by caller).
    pub fn connects_zones(
        &self,
        zone_at: impl Fn(GridCoord) -> u16,
        zone1: u16,
        zone2: u16,
    ) -> bool {
        if !self.destroyed {
            return false;
        }
        // C++ only sets found when groundCell zone equals zone1/zone2.
        // No special-case true for zone 0/uninitialized.
        let mut found1 = false;
        let mut found2 = false;
        for c in &self.ground_connect_cells {
            let z = zone_at(*c);
            if z == 0 {
                continue;
            }
            if z == zone1 {
                found1 = true;
            }
            if z == zone2 {
                found2 = true;
            }
            if found1 && found2 {
                return true;
            }
        }
        found1 && found2
    }
}

pub(crate) fn ignored_obstacle_cells(
    ignore_obstacle_id: Option<ObjectID>,
) -> Option<HashSet<GridCoord>> {
    // Wave 262: empty dual-world → None.
    if dual_world_registry_unavailable() {
        return None;
    }

    let object_id = ignore_obstacle_id?;
    if object_id == INVALID_ID {
        return None;
    }

    let positions = OBJECT_REGISTRY
        .with_object(object_id, |guard| object_footprint_positions(guard))
        .flatten()?;
    let mut cells = HashSet::with_capacity(positions.len());
    for pos in positions {
        cells.insert(GridCoord::from_world(&pos));
    }
    if cells.is_empty() {
        None
    } else {
        Some(cells)
    }
}

/// Cohen–Sutherland style cell-line clip against inclusive grid extent (lo, hi).
/// Matches C++ ClipLine2D usage in Pathfinder::clip.
pub(crate) fn clip_line_cells(
    p1: GridCoord,
    p2: GridCoord,
    extent: (GridCoord, GridCoord),
) -> Option<(GridCoord, GridCoord)> {
    let lo = extent.0;
    let hi = extent.1;
    let code = |c: GridCoord| -> u8 {
        let mut out = 0u8;
        if c.x < lo.x {
            out |= 1;
        } else if c.x > hi.x {
            out |= 2;
        }
        if c.y < lo.y {
            out |= 4;
        } else if c.y > hi.y {
            out |= 8;
        }
        out
    };
    let mut x1 = p1.x as f64;
    let mut y1 = p1.y as f64;
    let mut x2 = p2.x as f64;
    let mut y2 = p2.y as f64;
    let mut c1 = code(p1);
    let mut c2 = code(p2);
    for _ in 0..16 {
        if (c1 | c2) == 0 {
            return Some((
                GridCoord::new(x1.round() as i32, y1.round() as i32),
                GridCoord::new(x2.round() as i32, y2.round() as i32),
            ));
        }
        if (c1 & c2) != 0 {
            return None;
        }
        let out = if c1 != 0 { c1 } else { c2 };
        let (x, y) = if out & 1 != 0 {
            // left
            let y = if (x2 - x1).abs() < f64::EPSILON {
                y1
            } else {
                y1 + (y2 - y1) * (lo.x as f64 - x1) / (x2 - x1)
            };
            (lo.x as f64, y)
        } else if out & 2 != 0 {
            let y = if (x2 - x1).abs() < f64::EPSILON {
                y1
            } else {
                y1 + (y2 - y1) * (hi.x as f64 - x1) / (x2 - x1)
            };
            (hi.x as f64, y)
        } else if out & 4 != 0 {
            let x = if (y2 - y1).abs() < f64::EPSILON {
                x1
            } else {
                x1 + (x2 - x1) * (lo.y as f64 - y1) / (y2 - y1)
            };
            (x, lo.y as f64)
        } else {
            let x = if (y2 - y1).abs() < f64::EPSILON {
                x1
            } else {
                x1 + (x2 - x1) * (hi.y as f64 - y1) / (y2 - y1)
            };
            (x, hi.y as f64)
        };
        if out == c1 {
            x1 = x;
            y1 = y;
            c1 = code(GridCoord::new(x1.round() as i32, y1.round() as i32));
        } else {
            x2 = x;
            y2 = y;
            c2 = code(GridCoord::new(x2.round() as i32, y2.round() as i32));
        }
    }
    None
}

/// C++ `TCheckMovementInfo` result (AIPathfind.cpp checkForMovement).
#[derive(Debug, Clone)]
pub struct CheckMovementInfo {
    pub cell: GridCoord,
    pub layer: PathfindLayerEnum,
    pub center_in_cell: bool,
    pub radius: i32,
    pub consider_transient: bool,
    pub acceptable_surfaces: LocomotorSurfaceTypeMask,
    pub ally_fixed_count: i32,
    pub ally_moving: bool,
    pub ally_goal: bool,
    pub enemy_fixed: bool,
}

impl Default for CheckMovementInfo {
    fn default() -> Self {
        Self {
            cell: GridCoord::new(0, 0),
            layer: PathfindLayerEnum::Ground,
            center_in_cell: true,
            radius: 0,
            consider_transient: false,
            acceptable_surfaces: SURFACE_GROUND,
            ally_fixed_count: 0,
            ally_moving: false,
            ally_goal: false,
            enemy_fixed: false,
        }
    }
}

/// C++ pathfind ObjectID ring buffer (queueForPath / processPathfindQueue).
pub(crate) struct ObjectPathQueue {
    pub(crate) slots: [ObjectID; PATHFIND_QUEUE_LEN],
    pub(crate) head: usize,
    pub(crate) tail: usize,
}

impl ObjectPathQueue {
    pub(crate) fn new() -> Self {
        Self {
            slots: [INVALID_ID; PATHFIND_QUEUE_LEN],
            head: 0,
            tail: 0,
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.head == self.tail
    }

    /// C++ queueForPath: dedupe + ring push. Returns false if full.
    pub(crate) fn queue(&mut self, id: ObjectID) -> bool {
        if id == INVALID_ID {
            return false;
        }
        // Already queued?
        let mut slot = self.head;
        while slot != self.tail {
            if self.slots[slot] == id {
                return true;
            }
            slot += 1;
            if slot >= PATHFIND_QUEUE_LEN {
                slot = 0;
            }
        }
        let next = (self.tail + 1) % PATHFIND_QUEUE_LEN;
        if next == self.head {
            return false; // full
        }
        self.slots[self.tail] = id;
        self.tail = next;
        true
    }

    pub(crate) fn pop_front(&mut self) -> Option<ObjectID> {
        if self.head == self.tail {
            return None;
        }
        let id = self.slots[self.head];
        self.slots[self.head] = INVALID_ID;
        self.head = (self.head + 1) % PATHFIND_QUEUE_LEN;
        if id == INVALID_ID {
            None
        } else {
            Some(id)
        }
    }
}
