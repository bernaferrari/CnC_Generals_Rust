use super::*;

impl PathfindingSystem {
    /// C++ `LineInRegion` style segment vs AABB (2D).
    pub(crate) fn line_in_region(
        start: &Coord2D,
        end: &Coord2D,
        lo_x: f32,
        lo_y: f32,
        hi_x: f32,
        hi_y: f32,
    ) -> bool {
        // Liang-Barsky clip: any endpoint inside or segment crosses AABB.
        let inside = |x: f32, y: f32| x >= lo_x && x <= hi_x && y >= lo_y && y <= hi_y;
        if inside(start.x, start.y) || inside(end.x, end.y) {
            return true;
        }
        let dx = end.x - start.x;
        let dy = end.y - start.y;
        let mut t0 = 0.0f32;
        let mut t1 = 1.0f32;
        let clip = |p: f32, q: f32, t0: &mut f32, t1: &mut f32| -> bool {
            if p.abs() < f32::EPSILON {
                return q >= 0.0;
            }
            let r = q / p;
            if p < 0.0 {
                if r > *t1 {
                    return false;
                }
                if r > *t0 {
                    *t0 = r;
                }
            } else {
                if r < *t0 {
                    return false;
                }
                if r < *t1 {
                    *t1 = r;
                }
            }
            true
        };
        clip(-dx, start.x - lo_x, &mut t0, &mut t1)
            && clip(dx, hi_x - start.x, &mut t0, &mut t1)
            && clip(-dy, start.y - lo_y, &mut t0, &mut t1)
            && clip(dy, hi_y - start.y, &mut t0, &mut t1)
            && t0 <= t1
    }

    /// C++ `Pathfinder::patchPath` (AIPathfind.cpp:10344-10520).
    ///
    /// From current position, A* toward the nearest still-clear original path
    /// node, then splice the remaining original path tail.
    pub fn patch_path(
        &mut self,
        from: &Coord3D,
        original_waypoints: &[Coord3D],
        original_layers: &[PathfindLayerEnum],
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        blocked: bool,
        obj_id: ObjectID,
    ) -> PathResult {
        const CELL_LIMIT: usize = 2000;
        if original_waypoints.len() < 2 || !self.is_map_ready {
            return PathResult::none();
        }
        if let Ok(mut zones) = self.zones.lock() {
            zones.set_all_passable();
        }
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let start = Self::cell_for_unit_position(from, center_in_cell);
        if !self.is_valid_coord(start) {
            return PathResult::none();
        }

        self.is_tunneling = false;
        self.note_open_closed_cells(0, 0);

        // Walk original path reverse; stop at first blocked node.
        let mut start_node_idx = 0usize; // exclusive upper for patchable nodes
        let mut goal_pos = *original_waypoints.last().unwrap();
        let mut goal_delta = {
            let dx = goal_pos.x - from.x;
            let dy = goal_pos.y - from.y;
            dx * dx + dy * dy
        };
        // C++: for startNode = last; startNode != first; startNode = previous
        for idx in (1..original_waypoints.len()).rev() {
            let pos = &original_waypoints[idx];
            let layer = original_layers
                .get(idx)
                .copied()
                .unwrap_or(PathfindLayerEnum::Ground);
            let cell = GridCoord::from_world(pos);
            let mut info = CheckMovementInfo {
                cell,
                layer,
                center_in_cell,
                radius,
                consider_transient: blocked,
                acceptable_surfaces: surfaces,
                ..Default::default()
            };
            let dx = cell.x - start.x;
            let dy = cell.y - start.y;
            if dx < -2 || dx > 2 || dy < -2 || dy > 2 {
                info.consider_transient = false;
            }
            if !self.check_for_movement(obj_id, &mut info)
                || info.ally_fixed_count > 0
                || info.enemy_fixed
            {
                start_node_idx = idx;
                break;
            }
            let cur = {
                let dx = pos.x - from.x;
                let dy = pos.y - from.y;
                dx * dx + dy * dy
            };
            if cur < goal_delta {
                goal_pos = *pos;
                goal_delta = cur;
            }
            start_node_idx = idx; // still open through this node
        }
        // If last node itself failed immediately, C++ returns null when startNode==last
        if start_node_idx + 1 >= original_waypoints.len() {
            self.clean_open_and_closed_lists();
            return PathResult::none();
        }

        // A* from current toward goal_pos (matched path node).
        let mut request = PathRequest {
            object_id: obj_id,
            from: *from,
            to: goal_pos,
            surfaces,
            is_crusher,
            unit_radius,
            allow_partial: true,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human: false,
        };
        // Prefer finding path to a cell that matches some remaining path node coords.
        let mut result = self.find_path(request.clone());
        if !result.success {
            // Try intermediate path nodes between start_node and last.
            for idx in ((start_node_idx + 1)..original_waypoints.len()).rev() {
                request.to = original_waypoints[idx];
                let trial = self.find_path(request.clone());
                if trial.success {
                    result = trial;
                    goal_pos = original_waypoints[idx];
                    break;
                }
            }
        }
        if !result.success {
            self.is_tunneling = false;
            self.clean_open_and_closed_lists();
            return PathResult::none();
        }

        // Find match node on original path by world position of path end.
        let end = result.waypoints.last().copied().unwrap_or(goal_pos);
        let mut match_idx = None;
        for idx in ((start_node_idx + 1)..original_waypoints.len()).rev() {
            let p = &original_waypoints[idx];
            if (p.x - end.x).abs() < 0.5 && (p.y - end.y).abs() < 0.5 {
                match_idx = Some(idx);
                break;
            }
            // Also accept cell equality.
            let a = GridCoord::from_world(p);
            let b = GridCoord::from_world(&end);
            if a.x == b.x && a.y == b.y {
                match_idx = Some(idx);
                break;
            }
        }
        let match_idx = match_idx.unwrap_or(original_waypoints.len() - 1);

        // Splice: patched prefix + original from match to last.
        let mut waypoints = result.waypoints;
        let mut layers = result.layers;
        let mut can_optimize = result.can_optimize;
        // Drop last of patch if it duplicates match
        if let Some(last) = waypoints.last() {
            let m = &original_waypoints[match_idx];
            if (last.x - m.x).abs() < 0.5 && (last.y - m.y).abs() < 0.5 {
                waypoints.pop();
                layers.pop();
                can_optimize.pop();
            }
        }
        for idx in match_idx..original_waypoints.len() {
            waypoints.push(original_waypoints[idx]);
            layers.push(
                original_layers
                    .get(idx)
                    .copied()
                    .unwrap_or(PathfindLayerEnum::Ground),
            );
            can_optimize.push(true);
        }

        // Optimize patched path
        let optimized = self.optimize_path(&waypoints, &layers, &request);
        let opt_len = optimized.0.len();
        self.is_tunneling = false;
        self.note_open_closed_cells(CELL_LIMIT as i32 / 10, 0);
        self.clean_open_and_closed_lists();

        PathResult {
            success: !optimized.0.is_empty(),
            waypoints: optimized.0,
            layers: optimized.1,
            can_optimize: vec![true; opt_len],
            total_cost: 0,
            blocked_by_ally: blocked,
        }
    }

