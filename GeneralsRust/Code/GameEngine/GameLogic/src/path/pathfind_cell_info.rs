//! PathfindCellInfo implementation matching C++ PathfindCellInfo class
#![allow(missing_docs)]
//!
//! Memory-efficient cell information structure for A* pathfinding.
//! This is allocated only when needed to save memory.
//! This standalone cell-info facade is not Main's A* implementation; Main
//! uses `ai::pathfind_astar`. Its link operations do not perform list sorting
//! or automatically change the open/closed flags.

use super::*;
use crate::common::*;
use std::marker::PhantomData;

/// PathfindCellInfo structure matching C++ PathfindCellInfo class
#[derive(Debug)]
pub struct PathfindCellInfo {
    // Internal keys are resolved only through the owning pool's scope.
    next_open: Option<CellKey>,
    prev_open: Option<CellKey>,
    path_parent: Option<CellKey>,

    // A* cost estimates
    total_cost: u16,  // total estimated cost (f = g + h)
    cost_so_far: u16, // cost from start to this cell (g)

    // Cell coordinates (needed since cells are often accessed via pointer only)
    pos: ICoord2D,

    // Unit tracking
    goal_unit_id: ObjectID, // The objectID of the ground unit whose goal this is
    pos_unit_id: ObjectID,  // The objectID of the ground unit that is occupying this cell
    goal_aircraft_id: ObjectID, // The objectID of the aircraft whose goal this is

    // Obstacle tracking
    obstacle_id: ObjectID, // the object ID who overlaps this cell

    // Bit flags (packed into single u32 to match C++)
    flags: u32,
}

// Bit flag constants
const FLAG_IS_FREE: u32 = 1 << 0;
const FLAG_BLOCKED_BY_ALLY: u32 = 1 << 1;
const FLAG_OBSTACLE_IS_FENCE: u32 = 1 << 2;
const FLAG_OBSTACLE_IS_TRANSPARENT: u32 = 1 << 3;
const FLAG_OPEN: u32 = 1 << 4;
const FLAG_CLOSED: u32 = 1 << 5;

impl PathfindCellInfo {
    /// Create a new PathfindCellInfo
    pub fn new(pos: &ICoord2D) -> Self {
        Self {
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
            flags: FLAG_IS_FREE,
        }
    }

    /// Get position
    pub fn get_pos(&self) -> &ICoord2D {
        &self.pos
    }

    /// Set position
    pub fn set_pos(&mut self, pos: &ICoord2D) {
        self.pos = *pos;
    }

    /// Get total cost (f = g + h)
    pub fn get_total_cost(&self) -> u16 {
        self.total_cost
    }

    /// Set total cost
    pub fn set_total_cost(&mut self, cost: u16) {
        self.total_cost = cost;
    }

    /// Get cost so far (g)
    pub fn get_cost_so_far(&self) -> u16 {
        self.cost_so_far
    }

    /// Set cost so far
    pub fn set_cost_so_far(&mut self, cost: u16) {
        self.cost_so_far = cost;
    }

    /// Get goal unit ID
    pub fn get_goal_unit_id(&self) -> ObjectID {
        self.goal_unit_id
    }

    /// Set goal unit ID
    pub fn set_goal_unit_id(&mut self, id: ObjectID) {
        self.goal_unit_id = id;
    }

    /// Get position unit ID
    pub fn get_pos_unit_id(&self) -> ObjectID {
        self.pos_unit_id
    }

    /// Set position unit ID
    pub fn set_pos_unit_id(&mut self, id: ObjectID) {
        self.pos_unit_id = id;
    }

    /// Get goal aircraft ID
    pub fn get_goal_aircraft_id(&self) -> ObjectID {
        self.goal_aircraft_id
    }

    /// Set goal aircraft ID
    pub fn set_goal_aircraft_id(&mut self, id: ObjectID) {
        self.goal_aircraft_id = id;
    }

    /// Get obstacle ID
    pub fn get_obstacle_id(&self) -> ObjectID {
        self.obstacle_id
    }

    /// Set obstacle ID
    pub fn set_obstacle_id(&mut self, id: ObjectID) {
        self.obstacle_id = id;
    }

    /// Check if cell is free
    pub fn is_free(&self) -> bool {
        (self.flags & FLAG_IS_FREE) != 0
    }

    /// Set free status
    pub fn set_free(&mut self, free: bool) {
        if free {
            self.flags |= FLAG_IS_FREE;
        } else {
            self.flags &= !FLAG_IS_FREE;
        }
    }

