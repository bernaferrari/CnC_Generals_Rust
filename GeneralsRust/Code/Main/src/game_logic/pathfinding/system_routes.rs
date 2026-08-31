use super::*;

impl PathfindingSystem {
    /// `aircraft`: apply C++ tall-building aircraft path-around residual after A*.
    pub fn find_path_ex(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
        aircraft: bool,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        self.find_path_ex_surfaces(
            start,
            goal,
            objects,
            aircraft,
            if aircraft {
                SURFACE_AIR
            } else {
                SURFACE_GROUND
            },
            false,
            mover,
        )
    }

    /// Live path: crate `AStarPathfinder::find_path_ex` (AIPathfind.cpp:6438).
    pub fn find_path_ex_surfaces(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
        aircraft: bool,
        surfaces: u32,
        is_crusher: bool,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        self.ensure_dynamic_obstacles(objects);
        self.bind_seeker_from_mover(objects, mover);
        self.apply_seeker_human_flag();
        // Live seeker CrusherLevel wins so find_path / find_path_ex still crush.
        let is_crusher = is_crusher || self.seeker_crusher_level > 0;
        if is_crusher && self.seeker_crusher_level == 0 {
            self.seeker_crusher_level = 1;
        }
        self.grid
            .set_query_footprint(self.seeker_path_diameter, is_crusher);

        let mut goal = goal;
        self.grid.query_seeker_id = self.seeker_id.map(|id| id.0).unwrap_or(0);
        let seeker_adjusts = self
            .seeker_id
            .and_then(|id| objects.get(&id))
            .is_some_and(PathfindingGrid::is_aircraft_that_adjusts_destination);
        if aircraft && seeker_adjusts {
            self.grid.query_check_for_aircraft = true;
            let dest_layer = self.grid.layer_for_destination(goal);
            if let Some(adj) = self.grid.adjust_destination_on_layer(
                self.grid.world_to_grid(goal),
                surfaces,
                is_crusher,
                400,
                self.seeker_player,
                0,
                dest_layer,
            ) {
                let mut snapped = self.grid.grid_to_world(adj);
                snapped.y = goal.y;
                goal = snapped;
            }
            self.grid.query_check_for_aircraft = false;
        } else if !aircraft {
            // C++ AIState::onEnter (AIStates.cpp:1638-1645):
            // adjustDestination first; snapClosestGoalPosition is the
            // FALLBACK when adjustDestination fails — not an unconditional
            // re-center. Re-centering a valid clamped goal walks it to the
            // cell center, which can drift past a clamp margin.
            let unit_radius = self
                .seeker_id
                .and_then(|id| objects.get(&id))
                .map(|o| o.selection_radius)
                .unwrap_or(0.0);
            let cell = self.grid.world_to_grid(goal);
            let crusher_level = if is_crusher || self.seeker_crusher_level > 0 {
                self.seeker_crusher_level.max(1)
            } else {
                0
            };
            let dest_ok = self.grid.check_destination_for(
                cell,
                self.grid.layer_for_destination(goal),
                0,
                true,
                surfaces,
                is_crusher,
                self.seeker_player,
                crusher_level,
            );
            if !dest_ok {
                goal = self.snap_closest_goal_position(goal, surfaces, is_crusher, unit_radius);
            }
        }
        let start_grid = self
            .grid
            .cell_for_unit_position(start, self.seeker_center_in_cell);
        let goal_grid = self
            .grid
            .cell_for_unit_position(goal, self.seeker_center_in_cell);
        let start_layer = self
            .seeker_id
            .and_then(|id| objects.get(&id))
            .map(|o| self.grid.layer_for_destination(o.get_position()))
            .unwrap_or_else(|| self.grid.layer_for_destination(start));
        let dest_layer = self.grid.layer_for_destination(goal);

        // C++ getAircraftPath: circleClips only when appearance == LOCO_WINGS.
        // segmentIntersectsTallBuilding walk runs for every aircraft (hover/thrust).
        let mut path = if aircraft {
            let check_clips = self.seeker_wings;
            let avoid = Self::building_to_not_path_around(objects, self.seeker_id);
            let goal_adj = if check_clips {
                Self::circle_clips_tall_building(start, goal, 100.0, objects, avoid).unwrap_or(goal)
            } else {
                goal
            };
            // C++ getAircraftPath: first node is unit XY at dest Z (host Y).
            let mut start_at_dest = start;
            start_at_dest.y = goal.y;
            let direct = vec![start_at_dest, goal_adj];
            Self::detour_path_around_tall_buildings_ignoring(&direct, objects, avoid)
        } else {
            // C++ never computeQuickPaths across an ignored CELL_OBSTACLE
            // footprint: the direct branch bypasses A* ignore_cells entirely.
            // C++ AIUpdate.cpp:1665-1671: when both the destination AND the
            // start are off the pathfind extent, pathfinding is impossible —
            // computeQuickPath builds the direct two-node path instead of
            // consulting A* (which cannot seed an off-map cell).
            if self.leftover_should_force_direct_path_for_off_map_start(start, goal)
                && self.ignored_obstacle_cells().is_none()
            {
                Self::leftover_compute_quick_path_nodes(start, goal)
            } else {
                match self.find_path_via_crate(
                    start_grid,
                    goal_grid,
                    surfaces,
                    is_crusher,
                    start_layer,
                    dest_layer,
                ) {
                    Some(p) => p,
                    None => {
                        // C++ doPathfind / dozer packed goal: adjustToPossibleDestination
                        // then retry computePath (weaker than checkForAdjust).
                        let unit_radius = self
                            .seeker_id
                            .and_then(|id| objects.get(&id))
                            .map(|o| o.selection_radius)
                            .unwrap_or(0.0);
                        let mut possible = goal;
                        if !self.adjust_to_possible_destination(
                            start,
                            &mut possible,
                            surfaces,
                            is_crusher,
                            unit_radius,
                        ) {
                            return None;
                        }
                        let retry_grid = self
                            .grid
                            .cell_for_unit_position(possible, self.seeker_center_in_cell);
                        let retry_layer = self.grid.layer_for_destination(possible);
                        self.find_path_via_crate(
                            start_grid,
                            retry_grid,
                            surfaces,
                            is_crusher,
                            start_layer,
                            retry_layer,
                        )?
                    }
                }
            }
        };
        // Ground: keep crate/grid terrain-layer Y (hq-gd0jd). Do not lerp start→goal.
        // Aircraft: first/last stay at dest altitude; detours keep radial-offset Y
        // (hq-zqfpa). Do not flatten every node to start.y then overwrite first.
        if !aircraft {
            if let Some(first) = path.first_mut() {
                *first = start;
            }
        }
        // C++ checkDestination: snapped dest is the final position. Do not
        // restore the raw click (that walks units into building footprints).
        Some(path)
    }

