use super::*;

impl PathfindingSystem {
    /// C++ `Pathfinder::findPath` (AIPathfind.cpp:6364-6433).
    ///
    /// 1) clientSafeQuickDoesPathExist zone gate  
    /// 2) hierarchical path probe → clearPassableFlags; on failure setAllPassable  
    /// 3) internalFindPath A*
    pub fn find_path(&mut self, request: PathRequest) -> PathResult {
        self.find_path_with_movement_policy(request, None)
    }

    /// Path search using locomotor policy already borrowed from the AI that
    /// owns the queued path request. `None` retains the ID-based boundary.
    pub(crate) fn find_path_with_movement_policy(
        &mut self,
        request: PathRequest,
        downhill_only: Option<bool>,
    ) -> PathResult {
        self.find_path_with_observations(request, downhill_only, None)
            .expect("compatibility path observations are infallible")
    }

    pub(crate) fn find_path_with_observations(
        &mut self,
        mut request: PathRequest,
        downhill_only: Option<bool>,
        observations: Option<&PathfindObservations>,
    ) -> Result<PathResult, String> {
        if let Some(observations) = observations {
            request.is_human = observations.owner_is_human(request.object_id)?;
        }

        let downhill_only =
            downhill_only.unwrap_or_else(|| Self::object_is_downhill_only(request.object_id));
        // C++ findPath: clientSafeQuickDoesPathExist first.
        if !self.client_safe_quick_does_path_exist_with_observations(
            request.surfaces,
            &request.from,
            &request.to,
            request.ignore_obstacle_id,
            observations,
        ) {
            return Ok(PathResult::none());
        }

        // C++: clear_passable_flags; hierarchical probe; if no hPat → set_all_passable.
        // Probe marks A* zone-block passable (examineCellsCallback :6005).
        self.clear_zone_passable_flags();

        let start = GridCoord::from_world(&request.from);
        let goal = GridCoord::from_world(&request.to);
        let zone_join = {
            let connected =
                self.zones
                    .are_connected(start, goal, request.surfaces, request.is_crusher);
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
        let hier_ok = self.pathfinder.apply_hierarchical_zone_prune(
            start,
            goal,
            request.surfaces,
            request.is_crusher,
            &jumps,
        );
        if !hier_ok {
            // C++ setAllPassable / leftover set_all_passable
            self.set_all_zone_passable();
        }

        let _ = zone_join;

        let result =
            self.find_path_internal_with_observations(request, Some(downhill_only), observations)?;

        Ok(result)
    }

    /// Internal path finding implementation
    /// Matches C++ Pathfinder::internalFindPath() at AIPathfind.cpp:6438-6694
    pub(crate) fn find_path_internal(&mut self, request: PathRequest) -> PathResult {
        self.find_path_internal_with_downhill_policy(request, None)
    }

    fn find_path_internal_with_downhill_policy(
        &mut self,
        request: PathRequest,
        downhill_policy: Option<bool>,
    ) -> PathResult {
        self.find_path_internal_with_observations(request, downhill_policy, None)
            .expect("compatibility path observations are infallible")
    }

    pub(crate) fn find_path_internal_with_observations(
        &mut self,
        mut request: PathRequest,
        downhill_policy: Option<bool>,
        observations: Option<&PathfindObservations>,
    ) -> Result<PathResult, String> {
        if let Some(observations) = observations {
            request.is_human = observations.owner_is_human(request.object_id)?;
        }

        let observation_error = std::cell::RefCell::new(None);
        let start = GridCoord::from_world(&request.from);
        let goal = GridCoord::from_world(&request.to);
        // Validate coordinates
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal) {
            return Ok(PathResult::none());
        }
        // C++ seeds the clipped start even outside the logical extent. Human
        // restrictions apply to examined neighbors, including line seeding.

        // C++ internalFindPath determines tunneling before the zone test.
        let start_is_obstacle =
            self.pathfinder.get_cell_type(start) == Some(PathfindCellType::Obstacle);
        let ignored_goal = matches!(
            self.pathfinder.obstacle_observation_at(goal, PathfindLayerEnum::Ground),
            Some((PathfindCellType::Obstacle, Some(owner), _))
                if owner != INVALID_ID && request.ignore_obstacle_id == Some(owner)
        );
        let zones = &self.zones;
        let mut start_zone =
            zones.get_effective_zone(request.surfaces, request.is_crusher, zones.zone_at(start));
        let mut goal_zone =
            zones.get_effective_zone(request.surfaces, request.is_crusher, zones.zone_at(goal));
        // AIPathfind.cpp:6544–6552 enters/leaves buildings through terrain zones.
        if ignored_goal || start_is_obstacle {
            start_zone = zones.get_effective_zone(
                request.surfaces,
                request.is_crusher,
                zones.get_effective_terrain_zone(start_zone),
            );
            goal_zone = zones.get_effective_zone(
                request.surfaces,
                request.is_crusher,
                zones.get_effective_terrain_zone(goal_zone),
            );
        }
        if start_zone != goal_zone {
            return Ok(PathResult::none());
        }

        // C++ internalFindPath tunneling: start in obstacle → ignore obstacles until clear.
        let mut tunneling = start_is_obstacle;
        if !tunneling {
            // Source invalid movement → cheat tunnel (C++ validMovementPosition source fail).
            {
                let pf = &self.pathfinder;
                if !pf.is_passable(start, request.surfaces, request.is_crusher) {
                    tunneling = true;
                }
            }
        }
        // Persist for callers that read is_tunneling during this path.
        // Note: PathfindingSystem is &self here — use interior via cell local only.
        let is_dozer = observations.map_or_else(
            || Self::object_is_dozer(request.object_id),
            |world| {
                world.object(request.object_id).is_some_and(|object| {
                    object
                        .read()
                        .expect("admitted path owner poisoned")
                        .is_kind_of(KindOf::Dozer)
                })
            },
        );
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
        let ally_extra = |cell: GridCoord, layer: PathfindLayerEnum| -> u32 {
            if obj_id == INVALID_ID {
                return 0;
            }
            let mut info = CheckMovementInfo {
                cell,
                layer,
                center_in_cell,
                radius,
                consider_transient: false,
                acceptable_surfaces: request.surfaces,
                ..Default::default()
            };
            if !match self.check_for_movement_with_observations(
                obj_id,
                &mut info,
                request.ignore_obstacle_id.unwrap_or(INVALID_ID),
                observations,
            ) {
                Ok(value) => value,
                Err(error) => {
                    *observation_error.borrow_mut() = Some(error);
                    false
                }
            } {
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
        let downhill_only =
            downhill_policy.unwrap_or_else(|| Self::object_is_downhill_only(request.object_id));
        let ground_h = |cell: GridCoord| -> f32 {
            let wx = (cell.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            let wy = (cell.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            if let Some(world) = observations {
                world.terrain.get_layer_height(
                    wx,
                    wy,
                    crate::path::PathfindLayerEnum::Ground,
                    None,
                    false,
                )
            } else if let Some(terrain) = TheTerrainLogic::get() {
                terrain.get_layer_height(wx, wy, CommonPathfindLayerEnum::Ground)
            } else {
                0.0
            }
        };

        // Run A* pathfinding
        let pathfinder = &self.pathfinder;
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
            if let Some(world) = observations {
                let Some(source) = world.object(dozer_id) else {
                    return false;
                };
                let Some(target) = world.object(obs_id) else {
                    return false;
                };
                let source = source.read().expect("admitted dozer poisoned");
                if dozer_id == obs_id {
                    return true;
                }
                let target = target.read().expect("admitted obstacle poisoned");
                world.relationship(&source, &target) != Relationship::Enemies
            } else {
                Self::dozer_hack_allows_obstacle(dozer_id, obs_id)
            }
        };
        // C++ examineCellsCallback: abort line on enemyFixed / allyFixedCount.
        let line_ok = |cell: GridCoord, from_layer: PathfindLayerEnum| -> bool {
            if obj_id == INVALID_ID {
                return true;
            }
            let mut info = CheckMovementInfo {
                cell,
                layer: from_layer,
                center_in_cell,
                radius,
                consider_transient: false,
                acceptable_surfaces: request.surfaces,
                ..Default::default()
            };
            if !match self.check_for_movement_with_observations(
                obj_id,
                &mut info,
                request.ignore_obstacle_id.unwrap_or(INVALID_ID),
                observations,
            ) {
                Ok(value) => value,
                Err(error) => {
                    *observation_error.borrow_mut() = Some(error);
                    false
                }
            } {
                return false;
            }
            if info.enemy_fixed || info.ally_fixed_count > 0 {
                return false;
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
        let grid_path = pathfinder.find_path_ex6_with_obstacle_owner(
            start,
            goal,
            request.surfaces,
            request.is_crusher,
            MAX_PATH_ITERATIONS,
            request.allow_partial,
            request.ignore_obstacle_id,
            Some(&ally_extra as &dyn Fn(GridCoord, PathfindLayerEnum) -> u32),
            downhill_only,
            Some(&ground_h as &dyn Fn(GridCoord) -> f32),
            Some(&force_pass as &dyn Fn(GridCoord) -> bool),
            Some(&line_ok as &dyn Fn(GridCoord, PathfindLayerEnum) -> bool),
            seed_line,
            tunneling,
            Some(&cell_allowed as &dyn Fn(GridCoord) -> bool),
            dozer_ok_ref,
        );

        if let Some(error) = observation_error.into_inner() {
            return Err(error);
        }
        let Some((grid_path, cells_examined)) = grid_path else {
            return Ok(PathResult::none());
        };
        // C++ m_cumulativeCellsAllocated += cells examined this path.
        self.cumulative_cells_allocated = self
            .cumulative_cells_allocated
            .wrapping_add(cells_examined as i32);

        // Convert grid path via buildActualPath (centerInCell from unit radius).
        // Matches C++ buildActualPath() at AIPathfind.cpp:8954-9071
        let (_radius, center_in_cell) = Self::compute_radius_and_center(request.unit_radius);
        let built = self.build_actual_path_from_layered_path_with_observations(
            &grid_path,
            &request.from,
            &request.to,
            request.surfaces,
            request.is_crusher,
            false,
            center_in_cell,
            request.object_id,
            observations,
        );
        if built.success {
            let mut result = built;
            let coordinate_path: Vec<_> = grid_path.iter().map(|(coord, _)| *coord).collect();
            result.total_cost = self.calculate_path_cost(&coordinate_path);
            // C++ path->optimize(obj, surfaces, blocked) after prependCells.
            let optimized = self.optimize_path_blocked_with_observations(
                &result.waypoints,
                &result.layers,
                &request,
                result.blocked_by_ally,
                observations,
            )?;
            let opt_len = optimized.0.len();
            return Ok(PathResult {
                success: true,
                waypoints: optimized.0,
                layers: optimized.1,
                can_optimize: vec![true; opt_len],
                total_cost: result.total_cost,
                blocked_by_ally: result.blocked_by_ally,
            });
        }
        // C++ internalFindPath returns NULL when buildActualPath fails. Do not
        // reconstruct a path from raw grid coordinates: that bypasses the
        // layer/terrain/occupancy validation performed by buildActualPath and
        // can turn an A* failure into a straight march through an invalid cell.
        Ok(PathResult::none())
    }

    /// Find closest reachable path (for blocked destinations)
    /// Matches C++ Pathfinder::findClosestPath() at AIPathfind.cpp:8739-8926
    /// C++ `Pathfinder::findClosestPath` (AIPathfind.cpp:8739+).
    ///
    /// Hierarchical passable dance, then A* from start tracking the closest
    /// valid destination cell to the goal (screen distance + cost factor).
    /// Exact goal success returns buildActualPath; else path to closest cell.
    pub fn find_closest_path(&mut self, request: PathRequest) -> PathResult {
        self.find_closest_path_with_movement_policy(request, None, None)
    }

    pub(crate) fn find_closest_path_with_movement_policy(
        &mut self,
        mut request: PathRequest,
        downhill_only: Option<bool>,
        aircraft_goal_only: Option<bool>,
    ) -> PathResult {
        self.find_closest_path_with_observations(request, downhill_only, aircraft_goal_only, None)
            .expect("compatibility path observations are infallible")
    }

    pub(crate) fn find_closest_path_with_observations(
        &mut self,
        mut request: PathRequest,
        downhill_only: Option<bool>,
        aircraft_goal_only: Option<bool>,
        observations: Option<&PathfindObservations>,
    ) -> Result<PathResult, String> {
        if let Some(observations) = observations {
            request.is_human = observations.owner_is_human(request.object_id)?;
        }

        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        // C++ COST_TO_DISTANCE_FACTOR = 1/10 → SQR = 1/100.
        const COST_TO_DISTANCE_FACTOR_SQR: f32 = 0.01;
        const MAX_EXPAND: i32 = 4000;

        if !self.is_map_ready {
            return Ok(PathResult::none());
        }
        // C++ findClosestPath resets this per operation, including early returns.
        self.is_tunneling = false;

        let goal_grid = GridCoord::from_world(&request.to);
        let (radius, center_in_cell) = Self::compute_radius_and_center(request.unit_radius);
        let aircraft_goal_only = aircraft_goal_only
            .unwrap_or_else(|| Self::object_uses_aircraft_goal_reservations(request.object_id));
        let downhill_only =
            downhill_only.unwrap_or_else(|| Self::object_is_downhill_only(request.object_id));

        if aircraft_goal_only {
            let goal_layer = self.get_layer_for_coord(goal_grid);
            if self.check_destination_with_observations(
                &request,
                goal_grid,
                goal_layer,
                radius,
                center_in_cell,
                Some(aircraft_goal_only),
                observations,
            ) {
                let adjusted =
                    self.world_pos_for_coord_with_observations(goal_grid, goal_layer, observations);
                return Ok(Self::destination_only_result(
                    request.from,
                    adjusted,
                    goal_layer,
                ));
            }
            // Aircraft without exact goal: fall through to closest-cell A*.
        }

        let start = Self::cell_for_unit_position(&request.from, center_in_cell);
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal_grid) {
            return Ok(PathResult::none());
        }
        if request.is_human && !self.in_logical_extent(start) {
            return Ok(PathResult::none());
        }

        let start_layer = self.get_layer_for_coord(start);
        let mut start_info = CheckMovementInfo {
            cell: start,
            layer: start_layer,
            radius,
            center_in_cell,
            acceptable_surfaces: request.surfaces,
            ..Default::default()
        };
        let start_clear = self.check_for_movement_with_observations(
            request.object_id,
            &mut start_info,
            request.ignore_obstacle_id.unwrap_or(INVALID_ID),
            observations,
        )?;
        let started_stuck = !self.valid_movement_cell_on_layer(
            request.surfaces,
            request.is_crusher,
            start,
            start_layer,
            request.ignore_obstacle_id,
        ) || !start_clear
            || start_info.enemy_fixed;
        let mut tunneling = started_stuck;

        // C++ hierarchical passable flags (unless tunneling).
        if tunneling {
            self.zones.set_all_passable();
        } else {
            self.zones.clear_passable_flags();
            let start_c = GridCoord::from_world(&request.from);
            let hier_ok =
                self.zones
                    .are_connected(start_c, goal_grid, request.surfaces, request.is_crusher)
                    || self.hierarchical_zones_join_via_bridge(
                        start_c,
                        goal_grid,
                        request.surfaces,
                        request.is_crusher,
                    );
            if !hier_ok {
                self.zones.set_all_passable();
            }
        }

        let surfaces = request.surfaces;
        let is_crusher = request.is_crusher;
        let is_human = request.is_human;
        let can_path_through_units = request.move_allies; // C++ canPathThroughUnits loosely
        let path_cost_multiplier = 1.0f32;
        let ground_height = |cell: GridCoord| {
            let x = cell.x as f32 * PATHFIND_CELL_SIZE_F;
            let y = cell.y as f32 * PATHFIND_CELL_SIZE_F;
            observations.map_or_else(
                || {
                    TheTerrainLogic::get()
                        .map(|terrain| {
                            terrain.get_layer_height(x, y, CommonPathfindLayerEnum::Ground)
                        })
                        .unwrap_or(0.0)
                },
                |world| world.terrain.get_ground_height(x, y, None),
            )
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
        let mut parents: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        let h0 = heuristic(start);
        open.push(std::cmp::Reverse((h0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);

        let mut closest_cell: Option<(GridCoord, f32)> = None;
        let mut closest_screen_sqr = f32::MAX;
        let mut found_goal_cell = false;
        let mut expanded = 0i32;
        let destination_valid = |system: &Self, cell, layer| {
            if observations.is_some() {
                system.check_destination_with_observations(
                    &request,
                    cell,
                    layer,
                    radius,
                    center_in_cell,
                    Some(aircraft_goal_only),
                    observations,
                )
            } else {
                system.is_destination_valid(
                    cell,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    request.ignore_obstacle_id,
                )
            }
        };

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
                let goal_ok = can_path_through_units || destination_valid(self, cell, layer);
                if goal_ok || closest_cell.is_none() {
                    found_goal_cell = true;
                    closest_cell = Some((cell, 0.0));
                    break;
                } else {
                    found_goal_cell = true;
                    // C++ keeps scanning the existing frontier, without expanding it.
                    continue;
                }
            } else if !tunneling && destination_valid(self, cell, layer) {
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

            if found_goal_cell {
                continue;
            }

            // C++ checkChangeLayers: enqueue connect-layer same-xy at parent cost.
            if let Some(link) = self.check_change_layers(cell) {
                if !closed.contains(&(link.x, link.y)) {
                    let key = (link.x, link.y);
                    if !g_score.get(&key).is_some_and(|&og| g >= og) {
                        g_score.insert(key, g);
                        parents.insert(key, (cx, cy));
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
                if downhill_only && ground_height(cell) < ground_height(nc) {
                    continue;
                }
                let movement_valid = self.valid_movement_cell_on_layer(
                    surfaces,
                    is_crusher,
                    nc,
                    layer,
                    request.ignore_obstacle_id,
                );
                if !movement_valid && !tunneling {
                    continue;
                }
                // The original records the terrain opening before occupancy.
                neighbor_flags[i] = true;
                let mut info = CheckMovementInfo {
                    cell: nc,
                    layer,
                    radius,
                    center_in_cell,
                    acceptable_surfaces: surfaces,
                    ..Default::default()
                };
                let occupancy_clear = self.check_for_movement_with_observations(
                    request.object_id,
                    &mut info,
                    request.ignore_obstacle_id.unwrap_or(INVALID_ID),
                    observations,
                )? && !info.enemy_fixed;
                if !occupancy_clear && !tunneling {
                    continue;
                }
                let resolved_layer = self
                    .pathfinder
                    .resolved_layer_at(nc, layer)
                    .unwrap_or(layer);
                if movement_valid
                    && occupancy_clear
                    && !self
                        .pathfinder
                        .is_pinched_on_layer(nc, resolved_layer)
                        .unwrap_or(false)
                {
                    tunneling = false;
                }
                let step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                parents.insert(key, (cx, cy));
                let f = ng + if tunneling { 0 } else { heuristic(nc) };
                open.push(std::cmp::Reverse((f, ng, nx, ny)));
            }
        }

        let Some((best_cell, _)) = closest_cell else {
            return Ok(PathResult::none());
        };

        // Path to exact goal or closest valid cell.
        let to_pos = if found_goal_cell && best_cell.x == goal_grid.x && best_cell.y == goal_grid.y
        {
            request.to
        } else {
            let layer = self.get_layer_for_coord(best_cell);
            let mut p = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell_with_observations(
                best_cell.x,
                best_cell.y,
                center_in_cell,
                &mut p,
                layer,
                observations,
            );
            p
        };
        // buildActualPath follows the parent chain from this search; a second
        // search can reject or choose a different route out of a stuck start.
        let mut cursor = (best_cell.x, best_cell.y);
        let start_key = (start.x, start.y);
        let mut grid_path = vec![best_cell];
        while cursor != start_key {
            let Some(&parent) = parents.get(&cursor) else {
                return Err("closest path parent missing".into());
            };
            cursor = parent;
            grid_path.push(GridCoord::new(cursor.0, cursor.1));
            if grid_path.len() > parents.len() + 1 {
                return Err("closest path parent cycle".into());
            }
        }
        grid_path.reverse();
        let layered: Vec<_> = grid_path
            .iter()
            .map(|&cell| (cell, self.get_layer_for_coord(cell)))
            .collect();
        let mut result = self.build_actual_path_from_layered_path_with_observations(
            &layered,
            &request.from,
            &to_pos,
            surfaces,
            is_crusher,
            false,
            center_in_cell,
            request.object_id,
            observations,
        );
        if !result.success {
            return Ok(result);
        }
        result.total_cost = self.calculate_path_cost(&grid_path);
        request.to = to_pos;
        let (waypoints, layers) = self.optimize_path_blocked_with_observations(
            &result.waypoints,
            &result.layers,
            &request,
            result.blocked_by_ally,
            observations,
        )?;
        result.can_optimize = vec![true; waypoints.len()];
        result.waypoints = waypoints;
        result.layers = layers;
        Ok(result)
    }
}