    /// Check if blocked by ally
    pub fn is_blocked_by_ally(&self) -> bool {
        (self.flags & FLAG_BLOCKED_BY_ALLY) != 0
    }

    /// Set blocked by ally
    pub fn set_blocked_by_ally(&mut self, blocked: bool) {
        if blocked {
            self.flags |= FLAG_BLOCKED_BY_ALLY;
        } else {
            self.flags &= !FLAG_BLOCKED_BY_ALLY;
        }
    }

    /// Check if obstacle is fence
    pub fn is_obstacle_fence(&self) -> bool {
        (self.flags & FLAG_OBSTACLE_IS_FENCE) != 0
    }

    /// Set obstacle is fence
    pub fn set_obstacle_is_fence(&mut self, is_fence: bool) {
        if is_fence {
            self.flags |= FLAG_OBSTACLE_IS_FENCE;
        } else {
            self.flags &= !FLAG_OBSTACLE_IS_FENCE;
        }
    }

    /// Check if obstacle is transparent
    pub fn is_obstacle_transparent(&self) -> bool {
        (self.flags & FLAG_OBSTACLE_IS_TRANSPARENT) != 0
    }

    /// Set obstacle is transparent
    pub fn set_obstacle_is_transparent(&mut self, transparent: bool) {
        if transparent {
            self.flags |= FLAG_OBSTACLE_IS_TRANSPARENT;
        } else {
            self.flags &= !FLAG_OBSTACLE_IS_TRANSPARENT;
        }
    }

    /// Check if cell is on open list
    pub fn is_open(&self) -> bool {
        (self.flags & FLAG_OPEN) != 0
    }

    /// Set open list status
    pub fn set_open(&mut self, open: bool) {
        if open {
            self.flags |= FLAG_OPEN;
        } else {
            self.flags &= !FLAG_OPEN;
        }
    }

    /// Check if cell is on closed list
    pub fn is_closed(&self) -> bool {
        (self.flags & FLAG_CLOSED) != 0
    }

    /// Set closed list status
    pub fn set_closed(&mut self, closed: bool) {
        if closed {
            self.flags |= FLAG_CLOSED;
        } else {
            self.flags &= !FLAG_CLOSED;
        }
    }

    /// Clear path parent.
    pub fn clear_path_parent(&mut self) {
        self.path_parent = None;
    }

    /// Reset all values for reuse
    pub fn reset(&mut self, pos: &ICoord2D) {
        self.next_open = None;
        self.prev_open = None;
        self.path_parent = None;
        self.total_cost = 0;
        self.cost_so_far = 0;
        self.pos = *pos;
        self.goal_unit_id = crate::common::INVALID_ID;
        self.pos_unit_id = crate::common::INVALID_ID;
        self.goal_aircraft_id = crate::common::INVALID_ID;
        self.obstacle_id = crate::common::INVALID_ID;
        self.flags = FLAG_IS_FREE;
    }
}

impl Default for PathfindCellInfo {
    fn default() -> Self {
        Self::new(&ICoord2D::new(0, 0))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CellKey {
    index: usize,
    generation: u64,
}

/// Identity valid only within the pool scope that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PathfindCellId<'scope> {
    key: CellKey,
    // Invariance prevents identities from different scopes being mixed.
    brand: PhantomData<fn(&'scope mut ()) -> &'scope mut ()>,
}

struct CellEntry {
    info: PathfindCellInfo,
    generation: u64,
    allocated: bool,
}

/// Reusable storage for the standalone cell-info facade.
pub struct PathfindCellInfoPool {
    pool: Vec<CellEntry>,
    free_list: Vec<usize>,
}

/// Exclusive access to a pool and its scoped cell identities.
pub struct PathfindCellInfoScope<'scope> {
    pool: &'scope mut PathfindCellInfoPool,
    brand: PhantomData<fn(&'scope mut ()) -> &'scope mut ()>,
}

impl PathfindCellInfoPool {
    pub fn new() -> Self {
        Self { pool: Vec::new(), free_list: Vec::new() }
    }

    /// Identities cannot escape this closure or be used with another pool.
    ///
    /// ```compile_fail
    /// use gamelogic::path::PathfindCellInfoPool;
    /// use gamelogic::common::ICoord2D;
    /// let (mut first, mut second) = (PathfindCellInfoPool::new(), PathfindCellInfoPool::new());
    /// first.with_scope(|a| second.with_scope(|b| {
    ///     let foreign = a.allocate(&ICoord2D::new(0, 0));
    ///     b.get(foreign);
    /// }));
    /// ```
    pub fn with_scope<R>(&mut self, f: impl for<'scope> FnOnce(&mut PathfindCellInfoScope<'scope>) -> R) -> R {
        f(&mut PathfindCellInfoScope { pool: self, brand: PhantomData })
    }
}

impl Default for PathfindCellInfoPool {
    fn default() -> Self {
        Self::new()
    }
}

impl<'scope> PathfindCellInfoScope<'scope> {
    fn id(key: CellKey) -> PathfindCellId<'scope> {
        PathfindCellId { key, brand: PhantomData }
    }