    /// C++ `Pathfinder::clientSafeQuickDoesPathExist` (structure-aware, ground).
    pub fn client_safe_quick_does_path_exist(&self, from: Vec3, to: Vec3) -> bool {
        self.client_safe_quick_does_path_exist_for(from, to, SURFACE_GROUND)
    }

    /// C++ `clientSafeQuickDoesPathExist` with locomotor surfaces.
    pub fn client_safe_quick_does_path_exist_for(
        &self,
        from: Vec3,
        to: Vec3,
        surfaces: u32,
    ) -> bool {
        self.client_safe_quick_does_path_exist_for_crusher(from, to, surfaces, false)
    }

    /// C++ `clientSafeQuickDoesPathExist` with locomotor surfaces + crusher combiners.
    pub fn client_safe_quick_does_path_exist_for_crusher(
        &self,
        from: Vec3,
        to: Vec3,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        if self.grid.path_zones.iter().all(|&z| z == 0) {
            return self.grid.quick_path_exists_for_ui(from, to);
        }
        self.grid
            .quick_path_exists_for_crusher(from, to, surfaces, is_crusher)
    }

    /// C++ `Pathfinder::findBrokenBridge` on the live host grid.
    pub fn find_broken_bridge(&self, from: Vec3, to: Vec3) -> Option<ObjectId> {
        self.grid.find_broken_bridge(from, to)
    }

    /// C++ `Pathfinder::adjustToPossibleDestination` (AIPathfind.cpp:5510-5617).
    pub fn adjust_to_possible_destination(
        &self,
        start: Vec3,
        dest: &mut Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        self.grid.adjust_to_possible_destination_ex(
            start,
            dest,
            surfaces,
            is_crusher,
            unit_radius,
            self.seeker_player,
            crusher_level,
        )
    }

    /// C++ `adjustToPossibleDestination(obj, locomotorSet, dest)`.
    pub fn adjust_to_possible_destination_for(&self, obj: &Object, dest: &mut Vec3) -> bool {
        let surfaces = if obj.locomotor_surfaces != 0 {
            obj.locomotor_surfaces
        } else {
            SURFACE_GROUND
        };
        self.adjust_to_possible_destination(
            obj.get_position(),
            dest,
            surfaces,
            obj.crusher_level > 0,
            obj.selection_radius,
        )
    }

    /// C++ `Pathfinder::checkForPossible`.
    pub fn check_for_possible(
        &self,
        is_crusher: bool,
        from_zone: u16,
        center: bool,
        surfaces: u32,
        cell: GridPos,
        layer: PathfindLayerEnum,
        dest: &mut Vec3,
        starting_in_obstacle: bool,
    ) -> bool {
        self.grid.check_for_possible(
            is_crusher,
            from_zone,
            center,
            surfaces,
            cell,
            layer,
            dest,
            starting_in_obstacle,
        )
    }

