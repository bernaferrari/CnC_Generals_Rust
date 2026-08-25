use super::*;

impl PathfindingSystem {
    /// C++ `Pathfinder::findPath` (AIPathfind.cpp:6364-6433).
    ///
    /// 1) clientSafeQuickDoesPathExist zone gate  
    /// 2) hierarchical path probe → clearPassableFlags; on failure setAllPassable  
    /// 3) internalFindPath A*
    pub fn find_path(&self, request: PathRequest) -> PathResult {
        // Check cache first
        let cache_key = (
            GridCoord::from_world(&request.from),
            GridCoord::from_world(&request.to),
            request.surfaces,
            request.is_crusher,
            request.allow_partial,
            request.unit_radius.to_bits(),
            request.move_allies,
            request.ignore_obstacle_id.unwrap_or(INVALID_ID),
            request.is_human,
        );

        if let Ok(cache) = self.path_cache.lock() {
            if let Some(cached) = cache.get(&cache_key) {
                return cached.clone();
            }
        }

        // C++ findPath: clientSafeQuickDoesPathExist first.
        if !self.client_safe_quick_does_path_exist(request.surfaces, &request.from, &request.to) {
            return PathResult::none();
        }

        // C++: clear_passable_flags; hierarchical probe; if no hPat → set_all_passable.
        // Probe marks A* zone-block passable (examineCellsCallback :6005).
        self.clear_zone_passable_flags();

        let start = GridCoord::from_world(&request.from);
        let goal = GridCoord::from_world(&request.to);
        let zone_join = {
            let connected = self
                .zones
                .lock()
                .map(|z| z.are_connected(start, goal, request.surfaces, request.is_crusher))
                .unwrap_or(true);
            connected
                || self.hierarchical_zones_join_via_bridge(
                    start,
                    goal,
                    request.surfaces,
                    request.is_crusher,
                )
        };
        let jumps: Vec<(GridCoord, GridCoord)> = self
            .bridges
            .iter()
            .filter(|b| !b.destroyed)
            .flat_map(|b| {
                let mut pairs = vec![(b.start_cell, b.end_cell)];
                if let (Some(&a), Some(&z)) = (
                    b.ground_connect_cells.first(),
                    b.ground_connect_cells.last(),
                ) {
                    if a != z {
                        pairs.push((a, z));
                    }
                }
                pairs
            })
            .collect();
        let hier_ok = self
            .pathfinder
            .lock()
            .map(|mut pf| {
                pf.apply_hierarchical_zone_prune(
                    start,
                    goal,
                    request.surfaces,
                    request.is_crusher,
                    &jumps,
                )
            })
            .unwrap_or(false);
        if !hier_ok {
            // C++ setAllPassable / leftover set_all_passable
            self.set_all_zone_passable();
        }

        let _ = zone_join;

        let result = self.find_path_internal(request);

        // Cache the result
        if let Ok(mut cache) = self.path_cache.lock() {
            cache.insert(cache_key, result.clone());

            // Limit cache size
            if cache.len() > 1000 {
                cache.clear();
            }
        }

        result
    }

    /// Internal path finding implementation
    /// Matches C++ Pathfinder::internalFindPath() at AIPathfind.cpp:6438-6694
    pub(crate) fn find_path_internal(&self, request: PathRequest) -> PathResult {
        let start = GridCoord::from_world(&request.from);
        let goal = GridCoord::from_world(&request.to);
        let ignore_cells = ignored_obstacle_cells(request.ignore_obstacle_id);

        // Validate coordinates
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal) {
            return PathResult::none();
        }
        // C++ human: reject cells outside m_logicalExtent.
        if request.is_human && (!self.in_logical_extent(start) || !self.in_logical_extent(goal)) {
            return PathResult::none();
        }

        // Check zone connectivity for fast rejection
        // Matches C++ zone check at AIPathfind.cpp:6531-6559
        if let Ok(zones) = self.zones.lock() {
            if !zones.are_connected(start, goal, request.surfaces, request.is_crusher) {
                return PathResult::none();
            }
        }