    /// C++ `Pathfinder::getMoveAwayFromPath` (AIPathfind.cpp:10180-10340).
    ///
    /// A* from unit feet until a cell whose clearance box does not overlap the
    /// avoided path segments (and is not the start cell). Returns full path via
    /// buildActualPath-equivalent `find_path` to that cell.
    pub fn get_move_away_from_path(
        &mut self,
        from: &Coord3D,
        path_to_avoid: &[Coord3D],
        path_to_avoid2: Option<&[Coord3D]>,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        other_radius: f32,
    ) -> Option<Coord3D> {
        let path = self.get_move_away_from_path_result(
            from,
            path_to_avoid,
            path_to_avoid2,
            surfaces,
            is_crusher,
            unit_radius,
            other_radius,
            INVALID_ID,
            true,
        );
        if path.success {
            path.waypoints.last().copied()
        } else {
            None
        }
    }

    /// Full C++ `getMoveAwayFromPath` returning `PathResult` (waypoints + cost).
    pub fn get_move_away_from_path_result(
        &mut self,
        from: &Coord3D,
        path_to_avoid: &[Coord3D],
        path_to_avoid2: Option<&[Coord3D]>,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        other_radius: f32,
        obj_id: ObjectID,
        is_human: bool,
    ) -> PathResult {
        if !self.is_map_ready {
            return PathResult::none();
        }
        if let Ok(mut zones) = self.zones.lock() {
            zones.set_all_passable();
        }

        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let (other_r, other_center) = Self::compute_radius_and_center(other_radius);
        let start = Self::cell_for_unit_position(from, center_in_cell);
        if !self.is_valid_coord(start) {
            return PathResult::none();
        }

        // C++ tunneling when current cell invalid movement or enemyFixed.
        self.is_tunneling = false;
        {
            let Ok(pf) = self.pathfinder.lock() else {
                return PathResult::none();
            };
            if !pf.is_passable(start, surfaces, is_crusher) {
                self.is_tunneling = true;
            }
        }
        if obj_id != INVALID_ID {
            let mut info = CheckMovementInfo {
                cell: start,
                layer: PathfindLayerEnum::Ground,
                center_in_cell,
                radius,
                consider_transient: false,
                acceptable_surfaces: surfaces,
                ..Default::default()
            };
            if !self.check_for_movement(obj_id, &mut info) || info.enemy_fixed {
                self.is_tunneling = true;
            }
        }

        let mut box_half = radius as f32 * PATHFIND_CELL_SIZE_F - (PATHFIND_CELL_SIZE_F / 4.0);
        if center_in_cell {
            box_half += PATHFIND_CELL_SIZE_F / 2.0;
        }
        box_half += other_r as f32 * PATHFIND_CELL_SIZE_F;
        if other_center {
            box_half += PATHFIND_CELL_SIZE_F / 2.0;
        }

        // A* open list (lowest cost first) matching C++ examineNeighboringCells expansion.
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
        // (f_cost, g_cost, cell)
        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> =
            std::collections::BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);
        self.note_open_closed_cells(1, 0);

