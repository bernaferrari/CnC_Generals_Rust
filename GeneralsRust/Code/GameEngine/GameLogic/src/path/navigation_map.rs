//! Navigation map for terrain analysis and movement capabilities
//!
//! This module provides high-level navigation information,
//! including terrain classification and movement cost analysis.
#![allow(missing_docs)]

use super::*;
use crate::common::*;
use crate::locomotor::SURFACE_AIR;
use crate::physics::TerrainQuery;

/// Navigation map providing high-level pathfinding information
#[derive(Debug)]
pub struct NavigationMap {
    terrain_grid: Vec<Vec<TerrainInfo>>,
    extent: IRegion2D,
}

/// Terrain information for navigation
#[derive(Debug, Clone)]
pub struct TerrainInfo {
    pub terrain_type: PathfindCellType,
    pub movement_cost: u32,
    pub elevation: f32,
    pub slope: f32,
    pub pinched: bool,
}

impl Default for TerrainInfo {
    fn default() -> Self {
        Self {
            terrain_type: PathfindCellType::Clear,
            movement_cost: 10,
            elevation: 0.0,
            slope: 0.0,
            pinched: false,
        }
    }
}

impl NavigationMap {
    pub fn new() -> Self {
        Self {
            terrain_grid: Vec::new(),
            extent: IRegion2D::default(),
        }
    }

    /// Initialize navigation map for the supplied bounds.
    pub fn initialize(&mut self, bounds: &IRegion2D) {
        let width = (bounds.hi.x - bounds.lo.x + 1).max(1) as usize;
        let height = (bounds.hi.y - bounds.lo.y + 1).max(1) as usize;
        self.extent = *bounds;
        self.terrain_grid = vec![vec![TerrainInfo::default(); height]; width];
    }

    /// Update terrain information for area
    pub fn update_terrain_area(&mut self, bounds: &IRegion2D) {
        let terrain = crate::terrain::get_terrain_logic();
        let terrain_guard = terrain.read().ok();

        let cells: Vec<_> = self.iter_cells(bounds).collect();
        for cell in cells {
            let Some(info) = self.cell_info_mut(cell) else {
                continue;
            };

            if let Some(terrain_guard) = terrain_guard.as_ref() {
                let world = grid_to_world(&cell, crate::path::PathfindLayerEnum::Ground);
                info.elevation = terrain_guard.get_ground_height(world.x, world.y, None);

                let top_left_x = cell.x as f32 * crate::path::PATHFIND_CELL_SIZE_F;
                let top_left_y = cell.y as f32 * crate::path::PATHFIND_CELL_SIZE_F;
                let bottom_right_x = top_left_x + crate::path::PATHFIND_CELL_SIZE_F;
                let bottom_right_y = top_left_y + crate::path::PATHFIND_CELL_SIZE_F;

                let cliff = terrain_guard.is_cliff_cell(top_left_x, top_left_y);
                let underwater = terrain_guard.is_underwater(top_left_x, top_left_y, None, None)
                    || terrain_guard.is_underwater(top_left_x, bottom_right_y, None, None)
                    || terrain_guard.is_underwater(bottom_right_x, bottom_right_y, None, None)
                    || terrain_guard.is_underwater(bottom_right_x, top_left_y, None, None);

                let mut terrain_type = PathfindCellType::Clear;
                if cliff {
                    terrain_type = PathfindCellType::Cliff;
                }
                if underwater {
                    terrain_type = PathfindCellType::Water;
                }
                if let Some(bridge) = terrain_guard.find_bridge_at(&world) {
                    if bridge.get_bridge_info().cur_damage_state != BodyDamageType::Rubble {
                        terrain_type = PathfindCellType::Clear;
                    } else {
                        terrain_type = PathfindCellType::Impassable;
                    }
                } else if terrain_guard.is_on_bridge(&world).0 {
                    terrain_type = PathfindCellType::Clear;
                }
                info.terrain_type = terrain_type;
            }

            info.movement_cost = Self::default_movement_cost(info.terrain_type);
        }

        self.update_cliff_pinched();

        // Recompute slope after elevation updates so downstream queries see fresh values.
        let slope_cells: Vec<_> = self.iter_cells(bounds).collect();
        for cell in slope_cells {
            let slope = self.estimate_cell_slope(cell);
            if let Some(info) = self.cell_info_mut(cell) {
                info.slope = slope;
            }
        }
    }

