//! Core pathfinding implementation mirroring the classic C++ `Pathfinder`.
//!
//! This module provides deterministic grid based A* search, path patching, and
//! proximity lookups so the higher level game logic can schedule unit
//! movement exactly like the original engine.
#![allow(missing_docs)]

use super::*;
use crate::common::{Coord3D, CoordOrigin, ICoord2D, IRegion2D, ObjectID};
use crate::locomotor::SURFACE_AIR;
use crate::path::PathfindLayerEnum as PathLayer;
use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};

struct GridPassability<'a> {
    pathfinder: &'a Pathfinder,
    navigation: Option<&'a NavigationSamples>,
}

impl<'a> GridPassability<'a> {
    fn sample_passability(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        radius_cells: i32,
        crusher: bool,
    ) -> bool {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let dz = to.z - from.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance <= f32::EPSILON {
            return self.sample_point(from, surfaces, radius_cells, crusher);
        }

        let step_length = (crate::path::PATHFIND_CELL_SIZE_F * 0.5).max(0.1);
        let steps = (distance / step_length).ceil().max(1.0) as i32;

        for step in 0..=steps {
            let t = step as f32 / steps as f32;
            let sample = Coord3D::new(from.x + dx * t, from.y + dy * t, from.z + dz * t);
            if !self.sample_point(&sample, surfaces, radius_cells, crusher) {
                return false;
            }
        }

        true
    }

    fn sample_point(
        &self,
        pos: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        radius_cells: i32,
        crusher: bool,
    ) -> bool {
        let center = world_to_grid(pos);
        for dx in -radius_cells..=radius_cells {
            for dy in -radius_cells..=radius_cells {
                let cell = ICoord2D::new(center.x + dx, center.y + dy);
                if !self.cell_passable(cell, surfaces, crusher) {
                    return false;
                }
                if let Some(nav) = self.navigation {
                    if let Some(sample) = nav.sample_cell(cell) {
                        if sample.slope > nav.max_ground_slope && (surfaces & SURFACE_CLIFF == 0) {
                            return false;
                        }
                    }
                }
            }
        }
        true
    }

    fn cell_passable(
        &self,
        cell: ICoord2D,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
    ) -> bool {
        // C++ validMovementPosition: crushers may enter fence obstacles only.
        if self.pathfinder.crusher_may_cross_obstacle(cell, crusher) {
            return true;
        }
        let Some(cell_type) = self.pathfinder.cell_type(cell) else {
            return false;
        };
        let allowed = valid_locomotor_surfaces_for_cell_type(cell_type);
        allowed != 0 && (allowed & surfaces) != 0
    }
}

impl<'a> PassabilityQuery for GridPassability<'a> {
    fn is_line_passable(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        from: &Coord3D,
        to: &Coord3D,
        blocked: bool,
    ) -> bool {
        if blocked {
            return false;
        }
        self.sample_passability(from, to, surfaces, 0, false)
    }

    fn is_ground_line_passable(
        &self,
        crusher: bool,
        diameter: i32,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        let radius_cells = ((diameter.max(PATHFIND_CELL_SIZE) as f32)
            / crate::path::PATHFIND_CELL_SIZE_F)
            .ceil()
            .max(if crusher { 0.0 } else { 0.5 }) as i32;
        // Ground locomotor surfaces only — crushers do not fly over buildings.
        // Fence exceptions are applied in cell_passable via crusher_may_cross_obstacle.
        self.sample_passability(from, to, SURFACE_GROUND, radius_cells, crusher)
    }
}

/// Deterministic comparer for reasonably sized coordinate keys (avoids fragile tuple math).
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

/// Simple node for deterministic A* queue ordering.
#[derive(Clone, Copy, Eq, PartialEq, Debug)]
struct OpenNode {
    key: CellKey,
    g: u32,
    f: u32,
}

impl Ord for OpenNode {
    fn cmp(&self, other: &Self) -> Ordering {
        other
            .f
            .cmp(&self.f)
            .then_with(|| other.g.cmp(&self.g))
            .then_with(|| other.key.cmp(&self.key))
    }
}

