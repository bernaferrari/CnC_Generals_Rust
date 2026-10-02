// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// Zone manager state: the coarse ZONE_BLOCK_SIZE passable-block store
// (C++ PathfindZoneManager).

use crate::astar::AStarPathfinder;
use crate::cell::GridCoord;

impl AStarPathfinder {
    /// Stamp obstacle object id / fence flag on a cell (C++ setTypeAsObstacle).
    /// C++ cell connectLayer stamp for bridge/wall transitions.
    /// C++ ZONE_BLOCK_SIZE for hierarchical passable blocks.
    pub const ZONE_BLOCK_SIZE: i32 = 10;

    /// C++ `PathfindZoneManager::setPassable` — marks the whole zone block.
    pub fn set_zone_passable(&mut self, coord: GridCoord, passable: bool) {
        let bx = coord.x.div_euclid(Self::ZONE_BLOCK_SIZE);
        let by = coord.y.div_euclid(Self::ZONE_BLOCK_SIZE);
        let key = (bx, by);
        if passable {
            self.zone_impassable_blocks.remove(&key);
        } else {
            self.zone_impassable_blocks.insert(key);
        }
    }

    pub fn clear_zone_passable_flags(&mut self) {
        self.zone_impassable_blocks.clear();
    }

    /// Mark all blocks impassable (hierarchical closed until expanded).
    pub fn mark_all_zone_blocks_impassable(&mut self) {
        self.zone_impassable_blocks.clear();
        let bx_max = (self.width as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        let by_max = (self.height as i32 + Self::ZONE_BLOCK_SIZE - 1) / Self::ZONE_BLOCK_SIZE;
        for bx in 0..bx_max {
            for by in 0..by_max {
                self.zone_impassable_blocks.insert((bx, by));
            }
        }
    }

    /// C++ `PathfindZoneManager::isPassable`.
    #[inline]
    pub fn is_zone_passable(&self, coord: GridCoord) -> bool {
        let bx = coord.x.div_euclid(Self::ZONE_BLOCK_SIZE);
        let by = coord.y.div_euclid(Self::ZONE_BLOCK_SIZE);
        !self.zone_impassable_blocks.contains(&(bx, by))
    }

    /// C++ `clipIsPassable` — false when off-map; else block flag.
    #[inline]
    pub fn clip_is_zone_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        if cell_x < 0 || cell_y < 0 || cell_x >= self.width as i32 || cell_y >= self.height as i32 {
            return false;
        }
        self.is_zone_passable(GridCoord::new(cell_x, cell_y))
    }
}