    /// Get movement cost for terrain type and locomotor
    pub fn get_movement_cost(
        &self,
        terrain_type: PathfindCellType,
        locomotor_set: &LocomotorSet,
    ) -> u32 {
        // Match PathfindCell traversal costs for consistency with pathfinder.
        if locomotor_set.can_move_on_surface(SURFACE_AIR) {
            return 10;
        }
        match terrain_type {
            PathfindCellType::Clear => {
                if locomotor_set.can_move_on_surface(SURFACE_GROUND) {
                    10
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Water => {
                if locomotor_set.can_move_on_surface(SURFACE_WATER) {
                    15
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Cliff => {
                if locomotor_set.can_move_on_surface(SURFACE_CLIFF) {
                    24
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Rubble => {
                if locomotor_set.can_move_on_surface(SURFACE_RUBBLE) {
                    18
                } else if locomotor_set.is_crusher() {
                    12
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Obstacle => {
                if locomotor_set.is_crusher() {
                    30
                } else {
                    u32::MAX
                }
            }
            _ => u32::MAX,
        }
    }

    /// Check if terrain is passable for locomotor
    pub fn is_terrain_passable(
        &self,
        terrain_type: PathfindCellType,
        locomotor_set: &LocomotorSet,
    ) -> bool {
        self.get_movement_cost(terrain_type, locomotor_set) != u32::MAX
    }

    /// Get terrain info at position
    pub fn get_terrain_info(&self, pos: &Coord3D) -> Option<&TerrainInfo> {
        let cell = world_to_grid(pos);
        self.cell_info(cell)
    }

    /// Sample terrain info by grid cell.
    pub fn sample_cell(&self, cell: ICoord2D) -> Option<&TerrainInfo> {
        self.cell_info(cell)
    }

    /// Analyze terrain for pathfinding
    pub fn analyze_terrain_connectivity(&self) -> Vec<(ZoneStorageType, Vec<ZoneStorageType>)> {
        let width = (self.extent.hi.x - self.extent.lo.x + 1).max(1) as usize;
        let height = (self.extent.hi.y - self.extent.lo.y + 1).max(1) as usize;
        let mut zones = vec![vec![0u16; height]; width];
        let mut next_zone: ZoneStorageType = 1;

        for x in 0..width {
            for y in 0..height {
                if zones[x][y] != 0 {
                    continue;
                }
                let terrain = self.terrain_grid[x][y].terrain_type;
                if !Self::is_zone_eligible(terrain) {
                    continue;
                }
                if next_zone == ZoneStorageType::MAX {
                    break;
                }
                self.flood_fill_zone(
                    &mut zones,
                    ICoord2D::new(self.extent.lo.x + x as i32, self.extent.lo.y + y as i32),
                    next_zone,
                    terrain,
                );
                next_zone = next_zone.saturating_add(1);
            }
        }

        let mut adjacency: std::collections::HashMap<
            ZoneStorageType,
            std::collections::HashSet<ZoneStorageType>,
        > = std::collections::HashMap::new();
        for x in 0..width {
            for y in 0..height {
                let zone = zones[x][y];
                if zone == 0 {
                    continue;
                }
                if x + 1 < width {
                    let other = zones[x + 1][y];
                    if other != 0 && other != zone {
                        adjacency.entry(zone).or_default().insert(other);
                        adjacency.entry(other).or_default().insert(zone);
                    }
                }
                if y + 1 < height {
                    let other = zones[x][y + 1];
                    if other != 0 && other != zone {
                        adjacency.entry(zone).or_default().insert(other);
                        adjacency.entry(other).or_default().insert(zone);
                    }
                }
            }
        }

        let mut result: Vec<(ZoneStorageType, Vec<ZoneStorageType>)> = adjacency
            .into_iter()
            .map(|(zone, neighbors)| {
                let mut list: Vec<ZoneStorageType> = neighbors.into_iter().collect();
                list.sort_unstable();
                (zone, list)
            })
            .collect();
        result.sort_by_key(|(zone, _)| *zone);
        result
    }

    /// Assign elevation for the supplied grid cell (in world units).
    pub fn set_elevation(&mut self, cell: ICoord2D, elevation: f32) {
        if let Some(info) = self.cell_info_mut(cell) {
            info.elevation = elevation;
        }
    }

    /// Assign terrain type for a cell.
    pub fn set_terrain_type(&mut self, cell: ICoord2D, terrain: PathfindCellType) {
        let movement_cost = Self::default_movement_cost(terrain);
        if let Some(info) = self.cell_info_mut(cell) {
            info.terrain_type = terrain;
            info.movement_cost = movement_cost;
        }
    }

    /// Convert the navigation grid into immutable samples used by the pathfinder.
    pub fn to_samples(&self, max_ground_slope: f32) -> NavigationSamples {
        let width = (self.extent.hi.x - self.extent.lo.x + 1).max(1) as usize;
        let height = (self.extent.hi.y - self.extent.lo.y + 1).max(1) as usize;

        let mut grid = vec![vec![TerrainSample::default(); height]; width];

        for x in 0..width {
            for y in 0..height {
                let info = &self.terrain_grid[x][y];
                let mut sample = TerrainSample {
                    terrain_type: info.terrain_type,
                    movement_cost: info.movement_cost,
                    elevation: info.elevation,
                    slope: 0.0,
                    pinched: info.pinched,
                };

                let cell = ICoord2D::new(self.extent.lo.x + x as i32, self.extent.lo.y + y as i32);
                sample.slope = self.estimate_cell_slope(cell);

                grid[x][y] = sample;
            }
        }

        NavigationSamples {
            extent: self.extent,
            grid,
            max_ground_slope,
        }
    }

    fn estimate_cell_slope(&self, cell: ICoord2D) -> f32 {
        let Some((x, y)) = self.cell_indices(cell) else {
            return 0.0;
        };

        let center = self.terrain_grid[x][y].elevation;
        let mut max_delta = 0.0f32;

        const OFFSETS: &[(i32, i32)] = &[(1, 0), (-1, 0), (0, 1), (0, -1)];
        for (dx, dy) in OFFSETS {
            let neighbor = ICoord2D::new(cell.x + dx, cell.y + dy);
            if let Some((nx, ny)) = self.cell_indices(neighbor) {
                let neighbor_elev = self.terrain_grid[nx][ny].elevation;
                max_delta = max_delta.max((neighbor_elev - center).abs());
            }
        }

        max_delta / crate::path::PATHFIND_CELL_SIZE_F
    }

    fn cell_info(&self, cell: ICoord2D) -> Option<&TerrainInfo> {
        self.cell_indices(cell)
            .map(|(x, y)| &self.terrain_grid[x][y])
    }

    fn cell_info_mut(&mut self, cell: ICoord2D) -> Option<&mut TerrainInfo> {
        self.cell_indices(cell)
            .map(|(x, y)| &mut self.terrain_grid[x][y])
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

    fn default_movement_cost(terrain: PathfindCellType) -> u32 {
        match terrain {
            PathfindCellType::Clear => 10,
            PathfindCellType::Water => 15,
            PathfindCellType::Cliff => 24,
            PathfindCellType::Rubble => 18,
            PathfindCellType::Obstacle => 30,
            _ => u32::MAX,
        }
    }

    fn update_cliff_pinched(&mut self) {
        let width = self.terrain_grid.len();
        if width == 0 {
            return;
        }
        let height = self.terrain_grid[0].len();

        for x in 0..width {
            for y in 0..height {
                self.terrain_grid[x][y].pinched = false;
            }
        }

        for x in 0..width {
            for y in 0..height {
                if self.terrain_grid[x][y].terrain_type == PathfindCellType::Cliff {
                    let start_x = x.saturating_sub(1);
                    let end_x = (x + 1).min(width - 1);
                    let start_y = y.saturating_sub(1);
                    let end_y = (y + 1).min(height - 1);
                    for nx in start_x..=end_x {
                        for ny in start_y..=end_y {
                            if self.terrain_grid[nx][ny].terrain_type == PathfindCellType::Clear {
                                self.terrain_grid[nx][ny].pinched = true;
                            }
                        }
                    }
                }
            }
        }

        for x in 0..width {
            for y in 0..height {
                if self.terrain_grid[x][y].pinched
                    && self.terrain_grid[x][y].terrain_type == PathfindCellType::Clear
                {
                    self.terrain_grid[x][y].terrain_type = PathfindCellType::Cliff;
                }
            }
        }

        for x in 0..width {
            for y in 0..height {
                if self.terrain_grid[x][y].terrain_type == PathfindCellType::Cliff {
                    let start_x = x.saturating_sub(1);
                    let end_x = (x + 1).min(width - 1);
                    let start_y = y.saturating_sub(1);
                    let end_y = (y + 1).min(height - 1);
                    for nx in start_x..=end_x {
                        for ny in start_y..=end_y {
                            if self.terrain_grid[nx][ny].terrain_type == PathfindCellType::Clear {
                                self.terrain_grid[nx][ny].pinched = true;
                            }
                        }
                    }
                }
            }
        }

        for x in 0..width {
            for y in 0..height {
                let terrain = self.terrain_grid[x][y].terrain_type;
                self.terrain_grid[x][y].movement_cost = Self::default_movement_cost(terrain);
            }
        }
    }

    fn is_zone_eligible(terrain: PathfindCellType) -> bool {
        matches!(
            terrain,
            PathfindCellType::Clear
                | PathfindCellType::Water
                | PathfindCellType::Cliff
                | PathfindCellType::Rubble
        )
    }

    fn can_cells_be_in_same_zone(type1: PathfindCellType, type2: PathfindCellType) -> bool {
        match (type1, type2) {
            (PathfindCellType::Clear, PathfindCellType::Clear) => true,
            (PathfindCellType::Water, PathfindCellType::Water) => true,
            (PathfindCellType::Cliff, PathfindCellType::Cliff) => true,
            (PathfindCellType::Rubble, PathfindCellType::Rubble) => true,
            (PathfindCellType::Clear, PathfindCellType::Rubble)
            | (PathfindCellType::Rubble, PathfindCellType::Clear) => true,
            _ => false,
        }
    }

    fn flood_fill_zone(
        &self,
        zones: &mut [Vec<ZoneStorageType>],
        start: ICoord2D,
        zone: ZoneStorageType,
        target_type: PathfindCellType,
    ) {
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(start);
        while let Some(cell) = queue.pop_front() {
            let Some((x, y)) = self.cell_indices(cell) else {
                continue;
            };
            if zones[x][y] != 0 {
                continue;
            }
            let terrain = self.terrain_grid[x][y].terrain_type;
            if !Self::can_cells_be_in_same_zone(target_type, terrain) {
                continue;
            }
            zones[x][y] = zone;
            queue.push_back(ICoord2D::new(cell.x + 1, cell.y));
            queue.push_back(ICoord2D::new(cell.x - 1, cell.y));
            queue.push_back(ICoord2D::new(cell.x, cell.y + 1));
            queue.push_back(ICoord2D::new(cell.x, cell.y - 1));
        }
    }
}

/// Immutable navigation samples consumed by the pathfinder.
#[derive(Debug, Clone)]
pub struct NavigationSamples {
    extent: IRegion2D,
    grid: Vec<Vec<TerrainSample>>,
    pub(crate) max_ground_slope: f32,
}

impl NavigationSamples {
    pub(crate) fn sample_cell(&self, cell: ICoord2D) -> Option<&TerrainSample> {
        let width = (self.extent.hi.x - self.extent.lo.x + 1) as i32;
        let height = (self.extent.hi.y - self.extent.lo.y + 1) as i32;
        let x = cell.x - self.extent.lo.x;
        let y = cell.y - self.extent.lo.y;
        if x >= 0 && x < width && y >= 0 && y < height {
            Some(&self.grid[x as usize][y as usize])
        } else {
            None
        }
    }
}

#[derive(Debug, Clone)]
pub struct TerrainSample {
    pub terrain_type: PathfindCellType,
    pub movement_cost: u32,
    pub elevation: f32,
    pub slope: f32,
    pub pinched: bool,
}

impl Default for TerrainSample {
    fn default() -> Self {
        Self {
            terrain_type: PathfindCellType::Clear,
            movement_cost: 10,
            elevation: 0.0,
            slope: 0.0,
            pinched: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{ICoord2D, IRegion2D};

    #[test]
    fn navigation_samples_respect_slope_limit() {
        let mut map = NavigationMap::new();
        let bounds = IRegion2D::new(ICoord2D::new(0, 0), ICoord2D::new(2, 0));
        map.initialize(&bounds);
        map.set_elevation(ICoord2D::new(0, 0), 0.0);
        map.set_elevation(ICoord2D::new(1, 0), crate::path::PATHFIND_CELL_SIZE_F);
        let samples = map.to_samples(0.5);
        let sample = samples.sample_cell(ICoord2D::new(1, 0)).expect("sample");
        assert!(sample.slope > 0.5);
    }
}
