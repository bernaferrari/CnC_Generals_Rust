//! Collision detection and obstacle management mirroring the classic C++ implementation.
#![allow(missing_docs)]
//!
//! The collision map is responsible for writing static obstacles (structures,
//! walls, bridges) and dynamic units into the pathfinding grid.  The vessel is
//! deliberately lightweight: the authoritative terrain classification lives in
//! the higher-level systems, while the collision map tracks occupancy and feeds
//! that information back into the pathfinder.

use super::*;
use crate::common::{Coord3D, ICoord2D, IRegion2D, ObjectID};
use crate::path::LocomotorSurfaceTypeMask;
use std::collections::HashMap;

#[derive(Debug)]
pub struct CollisionMap {
    grid: Vec<Vec<Cell>>,
    extent: IRegion2D,
    dynamic_index: HashMap<ObjectID, CellKey>,
}

#[derive(Debug, Default, Clone)]
pub struct Cell {
    static_obstacles: Vec<ObjectID>,
    dynamic_units: Vec<ObjectID>,
    terrain: PathfindCellType,
}

#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd, Hash, Debug)]
struct CellKey {
    x: i32,
    y: i32,
}

impl From<ICoord2D> for CellKey {
    fn from(cell: ICoord2D) -> Self {
        Self {
            x: cell.x,
            y: cell.y,
        }
    }
}

impl CellKey {
    fn to_coord(self) -> ICoord2D {
        ICoord2D::new(self.x, self.y)
    }
}

impl CollisionMap {
    pub fn new() -> Self {
        Self {
            grid: Vec::new(),
            extent: IRegion2D::default(),
            dynamic_index: HashMap::new(),
        }
    }

    /// Initialise the collision grid for the supplied bounds (inclusive in grid space).
    pub fn initialize(&mut self, bounds: &IRegion2D) {
        let width = (bounds.hi.x - bounds.lo.x + 1).max(1) as usize;
        let height = (bounds.hi.y - bounds.lo.y + 1).max(1) as usize;
        self.extent = *bounds;
        self.grid = vec![vec![Cell::default(); height]; width];
        self.dynamic_index.clear();
    }

    /// Write a static obstacle across the requested bounds.
    pub fn add_static_obstacle(&mut self, obj_id: ObjectID, bounds: &IRegion2D) {
        let cells: Vec<ICoord2D> = self.iter_cells(bounds).collect();
        for cell in cells {
            if let Some((x, y)) = self.cell_indices(cell) {
                let entry = &mut self.grid[x][y];
                if !entry.static_obstacles.contains(&obj_id) {
                    entry.static_obstacles.push(obj_id);
                }
                entry.terrain = PathfindCellType::Obstacle;
            }
        }
    }

    /// Remove a static obstacle from every cell it occupies.
    pub fn remove_static_obstacle(&mut self, obj_id: ObjectID) {
        for column in &mut self.grid {
            for cell in column {
                cell.static_obstacles.retain(|id| *id != obj_id);
                if cell.static_obstacles.is_empty() && cell.dynamic_units.is_empty() {
                    cell.terrain = PathfindCellType::Clear;
                }
            }
        }
    }

    /// Add a moving unit into the collision grid.
    pub fn add_dynamic_unit(&mut self, obj_id: ObjectID, pos: &Coord3D) {
        let cell = self.world_to_cell(pos);
        if let Some((x, y)) = self.cell_indices(cell) {
            let entry = &mut self.grid[x][y];
            if !entry.dynamic_units.contains(&obj_id) {
                entry.dynamic_units.push(obj_id);
            }
            self.dynamic_index.insert(obj_id, CellKey::from(cell));
        }
    }

    /// Remove a dynamic unit entirely.
    pub fn remove_dynamic_unit(&mut self, obj_id: ObjectID) {
        if let Some(key) = self.dynamic_index.remove(&obj_id) {
            if let Some((x, y)) = self.cell_indices(key.to_coord()) {
                let entry = &mut self.grid[x][y];
                entry.dynamic_units.retain(|id| *id != obj_id);
            }
        }
    }

    /// Update a dynamic unit's position.
    pub fn update_dynamic_unit(&mut self, obj_id: ObjectID, new_pos: &Coord3D) {
        let new_cell = self.world_to_cell(new_pos);
        let new_key = CellKey::from(new_cell);
        if self
            .dynamic_index
            .get(&obj_id)
            .copied()
            .map(|key| key == new_key)
            .unwrap_or(false)
        {
            return;
        }

        self.remove_dynamic_unit(obj_id);
        self.add_dynamic_unit(obj_id, new_pos);
    }

    /// Query whether a position is blocked by any static obstacle or dynamic unit.
    pub fn is_blocked(
        &self,
        pos: &Coord3D,
        unit_radius: f32,
        ignore_unit: Option<ObjectID>,
    ) -> bool {
        let center = CellKey::from(self.world_to_cell(pos));
        let radius_cells = (unit_radius / PATHFIND_CELL_SIZE_F).ceil() as i32;

        for dx in -radius_cells..=radius_cells {
            for dy in -radius_cells..=radius_cells {
                let neighbor = CellKey {
                    x: center.x + dx,
                    y: center.y + dy,
                };
                let Some((x, y)) = self.cell_indices(neighbor.to_coord()) else {
                    continue;
                };
                let entry = &self.grid[x][y];
                if !entry.static_obstacles.is_empty() {
                    return true;
                }
                if entry
                    .dynamic_units
                    .iter()
                    .any(|id| Some(*id) != ignore_unit)
                {
                    return true;
                }
            }
        }
        false
    }