    /// C++ `Pathfinder::patchPath` (AIPathfind.cpp:10344-10520).
    pub fn patch_path(
        &mut self,
        from: Vec3,
        original: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
        objects: &HashMap<ObjectId, Object>,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        if self.ignore_obstacle_id.is_some() {
            self.grid
                .update_dynamic_obstacles_ignoring(objects, self.ignore_obstacle_id);
        } else {
            self.grid.update_dynamic_obstacles(objects);
        }
        self.bind_seeker_from_mover(objects, mover);
        if original.len() < 2 {
            return None;
        }
        let start = self
            .grid
            .cell_for_unit_position(from, self.seeker_center_in_cell);
        let blocked = true;
        // C++ patchPath uses getRadiusAndCenter(obj) directly (AIPathfind.cpp
        // :10380-10383): infantry get radius 1, not pathDiameter/2 (=0).
        let radius = self
            .seeker_id
            .and_then(|id| objects.get(&id))
            .map(|o| PathfindingGrid::radius_and_center(o.selection_radius, self.grid.grid_size()).0)
            .unwrap_or_else(|| (self.seeker_path_diameter.max(1) / 2).max(0));
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let xz_dist_sq = |a: Vec3, b: Vec3| {
            let dx = a.x - b.x;
            let dz = a.z - b.z;
            dx * dx + dz * dz
        };
        // Walk original path reverse; stop at first blocked / occupied node.
        let mut start_node_idx = 0usize;
        let mut goal_pos = *original.last().unwrap();
        let mut goal_delta = xz_dist_sq(goal_pos, from);
        for idx in (1..original.len()).rev() {
            let cell = self.grid.world_to_grid(original[idx]);
            let layer = self.grid.layer_for_destination(original[idx]);
            if !self
                .grid
                .cell_passable_for_layer(cell, layer, surfaces, is_crusher)
            {
                start_node_idx = idx;
                break;
            }
            let dx = cell.x - start.x;
            let dy = cell.y - start.y;
            let consider_transient = blocked && dx >= -2 && dx <= 2 && dy >= -2 && dy <= 2;
            if self.grid.patch_cell_occupied(
                cell,
                layer,
                consider_transient,
                self.seeker_player,
                self.seeker_is_infantry,
                crusher_level,
                radius,
                self.seeker_center_in_cell,
            ) {
                start_node_idx = idx;
                break;
            }
            let cur = xz_dist_sq(original[idx], from);
            if cur < goal_delta {
                goal_pos = original[idx];
                goal_delta = cur;
            }
            start_node_idx = idx;
        }
        // C++: startNode == last → no open suffix to splice onto.
        if start_node_idx + 1 >= original.len() {
            return None;
        }

        let start_layer = self.grid.layer_for_destination(from);
        let mut try_goal = |goal: Vec3| -> Option<Vec<Vec3>> {
            let goal_cell = self.grid.world_to_grid(goal);
            let dest_layer = self.grid.layer_for_destination(goal);
            self.find_path_via_crate(
                start,
                goal_cell,
                surfaces,
                is_crusher,
                start_layer,
                dest_layer,
            )
        };
        let mut prefix = try_goal(goal_pos);
        if prefix.is_none() {
            for idx in ((start_node_idx + 1)..original.len()).rev() {
                if let Some(trial) = try_goal(original[idx]) {
                    prefix = Some(trial);
                    goal_pos = original[idx];
                    break;
                }
            }
        }
        let mut prefix = prefix?;

        let end = prefix.last().copied().unwrap_or(goal_pos);
        let mut match_idx = None;
        for idx in ((start_node_idx + 1)..original.len()).rev() {
            let p = original[idx];
            if (p.x - end.x).abs() < 0.5 && (p.z - end.z).abs() < 0.5 {
                match_idx = Some(idx);
                break;
            }
            let a = self.grid.world_to_grid(p);
            let b = self.grid.world_to_grid(end);
            if a.x == b.x && a.y == b.y {
                match_idx = Some(idx);
                break;
            }
        }
        let match_idx = match_idx.unwrap_or(original.len() - 1);
        if let Some(last) = prefix.last() {
            let m = original[match_idx];
            if (last.x - m.x).abs() < 0.5 && (last.z - m.z).abs() < 0.5 {
                prefix.pop();
            }
        }
        prefix.extend_from_slice(&original[match_idx..]);
        let optimized = self.grid.optimize_ground_path_ex(
            &prefix,
            surfaces,
            is_crusher,
            self.seeker_player,
            crusher_level,
        );
        if optimized.is_empty() {
            None
        } else {
            Some(optimized)
        }
    }

    /// C++ `Pathfinder::getMoveAwayFromPath` (AIPathfind.cpp:10171-10338).
    /// A* to a cell whose clearance box does not overlap avoided path segments.
    pub fn get_move_away_from_path(
        &mut self,
        from: Vec3,
        path_to_avoid: &[Vec3],
        path_to_avoid2: Option<&[Vec3]>,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
        other_radius: f32,
        seeker_player: Option<u32>,
        crusher_level: u8,
        tunnel: bool,
    ) -> Option<Vec<Vec3>> {
        if path_to_avoid.len() < 2 {
            return None;
        }
        let (radius, center_in_cell) =
            PathfindingGrid::radius_and_center(unit_radius, self.grid.grid_size());
        let (other_r, other_center) =
            PathfindingGrid::radius_and_center(other_radius, self.grid.grid_size());
        let start = self.grid.cell_for_unit_position(from, center_in_cell);
        if !self.grid.is_valid_pos(start) {
            return None;
        }
        let cell = self.grid.grid_size();
        let mut box_half = radius as f32 * cell - cell / 4.0;
        if center_in_cell {
            box_half += cell / 2.0;
        }
        box_half += other_r as f32 * cell;
        if other_center {
            box_half += cell / 2.0;
        }
        let layer = self.grid.layer_for_destination(from);
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<GridPos, i32> = HashMap::new();
        let mut closed: HashSet<GridPos> = HashSet::new();
        open.push(std::cmp::Reverse((0, start.x, start.y)));
        g_score.insert(start, 0);
        let mut found: Option<GridPos> = None;
        let mut expanded = 0i32;
        const MAX_EXPAND: i32 = 2500;
        let overlaps = |cell_pos: GridPos| -> bool {
            let mut center = self.grid.grid_to_world(cell_pos);
            if center_in_cell {
                center.x += cell * 0.5;
                center.z += cell * 0.5;
            }
            let region = CellXz {
                lo_x: center.x - box_half,
                lo_z: center.z - box_half,
                hi_x: center.x + box_half,
                hi_z: center.z + box_half,
            };
            let check = |path: &[Vec3]| {
                path.windows(2)
                    .any(|w| line_in_region_xz(w[0].x, w[0].z, w[1].x, w[1].z, &region))
            };
            check(path_to_avoid) || path_to_avoid2.is_some_and(check)
        };
        while let Some(std::cmp::Reverse((g, cx, cy))) = open.pop() {
            let cell_pos = GridPos::new(cx, cy);
            if !closed.insert(cell_pos) {
                continue;
            }
            expanded += 1;
            if expanded > MAX_EXPAND {
                break;
            }
            let overlap = cell_pos == start || overlaps(cell_pos);
            if !overlap
                && self.grid.check_destination_for(
                    cell_pos,
                    layer,
                    radius,
                    center_in_cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                )
            {
                found = Some(cell_pos);
                break;
            }
            for n in cell_pos.neighbors() {
                if !self.grid.is_valid_pos(n) || closed.contains(&n) {
                    continue;
                }
                if !tunnel
                    && !self
                        .grid
                        .cell_passable_for_layer(n, layer, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(n) && is_crusher)
                {
                    continue;
                }
                let step = if (n.x - cx).abs() + (n.y - cy).abs() == 2 {
                    14
                } else {
                    10
                };
                let ng = g + step;
                if g_score.get(&n).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(n, ng);
                open.push(std::cmp::Reverse((ng, n.x, n.y)));
            }
        }
        let dest = found?;
        if dest == start {
            return None;
        }
        let world_dest = {
            let mut w = self.grid.grid_to_world(dest);
            if center_in_cell {
                w.x += cell * 0.5;
                w.z += cell * 0.5;
            }
            w.y = from.y;
            w
        };
        if let Some(path) =
            self.find_path_via_crate(start, dest, surfaces, is_crusher, layer, layer)
        {
            if path.len() >= 2 {
                return Some(path);
            }
        }
        Some(vec![from, world_dest])
    }

