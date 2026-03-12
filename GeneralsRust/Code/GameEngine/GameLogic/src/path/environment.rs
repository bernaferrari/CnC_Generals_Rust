//! High-level path environment binding the collision map and pathfinder to the world state.
#![allow(missing_docs)]
//!
//! The original engine rebuilds a collision grid every frame from the live world.  This module
//! mirrors that behaviour: ingest world entities, update the collision map, and synchronize the
//! `Pathfinder`'s cell metadata before queries are issued.

use super::{CollisionMap, NavigationMap, PathfindCellType, Pathfinder};
use crate::common::{Coord3D, ICoord2D, IRegion2D};
use crate::path::{world_to_grid, PATHFIND_CELL_SIZE_F};
use crate::world::World;

/// Margin (in cells) added around detected world bounds when rebuilding the grid.
const EXTENT_MARGIN_CELLS: i32 = 8;
fn default_extent() -> IRegion2D {
    IRegion2D {
        lo: ICoord2D::new(0, 0),
        hi: ICoord2D::new(127, 127),
    }
}

#[derive(Debug)]
pub struct PathEnvironment {
    pathfinder: Pathfinder,
    collision: CollisionMap,
    navigation: NavigationMap,
}

impl PathEnvironment {
    pub fn new() -> Self {
        let mut env = Self {
            pathfinder: Pathfinder::new(),
            collision: CollisionMap::new(),
            navigation: NavigationMap::new(),
        };
        env.initialise_extent(default_extent());
        env
    }

    /// Read-only access to the pathfinder.
    pub fn pathfinder(&self) -> &Pathfinder {
        &self.pathfinder
    }

    /// Mutable access to the pathfinder.
    pub fn pathfinder_mut(&mut self) -> &mut Pathfinder {
        &mut self.pathfinder
    }

    /// Read-only access to the collision map.
    pub fn collision_map(&self) -> &CollisionMap {
        &self.collision
    }

    /// Mutable access to the navigation map.
    pub fn navigation_map_mut(&mut self) -> &mut NavigationMap {
        &mut self.navigation
    }

    /// Read-only access to the navigation map.
    pub fn navigation_map(&self) -> &NavigationMap {
        &self.navigation
    }

    /// Recompute pathfinder navigation samples after mutating the navigation map.
    pub fn sync_navigation(&mut self) {
        let samples = self.navigation.to_samples(0.65);
        self.pathfinder.set_navigation(samples);
    }

    /// Synchronise the path environment with the live world state.
    pub fn update_from_world(&mut self, world: &World) {
        let extent = self.derive_extent(world);
        self.initialise_extent(extent);
        self.navigation.update_terrain_area(&extent);

        // Rebuild collision map from world entities.
        for entity in world.entities() {
            let pos = entity.transform.position;
            let world_pos = Coord3D::new(pos.x, pos.y, pos.z);
            self.collision.add_dynamic_unit(entity.id.get(), &world_pos);
        }

        self.push_collision_into_pathfinder();
        self.sync_navigation();
    }

    fn initialise_extent(&mut self, extent: IRegion2D) {
        self.collision.initialize(&extent);
        self.navigation.initialize(&extent);
        self.pathfinder.set_extent(extent);
    }

    fn derive_extent(&self, world: &World) -> IRegion2D {
        if let Some(terrain_extent) = self.terrain_extent() {
            return terrain_extent;
        }

        let mut min_x = i32::MAX;
        let mut max_x = i32::MIN;
        let mut min_y = i32::MAX;
        let mut max_y = i32::MIN;
        let mut any = false;

        for entity in world.entities() {
            let pos = entity.transform.position;
            let cell = world_to_grid(&Coord3D::new(pos.x, pos.y, pos.z));
            min_x = min_x.min(cell.x);
            max_x = max_x.max(cell.x);
            min_y = min_y.min(cell.y);
            max_y = max_y.max(cell.y);
            any = true;
        }

        if !any {
            return default_extent();
        }

        let lo = ICoord2D::new(
            min_x.saturating_sub(EXTENT_MARGIN_CELLS),
            min_y.saturating_sub(EXTENT_MARGIN_CELLS),
        );
        let hi = ICoord2D::new(
            max_x.saturating_add(EXTENT_MARGIN_CELLS),
            max_y.saturating_add(EXTENT_MARGIN_CELLS),
        );

        IRegion2D { lo, hi }
    }

    fn terrain_extent(&self) -> Option<IRegion2D> {
        let terrain = crate::terrain::get_terrain_logic();
        let terrain_guard = terrain.read().ok()?;
        let extent = terrain_guard.get_maximum_pathfind_extent();

        let lo_x = (extent.lo.x / PATHFIND_CELL_SIZE_F).floor() as i32;
        let lo_y = (extent.lo.y / PATHFIND_CELL_SIZE_F).floor() as i32;
        let mut hi_x = (extent.hi.x / PATHFIND_CELL_SIZE_F).floor() as i32 - 1;
        let mut hi_y = (extent.hi.y / PATHFIND_CELL_SIZE_F).floor() as i32 - 1;

        if hi_x < lo_x {
            hi_x = lo_x;
        }
        if hi_y < lo_y {
            hi_y = lo_y;
        }

        Some(IRegion2D {
            lo: ICoord2D::new(lo_x, lo_y),
            hi: ICoord2D::new(hi_x, hi_y),
        })
    }

    fn push_collision_into_pathfinder(&mut self) {
        let extent = self.collision.extent();
        for x in extent.lo.x..=extent.hi.x {
            for y in extent.lo.y..=extent.hi.y {
                let cell = ICoord2D::new(x, y);
                let entry = self.collision.cell(cell);
                let base_type = self
                    .navigation
                    .sample_cell(cell)
                    .map(|sample| sample.terrain_type)
                    .unwrap_or(PathfindCellType::Clear);
                let cell_type = match entry {
                    Some(cell) if cell.has_static_obstacle() => PathfindCellType::Obstacle,
                    Some(cell) if cell.has_dynamic_units() => PathfindCellType::Obstacle,
                    _ => base_type,
                };
                self.pathfinder.set_cell_type(cell, cell_type);
            }
        }

        self.pathfinder.refresh_pinched_cells();
        self.sync_navigation();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::entities::{TemplateRef, Transform};
    use crate::world::World;

    #[test]
    fn dynamic_units_mark_collision_cells() {
        let mut env = PathEnvironment::new();
        let mut world = World::new(1);

        world.spawn_entity(
            TemplateRef::new("TestObstacle"),
            None,
            Transform::new([5.0, 5.0, 0.0], 0.0),
            100.0,
        );

        env.update_from_world(&world);

        let cell = world_to_grid(&Coord3D::new(5.0, 5.0, 0.0));
        let cell_type = env
            .pathfinder()
            .cell_type(cell)
            .expect("cell within extent");
        assert_eq!(cell_type, PathfindCellType::Obstacle);
    }
}