        let mut found: Option<(GridCoord, Coord3D)> = None;
        let mut expanded = 0i32;
        const MAX_EXPAND: i32 = 2500;

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            let cell = GridCoord::new(cx, cy);
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            expanded += 1;
            if expanded > MAX_EXPAND {
                break;
            }

            let mut center = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell(
                cell.x,
                cell.y,
                center_in_cell,
                &mut center,
                PathfindLayerEnum::Ground,
            );
            let lo_x = center.x - box_half;
            let lo_y = center.y - box_half;
            let hi_x = center.x + box_half;
            let hi_y = center.y + box_half;

            let mut overlap = false;
            // C++: must move at least one cell from start.
            if cell.x == start.x && cell.y == start.y {
                overlap = true;
            }
            let check_path = |path: &[Coord3D]| -> bool {
                for w in path.windows(2) {
                    let s = Coord2D::new(w[0].x, w[0].y);
                    let e = Coord2D::new(w[1].x, w[1].y);
                    if Self::line_in_region(&s, &e, lo_x, lo_y, hi_x, hi_y) {
                        return true;
                    }
                }
                false
            };
            if !overlap && check_path(path_to_avoid) {
                overlap = true;
            }
            if !overlap {
                if let Some(p2) = path_to_avoid2 {
                    if check_path(p2) {
                        overlap = true;
                    }
                }
            }

            if !overlap
                && self.is_destination_valid(
                    cell,
                    PathfindLayerEnum::Ground,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    None,
                )
            {
                // Human clamp like C++ examineNeighboringCells isHuman path.
                if is_human && !self.in_logical_extent(cell) {
                    // not a valid final goal for humans
                } else {
                    found = Some((cell, center));
                    break;
                }
            }

            // Expand neighbors (C++ examineNeighboringCells orthogonal+diagonal).
            // C++ checkChangeLayers: enqueue connect-layer same-xy at parent cost.
            if let Some(link) = self.check_change_layers(cell) {
                if !closed.contains(&(link.x, link.y)) {
                    let key = (link.x, link.y);
                    if !g_score.get(&key).is_some_and(|&og| g >= og) {
                        g_score.insert(key, g);
                        if link.x != cx || link.y != cy {
                            came_from.insert(key, (cx, cy));
                        }
                        open.push(std::cmp::Reverse((g, g, link.x, link.y)));
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
                // C++ 6181-6185: one open orthogonal neighborFlag is enough.
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
                let step = if i >= 4 { 14 } else { 10 }; // diagonal ~1.4
                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                came_from.insert(key, (cx, cy));
                // No goal heuristic — pure Dijkstra like C++ startPathfind(NULL).
                open.push(std::cmp::Reverse((ng, ng, nx, ny)));
            }
        }

        self.note_open_closed_cells(open.len() as i32, closed.len() as i32);
        self.clean_open_and_closed_lists();
        self.is_tunneling = false;

        let Some((_goal_cell, goal_pos)) = found else {
            return PathResult::none();
        };

        // C++ buildActualPath from unit position to goal cell.
        let req = PathRequest {
            object_id: obj_id,
            from: *from,
            to: goal_pos,
            surfaces,
            is_crusher,
            unit_radius,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human,
        };
        let result = self.find_path(req);
        if result.success {
            result
        } else {
            // Fallback: two-node path feet → goal (still better than bare coord).
            PathResult {
                success: true,
                waypoints: vec![*from, goal_pos],
                layers: vec![PathfindLayerEnum::Ground, PathfindLayerEnum::Ground],
                can_optimize: vec![true, true],
                total_cost: g_score
                    .get(&(_goal_cell.x, _goal_cell.y))
                    .copied()
                    .unwrap_or(0) as u32,
                blocked_by_ally: false,
            }
        }
    }