        // C++ internalFindPath tunneling: start in obstacle → ignore obstacles until clear.
        let start_is_obstacle = self
            .pathfinder
            .lock()
            .ok()
            .map(|pf| pf.get_cell_type(start) == Some(PathfindCellType::Obstacle))
            .unwrap_or(false);
        let mut tunneling = start_is_obstacle;
        if !tunneling {
            // Source invalid movement → cheat tunnel (C++ validMovementPosition source fail).
            if let Ok(pf) = self.pathfinder.lock() {
                if !pf.is_passable(start, request.surfaces, request.is_crusher) {
                    tunneling = true;
                }
            }
        }
        // Persist for callers that read is_tunneling during this path.
        // Note: PathfindingSystem is &self here — use interior via cell local only.
        let is_dozer = Self::object_is_dozer(request.object_id);
        let dozer_id = request.object_id;
        // Snapshot obstacle owners before A* so dozerHack does not re-lock pathfinder.
        let obstacle_owners: HashMap<(i32, i32), ObjectID> = if is_dozer {
            self.snapshot_cell_obstacle_ids()
        } else {
            HashMap::new()
        };

        // C++ examineNeighboringCells: allyFixedCount → +3*COST_DIAGONAL;
        // C++ examineNeighboringCells: allyFixedCount → +3*COST_DIAGONAL;
        // allyMoving && dx<10 && dy<10 → +3*COST_DIAGONAL.
        let (radius, center_in_cell) = Self::compute_radius_and_center(request.unit_radius);
        let obj_id = request.object_id;
        let start_cell = start;
        let ally_extra = |cell: GridCoord| -> u32 {
            if obj_id == INVALID_ID {
                return 0;
            }
            let mut info = CheckMovementInfo {
                cell,
                layer: PathfindLayerEnum::Ground,
                center_in_cell,
                radius,
                consider_transient: false,
                acceptable_surfaces: request.surfaces,
                ..Default::default()
            };
            if !self.check_for_movement(obj_id, &mut info) {
                return 0;
            }
            if info.ally_fixed_count > 0 {
                // C++ 3*COST_DIAGONAL regardless of canPathThroughUnits.
                return 3 * COST_DIAGONAL;
            }
            // C++: if (info.allyMoving && dx<10 && dy<10) newCost += 3*COST_DIAGONAL
            if info.ally_moving {
                let dx = (cell.x - start_cell.x).abs();
                let dy = (cell.y - start_cell.y).abs();
                if dx < 10 && dy < 10 {
                    return 3 * COST_DIAGONAL;
                }
            }
            0
        };

