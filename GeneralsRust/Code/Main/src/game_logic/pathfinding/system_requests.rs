use super::*;

impl PathfindingSystem {
    pub fn new(world_width: f32, world_height: f32) -> Self {
        Self::new_with_origin(Vec3::ZERO, world_width, world_height)
    }

    pub fn new_with_origin(origin: Vec3, world_width: f32, world_height: f32) -> Self {
        const GRID_SIZE: f32 = 10.0; // 10 units per grid cell

        Self {
            grid: PathfindingGrid::new_with_origin(origin, world_width, world_height, GRID_SIZE),
            flow_fields: HashMap::new(),
            logic_frame: 0,
            dynamic_obstacle_frame: u64::MAX,
            crate_astar: None,
            pending_paths: VecDeque::new(),
            seeker_player: None,
            seeker_is_infantry: false,
            seeker_wings: false,
            seeker_id: None,
            seeker_team: None,
            seeker_crusher_level: 0,
            seeker_path_diameter: 1,
            seeker_center_in_cell: true,
            ignore_obstacle_id: None,
            human_player_mask: 0,
            seeker_is_human: false,
            seeker_is_dozer: false,
            seeker_downhill_only: false,
            seeker_can_path_through_units: false,
            terrain_height_samples: None,
            cumulative_cells_allocated: 0,
            pathfind_cells_per_frame: PATHFIND_CELLS_PER_FRAME,
        }
    }

    /// C++ `Player::getPlayerType() == PLAYER_HUMAN` bits for logical-extent clamp.
    pub fn set_human_player_mask(&mut self, mask: u16) {
        self.human_player_mask = mask;
    }

    pub(super) fn apply_seeker_human_flag(&mut self) {
        self.seeker_is_human = match self.seeker_player {
            Some(p) if self.human_player_mask != 0 => {
                (self.human_player_mask & (1u16 << p.min(15))) != 0
            }
            // No mask yet: player-path default is human (full-grid extent is a no-op).
            _ => self.human_player_mask == 0,
        };
        self.grid.set_query_is_human(self.seeker_is_human);
    }

    /// C++ `findPath(obj, ...)` — seeker flags come from the mover, not nearest-to-start.
    pub(super) fn bind_seeker_from_mover(
        &mut self,
        objects: &HashMap<ObjectId, Object>,
        mover: Option<ObjectId>,
    ) {
        if let Some(o) = mover
            .and_then(|id| objects.get(&id))
            .filter(|o| o.is_alive())
        {
            self.seeker_player = o.owner_player_id.or(Some(o.team as u32));
            self.seeker_is_infantry = o.is_kind_of(KindOf::Infantry);
            self.seeker_wings = matches!(o.loco_appearance, LocomotorAppearance::Wings);
            self.seeker_id = Some(o.id);
            self.seeker_team = Some(o.team);
            self.seeker_crusher_level = o.crusher_level;
            self.seeker_is_dozer = o.is_kind_of(KindOf::Dozer);
            self.seeker_downhill_only = o.downhill_only;
            self.seeker_can_path_through_units = o.can_path_through_units;
            self.seeker_path_diameter = PathfindingGrid::path_diameter_for_unit(
                o.selection_radius,
                self.grid.grid_size(),
                o.is_kind_of(KindOf::Vehicle),
            );
            self.seeker_center_in_cell =
                PathfindingGrid::radius_and_center(o.selection_radius, self.grid.grid_size()).1;
        } else {
            self.seeker_player = None;
            self.seeker_is_infantry = false;
            self.seeker_wings = false;
            self.seeker_id = None;
            self.seeker_team = None;
            self.seeker_crusher_level = 0;
            self.seeker_path_diameter = 1;
            self.seeker_center_in_cell = true;
            self.seeker_is_dozer = false;
            self.seeker_downhill_only = false;
            self.seeker_can_path_through_units = false;
        }
    }

    /// C++ dozerHack: obstacle exists and relationship != ENEMIES.
    pub(super) fn dozer_hack_allows(&self, cell: GridCoord) -> bool {
        if !self.seeker_is_dozer {
            return false;
        }
        let Some((_id, owner, team)) = self.grid.obstacle_owner(GridPos::new(cell.x, cell.y))
        else {
            return false;
        };
        if let (Some(sp), Some(op)) = (self.seeker_player, owner) {
            if sp == op {
                return true;
            }
            let ally = self.grid.ally_mask_for(sp);
            if (ally & (1u16 << op.min(15))) != 0 {
                return true;
            }
            return matches!(team, Some(Team::Neutral));
        }
        match (self.seeker_team, team) {
            (Some(a), Some(b)) if a == b => true,
            (Some(Team::Neutral), _) | (_, Some(Team::Neutral)) => true,
            _ => false,
        }
    }

    /// C++ getRelationship == ALLIES bits for occupancy crush-through.
    pub fn set_player_ally_masks(&mut self, masks: [u16; 16]) {
        self.grid.set_player_ally_masks(masks);
    }

    /// C++ `Pathfinder::adjustToLandingDestination`. Off-map unit+dest is scripted OK.
    pub fn adjust_to_landing_destination(&self, from: Vec3, dest: Vec3) -> Vec3 {
        self.adjust_to_landing_destination_radius(from, dest, 0.0)
    }

    pub fn adjust_to_landing_destination_radius(
        &self,
        from: Vec3,
        dest: Vec3,
        unit_radius: f32,
    ) -> Vec3 {
        let dest_cell = self.grid.world_to_grid(dest);
        let from_cell = self.grid.world_to_grid(from);
        if !self.grid.is_valid_pos(dest_cell) && !self.grid.is_valid_pos(from_cell) {
            return dest;
        }
        let (radius, center_in_cell) =
            PathfindingGrid::radius_and_center(unit_radius, self.grid.grid_size());
        let mut adjust = dest;
        if !center_in_cell {
            let half = self.grid.grid_size() * 0.5;
            adjust.x += half;
            adjust.z += half;
        }
        let dest_cell = self.grid.world_to_grid(adjust);
        let layer = self.grid.layer_for_destination(dest);
        let Some(adj) = self.grid.adjust_to_landing_destination_for(
            dest_cell,
            400,
            layer,
            radius,
            center_in_cell,
        ) else {
            return dest;
        };
        let mut world = self.grid.grid_to_world(adj);
        world.y = dest.y;
        world
    }