    /// C++ `Pathfinder::crc` (AIPathfind.cpp:11043-11082).
    pub fn crc(&self, xfer: &mut dyn Xfer) {
        // m_extent as two ICoord2D (lo, hi) — C++ xferUser sizeof(IRegion2D)
        let mut lo_x = self.extent_lo.x;
        let mut lo_y = self.extent_lo.y;
        let mut hi_x = self.extent_hi.x;
        let mut hi_y = self.extent_hi.y;
        let _ = xfer.xfer_int(&mut lo_x);
        let _ = xfer.xfer_int(&mut lo_y);
        let _ = xfer.xfer_int(&mut hi_x);
        let _ = xfer.xfer_int(&mut hi_y);

        let mut map_ready = self.is_map_ready;
        let _ = xfer.xfer_bool(&mut map_ready);
        let mut tunneling = self.is_tunneling;
        let _ = xfer.xfer_bool(&mut tunneling);

        let mut obsolete1: i32 = 0;
        let _ = xfer.xfer_int(&mut obsolete1);

        let mut ignore = self.ignore_obstacle_id;
        let _ = xfer.xfer_object_id(&mut ignore);

        // m_queuedPathfindRequests full ring + head/tail
        if let Ok(oq) = self.object_path_queue.lock() {
            for slot in oq.slots.iter() {
                let mut id = *slot;
                let _ = xfer.xfer_object_id(&mut id);
            }
            let mut head = oq.head as i32;
            let mut tail = oq.tail as i32;
            let _ = xfer.xfer_int(&mut head);
            let _ = xfer.xfer_int(&mut tail);
        } else {
            for _ in 0..PATHFIND_QUEUE_LEN {
                let mut id = INVALID_ID;
                let _ = xfer.xfer_object_id(&mut id);
            }
            let mut z = 0i32;
            let _ = xfer.xfer_int(&mut z);
            let _ = xfer.xfer_int(&mut z);
        }

        let mut num_wall = self.wall_pieces.len() as i32;
        let _ = xfer.xfer_int(&mut num_wall);
        for i in 0..MAX_WALL_PIECES {
            let mut id = self.wall_pieces.get(i).copied().unwrap_or(INVALID_ID);
            let _ = xfer.xfer_object_id(&mut id);
        }

        let mut wall_h = self.wall_height;
        let _ = xfer.xfer_real(&mut wall_h);
        let mut cells = self.cumulative_cells_allocated();
        let _ = xfer.xfer_int(&mut cells);
        self.cumulative_cells_allocated
            .store(cells, Ordering::Relaxed);
    }

    /// C++ `Pathfinder::xfer` — version only (AIPathfind.cpp:11085-11093).
    pub fn xfer(&mut self, xfer: &mut dyn Xfer) {
        let mut version: u8 = 1;
        let _ = xfer.xfer_version(&mut version, 1);
    }

    /// C++ `Pathfinder::loadPostProcess` — empty.
    pub fn load_post_process(&mut self) {}

