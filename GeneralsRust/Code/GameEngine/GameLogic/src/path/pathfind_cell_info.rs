//! PathfindCellInfo implementation matching C++ PathfindCellInfo class
#![allow(missing_docs)]
//!
//! Memory-efficient cell information structure for A* pathfinding.
//! This is allocated only when needed to save memory.

use super::*;
use crate::common::*;
use std::ptr;

/// PathfindCellInfo structure matching C++ PathfindCellInfo class
#[derive(Debug)]
pub struct PathfindCellInfo {
    // A* search linked lists (using raw pointers to match C++ behavior)
    next_open: *mut PathfindCellInfo, // for A* "open" list, shared by closed list
    prev_open: *mut PathfindCellInfo, // for A* "open" list, shared by closed list

    // A* search parent tracking
    path_parent: *mut PathfindCellInfo, // "parent" cell from pathfinder

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
            next_open: ptr::null_mut(),
            prev_open: ptr::null_mut(),
            path_parent: ptr::null_mut(),
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

    /// Get path parent (unsafe due to raw pointer)
    pub fn get_path_parent(&self) -> Option<&PathfindCellInfo> {
        if self.path_parent.is_null() {
            None
        } else {
            Some(unsafe { &*self.path_parent })
        }
    }

    /// Set path parent
    pub fn set_path_parent(&mut self, parent: *mut PathfindCellInfo) {
        self.path_parent = parent;
    }

    /// Clear path parent
    pub fn clear_path_parent(&mut self) {
        self.path_parent = ptr::null_mut();
    }

    /// Get next open list item (unsafe due to raw pointer)
    pub fn get_next_open(&self) -> Option<&PathfindCellInfo> {
        if self.next_open.is_null() {
            None
        } else {
            Some(unsafe { &*self.next_open })
        }
    }

    /// Set next open list item
    pub fn set_next_open(&mut self, next: *mut PathfindCellInfo) {
        self.next_open = next;
    }

    /// Get previous open list item (unsafe due to raw pointer)
    pub fn get_prev_open(&self) -> Option<&PathfindCellInfo> {
        if self.prev_open.is_null() {
            None
        } else {
            Some(unsafe { &*self.prev_open })
        }
    }

    /// Set previous open list item
    pub fn set_prev_open(&mut self, prev: *mut PathfindCellInfo) {
        self.prev_open = prev;
    }

    /// Reset all values for reuse
    pub fn reset(&mut self, pos: &ICoord2D) {
        self.next_open = ptr::null_mut();
        self.prev_open = ptr::null_mut();
        self.path_parent = ptr::null_mut();
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

// Memory pool management for PathfindCellInfo (simplified version of C++ implementation)
pub struct PathfindCellInfoPool {
    pool: Vec<Box<PathfindCellInfo>>,
    free_list: Vec<*mut PathfindCellInfo>,
}

impl PathfindCellInfoPool {
    /// Create a new pool
    pub fn new() -> Self {
        Self {
            pool: Vec::new(),
            free_list: Vec::new(),
        }
    }

    /// Allocate a PathfindCellInfo from the pool
    pub fn allocate(&mut self, pos: &ICoord2D) -> *mut PathfindCellInfo {
        if let Some(ptr) = self.free_list.pop() {
            unsafe {
                (*ptr).reset(pos);
                ptr
            }
        } else {
            let mut info = Box::new(PathfindCellInfo::new(pos));
            let ptr = info.as_mut() as *mut PathfindCellInfo;
            self.pool.push(info);
            ptr
        }
    }

    /// Release a PathfindCellInfo back to the pool
    pub fn release(&mut self, ptr: *mut PathfindCellInfo) {
        if !ptr.is_null() {
            self.free_list.push(ptr);
        }
    }

    /// Clear all allocations
    pub fn clear(&mut self) {
        self.pool.clear();
        self.free_list.clear();
    }
}

unsafe impl Send for PathfindCellInfoPool {}
unsafe impl Sync for PathfindCellInfoPool {}

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
    fn test_pathfind_cell_info_pool() {
        let mut pool = PathfindCellInfoPool::new();
        let pos = ICoord2D::new(5, 10);

        let ptr = pool.allocate(&pos);
        assert!(!ptr.is_null());

        unsafe {
            assert_eq!((*ptr).get_pos(), &pos);
        }

        pool.release(ptr);

        // Allocate again should reuse the same object
        let ptr2 = pool.allocate(&ICoord2D::new(15, 25));
        assert_eq!(ptr, ptr2); // Should be the same pointer

        unsafe {
            assert_eq!((*ptr2).get_pos(), &ICoord2D::new(15, 25));
        }
    }
}
