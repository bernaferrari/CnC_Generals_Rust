use super::*;

impl PathfindingSystem {
    /// C++ `Pathfinder::snapClosestGoalPosition` (AIPathfind.cpp:5101-5156).
    ///
    /// Snap `pos` to a nearby valid goal cell (3×3 neighborhood). Does not run
    /// the full adjustDestination spiral.
    pub fn snap_closest_goal_position(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        pos: &mut Coord3D,
        unit_radius: f32,
        unit_id: ObjectID,
    ) {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let mut adjust_dest = *pos;
        if !center_in_cell {
            adjust_dest.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_dest.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let layer = self.get_layer_for_coord(GridCoord::from_world(pos));
        let cell = GridCoord::from_world(&adjust_dest);

        // Always snap seed cell first (C++ adjustCoordToCell even if check fails).
        self.adjust_coord_to_cell(
            cell.x,
            cell.y,
            center_in_cell,
            pos,
            PathfindLayerEnum::Ground,
        );

        if self.is_destination_valid(
            cell,
            layer,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            Some(unit_id).filter(|&id| id != INVALID_ID),
        ) {
            return;
        }

        // 3×3 neighborhood
        for i in (cell.x - 1)..(cell.x + 2) {
            for j in (cell.y - 1)..(cell.y + 2) {
                let c = GridCoord::new(i, j);
                if !self.is_valid_coord(c) {
                    continue;
                }
                if self.is_destination_valid(
                    c,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    Some(unit_id).filter(|&id| id != INVALID_ID),
                ) {
                    self.adjust_coord_to_cell(i, j, center_in_cell, pos, layer);
                    return;
                }
            }
        }

        // C++ radius==0: prefer unoccupied goal cell, then non-FIXED present.
        if radius == 0 {
            for i in (cell.x - 1)..(cell.x + 2) {
                for j in (cell.y - 1)..(cell.y + 2) {
                    let c = GridCoord::new(i, j);
                    if !self.is_valid_coord(c) {
                        continue;
                    }
                    if self.goal_cell_available(c, layer, unit_id) {
                        self.adjust_coord_to_cell(i, j, center_in_cell, pos, layer);
                        return;
                    }
                }
            }
            for i in (cell.x - 1)..(cell.x + 2) {
                for j in (cell.y - 1)..(cell.y + 2) {
                    let c = GridCoord::new(i, j);
                    if !self.is_valid_coord(c) {
                        continue;
                    }
                    if !self.goal_cell_fixed_occupied(c, layer) {
                        self.adjust_coord_to_cell(i, j, center_in_cell, pos, layer);
                        return;
                    }
                }
            }
        }
    }

    /// C++ adjustCoordToCell — write cell center (or corner) into pos.
    pub(crate) fn adjust_coord_to_cell(
        &self,
        cell_x: i32,
        cell_y: i32,
        center_in_cell: bool,
        pos: &mut Coord3D,
        layer: PathfindLayerEnum,
    ) {
        let coord = GridCoord::new(cell_x, cell_y);
        let snapped = if center_in_cell {
            coord.to_world(layer)
        } else {
            // Corner-aligned: cell origin + small bias (C++ uses non-center footprint).
            Coord3D::new(
                cell_x as f32 * PATHFIND_CELL_SIZE_F + 0.05,
                cell_y as f32 * PATHFIND_CELL_SIZE_F + 0.05,
                0.0,
            )
        };
        pos.x = snapped.x;
        pos.y = snapped.y;
        if let Some(terrain) = TheTerrainLogic::get() {
            pos.z = terrain.get_layer_height(pos.x, pos.y, CommonPathfindLayerEnum::Ground);
        } else {
            pos.z = snapped.z;
        }
    }

    pub(crate) fn goal_cell_available(
        &self,
        cell: GridCoord,
        layer: PathfindLayerEnum,
        unit_id: ObjectID,
    ) -> bool {
        let Ok(goals) = self.goal_cells.lock() else {
            return true;
        };
        let Some(row) = goals.get(cell.x as usize) else {
            return true;
        };
        let Some(gc) = row.get(cell.y as usize) else {
            return true;
        };
        let goal = gc.get_goal_unit(layer);
        goal == INVALID_ID || goal == unit_id
    }

    pub(crate) fn goal_cell_fixed_occupied(&self, cell: GridCoord, layer: PathfindLayerEnum) -> bool {
        let Ok(goals) = self.goal_cells.lock() else {
            return false;
        };
        let Some(row) = goals.get(cell.x as usize) else {
            return false;
        };
        let Some(gc) = row.get(cell.y as usize) else {
            return false;
        };
        // C++ getFlags() != UNIT_PRESENT_FIXED
        const UNIT_PRESENT_FIXED: u8 = 0x03;
        let flags = Self::cell_occupancy_flags(gc.get_goal_unit(layer), gc.get_pos_unit(layer));
        flags == UNIT_PRESENT_FIXED
    }

    /// Snap a world position to the nearest cell center.
    /// Matches C++ Pathfinder::adjustCoordToCell() at AIPathfind.cpp:8936-8946.
    /// C++ `Pathfinder::snapPosition` (AIPathfind.cpp:5082-5095).
    ///
    /// Half-cell bias when not center-in-cell, then adjustCoordToCell on ground.
    pub fn snap_position(&self, pos: &Coord3D) -> Coord3D {
        self.snap_position_for_radius(pos, 0.0)
    }

    /// snapPosition with unit radius → radius/center via getRadiusAndCenter.
    pub fn snap_position_for_radius(&self, pos: &Coord3D, unit_radius: f32) -> Coord3D {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let _ = radius;
        let mut adjust = *pos;
        if !center_in_cell {
            adjust.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let cell = GridCoord::from_world(&adjust);
        let mut out = *pos;
        self.adjust_coord_to_cell(
            cell.x,
            cell.y,
            center_in_cell,
            &mut out,
            PathfindLayerEnum::Ground,
        );
        out
    }

    /// C++ `Pathfinder::goalPosition` (AIPathfind.cpp:5162-5174).
    ///
    /// Returns world position for a unit's tracked pathfind goal cell.
    pub fn goal_position(&self, unit_id: ObjectID, unit_radius: f32, out: &mut Coord3D) -> bool {
        let cell = {
            let Ok(goals) = self.unit_goal_cells.lock() else {
                return false;
            };
            match goals.get(&unit_id).copied() {
                Some(c) if c.x >= 0 && c.y >= 0 => c,
                _ => return false,
            }
        };
        let (_radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        out.x = 0.0;
        out.y = 0.0;
        out.z = 0.0;
        self.adjust_coord_to_cell(
            cell.x,
            cell.y,
            center_in_cell,
            out,
            PathfindLayerEnum::Ground,
        );
        true
    }

    /// C++ `Pathfinder::checkPathCost` (AIPathfind.cpp:8432+).
    ///
    /// Limited A* (MAX_CELL_COUNT=500) returning path `costSoFar` at the goal.
    /// Used by checkForAdjust: reject if `1.4*(|dx|+|dy|) < cost` (world dx/dy).
    /// Returns MAX_COST (0x7fff0000) when no path / not ready / invalid.
    pub fn check_path_cost(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        from: &Coord3D,
        to: &Coord3D,
    ) -> f32 {
        const MAX_COST: f32 = 0x7fff_0000u32 as f32;
        const MAX_CELL_COUNT: i32 = 500;
        // C++ COST_ORTHOGONAL / COST_DIAGONAL style (matches pathfind_astar).
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;

        if !self.is_map_ready {
            return MAX_COST;
        }
        let start = GridCoord::from_world(from);
        let goal = GridCoord::from_world(to);
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal) {
            return MAX_COST;
        }
        {
            let Ok(pf) = self.pathfinder.lock() else {
                return MAX_COST;
            };
            if !pf.is_passable(start, surfaces, is_crusher) {
                return MAX_COST;
            }
        }
        if start.x == goal.x && start.y == goal.y {
            return 0.0;
        }

        let heuristic = |c: GridCoord| -> i32 {
            let dx = (goal.x - c.x).abs();
            let dy = (goal.y - c.y).abs();
            // octile
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

        // min-heap by f = g+h; store (f, g, x, y)
        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> =
            std::collections::BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        let h0 = heuristic(start);
        open.push(std::cmp::Reverse((h0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);
        let mut cell_count = 0i32;

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            if cx == goal.x && cy == goal.y {
                // C++ returns getTotalCost at goal (= costSoFar when h=0).
                return g as f32;
            }
            if cell_count > MAX_CELL_COUNT {
                continue;
            }
            let parent = GridCoord::new(cx, cy);
            // C++ checkChangeLayers: enqueue connect-layer same-xy at parent cost.
            if let Some(link) = self.check_change_layers(parent) {
                if !closed.contains(&(link.x, link.y)) {
                    let key = (link.x, link.y);
                    if !g_score.get(&key).is_some_and(|&og| g >= og) {
                        g_score.insert(key, g);
                        let f = g + heuristic(link);
                        open.push(std::cmp::Reverse((f, g, link.x, link.y)));
                        cell_count += 1;
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
                // C++ 6181-6185: one open orthogonal neighborFlag is enough.
                if Self::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                {
                    let Ok(pf) = self.pathfinder.lock() else {
                        continue;
                    };
                    if !pf.is_passable(nc, surfaces, is_crusher) {
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
                cell_count += 1;
            }
        }
        MAX_COST
    }

    /// C++ `Pathfinder::pathDestination` (AIPathfind.cpp:8154+).
    ///
    /// Limited open-list search from `dest` toward `group_dest`, keeping the
    /// closest cell that passes checkForAdjust. Writes result into `dest`.
    pub fn path_destination(
        &self,
        dest: &mut Coord3D,
        group_dest: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        is_human: bool,
    ) -> bool {
        const MAX_CELL_COUNT: i32 = 500;
        if !self.is_map_ready {
            return false;
        }
        // C++ rejects 0,0 as group dest in hierarchical; pathDestination uses groupDest as goal.
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let _ = radius;
        let start = GridCoord::from_world(dest);
        let goal = GridCoord::from_world(group_dest);
        if !self.is_valid_coord(start) || !self.is_valid_coord(goal) {
            return false;
        }

        // Start must be valid movement.
        {
            let Ok(pf) = self.pathfinder.lock() else {
                return false;
            };
            if !pf.is_passable(start, surfaces, is_crusher) {
                return false;
            }
        }

        // BFS/A* lite: expand orthogonal+diagonal like C++, budget MAX_CELL_COUNT.
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
        let mut open: std::collections::VecDeque<GridCoord> = std::collections::VecDeque::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push_back(start);
        closed.insert((start.x, start.y));

        let mut closest: Option<(GridCoord, Coord3D, i32)> = None;
        let mut cell_count: i32 = 0;

        while let Some(parent) = open.pop_front() {
            // C++ checkForAdjust(obj, locomotorSet, isHuman, x, y, layer, radius, center, &pos, groupDest)
            let mut adjust_pos = parent.to_world(PathfindLayerEnum::Ground);
            self.adjust_coord_to_cell(
                parent.x,
                parent.y,
                center_in_cell,
                &mut adjust_pos,
                PathfindLayerEnum::Ground,
            );
            if self.check_for_adjust_ex(
                &mut adjust_pos,
                surfaces,
                is_crusher,
                unit_radius,
                None,
                is_human,
                None, // no unit from-pos in this entry (group search only)
                Some(group_dest),
            ) {
                let dx = (goal.x - parent.x).abs();
                let dy = (goal.y - parent.y).abs();
                let dist = dx * dx + dy * dy;
                let better = match closest {
                    None => true,
                    Some((_, _, best)) => dist < best,
                };
                if better {
                    closest = Some((parent, adjust_pos, dist));
                } else {
                    // C++: if not closer, continue without expanding neighbors
                    continue;
                }
            } else {
                // C++: checkForAdjust failed → continue (no neighbor expand)
                continue;
            }

            if cell_count > MAX_CELL_COUNT {
                continue;
            }
            // C++ checkChangeLayers(parent): enqueue connect-layer same-xy at parent cost.
            if let Some(link) = self.check_change_layers(parent) {
                if !closed.contains(&(link.x, link.y)) {
                    closed.insert((link.x, link.y));
                    open.push_back(link);
                    cell_count += 1;
                }
            }

            let mut neighbor_flags = [false; 8];
            for (i, (dx, dy)) in deltas.iter().enumerate() {
                let nx = parent.x + dx;
                let ny = parent.y + dy;
                let nc = GridCoord::new(nx, ny);
                if !self.is_valid_coord(nc) || closed.contains(&(nx, ny)) {
                    continue;
                }
                // C++ 6181-6185: one open orthogonal neighborFlag is enough.
                if Self::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                {
                    let Ok(pf) = self.pathfinder.lock() else {
                        continue;
                    };
                    if !pf.is_passable(nc, surfaces, is_crusher) {
                        continue;
                    }
                }
                neighbor_flags[i] = true;
                closed.insert((nx, ny));
                open.push_back(nc);
                cell_count += 1;
            }
        }

        if let Some((_, pos, _)) = closest {
            *dest = pos;
            let _ = (radius, center_in_cell);
            true
        } else {
            false
        }
    }

    /// C++ `Pathfinder::updateAircraftGoal` (AIPathfind.cpp:9803-9854).
    ///
    /// Clears prior goal, stamps goalAircraft on ground cells for hover/wings aircraft.
    pub fn update_aircraft_goal(
        &self,
        goal_pos: &Coord3D,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
    ) {
        let new_cell = Self::cell_for_unit_position(goal_pos, center_in_cell);
        if let Ok(goals) = self.unit_goal_cells.lock() {
            if let Some(prev) = goals.get(&unit_id) {
                if prev.x == new_cell.x && prev.y == new_cell.y {
                    return;
                }
            }
        }
        // C++ removeGoal first (clears both unit + aircraft stamps for prior cell).
        self.remove_goal(unit_id, radius, center_in_cell, PathfindLayerEnum::Ground);
        if let Ok(mut goals) = self.unit_goal_cells.lock() {
            goals.insert(unit_id, ICoord2D::new(new_cell.x, new_cell.y));
        }
        self.set_aircraft_goal_cells(
            unit_id,
            ICoord2D::new(new_cell.x, new_cell.y),
            radius,
            center_in_cell,
        );
    }

    /// C++ `Pathfinder::updateGoal` (AIPathfind.cpp:9701+).
    pub fn update_goal(
        &self,
        cell: GridCoord,
        unit_id: ObjectID,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
        interacts_with_bridge_end: bool,
    ) {
        let new_cell = ICoord2D::new(cell.x, cell.y);
        if let Ok(goals) = self.unit_goal_cells.lock() {
            if let Some(prev) = goals.get(&unit_id) {
                if prev.x == new_cell.x && prev.y == new_cell.y {
                    return;
                }
            }
        }
        self.remove_goal(unit_id, radius, center_in_cell, layer);
        if let Ok(mut goals) = self.unit_goal_cells.lock() {
            goals.insert(unit_id, new_cell);
        }
        // C++ updateGoal: LAYER_GROUND → doGround; else doLayer, and also doGround
        // when TheTerrainLogic->objectInteractsWithBridgeEnd.
        let do_layer = layer != PathfindLayerEnum::Ground;
        let do_ground = layer == PathfindLayerEnum::Ground || interacts_with_bridge_end;
        self.set_goal_cells(
            unit_id,
            new_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }

    /// C++ `Pathfinder::removeGoal` (AIPathfind.cpp:9861+).
    pub fn remove_goal(
        &self,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
    ) {
        let goal_cell = {
            let mut goals = match self.unit_goal_cells.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            goals.remove(&unit_id)
        };
        let Some(goal_cell) = goal_cell else {
            return;
        };
        if goal_cell.x < 0 || goal_cell.y < 0 {
            return;
        }
        let mut radius = radius;
        if radius == 0 {
            radius = 1;
        }
        self.clear_goal_cells(
            unit_id,
            goal_cell,
            radius,
            center_in_cell,
            layer,
            true,
            layer != PathfindLayerEnum::Ground,
        );
        // C++ also clears goalAircraft on ground cells.
        self.clear_aircraft_goal_cells(unit_id, goal_cell, radius, center_in_cell);
    }

    /// C++ `Pathfinder::updatePos` (AIPathfind.cpp:9921+).
    pub fn update_pos(
        &self,
        cell: GridCoord,
        unit_id: ObjectID,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
        interacts_with_bridge_end: bool,
    ) {
        if !self.is_map_ready {
            return;
        }
        let new_cell = ICoord2D::new(cell.x, cell.y);
        if let Ok(pos) = self.unit_pos_cells.lock() {
            if let Some(prev) = pos.get(&unit_id) {
                if prev.x == new_cell.x && prev.y == new_cell.y {
                    return;
                }
            }
        }
        self.remove_pos(unit_id, radius, center_in_cell, layer);
        if let Ok(mut pos) = self.unit_pos_cells.lock() {
            pos.insert(unit_id, new_cell);
        }
        // C++ updatePos: setPosUnit on layer (+ ground at bridge end).
        let do_layer = layer != PathfindLayerEnum::Ground;
        let do_ground = layer == PathfindLayerEnum::Ground || interacts_with_bridge_end;
        self.set_pos_cells(
            unit_id,
            new_cell,
            radius,
            center_in_cell,
            layer,
            do_ground,
            do_layer,
        );
    }

    /// C++ `Pathfinder::removePos` — clear previous position footprint.
    pub fn remove_pos(
        &self,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
    ) {
        let cur = {
            let mut pos = match self.unit_pos_cells.lock() {
                Ok(p) => p,
                Err(_) => return,
            };
            pos.remove(&unit_id)
        };
        let Some(cur) = cur else {
            return;
        };
        if cur.x < 0 || cur.y < 0 {
            return;
        }
        let mut radius = radius;
        if radius == 0 {
            radius = 1;
        }
        self.clear_pos_cells(
            unit_id,
            cur,
            radius,
            center_in_cell,
            layer,
            true,
            layer != PathfindLayerEnum::Ground,
        );
    }

    /// C++ `Pathfinder::removeUnitFromPathfindMap` (AIPathfind.cpp:10082).
    pub fn remove_unit_from_pathfind_map(
        &self,
        unit_id: ObjectID,
        radius: i32,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
    ) {
        self.remove_goal(unit_id, radius, center_in_cell, layer);
        self.remove_pos(unit_id, radius, center_in_cell, layer);
    }

    /// Compute goal/pos cell from world like C++ getRadiusAndCenter + worldToCell.
    pub fn cell_for_unit_position(pos: &Coord3D, center_in_cell: bool) -> GridCoord {
        if center_in_cell {
            GridCoord::new(
                (pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        } else {
            GridCoord::new(
                (0.5 + pos.x / PATHFIND_CELL_SIZE_F).floor() as i32,
                (0.5 + pos.y / PATHFIND_CELL_SIZE_F).floor() as i32,
            )
        }
    }

    /// C++ PathfindLayer::classifyMapCell bridge clearance (AIPathfind.cpp:3711+).
    ///
    /// For each cell in bridge bounds: if ground height + LAYER_Z_CLOSE_ENOUGH_F
    /// exceeds bridge deck height, mark ground cell BridgeImpassable (unless
    /// already Obstacle). Entry-point cells keep Clear + connect-layer stamps.
    pub fn classify_bridge_cells(&self, bridge_idx: usize) {
        let Some(bridge) = self.bridges.get(bridge_idx) else {
            return;
        };
        if bridge.destroyed {
            return;
        }
        let lo = bridge.bounds.0;
        let hi = bridge.bounds.1;
        let deck_z = {
            let sx = (bridge.start_cell.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            let sy = (bridge.start_cell.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            let ex = (bridge.end_cell.x as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            let ey = (bridge.end_cell.y as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
            if let Some(terrain) = TheTerrainLogic::get() {
                let zs = terrain.get_layer_height(sx, sy, CommonPathfindLayerEnum::Ground);
                let ze = terrain.get_layer_height(ex, ey, CommonPathfindLayerEnum::Ground);
                zs.max(ze) + PATHFIND_CELL_SIZE_F
            } else {
                PATHFIND_CELL_SIZE_F * 2.0
            }
        };

        let Ok(mut pathfinder) = self.pathfinder.lock() else {
            return;
        };
        for bx in lo.x..=hi.x {
            for by in lo.y..=hi.y {
                let coord = GridCoord::new(bx, by);
                if !self.is_valid_coord(coord) {
                    continue;
                }
                let is_entry = bridge
                    .ground_connect_cells
                    .iter()
                    .any(|c| c.x == bx && c.y == by)
                    || (bridge.start_cell.x == bx && bridge.start_cell.y == by)
                    || (bridge.end_cell.x == bx && bridge.end_cell.y == by);
                if is_entry {
                    if pathfinder.get_cell_type(coord) != Some(PathfindCellType::Obstacle) {
                        pathfinder.set_cell_type(coord, PathfindCellType::Clear);
                    }
                    continue;
                }
                let cx = (bx as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
                let cy = (by as f32 + 0.5) * PATHFIND_CELL_SIZE_F;
                let ground_z = if let Some(terrain) = TheTerrainLogic::get() {
                    terrain.get_layer_height(cx, cy, CommonPathfindLayerEnum::Ground)
                } else {
                    0.0
                };
                if ground_z + LAYER_Z_CLOSE_ENOUGH_F > deck_z {
                    if pathfinder.get_cell_type(coord) != Some(PathfindCellType::Obstacle) {
                        pathfinder.set_cell_type(coord, PathfindCellType::BridgeImpassable);
                    }
                }
            }
        }
    }

    /// Change bridge state on the pathfind map.
    /// Matches C++ PathfindLayer::setDestroyed() at AIPathfind.cpp:3589-3597.
    ///
    /// When destroyed, all bridge cells become BridgeImpassable and the
    /// ground layer is disconnected from the bridge layer.
    pub fn change_bridge_state(&mut self, x: i32, y: i32, destroyed: bool) {
        let coord = GridCoord::new(x, y);
        let Some(idx) = self.bridges.iter().position(|b| b.contains(coord)) else {
            return;
        };
        self.bridges[idx].destroyed = destroyed;
        let lo = self.bridges[idx].bounds.0;
        let hi = self.bridges[idx].bounds.1;
        if destroyed {
            if let Ok(mut pathfinder) = self.pathfinder.lock() {
                for bx in lo.x..=hi.x {
                    for by in lo.y..=hi.y {
                        pathfinder.set_cell_type(
                            GridCoord::new(bx, by),
                            PathfindCellType::BridgeImpassable,
                        );
                    }
                }
            }
        } else {
            for bx in lo.x..=hi.x {
                for by in lo.y..=hi.y {
                    self.classify_map_cell(bx, by);
                }
            }
            self.classify_bridge_cells(idx);
        }
        self.clear_cache();
    }

    /// Get the width of the pathfinding grid.
    pub fn width(&self) -> usize {
        self.width
    }

    /// Get the height of the pathfinding grid.
    pub fn height(&self) -> usize {
        self.height
    }
}
