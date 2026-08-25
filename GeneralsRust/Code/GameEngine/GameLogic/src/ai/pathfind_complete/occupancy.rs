use super::*;

impl PathfindingSystem {
    pub(crate) fn set_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
        do_ground: bool,
        do_layer: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                if do_ground {
                    cell.set_goal_unit(PathfindLayerEnum::Ground, unit_id);
                }
                if do_layer {
                    cell.set_goal_unit(layer, unit_id);
                }
            }
        });
    }

    /// C++ setPosUnit footprint stamp (UNIT_PRESENT_FIXED).
    pub(crate) fn set_pos_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
        do_ground: bool,
        do_layer: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                if do_ground {
                    cell.set_pos_unit(PathfindLayerEnum::Ground, unit_id);
                }
                if do_layer {
                    cell.set_pos_unit(layer, unit_id);
                }
            }
        });
    }

    pub(crate) fn clear_pos_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
        clear_ground: bool,
        clear_layer: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                if clear_ground {
                    cell.clear_pos_unit(PathfindLayerEnum::Ground, unit_id);
                }
                if clear_layer {
                    cell.clear_pos_unit(layer, unit_id);
                }
            }
        });
    }

    pub(crate) fn clear_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
        clear_ground: bool,
        clear_layer: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                if clear_ground {
                    cell.clear_goal_unit(PathfindLayerEnum::Ground, unit_id);
                }
                if clear_layer {
                    cell.clear_goal_unit(layer, unit_id);
                }
            }
        });
    }

    pub(crate) fn set_aircraft_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                cell.set_goal_aircraft(unit_id);
            }
        });
    }

    pub(crate) fn clear_aircraft_goal_cells(
        &self,
        unit_id: ObjectID,
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
    ) {
        let Ok(mut goals) = self.goal_cells.lock() else {
            return;
        };
        self.for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !self.is_valid_coord(coord) {
                return;
            }
            if let Some(cell) = goals
                .get_mut(coord.x as usize)
                .and_then(|row| row.get_mut(coord.y as usize))
            {
                cell.clear_goal_aircraft(unit_id);
            }
        });
    }

    pub(crate) fn get_goal_unit(&self, coord: GridCoord, layer: PathfindLayerEnum) -> ObjectID {
        let Ok(goals) = self.goal_cells.lock() else {
            return INVALID_ID;
        };
        goals
            .get(coord.x as usize)
            .and_then(|row| row.get(coord.y as usize))
            .map(|cell| cell.get_goal_unit(layer))
            .unwrap_or(INVALID_ID)
    }

    pub(crate) fn get_goal_aircraft(&self, coord: GridCoord) -> ObjectID {
        let Ok(goals) = self.goal_cells.lock() else {
            return INVALID_ID;
        };
        goals
            .get(coord.x as usize)
            .and_then(|row| row.get(coord.y as usize))
            .map(|cell| cell.goal_aircraft)
            .unwrap_or(INVALID_ID)
    }

    pub(crate) fn has_aircraft_goal(&self, coord: GridCoord) -> bool {
        let Ok(goals) = self.goal_cells.lock() else {
            return false;
        };
        goals
            .get(coord.x as usize)
            .and_then(|row| row.get(coord.y as usize))
            .map(|cell| cell.has_aircraft_goal())
            .unwrap_or(false)
    }

    pub fn refresh_pinched_for_positions(&self, positions: &[Coord3D]) {
        if positions.is_empty() {
            return;
        }

        let mut lo = GridCoord::from_world(&positions[0]);
        let mut hi = lo;
        for pos in positions.iter().skip(1) {
            let coord = GridCoord::from_world(pos);
            lo.x = lo.x.min(coord.x);
            lo.y = lo.y.min(coord.y);
            hi.x = hi.x.max(coord.x);
            hi.y = hi.y.max(coord.y);
        }

        let margin = 2;
        lo.x = (lo.x - margin).max(0);
        lo.y = (lo.y - margin).max(0);
        hi.x = (hi.x + margin).min(self.width.saturating_sub(1) as i32);
        hi.y = (hi.y + margin).min(self.height.saturating_sub(1) as i32);

        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.refresh_pinched_cells_in_bounds(lo, hi);
        }
    }

    /// Line-clear check against impassable cells (matches C++ path validation usage).
    pub fn is_line_clear_between(&self, from: &Coord3D, to: &Coord3D) -> bool {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let dz = to.z - from.z;
        let distance = (dx * dx + dy * dy + dz * dz).sqrt();
        if distance <= f32::EPSILON {
            return true;
        }

        let step = (PATHFIND_CELL_SIZE_F * 0.5).max(0.1);
        let steps = (distance / step).ceil().max(1.0) as i32;

        let Ok(pathfinder) = self.pathfinder.lock() else {
            return true;
        };

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let sample = Coord3D::new(from.x + dx * t, from.y + dy * t, from.z + dz * t);
            let cell = GridCoord::from_world(&sample);
            if pathfinder.is_impassable_cell(cell) {
                return false;
            }
        }

        true
    }

    /// Line passability check using surface mask and optional ignored obstacle.
    /// C++ `Pathfinder::isLinePassable` (default allowPinched=false, isCrusher=false).
    pub fn is_line_passable_for_surfaces(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        self.is_line_passable_ex(from, to, surfaces, false, ignore_obstacle_id, false)
    }

    /// C++ `Pathfinder::isLinePassable` with crusher + allowPinched flags.
    pub fn is_line_passable_ex(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        ignore_obstacle_id: Option<ObjectID>,
        allow_pinched: bool,
    ) -> bool {
        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        self.is_line_passable(
            from,
            to,
            surfaces,
            is_crusher,
            PathfindLayerEnum::Ground,
            ignore_cells.as_ref(),
            allow_pinched,
        )
    }

    /// C++ `Pathfinder::isLinePassable` with object footprint occupancy.
    pub fn is_line_passable_for_object(
        &self,
        obj_id: ObjectID,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        ignore_obstacle_id: Option<ObjectID>,
        allow_pinched: bool,
        blocked: bool,
        unit_radius: f32,
    ) -> bool {
        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        self.is_line_passable_for_object_inner(
            obj_id,
            from,
            to,
            surfaces,
            is_crusher,
            PathfindLayerEnum::Ground,
            ignore_cells.as_ref(),
            allow_pinched,
            blocked,
            radius,
            center_in_cell,
        )
    }

    // ========================================================================
    // GROUP A – Core A* ground pathfinding
    // ========================================================================

    /// Main ground A* pathfinding entry point.
    /// Matches C++ Pathfinder::findPath() at AIPathfind.cpp:6364-6433.
    ///
    /// Returns `PathResult` with full waypoint list from `from` to `to` using
    /// ground-surface A* with zone-based early rejection.
    pub fn find_ground_path(
        &self,
        from: Coord3D,
        to: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        allow_partial: bool,
        move_allies: bool,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> PathResult {
        let request = PathRequest {
            object_id: INVALID_ID,
            from,
            to,
            surfaces,
            is_crusher,
            unit_radius,
            allow_partial,
            move_allies,
            ignore_obstacle_id,
            is_human: false,
        };
        self.find_path(request)
    }

    /// Build a concrete `Path` (node-linked-list) from an A* grid result.
    /// Matches C++ Pathfinder::buildActualPath() at AIPathfind.cpp:8954-9001.
    ///
    /// Takes a list of grid coordinates and produces a `Path` with world-space
    /// waypoints, terrain layers, and path optimization applied.
    pub(crate) fn object_is_vehicle(object_id: ObjectID) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |g| g.is_kind_of(KindOf::Vehicle))
            .unwrap_or(false)
    }

    pub(crate) fn object_is_idle(object_id: ObjectID) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |g| g.is_idle())
            .unwrap_or(false)
    }

    pub(crate) fn pos_unit_at(&self, cell: GridCoord, layer: PathfindLayerEnum) -> ObjectID {
        let Ok(goals) = self.goal_cells.lock() else {
            return INVALID_ID;
        };
        goals
            .get(cell.x as usize)
            .and_then(|row| row.get(cell.y as usize))
            .map(|gc| gc.get_pos_unit(layer))
            .unwrap_or(INVALID_ID)
    }

    /// C++ obj->isKindOf(KINDOF_DOZER).
    pub(crate) fn object_is_dozer(object_id: ObjectID) -> bool {
        // Missing object → not a dozer (callback None).
        if object_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |g| g.is_kind_of(KindOf::Dozer))
            .unwrap_or(false)
    }

    /// Snapshot CELL_OBSTACLE owners so A* dozerHack does not re-lock pathfinder.
    pub(crate) fn snapshot_cell_obstacle_ids(&self) -> HashMap<(i32, i32), ObjectID> {
        let Ok(pf) = self.pathfinder.lock() else {
            return HashMap::new();
        };
        let mut owners = HashMap::new();
        for x in 0..self.width {
            for y in 0..self.height {
                let c = GridCoord::new(x as i32, y as i32);
                if pf.get_cell_type(c) != Some(PathfindCellType::Obstacle) {
                    continue;
                }
                if let Some(id) = pf.get_cell_obstacle_id(c) {
                    if id != INVALID_ID {
                        owners.insert((c.x, c.y), id);
                    }
                }
            }
        }
        owners
    }

    /// C++ dozerHack: obstacle object exists AND relationship != ENEMIES.
    /// Missing obstacle object → false (fail-closed, not dozerHack).
    pub(crate) fn dozer_hack_allows_obstacle(dozer_id: ObjectID, obstacle_id: ObjectID) -> bool {
        if dozer_id == INVALID_ID || obstacle_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(dozer_id, |dozer| {
                OBJECT_REGISTRY.with_object(obstacle_id, |obstacle| {
                    dozer.relationship_to(obstacle) != Relationship::Enemies
                })
            })
            .flatten()
            .unwrap_or(false)
    }

    /// C++ locomotorSet.isDownhillOnly() for pathing object.
    pub(crate) fn object_is_downhill_only(object_id: ObjectID) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if object_id == INVALID_ID {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |g| {
                if let Some(ai) = g.get_ai_update_interface() {
                    if let Ok(ai_g) = ai.lock() {
                        if let Some(loco) = ai_g.get_cur_locomotor() {
                            if let Ok(loco_g) = loco.lock() {
                                return loco_g.template.downhill_only;
                            }
                        }
                    }
                }
                false
            })
            .unwrap_or(false)
    }

    /// True when a standing ally occupies `cell` (C++ PathfindCell::isBlockedByAlly stamp).
    pub(crate) fn cell_blocked_by_ally(
        &self,
        cell: GridCoord,
        layer: PathfindLayerEnum,
        object_id: ObjectID,
    ) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let pos_unit = {
            let Ok(goals) = self.goal_cells.lock() else {
                return false;
            };
            goals
                .get(cell.x as usize)
                .and_then(|row| row.get(cell.y as usize))
                .map(|gc| gc.get_pos_unit(layer))
                .unwrap_or(INVALID_ID)
        };
        if pos_unit == INVALID_ID || pos_unit == object_id {
            return false;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |self_g| {
                OBJECT_REGISTRY.with_object(pos_unit, |other_g| {
                    self_g.relationship_to(other_g) == crate::common::Relationship::Allies
                })
            })
            .flatten()
            .unwrap_or(false)
    }

    /// Build path from A* grid cells — C++ `buildActualPath` + `prependCells`.
    ///
    /// Walks cells in reverse (goal→start), applies cliff optimize flags, layer
    /// transition handling, and prepends the real unit foot position.
    pub fn build_actual_path(
        &self,
        grid_path: &[GridCoord],
        from_world: &Coord3D,
        to_world: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        blocked: bool,
        center_in_cell: bool,
    ) -> PathResult {
        self.build_actual_path_for_object(
            grid_path,
            from_world,
            to_world,
            surfaces,
            is_crusher,
            blocked,
            center_in_cell,
            INVALID_ID,
        )
    }

    /// C++ buildActualPath with object for isBlockedByAlly cell stamps.
    pub fn build_actual_path_for_object(
        &self,
        grid_path: &[GridCoord],
        from_world: &Coord3D,
        to_world: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        blocked: bool,
        center_in_cell: bool,
        object_id: ObjectID,
    ) -> PathResult {
        let _ = (surfaces, is_crusher);
        if grid_path.is_empty() {
            return PathResult::none();
        }

        // grid_path is start→goal; prependCells walks goal→start.
        // C++ buildActualPath(..., centerInCell, blocked).
        let center = center_in_cell;
        let mut waypoints: Vec<Coord3D> = Vec::with_capacity(grid_path.len() + 1);
        let mut layers: Vec<PathfindLayerEnum> = Vec::with_capacity(grid_path.len() + 1);
        let mut can_optimize: Vec<bool> = Vec::with_capacity(grid_path.len() + 1);
        let mut blocked_by_ally = blocked;

        // Reverse walk excluding the start cell (same cell as unit feet).
        // C++: for (cell = goal; cell->parent; cell = parent)
        let mut prev_type: Option<PathfindCellType> = None;
        let mut prev_layer: Option<PathfindLayerEnum> = None;
        let mut prev_coord: Option<GridCoord> = None;

        for idx in (0..grid_path.len()).rev() {
            let coord = grid_path[idx];
            let layer = self.get_layer_for_coord(coord);
            let ctype = self
                .get_cell_type(&coord.to_world(layer))
                .unwrap_or(PathfindCellType::Clear);

            // Same cell layer transition: skip duplicate x,y (C++ continue).
            if let Some(pc) = prev_coord {
                if pc.x == coord.x && pc.y == coord.y {
                    if let Some(first_layer) = layers.first_mut() {
                        let use_layer = if layer == PathfindLayerEnum::Ground {
                            prev_layer.unwrap_or(layer)
                        } else {
                            layer
                        };
                        *first_layer = use_layer;
                    }
                    prev_type = Some(ctype);
                    prev_layer = Some(layer);
                    continue;
                }
            }

            // Skip last node in reverse (start cell) — unit feet added below.
            if idx == 0 {
                prev_type = Some(ctype);
                prev_layer = Some(layer);
                prev_coord = Some(coord);
                // C++ setPassable(start cell) when building ground path reverse.
                if let Ok(mut zones) = self.zones.lock() {
                    zones.set_passable(coord.x, coord.y, true);
                }
                if let Ok(mut pf) = self.pathfinder.lock() {
                    pf.set_zone_passable(coord, true);
                }
                break;
            }

            let mut can_opt = true;
            if ctype == PathfindCellType::Cliff {
                if prev_type.is_some_and(|t| t != PathfindCellType::Cliff) {
                    if let Some(first) = can_optimize.first_mut() {
                        *first = false;
                    }
                }
            } else if prev_type == Some(PathfindCellType::Cliff) {
                can_opt = false;
            }

            let mut pos = if idx + 1 == grid_path.len() {
                // first reverse step is goal cell — keep requested goal world pos.
                *to_world
            } else {
                // C++ adjustCoordToCell(cellX, cellY, centerInCell, pos, layer).
                let mut p = Coord3D::new(0.0, 0.0, 0.0);
                self.adjust_coord_to_cell(coord.x, coord.y, center, &mut p, layer);
                p
            };
            if let Some(terrain) = TheTerrainLogic::get() {
                pos.z = terrain.get_layer_height(pos.x, pos.y, CommonPathfindLayerEnum::Ground);
            }

            // prepend
            waypoints.insert(0, pos);
            layers.insert(0, layer);
            can_optimize.insert(0, can_opt);

            // C++ cell->isBlockedByAlly() → path.setBlockedByAlly(true)
            if object_id != INVALID_ID {
                if self.cell_blocked_by_ally(coord, layer, object_id) {
                    blocked_by_ally = true;
                }
            }

            prev_type = Some(ctype);
            prev_layer = Some(layer);
            prev_coord = Some(coord);
        }

        // Very short path: only goal (no parent) — C++ goalCellNull.
        if waypoints.is_empty() && !grid_path.is_empty() {
            let coord = *grid_path.last().unwrap();
            let layer = self.get_layer_for_coord(coord);
            let mut pos = *to_world;
            if let Some(terrain) = TheTerrainLogic::get() {
                pos.z = terrain.get_layer_height(pos.x, pos.y, CommonPathfindLayerEnum::Ground);
            }
            waypoints.push(pos);
            layers.push(layer);
            can_optimize.push(true);
        }

        // Prepend actual unit feet if different from first node.
        if let Some(first) = waypoints.first() {
            if (from_world.x - first.x).abs() > 0.01 || (from_world.y - first.y).abs() > 0.01 {
                let layer = layers.first().copied().unwrap_or(PathfindLayerEnum::Ground);
                waypoints.insert(0, *from_world);
                layers.insert(0, layer);
                can_optimize.insert(0, true);
            }
        }

        PathResult {
            success: !waypoints.is_empty(),
            waypoints,
            layers,
            can_optimize,
            total_cost: 0,
            blocked_by_ally,
        }
    }

    /// Classify the entire pathfind map based on terrain data.
    /// Matches C++ Pathfinder::classifyMap() which iterates all cells and sets
    /// terrain cell types, expands cliff cells, and recalculates zones.
    /// C++ `Pathfinder::newMap` (AIPathfind.cpp:4524-4573).
    ///
    /// Resize/classify grid from terrain extent, classify map cells, mark ready.
    /// Object footprint classification is caller-driven (iterate objects).
    /// C++ `Pathfinder::buildGroundPath` (AIPathfind.cpp:6765-6807).
    pub fn build_ground_path(
        &self,
        from: &Coord3D,
        grid_path: &[GridCoord],
        is_crusher: bool,
        center: bool,
        path_diameter: i32,
    ) -> PathResult {
        if grid_path.is_empty() {
            return PathResult::none();
        }
        let to = grid_path
            .last()
            .map(|c| c.to_world(PathfindLayerEnum::Ground))
            .unwrap_or(*from);
        let built = self.build_actual_path(
            grid_path,
            from,
            &to,
            SURFACE_GROUND,
            is_crusher,
            false,
            center,
        );
        if !built.success {
            return built;
        }
        let pass = |a: &Coord3D, b: &Coord3D, _diam: i32| {
            self.is_line_passable_ex(a, b, SURFACE_GROUND, is_crusher, None, false)
        };
        let (waypoints, layers) = self.optimizer.optimize_ground_path(
            &built.waypoints,
            &built.layers,
            is_crusher,
            path_diameter,
            pass,
        );
        let len = waypoints.len();
        PathResult {
            success: !waypoints.is_empty(),
            waypoints,
            layers,
            can_optimize: vec![true; len],
            total_cost: built.total_cost,
            blocked_by_ally: false,
        }
    }

    /// C++ `Pathfinder::buildHierachicalPath` (AIPathfind.cpp:6813-6867).
    pub fn build_hierarchical_path(&self, from: &Coord3D, grid_path: &[GridCoord]) -> PathResult {
        if grid_path.is_empty() {
            return PathResult::none();
        }
        let to = grid_path
            .last()
            .map(|c| c.to_world(PathfindLayerEnum::Ground))
            .unwrap_or(*from);
        let built =
            self.build_actual_path(grid_path, from, &to, SURFACE_GROUND, false, false, true);
        if !built.success || built.waypoints.is_empty() {
            return built;
        }
        // Expand hierarchical path around start: setPassable in ZONE_BLOCK_SIZE box.
        let pos = built.waypoints[0];
        let half = ZONE_BLOCK_SIZE as f32 * PATHFIND_CELL_SIZE_F;
        let min_pos = Coord3D::new(pos.x - half, pos.y - half, pos.z);
        let max_pos = Coord3D::new(pos.x + half, pos.y + half, pos.z);
        let lo = GridCoord::from_world(&min_pos);
        let hi = GridCoord::from_world(&max_pos);
        if let Ok(mut zones) = self.zones.lock() {
            for i in lo.x..=hi.x {
                for j in lo.y..=hi.y {
                    zones.set_passable(i, j, true);
                }
            }
        }
        // Keep A* notZonePassable table in sync with hierarchical expansion.
        if let Ok(mut pf) = self.pathfinder.lock() {
            for i in lo.x..=hi.x {
                for j in lo.y..=hi.y {
                    pf.set_zone_passable(GridCoord::new(i, j), true);
                }
            }
        }
        built
    }

    /// C++ `Pathfinder::setDebugPath`.
    pub fn set_debug_path(&mut self, path: Option<PathResult>) {
        self.debug_path = path;
    }

    pub fn debug_path(&self) -> Option<&PathResult> {
        self.debug_path.as_ref()
    }

    /// C++ `setDebugPathPosition`.
    pub fn set_debug_path_position(&mut self, pos: Coord3D) {
        self.debug_path_pos = pos;
    }

    pub fn debug_path_position(&self) -> Coord3D {
        self.debug_path_pos
    }

    /// C++ `PathfindZoneManager::setBridge`.
    pub fn set_zone_bridge(&self, cell: GridCoord, bridge: bool) {
        if let Ok(mut z) = self.zones.lock() {
            z.set_bridge(cell.x, cell.y, bridge);
        }
    }

    /// C++ `PathfindZoneManager::interactsWithBridge`.
    pub fn zone_interacts_with_bridge(&self, cell: GridCoord) -> bool {
        self.zones
            .lock()
            .map(|z| z.interacts_with_bridge(cell.x, cell.y))
            .unwrap_or(false)
    }

    /// C++ `PathfindZoneManager::setPassable` — zone block + A* cost table.
    pub fn set_zone_cell_passable(&self, cell: GridCoord, passable: bool) {
        if let Ok(mut z) = self.zones.lock() {
            z.set_passable(cell.x, cell.y, passable);
        }
        if let Ok(mut pf) = self.pathfinder.lock() {
            pf.set_zone_passable(cell, passable);
        }
    }

    /// C++ `PathfindZoneManager::clearPassableFlags`.
    pub fn clear_zone_passable_flags(&self) {
        if let Ok(mut z) = self.zones.lock() {
            z.clear_passable_flags();
        }
        if let Ok(mut pf) = self.pathfinder.lock() {
            pf.mark_all_zone_blocks_impassable();
        }
    }

    /// C++ `PathfindZoneManager::setAllPassable` — zone blocks + A* table.
    pub fn set_all_zone_passable(&self) {
        if let Ok(mut z) = self.zones.lock() {
            z.set_all_passable();
        }
        if let Ok(mut pf) = self.pathfinder.lock() {
            pf.clear_zone_passable_flags();
        }
    }

    /// C++ `PathfindZoneManager::markZonesDirty` / force zone rebuild next processQueue.
    pub fn mark_zones_dirty(&self) {
        if let Ok(mut z) = self.zones.lock() {
            z.mark_zones_dirty(true);
        }
    }

    pub fn new_map(&mut self) {
        // Extent from current width/height (already allocated). Re-classify.
        self.extent_lo = ICoord2D::new(0, 0);
        self.extent_hi = ICoord2D::new(
            self.width.saturating_sub(1) as i32,
            self.height.saturating_sub(1) as i32,
        );
        // Default logical = full map until process_queue refreshes from terrain.
        self.logical_extent_lo = self.extent_lo;
        self.logical_extent_hi = self.extent_hi;
        self.classify_map();
        self.recalculate_zones_from_cells();
        self.is_map_ready = true;
    }

    /// Snapshot cell types + fence flags + connect layers; rebuild zones + combiners.
    pub(crate) fn recalculate_zones_from_cells(&mut self) {
        let snapshot = if let Ok(pf) = self.pathfinder.lock() {
            let mut grid = vec![vec![PathfindCellType::Clear; self.height]; self.width];
            let mut fences = vec![vec![false; self.height]; self.width];
            let mut connects = vec![vec![0u8; self.height]; self.width];
            for x in 0..self.width {
                for y in 0..self.height {
                    let c = GridCoord::new(x as i32, y as i32);
                    if let Some(ct) = pf.get_cell_type(c) {
                        grid[x][y] = ct;
                    }
                    fences[x][y] = pf.is_obstacle_fence(c);
                    if let Some(cl) = pf.get_cell_connect_layer(c) {
                        connects[x][y] = cl as u8;
                    }
                }
            }
            Some((grid, fences, connects))
        } else {
            None
        };
        if let Ok(mut zones) = self.zones.lock() {
            if let Some((types, fences, connects)) = snapshot {
                // Flood-fill ground cells once.
                zones.flood_fill_from_types(&types);
                // C++ PathfindLayer::m_zone — each elevated layer gets its own zone id
                // (distinct from ground cells) so connectLayer hierarchical resolve merges.
                let mut layer_zones = vec![0u16; 32];
                for bridge in self.bridges.iter_mut() {
                    let z = zones.allocate_zone_id();
                    bridge.zone = z;
                    let lid = bridge.layer_id as usize;
                    if lid < layer_zones.len() {
                        layer_zones[lid] = z;
                    }
                }
                zones.build_surface_combiners(
                    &types,
                    Some(&fences),
                    Some(&connects),
                    Some(&layer_zones),
                );
                zones.rebuild_zone_blocks(Some(&types), Some(&fences));
                // C++ after layer applyZone: setBridge(start/end) for live layers.
                zones.clear_bridge_flags();
                for bridge in &self.bridges {
                    if bridge.destroyed {
                        continue;
                    }
                    zones.set_bridge(bridge.start_cell.x, bridge.start_cell.y, true);
                    zones.set_bridge(bridge.end_cell.x, bridge.end_cell.y, true);
                    // Also stamp ground-connect entry cells (entry points).
                    for c in &bridge.ground_connect_cells {
                        zones.set_bridge(c.x, c.y, true);
                    }
                }
                zones.zones_dirty = false;
            } else {
                zones.calculate_zones();
            }
        }
    }

    /// C++ `Pathfinder::forceMapRecalculation` — reclassify all cells.
    /// C++ `Pathfinder::checkChangeLayers` (AIPathfind.cpp:5942-5984).
    ///
    /// When a parent cell has a connectLayer link (bridge entry/exit), return the
    /// same-xy transition cell so A* can enqueue it with parent cost.
    pub fn check_change_layers(&self, parent: GridCoord) -> Option<GridCoord> {
        let Ok(pathfinder) = self.pathfinder.lock() else {
            return None;
        };
        let cl = pathfinder.get_cell_connect_layer(parent)?;
        if cl == PathfindLayerEnum::Invalid {
            return None;
        }
        // C++ fetches getCell(connectLayer or GROUND, x, y) at same indices.
        Some(parent)
    }

    /// C++ checkChangeLayers insert: same-xy connect-layer cell if not closed.
    ///
    /// Callers enqueue the result at the parent's `costSoFar` (0 extra cost)
    /// before expanding orthogonal/diagonal neighbors.
    pub fn change_layer_open_link(
        &self,
        parent: GridCoord,
        closed: &HashSet<(i32, i32)>,
    ) -> Option<GridCoord> {
        let link = self.check_change_layers(parent)?;
        if closed.contains(&(link.x, link.y)) {
            None
        } else {
            Some(link)
        }
    }

    /// C++ `examineNeighboringCells` firstDiagonal check (AIPathfind.cpp:6181-6185).
    ///
    /// `adjacent = {0,1,2,3,0}`. Skip diagonal only when BOTH adjacent
    /// orthogonal `neighborFlags` are false — one open orthogonal is enough.
    #[inline]
    pub(crate) fn skip_diagonal_if_squeezed(i: usize, neighbor_flags: &[bool; 8]) -> bool {
        const FIRST_DIAGONAL: usize = 4;
        const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
        i >= FIRST_DIAGONAL && !neighbor_flags[ADJACENT[i - 4]] && !neighbor_flags[ADJACENT[i - 3]]
    }

    /// Stamp connectLayer on a cell (bridge ground-connect / wall link).
    /// C++ PathfindCell::isObstacleTransparent.
    pub fn is_cell_obstacle_transparent(&self, cell: GridCoord) -> bool {
        self.pathfinder
            .lock()
            .map(|pf| pf.is_obstacle_transparent(cell))
            .unwrap_or(false)
    }

    /// C++ PathfindCell::getObstacleID via A* obstacle_owners.
    pub fn get_cell_obstacle_id(&self, cell: GridCoord) -> Option<ObjectID> {
        self.pathfinder
            .lock()
            .ok()
            .and_then(|pf| pf.get_cell_obstacle_id(cell))
    }

    pub fn set_connect_layer(&self, cell: GridCoord, layer: PathfindLayerEnum) {
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.set_cell_connect_layer(cell, layer);
        }
    }

    pub fn force_map_recalculation(&mut self) {
        self.classify_map();
        if !self.wall_pieces.is_empty() {
            self.classify_wall_cells();
        }
        self.recalculate_zones_from_cells();
    }

    /// C++ `Pathfinder::addWallPiece`.
    pub fn add_wall_piece(&mut self, wall_piece_id: ObjectID) {
        if self.wall_pieces.len() < MAX_WALL_PIECES.saturating_sub(1)
            && !self.wall_pieces.contains(&wall_piece_id)
        {
            self.wall_pieces.push(wall_piece_id);
        }
    }

    /// C++ `Pathfinder::removeWallPiece`.
    pub fn remove_wall_piece(&mut self, wall_piece_id: ObjectID) {
        if let Some(i) = self.wall_pieces.iter().position(|&id| id == wall_piece_id) {
            let last = self.wall_pieces.len() - 1;
            self.wall_pieces.swap(i, last);
            self.wall_pieces.pop();
        }
    }

    pub fn wall_piece_count(&self) -> usize {
        self.wall_pieces.len()
    }

    /// C++ `Pathfinder::isPointOnWall` (AIPathfind.cpp:3929-3942).
    pub fn is_point_on_wall(&self, pos: &Coord3D) -> bool {
        if self.wall_pieces.is_empty() {
            return false;
        }
        let cell = GridCoord::from_world(pos);
        let Ok(walls) = self.wall_cells.lock() else {
            return false;
        };
        walls.contains(&(cell.x, cell.y))
    }

    /// Residual wall-cell classification from registered wall piece positions.
    /// C++ `PathfindLayer::classifyWallCells` — marks ground cells under pieces as wall.
    pub fn classify_wall_cells(&mut self) {
        let Ok(mut walls) = self.wall_cells.lock() else {
            return;
        };
        walls.clear();
        if self.wall_pieces.is_empty() {
            return;
        }
        let pieces = self.wall_pieces.clone();
        let w = self.width as i32;
        let h = self.height as i32;
        let mut types: HashMap<(i32, i32), crate::path::PathfindCellType> = HashMap::new();
        for y in 0..h {
            for x in 0..w {
                let mut cell = crate::path::PathfindCell::new();
                crate::path::pathfind_layer_classify::classify_wall_map_cell(
                    x, y, &mut cell, &pieces,
                );
                types.insert((x, y), cell.get_type());
            }
        }
        let mut pinched = HashSet::new();
        for y in 1..h.saturating_sub(1) {
            for x in 1..w.saturating_sub(1) {
                let mut pinch = false;
                'adj: for dy in -1..=1 {
                    for dx in -1..=1 {
                        if types.get(&(x + dx, y + dy)).copied()
                            != Some(crate::path::PathfindCellType::Clear)
                        {
                            pinch = true;
                            break 'adj;
                        }
                    }
                }
                if pinch {
                    pinched.insert((x, y));
                }
            }
        }
        let mut pf = self.pathfinder.lock();
        for y in 0..h {
            for x in 0..w {
                let mut ty = types
                    .get(&(x, y))
                    .copied()
                    .unwrap_or(crate::path::PathfindCellType::Impassable);
                if pinched.contains(&(x, y)) && ty == crate::path::PathfindCellType::Clear {
                    ty = crate::path::PathfindCellType::Cliff;
                }
                if ty == crate::path::PathfindCellType::Clear {
                    walls.insert((x, y));
                }
                if let Ok(finder) = &mut pf {
                    let astar_ty = match ty {
                        crate::path::PathfindCellType::Clear => PathfindCellType::Clear,
                        crate::path::PathfindCellType::Water => PathfindCellType::Water,
                        crate::path::PathfindCellType::Cliff => PathfindCellType::Cliff,
                        crate::path::PathfindCellType::Rubble => PathfindCellType::Rubble,
                        crate::path::PathfindCellType::Obstacle => PathfindCellType::Obstacle,
                        crate::path::PathfindCellType::BridgeImpassable => {
                            PathfindCellType::BridgeImpassable
                        }
                        crate::path::PathfindCellType::Impassable => PathfindCellType::Impassable,
                    };
                    finder.set_cell_type_on_layer(
                        GridCoord::new(x, y),
                        PathfindLayerEnum::Wall,
                        astar_ty,
                    );
                }
            }
        }
    }

    /// Stamp a single wall cell (used when object positions are known).
    pub fn classify_wall_cell_at(&self, x: i32, y: i32, clear_for_walk: bool) {
        if x < 0 || y < 0 || x as usize >= self.width || y as usize >= self.height {
            return;
        }
        if let Ok(mut walls) = self.wall_cells.lock() {
            if clear_for_walk {
                walls.insert((x, y));
            } else {
                walls.remove(&(x, y));
            }
        }
    }

    /// C++ `Pathfinder::updateLayer` — demote to ground if not interacting with bridge.
    pub fn update_layer_for_object(
        &self,
        desired_layer: PathfindLayerEnum,
        interacts_with_bridge_layer: bool,
    ) -> PathfindLayerEnum {
        if desired_layer != PathfindLayerEnum::Ground && !interacts_with_bridge_layer {
            PathfindLayerEnum::Ground
        } else {
            desired_layer
        }
    }

    pub fn is_map_ready(&self) -> bool {
        self.is_map_ready
    }

    pub fn set_ignore_obstacle_id(&mut self, id: ObjectID) {
        self.ignore_obstacle_id = id;
    }

    pub fn ignore_obstacle_id(&self) -> ObjectID {
        self.ignore_obstacle_id
    }

    pub fn set_is_tunneling(&mut self, tunneling: bool) {
        self.is_tunneling = tunneling;
    }

    pub fn is_tunneling(&self) -> bool {
        self.is_tunneling
    }

    pub fn set_wall_height(&mut self, h: f32) {
        self.wall_height = h;
    }

    pub fn wall_height(&self) -> f32 {
        self.wall_height
    }

    pub fn cumulative_cells_allocated(&self) -> i32 {
        self.cumulative_cells_allocated.load(Ordering::Relaxed)
    }

    /// C++ `Pathfinder::cleanOpenAndClosedLists` (AIPathfind.cpp:4788-4824).
    pub fn clean_open_and_closed_lists(&mut self) {
        let mut count = 0i32;
        count += self.open_list_count;
        count += self.closed_list_count;
        self.open_list_count = 0;
        self.closed_list_count = 0;
        let _ = self
            .cumulative_cells_allocated
            .fetch_add(count, Ordering::Relaxed);
    }

    /// Track residual open-list cell allocation (A* bookkeeping).
    pub fn note_open_closed_cells(&mut self, open: i32, closed: i32) {
        self.open_list_count = open.max(0);
        self.closed_list_count = closed.max(0);
    }
}