        // Downhill-only locomotors (C++ isDownhillOnly) — reject uphill A* steps.
        let downhill_only = Self::object_is_downhill_only(request.object_id);
        let ground_h = |cell: GridCoord| -> f32 {
            let wx = (cell.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            let wy = (cell.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            if let Some(terrain) = TheTerrainLogic::get() {
                terrain.get_layer_height(wx, wy, CommonPathfindLayerEnum::Ground)
            } else {
                0.0
            }
        };

        // Run A* pathfinding
        let pathfinder = self.pathfinder.lock().unwrap();
        // force_passable stays Some (never None) so find_path_ex6 goal-obstacle
        // early-out matches prior host behavior. DozerHack is dozer_obstacle_ok.
        let force_pass = |_cell: GridCoord| -> bool { false };
        // C++ examineNeighboringCells 6207-6226: KINDOF_DOZER + CELL_OBSTACLE +
        // findObjectByID && relationship != ENEMIES. Missing obstacle → false.
        let dozer_obstacle_ok = |coord: GridCoord| -> bool {
            if !is_dozer {
                return false;
            }
            let Some(&obs_id) = obstacle_owners.get(&(coord.x, coord.y)) else {
                return false;
            };
            Self::dozer_hack_allows_obstacle(dozer_id, obs_id)
        };
        // C++ examineCellsCallback: abort line on enemyFixed / allyFixedCount.
        let line_ok = |cell: GridCoord| -> bool {
            if obj_id == INVALID_ID {
                return true;
            }
            let mut info = CheckMovementInfo {
                cell,
                layer: PathfindLayerEnum::Ground,
                center_in_cell,
                radius,
                consider_transient: false,
                acceptable_surfaces: request.surfaces,
                ..Default::default()
            };
            if !self.check_for_movement(obj_id, &mut info) {
                return false;
            }
            if info.enemy_fixed || info.ally_fixed_count > 0 {
                return false;
            }
            // Seed line never walks occupying enemies, crushable included.
            // check_for_movement only sets enemy_fixed when !can_crush.
            let pos_u = self.pos_unit_at(cell, PathfindLayerEnum::Ground);
            if pos_u != INVALID_ID && pos_u != obj_id {
                let is_enemy = OBJECT_REGISTRY
                    .with_object(obj_id, |g| {
                        OBJECT_REGISTRY.with_object(pos_u, |u| {
                            g.relationship_to(&u) != crate::common::Relationship::Allies
                        })
                    })
                    .flatten()
                    .unwrap_or(false);
                if is_enemy {
                    return false;
                }
            }
            true
        };
        // Seed line when not tunneling and not downhill-only (C++ guards).
        let seed_line = !tunneling && !downhill_only;
        let is_human = request.is_human;
        let cell_allowed = |cell: GridCoord| -> bool {
            // C++ computer players may path outside logical map; humans may not.
            if is_human {
                return self.in_logical_extent(cell);
            }
            true
        };
        let dozer_ok_ref: Option<&dyn Fn(GridCoord) -> bool> = if is_dozer {
            Some(&dozer_obstacle_ok as &dyn Fn(GridCoord) -> bool)
        } else {
            None
        };
        let grid_path = pathfinder.find_path_ex6(
            start,
            goal,
            request.surfaces,
            request.is_crusher,
            MAX_PATH_ITERATIONS,
            request.allow_partial,
            ignore_cells.as_ref(),
            Some(&ally_extra as &dyn Fn(GridCoord) -> u32),
            downhill_only,
            Some(&ground_h as &dyn Fn(GridCoord) -> f32),
            Some(&force_pass as &dyn Fn(GridCoord) -> bool),
            Some(&line_ok as &dyn Fn(GridCoord) -> bool),
            seed_line,
            tunneling,
            Some(&cell_allowed as &dyn Fn(GridCoord) -> bool),
            dozer_ok_ref,
        );

        drop(pathfinder); // Release lock

        let Some((grid_path, cells_examined)) = grid_path else {
            return PathResult::none();
        };
        // C++ m_cumulativeCellsAllocated += cells examined this path.
        let _ = self
            .cumulative_cells_allocated
            .fetch_add(cells_examined as i32, Ordering::Relaxed);

        // Convert grid path via buildActualPath (centerInCell from unit radius).
        // Matches C++ buildActualPath() at AIPathfind.cpp:8954-9071
        let (_radius, center_in_cell) = Self::compute_radius_and_center(request.unit_radius);
        let built = self.build_actual_path_for_object(
            &grid_path,
            &request.from,
            &request.to,
            request.surfaces,
            request.is_crusher,
            false,
            center_in_cell,
            request.object_id,
        );
        if built.success {
            let mut result = built;
            result.total_cost = self.calculate_path_cost(&grid_path);
            // C++ path->optimize(obj, surfaces, blocked) after prependCells.
            let optimized = self.optimize_path_blocked(
                &result.waypoints,
                &result.layers,
                &request,
                result.blocked_by_ally,
            );
            let opt_len = optimized.0.len();
            return PathResult {
                success: true,
                waypoints: optimized.0,
                layers: optimized.1,
                can_optimize: vec![true; opt_len],
                total_cost: result.total_cost,
                blocked_by_ally: result.blocked_by_ally,
            };
        }
        // C++ internalFindPath returns NULL when buildActualPath fails. Do not
        // reconstruct a path from raw grid coordinates: that bypasses the
        // layer/terrain/occupancy validation performed by buildActualPath and
        // can turn an A* failure into a straight march through an invalid cell.
        PathResult::none()
    }

    /// Find closest reachable path (for blocked destinations)
    /// Matches C++ Pathfinder::findClosestPath() at AIPathfind.cpp:8739-8926
    /// C++ `Pathfinder::findClosestPath` (AIPathfind.cpp:8739+).
    ///
    /// Hierarchical passable dance, then A* from start tracking the closest
    /// valid destination cell to the goal (screen distance + cost factor).
    /// Exact goal success returns buildActualPath; else path to closest cell.
    pub fn find_closest_path(&self, mut request: PathRequest) -> PathResult {
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        // C++ COST_TO_DISTANCE_FACTOR = 1/10 → SQR = 1/100.
        const COST_TO_DISTANCE_FACTOR_SQR: f32 = 0.01;
        const MAX_EXPAND: i32 = 4000;

        if !self.is_map_ready {
            return PathResult::none();
        }

        let goal_grid = GridCoord::from_world(&request.to);
        let (radius, center_in_cell) = Self::compute_radius_and_center(request.unit_radius);
        let aircraft_goal_only = Self::object_uses_aircraft_goal_reservations(request.object_id);

        if aircraft_goal_only {
            let goal_layer = self.get_layer_for_coord(goal_grid);
            if self.check_destination(&request, goal_grid, goal_layer, radius, center_in_cell) {
                let adjusted = self.world_pos_for_coord(goal_grid, goal_layer);
                return Self::destination_only_result(request.from, adjusted, goal_layer);
            }
            // Aircraft without exact goal: fall through to closest-cell A*.
        }

        // C++ hierarchical passable flags (unless tunneling).
        let started_stuck = self.is_tunneling;
        if self.is_tunneling {
            if let Ok(mut zones) = self.zones.lock() {
                zones.set_all_passable();
            }
        } else {
            if let Ok(mut zones) = self.zones.lock() {
                zones.clear_passable_flags();
            }
            let start_c = GridCoord::from_world(&request.from);
            let hier_ok = self
                .zones
                .lock()
                .map(|z| z.are_connected(start_c, goal_grid, request.surfaces, request.is_crusher))
                .unwrap_or(true)
                || self.hierarchical_zones_join_via_bridge(
                    start_c,
                    goal_grid,
                    request.surfaces,
                    request.is_crusher,
                );
            if !hier_ok {
                if let Ok(mut zones) = self.zones.lock() {
                    zones.set_all_passable();
                }
            }
        }

        let start = Self::cell_for_unit_position(&request.from, center_in_cell);
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal_grid) {
            return PathResult::none();
        }
        if request.is_human
            && (!self.in_logical_extent(start) || !self.in_logical_extent(goal_grid))
        {
            // Computer can leave logical map; humans cannot start outside.
            if request.is_human && !self.in_logical_extent(start) {
                return PathResult::none();
            }
        }

        let surfaces = request.surfaces;
        let is_crusher = request.is_crusher;
        let is_human = request.is_human;
        let can_path_through_units = request.move_allies; // C++ canPathThroughUnits loosely
        let path_cost_multiplier = 1.0f32;

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
        let heuristic = |c: GridCoord| -> i32 {
            let dx = (goal_grid.x - c.x).abs();
            let dy = (goal_grid.y - c.y).abs();
            let dmin = dx.min(dy);
            let dmax = dx.max(dy);
            COST_DIAG * dmin + COST_ORTHO * (dmax - dmin)
        };

        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> =
            std::collections::BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        let h0 = heuristic(start);
        open.push(std::cmp::Reverse((h0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);

        let mut closest_cell: Option<(GridCoord, f32)> = None;
        let mut closest_screen_sqr = f32::MAX;
        let mut found_goal_cell = false;
        let mut expanded = 0i32;

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            expanded += 1;
            if expanded > MAX_EXPAND {
                break;
            }
            let cell = GridCoord::new(cx, cy);
            let layer = self.get_layer_for_coord(cell);

            if cx == goal_grid.x && cy == goal_grid.y {
                // C++: if goal invalid destination and we have closer, keep scanning.
                let goal_ok = can_path_through_units
                    || self.is_destination_valid(
                        cell,
                        layer,
                        surfaces,
                        is_crusher,
                        radius,
                        center_in_cell,
                        request.ignore_obstacle_id,
                    );
                if goal_ok || closest_cell.is_none() {
                    found_goal_cell = true;
                    closest_cell = Some((cell, 0.0));
                    break;
                } else {
                    found_goal_cell = true;
                    // continue scanning for closer valid cell
                }
            } else if !self.is_tunneling
                && self.is_destination_valid(
                    cell,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    request.ignore_obstacle_id,
                )
            {
                // C++: if (!startedStuck || validMovementPosition(...))
                let movement_ok = !started_stuck
                    || self.valid_movement_cell(
                        surfaces,
                        is_crusher,
                        cell,
                        request.ignore_obstacle_id,
                    );
                if movement_ok {
                    let dx = (goal_grid.x - cx).abs() as f32;
                    let dy = (goal_grid.y - cy).abs() as f32;
                    let dist_screen = dx * dx + dy * dy;
                    if dist_screen < closest_screen_sqr {
                        closest_screen_sqr = dist_screen;
                    }
                    let cost_term = (g as f32)
                        * (g as f32)
                        * COST_TO_DISTANCE_FACTOR_SQR
                        * path_cost_multiplier;
                    let dist_sqr = dist_screen + cost_term;
                    let better = match closest_cell {
                        None => true,
                        Some((_, best)) => dist_sqr < best,
                    };
                    if better {
                        closest_cell = Some((cell, dist_sqr));
                    }
                }
            }

            // C++ checkChangeLayers: enqueue connect-layer same-xy at parent cost.
            if let Some(link) = self.check_change_layers(cell) {
                if !closed.contains(&(link.x, link.y)) {
                    let key = (link.x, link.y);
                    if !g_score.get(&key).is_some_and(|&og| g >= og) {
                        g_score.insert(key, g);
                        let f = g + heuristic(link);
                        open.push(std::cmp::Reverse((f, g, link.x, link.y)));
                    }
                }
            }
            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridCoord::new(nx, ny);
                if !self.is_valid_coord(nc) || closed.contains(&(nx, ny)) {
                    continue;
                }
                if is_human && !self.in_logical_extent(nc) {
                    continue;
                }
                // C++ 6181-6185: skip diagonal only if BOTH adjacent flags false.
                if Self::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                {
                    let Ok(pf) = self.pathfinder.lock() else {
                        continue;
                    };
                    if !self.is_tunneling && !pf.is_passable(nc, surfaces, is_crusher) {
                        continue;
                    }
                }
                neighbor_flags[i] = true;
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                let f = ng + heuristic(nc);
                open.push(std::cmp::Reverse((f, ng, nx, ny)));
            }
        }

        let Some((best_cell, _)) = closest_cell else {
            return PathResult::none();
        };

        // Path to exact goal or closest valid cell.
        let to_pos = if found_goal_cell && best_cell.x == goal_grid.x && best_cell.y == goal_grid.y
        {
            request.to
        } else {
            let layer = self.get_layer_for_coord(best_cell);
            let mut p = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell(best_cell.x, best_cell.y, center_in_cell, &mut p, layer);
            p
        };
        let from = request.from;
        request.to = to_pos;
        // Use internal path to avoid hierarchical precheck doubling work.
        let result = self.find_path_internal(request);
        if result.success {
            result
        } else if aircraft_goal_only {
            Self::destination_only_result(from, to_pos, self.get_layer_for_coord(best_cell))
        } else {
            PathResult::none()
        }
    }
}