impl PartialOrd for OpenNode {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Soft-repulsors used by safe-path requests.
#[derive(Clone, Copy, Debug)]
struct Repulsor {
    center: Coord3D,
    radius: f32,
}

impl Repulsor {
    fn penalty(&self, position: Coord3D) -> u32 {
        let dx = position.x - self.center.x;
        let dy = position.y - self.center.y;
        let dist_sq = dx * dx + dy * dy;
        if dist_sq >= self.radius * self.radius {
            return 0;
        }

        let dist = dist_sq.sqrt();
        let influence = (self.radius - dist) / self.radius;
        ((influence * 400.0) + 200.0) as u32
    }
}

/// Aggregated penalties for safe-path requests.
#[derive(Default)]
struct SearchPenalties {
    repulsors: Vec<Repulsor>,
}

impl SearchPenalties {
    fn new(pos1: &Coord3D, pos2: &Coord3D, radius: f32) -> Self {
        if radius <= 0.0 {
            return Self::default();
        }
        Self {
            repulsors: vec![
                Repulsor {
                    center: *pos1,
                    radius,
                },
                Repulsor {
                    center: *pos2,
                    radius,
                },
            ],
        }
    }

    fn penalty_for(&self, cell: ICoord2D) -> u32 {
        if self.repulsors.is_empty() {
            return 0;
        }
        let world = grid_to_world(&cell, PathLayer::Ground);
        self.repulsors
            .iter()
            .fold(0u32, |acc, rep| acc.saturating_add(rep.penalty(world)))
    }
}

/// Core pathfinder (grid based A* search) matching the original C++ behaviour.
#[derive(Debug)]
pub struct Pathfinder {
    grid: Vec<Vec<PathfindCell>>,
    extent: IRegion2D,
    paths: HashMap<PathHandle, Path>,
    next_path_id: u32,
    navigation: Option<NavigationSamples>,
}

impl Pathfinder {
    pub fn new() -> Self {
        let mut pathfinder = Self {
            grid: Vec::new(),
            extent: IRegion2D::default(),
            paths: HashMap::new(),
            next_path_id: 1,
            navigation: None,
        };
        pathfinder.reset();
        pathfinder
    }

    pub fn reset(&mut self) {
        let default_extent = IRegion2D::new(ICoord2D::new(0, 0), ICoord2D::new(127, 127));
        self.resize_grid(default_extent);
        self.paths.clear();
        self.next_path_id = 1;
        self.navigation = None;
    }

    pub fn new_map(&mut self) {
        self.reset();
    }

    pub fn set_extent(&mut self, extent: IRegion2D) {
        self.resize_grid(extent);
    }

    pub fn set_cell_type(&mut self, cell: ICoord2D, cell_type: PathfindCellType) {
        if let Some((x, y)) = self.cell_indices(cell) {
            self.grid[x][y].set_type(cell_type);
        }
    }

    /// Stamp CELL_OBSTACLE with C++ `setTypeAsObstacle` fence flag.
    /// `is_fence == true` is crushable; solid buildings stay blocked for ground crushers.
    pub fn set_cell_as_obstacle(&mut self, cell: ICoord2D, is_fence: bool) {
        if let Some((x, y)) = self.cell_indices(cell) {
            let _ = self.grid[x][y].set_type_as_obstacle(1, is_fence, &cell);
        }
    }

    /// C++ `PathfindCell::isObstacleFence`.
    pub fn is_obstacle_fence(&self, cell: ICoord2D) -> bool {
        self.cell_indices(cell)
            .map(|(x, y)| self.grid[x][y].is_obstacle_fence())
            .unwrap_or(false)
    }

    /// C++ `isCrusher && toCell->isObstacleFence()` in validMovementPosition.
    #[inline]
    fn crusher_may_cross_obstacle(&self, cell: ICoord2D, is_crusher: bool) -> bool {
        is_crusher
            && matches!(self.cell_type(cell), Some(PathfindCellType::Obstacle))
            && self.is_obstacle_fence(cell)
    }

    pub fn set_navigation(&mut self, samples: NavigationSamples) {
        self.navigation = Some(samples);
    }