    /// C++ `Pathfinder::findSafePath` Dijkstra flee (AIPathfind.cpp:10885+).
    /// Single-repulsor wrapper — both radii share `repulsor`.
    pub fn find_safe_path(
        &mut self,
        from: Vec3,
        repulsor: Vec3,
        repulsor_radius: f32,
        surfaces: u32,
        is_crusher: bool,
    ) -> Option<Vec<Vec3>> {
        self.find_safe_path_from(
            from,
            repulsor,
            repulsor,
            repulsor_radius,
            surfaces,
            is_crusher,
            true,
        )
    }

    /// C++ `Pathfinder::findSafePath` with two repulsors, human extent,
    /// destination_cell_ok, then `findPath` to the flee cell.
    pub fn find_safe_path_from(
        &mut self,
        from: Vec3,
        repulsor1: Vec3,
        repulsor2: Vec3,
        repulsor_radius: f32,
        surfaces: u32,
        is_crusher: bool,
        is_human: bool,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        let start = self
            .grid
            .cell_for_unit_position(from, self.seeker_center_in_cell);
        if !self.grid.is_valid_pos(start) {
            return None;
        }
        let radius_sqr = repulsor_radius * repulsor_radius;
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let seeker_player = self.seeker_player;
        let start_layer = self.grid.layer_for_destination(from);
        let start_lid = start_layer as u8;
        const MAX_CELLS: i32 = 2000;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let deltas: [(i32, i32); 8] = [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ];
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32, u8)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32, u8), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32, u8)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y, start_lid)));
        g_score.insert((start.x, start.y, start_lid), 0);
        let mut farthest: Option<(GridPos, PathfindLayerEnum, f32)> = None;
        let mut found: Option<(GridPos, PathfindLayerEnum)> = None;
        let mut cell_count = 0i32;
        while let Some(std::cmp::Reverse((_f, g, cx, cy, lid))) = open.pop() {
            let key = (cx, cy, lid);
            if !closed.insert(key) {
                continue;
            }
            let cell = GridPos::new(cx, cy);
            let layer = PathfindLayerEnum::from_u32(lid as u32);
            // C++ findSafePath measures from adjustCoordToCell (cell center
            // when centerInCell); grid_to_world returns the cell CORNER.
            let world = {
                let mut w = self.grid.grid_to_world_on_layer(cell, layer);
                if self.seeker_center_in_cell {
                    w.x += self.grid.grid_size() * 0.5;
                    w.z += self.grid.grid_size() * 0.5;
                }
                w
            };
            let d1 = {
                let dx = world.x - repulsor1.x;
                let dz = world.z - repulsor1.z;
                dx * dx + dz * dz
            };
            let d2 = {
                let dx = world.x - repulsor2.x;
                let dz = world.z - repulsor2.z;
                dx * dx + dz * dz
            };
            let nearest = d1.min(d2);
            let mut ok = nearest > radius_sqr;
            if open.is_empty() && cell_count > 0 {
                // Faithful C++ exhaustion clause (AIPathfind.cpp:10945-47),
                // but the picked flee cell must still clear both repulsors —
                // C++ findSafePath only terminates on a checkDestination-pass
                // outside the radii, never inside one.
                ok = nearest > radius_sqr;
            }
            if farthest.map(|(_, _, d)| nearest > d).unwrap_or(true) {
                farthest = Some((cell, layer, nearest));
                if cell_count > MAX_CELLS {
                    ok = true;
                }
            }
            if ok
                && self.grid.destination_cell_ok(
                    cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                )
                && self.grid.human_extent_allows(cell, is_human)
            {
                found = Some((cell, layer));
                break;
            }
            if self
                .grid
                .enqueue_connect_layer(cell, layer, g, g, &closed, &mut g_score, &mut open)
            {
                cell_count += 1;
            }
            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.grid.is_valid_pos(nc) || closed.contains(&(nx, ny, lid)) {
                    continue;
                }
                if !self.grid.human_extent_allows(nc, is_human) {
                    continue;
                }
                if PathfindingGrid::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self
                    .grid
                    .cell_passable_for_layer(nc, layer, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let nkey = (nx, ny, lid);
                if g_score.get(&nkey).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(nkey, ng);
                open.push(std::cmp::Reverse((ng, ng, nx, ny, lid)));
                cell_count += 1;
            }
        }
        let (dest, dest_layer) = if let Some((cell, layer)) = found {
            (cell, layer)
        } else if let Some((cell, layer, d)) = farthest {
            // C++ findSafePath only ever accepts a checkDestination-pass cell
            // OUTSIDE the repulsor radii; an exhausted-search fallback must
            // never park the unit closer than the radius either.
            if !self.grid.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            ) || d <= radius_sqr
            {
                return None;
            }
            (cell, layer)
        } else {
            return None;
        };
        if dest == start {
            return None;
        }
        // C++ buildActualPath: the flee path is rebuilt through A* to the
        // validated safe cell. No straight-line fail-open exists in C++
        // (AIUpdate findSafePath) — a direct from→goal segment would march
        // back inside both repulsor radii.
        let path = self.find_path_via_crate(
            start,
            dest,
            surfaces,
            is_crusher,
            start_layer,
            dest_layer,
        )?;
        if path.len() < 2 {
            return None;
        }
        Some(path)
    }

    /// C++ `Pathfinder::findClosestPath` (AIPathfind.cpp:8739+).
    /// Goal cell need not be passable. Tracks closest *valid destination*
    /// by screen-dist + cost², then paths to that cell.
    pub fn find_closest_path(
        &mut self,
        from: Vec3,
        goal: Vec3,
        surfaces: u32,
        is_crusher: bool,
        is_human: bool,
    ) -> Option<Vec<Vec3>> {
        self.sync_crate_astar();
        self.grid.query_seeker_id = self.seeker_id.map(|id| id.0).unwrap_or(0);
        let start = self.grid.cell_for_unit_position(from, self.seeker_center_in_cell);
        let goal_grid = self.grid.cell_for_unit_position(goal, self.seeker_center_in_cell);
        if !self.grid.is_valid_pos(start) || !self.grid.is_valid_pos(goal_grid) {
            return None;
        }
        if is_human && !self.grid.human_extent_allows(start, true) {
            return None;
        }
        let crusher_level = if is_crusher { self.seeker_crusher_level.max(1) } else { 0 };
        let seeker_player = self.seeker_player;
        let start_layer = self.grid.layer_for_destination(from);
        let start_lid = start_layer as u8;
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32, u8)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32, u8), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32, u8)> = HashSet::new();
        // Leftover goal accept honors canPathThroughUnits (per-pop C++ check).
        let goal_accept_ok = self.seeker_can_path_through_units
            || self.grid.destination_cell_ok(
                goal_grid,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                start_layer,
            );
        let mut closest_cell: Option<(GridPos, PathfindLayerEnum, f32)> = None;
        let mut closest_screen_sqr = f32::MAX;
        let mut found_goal_cell = false;
        let mut expanded = 0i32;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        const COST_TO_DISTANCE_FACTOR_SQR: f32 = 0.01;
        const MAX_EXPAND: i32 = 4000;
        let heuristic = |c: GridPos| -> i32 {
            let dx = (goal_grid.x - c.x).abs();
            let dy = (goal_grid.y - c.y).abs();
            let dmin = dx.min(dy);
            let dmax = dx.max(dy);
            COST_DIAG * dmin + COST_ORTHO * (dmax - dmin)
        };
        let deltas: [(i32, i32); 8] = [
            (1, 0),
            (0, 1),
            (-1, 0),
            (0, -1),
            (1, 1),
            (-1, 1),
            (-1, -1),
            (1, -1),
        ];
        let h0 = heuristic(start);
        open.push(std::cmp::Reverse((h0, 0, start.x, start.y, start_lid)));
        g_score.insert((start.x, start.y, start_lid), 0);
        let closest_jumps = self.hierarchical_bridge_jumps();
        let closest_start = self.host_to_crate_coord(start);
        let closest_goal = self.host_to_crate_coord(goal_grid);
        if let Some(crate_pf) = self.crate_astar.as_mut() {
            crate_pf.finder.apply_hierarchical_zone_prune(
                closest_start,
                closest_goal,
                surfaces,
                is_crusher,
                &closest_jumps,
            );
        }
        while let Some(std::cmp::Reverse((_f, g, cx, cy, lid))) = open.pop() {
            let key = (cx, cy, lid);
            if !closed.insert(key) {
                continue;
            }
            expanded += 1;
            if expanded > MAX_EXPAND {
                break;
            }
            let cell = GridPos::new(cx, cy);
            let layer = PathfindLayerEnum::from_u32(lid as u32);
            if cx == goal_grid.x && cy == goal_grid.y {
                let goal_ok = goal_accept_ok;
                if goal_ok || closest_cell.is_none() {
                    found_goal_cell = true;
                    closest_cell = Some((cell, layer, 0.0));
                    break;
                } else {
                    found_goal_cell = true;
                }
            } else if self.grid.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            ) {
                let dx = (goal_grid.x - cx).abs() as f32;
                let dy = (goal_grid.y - cy).abs() as f32;
                let dist_screen = dx * dx + dy * dy;
                if dist_screen < closest_screen_sqr {
                    closest_screen_sqr = dist_screen;
                }
                // pathCostMultiplier 0.2 per C++ callers (AIUpdate.cpp:410).
                let cost_term = (g as f32) * (g as f32) * COST_TO_DISTANCE_FACTOR_SQR * 0.2;
                let dist_sqr = dist_screen + cost_term;
                let better = match closest_cell {
                    None => true,
                    Some((_, _, best)) => dist_sqr < best,
                };
                if better {
                    closest_cell = Some((cell, layer, dist_sqr));
                }
            }
            let f_hop = g + heuristic(cell);
            self.grid.enqueue_connect_layer(
                cell,
                layer,
                g,
                f_hop,
                &closed,
                &mut g_score,
                &mut open,
            );
            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.grid.is_valid_pos(nc) || closed.contains(&(nx, ny, lid)) {
                    continue;
                }
                if !self.grid.human_extent_allows(nc, is_human) {
                    continue;
                }
                if PathfindingGrid::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self
                    .grid
                    .cell_passable_for_layer(nc, layer, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let nkey = (nx, ny, lid);
                if g_score.get(&nkey).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(nkey, ng);
                let f = ng + heuristic(nc);
                open.push(std::cmp::Reverse((f, ng, nx, ny, lid)));
            }
        }
        let (best_cell, best_layer, _) = closest_cell?;
        let to_pos = if found_goal_cell && best_cell == goal_grid {
            goal
        } else {
            self.grid.grid_to_world_on_layer(best_cell, best_layer)
        };
        let to_cell = self
            .grid
            .cell_for_unit_position(to_pos, self.seeker_center_in_cell);
        // Leftover buildActualPath via find_path_via_crate; failure stays
        // None — no fail-open line.
        if let Some(path) = self.find_path_via_crate(
            start,
            to_cell,
            surfaces,
            is_crusher,
            start_layer,
            best_layer,
        ) {
            let mut world = path;
            if let Some(first) = world.first_mut() {
                *first = from;
            }
            if let Some(last) = world.last_mut() {
                *last = to_pos;
            }
            return Some(self.grid.optimize_ground_path(&world, surfaces, is_crusher));
        }
        // Leftover returns none when internalFindPath fails — no fail-open line.
        if start == to_cell {
            Some(vec![from])
        } else {
            None
        }
    }

    /// C++ `adjustDestination(..., groupDest)` for ground group members.
    pub fn adjust_group_destination(
        &mut self,
        from: Vec3,
        dest: Vec3,
        group_dest: Vec3,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        seeker_id: u32,
    ) -> Vec3 {
        let dest_cell = self.grid.world_to_grid(dest);
        let group_cell = self.grid.world_to_grid(group_dest);
        let layer = self.grid.layer_for_destination(group_dest);
        self.grid.query_from = Some(self.grid.world_to_grid(from));
        self.grid.query_orig_dest = Some(dest_cell);
        self.grid.query_seeker_id = seeker_id;
        let adj = self.grid.adjust_destination_for_group(
            dest_cell,
            group_cell,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        );
        self.grid.query_from = None;
        self.grid.query_orig_dest = None;
        self.grid.query_seeker_id = 0;
        match adj {
            Some(cell) => {
                let mut w = self.grid.grid_to_world(cell);
                w.y = dest.y;
                w
            }
            None => dest,
        }
    }

    /// C++ `Path::computePointOnPath` on a host XZ polyline.
    pub fn compute_point_on_path(pos: Vec3, waypoints: &[Vec3]) -> Vec3 {
        Self::compute_point_on_path_ex(pos, waypoints, None, SURFACE_GROUND, false)
    }

    pub fn compute_point_on_path_ex(
        pos: Vec3,
        waypoints: &[Vec3],
        grid: Option<&PathfindingGrid>,
        surfaces: u32,
        is_crusher: bool,
    ) -> Vec3 {
        Self::compute_point_on_path_for(
            pos,
            waypoints,
            grid,
            surfaces,
            is_crusher,
            None,
            if is_crusher { 1 } else { 0 },
        )
    }

    /// C++ `Path::computePointOnPath` (AIPathfind.cpp:732-1014).
    /// Always tries `isLinePassable` to the next node; k = offset/(3*CELL)
    /// only chooses the remaining-segment midpoint when the lead fails.
    /// A blocked lead stays on the closest point so we do not aim through
    /// buildings (hq-2f8q0 / hq-5g9m3).
    pub fn compute_point_on_path_for(
        pos: Vec3,
        waypoints: &[Vec3],
        grid: Option<&PathfindingGrid>,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> Vec3 {
        if waypoints.is_empty() {
            return pos;
        }
        if waypoints.len() == 1 {
            return waypoints[0];
        }
        let mut best_d2 = f32::MAX;
        let mut best = waypoints[0];
        let mut best_seg = 0usize;
        let mut best_t = 0.0f32;
        for i in 0..waypoints.len() - 1 {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let sx = b.x - a.x;
            let sz = b.z - a.z;
            let len_sqr = sx * sx + sz * sz;
            let t = if len_sqr <= 1.0e-8 {
                0.0
            } else {
                let tx = pos.x - a.x;
                let tz = pos.z - a.z;
                ((tx * sx + tz * sz) / len_sqr).clamp(0.0, 1.0)
            };
            let px = a.x + sx * t;
            let pz = a.z + sz * t;
            let dx = pos.x - px;
            let dz = pos.z - pz;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best = Vec3::new(px, a.y, pz);
                best_seg = i;
                best_t = t;
            }
        }
        let cell = grid.map(|g| g.grid_size()).unwrap_or(10.0);
        let max_err = 3.0 * cell;
        let k = (best_d2.sqrt() / max_err).clamp(0.0, 1.0);
        let next = waypoints[best_seg + 1];
        let a = waypoints[best_seg];
        let sx = next.x - a.x;
        let sz = next.z - a.z;
        let seg_len = (sx * sx + sz * sz).sqrt();
        let along = best_t * seg_len;
        let line_ok = |to: Vec3| match grid {
            Some(g) => {
                let from = g.world_to_grid(pos);
                let dest = g.world_to_grid(to);
                g.line_passable_ex(
                    from,
                    dest,
                    surfaces,
                    is_crusher,
                    true,
                    seeker_player,
                    crusher_level,
                    false,
                )
            }
            None => true,
        };
        // C++ AIPathfind.cpp:910-950 — always try next node, then tryAhead.
        if line_ok(next) {
            let remaining = seg_len - along;
            let try_ahead = best_t > 0.5 || remaining < 1.0;
            if try_ahead {
                if let Some(ahead) = waypoints.get(best_seg + 2) {
                    let mid = Vec3::new((next.x + ahead.x) * 0.5, next.y, (next.z + ahead.z) * 0.5);
                    if remaining < 1.0 || line_ok(mid) {
                        return mid;
                    }
                }
            }
            return next;
        }
        // C++ :952-966 — k>0.5 tries the remaining-segment midpoint.
        if k > 0.5 && seg_len > 1.0e-6 {
            let try_dist = along + 0.5 * (seg_len - along);
            let t = (try_dist / seg_len).clamp(0.0, 1.0);
            let mid = Vec3::new(a.x + sx * t, a.y, a.z + sz * t);
            if line_ok(mid) {
                return mid;
            }
        }
        best
    }

    /// C++ `Path::computePointOnPath` `distAlongPath` (AIPathfind.cpp:997).
    /// Remaining polyline from the closest point on the path to the end.
    /// Lead-point raise (`:999-1007`) is applied by the locomotor dispatcher.
    pub fn dist_along_path(pos: Vec3, waypoints: &[Vec3]) -> f32 {
        if waypoints.is_empty() {
            return 0.0;
        }
        if waypoints.len() == 1 {
            let dx = pos.x - waypoints[0].x;
            let dz = pos.z - waypoints[0].z;
            return (dx * dx + dz * dz).sqrt();
        }
        let mut best_d2 = f32::MAX;
        let mut best_seg = 0usize;
        let mut best_t = 0.0f32;
        for i in 0..waypoints.len() - 1 {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let sx = b.x - a.x;
            let sz = b.z - a.z;
            let len_sqr = sx * sx + sz * sz;
            let t = if len_sqr <= 1.0e-8 {
                0.0
            } else {
                let tx = pos.x - a.x;
                let tz = pos.z - a.z;
                ((tx * sx + tz * sz) / len_sqr).clamp(0.0, 1.0)
            };
            let px = a.x + sx * t;
            let pz = a.z + sz * t;
            let dx = pos.x - px;
            let dz = pos.z - pz;
            let d2 = dx * dx + dz * dz;
            if d2 < best_d2 {
                best_d2 = d2;
                best_seg = i;
                best_t = t;
            }
        }
        let mut remaining = 0.0f32;
        for i in best_seg..waypoints.len() - 1 {
            let a = waypoints[i];
            let b = waypoints[i + 1];
            let seg = ((b.x - a.x) * (b.x - a.x) + (b.z - a.z) * (b.z - a.z)).sqrt();
            if i == best_seg {
                remaining += (1.0 - best_t) * seg;
            } else {
                remaining += seg;
            }
        }
        remaining
    }

    /// C++ `Path::computeFlightDistToGoal` (AIPathfind.cpp:1022-1074).
    /// Remaining 2D projection along subsequent path segments — not
    /// closest-point winding. Hover/air use this so a dogleg shortcut
    /// does not snap remaining onto a later segment and brake early.
    pub fn compute_flight_dist_to_goal(pos: Vec3, waypoints: &[Vec3]) -> f32 {
        if waypoints.is_empty() {
            return 0.0;
        }
        if waypoints.len() == 1 {
            let dx = waypoints[0].x - pos.x;
            let dz = waypoints[0].z - pos.z;
            return (dx * dx + dz * dz).sqrt();
        }
        let mut distance = 0.0f32;
        for i in 0..waypoints.len() - 1 {
            let start = waypoints[i];
            let end = waypoints[i + 1];
            let pvx = end.x - start.x;
            let pvz = end.z - start.z;
            let plen = (pvx * pvx + pvz * pvz).sqrt();
            if plen <= 1.0e-8 {
                continue;
            }
            let nx = pvx / plen;
            let nz = pvz / plen;
            let dot = (end.x - pos.x) * nx + (end.z - pos.z) * nz;
            if dot >= 0.0 {
                distance += dot;
            }
        }
        distance
    }

    pub(super) fn is_allied_to(&self, mover: &Object, other: &Object) -> bool {
        let mover_p = mover.owner_player_id.unwrap_or(mover.team as u32);
        let other_p = other.owner_player_id.unwrap_or(other.team as u32);
        if mover_p == other_p {
            return true;
        }
        let bit = 1u16 << other_p.min(15);
        (self.grid.ally_mask_for(mover_p) & bit) != 0
    }

    /// C++ `Pathfinder::moveAllies` — idle allies occupying the new path scoot.
    pub fn allies_to_nudge_off_path(
        &self,
        mover_id: ObjectId,
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<ObjectId> {
        let Some(mover) = objects.get(&mover_id) else {
            return Vec::new();
        };
        if path.len() < 2 {
            return Vec::new();
        }
        let is_dozer = mover.is_kind_of(KindOf::Dozer);
        let is_harvester = mover.is_kind_of(KindOf::Harvester);
        let mover_infantry = mover.is_kind_of(KindOf::Infantry);
        let (radius, center_in_cell) =
            PathfindingGrid::radius_and_center(mover.selection_radius, self.grid.grid_size());
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        let ignore = self.ignore_obstacle_id;
        let blocked_by_ally = path.iter().skip(1).any(|wp| {
            let cell = self.grid.world_to_grid(*wp);
            objects.values().any(|obj| {
                if obj.id == mover_id || !obj.is_alive() || ignore == Some(obj.id) {
                    return false;
                }
                if !self.is_allied_to(mover, obj) {
                    return false;
                }
                let oc = self.grid.world_to_grid(obj.get_position());
                oc.x >= cell.x - radius
                    && oc.x < cell.x + num_above
                    && oc.y >= cell.y - radius
                    && oc.y < cell.y + num_above
            })
        });
        if !is_dozer && !is_harvester && !blocked_by_ally {
            return Vec::new();
        }
        let mut nudged = Vec::new();
        for wp in path.iter().skip(1).rev() {
            let cell = self.grid.world_to_grid(*wp);
            for obj in objects.values() {
                if obj.id == mover_id || !obj.is_alive() {
                    continue;
                }
                if ignore == Some(obj.id) {
                    continue;
                }
                if !self.is_allied_to(mover, obj) {
                    continue;
                }
                if obj.is_kind_of(KindOf::Structure) || obj.is_kind_of(KindOf::Immobile) {
                    continue;
                }
                if mover_infantry && obj.is_kind_of(KindOf::Infantry) {
                    continue;
                }
                if mover_infantry && !obj.is_kind_of(KindOf::Infantry) && !blocked_by_ally {
                    continue;
                }
                if !obj.movement.path.is_empty()
                    || obj.movement.velocity.length_squared() > 0.25
                    || obj.status.attacking
                    || obj.status.using_ability
                    || obj.deploy_style.as_ref().is_some_and(|d| d.is_busy())
                {
                    continue;
                }
                let oc = self.grid.world_to_grid(obj.get_position());
                if oc.x >= cell.x - radius
                    && oc.x < cell.x + num_above
                    && oc.y >= cell.y - radius
                    && oc.y < cell.y + num_above
                    && !nudged.contains(&obj.id)
                {
                    nudged.push(obj.id);
                }
            }
        }
        nudged
    }

    /// Move unit along path
    pub fn move_unit_along_path(
        &self,
        object_id: ObjectId,
        objects: &mut HashMap<ObjectId, Object>,
        dt: f32,
    ) -> bool {
        if let Some(unit) = objects.get_mut(&object_id) {
            if unit.movement.path.is_empty()
                || unit.movement.current_path_index >= unit.movement.path.len()
            {
                unit.stop_moving();
                return false;
            }

            let target_waypoint = unit.movement.path[unit.movement.current_path_index];
            let current_pos = unit.get_position();
            let distance_to_waypoint = current_pos.distance(target_waypoint);

            if distance_to_waypoint < 5.0 {
                // Reached waypoint, move to next
                unit.movement.current_path_index += 1;
                if unit.movement.current_path_index >= unit.movement.path.len() {
                    // Reached final destination
                    unit.stop_moving();
                    return true;
                }
                return false; // Continue to next waypoint
            }

            // Move toward waypoint
            let direction = (target_waypoint - current_pos).normalize_or_zero();
            let move_distance = unit.movement.max_speed * dt;
            let new_position = current_pos + direction * move_distance;

            unit.set_position(new_position);
            unit.set_orientation((-direction.z).atan2(direction.x));

            false
        } else {
            false
        }
    }

    /// Set up flow field for group movement
    pub fn create_flow_field(
        &mut self,
        goal_object_id: ObjectId,
        goal_pos: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) {
        // Update obstacles and create flow field (once per logic frame).
        self.ensure_dynamic_obstacles(objects);

        let goal_grid = self.grid.world_to_grid(goal_pos);
        let mut flow_field = FlowField::new_with_origin(
            self.grid.origin(),
            self.grid.width as f32 * self.grid.grid_size,
            self.grid.height as f32 * self.grid.grid_size,
            self.grid.grid_size,
        );

        flow_field.generate_flow_field(goal_grid, &self.grid);
        self.flow_fields.insert(goal_object_id, flow_field);
    }

    /// Move group of units using flow field
    pub fn move_group_with_flow_field(
        &self,
        goal_object_id: ObjectId,
        unit_ids: &[ObjectId],
        objects: &mut HashMap<ObjectId, Object>,
        dt: f32,
    ) {
        if let Some(flow_field) = self.flow_fields.get(&goal_object_id) {
            // Calculate movements
            let movements: Vec<(ObjectId, Vec3, f32)> = unit_ids
                .iter()
                .filter_map(|&unit_id| {
                    if let Some(unit) = objects.get(&unit_id) {
                        let flow_direction = flow_field.get_flow_direction(unit.get_position());

                        if flow_direction.length() > 0.1 {
                            let move_distance = unit.movement.max_speed * dt;
                            let new_position = unit.get_position() + flow_direction * move_distance;
                            let new_orientation = (-flow_direction.z).atan2(flow_direction.x);

                            Some((unit_id, new_position, new_orientation))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect();

            // Apply movements directly

            // Apply movements
            for (unit_id, new_position, new_orientation) in movements {
                if let Some(unit) = objects.get_mut(&unit_id) {
                    unit.set_position(new_position);
                    unit.set_orientation(new_orientation);
                }
            }
        }
    }

    /// Clean up flow fields
    pub fn cleanup_flow_field(&mut self, goal_object_id: ObjectId) {
        self.flow_fields.remove(&goal_object_id);
    }

    /// Batch pathfinding for multiple units
    pub fn find_paths_batch(
        &mut self,
        path_requests: Vec<(ObjectId, Vec3, Vec3)>, // (unit_id, start, goal)
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<(ObjectId, Option<Vec<Vec3>>)> {
        self.ensure_dynamic_obstacles(objects);

        // Process all pathfinding requests sequentially
        let mut results = Vec::new();

        for (unit_id, start, goal) in path_requests {
            let start_grid = self.grid.world_to_grid(start);
            let goal_grid = self.grid.world_to_grid(goal);

            let path = self.grid.find_path(start_grid, goal_grid);
            results.push((unit_id, path));
        }

        results
    }
}