    /// C++ `Pathfinder::moveAllies` (AIPathfind.cpp:10088-10164).
    ///
    /// Walk path nodes reverse; nudge idle allied units blocking the path.
    /// Returns true if any ally was asked to move.
    pub fn move_allies(
        &mut self,
        obj_id: ObjectID,
        path_waypoints: &[Coord3D],
        path_layers: &[PathfindLayerEnum],
        blocked_by_ally: bool,
        unit_radius: f32,
    ) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        if obj_id == INVALID_ID || path_waypoints.len() < 2 {
            return false;
        }
        let Some((is_dozer, is_harvester, is_infantry, ignore_id)) =
            OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
                let mut ignore_id = INVALID_ID;
                if let Some(ai) = obj_guard.get_ai_update_interface() {
                    if let Ok(ai_g) = ai.lock() {
                        ignore_id = ai_g.get_ignored_obstacle_id();
                    }
                }
                (
                    obj_guard.is_kind_of(KindOf::Dozer),
                    obj_guard.is_kind_of(KindOf::Harvester),
                    obj_guard.is_kind_of(KindOf::Infantry),
                    ignore_id,
                )
            })
        else {
            return false;
        };
        if !is_dozer && !is_harvester && !blocked_by_ally {
            return false;
        }
        if self.move_allies_depth > 2 {
            return false;
        }
        self.move_allies_depth += 1;
        let result = (|| {
            let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
            let mut num_above = radius;
            if center_in_cell {
                num_above += 1;
            };
            let mut moved_any = false;
            // C++: for node = last; node && node != first; node = previous
            if path_waypoints.len() < 2 {
                return false;
            }
            for idx in (1..path_waypoints.len()).rev() {
                let pos = &path_waypoints[idx];
                let layer = path_layers
                    .get(idx)
                    .copied()
                    .unwrap_or(PathfindLayerEnum::Ground);
                let cur = GridCoord::from_world(pos);
                for i in (cur.x - radius)..(cur.x + num_above) {
                    for j in (cur.y - radius)..(cur.y + num_above) {
                        let cell = GridCoord::new(i, j);
                        if !self.is_valid_coord(cell) {
                            continue;
                        }
                        // C++ PathfindCell::getPosUnit() — standing occupancy, not goal claim.
                        let pos_unit = {
                            let Ok(goals) = self.goal_cells.lock() else {
                                continue;
                            };
                            goals
                                .get(i as usize)
                                .and_then(|row| row.get(j as usize))
                                .map(|gc| gc.get_pos_unit(layer))
                                .unwrap_or(INVALID_ID)
                        };
                        if pos_unit == INVALID_ID || pos_unit == obj_id || pos_unit == ignore_id {
                            continue;
                        }
                        let Some(other_ai) = OBJECT_REGISTRY
                            .with_object(pos_unit, |other_guard| {
                                let is_ally = OBJECT_REGISTRY
                                    .with_object(obj_id, |obj_guard| {
                                        obj_guard.relationship_to(&other_guard)
                                            == Relationship::Allies
                                    })
                                    .unwrap_or(false);
                                if !is_ally {
                                    return None;
                                }
                                let other_infantry = other_guard.is_kind_of(KindOf::Infantry);
                                if is_infantry && other_infantry {
                                    return None;
                                }
                                if is_infantry && !other_infantry && !blocked_by_ally {
                                    return None;
                                }
                                let other_ai = other_guard.get_ai_update_interface()?;
                                {
                                    let Ok(ai_g) = other_ai.lock() else {
                                        return None;
                                    };
                                    // C++: skip if moving; also skip attacking / busy / ability.
                                    if ai_g.is_moving() {
                                        return None;
                                    }
                                    if ai_g.is_attacking() || ai_g.is_busy() {
                                        return None;
                                    }
                                }
                                if other_guard.test_status(ObjectStatusTypes::IsUsingAbility) {
                                    return None;
                                }
                                Some(other_ai)
                            })
                            .flatten()
                        else {
                            continue;
                        };
                        use crate::modules::AIUpdateInterfaceExt;
                        other_ai.ai_move_away_from_unit(
                            obj_id,
                            crate::common::CommandSourceType::FromAi,
                        );
                        moved_any = true;
                    }
                }
            }
            let _ = moved_any;
            // C++ returns true after scanning the path (even if no ally moved).
            true
        })();
        self.move_allies_depth -= 1;
        result
    }

    pub fn classify_map(&mut self) {
        let pathfinder = self.pathfinder.lock().unwrap();
        let w = pathfinder.width();
        let h = pathfinder.height();
        drop(pathfinder);

        for x in 0..w {
            for y in 0..h {
                self.classify_map_cell(x as i32, y as i32);
            }
        }
        self.expand_cliff_cells_like_cpp();

        // Recalculate zones after full classification
        if let Ok(mut zones) = self.zones.lock() {
            zones.calculate_zones();
        }
    }

    pub(crate) fn expand_cliff_cells_like_cpp(&self) {
        let Ok(mut pathfinder) = self.pathfinder.lock() else {
            return;
        };
        let w = pathfinder.width() as i32;
        let h = pathfinder.height() as i32;

        let mut first_ring = Vec::new();
        for x in 0..w {
            for y in 0..h {
                let coord = GridCoord::new(x, y);
                if pathfinder.get_cell_type(coord) != Some(PathfindCellType::Cliff) {
                    continue;
                }
                for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                    for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
                        let neighbor = GridCoord::new(nx, ny);
                        if pathfinder.get_cell_type(neighbor) == Some(PathfindCellType::Clear) {
                            first_ring.push(neighbor);
                        }
                    }
                }
            }
        }

        for coord in &first_ring {
            pathfinder.set_pinched(*coord, true);
        }
        for coord in first_ring {
            if pathfinder.get_cell_type(coord) == Some(PathfindCellType::Clear) {
                pathfinder.set_cell_type(coord, PathfindCellType::Cliff);
            }
        }

        let mut second_ring = Vec::new();
        for x in 0..w {
            for y in 0..h {
                let coord = GridCoord::new(x, y);
                if pathfinder.get_cell_type(coord) != Some(PathfindCellType::Cliff) {
                    continue;
                }
                for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                    for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
                        let neighbor = GridCoord::new(nx, ny);
                        if pathfinder.get_cell_type(neighbor) == Some(PathfindCellType::Clear) {
                            second_ring.push(neighbor);
                        }
                    }
                }
            }
        }

        for coord in second_ring {
            pathfinder.set_pinched(coord, true);
        }
    }

    /// Classify a single map cell based on terrain data.
    /// Matches C++ Pathfinder::classifyMapCell() at AIPathfind.cpp:4485.
    ///
    /// Sets cell type to Clear/Cliff/Water while preserving existing obstacles.
    pub fn classify_map_cell(&self, x: i32, y: i32) {
        if x < 0 || y < 0 {
            return;
        }
        let coord = GridCoord::new(x, y);
        let top_left_x = x as f32 * PATHFIND_CELL_SIZE_F;
        let top_left_y = y as f32 * PATHFIND_CELL_SIZE_F;
        let bottom_right_x = top_left_x + PATHFIND_CELL_SIZE_F;
        let bottom_right_y = top_left_y + PATHFIND_CELL_SIZE_F;

        let has_obstacle = self
            .pathfinder
            .lock()
            .ok()
            .and_then(|pathfinder| pathfinder.get_cell_type(coord))
            == Some(PathfindCellType::Obstacle);

        let mut cell_type = PathfindCellType::Clear;
        if let Some(terrain) = TheTerrainLogic::get() {
            if terrain.is_cliff_cell(top_left_x, top_left_y) {
                cell_type = PathfindCellType::Cliff;
            }

            if terrain.is_underwater(top_left_x, top_left_y, None, None)
                || terrain.is_underwater(top_left_x, bottom_right_y, None, None)
                || terrain.is_underwater(bottom_right_x, bottom_right_y, None, None)
                || terrain.is_underwater(bottom_right_x, top_left_y, None, None)
            {
                cell_type = PathfindCellType::Water;
            }
        }
        if has_obstacle {
            cell_type = PathfindCellType::Obstacle;
        }

        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.set_cell_type(coord, cell_type);
        }
    }

    /// Mark/remove an object's footprint cells as obstacles.
    /// Matches C++ `Pathfinder::classifyObjectFootprint` (AIPathfind.cpp:4093+).
    pub fn classify_object_footprint(&mut self, obj: &crate::object::Object) {
        self.classify_object_footprint_ex(obj, true);
    }

    /// C++ `classifyObjectFootprint(obj, insert)`.
    pub fn classify_object_footprint_ex(&mut self, obj: &crate::object::Object, insert: bool) {
        use crate::common::KindOf;

        if obj.is_kind_of(KindOf::Mine)
            || obj.is_kind_of(KindOf::Projectile)
            || obj.is_kind_of(KindOf::BridgeTower)
        {
            return;
        }

        let fence_width = obj.get_template().get_fence_width();
        if fence_width > 0.0 && !obj.is_kind_of(KindOf::DefensiveWall) {
            self.classify_fence(obj, insert, fence_width);
            return;
        }

        if !insert {
            // C++ permanent blast crater footprints never remove.
            if obj.is_kind_of(KindOf::BlastCrater) {
                return;
            }
            self.remove_object_footprint(obj);
            return;
        }

        if !obj.is_kind_of(KindOf::Structure) {
            return;
        }
        if obj.is_mobile() {
            return;
        }
        let geo = obj.get_geometry_info();
        if geo.get_is_small() {
            return;
        }
        if obj.get_height_above_terrain() > PATHFIND_CELL_SIZE_F
            && !obj.is_kind_of(KindOf::BlastCrater)
        {
            return;
        }

        self.internal_classify_object_footprint(obj, true);
    }

    /// C++ `Pathfinder::classifyFence` (AIPathfind.cpp:3983+).
    pub(crate) fn classify_fence(
        &mut self,
        obj: &crate::object::Object,
        insert: bool,
        fence_width: f32,
    ) {
        let pos = obj.get_position();
        let angle = obj.get_orientation();
        let halfsize_x = fence_width * 0.5;
        let halfsize_y = PATHFIND_CELL_SIZE_F / 10.0;
        let fence_offset = obj.get_template().get_fence_x_offset();
        let (s, c) = angle.sin_cos();
        const STEP_SIZE: f32 = PATHFIND_CELL_SIZE_F * 0.5;
        let ydx = s * STEP_SIZE;
        let ydy = -c * STEP_SIZE;
        let xdx = c * STEP_SIZE;
        let xdy = s * STEP_SIZE;
        let num_steps_x = ((2.0 * halfsize_x / STEP_SIZE).ceil() as i32).max(1);
        let num_steps_y = ((2.0 * halfsize_y / STEP_SIZE).ceil() as i32).max(1);
        let mut tl_x = pos.x - fence_offset * c - halfsize_y * s;
        let mut tl_y = pos.y + halfsize_y * c - fence_offset * s;
        let obj_id = obj.get_id();
        let mut lo_x = i32::MAX;
        let mut lo_y = i32::MAX;
        let mut hi_x = i32::MIN;
        let mut hi_y = i32::MIN;
        let mut did = false;

        for _iy in 0..num_steps_y {
            let mut x = tl_x;
            let mut y = tl_y;
            for _ix in 0..num_steps_x {
                let cx = ((x + 0.5) / PATHFIND_CELL_SIZE_F).floor() as i32;
                let cy = ((y + 0.5) / PATHFIND_CELL_SIZE_F).floor() as i32;
                if cx >= 0 && cy >= 0 && (cx as usize) < self.width && (cy as usize) < self.height {
                    if self.set_or_clear_obstacle_cell(cx, cy, obj_id, true, insert) {
                        did = true;
                    }
                    lo_x = lo_x.min(cx);
                    lo_y = lo_y.min(cy);
                    hi_x = hi_x.max(cx);
                    hi_y = hi_y.max(cy);
                }
                x += xdx;
                y += xdy;
            }
            tl_x += ydx;
            tl_y += ydy;
        }

        if did {
            if let Ok(mut zones) = self.zones.lock() {
                zones.mark_zones_dirty(insert);
            }
            self.refresh_pinched_bounds(lo_x, lo_y, hi_x, hi_y);
        }
    }

    /// C++ `internal_classifyObjectFootprint` box/cylinder raster.
    pub(crate) fn internal_classify_object_footprint(
        &mut self,
        obj: &crate::object::Object,
        insert: bool,
    ) {
        let pos = obj.get_position();
        let geo = obj.get_geometry_info();
        let obj_id = obj.get_id();
        let mut lo_x = i32::MAX;
        let mut lo_y = i32::MAX;
        let mut hi_x = i32::MIN;
        let mut hi_y = i32::MIN;
        let mut did = false;

        match geo.get_geometry_type() {
            game_engine::system::geometry::GeometryType::Box => {
                let angle = obj.get_orientation();
                let halfsize_x = geo.get_major_radius();
                let halfsize_y = geo.get_minor_radius();
                let (s, c) = angle.sin_cos();
                const STEP_SIZE: f32 = PATHFIND_CELL_SIZE_F * 0.5;
                let ydx = s * STEP_SIZE;
                let ydy = -c * STEP_SIZE;
                let xdx = c * STEP_SIZE;
                let xdy = s * STEP_SIZE;
                let num_steps_x = ((2.0 * halfsize_x / STEP_SIZE).ceil() as i32).max(1);
                let num_steps_y = ((2.0 * halfsize_y / STEP_SIZE).ceil() as i32).max(1);
                let mut tl_x = pos.x - halfsize_x * c - halfsize_y * s;
                let mut tl_y = pos.y + halfsize_y * c - halfsize_x * s;
                for _iy in 0..num_steps_y {
                    let mut x = tl_x;
                    let mut y = tl_y;
                    for _ix in 0..num_steps_x {
                        let cx = ((x + 0.5) / PATHFIND_CELL_SIZE_F).floor() as i32;
                        let cy = ((y + 0.5) / PATHFIND_CELL_SIZE_F).floor() as i32;
                        if cx >= 0
                            && cy >= 0
                            && (cx as usize) < self.width
                            && (cy as usize) < self.height
                        {
                            if self.set_or_clear_obstacle_cell(cx, cy, obj_id, false, insert) {
                                did = true;
                            }
                            lo_x = lo_x.min(cx);
                            lo_y = lo_y.min(cy);
                            hi_x = hi_x.max(cx);
                            hi_y = hi_y.max(cy);
                        }
                        x += xdx;
                        y += xdy;
                    }
                    tl_x += ydx;
                    tl_y += ydy;
                }
            }
            game_engine::system::geometry::GeometryType::Sphere
            | game_engine::system::geometry::GeometryType::Cylinder => {
                let radius = geo.get_major_radius();
                let center = GridCoord::from_world(pos);
                let radius_cells = (radius / PATHFIND_CELL_SIZE_F).ceil() as i32 + 1;
                let effective_radius = radius + PATHFIND_CELL_SIZE_F * 0.4;
                let eff2 = effective_radius * effective_radius;
                for dy in -radius_cells..=radius_cells {
                    for dx in -radius_cells..=radius_cells {
                        let cx = center.x + dx;
                        let cy = center.y + dy;
                        if cx < 0
                            || cy < 0
                            || (cx as usize) >= self.width
                            || (cy as usize) >= self.height
                        {
                            continue;
                        }
                        let cell_center =
                            GridCoord::new(cx, cy).to_world(PathfindLayerEnum::Ground);
                        let ddx = cell_center.x - pos.x;
                        let ddy = cell_center.y - pos.y;
                        if ddx * ddx + ddy * ddy > eff2 {
                            continue;
                        }
                        if self.set_or_clear_obstacle_cell(cx, cy, obj_id, false, insert) {
                            did = true;
                        }
                        lo_x = lo_x.min(cx);
                        lo_y = lo_y.min(cy);
                        hi_x = hi_x.max(cx);
                        hi_y = hi_y.max(cy);
                    }
                }
            }
        }

        if did {
            if let Ok(mut zones) = self.zones.lock() {
                zones.mark_zones_dirty(insert);
            }
            self.refresh_pinched_bounds(lo_x, lo_y, hi_x, hi_y);
        }
    }

    pub(crate) fn remove_object_footprint(&mut self, obj: &crate::object::Object) {
        // Re-raster with insert=false using geometry (and fence path).
        let fence_width = obj.get_template().get_fence_width();
        if fence_width > 0.0 && !obj.is_kind_of(crate::common::KindOf::DefensiveWall) {
            self.classify_fence(obj, false, fence_width);
            return;
        }
        if obj.is_kind_of(crate::common::KindOf::Structure) && !obj.is_mobile() {
            let geo = obj.get_geometry_info();
            if !geo.get_is_small() {
                self.internal_classify_object_footprint(obj, false);
            }
        }
    }

    pub(crate) fn set_or_clear_obstacle_cell(
        &self,
        cx: i32,
        cy: i32,
        obj_id: ObjectID,
        is_fence: bool,
        insert: bool,
    ) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let coord = GridCoord::new(cx, cy);
        // C++ m_obstacleIsTransparent from KINDOF_CAN_SEE_THROUGH_STRUCTURE.
        let is_transparent = OBJECT_REGISTRY
            .with_object(obj_id, |g| g.is_kind_of(KindOf::CanSeeThrough))
            .unwrap_or(false);
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            if insert {
                pathfinder.set_cell_type(coord, PathfindCellType::Obstacle);
                pathfinder.set_cell_obstacle_id(coord, obj_id, is_fence, is_transparent);
                true
            } else if pathfinder.clear_cell_obstacle_id(coord, obj_id) {
                true
            } else {
                false
            }
        } else {
            false
        }
    }

    pub(crate) fn refresh_pinched_bounds(&self, lo_x: i32, lo_y: i32, hi_x: i32, hi_y: i32) {
        if lo_x == i32::MAX {
            return;
        }
        let lo = GridCoord::new((lo_x - 2).max(0), (lo_y - 2).max(0));
        let hi = GridCoord::new(
            (hi_x + 2).min(self.width as i32 - 1),
            (hi_y + 2).min(self.height as i32 - 1),
        );
        if let Ok(mut pathfinder) = self.pathfinder.lock() {
            pathfinder.refresh_pinched_cells_in_bounds(lo, hi);
        }
    }
}