    /// Check whether a straight-line segment is clear of obstacles.
    pub fn line_clear(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        unit_radius: f32,
        ignore_unit: Option<ObjectID>,
    ) -> bool {
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let dz = end.z - start.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();

        if distance <= f32::EPSILON {
            return !self.is_blocked(start, unit_radius, ignore_unit);
        }

        let step_length = (PATHFIND_CELL_SIZE_F * 0.5).max(0.1);
        let steps = (distance / step_length).ceil() as i32;
        let steps = steps.max(1);

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let sample = Coord3D::new(start.x + dx * t, start.y + dy * t, start.z + dz * t);
            if self.is_blocked(&sample, unit_radius, ignore_unit) {
                return false;
            }
        }

        true
    }

    /// Collect all static obstacles intersecting the supplied bounds.
    pub fn get_obstacles_in_area(&self, bounds: &IRegion2D) -> Vec<ObjectID> {
        let mut obstacles = Vec::new();
        for cell in self.iter_cells(bounds) {
            if let Some((x, y)) = self.cell_indices(cell) {
                obstacles.extend(&self.grid[x][y].static_obstacles);
            }
        }
        obstacles.sort();
        obstacles.dedup();
        obstacles
    }

    fn world_to_cell(&self, pos: &Coord3D) -> ICoord2D {
        world_to_grid(pos)
    }

    fn cell_indices(&self, cell: ICoord2D) -> Option<(usize, usize)> {
        let width = (self.extent.hi.x - self.extent.lo.x + 1) as i32;
        let height = (self.extent.hi.y - self.extent.lo.y + 1) as i32;
        let x = cell.x - self.extent.lo.x;
        let y = cell.y - self.extent.lo.y;
        if x >= 0 && x < width && y >= 0 && y < height {
            Some((x as usize, y as usize))
        } else {
            None
        }
    }

    fn iter_cells(&self, bounds: &IRegion2D) -> impl Iterator<Item = ICoord2D> + '_ {
        let lo_x = bounds.lo.x.max(self.extent.lo.x);
        let hi_x = bounds.hi.x.min(self.extent.hi.x);
        let lo_y = bounds.lo.y.max(self.extent.lo.y);
        let hi_y = bounds.hi.y.min(self.extent.hi.y);
        (lo_x..=hi_x).flat_map(move |x| (lo_y..=hi_y).map(move |y| ICoord2D::new(x, y)))
    }

    pub fn extent(&self) -> IRegion2D {
        self.extent
    }

    pub fn cell(&self, cell: ICoord2D) -> Option<&Cell> {
        self.cell_indices(cell).map(|(x, y)| &self.grid[x][y])
    }
}

impl Cell {
    pub(crate) fn has_static_obstacle(&self) -> bool {
        !self.static_obstacles.is_empty()
    }

    pub(crate) fn has_dynamic_units(&self) -> bool {
        !self.dynamic_units.is_empty()
    }
}

impl PassabilityQuery for CollisionMap {
    fn is_line_passable(
        &self,
        _surfaces: LocomotorSurfaceTypeMask,
        from: &Coord3D,
        to: &Coord3D,
        blocked: bool,
    ) -> bool {
        if blocked {
            return false;
        }
        let radius = PATHFIND_CELL_SIZE_F * 0.5;
        self.line_clear(from, to, radius, None)
    }

    fn is_ground_line_passable(
        &self,
        _crusher: bool,
        diameter: i32,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        let radius = (diameter as f32).max(PATHFIND_CELL_SIZE_F) * 0.5;
        self.line_clear(from, to, radius, None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_clear_detects_static_obstacle() {
        let mut map = CollisionMap::new();
        let bounds = IRegion2D {
            lo: ICoord2D::new(0, 0),
            hi: ICoord2D::new(4, 4),
        };
        map.initialize(&bounds);

        let obstacle_bounds = IRegion2D {
            lo: ICoord2D::new(1, 0),
            hi: ICoord2D::new(1, 0),
        };
        map.add_static_obstacle(1, &obstacle_bounds);

        let start = Coord3D::new(5.0, 5.0, 0.0);
        let end = Coord3D::new(25.0, 5.0, 0.0);

        assert!(
            !map.line_clear(&start, &end, PATHFIND_CELL_SIZE_F * 0.25, None),
            "Obstacle should block the segment"
        );
    }

    #[test]
    fn passability_query_respects_blocked_flag() {
        let map = CollisionMap::new();
        let from = Coord3D::new(0.0, 0.0, 0.0);
        let to = Coord3D::new(10.0, 0.0, 0.0);

        assert!(
            !map.is_line_passable(SURFACE_GROUND, &from, &to, true),
            "Blocked flag should force optimization to keep intermediate nodes"
        );
    }
}