    fn valid(&self, key: CellKey) -> bool {
        self.pool.pool.get(key.index)
            .is_some_and(|slot| slot.allocated && slot.generation == key.generation)
    }

    pub fn allocate(&mut self, pos: &ICoord2D) -> PathfindCellId<'scope> {
        let index = if let Some(index) = self.pool.free_list.pop() {
            let slot = &mut self.pool.pool[index];
            slot.info.reset(pos);
            slot.allocated = true;
            index
        } else {
            let index = self.pool.pool.len();
            self.pool.pool.push(CellEntry {
                info: PathfindCellInfo::new(pos),
                generation: 0,
                allocated: true,
            });
            index
        };
        Self::id(CellKey { index, generation: self.pool.pool[index].generation })
    }

    pub fn get(&self, id: PathfindCellId<'scope>) -> Option<&PathfindCellInfo> {
        self.valid(id.key).then(|| &self.pool.pool[id.key.index].info)
    }

    fn get_mut(&mut self, id: PathfindCellId<'scope>) -> Option<&mut PathfindCellInfo> {
        if self.valid(id.key) {
            Some(&mut self.pool.pool[id.key.index].info)
        } else {
            None
        }
    }

    pub fn set_costs(&mut self, id: PathfindCellId<'scope>, cost_so_far: u16, total_cost: u16) -> bool {
        if let Some(info) = self.get_mut(id) {
            info.set_cost_so_far(cost_so_far);
            info.set_total_cost(total_cost);
            true
        } else {
            false
        }
    }

    pub fn set_open(&mut self, id: PathfindCellId<'scope>, open: bool) -> bool {
        if let Some(info) = self.get_mut(id) {
            info.set_open(open);
            true
        } else {
            false
        }
    }

    pub fn set_closed(&mut self, id: PathfindCellId<'scope>, closed: bool) -> bool {
        if let Some(info) = self.get_mut(id) {
            info.set_closed(closed);
            true
        } else {
            false
        }
    }

    /// Release a live cell; stale identities are rejected, including after reuse.
    pub fn release(&mut self, id: PathfindCellId<'scope>) -> bool {
        if !self.valid(id.key) {
            return false;
        }
        let slot = &mut self.pool.pool[id.key.index];
        slot.allocated = false;
        // Retire the slot rather than wrapping its generation.
        if let Some(next) = slot.generation.checked_add(1) {
            slot.generation = next;
            self.pool.free_list.push(id.key.index);
        }
        true
    }

    pub fn clear(&mut self) {
        for index in 0..self.pool.pool.len() {
            let slot = &self.pool.pool[index];
            if slot.allocated {
                let id = Self::id(CellKey { index, generation: slot.generation });
                self.release(id);
            }
        }
    }

    fn link(&self, id: PathfindCellId<'scope>, select: impl FnOnce(&PathfindCellInfo) -> Option<CellKey>) -> Option<PathfindCellId<'scope>> {
        let key = select(self.get(id)?)?;
        self.valid(key).then(|| Self::id(key))
    }

    fn set_link(&mut self, id: PathfindCellId<'scope>, target: Option<PathfindCellId<'scope>>, select: impl FnOnce(&mut PathfindCellInfo) -> &mut Option<CellKey>) -> bool {
        if target.is_some_and(|target| !self.valid(target.key)) {
            return false;
        }
        if let Some(info) = self.get_mut(id) {
            *select(info) = target.map(|target| target.key);
            true
        } else {
            false
        }
    }

    pub fn path_parent(&self, id: PathfindCellId<'scope>) -> Option<PathfindCellId<'scope>> {
        self.link(id, |info| info.path_parent)
    }
    pub fn set_path_parent(&mut self, id: PathfindCellId<'scope>, parent: Option<PathfindCellId<'scope>>) -> bool {
        self.set_link(id, parent, |info| &mut info.path_parent)
    }
    pub fn next_open(&self, id: PathfindCellId<'scope>) -> Option<PathfindCellId<'scope>> {
        self.link(id, |info| info.next_open)
    }
    pub fn set_next_open(&mut self, id: PathfindCellId<'scope>, next: Option<PathfindCellId<'scope>>) -> bool {
        self.set_link(id, next, |info| &mut info.next_open)
    }
    pub fn prev_open(&self, id: PathfindCellId<'scope>) -> Option<PathfindCellId<'scope>> {
        self.link(id, |info| info.prev_open)
    }
    pub fn set_prev_open(&mut self, id: PathfindCellId<'scope>, prev: Option<PathfindCellId<'scope>>) -> bool {
        self.set_link(id, prev, |info| &mut info.prev_open)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pathfind_cell_info_creation() {
        let pos = ICoord2D::new(10, 20);
        let info = PathfindCellInfo::new(&pos);

        assert_eq!(info.get_pos(), &pos);
        assert_eq!(info.get_total_cost(), 0);
        assert_eq!(info.get_cost_so_far(), 0);
        assert_eq!(info.get_goal_unit_id(), crate::common::INVALID_ID);
        assert!(info.is_free());
        assert!(!info.is_blocked_by_ally());
        assert!(!info.is_open());
        assert!(!info.is_closed());
    }

    #[test]
    fn test_pathfind_cell_info_costs() {
        let pos = ICoord2D::new(0, 0);
        let mut info = PathfindCellInfo::new(&pos);

        info.set_cost_so_far(100);
        info.set_total_cost(150);

        assert_eq!(info.get_cost_so_far(), 100);
        assert_eq!(info.get_total_cost(), 150);
    }

    #[test]
    fn test_pathfind_cell_info_flags() {
        let pos = ICoord2D::new(0, 0);
        let mut info = PathfindCellInfo::new(&pos);

        assert!(info.is_free());
        assert!(!info.is_blocked_by_ally());

        info.set_blocked_by_ally(true);
        assert!(info.is_blocked_by_ally());

        info.set_open(true);
        assert!(info.is_open());
        assert!(!info.is_closed());

        info.set_closed(true);
        assert!(info.is_closed());
    }

    #[test]
    fn pool_links_and_reuse_are_scoped() {
        let mut pool = PathfindCellInfoPool::new();
        pool.with_scope(|cells| {
            let a = cells.allocate(&ICoord2D::new(1, 0));
            let b = cells.allocate(&ICoord2D::new(2, 0));
            let c = cells.allocate(&ICoord2D::new(3, 0));
            assert!(cells.set_costs(a, 10, 20));
            assert!(cells.set_open(a, true));
            assert_eq!(cells.get(a).unwrap().get_total_cost(), 20);
            assert!(cells.get(a).unwrap().is_open());
            assert!(cells.set_next_open(a, Some(b)));
            assert!(cells.set_prev_open(b, Some(a)));
            assert!(cells.set_next_open(b, Some(c)));
            assert!(cells.set_prev_open(c, Some(b)));
            assert!(cells.set_path_parent(c, Some(b)));
            assert!(cells.set_path_parent(b, Some(a)));
            assert_eq!(cells.next_open(a), Some(b));
            assert_eq!(cells.prev_open(c), Some(b));
            assert_eq!(cells.path_parent(c).and_then(|id| cells.path_parent(id)), Some(a));

            // Remove the middle list member, preserving its neighbours.
            assert!(cells.set_next_open(a, Some(c)));
            assert!(cells.set_prev_open(c, Some(a)));
            assert!(cells.release(b));
            assert_eq!(cells.next_open(a), Some(c));
            assert_eq!(cells.prev_open(c), Some(a));
            assert_eq!(cells.path_parent(c), None);
            assert!(cells.get(b).is_none());
            assert!(!cells.release(b));
            let reused = cells.allocate(&ICoord2D::new(4, 0));
            assert_ne!(b, reused);
            assert_eq!(cells.get(reused).unwrap().get_pos(), &ICoord2D::new(4, 0));
            assert_eq!(cells.path_parent(c), None);
            assert!(!cells.set_next_open(a, Some(b)));
            cells.clear();
            assert!(cells.get(a).is_none());
            let after_clear = cells.allocate(&ICoord2D::new(5, 0));
            assert_ne!(a, after_clear);
        });
    }
}