    /// C++ `Weapon::computeApproachTarget` aircraft branch + leftover
    /// `Pathfinder::adjustTargetDestination` so two Comanches do not share a hover cell.
    pub fn adjust_target_destination(
        &self,
        seeker: u32,
        objects: &HashMap<ObjectId, Object>,
        dest: Vec3,
        target_pos: Vec3,
        unit_radius: f32,
        surfaces: u32,
        is_crusher: bool,
        source_radius: f32,
        target_radius: f32,
        attack_range: f32,
        min_range: f32,
    ) -> Vec3 {
        let in_range = |goal: Vec3| {
            crate::game_logic::weapon_bootstrap::is_goal_pos_within_attack_range(
                goal,
                target_pos,
                attack_range,
                min_range,
                source_radius,
                target_radius,
            )
        };
        let mut dest = dest;

        let ai_store = gamelogic::ai::the_ai(); if let Ok(ai) = ai_store.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(pf) = pf.read() {
                    let mut dest3 = gamelogic::common::Coord3D::new(dest.x, dest.z, dest.y);
                    let _ = pf.adjust_target_destination_for(
                        surfaces,
                        is_crusher,
                        unit_radius,
                        Some(seeker),
                        &mut dest3,
                        |goal| in_range(Vec3::new(goal.x, goal.z, goal.y)),
                    );
                    dest = Vec3::new(dest3.x, dest.y, dest3.y);
                }
            }
        }

        let mut claimed = HashSet::new();
        let cell_size = self.grid.grid_size();
        for (id, obj) in objects {
            if id.0 == seeker {
                continue;
            }
            if !PathfindingGrid::is_aircraft_that_adjusts_destination(obj) {
                continue;
            }
            let Some(goal) = PathfindingGrid::aircraft_goal_dest(obj) else {
                continue;
            };
            let (or, oc) = PathfindingGrid::radius_and_center(obj.selection_radius, cell_size);
            let cell = self.grid.world_to_grid(goal);
            let mut num_above = or;
            if oc {
                num_above += 1;
            }
            for i in (cell.x - or)..(cell.x + num_above) {
                for j in (cell.y - or)..(cell.y + num_above) {
                    claimed.insert(GridPos::new(i, j));
                }
            }
        }

        let mut out = dest;
        if self
            .grid
            .adjust_target_destination(&mut out, unit_radius, seeker, &claimed, in_range)
        {
            out.y = dest.y;
            return out;
        }
        dest
    }

    /// Stamp occupancy then unstack landing dest for `seeker` (C++ checkDestination objID).
    pub fn adjust_landing_destination_for(
        &mut self,
        seeker: u32,
        objects: &HashMap<ObjectId, Object>,
        from: Vec3,
        dest: Vec3,
    ) -> Vec3 {
        self.grid.update_dynamic_obstacles(objects);
        self.grid.query_seeker_id = seeker;
        let unit_radius = objects
            .get(&ObjectId(seeker))
            .map(|o| o.selection_radius)
            .unwrap_or(0.0);
        let adj = self.adjust_to_landing_destination_radius(from, dest, unit_radius);
        self.grid.query_seeker_id = 0;
        adj
    }

    pub fn clear_static_blocks(&mut self) {
        self.grid.clear_static_blocks();
        self.crate_astar = None;
    }

    /// Mark the active host logic frame so dynamic obstacle rebuilds run once
    /// per frame across many find_path_ex calls.
    #[inline]
    pub fn note_logic_frame(&mut self, frame: u64) {
        self.logic_frame = frame;
    }

    /// Rebuild vehicle/structure dynamic blocks at most once per logic frame.
    #[inline]
    pub(super) fn ensure_dynamic_obstacles(&mut self, objects: &HashMap<ObjectId, Object>) {
        if self.ignore_obstacle_id.is_some() {
            self.grid
                .update_dynamic_obstacles_ignoring(objects, self.ignore_obstacle_id);
            // Do not cache an ignore-filtered occupancy stamp.
            self.dynamic_obstacle_frame = u64::MAX;
            return;
        }
        if self.dynamic_obstacle_frame != self.logic_frame {
            self.grid.update_dynamic_obstacles(objects);
            self.dynamic_obstacle_frame = self.logic_frame;
        }
    }

    /// C++ `AIUpdateInterface::ignoreObstacle` for the next `find_path_ex_*`.
    pub fn set_ignore_obstacle(&mut self, id: Option<ObjectId>) {
        self.ignore_obstacle_id = id;
        if id.is_some() {
            self.dynamic_obstacle_frame = u64::MAX;
        }
    }

    pub fn ignore_obstacle(&self) -> Option<ObjectId> {
        self.ignore_obstacle_id
    }

    /// Leftover `ignored_obstacle_cells`: footprint cells of `m_ignoreObstacleID`.
    pub(super) fn ignored_obstacle_cells(&self) -> Option<HashSet<GridCoord>> {
        let id = self.ignore_obstacle_id?.0;
        if id == 0 {
            return None;
        }
        let mut cells = HashSet::new();
        for y in 0..self.grid.height {
            for x in 0..self.grid.width {
                let pos = GridPos::new(x, y);
                if self.grid.cell_type(pos) != PathfindCellType::Obstacle {
                    continue;
                }
                if self
                    .grid
                    .obstacle_owner(pos)
                    .is_some_and(|(oid, _, _)| oid == id)
                {
                    cells.insert(GridCoord::new(x, y));
                }
            }
        }
        if cells.is_empty() {
            None
        } else {
            Some(cells)
        }
    }

    /// C++ `processPathfindQueue` reset of `m_cumulativeCellsAllocated` + extent.
    pub fn begin_pathfind_queue_frame(&mut self) {
        self.grid.refresh_logical_extent();
        self.cumulative_cells_allocated = 0;
    }

    pub fn pathfind_budget_remaining(&self) -> bool {
        self.cumulative_cells_allocated < self.pathfind_cells_per_frame
    }

    pub fn pop_pending_path(&mut self) -> Option<PendingHostPath> {
        self.pending_paths.pop_front()
    }

    pub(super) fn note_cells_allocated(&mut self, n: usize) {
        self.cumulative_cells_allocated = self
            .cumulative_cells_allocated
            .saturating_add(n.min(i32::MAX as usize) as i32);
    }

    #[cfg(test)]
    pub fn set_pathfind_cells_per_frame(&mut self, n: i32) {
        self.pathfind_cells_per_frame = n.max(1);
    }

    #[cfg(test)]
    pub fn cumulative_cells_allocated(&self) -> i32 {
        self.cumulative_cells_allocated
    }

    pub(super) fn sync_crate_astar(&mut self) {
        let w = self.grid.width.max(0) as usize;
        let h = self.grid.height.max(0) as usize;
        if w == 0 || h == 0 {
            self.crate_astar = None;
            return;
        }
        let stamp = self.grid.terrain_gen;
        let needs_rebuild = match &self.crate_astar {
            Some(c) => c.stamp != stamp || c.finder.width() != w || c.finder.height() != h,
            None => true,
        };
        if !needs_rebuild {
            return;
        }
        let mut finder = AStarPathfinder::new(w, h);
        for y in 0..self.grid.height {
            for x in 0..self.grid.width {
                let pos = GridPos::new(x, y);
                let coord = GridCoord::new(x, y);
                finder.set_cell_type(coord, self.grid.cell_type(pos));
                finder.set_pinched(coord, self.grid.is_pinched(pos));
                if self.grid.cell_type(pos) == PathfindCellType::Obstacle {
                    finder.set_cell_obstacle_id(
                        coord,
                        self.grid
                            .obstacle_owner(pos)
                            .map(|(id, _, _)| id)
                            .unwrap_or(1),
                        self.grid.is_obstacle_fence(pos),
                        self.grid.is_obstacle_transparent(pos),
                    );
                }
                let connect = self.grid.ground_connect_layer(pos);
                if connect != 0 {
                    finder
                        .set_cell_connect_layer(coord, PathfindLayerEnum::from_u32(connect as u32));
                }
            }
        }
        for layer in &self.grid.bridge_layers {
            let pf_layer = PathfindLayerEnum::from_u32(layer.id as u32);
            for ((x, y), (ty, connect)) in &layer.cells {
                let coord = GridCoord::new(*x, *y);
                finder.set_cell_type_on_layer(coord, pf_layer, *ty);
                if *connect != 0 {
                    finder.set_cell_connect_layer_on_layer(
                        coord,
                        pf_layer,
                        PathfindLayerEnum::from_u32(*connect as u32),
                    );
                }
            }
        }
        if !self.grid.wall_cells.is_empty() {
            let pf_layer = PathfindLayerEnum::Wall;
            for ((x, y), ty) in &self.grid.wall_cells {
                finder.set_cell_type_on_layer(GridCoord::new(*x, *y), pf_layer, *ty);
            }
        }
        self.crate_astar = Some(HostCrateAStar { finder, stamp });
    }

    pub(super) fn host_to_crate_coord(&self, pos: GridPos) -> GridCoord {
        GridCoord::new(pos.x, pos.y)
    }

    pub(super) fn crate_path_to_world(&self, cells: &[GridCoord]) -> Vec<Vec3> {
        let center = self.seeker_center_in_cell;
        cells
            .iter()
            .map(|c| {
                let pos = GridPos::new(c.x, c.y);
                if let Some(id) = self.grid.host_deck_layer_at(pos) {
                    self.grid.adjust_coord_to_cell_on_layer(
                        pos,
                        center,
                        PathfindLayerEnum::from_u32(id as u32),
                    )
                } else {
                    self.grid.adjust_coord_to_cell(pos, center)
                }
            })
            .collect()
    }

    /// C++ hierarchical bridge start/end cells (AIPathfind.cpp:7595-7623).
    pub(super) fn hierarchical_bridge_jumps(&self) -> Vec<(GridCoord, GridCoord)> {
        let mut out = Vec::new();
        for layer in &self.grid.bridge_layers {
            let start = self.grid.world_to_grid(Vec3::new(
                (layer.from_left.x + layer.from_right.x) * 0.5,
                0.0,
                (layer.from_left.z + layer.from_right.z) * 0.5,
            ));
            let end = self.grid.world_to_grid(Vec3::new(
                (layer.to_left.x + layer.to_right.x) * 0.5,
                0.0,
                (layer.to_left.z + layer.to_right.z) * 0.5,
            ));
            out.push((
                GridCoord::new(start.x, start.y),
                GridCoord::new(end.x, end.y),
            ));
            let connects: Vec<GridCoord> = layer
                .cells
                .iter()
                .filter(|(_, (_, connect))| *connect == PathfindLayerEnum::Ground as u8)
                .map(|((x, y), _)| GridCoord::new(*x, *y))
                .collect();
            if connects.len() >= 2 {
                let mut lo = connects[0];
                let mut hi = connects[0];
                for &c in &connects {
                    if c.x + c.y < lo.x + lo.y {
                        lo = c;
                    }
                    if c.x + c.y > hi.x + hi.y {
                        hi = c;
                    }
                }
                if lo != hi {
                    out.push((lo, hi));
                }
            }
        }
        out
    }

    /// C++ `Pathfinder::findPath` via crate A* after hierarchical zone-block prune.
    /// Falls back to the host grid A* if crate types cannot run (empty grid).

    pub(super) fn find_path_via_crate(
        &mut self,
        start: GridPos,
        goal: GridPos,
        surfaces: u32,
        is_crusher: bool,
        start_layer: PathfindLayerEnum,
        dest_layer: PathfindLayerEnum,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        self.grid.refresh_logical_extent();
        self.apply_seeker_human_flag();
        self.grid.query_seeker_id = self.seeker_id.map(|id| id.0).unwrap_or(0);
        self.grid
            .set_query_footprint(self.seeker_path_diameter, is_crusher);
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let start_seed = self.grid.clamp_pos(start);
        let goal_seed = self.grid.clamp_pos(goal);
        self.grid.query_from = Some(start_seed);
        self.grid.query_orig_dest = Some(start_seed);
        // C++ adjustDestination: on failure leave dest unchanged (no static-open fallback).
        let start = self
            .grid
            .adjust_destination_on_layer(
                start_seed,
                surfaces,
                is_crusher,
                64,
                self.seeker_player,
                crusher_level,
                start_layer,
            )
            .unwrap_or(start_seed);
        // C++ adjustDestination: snap water/cliff/impassable/occupied clicks
        // on destinationLayer (AIPathfind.cpp:5352-5355).
        self.grid.query_from = Some(start);
        self.grid.query_orig_dest = Some(goal_seed);
        let mut goal = self
            .grid
            .adjust_destination_on_layer(
                goal_seed,
                surfaces,
                is_crusher,
                400,
                self.seeker_player,
                crusher_level,
                dest_layer,
            )
            .filter(|c| {
                // C++ internalFindPath refuses a goal that fails
                // validMovementPosition (AIPathfind.cpp:6561-6568); the zone
                // gate in checkDestination must not accept Impassable/Obstacle.
                !matches!(
                    self.grid.resolved_cell_type(dest_layer, *c),
                    PathfindCellType::Impassable
                        | PathfindCellType::Obstacle
                        | PathfindCellType::BridgeImpassable
                )
            })
            .unwrap_or(goal_seed);
        self.grid.query_from = None;
        self.grid.query_orig_dest = None;
        let ignore_cells = self.ignored_obstacle_cells();
        let ignore_covers = |pos: GridPos| {
            ignore_cells
                .as_ref()
                .is_some_and(|s| s.contains(&GridCoord::new(pos.x, pos.y)))
        };
        let start_obstacle = self.grid.cell_type(start) == PathfindCellType::Obstacle;
        // C++ internalFindPath bails before seeding when the goal fails
        // validMovementPosition (AIPathfind.cpp:6561-6568) — a wall click
        // must fail closed, never fall back to closest on the far side.
        if !ignore_covers(goal)
            && matches!(
                self.grid.resolved_cell_type(dest_layer, goal),
                PathfindCellType::Impassable
                    | PathfindCellType::Obstacle
                    | PathfindCellType::BridgeImpassable
            )
            && self.grid.cell_type(goal) == PathfindCellType::Impassable
        {
            return None;
        }
        // C++ ignoreObstacle / tunneling uses terrain zones (AIPathfind.cpp:6544-6550).
        if (surfaces & SURFACE_AIR) == 0 {
            let from_w = self.grid.grid_to_world(start);
            let to_w = self.grid.grid_to_world(goal);
            let ignore_or_tunnel = start_obstacle || ignore_covers(start) || ignore_covers(goal);
            let quick = self.grid.quick_path_exists_for_crusher(from_w, to_w, surfaces, is_crusher);
            if !ignore_or_tunnel && !quick {
                return None;
            }
        }
        if self
            .grid
            .has_allied_goal_on(goal, self.seeker_player, dest_layer)
        {
            if let Some(adj) = self.grid.adjust_destination_on_layer(
                goal,
                surfaces,
                is_crusher,
                64,
                self.seeker_player,
                crusher_level,
                dest_layer,
            ) {
                if !self
                    .grid
                    .has_allied_goal_on(adj, self.seeker_player, dest_layer)
                {
                    goal = adj;
                }
            }
        }
        if !self
            .grid
            .cell_passable_for_layer(start, start_layer, surfaces, is_crusher)
            || !self
                .grid
                .cell_passable_for_layer(goal, dest_layer, surfaces, is_crusher)
        {
            // Still try — crusher fences / air / tunneling / ignoreObstacle may pass.
            if start_layer == PathfindLayerEnum::Ground
                && self.grid.is_static_blocked(start)
                && !self.grid.is_obstacle_fence(start)
                && !start_obstacle
                && !ignore_covers(start)
            {
                return None;
            }
        }
        if start == goal {
            return Some(vec![self
                .grid
                .adjust_coord_to_cell(start, self.seeker_center_in_cell)]);
        }
        let start_c = self.host_to_crate_coord(start);
        let goal_c = self.host_to_crate_coord(goal);
        let start_is_obstacle = self.crate_astar.as_ref().is_some_and(|crate_pf| {
            crate_pf.finder.get_cell_type_on_layer(start_c, start_layer)
                == Some(PathfindCellType::Obstacle)
        });
        // Seeker policy inputs to the crate A* call (leftover downhill-only etc).
        let downhill_only = self.seeker_downhill_only;
        let is_human = self.seeker_is_human;
        let cell_allowed = |c: GridCoord| {
            if is_human {
                self.grid.in_logical_extent(GridPos::new(c.x, c.y))
            } else {
                true
            }
        };
        let is_dozer = self.seeker_is_dozer;
        let seeker_player = self.seeker_player;
        let seeker_team = self.seeker_team;
        let ally_mask = seeker_player
            .map(|p| self.grid.ally_mask_for(p))
            .unwrap_or(0);
        let dozer_ok = |c: GridCoord| {
            if !is_dozer {
                return false;
            }
            let Some((_id, owner, team)) = self.grid.obstacle_owner(GridPos::new(c.x, c.y)) else {
                return false;
            };
            if let (Some(sp), Some(op)) = (seeker_player, owner) {
                if sp == op {
                    return true;
                }
                if (ally_mask & (1u16 << op.min(15))) != 0 {
                    return true;
                }
                return matches!(team, Some(Team::Neutral));
            }
            match (seeker_team, team) {
                (Some(a), Some(b)) if a == b => true,
                (Some(Team::Neutral), _) | (_, Some(Team::Neutral)) => true,
                _ => false,
            }
        };
        let dozer_ok_ref: Option<&dyn Fn(GridCoord) -> bool> = if is_dozer {
            Some(&dozer_ok as &dyn Fn(GridCoord) -> bool)
        } else {
            None
        };
        let seed_line = !start_is_obstacle && !downhill_only;
        // C++ findPath: clearPassableFlags + findHierarchicalPath corridor
        // (AIPathfind.cpp:6375-6381). Fine A* then consults isPassable.
        let jumps = self.hierarchical_bridge_jumps();
        if let Some(crate_pf) = self.crate_astar.as_mut() {
            crate_pf
                .finder
                .apply_hierarchical_zone_prune(start_c, goal_c, surfaces, is_crusher, &jumps);
        }

        let (exact_path, examined) = {
            let width = self.grid.width;
            let occ_fixed = self.grid.occ_fixed_mask.clone();
            let occ_moving = self.grid.occ_moving_mask.clone();
            let occ_goal = self.grid.occ_goal_mask.clone();
            let occ_infantry = self.grid.occ_infantry_mask.clone();
            let occ_crush = self.grid.occ_fixed_max_crushable.clone();
            let occ_pos_unit = self.grid.occ_pos_unit.clone();
            let occ_pos_player = self.grid.occ_pos_player.clone();
            let occ_pos_flags = self.grid.occ_pos_flags.clone();
            let occ_pos_crush = self.grid.occ_pos_crushable.clone();
            let layer_occ = self.grid.layer_occ.clone();
            let start_layer_id = start_layer as u8;
            let dest_layer_id = dest_layer as u8;
            let layer_cells = |id: u8| -> HashSet<(i32, i32)> {
                if id <= PathfindLayerEnum::Ground as u8 {
                    return HashSet::new();
                }
                if id == LAYER_WALL_ID {
                    return self.grid.wall_cells.keys().copied().collect();
                }
                self.grid
                    .bridge_layers
                    .iter()
                    .find(|layer| layer.id == id)
                    .map(|layer| layer.cells.keys().copied().collect())
                    .unwrap_or_default()
            };
            let start_layer_cells = layer_cells(start_layer_id);
            let dest_layer_cells = layer_cells(dest_layer_id);
            let seeker = self.seeker_player;
            let seeker_inf = self.seeker_is_infantry;
            let ally_mask = seeker.map(|p| self.grid.ally_mask_for(p)).unwrap_or(0);
            let height = self.grid.height;
            let path_diameter = self.seeker_path_diameter;
            let cell_types = self.grid.cell_types.clone();
            let fence_bits = self.grid.fence_bits.clone();
            let diameter_crusher = is_crusher;
            let start_for_cost = start;
            let extra = move |c: GridCoord| {
                if c.x < 0 || c.y < 0 || c.x >= width {
                    return 0;
                }
                // C++ internalFindPath has NO clearCellForDiameter gate on
                // neighbor expansion (AIPathfind.cpp:6125-6260; the diameter
                // check lives only in adjustDestination / findGroundPath).
                // The hq-985ts one-cell-corridor veto below is a repo guard
                // for generic vehicles; dozer seekers follow the C++
                // dozerHack (AIPathfind.cpp:6208-6225) which admits obstacle
                // gaps of any width, so they skip it.
                if path_diameter >= 2 && dozer_ok_ref.is_none() {
                    let d = clear_cell_for_diameter_impl(
                        width,
                        height,
                        &cell_types,
                        &fence_bits,
                        &occ_fixed,
                        &occ_crush,
                        diameter_crusher,
                        GridPos::new(c.x, c.y),
                        path_diameter,
                    );
                    if d != path_diameter {
                        return u32::MAX / 8;
                    }
                }
                let key = (c.x, c.y);
                let layer_id = if start_layer_id > PathfindLayerEnum::Ground as u8
                    && start_layer_cells.contains(&key)
                {
                    Some(start_layer_id)
                } else if dest_layer_id > PathfindLayerEnum::Ground as u8
                    && dest_layer_cells.contains(&key)
                {
                    Some(dest_layer_id)
                } else {
                    None
                };
                let idx = c.y as usize * width as usize + c.x as usize;
                let (fixed, moving, goal_m, infantry, crush, pos_u, pos_p, pos_flags, pos_cr) =
                    if let Some(lid) = layer_id {
                        if let Some(occ) = layer_occ.get(&lid) {
                            (
                                occ.occ_fixed_mask.get(&key).copied().unwrap_or(0),
                                occ.occ_moving_mask.get(&key).copied().unwrap_or(0),
                                occ.occ_goal_mask.get(&key).copied().unwrap_or(0),
                                occ.occ_infantry_mask.get(&key).copied().unwrap_or(0),
                                occ.occ_fixed_max_crushable.get(&key).copied().unwrap_or(0),
                                occ.occ_pos_unit.get(&key).copied().unwrap_or(0),
                                occ.occ_pos_player.get(&key).copied().unwrap_or(0),
                                occ.occ_pos_flags.get(&key).copied().unwrap_or(0),
                                occ.occ_pos_crushable.get(&key).copied().unwrap_or(0),
                            )
                        } else {
                            (0, 0, 0, 0, 0, 0, 0, 0, 0)
                        }
                    } else {
                        (
                            occ_fixed.get(idx).copied().unwrap_or(0),
                            occ_moving.get(idx).copied().unwrap_or(0),
                            occ_goal.get(idx).copied().unwrap_or(0),
                            occ_infantry.get(idx).copied().unwrap_or(0),
                            occ_crush.get(idx).copied().unwrap_or(0),
                            occ_pos_unit.get(idx).copied().unwrap_or(0),
                            occ_pos_player.get(idx).copied().unwrap_or(0),
                            occ_pos_flags.get(idx).copied().unwrap_or(0),
                            occ_pos_crush.get(idx).copied().unwrap_or(0),
                        )
                    };
                // C++ INFANTRY_MOVES_THROUGH_INFANTRY: stream even when a goal is set.
                if seeker_inf && infantry != 0 && (fixed | moving) == infantry {
                    return 0;
                }
                if fixed == 0 && moving == 0 && goal_m == 0 {
                    return 0;
                }
                let Some(player) = seeker else {
                    return 3 * COST_DIAGONAL;
                };
                let bit = 1u16 << player.min(15);
                let friend = bit | ally_mask;
                if seeker_inf && (infantry & !bit) != 0 && (fixed & !bit) == (infantry & !bit) {
                    let leftover_fixed = fixed & !infantry;
                    let leftover_moving = moving & !infantry;
                    if leftover_fixed == 0 && leftover_moving == 0 {
                        return 0;
                    }
                }
                let mut extra = 0u32;
                // C++ checkForMovement verdict on the cell's single m_posUnitID
                // (AIPathfind.cpp:5020-5066): ally fixed → allyFixed cost, enemy
                // uncrushable → impassable. Bitset view only when no identity.
                let pos_moving = pos_flags & 1 != 0;
                if pos_u != 0 {
                    if (pos_p & friend) == 0 {
                        let crushable = crusher_level > 0 && crusher_level > pos_cr;
                        if !crushable {
                            return u32::MAX / 8;
                        }
                    } else if !pos_moving && (fixed & friend) != 0 {
                        extra += 3 * COST_DIAGONAL;
                    }
                } else {
                    if (fixed & !friend) != 0 {
                        let max_c = crush;
                        if crusher_level == 0 || crusher_level <= max_c {
                            return u32::MAX / 8;
                        }
                    }
                    if (fixed & friend) != 0 {
                        extra += 3 * COST_DIAGONAL;
                    }
                }
                extra
            };
            let extra_ref = &extra;
            let line_ok = |c: GridCoord| {
                if extra_ref(c) >= u32::MAX / 8 {
                    return false;
                }
                // C++ examineCellsCallback: abort on allyFixed / any enemyFixed
                // (crushable included). extra() only MAX/8s uncrushable enemies.
                let ally = self
                    .seeker_player
                    .map(|p| self.grid.ally_mask_for(p))
                    .unwrap_or(0);
                self.grid.seed_line_occupancy_ok(
                    GridPos::new(c.x, c.y),
                    self.seeker_player,
                    ally,
                    self.seeker_is_infantry,
                    start_layer,
                )
            };
            let ground_h = |c: GridCoord| {
                let w = self.grid.grid_to_world(GridPos::new(c.x, c.y));
                sample_host_ground_height(w.x, w.z)
            };
            if is_human
                && (!self.grid.in_logical_extent(start) || !self.grid.in_logical_extent(goal))
            {
                if !self.grid.in_logical_extent(start) {
                    return None;
                }
            }
            // C++ internalFindPath always runs on the live pathfind map; no
            // secondary solver exists. Without A* there is no path — returning
            // the legacy grid result would let non-dozers cross obstacle gaps.
            let Some(crate_pf) = self.crate_astar.as_ref() else {
                return None;
            };
            let run = |allow_partial: bool| {
                crate_pf.finder.find_path_with_start_layer(
                    start_c,
                    goal_c,
                    start_layer,
                    dest_layer,
                    surfaces,
                    is_crusher,
                    MAX_PATH_ITERATIONS,
                    allow_partial,
                    ignore_cells.as_ref(),
                    Some(extra_ref as &dyn Fn(GridCoord) -> u32),
                    downhill_only,
                    Some(&ground_h as &dyn Fn(GridCoord) -> f32),
                    None,
                    Some(&line_ok as &dyn Fn(GridCoord) -> bool),
                    seed_line,
                    start_is_obstacle,
                    Some(&cell_allowed as &dyn Fn(GridCoord) -> bool),
                    dozer_ok_ref,
                )
            };

            let exact = run(false);
            let examined = exact.as_ref().map(|(_, n)| *n).unwrap_or(0);
            let exact_path = exact.map(|(path, _)| path);
            (exact_path, examined)
        };
        self.note_cells_allocated(examined);
        if let Some(cells) = exact_path {
            let world = self.crate_path_to_world(&cells);
            return Some(self.grid.optimize_ground_path_ex(
                &world,
                surfaces,
                is_crusher,
                self.seeker_player,
                crusher_level,
            ));
        }
        // C++ Pathfinder::findPath (AIPathfind.cpp:6364-6436) returns NULL
        // when internalFindPath fails. findClosestPath is a separate service
        // invoked by AIUpdateInterface::computePath (AIUpdate.cpp:1713-1717)
        // — never an unconditional fallback inside the pathfinder. The one
        // carried-over closest walk is the structure-sealed goal (every
        // boundary cell a CELL_OBSTACLE building, hq-985ts click-on-building
        // UX); a terrain-sealed goal — including a doze-able obstacle gap in
        // an Impassable wall — fails closed so non-dozers cannot dozerHack
        // across it (hq-8kkhs).
        if self.grid.structure_sealed_goal(start, goal) {
            let from_w = self.grid.grid_to_world(start);
            let to_w = self.grid.grid_to_world(goal);
            if let Some(closest) =
                self.find_closest_path(from_w, to_w, surfaces, is_crusher, is_human)
            {
                return Some(closest);
            }
        }
        None
    }

    /// Queue a path request for next-frame resolve (C++ queueForPath).
    /// Duplicate ObjectID updates dest in place. Full queue refuses the new request.
    pub fn queue_path(&mut self, req: PendingHostPath) -> bool {
        if let Some(existing) = self
            .pending_paths
            .iter_mut()
            .find(|p| p.unit_id == req.unit_id)
        {
            *existing = req;
            return true;
        }
        if self.pending_paths.len() >= PATHFIND_QUEUE_LEN {
            return false;
        }
        self.pending_paths.push_back(req);
        true
    }

    pub fn take_pending_paths(&mut self) -> Vec<PendingHostPath> {
        self.pending_paths.drain(..).collect()
    }

    pub fn pending_path_count(&self) -> usize {
        self.pending_paths.len()
    }

    pub fn pending_paths(&self) -> impl Iterator<Item = &PendingHostPath> {
        self.pending_paths.iter()
    }

    pub fn stamp_rubble_at_world(&mut self, world: Vec3, radius_cells: i32) {
        let cell = self.grid.world_to_grid(world);
        self.grid.stamp_rubble_footprint(cell, radius_cells);
        // Destroy path stamps after sync_structure_path_blocks; rebuild so
        // rubble is not left in the previous Clear zone (hq-0r5xs).
        self.grid.rebuild_path_zones();
    }

    pub fn block_structure_at_world(&mut self, pos: Vec3, radius_cells: i32) {
        let center = self.grid.world_to_grid(pos);
        self.grid.block_structure_footprint(center, radius_cells);
    }

    pub fn is_attack_view_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.is_attack_view_blocked_for(from, to, None, None)
    }

    /// C++ `TheAI->getAiData()->m_attackUsesLineOfSight` (default true).
    pub(super) fn attack_uses_line_of_sight() -> bool {
        if let Some(data) = game_engine::common::ini::get_ai_data_store().get_active() {
            return data.attack_uses_line_of_sight;
        }
        gamelogic::ai::the_ai()
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.attack_uses_line_of_sight)
            })
            .unwrap_or(true)
    }

    /// C++ `Pathfinder::isAttackViewBlockedByObstacle` leftover gates + static Bresenham.
    pub fn is_attack_view_blocked_for(
        &self,
        from: Vec3,
        to: Vec3,
        attacker: Option<&Object>,
        victim: Option<&Object>,
    ) -> bool {
        if !Self::attack_uses_line_of_sight() {
            return false;
        }
        if let Some(atk) = attacker {
            if !atk.is_kind_of(KindOf::AttackNeedsLineOfSight) {
                return false;
            }
        }
        if let Some(v) = victim {
            // C++ isSignificantlyAboveTerrain — do not LOS-check flying victims.
            if v.is_significantly_above_terrain() {
                return false;
            }
        }
        // LOS_TERRAIN: leftover is_attack_view_blocked_by_obstacle
        // (skip Immobile — cannot path around terrain blockage).
        if let Some(atk) = attacker {
            if !atk.is_kind_of(KindOf::Immobile) {
                let has_weapon = atk.weapon.is_some() || atk.secondary_weapon.is_some();
                if has_weapon {
                    // leftover Weapon::is_clear_goal_firing_line_of_sight_terrain
                    // uses GeometryInfo::getMaxHeightAbovePosition as the eye.
                    let eye_from = Self::leftover_firing_los_eye_y(atk, from.y);
                    let eye_to = victim
                        .map(|v| Self::leftover_firing_los_eye_y(v, to.y))
                        .unwrap_or(to.y);
                    if !self.is_clear_line_of_sight_terrain(
                        Vec3::new(from.x, eye_from, from.z),
                        Vec3::new(to.x, eye_to, to.z),
                    ) {
                        return true;
                    }
                } else if !self.is_clear_line_of_sight_terrain(from, to) {
                    // leftover: no current weapon → terrain.is_clear_line_of_sight
                    return true;
                }
            }
        }
        let mut skip_ids: Vec<u32> = Vec::new();
        if let Some(atk) = attacker {
            skip_ids.push(atk.id.0);
            if let Some(c) = atk.contained_by {
                skip_ids.push(c.0);
            }
            if let Some(p) = atk.producer_id {
                skip_ids.push(p.0);
            }
        }
        if let Some(v) = victim {
            skip_ids.push(v.id.0);
            if let Some(p) = v.producer_id {
                skip_ids.push(p.0);
            }
        }
        let skip_count = if attacker.is_some() {
            let layer = self.grid.layer_for_destination(from);
            if !matches!(
                layer,
                PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid
            ) {
                3
            } else {
                0
            }
        } else {
            0
        };
        self.grid
            .is_attack_view_blocked_static_ex(from, to, skip_count, &skip_ids)
    }

    /// Install coarse pathfind-cell heights for LOS_TERRAIN (tests + save restore).
    pub fn set_terrain_height_samples(&mut self, width: i32, height: i32, values: Vec<f32>) {
        if width > 0 && height > 0 && values.len() == (width as usize) * (height as usize) {
            self.terrain_height_samples = Some((width, height, values));
        }
    }

    pub(super) fn sample_terrain_height(&self, x: f32, z: f32) -> f32 {
        if let Some((w, h, vals)) = &self.terrain_height_samples {
            let cell = self.grid.world_to_grid(Vec3::new(x, 0.0, z));
            if cell.x >= 0 && cell.y >= 0 && cell.x < *w && cell.y < *h {
                let idx = (cell.y * *w + cell.x) as usize;
                if let Some(&v) = vals.get(idx) {
                    return v;
                }
            }
        }
        sample_host_ground_height(x, z)
    }

    /// Leftover `Weapon::is_clear_goal_firing_line_of_sight_terrain` eye:
    /// `GeometryInfo::getMaxHeightAbovePosition` when authored, else selection radius.
    pub(super) fn leftover_firing_los_eye_y(obj: &Object, pos_y: f32) -> f32 {
        let geom = &obj.thing.template.geometry_info;
        let h = if geom.authored {
            geom.max_height_above_position()
        } else {
            obj.selection_radius.max(1.0)
        };
        pos_y + h
    }

    /// C++ `Weapon::isClearGoalFiringLineOfSightTerrain` residual (height samples).
    /// Fail-open when leftover terrain is empty and no cache is installed.
    pub fn is_clear_line_of_sight_terrain(&self, from: Vec3, to: Vec3) -> bool {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let dist_xz = (dx * dx + dz * dz).sqrt();
        if dist_xz <= 0.001 {
            return true;
        }
        let from_y = from.y;
        let to_y = to.y;
        let step_len = 10.0_f32;
        let steps = (dist_xz / step_len).ceil().clamp(2.0, 512.0) as u32;
        const CLEARANCE: f32 = 5.0;
        for i in 1..steps {
            let tfrac = i as f32 / steps as f32;
            let x = from.x + dx * tfrac;
            let z = from.z + dz * tfrac;
            let expected_y = from_y + (to_y - from_y) * tfrac;
            if self.sample_terrain_height(x, z) > expected_y + CLEARANCE {
                return false;
            }
        }
        true
    }

    /// C++ `Pathfinder::snapClosestGoalPosition` (AIPathfind.cpp:5101-5156).
    pub fn snap_closest_goal_position(
        &self,
        pos: Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
    ) -> Vec3 {
        let cell_size = self.grid.grid_size();
        let (radius, center_in_cell) = PathfindingGrid::radius_and_center(unit_radius, cell_size);
        let mut adjust = pos;
        if !center_in_cell {
            adjust.x += cell_size * 0.5;
            adjust.z += cell_size * 0.5;
        }
        let layer = self.grid.layer_for_destination(pos);
        let cell = self.grid.world_to_grid(adjust);
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        // Leftover/C++ adjustCoordToCell: first snap LAYER_GROUND, neighborhood on dest layer.
        let snap_ground = |c: GridPos| self.grid.adjust_coord_to_cell(c, center_in_cell);
        let snap_layer = |c: GridPos| {
            self.grid
                .adjust_coord_to_cell_on_layer(c, center_in_cell, layer)
        };
        let dest_ok = |c: GridPos| {
            self.grid.destination_cell_ok(
                c,
                surfaces,
                is_crusher,
                self.seeker_player,
                crusher_level,
                layer,
            )
        };
        let mut out = snap_ground(cell);
        if dest_ok(cell) {
            return out;
        }
        for i in (cell.x - 1)..(cell.x + 2) {
            for j in (cell.y - 1)..(cell.y + 2) {
                let c = GridPos::new(i, j);
                if !self.grid.is_valid_pos(c) {
                    continue;
                }
                if dest_ok(c) {
                    return snap_layer(c);
                }
            }
        }
        if radius == 0 {
            for i in (cell.x - 1)..(cell.x + 2) {
                for j in (cell.y - 1)..(cell.y + 2) {
                    let c = GridPos::new(i, j);
                    if !self.grid.is_valid_pos(c) {
                        continue;
                    }
                    let bits = self.grid.occupancy_bits(c, layer);
                    let own = self.seeker_id.map(|id| id.0).unwrap_or(0);
                    if bits.goal_unit == 0 || (own != 0 && bits.goal_unit == own) {
                        return snap_layer(c);
                    }
                }
            }
            for i in (cell.x - 1)..(cell.x + 2) {
                for j in (cell.y - 1)..(cell.y + 2) {
                    let c = GridPos::new(i, j);
                    if !self.grid.is_valid_pos(c) {
                        continue;
                    }
                    if self.grid.occupancy_bits(c, layer).fixed == 0 {
                        return snap_layer(c);
                    }
                }
            }
        }
        let _ = out;
        snap_ground(cell)
    }

    /// C++ snapClosestGoalPosition at plant (AIStates idle restake / group pad).
    /// Occupancy ignores `seeker` so the arriver does not block their own cell.
    pub fn snap_plant_goal(
        &mut self,
        pos: Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
        seeker: ObjectId,
        seeker_player: Option<u32>,
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec3 {
        self.seeker_id = Some(seeker);
        self.seeker_player = seeker_player;
        self.grid.query_seeker_id = seeker.0;
        self.grid
            .update_dynamic_obstacles_ignoring(objects, Some(seeker));
        self.snap_closest_goal_position(pos, surfaces, is_crusher, unit_radius)
    }

    /// Rebuild structure static obstacles from live objects (map load / bulk sync).
    /// Does not clear terrain slope blocks — only ORs structure footprints.
    /// C++ `addObjectToPathfindMap` includes scaffolds (DozerAIUpdate.cpp:1698-1699).
    pub fn apply_structure_static_blocks(&mut self, objects: &HashMap<ObjectId, Object>) {
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        for obj in objects.values() {
            let rubble = PathfindingGrid::object_is_pathfind_rubble(obj);
            let blast_crater = obj.is_kind_of(KindOf::BlastCrater);
            // C++ classifyObjectFootprint(!insert) returns for BLAST_CRATER.
            if !obj.is_alive() && !rubble && !blast_crater {
                continue;
            }
            if rubble && obj.is_kind_of(KindOf::Structure) && !blast_crater {
                if let Some((a, b)) = self.grid.classify_object_footprint(obj, true) {
                    PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, a);
                    PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, b);
                    did = true;
                }
                continue;
            }
            if !obj.is_alive() && !blast_crater {
                continue;
            }
            if let Some((a, b)) = self.grid.classify_object_footprint(obj, false) {
                PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, a);
                PathfindingGrid::expand_stamp_bounds(&mut lo, &mut hi, b);
                did = true;
            }
        }
        self.grid
            .restamp_permanent_blast_craters(&mut lo, &mut hi, &mut did);
        if did {
            self.grid.refresh_pinched_bounds(lo, hi);
        }
        self.grid.rebuild_path_zones();
        self.sync_wall_pieces_from_objects(objects);
    }

    /// Incremental leftover classify + pinch for one live structure (dozer place).
    pub fn classify_and_pinch_object(&mut self, obj: &Object) {
        if let Some((lo, hi)) = self.grid.classify_object_footprint(obj, false) {
            self.grid.refresh_pinched_bounds(lo, hi);
            self.grid.rebuild_path_zones();
        }
    }

    /// C++ `Pathfinder::createAWallFromMyFootprint` — trains are mobile vehicles.
    pub fn create_wall_from_object(&mut self, obj: &Object) {
        if self.grid.create_wall_from_object(obj).is_some() {
            self.grid.rebuild_path_zones();
        }
    }

    /// C++ `Pathfinder::removeWallFromMyFootprint`.
    pub fn remove_wall_from_object(&mut self, obj: &Object) {
        if self.grid.remove_wall_from_object(obj).is_some() {
            self.grid.rebuild_path_zones();
        }
    }

    /// C++ `Pathfinder::addWallPiece` from a live host object.
    pub fn add_wall_piece_from_object(&mut self, obj: &Object) {
        let geom = &obj.thing.template.geometry_info;
        let major = if geom.authored && geom.major_radius > 0.0 {
            geom.major_radius
        } else {
            obj.selection_radius.max(1.0)
        };
        let minor = if geom.authored {
            if matches!(geom.geom_type, crate::game_logic::HostGeometryType::Sphere) {
                major
            } else {
                geom.minor_radius.max(0.1)
            }
        } else {
            major
        };
        if self.grid.wall_height <= 0.0 && geom.authored && geom.height > 0.0 {
            self.grid.wall_height = geom.height;
        }
        if self.grid.wall_height <= 0.0 {
            let ai_store = gamelogic::ai::the_ai(); if let Ok(ai) = ai_store.read() {
                if let Ok(data) = ai.get_ai_data().read() {
                    if data.wall_height > 0.0 {
                        self.grid.wall_height = data.wall_height;
                    }
                }
            }
        }
        self.grid.add_wall_piece(
            obj.id.0,
            obj.get_position(),
            obj.get_orientation(),
            major,
            minor,
        );
    }

    pub fn remove_wall_piece(&mut self, id: ObjectId) {
        self.grid.remove_wall_piece(id.0);
    }

    pub fn is_point_on_wall(&self, pos: Vec3) -> bool {
        self.grid.is_point_on_wall(pos)
    }

    pub fn set_wall_height(&mut self, h: f32) {
        self.grid.set_wall_height(h);
    }

    pub fn wall_height(&self) -> f32 {
        self.grid.wall_height()
    }

    /// Rebuild wall pieces from live `WALK_ON_TOP_OF_WALL` objects.
    pub fn sync_wall_pieces_from_objects(&mut self, objects: &HashMap<ObjectId, Object>) {
        self.grid.wall_pieces.clear();
        self.grid.wall_cells.clear();
        for obj in objects.values() {
            if obj.is_alive() && obj.is_kind_of(KindOf::WalkOnTopOfWall) {
                self.add_wall_piece_from_object(obj);
            }
        }
        if self.grid.wall_pieces.is_empty() {
            self.grid.terrain_gen = self.grid.terrain_gen.wrapping_add(1);
        }
    }
}
