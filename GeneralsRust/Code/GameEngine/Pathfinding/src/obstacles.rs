// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// Obstacle annotation: which object owns a cell and whether that obstacle is
// a fence or transparent (C++ PathfindCellInfo obstacleID / m_obstacleIsFence /
// m_obstacleIsTransparent).

use crate::astar::AStarPathfinder;
use crate::cell::{GridCoord, PathfindCellType, PathfindLayerEnum};

impl AStarPathfinder {
    /// C++ PathfindCell::getObstacleID.
    pub fn get_cell_obstacle_id(&self, coord: GridCoord) -> Option<u32> {
        self.obstacle_owners
            .get(&Self::obstacle_key(coord, PathfindLayerEnum::Ground))
            .copied()
    }

    pub fn set_cell_obstacle_id(
        &mut self,
        coord: GridCoord,
        obj_id: u32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        self.set_cell_obstacle_id_on_layer(
            coord,
            PathfindLayerEnum::Ground,
            obj_id,
            is_fence,
            is_transparent,
        );
    }

    pub fn set_cell_obstacle_id_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        obj_id: u32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, layer) {
            cell.set_type(PathfindCellType::Obstacle);
            if Self::is_elevated_layer(layer) {
                cell.set_layer(layer);
            }
        }
        let key = Self::obstacle_key(coord, layer);
        self.obstacle_owners.insert(key, obj_id);
        if is_fence {
            self.obstacle_fence.insert(key);
        } else {
            self.obstacle_fence.remove(&key);
        }
        if is_transparent {
            self.obstacle_transparent.insert(key);
        } else {
            self.obstacle_transparent.remove(&key);
        }
    }

    /// C++ PathfindCell::isObstacleTransparent.
    pub fn is_obstacle_transparent(&self, coord: GridCoord) -> bool {
        self.obstacle_transparent
            .contains(&Self::obstacle_key(coord, PathfindLayerEnum::Ground))
    }

    pub fn is_obstacle_fence(&self, coord: GridCoord) -> bool {
        self.obstacle_fence
            .contains(&Self::obstacle_key(coord, PathfindLayerEnum::Ground))
    }

    /// Clear obstacle if it matches obj_id (C++ removeObstacle).
    pub fn clear_cell_obstacle_id(&mut self, coord: GridCoord, obj_id: u32) -> bool {
        let key = Self::obstacle_key(coord, PathfindLayerEnum::Ground);
        match self.obstacle_owners.get(&key).copied() {
            Some(owner) if owner == obj_id => {
                self.obstacle_owners.remove(&key);
                self.obstacle_fence.remove(&key);
                self.obstacle_transparent.remove(&key);
                if let Some(cell) = self.get_cell_mut(coord) {
                    cell.set_type(PathfindCellType::Clear);
                }
                true
            }
            _ => false,
        }
    }
}
