use super::*;

impl PathfindingSystem {
    /// C++ `Pathfinder::findAttackPath` (AIPathfind.cpp:10530+).
    ///
    /// 1) Quick steps toward victim if already in weapon range with LOS.
    /// 2) Else hierarchical connectivity probe + spiral/A* to an in-range cell.
    ///
    /// `in_range(goal)` should implement weapon isGoalPosWithinAttackRange.
    /// `view_blocked(from,goal)` should implement isAttackViewBlockedByObstacle.
    pub fn find_attack_path<F, G>(
        &self,
        from: &Coord3D,
        victim_pos: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        attack_distance: f32,
        obj_id: ObjectID,
        is_human: bool,
        mut in_range: F,
        mut view_blocked: G,
    ) -> PathResult
    where
        F: FnMut(&Coord3D) -> bool,
        G: FnMut(&Coord3D, &Coord3D) -> bool,
    {
        // C++ Pathfinder::findAttackPath (AIPathfind.cpp:10506-10880).
        if !self.is_map_ready {
            return PathResult::none();
        }
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let layer = PathfindLayerEnum::Ground;

        // Quick check: step toward victim (C++ i=1..10, delta * i * 0.5 * cell)
        {
            let mut delta = Coord3D::new(victim_pos.x - from.x, victim_pos.y - from.y, 0.0);
            let len = (delta.x * delta.x + delta.y * delta.y).sqrt();
            if len > f32::EPSILON {
                delta.x = (delta.x / len) * PATHFIND_CELL_SIZE_F;
                delta.y = (delta.y / len) * PATHFIND_CELL_SIZE_F;
                for i in 1..10 {
                    let test = Coord3D::new(
                        from.x + delta.x * i as f32 * 0.5,
                        from.y + delta.y * i as f32 * 0.5,
                        from.z,
                    );
                    let cell = GridCoord::from_world(&test);
                    if !self.is_valid_coord(cell) {
                        break;
                    }
                    {
                        let Ok(pf) = self.pathfinder.lock() else {
                            break;
                        };
                        if !pf.is_passable(cell, surfaces, is_crusher) {
                            break;
                        }
                    }
                    if !self.is_destination_valid(
                        cell,
                        layer,
                        surfaces,
                        is_crusher,
                        radius,
                        center_in_cell,
                        None,
                    ) {
                        break;
                    }
                    if is_human && !self.in_logical_extent(cell) {
                        break;
                    }
                    if in_range(&test) && !view_blocked(from, &test) {
                        return PathResult {
                            success: true,
                            waypoints: vec![*from, test],
                            layers: vec![layer, layer],
                            can_optimize: vec![true, true],
                            total_cost: COST_ORTHOGONAL * i as u32,
                            blocked_by_ally: false,
                        };
                    }
                }
            }
        }

        // Hierarchical connectivity probe (C++ findClosestHierarchicalPath)
        if let Ok(mut zones) = self.zones.lock() {
            zones.clear_passable_flags();
        }
        let h = self.find_closest_hierarchical_path(*from, *victim_pos, surfaces, is_crusher);
        if h.is_none() {
            if let Ok(mut zones) = self.zones.lock() {
                zones.set_all_passable();
            }
        }

        // C++ attackDistance includes +3*PATHFIND_CELL_SIZE already at call sites.
        // Here `attack_distance` is weapon range; match C++ by adding 3 cells for expand budget.
        let attack_dist_cells = ((attack_distance / PATHFIND_CELL_SIZE_F).round() as i32) + 3;
        let attack_dist_cost_units = attack_dist_cells.max(0) as u32 * COST_ORTHOGONAL;

        let start = Self::cell_for_unit_position(from, center_in_cell);
        if !self.is_valid_coord(start) {
            return PathResult::none();
        }
        let victim_cell = GridCoord::from_world(victim_pos);
        let is_vehicle = Self::object_is_vehicle(obj_id);

        // A* open list: (f, g, x, y). Goal is any in-range attack cell, not victim cell.
        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> =
            std::collections::BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);

