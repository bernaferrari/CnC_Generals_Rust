// pathfind_astar.rs
// A* Pathfinding Algorithm - Faithful C++ Port
// Reference: /GeneralsMD/Code/GameEngine/Source/GameLogic/AI/AIPathfind.cpp
//
// Stored-grid mutation: cell types, bridge/wall connect-layer stamps, pinch
// flags and the pinch recompute pass (C++ Pathfinder map editing entry points).

use crate::astar::AStarPathfinder;
use crate::cell::{GridCoord, PathfindCellType, PathfindLayerEnum};

impl AStarPathfinder {
    /// Set cell type at coordinates (LAYER_GROUND / C++ `m_map`).
    pub fn set_cell_type(&mut self, coord: GridCoord, cell_type: PathfindCellType) {
        if let Some(cell) = self.get_cell_mut(coord) {
            cell.set_type(cell_type);
        }
    }

    /// Write `ty` on `layer`. Ground goes to `m_map`; Top/other lazily allocates
    /// that layer's grid (C++ `m_layers[layer]`).
    pub fn set_cell_type_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        ty: PathfindCellType,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, layer) {
            cell.set_type(ty);
            if Self::is_elevated_layer(layer) {
                cell.set_layer(layer);
            }
        }
    }

    /// Stored type on `layer` with **no** C++ getCell fallback.
    ///
    /// Ground → `get_cell_type`. Elevated missing/OOB → `None`. Search still
    /// uses `get_cell_on_layer`, which falls back to ground when the elevated
    /// cell is missing or CELL_IMPASSABLE (AIPathfind.h:899-917).
    pub fn get_cell_type_on_layer(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
    ) -> Option<PathfindCellType> {
        if !Self::is_elevated_layer(layer) {
            return self.get_cell_type(coord);
        }
        self.layer_cell(coord, layer).map(|c| c.get_type())
    }

    pub fn set_cell_connect_layer(&mut self, coord: GridCoord, layer: PathfindLayerEnum) {
        self.set_cell_connect_layer_on_layer(coord, PathfindLayerEnum::Ground, layer);
    }

    pub fn get_cell_connect_layer(&self, coord: GridCoord) -> Option<PathfindLayerEnum> {
        self.get_cell_connect_layer_on_layer(coord, PathfindLayerEnum::Ground)
    }

    pub fn set_cell_connect_layer_on_layer(
        &mut self,
        coord: GridCoord,
        on_layer: PathfindLayerEnum,
        connect: PathfindLayerEnum,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, on_layer) {
            cell.set_connect_layer(connect);
        }
    }

    pub fn get_cell_connect_layer_on_layer(
        &self,
        coord: GridCoord,
        on_layer: PathfindLayerEnum,
    ) -> Option<PathfindLayerEnum> {
        // Stored connect_layer only — no getCell fallback (missing elevated → None).
        if Self::is_elevated_layer(on_layer) {
            return self
                .layer_cell(coord, on_layer)
                .map(|c| c.get_connect_layer());
        }
        self.get_cell(coord).map(|c| c.get_connect_layer())
    }

    /// C++ `Pathfinder::checkChangeLayers` — enqueue same-xy cell on connect layer.
    ///
    /// Returns extra neighbor coords (same x,y) when connectLayer is valid and not already
    /// represented by the normal ground neighbor set. Caller merges into open set.
    pub fn connect_layer_transition_coord(&self, coord: GridCoord) -> Option<GridCoord> {
        let cell = self.get_cell(coord)?;
        let cl = cell.get_connect_layer();
        if cl == PathfindLayerEnum::Invalid {
            return None;
        }
        // Transition stays at same indices; layer change is tracked externally.
        Some(coord)
    }

    /// Get cell type at coordinates.
    pub fn get_cell_type(&self, coord: GridCoord) -> Option<PathfindCellType> {
        self.get_cell(coord).map(|cell| cell.get_type())
    }

    /// Mark a cell as pinched (surrounded by obstacles)
    pub fn set_pinched(&mut self, coord: GridCoord, pinched: bool) {
        self.set_pinched_on_layer(coord, PathfindLayerEnum::Ground, pinched);
    }

    /// Get whether a cell is pinched.
    pub fn is_pinched(&self, coord: GridCoord) -> Option<bool> {
        self.is_pinched_on_layer(coord, PathfindLayerEnum::Ground)
    }

    pub fn set_pinched_on_layer(
        &mut self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
        pinched: bool,
    ) {
        if let Some(cell) = self.get_cell_mut_on_layer(coord, layer) {
            cell.set_pinched(pinched);
            if Self::is_elevated_layer(layer) {
                cell.set_layer(layer);
            }
        }
    }

    /// Stored pinch flag on `layer` (no getCell fallback). Search uses the
    /// resolved cell from `get_cell_on_layer` instead.
    pub fn is_pinched_on_layer(&self, coord: GridCoord, layer: PathfindLayerEnum) -> Option<bool> {
        if Self::is_elevated_layer(layer) {
            return self.layer_cell(coord, layer).map(|c| c.is_pinched());
        }
        self.get_cell(coord).map(|cell| cell.is_pinched())
    }

    pub fn refresh_pinched_cells_in_bounds(&mut self, lo: GridCoord, hi: GridCoord) {
        let min_x = lo.x.max(0);
        let min_y = lo.y.max(0);
        let max_x = hi.x.min(self.width as i32 - 1);
        let max_y = hi.y.min(self.height as i32 - 1);

        if min_x > max_x || min_y > max_y {
            return;
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let cell = &mut self.grid[x as usize][y as usize];
                if cell.get_type() == PathfindCellType::Impassable {
                    cell.set_type(PathfindCellType::Clear);
                }
                cell.set_pinched(false);
            }
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if self.grid[x as usize][y as usize].get_type() != PathfindCellType::Clear {
                    continue;
                }
                let mut total_count = 0;
                let mut orthogonal_count = 0;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                            continue;
                        }
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if self.grid[nx as usize][ny as usize].get_type() == PathfindCellType::Clear
                        {
                            total_count += 1;
                            if dx == 0 || dy == 0 {
                                orthogonal_count += 1;
                            }
                        }
                    }
                }
                if orthogonal_count < 2 || total_count < 4 {
                    self.grid[x as usize][y as usize].set_pinched(true);
                }
            }
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                let cell = &mut self.grid[x as usize][y as usize];
                if cell.is_pinched() && cell.get_type() == PathfindCellType::Clear {
                    cell.set_type(PathfindCellType::Impassable);
                    cell.set_pinched(false);
                }
            }
        }

        for x in min_x..=max_x {
            for y in min_y..=max_y {
                if self.grid[x as usize][y as usize].get_type() != PathfindCellType::Clear {
                    continue;
                }
                let mut obstacle_adjacent = false;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= self.width as i32 || ny >= self.height as i32 {
                            continue;
                        }
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if dx != 0 && dy != 0 {
                            continue;
                        }
                        if self.grid[nx as usize][ny as usize].get_type()
                            == PathfindCellType::Obstacle
                        {
                            obstacle_adjacent = true;
                            break;
                        }
                    }
                    if obstacle_adjacent {
                        break;
                    }
                }
                if obstacle_adjacent {
                    self.grid[x as usize][y as usize].set_pinched(true);
                }
            }
        }
    }
}