    pub fn refresh_pinched_cells(&mut self) {
        let width = self.grid.len() as i32;
        if width == 0 {
            return;
        }
        let height = self.grid[0].len() as i32;

        for x in 0..width {
            for y in 0..height {
                let cell = &mut self.grid[x as usize][y as usize];
                if matches!(cell.get_type(), PathfindCellType::Impassable) {
                    cell.set_type(PathfindCellType::Clear);
                }
                cell.set_pinched(false);
            }
        }

        for x in 0..width {
            for y in 0..height {
                if self.grid[x as usize][y as usize].get_type() != PathfindCellType::Clear {
                    continue;
                }
                let mut total_count = 0;
                let mut orthogonal_count = 0;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= width || ny >= height {
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

        for x in 0..width {
            for y in 0..height {
                let cell = &mut self.grid[x as usize][y as usize];
                if cell.get_pinched() && cell.get_type() == PathfindCellType::Clear {
                    cell.set_type(PathfindCellType::Impassable);
                    cell.set_pinched(false);
                }
            }
        }

        for x in 0..width {
            for y in 0..height {
                if self.grid[x as usize][y as usize].get_type() != PathfindCellType::Clear {
                    continue;
                }
                let mut obstacle_adjacent = false;
                for dx in -1..=1 {
                    for dy in -1..=1 {
                        let nx = x + dx;
                        let ny = y + dy;
                        if nx < 0 || ny < 0 || nx >= width || ny >= height {
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

    pub fn path(&self, handle: PathHandle) -> Option<&Path> {
        self.paths.get(&handle)
    }

    pub fn take_path(&mut self, handle: PathHandle) -> Option<Path> {
        self.paths.remove(&handle)
    }

    pub fn release_path(&mut self, handle: PathHandle) {
        self.paths.remove(&handle);
    }

    fn resize_grid(&mut self, extent: IRegion2D) {
        let width = (extent.hi.x - extent.lo.x + 1).max(1) as usize;
        let height = (extent.hi.y - extent.lo.y + 1).max(1) as usize;
        self.extent = extent;
        self.grid = (0..width)
            .map(|_| (0..height).map(|_| PathfindCell::new()).collect::<Vec<_>>())
            .collect();
        self.navigation = None;
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

    pub fn cell_type(&self, cell: ICoord2D) -> Option<PathfindCellType> {
        self.cell_indices(cell)
            .map(|(x, y)| self.grid[x][y].get_type())
    }

    /// C++ `Pathfinder::validMovementPosition` (AIPathfind.cpp:4839-4847).
    /// Crushers enter fence obstacles only; AIR may enter obstacle/impassable/bridge_impassable.
    fn is_passable(&self, locomotor: &LocomotorSet, cell: ICoord2D) -> bool {
        if self.crusher_may_cross_obstacle(cell, locomotor.is_crusher()) {
            return true;
        }
        let Some(cell_type) = self.cell_type(cell) else {
            return false;
        };
        let surfaces = valid_locomotor_surfaces_for_cell_type(cell_type);
        surfaces != 0 && (locomotor.get_valid_surfaces() & surfaces) != 0
    }

    fn traversal_cost(&self, locomotor: &LocomotorSet, cell: ICoord2D) -> u32 {
        if locomotor.can_move_on_surface(SURFACE_AIR) {
            return COST_ORTHOGONAL;
        }
        let Some(cell_type) = self.cell_type(cell) else {
            return u32::MAX;
        };
        match cell_type {
            PathfindCellType::Clear => COST_ORTHOGONAL,
            PathfindCellType::Water => {
                if locomotor.can_move_on_surface(SURFACE_WATER) {
                    15
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Cliff => {
                if locomotor.can_move_on_surface(SURFACE_CLIFF) {
                    24
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Rubble => {
                if locomotor.can_move_on_surface(SURFACE_RUBBLE) {
                    18
                } else if locomotor.is_crusher() {
                    12
                } else {
                    u32::MAX
                }
            }
            PathfindCellType::Obstacle => {
                if self.crusher_may_cross_obstacle(cell, locomotor.is_crusher()) {
                    // C++ examineNeighboringCells: CELL_OBSTACLE += 100*COST_ORTHOGONAL
                    100 * COST_ORTHOGONAL
                } else {
                    u32::MAX
                }
            }
            _ => u32::MAX,
        }
    }

    fn store_path(
        &mut self,
        cells: &[ICoord2D],
        layer: PathLayer,
        locomotor: &LocomotorSet,
        blocked: bool,
    ) -> PathHandle {
        let mut path = Path::new();
        for cell in cells {
            let pos = grid_to_world(cell, layer);
            path.append_node(&pos, layer);
        }
        let handle = PathHandle(self.next_path_id);
        self.next_path_id = self.next_path_id.wrapping_add(1).max(1);
        self.paths.insert(handle, path);
        self.optimize_path(handle, locomotor, blocked);
        handle
    }

    fn optimize_path(&mut self, handle: PathHandle, locomotor: &LocomotorSet, blocked: bool) {
        let Some(mut path) = self.paths.remove(&handle) else {
            return;
        };

        let navigation = self.navigation.as_ref();
        let passability = GridPassability {
            pathfinder: self,
            navigation,
        };
        let diameter = (locomotor.get_radius() * 2.0)
            .max(crate::path::PATHFIND_CELL_SIZE_F)
            .round() as i32;

        path.optimize_internal(locomotor.get_valid_surfaces(), blocked, Some(&passability));
        path.optimize_ground_internal(locomotor.is_crusher(), diameter, Some(&passability));
        path.smooth(0.35);
        path.mark_optimized();

        self.paths.insert(handle, path);
    }

    fn reconstruct_path(
        &self,
        came_from: &HashMap<CellKey, CellKey>,
        mut current: CellKey,
    ) -> Vec<ICoord2D> {
        let mut nodes = vec![current];
        while let Some(prev) = came_from.get(&current) {
            current = *prev;
            nodes.push(current);
        }
        nodes.reverse();
        nodes.into_iter().map(CellKey::to_coord).collect()
    }

    fn heuristic(a: CellKey, b: CellKey) -> u32 {
        ((a.x - b.x).abs() + (a.y - b.y).abs()) as u32 * 10
    }

    fn search_path(
        &self,
        start: ICoord2D,
        goal: ICoord2D,
        locomotor: &LocomotorSet,
        penalties: Option<&SearchPenalties>,
    ) -> Option<Vec<ICoord2D>> {
        if start == goal {
            return Some(vec![start]);
        }

        let start_key = CellKey::from(start);
        let goal_key = CellKey::from(goal);

        let mut open_set = BTreeSet::new();
        let mut came_from: HashMap<CellKey, CellKey> = HashMap::new();
        let mut g_score: HashMap<CellKey, u32> = HashMap::new();
        let mut closed: HashMap<CellKey, bool> = HashMap::new();

        open_set.insert(OpenNode {
            key: start_key,
            g: 0,
            f: Self::heuristic(start_key, goal_key),
        });
        g_score.insert(start_key, 0);

        while let Some(node) = open_set.iter().next().copied() {
            open_set.remove(&node);

            if node.key == goal_key {
                return Some(self.reconstruct_path(&came_from, node.key));
            }

            if closed.get(&node.key).copied().unwrap_or(false) {
                continue;
            }
            closed.insert(node.key, true);

            for delta in NEIGHBOR_DELTAS {
                let (dx, dy, step_cost) = *delta;
                if dx == 0 && dy == 0 {
                    continue;
                }

                let neighbor_cell = ICoord2D::new(node.key.x + dx, node.key.y + dy);
                let neighbor_key = CellKey::from(neighbor_cell);

                if self.cell_type(neighbor_cell).is_none() {
                    continue;
                }

                if !self.is_passable(locomotor, neighbor_cell) {
                    continue;
                }

                if let Some(nav) = self.navigation.as_ref() {
                    if let Some(sample) = nav.sample_cell(neighbor_cell) {
                        if sample.terrain_type == PathfindCellType::Cliff && sample.pinched {
                            continue;
                        }
                        if sample.slope > nav.max_ground_slope
                            && !locomotor.can_move_on_surface(SURFACE_CLIFF)
                        {
                            continue;
                        }
                    }
                }

                if dx != 0 && dy != 0 {
                    let adj1 = ICoord2D::new(node.key.x + dx, node.key.y);
                    let adj2 = ICoord2D::new(node.key.x, node.key.y + dy);
                    if self.cell_type(adj1).is_none() || !self.is_passable(locomotor, adj1) {
                        continue;
                    }
                    if self.cell_type(adj2).is_none() || !self.is_passable(locomotor, adj2) {
                        continue;
                    }
                }

                let mut terrain_cost = self.traversal_cost(locomotor, neighbor_cell);
                if let Some(nav) = self.navigation.as_ref() {
                    if let Some(sample) = nav.sample_cell(neighbor_cell) {
                        if !locomotor.can_move_on_surface(SURFACE_AIR) {
                            terrain_cost = sample.movement_cost;
                        }
                    }
                }
                if terrain_cost == u32::MAX {
                    continue;
                }

                let mut tentative_g = node.g.saturating_add(step_cost);
                tentative_g = tentative_g.saturating_add(terrain_cost);

                if let Some(penalties) = penalties {
                    tentative_g = tentative_g.saturating_add(penalties.penalty_for(neighbor_cell));
                }

                let entry = g_score.get(&neighbor_key).copied().unwrap_or(u32::MAX);
                if tentative_g < entry {
                    came_from.insert(neighbor_key, node.key);
                    g_score.insert(neighbor_key, tentative_g);
                    let f_score =
                        tentative_g.saturating_add(Self::heuristic(neighbor_key, goal_key));
                    open_set.insert(OpenNode {
                        key: neighbor_key,
                        g: tentative_g,
                        f: f_score,
                    });
                }
            }
        }

        None
    }
}

/// C++ `COST_ORTHOGONAL` / `COST_DIAGONAL` (AIPathfind.cpp:1649).
const COST_ORTHOGONAL: u32 = 10;
const COST_DIAGONAL: u32 = 14;

const NEIGHBOR_DELTAS: &[(i32, i32, u32)] = &[
    (-1, -1, COST_DIAGONAL),
    (-1, 0, COST_ORTHOGONAL),
    (-1, 1, COST_DIAGONAL),
    (0, -1, COST_ORTHOGONAL),
    (0, 1, COST_ORTHOGONAL),
    (1, -1, COST_DIAGONAL),
    (1, 0, COST_ORTHOGONAL),
    (1, 1, COST_DIAGONAL),
];

impl PathfindServicesInterface for Pathfinder {
    fn find_path(
        &mut self,
        _obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        to: &Coord3D,
    ) -> Option<PathHandle> {
        let start_cell = world_to_grid(from);
        let goal_cell = world_to_grid(to);
        let cells = self.search_path(start_cell, goal_cell, locomotor_set, None)?;
        Some(self.store_path(&cells, PathLayer::Ground, locomotor_set, false))
    }

    fn find_closest_path(
        &mut self,
        _obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        to: &mut Coord3D,
        _blocked: bool,
        path_cost_multiplier: f32,
        _move_allies: bool,
    ) -> Option<PathHandle> {
        let start_cell = world_to_grid(from);
        let goal_cell = world_to_grid(to);

        if let Some(cells) = self.search_path(start_cell, goal_cell, locomotor_set, None) {
            return Some(self.store_path(&cells, PathLayer::Ground, locomotor_set, false));
        }

        let mut best: Option<(Vec<ICoord2D>, f32)> = None;
        let max_radius: i32 = 16;
        for radius in 1..=max_radius {
            for dx in -radius..=radius {
                for dy in -radius..=radius {
                    if dx.abs() != radius && dy.abs() != radius {
                        continue;
                    }
                    let candidate = ICoord2D::new(goal_cell.x + dx, goal_cell.y + dy);
                    if self.cell_type(candidate).is_none() {
                        continue;
                    }
                    if !self.is_passable(locomotor_set, candidate) {
                        continue;
                    }
                    if let Some(cells) =
                        self.search_path(start_cell, candidate, locomotor_set, None)
                    {
                        let world = grid_to_world(&candidate, PathLayer::Ground);
                        let dist_sq = (world.x - to.x).powi(2) + (world.y - to.y).powi(2);
                        let adjusted = dist_sq * path_cost_multiplier;
                        match best {
                            Some((_, best_score)) if adjusted >= best_score => {}
                            _ => {
                                best = Some((cells, adjusted));
                                *to = world;
                            }
                        }
                    }
                }
            }
            if best.is_some() {
                break;
            }
        }

        best.map(|(cells, _)| self.store_path(&cells, PathLayer::Ground, locomotor_set, false))
    }

    fn find_attack_path(
        &mut self,
        _obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        _victim: ObjectID,
        victim_pos: &Coord3D,
        _weapon: Option<&WeaponHandle>,
    ) -> Option<PathHandle> {
        self.find_path(_obj, locomotor_set, from, victim_pos)
    }

    fn patch_path(
        &mut self,
        _obj: ObjectID,
        locomotor_set: &LocomotorSet,
        original_path: PathHandle,
        _blocked: bool,
    ) -> Option<PathHandle> {
        let existing = self.paths.get(&original_path)?;
        let goal = existing
            .get_last_node()
            .map(|node| *node.get_position())
            .unwrap_or_else(|| Coord3D::origin());
        let start = existing
            .get_first_node()
            .map(|node| *node.get_position())
            .unwrap_or_else(|| Coord3D::origin());

        let start_cell = world_to_grid(&start);
        let goal_cell = world_to_grid(&goal);
        let cells = self.search_path(start_cell, goal_cell, locomotor_set, None)?;
        Some(self.store_path(
            &cells,
            PathLayer::Ground,
            locomotor_set,
            existing.get_blocked_by_ally(),
        ))
    }

    fn find_safe_path(
        &mut self,
        _obj: ObjectID,
        locomotor_set: &LocomotorSet,
        from: &Coord3D,
        repulsor_pos1: &Coord3D,
        repulsor_pos2: &Coord3D,
        repulsor_radius: f32,
    ) -> Option<PathHandle> {
        let start_cell = world_to_grid(from);
        let penalties = SearchPenalties::new(repulsor_pos1, repulsor_pos2, repulsor_radius);

        let target_cell = if penalties.repulsors.is_empty() {
            start_cell
        } else {
            penalties
                .repulsors
                .iter()
                .map(|rep| world_to_grid(&rep.center))
                .fold(start_cell, |acc, rep| {
                    if acc == rep {
                        ICoord2D::new(acc.x + 2, acc.y + 2)
                    } else {
                        acc
                    }
                })
        };

        let cells = self.search_path(start_cell, target_cell, locomotor_set, Some(&penalties))?;
        Some(self.store_path(&cells, PathLayer::Ground, locomotor_set, false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::common::{Coord3D, ICoord2D, IRegion2D};
    use crate::locomotor::SURFACE_AIR;
    use crate::path::{
        LocomotorSet, NavigationMap, PATHFIND_CELL_SIZE_F, PathHandle, PathfindCellType,
        SURFACE_GROUND, grid_to_world, world_to_grid,
    };

    fn clear_grid(pathfinder: &mut Pathfinder, width: i32, height: i32) {
        let extent = IRegion2D::new(ICoord2D::new(0, 0), ICoord2D::new(width - 1, height - 1));
        pathfinder.set_extent(extent);
        for x in 0..width {
            for y in 0..height {
                pathfinder.set_cell_type(ICoord2D::new(x, y), PathfindCellType::Clear);
            }
        }
    }

    fn path_nodes(pathfinder: &Pathfinder, handle: PathHandle) -> Vec<Coord3D> {
        let path = pathfinder.path(handle).expect("stored path");
        let mut nodes = Vec::new();
        let mut cursor = path.get_first_node();
        while let Some(node) = cursor {
            nodes.push(*node.get_position());
            cursor = node.get_next();
        }
        nodes
    }

    fn path_crosses_column(nodes: &[Coord3D], column: i32) -> bool {
        if nodes.iter().any(|p| world_to_grid(p).x == column) {
            return true;
        }
        for window in nodes.windows(2) {
            let a = window[0];
            let b = window[1];
            let dx = b.x - a.x;
            let dy = b.y - a.y;
            let dist = (dx * dx + dy * dy).sqrt();
            if dist <= f32::EPSILON {
                continue;
            }
            let steps = (dist / (PATHFIND_CELL_SIZE_F * 0.25)).ceil().max(1.0) as i32;
            for step in 0..=steps {
                let t = step as f32 / steps as f32;
                let sample = Coord3D::new(a.x + dx * t, a.y + dy * t, 0.0);
                if world_to_grid(&sample).x == column {
                    return true;
                }
            }
        }
        false
    }

    #[test]
    fn steep_slope_blocks_ground_path() {
        let mut pathfinder = Pathfinder::new();
        let extent = IRegion2D::new(ICoord2D::new(0, 0), ICoord2D::new(2, 0));
        pathfinder.set_extent(extent);

        for x in extent.lo.x..=extent.hi.x {
            for y in extent.lo.y..=extent.hi.y {
                pathfinder.set_cell_type(ICoord2D::new(x, y), PathfindCellType::Clear);
            }
        }

        let mut navigation = NavigationMap::new();
        navigation.initialize(&extent);
        navigation.set_elevation(ICoord2D::new(0, 0), 0.0);
        navigation.set_elevation(ICoord2D::new(1, 0), crate::path::PATHFIND_CELL_SIZE_F);
        navigation.set_elevation(ICoord2D::new(2, 0), crate::path::PATHFIND_CELL_SIZE_F);
        pathfinder.set_navigation(navigation.to_samples(0.5));

        let locomotor = LocomotorSet::new(SURFACE_GROUND, false, 5.0);
        let from = grid_to_world(&ICoord2D::new(0, 0), PathLayer::Ground);
        let to = grid_to_world(&ICoord2D::new(2, 0), PathLayer::Ground);

        assert!(pathfinder.find_path(0, &locomotor, &from, &to).is_none());
    }

    #[test]
    fn crusher_find_path_only_crosses_fence_obstacles_like_cpp() {
        // Full-height wall at x=4 so the only route is through the obstacle column.
        // C++ AIPathfind.cpp:4840-4842 — crushers enter isObstacleFence only.
        let mut pf = Pathfinder::new();
        clear_grid(&mut pf, 9, 5);
        for y in 0..5 {
            pf.set_cell_as_obstacle(ICoord2D::new(4, y), false);
        }
        let start = ICoord2D::new(0, 2);
        let goal = ICoord2D::new(8, 2);
        let from = grid_to_world(&start, PathLayer::Ground);
        let to = grid_to_world(&goal, PathLayer::Ground);
        let crusher = LocomotorSet::new(SURFACE_GROUND, true, 5.0);
        let infantry = LocomotorSet::new(SURFACE_GROUND, false, 5.0);

        assert!(
            !pf.is_obstacle_fence(ICoord2D::new(4, 2)),
            "solid building must not be stamped as fence"
        );
        assert!(
            !pf.is_passable(&crusher, ICoord2D::new(4, 2)),
            "crusher must not enter solid CELL_OBSTACLE buildings"
        );
        assert!(!pf.is_passable(&infantry, ICoord2D::new(4, 2)));
        assert_eq!(pf.traversal_cost(&crusher, ICoord2D::new(4, 2)), u32::MAX);
        assert!(
            pf.find_path(0, &crusher, &from, &to).is_none(),
            "crusher must not path through solid CELL_OBSTACLE buildings"
        );
        assert!(pf.find_path(0, &infantry, &from, &to).is_none());

        for y in 0..5 {
            pf.set_cell_as_obstacle(ICoord2D::new(4, y), true);
        }
        assert!(
            pf.is_obstacle_fence(ICoord2D::new(4, 2)),
            "wall must be stamped as fence"
        );
        assert!(
            !pf.is_passable(&infantry, ICoord2D::new(4, 2)),
            "non-crusher still blocked by fence"
        );
        assert!(
            pf.is_passable(&crusher, ICoord2D::new(4, 2)),
            "crusher may enter fence cells"
        );
        assert_eq!(
            pf.traversal_cost(&crusher, ICoord2D::new(4, 2)),
            100 * COST_ORTHOGONAL,
            "fence crusher cost matches C++ 100*COST_ORTHOGONAL"
        );
        assert!(
            pf.find_path(0, &infantry, &from, &to).is_none(),
            "non-crusher must not path through a fence wall"
        );
        let handle = pf
            .find_path(0, &crusher, &from, &to)
            .expect("crusher must path through a fence wall");
        let nodes = path_nodes(&pf, handle);
        assert!(!nodes.is_empty(), "crusher fence path must contain nodes");
        assert_eq!(world_to_grid(&nodes[0]), start);
        assert_eq!(world_to_grid(nodes.last().unwrap()), goal);
        assert!(
            path_crosses_column(&nodes, 4),
            "crusher path must cross fence column: {:?}",
            nodes
        );
    }

    #[test]
    fn air_find_path_crosses_obstacle_impassable_and_bridge_impassable() {
        // C++ validLocomotorSurfacesForCellType: OBSTACLE/IMPASSABLE/BRIDGE_IMPASSABLE → AIR.
        let mut pf = Pathfinder::new();
        clear_grid(&mut pf, 9, 5);
        for y in 0..5 {
            pf.set_cell_as_obstacle(ICoord2D::new(4, y), false);
        }
        let from = grid_to_world(&ICoord2D::new(0, 2), PathLayer::Ground);
        let to = grid_to_world(&ICoord2D::new(8, 2), PathLayer::Ground);
        let air = LocomotorSet::new(SURFACE_AIR, false, 5.0);
        let ground = LocomotorSet::new(SURFACE_GROUND, false, 5.0);

        assert!(pf.is_passable(&air, ICoord2D::new(4, 2)));
        assert!(!pf.is_passable(&ground, ICoord2D::new(4, 2)));
        assert!(
            pf.find_path(0, &air, &from, &to).is_some(),
            "AIR must path through solid obstacles"
        );
        assert!(pf.find_path(0, &ground, &from, &to).is_none());

        for y in 0..5 {
            pf.set_cell_type(ICoord2D::new(4, y), PathfindCellType::Impassable);
        }
        assert!(pf.is_passable(&air, ICoord2D::new(4, 2)));
        assert!(!pf.is_passable(&ground, ICoord2D::new(4, 2)));
        assert!(
            pf.find_path(0, &air, &from, &to).is_some(),
            "AIR must path through CELL_IMPASSABLE"
        );
        assert!(pf.find_path(0, &ground, &from, &to).is_none());

        for y in 0..5 {
            pf.set_cell_type(ICoord2D::new(4, y), PathfindCellType::BridgeImpassable);
        }
        assert!(pf.is_passable(&air, ICoord2D::new(4, 2)));
        assert!(!pf.is_passable(&ground, ICoord2D::new(4, 2)));
        assert!(
            pf.find_path(0, &air, &from, &to).is_some(),
            "AIR must path through CELL_BRIDGE_IMPASSABLE"
        );
        assert!(pf.find_path(0, &ground, &from, &to).is_none());
    }

    #[test]
    fn crusher_find_path_goes_around_solid_building_not_through() {
        // Building occupies (3,1)-(5,3); start/goal on the mid row so a tank
        // that treated every CELL_OBSTACLE as crushable would cut through.
        let mut pf = Pathfinder::new();
        clear_grid(&mut pf, 9, 5);
        for x in 3..=5 {
            for y in 1..=3 {
                pf.set_cell_as_obstacle(ICoord2D::new(x, y), false);
            }
        }
        let from = grid_to_world(&ICoord2D::new(0, 2), PathLayer::Ground);
        let to = grid_to_world(&ICoord2D::new(8, 2), PathLayer::Ground);
        let crusher = LocomotorSet::new(SURFACE_GROUND, true, 5.0);
        let infantry = LocomotorSet::new(SURFACE_GROUND, false, 5.0);

        assert!(!pf.is_passable(&crusher, ICoord2D::new(4, 2)));
        let handle = pf
            .find_path(0, &crusher, &from, &to)
            .expect("crusher must detour around solid buildings");
        let infantry_handle = pf
            .find_path(0, &infantry, &from, &to)
            .expect("infantry must detour around solid buildings");
        let crusher_nodes = path_nodes(&pf, handle);
        let infantry_nodes = path_nodes(&pf, infantry_handle);
        assert!(
            !path_crosses_column(&crusher_nodes, 4)
                || crusher_nodes.iter().all(|p| {
                    let c = world_to_grid(p);
                    c.x != 4 || c.y == 0 || c.y == 4
                }),
            "crusher must not enter the building interior: {:?}",
            crusher_nodes
        );
        for node in &crusher_nodes {
            let cell = world_to_grid(node);
            if (3..=5).contains(&cell.x) && (1..=3).contains(&cell.y) {
                panic!("crusher waypoint inside solid building {:?}", cell);
            }
        }
        assert!(
            !infantry_nodes.is_empty() && !crusher_nodes.is_empty(),
            "both ground units must find the around-building route"
        );
    }
}