        const ATTACK_CELL_LIMIT: i32 = 2500;
        let mut cell_count = 0i32;
        let mut found_cell: Option<GridCoord> = None;
        let mut closest_cell: Option<GridCoord> = None;
        let mut closest_dist_sqr = f32::MAX;

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

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            let cell = GridCoord::new(cx, cy);
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));

            let mut cell_center = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell(cx, cy, center_in_cell, &mut cell_center, layer);

            // C++: weapon in range + checkDestination → candidate; reject start cell.
            let dest_ok = self.is_destination_valid(
                cell,
                layer,
                surfaces,
                is_crusher,
                radius,
                center_in_cell,
                None,
            );
            if dest_ok && in_range(&cell_center) {
                let mut blocked = false;
                // Never accept starting cell (C++ viewBlocked = true for start).
                if cell.x == start.x && cell.y == start.y {
                    blocked = true;
                } else {
                    let dx = cell_center.x - from.x;
                    let dy = cell_center.y - from.y;
                    if dx * dx + dy * dy < (PATHFIND_CELL_SIZE_F * 0.5).powi(2) {
                        blocked = true;
                    }
                }
                if !blocked && view_blocked(from, &cell_center) {
                    blocked = true;
                }
                if !blocked {
                    found_cell = Some(cell);
                    break;
                }
            }

            // Track closest valid movement cell to victim (fallback).
            if dest_ok {
                let Ok(pf) = self.pathfinder.lock() else {
                    continue;
                };
                if pf.is_passable(cell, surfaces, is_crusher) {
                    let dx = (victim_cell.x - cx).abs() as f32;
                    let dy = (victim_cell.y - cy).abs() as f32;
                    let d2 = dx * dx + dy * dy;
                    if d2 < closest_dist_sqr {
                        closest_dist_sqr = d2;
                        closest_cell = Some(cell);
                    }
                }
            }

            if cell_count >= ATTACK_CELL_LIMIT {
                continue;
            }
            cell_count += 1;
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

            // Expand neighbors with attackDistance costs (C++ examineNeighboringCells).
            let mut neighbor_flags = [false; 8];
            for (i, &(dx, dy)) in deltas.iter().enumerate() {
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
                    if !pf.is_passable(nc, surfaces, is_crusher) {
                        continue;
                    }
                }
                neighbor_flags[i] = true;

                let mut step = if i >= 4 {
                    COST_DIAGONAL as i32
                } else {
                    COST_ORTHOGONAL as i32
                };

                // Movement occupancy costs
                let mut info = CheckMovementInfo {
                    cell: nc,
                    layer,
                    center_in_cell,
                    radius,
                    consider_transient: false,
                    acceptable_surfaces: surfaces,
                    ..Default::default()
                };
                let move_ok = if obj_id != INVALID_ID {
                    self.check_for_movement(obj_id, &mut info)
                } else {
                    true
                };
                if !move_ok || info.enemy_fixed {
                    continue;
                }
                if info.ally_fixed_count > 0 {
                    step += 3 * COST_DIAGONAL as i32;
                }
                let sdx = (nx - start.x).abs();
                let sdy = (ny - start.y).abs();
                if info.ally_moving && sdx < 10 && sdy < 10 {
                    step += 3 * COST_DIAGONAL as i32;
                }
                // C++ attack path: allyGoal → vehicle 3*ORTHO else ORTHO
                if info.ally_goal {
                    if is_vehicle {
                        step += 3 * COST_ORTHOGONAL as i32;
                    } else {
                        step += COST_ORTHOGONAL as i32;
                    }
                }
                {
                    let Ok(pf) = self.pathfinder.lock() else {
                        continue;
                    };
                    if pf.is_pinched(nc).unwrap_or(false) {
                        step += COST_ORTHOGONAL as i32 + COST_DIAGONAL as i32;
                    }
                    if !pf.is_zone_passable(nc) {
                        step += 100 * COST_ORTHOGONAL as i32;
                    }
                }

                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                came_from.insert(key, (cx, cy));

                // C++ attack heuristic: COST_ORTHO * euclid - attackDistance/2
                let hdx = (nx - victim_cell.x) as f32;
                let hdy = (ny - victim_cell.y) as f32;
                let heu = (COST_ORTHOGONAL as f32) * (hdx * hdx + hdy * hdy).sqrt();
                let mut h_rem = heu - (attack_dist_cost_units as f32) / 2.0;
                if h_rem < 0.0 {
                    h_rem = 0.0;
                }
                let f_cost = ng + h_rem as i32;
                open.push(std::cmp::Reverse((f_cost, ng, nx, ny)));
            }
        }

        let mut goal_cell = found_cell.or(closest_cell);
        let Some(mut goal) = goal_cell.take() else {
            return PathResult::none();
        };

        // C++ vehicle strip-back: walk parent chain; if blocked by ally/enemy, retreat.
        if is_vehicle && obj_id != INVALID_ID {
            let mut chain: Vec<GridCoord> = Vec::new();
            let mut cur = (goal.x, goal.y);
            chain.push(GridCoord::new(cur.0, cur.1));
            while let Some(&p) = came_from.get(&cur) {
                chain.push(GridCoord::new(p.0, p.1));
                cur = p;
                if chain.len() > 64 {
                    break;
                }
            }
            // chain is goal→…→start; walk from goal toward start like C++.
            let mut last_blocked: Option<GridCoord> = None;
            let mut use_large = false;
            let cell_limit = 12usize;
            for (idx, c) in chain.iter().enumerate() {
                if idx >= cell_limit {
                    break;
                }
                let (r_use, cic_use) = if use_large {
                    (radius, center_in_cell)
                } else {
                    (0, true)
                };
                let mut info = CheckMovementInfo {
                    cell: *c,
                    layer,
                    center_in_cell: cic_use,
                    radius: r_use,
                    consider_transient: false,
                    acceptable_surfaces: surfaces,
                    ..Default::default()
                };
                let mut unit_idle = false;
                let pos_unit = self.pos_unit_at(*c, layer);
                if pos_unit != INVALID_ID {
                    unit_idle = Self::object_is_idle(pos_unit);
                }
                let check_movement = self.check_for_movement(obj_id, &mut info);
                let mut blocked_by_allies = info.ally_fixed_count > 0 || info.ally_goal;
                if unit_idle {
                    blocked_by_allies = false;
                }
                if !check_movement || info.enemy_fixed || blocked_by_allies {
                    last_blocked = Some(*c);
                    use_large = true;
                } else {
                    use_large = false;
                }
            }
            if let Some(lb) = last_blocked {
                // Prefer parent of last blocked.
                if let Some(&(px, py)) = came_from.get(&(lb.x, lb.y)) {
                    goal = GridCoord::new(px, py);
                } else {
                    goal = lb;
                }
            }
        }

        let goal_pos = {
            let mut p = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell(goal.x, goal.y, center_in_cell, &mut p, layer);
            p
        };
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
            PathResult {
                success: true,
                waypoints: vec![*from, goal_pos],
                layers: vec![layer, layer],
                can_optimize: vec![true, true],
                total_cost: g_score.get(&(goal.x, goal.y)).copied().unwrap_or(0) as u32,
                blocked_by_ally: false,
            }
        }
    }

    /// C++ `Pathfinder::isAttackViewBlockedByObstacle` (AIPathfind.cpp:9360-9429).
    ///
    /// Returns true when an opaque obstacle cell lies on the Bresenham line from
    /// attacker to victim (after KINDOF / AI global / transparent / self/victim skips).
    pub fn is_attack_view_blocked_by_obstacle(
        &self,
        attacker_id: ObjectID,
        attacker_pos: &Coord3D,
        victim_id: Option<ObjectID>,
        victim_pos: &Coord3D,
    ) -> bool {
        // Grid Bresenham + cell obstacle IDs run even with an empty registry.
        // KINDOF / container / slaver lookups skip when attacker INVALID_ID.

        // Global switch TheAI->getAiData()->m_attackUsesLineOfSight
        let ai_store = crate::ai::the_ai();let los_enabled = ai_store
            .read()
            .ok()
            .and_then(|ai| {
                ai.get_ai_data()
                    .read()
                    .ok()
                    .map(|d| d.attack_uses_line_of_sight)
            })
            .unwrap_or(true);
        if !los_enabled {
            return false;
        }

        // If attacker doesn't need LOS, not blocked.
        if attacker_id != INVALID_ID {
            let early = OBJECT_REGISTRY.with_object(attacker_id, |g| {
                if !g.is_kind_of(KindOf::AttackNeedsLineOfSight) {
                    return Some(false);
                }
                // Flying victim: C++ isViewBlocked early-out for significantly above terrain.
                if let Some(vid) = victim_id {
                    let flying = OBJECT_REGISTRY
                        .with_object(vid, |vg| vg.is_significantly_above_terrain())
                        .unwrap_or(false);
                    if flying {
                        return Some(false);
                    }
                }
                // LOS_TERRAIN: C++ Weapon::isClearGoalFiringLineOfSightTerrain
                // (skip for immobile — cannot path around terrain blockage).
                if !g.is_kind_of(KindOf::Immobile) {
                    if let Some((weapon, _)) = g.get_current_weapon() {
                        let clear = if let Some(vid) = victim_id {
                            weapon.is_clear_goal_firing_line_of_sight_terrain(
                                attacker_id,
                                attacker_pos,
                                vid,
                            )
                        } else {
                            weapon.is_clear_goal_firing_line_of_sight_terrain_pos(
                                attacker_id,
                                attacker_pos,
                                victim_pos,
                            )
                        };
                        if !clear {
                            return Some(true);
                        }
                    } else if let Ok(terrain) = crate::terrain::get_terrain_logic().read() {
                        if !terrain.is_clear_line_of_sight(attacker_pos, victim_pos) {
                            return Some(true);
                        }
                    }
                }
                None
            });
            if let Some(blocked) = early.flatten() {
                return blocked;
            }
        }

        let to_pf_layer = |l: CommonPathfindLayerEnum| -> PathfindLayerEnum {
            let v = l as u32;
            if (2..=14).contains(&v) || v == 15 {
                PathfindLayerEnum::from_u32(v)
            } else {
                PathfindLayerEnum::Ground
            }
        };
        let mut skip_count = 0i32;
        let mut layer = PathfindLayerEnum::Ground;
        if let Some(vid) = victim_id {
            if let Some(vlayer) = OBJECT_REGISTRY.with_object(vid, |vg| vg.get_layer()) {
                layer = to_pf_layer(vlayer);
            }
        }
        if attacker_id != INVALID_ID {
            if let Some(al) = OBJECT_REGISTRY.with_object(attacker_id, |g| g.get_layer()) {
                if !matches!(
                    al,
                    CommonPathfindLayerEnum::Ground | CommonPathfindLayerEnum::Invalid
                ) {
                    // Magic 3: bridge/rooftop can see 3 cells off structure.
                    skip_count = 3;
                    if layer == PathfindLayerEnum::Ground {
                        layer = to_pf_layer(al);
                    }
                }
            }
        }

        let victim_cell = GridCoord::from_world(victim_pos);
        let victim_obstacle_id = self
            .pathfinder
            .lock()
            .ok()
            .and_then(|pf| pf.get_cell_obstacle_id(victim_cell));

        let attacker_container = Self::object_container_id(attacker_id);
        let attacker_slaver = Self::object_slaver_id(attacker_id);
        let victim_slaver = victim_id.map(Self::object_slaver_id).unwrap_or(INVALID_ID);

        let mut remaining_skip = skip_count;
        let ret = self.iterate_cells_along_line_world(
            attacker_pos,
            victim_pos,
            layer,
            |_from, to, _x, _y| {
                if remaining_skip > 0 {
                    remaining_skip -= 1;
                    return 0;
                }
                let Ok(pf) = self.pathfinder.lock() else {
                    return 0;
                };
                if pf.get_cell_type(to) != Some(PathfindCellType::Obstacle) {
                    return 0;
                }
                let obs_id = pf.get_cell_obstacle_id(to).unwrap_or(INVALID_ID);
                // never block own view
                if attacker_id != INVALID_ID && obs_id == attacker_id {
                    return 0;
                }
                if let Some(vid) = victim_id {
                    if obs_id == vid {
                        return 0;
                    }
                    if victim_slaver != INVALID_ID && obs_id == victim_slaver {
                        return 0;
                    }
                }
                if attacker_container != INVALID_ID && obs_id == attacker_container {
                    return 0;
                }
                if attacker_slaver != INVALID_ID && obs_id == attacker_slaver {
                    return 0;
                }
                if pf.is_obstacle_transparent(to) {
                    return 0;
                }
                // Victim inside another object's footprint — don't block (edge case).
                if let Some(void) = victim_obstacle_id {
                    if obs_id == void {
                        return 0;
                    }
                }
                1 // blocked
            },
        );
        ret != 0
    }

    pub(crate) fn object_container_id(object_id: ObjectID) -> ObjectID {
        // Wave 262: empty dual-world → invalid id.
        if dual_world_registry_unavailable() {
            return INVALID_ID;
        }

        if object_id == INVALID_ID {
            return INVALID_ID;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |g| g.get_contained_by().unwrap_or(INVALID_ID))
            .unwrap_or(INVALID_ID)
    }

    pub(crate) fn object_slaver_id(object_id: ObjectID) -> ObjectID {
        // Wave 262: empty dual-world → invalid id.
        if dual_world_registry_unavailable() {
            return INVALID_ID;
        }

        if object_id == INVALID_ID {
            return INVALID_ID;
        }
        OBJECT_REGISTRY
            .with_object(object_id, |g| {
                // C++ getSlaverID via SlavedUpdate / MobMemberSlavedUpdate.
                g.with_slaved_update_interface(|s| s.slaver_id())
                    .flatten()
                    .unwrap_or(INVALID_ID)
            })
            .unwrap_or(INVALID_ID)
    }

    /// Convenience: find_attack_path with simple 2D circle range and optional LOS.
    pub fn find_attack_path_range(
        &self,
        from: &Coord3D,
        victim_pos: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        attack_range: f32,
        obj_id: ObjectID,
        check_los: bool,
    ) -> PathResult {
        let range_sqr = attack_range * attack_range;
        let victim = *victim_pos;
        // C++: view_blocked applied during candidate selection (not post-filter only).
        // Use ground line passability as the pathfinder LOS probe when check_los.
        self.find_attack_path(
            from,
            victim_pos,
            surfaces,
            is_crusher,
            unit_radius,
            attack_range,
            obj_id,
            true,
            move |goal| {
                let dx = goal.x - victim.x;
                let dy = goal.y - victim.y;
                dx * dx + dy * dy <= range_sqr
            },
            |a, b| {
                if !check_los {
                    return false;
                }
                // C++ isAttackViewBlockedByObstacle from attack cell `a` toward victim `b`.
                // When attacker id known, use full obstacle LOS; else line passability fallback.
                if obj_id != INVALID_ID {
                    self.is_attack_view_blocked_by_obstacle(obj_id, a, None, b)
                } else {
                    !self.is_line_passable_ex(a, b, surfaces, is_crusher, None, false)
                }
            },
        )
    }

    /// C++ `Pathfinder::findSafePath` (AIPathfind.cpp:10885-11040).
    ///
    /// A* from unit feet until a destination is outside both repulsor radii
    /// (or budget exhausted with farthest cell). Builds path via find_path.
    pub fn find_safe_path(
        &self,
        request: PathRequest,
        repulsor_pos1: &Coord3D,
        repulsor_pos2: &Coord3D,
        repulsor_radius: f32,
    ) -> PathResult {
        const MAX_CELLS: i32 = 2000;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;

        if !self.is_map_ready {
            return PathResult::none();
        }
        if let Ok(mut zones) = self.zones.lock() {
            zones.set_all_passable();
        }

        let (radius, center_in_cell) = Self::compute_radius_and_center(request.unit_radius);
        let start = Self::cell_for_unit_position(&request.from, center_in_cell);
        if !self.is_valid_coord(start) {
            return PathResult::none();
        }
        let is_human = request.is_human;
        let surfaces = request.surfaces;
        let is_crusher = request.is_crusher;
        let repulsor_radius_sqr = repulsor_radius * repulsor_radius;

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

        // Dijkstra open list (C++ startPathfind(NULL) — no goal heuristic).
        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> =
            std::collections::BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);

        let mut farthest: Option<(GridCoord, f32)> = None;
        let mut cell_count = 0i32;
        let mut found: Option<(GridCoord, Coord3D)> = None;

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            let cell = GridCoord::new(cx, cy);
            let layer = self.get_layer_for_coord(cell);
            let mut center = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell(cx, cy, center_in_cell, &mut center, layer);

            let d1 = (center.x - repulsor_pos1.x) * (center.x - repulsor_pos1.x)
                + (center.y - repulsor_pos1.y) * (center.y - repulsor_pos1.y);
            let d2 = (center.x - repulsor_pos2.x) * (center.x - repulsor_pos2.x)
                + (center.y - repulsor_pos2.y) * (center.y - repulsor_pos2.y);
            let nearest = d1.min(d2);

            let mut ok = nearest > repulsor_radius_sqr;
            // C++: exhausted open list after expanding → take last cell.
            if open.is_empty() && cell_count > 0 {
                ok = true;
            }
            if farthest.map(|(_, d)| nearest > d).unwrap_or(true) {
                farthest = Some((cell, nearest));
                // C++: if already big search and this is farthest, accept early.
                if cell_count > MAX_CELLS {
                    ok = true;
                }
            }

            if ok
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
                if !(is_human && !self.in_logical_extent(cell)) {
                    found = Some((cell, center));
                    break;
                }
            }

            // put on closed and expand neighbors
            // C++ checkChangeLayers: enqueue connect-layer same-xy at parent cost.
            if let Some(link) = self.check_change_layers(cell) {
                if !closed.contains(&(link.x, link.y)) {
                    let key = (link.x, link.y);
                    if !g_score.get(&key).is_some_and(|&og| g >= og) {
                        g_score.insert(key, g);
                        open.push(std::cmp::Reverse((g, g, link.x, link.y)));
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
                open.push(std::cmp::Reverse((ng, ng, nx, ny)));
                cell_count += 1;
            }
        }

        let goal_pos = if let Some((_, pos)) = found {
            pos
        } else if let Some((cell, _)) = farthest {
            let layer = self.get_layer_for_coord(cell);
            if !self.is_destination_valid(
                cell,
                layer,
                surfaces,
                is_crusher,
                radius,
                center_in_cell,
                request.ignore_obstacle_id,
            ) {
                return PathResult::none();
            }
            let mut center = Coord3D::new(0.0, 0.0, 0.0);
            self.adjust_coord_to_cell(cell.x, cell.y, center_in_cell, &mut center, layer);
            center
        } else {
            return PathResult::none();
        };

        // C++ buildActualPath from unit position to chosen cell.
        let from = request.from;
        let mut req = request;
        req.to = goal_pos;
        let result = self.find_path(req);
        if result.success {
            result
        } else {
            PathResult {
                success: true,
                waypoints: vec![from, goal_pos],
                layers: vec![PathfindLayerEnum::Ground, PathfindLayerEnum::Ground],
                can_optimize: vec![true, true],
                total_cost: 0,
                blocked_by_ally: false,
            }
        }
    }
}
