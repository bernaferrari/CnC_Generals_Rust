use super::*;

impl PathfindingSystem {
    /// C++ classifyObjectFootprint wall remove: DAMAGE_FALLING / DEATH_SPLATTED.
    pub fn splat_units_on_wall_piece(
        &self,
        piece_id: ObjectId,
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<ObjectId> {
        let wall_h = self.grid.wall_height;
        objects
            .values()
            .filter(|obj| {
                if obj.id == piece_id || !obj.is_alive() {
                    return false;
                }
                if obj.is_kind_of(KindOf::Structure) {
                    return false;
                }
                if !self
                    .grid
                    .is_point_on_wall_piece(piece_id.0, obj.get_position())
                {
                    return false;
                }
                // Stand-in for C++ `obj->getLayer() == LAYER_WALL`: unit Y
                // must sit on the wall deck, not the ground footprint.
                (obj.get_position().y - wall_h).abs() <= LAYER_Z_CLOSE_ENOUGH_F && wall_h > 0.0
            })
            .map(|obj| obj.id)
            .collect()
    }

    /// C++ `Pathfinder::findAttackPath` (AIPathfind.cpp:10530+).
    ///
    /// Quick steps toward the victim, then A* whose goal is any in-range
    /// `checkDestination` cell with LOS. Start cell is never accepted.
    /// Closest valid movement cell is the leftover fallback.
    pub fn find_attack_firing_position(
        &mut self,
        from: Vec3,
        victim: Vec3,
        weapon_range: f32,
        objects: &HashMap<ObjectId, Object>,
        is_crusher: bool,
        mover: Option<ObjectId>,
    ) -> Option<Vec<Vec3>> {
        self.ensure_dynamic_obstacles(objects);
        // Leftover find_attack_path(obj_id, surfaces, is_human): bind seeker from the attacker.
        self.bind_seeker_from_mover(objects, mover);
        self.apply_seeker_human_flag();
        let is_crusher = is_crusher || self.seeker_crusher_level > 0;
        if is_crusher && self.seeker_crusher_level == 0 {
            self.seeker_crusher_level = 1;
        }
        self.grid
            .set_query_footprint(self.seeker_path_diameter, is_crusher);
        self.grid.query_seeker_id = self.seeker_id.map(|id| id.0).unwrap_or(0);
        let range = weapon_range.max(self.grid.grid_size());
        let cell_size = self.grid.grid_size();
        let center_in_cell = self.seeker_center_in_cell;
        let start = self.grid.cell_for_unit_position(from, center_in_cell);
        let victim_cell = self.grid.world_to_grid(victim);
        let surfaces = mover
            .and_then(|id| objects.get(&id))
            .map(|o| o.locomotor_surfaces)
            .filter(|&s| s != 0)
            .unwrap_or(SURFACE_GROUND);
        let crusher_level = if is_crusher {
            self.seeker_crusher_level.max(1)
        } else {
            0
        };
        let seeker_player = self.seeker_player;
        let is_human = self.seeker_is_human;
        let layer = PathfindLayerEnum::Ground;
        let attacker_id = self.seeker_id;
        let victim_id = objects
            .values()
            .filter(|o| {
                let p = o.get_position();
                let dx = p.x - victim.x;
                let dz = p.z - victim.z;
                dx * dx + dz * dz <= (cell_size * 2.0).powi(2)
            })
            .min_by(|a, b| {
                let da = {
                    let p = a.get_position();
                    let dx = p.x - victim.x;
                    let dz = p.z - victim.z;
                    dx * dx + dz * dz
                };
                let db = {
                    let p = b.get_position();
                    let dx = p.x - victim.x;
                    let dz = p.z - victim.z;
                    dx * dx + dz * dz
                };
                da.partial_cmp(&db).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|o| o.id);
        let is_vehicle = attacker_id
            .and_then(|id| objects.get(&id))
            .is_some_and(|o| o.is_kind_of(KindOf::Vehicle));
        let ally_mask = seeker_player
            .map(|p| self.grid.ally_mask_for(p))
            .unwrap_or(0);
        let seeker_inf = self.seeker_is_infantry;

        // Quick check: step toward victim (C++ i=1..10, delta * i * 0.5 * cell).
        {
            let mut delta = Vec3::new(victim.x - from.x, 0.0, victim.z - from.z);
            let len = (delta.x * delta.x + delta.z * delta.z).sqrt();
            if len > f32::EPSILON {
                delta = delta / len * cell_size;
                for i in 1..10 {
                    let test = from + delta * (i as f32 * 0.5);
                    let cell = self.grid.world_to_grid(test);
                    if !self.grid.is_valid_pos(cell)
                        || !self.grid.cell_passable_for(cell, surfaces, is_crusher)
                    {
                        break;
                    }
                    if !self.grid.destination_cell_ok(
                        cell,
                        surfaces,
                        is_crusher,
                        seeker_player,
                        crusher_level,
                        layer,
                    ) {
                        break;
                    }
                    if !self.grid.human_extent_allows(cell, is_human) {
                        break;
                    }
                    let dist = {
                        let dx = test.x - victim.x;
                        let dz = test.z - victim.z;
                        (dx * dx + dz * dz).sqrt()
                    };
                    if dist <= range
                        && !self.is_attack_view_blocked_for(
                            test,
                            victim,
                            attacker_id.and_then(|id| objects.get(&id)),
                            victim_id.and_then(|id| objects.get(&id)),
                        )
                    {
                        let path = self.find_path_ex_surfaces(
                            from,
                            test,
                            objects,
                            false,
                            surfaces,
                            is_crusher,
                            attacker_id,
                        );
                        self.grid.set_query_is_human(false);
                        return path;
                    }
                }
            }
        }

        // Hierarchical connectivity probe (C++ findClosestHierarchicalPath).
        self.sync_crate_astar();
        let attack_jumps = self.hierarchical_bridge_jumps();
        let attack_start = self.host_to_crate_coord(start);
        let attack_victim = self.host_to_crate_coord(victim_cell);
        if let Some(crate_pf) = self.crate_astar.as_mut() {
            crate_pf.finder.apply_hierarchical_zone_prune(
                attack_start,
                attack_victim,
                surfaces,
                is_crusher,
                &attack_jumps,
            );
        }

        const ATTACK_CELL_LIMIT: i32 = 2500;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let attack_dist_cells = ((range / cell_size).round() as i32) + 3;
        let attack_dist_cost_units = attack_dist_cells.max(0) * COST_ORTHO;
        if !self.grid.is_valid_pos(start) {
            self.grid.set_query_is_human(false);
            return None;
        }

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
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        open.push(std::cmp::Reverse((0, 0, start.x, start.y)));
        g_score.insert((start.x, start.y), 0);

        let mut cell_count = 0i32;
        let mut found_cell: Option<GridPos> = None;
        let mut closest_cell: Option<GridPos> = None;
        let mut closest_dist_sqr = f32::MAX;
        let half_cell_sqr = (cell_size * 0.5).powi(2);

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            let cell = GridPos::new(cx, cy);
            let world = self
                .grid
                .adjust_coord_to_cell_on_layer(cell, center_in_cell, layer);
            let dest_ok = self.grid.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            );
            if dest_ok {
                let dx = world.x - victim.x;
                let dz = world.z - victim.z;
                let in_range = dx * dx + dz * dz <= range * range;
                if in_range {
                    let mut blocked = cell == start;
                    if !blocked {
                        let sdx = world.x - from.x;
                        let sdz = world.z - from.z;
                        if sdx * sdx + sdz * sdz < half_cell_sqr {
                            blocked = true;
                        }
                    }
                    if !blocked
                        && self.is_attack_view_blocked_for(
                            world,
                            victim,
                            attacker_id.and_then(|id| objects.get(&id)),
                            victim_id.and_then(|id| objects.get(&id)),
                        )
                    {
                        blocked = true;
                    }
                    if !blocked {
                        found_cell = Some(cell);
                        break;
                    }
                }
                if self.grid.cell_passable_for(cell, surfaces, is_crusher) {
                    let ddx = (victim_cell.x - cx).abs() as f32;
                    let ddy = (victim_cell.y - cy).abs() as f32;
                    let d2 = ddx * ddx + ddy * ddy;
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
            if let Some(_link) = self.grid.connect_layer_of(cell, layer) {
                let key = (cx, cy);
                if !closed.contains(&key) && !g_score.get(&key).is_some_and(|&og| g >= og) {
                    g_score.insert(key, g);
                    if cx != start.x || cy != start.y {
                        came_from.insert(key, (cx, cy));
                    }
                    open.push(std::cmp::Reverse((g, g, cx, cy)));
                }
            }
            let mut neighbor_flags = [false; 8];
            for (i, &(dx, dy)) in deltas.iter().enumerate() {
                let nx = cx + dx;
                let ny = cy + dy;
                let nc = GridPos::new(nx, ny);
                if !self.grid.is_valid_pos(nc) || closed.contains(&(nx, ny)) {
                    continue;
                }
                if !self.grid.human_extent_allows(nc, is_human) {
                    continue;
                }
                if PathfindingGrid::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self.grid.cell_passable_for(nc, surfaces, is_crusher)
                    && !(self.grid.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
                }
                neighbor_flags[i] = true;
                let mut step = if i >= 4 { COST_DIAG } else { COST_ORTHO };
                match self.grid.attack_step_occupancy(
                    nc,
                    start,
                    seeker_player,
                    ally_mask,
                    seeker_inf,
                    is_vehicle,
                    crusher_level,
                    layer,
                ) {
                    None => continue,
                    Some(extra) => step += extra,
                }
                if self.grid.is_pinched(nc) {
                    step += COST_ORTHO + COST_DIAG;
                }
                if let Some(crate_pf) = self.crate_astar.as_ref() {
                    if !crate_pf.finder.is_zone_passable(GridCoord::new(nx, ny)) {
                        step += 100 * COST_ORTHO;
                    }
                }
                let ng = g + step;
                let key = (nx, ny);
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                came_from.insert(key, (cx, cy));
                let hdx = (nx - victim_cell.x) as f32;
                let hdy = (ny - victim_cell.y) as f32;
                let heu = (COST_ORTHO as f32) * (hdx * hdx + hdy * hdy).sqrt();
                let h_rem = (heu - (attack_dist_cost_units as f32) / 2.0).max(0.0);
                let f_cost = ng + h_rem as i32;
                open.push(std::cmp::Reverse((f_cost, ng, nx, ny)));
            }
        }

        let Some(mut goal_cell) = found_cell.or(closest_cell) else {
            self.grid.set_query_is_human(false);
            return None;
        };
        // C++ vehicle strip-back: walk parent chain ≤12; last blocked → parent.
        if is_vehicle {
            let mut chain: Vec<GridPos> = Vec::new();
            let mut cur = (goal_cell.x, goal_cell.y);
            chain.push(GridPos::new(cur.0, cur.1));
            while let Some(&p) = came_from.get(&cur) {
                chain.push(GridPos::new(p.0, p.1));
                cur = p;
                if chain.len() > 64 {
                    break;
                }
            }
            let mut last_blocked: Option<GridPos> = None;
            let mut use_large = false;
            let large_r = (self.seeker_path_diameter.max(1) / 2).max(1);
            for (idx, c) in chain.iter().enumerate() {
                if idx >= 12 {
                    break;
                }
                let r = if use_large { large_r } else { 0 };
                let mut fixed = 0u16;
                let mut goal_m = 0u16;
                let mut crush = 0u8;
                for dx in -r..=r {
                    for dy in -r..=r {
                        let n = GridPos::new(c.x + dx, c.y + dy);
                        if !self.grid.is_valid_pos(n) {
                            continue;
                        }
                        let b = self.grid.occupancy_bits(n, layer);
                        fixed |= b.fixed;
                        goal_m |= b.goal;
                        crush = crush.max(b.crushable);
                    }
                }
                let friend = match seeker_player {
                    Some(player) => (1u16 << player.min(15)) | ally_mask,
                    None => 0,
                };
                let enemy_fixed = seeker_player.is_some()
                    && (fixed & !friend) != 0
                    && (crusher_level == 0 || crusher_level <= crush);
                let mut blocked_by_allies = (fixed & friend) != 0 || (goal_m & friend) != 0;
                let occupant = objects.values().find(|o| {
                    let gp = self.grid.world_to_grid(o.get_position());
                    (gp.x - c.x).abs() <= r && (gp.y - c.y).abs() <= r
                });
                let unit_idle = occupant
                    .is_some_and(|o| matches!(o.ai_state, crate::game_logic::AIState::Idle));
                if unit_idle {
                    blocked_by_allies = false;
                }
                let move_ok = self.grid.cell_passable_for(*c, surfaces, is_crusher)
                    || (self.grid.is_obstacle_fence(*c) && is_crusher);
                if !move_ok || enemy_fixed || blocked_by_allies {
                    last_blocked = Some(*c);
                    use_large = true;
                } else {
                    use_large = false;
                }
            }
            if let Some(lb) = last_blocked {
                if let Some(&(px, py)) = came_from.get(&(lb.x, lb.y)) {
                    goal_cell = GridPos::new(px, py);
                } else {
                    goal_cell = lb;
                }
            }
        }
        let goal = self
            .grid
            .adjust_coord_to_cell_on_layer(goal_cell, center_in_cell, layer);
        let path = self.find_path_ex_surfaces(
            from,
            goal,
            objects,
            false,
            surfaces,
            is_crusher,
            attacker_id,
        );
        self.grid.set_query_is_human(false);
        path
    }

    /// C++ `computeNormalRadialOffset` residual (AIPathfind.cpp) on host XZ ground plane.
    pub fn compute_normal_radial_offset_xz(
        from: Vec3,
        to: Vec3,
        obj_pos: Vec3,
        radius: f32,
    ) -> Vec3 {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let obj_dx = obj_pos.x - from.x;
        let obj_dz = obj_pos.z - from.z;
        let cross = dx * obj_dz - dz * obj_dx;
        let (mut nx, mut nz) = if cross > 0.0 { (dz, -dx) } else { (-dz, dx) };
        let len = (nx * nx + nz * nz).sqrt();
        if len > 0.0001 {
            nx /= len;
            nz /= len;
        } else {
            nx = 1.0;
            nz = 0.0;
        }
        Vec3::new(obj_pos.x + nx * radius, obj_pos.y, obj_pos.z + nz * radius)
    }

    /// Leftover `tall_buildings.rs`: bounding circle + 2 pathfind cells.
    pub(super) fn aircraft_path_around_radius(obj: &Object) -> Option<f32> {
        if !obj.is_alive() || !obj.is_kind_of(crate::game_logic::KindOf::AircraftPathAround) {
            return None;
        }
        let geom = obj.thing.template.geometry_info;
        let r = if geom.authored {
            geom.bounding_circle_radius()
        } else {
            obj.selection_radius.max(0.0)
        };
        Some(r + 2.0 * 10.0)
    }

    /// Leftover `find_tall_building_along_segment`: AIRCRAFT_PATH_AROUND only.
    pub(super) fn find_tall_building_along_segment(
        from: Vec3,
        to: Vec3,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<(ObjectId, Vec3, f32)> {
        let dx = to.x - from.x;
        let dz = to.z - from.z;
        let len = (dx * dx + dz * dz).sqrt();
        if len < 0.01 {
            return None;
        }
        let steps = ((len / 5.0).ceil() as i32).clamp(1, 256);
        let mut best: Option<(ObjectId, Vec3, f32, f32)> = None;
        for obj in objects.values() {
            if ignore == Some(obj.id) {
                continue;
            }
            let Some(radius) = Self::aircraft_path_around_radius(obj) else {
                continue;
            };
            let p = obj.get_position();
            let t = (((p.x - from.x) * dx + (p.z - from.z) * dz) / (len * len)).clamp(0.0, 1.0);
            let cx = from.x + dx * t;
            let cz = from.z + dz * t;
            let dist = ((p.x - cx) * (p.x - cx) + (p.z - cz) * (p.z - cz)).sqrt();
            if dist > radius {
                continue;
            }
            match best {
                Some((_, _, _, bt)) if t >= bt => {}
                _ => best = Some((obj.id, p, radius, t)),
            }
        }
        if let Some((id, p, r, _)) = best {
            for i in 0..=steps {
                let t = i as f32 / steps as f32;
                let sx = from.x + dx * t;
                let sz = from.z + dz * t;
                let d = ((p.x - sx) * (p.x - sx) + (p.z - sz) * (p.z - sz)).sqrt();
                if d <= r {
                    return Some((id, p, r));
                }
            }
        }
        None
    }

    /// C++ `Pathfinder::segmentIntersectsTallBuilding` residual (host XZ).
    /// Returns optional nudged `to` plus three insert waypoints.
    pub fn segment_intersects_tall_building(
        from: Vec3,
        mut to: Vec3,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<(Vec3, Vec3, Vec3, Vec3)> {
        let mut from_pos = from;
        let mut to_pos = to;
        for _ in 0..2 {
            let Some((_id, bldg_pos, radius)) =
                Self::find_tall_building_along_segment(from_pos, to_pos, objects, ignore)
            else {
                return None;
            };

            // If to inside radius, push out and retry.
            let mut delta_x = to_pos.x - bldg_pos.x;
            let mut delta_z = to_pos.z - bldg_pos.z;
            let mut len = (delta_x * delta_x + delta_z * delta_z).sqrt();
            if len <= radius * 0.98 {
                if len < 0.1 {
                    delta_x = 1.0;
                    delta_z = 0.0;
                    len = 1.0;
                }
                delta_x = delta_x / len * radius;
                delta_z = delta_z / len * radius;
                to_pos.x = bldg_pos.x + delta_x;
                to_pos.z = bldg_pos.z + delta_z;
                to = to_pos;
                continue;
            }

            // If from inside radius, push from out.
            delta_x = from_pos.x - bldg_pos.x;
            delta_z = from_pos.z - bldg_pos.z;
            len = (delta_x * delta_x + delta_z * delta_z).sqrt();
            if len <= radius * 0.98 {
                if len < 0.1 {
                    delta_x = 1.0;
                    delta_z = 0.0;
                    len = 1.0;
                }
                delta_x = delta_x / len * radius;
                delta_z = delta_z / len * radius;
                from_pos.x = bldg_pos.x + delta_x;
                from_pos.z = bldg_pos.z + delta_z;
            }

            let insert2 = Self::compute_normal_radial_offset_xz(from_pos, to_pos, bldg_pos, radius);
            let insert1 =
                Self::compute_normal_radial_offset_xz(from_pos, insert2, bldg_pos, radius);
            let insert3 = Self::compute_normal_radial_offset_xz(insert2, to_pos, bldg_pos, radius);
            return Some((to, insert1, insert2, insert3));
        }
        None
    }

    /// C++ `AIUpdateInterface::getBuildingToNotPathAround` (Chinook combat-drop).
    pub(super) fn building_to_not_path_around(
        objects: &HashMap<ObjectId, Object>,
        seeker: Option<ObjectId>,
    ) -> Option<ObjectId> {
        let obj = seeker.and_then(|id| objects.get(&id))?;
        let ai = obj.chinook_ai.as_ref()?;
        use crate::game_logic::host_combat_chinook::HostChinookAIState;
        if matches!(
            ai.state,
            HostChinookAIState::MoveToCombatDrop | HostChinookAIState::DoCombatDrop
        ) {
            ai.combat_drop_target.map(ObjectId)
        } else {
            None
        }
    }

    /// C++ aircraft tall-building path detour residual: walk path segments and
    /// insert radial offsets when AIRCRAFT_PATH_AROUND / tall structures clip.
    pub fn detour_path_around_tall_buildings(
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
    ) -> Vec<Vec3> {
        Self::detour_path_around_tall_buildings_ignoring(path, objects, None)
    }

    /// Leftover `get_aircraft_path` segment walk (`limit=20`, insert n1,n2,n3).
    pub fn detour_path_around_tall_buildings_ignoring(
        path: &[Vec3],
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Vec<Vec3> {
        if path.len() < 2 {
            return path.to_vec();
        }
        let mut waypoints = path.to_vec();
        let mut limit = 20i32;
        let mut idx = 0usize;
        while idx + 1 < waypoints.len() && limit >= 0 {
            let cur = waypoints[idx];
            let next = waypoints[idx + 1];
            if let Some((nudged_to, n1, n2, n3)) =
                Self::segment_intersects_tall_building(cur, next, objects, ignore)
            {
                waypoints[idx + 1] = nudged_to;
                waypoints.insert(idx + 1, n3);
                waypoints.insert(idx + 1, n2);
                waypoints.insert(idx + 1, n1);
                idx += 2;
            } else {
                idx += 1;
            }
            limit -= 1;
        }
        waypoints
    }

    pub fn find_path(
        &mut self,
        start: Vec3,
        goal: Vec3,
        objects: &HashMap<ObjectId, Object>,
    ) -> Option<Vec<Vec3>> {
        self.find_path_ex(start, goal, objects, false, None)
    }
    /// Leftover C++ `circleClipsTallBuilding` / `circle_clips_tall_building`:
    /// AIRCRAFT_PATH_AROUND + normal offset.
    pub fn circle_clips_tall_building(
        from: Vec3,
        to: Vec3,
        circle_radius: f32,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) -> Option<Vec3> {
        let mut best: Option<(ObjectId, Vec3, f32, f32)> = None;
        for obj in objects.values() {
            if ignore == Some(obj.id) {
                continue;
            }
            let Some(bldg_r) = Self::aircraft_path_around_radius(obj) else {
                continue;
            };
            let p = obj.get_position();
            let dx = p.x - to.x;
            let dz = p.z - to.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > circle_radius {
                continue;
            }
            match best {
                Some((_, _, _, bd)) if d >= bd => {}
                _ => best = Some((obj.id, p, bldg_r, d)),
            }
        }
        let Some((tall_id, bldg_pos, bldg_r, _)) = best else {
            return None;
        };
        let mut adjust =
            Self::compute_normal_radial_offset_xz(from, to, bldg_pos, circle_radius + bldg_r);
        adjust.y = to.y;
        // Leftover second AIRCRAFT_PATH_AROUND near adjust_to.
        let mut other: Option<(Vec3, f32, f32)> = None;
        for obj in objects.values() {
            if ignore == Some(obj.id) || obj.id == tall_id {
                continue;
            }
            let Some(bldg_r) = Self::aircraft_path_around_radius(obj) else {
                continue;
            };
            let p = obj.get_position();
            let dx = p.x - adjust.x;
            let dz = p.z - adjust.z;
            let d = (dx * dx + dz * dz).sqrt();
            if d > circle_radius {
                continue;
            }
            match other {
                Some((_, _, bd)) if d >= bd => {}
                _ => other = Some((p, bldg_r, d)),
            }
        }
        if let Some((op, or, _)) = other {
            let tmp = adjust;
            adjust = Self::compute_normal_radial_offset_xz(from, tmp, op, circle_radius + or);
            adjust.y = to.y;
        }
        Some(adjust)
    }

    /// Leftover XY + Z-up from host XZ + Y-up.
    pub(super) fn leftover_coord_from_host(v: Vec3) -> gamelogic::common::Coord3D {
        gamelogic::common::Coord3D::new(v.x, v.z, v.y)
    }

    pub(super) fn leftover_host_from_coord(c: gamelogic::common::Coord3D) -> Vec3 {
        Vec3::new(c.x, c.z, c.y)
    }

    /// Leftover `TerrainLogic::get_maximum_pathfind_extent` is ready (not 0..0).
    pub(super) fn leftover_maximum_pathfind_extent_ready() -> bool {
        gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|t| {
                let e = t.get_maximum_pathfind_extent();
                e.hi.x > e.lo.x && e.hi.y > e.lo.y
            })
            .unwrap_or(false)
    }

    /// Leftover `should_force_direct_path_for_off_map_start` (ai_path.rs).
    /// When leftover TerrainLogic extent is ready, use that C++ region.
    /// Otherwise leftover-install `getMaximumPathfindExtent` onto live world bounds
    /// so off-map reinforcements still get computeQuickPath.
    pub fn leftover_should_force_direct_path_for_off_map_start(
        &self,
        start: Vec3,
        dest: Vec3,
    ) -> bool {
        if Self::leftover_maximum_pathfind_extent_ready() {
            return gamelogic::object::unit::leftover_should_force_direct_path_for_off_map_start(
                &Self::leftover_coord_from_host(start),
                &Self::leftover_coord_from_host(dest),
            );
        }
        !self.leftover_in_live_world_extent(dest) && !self.leftover_in_live_world_extent(start)
    }

    /// Leftover `Region3D::isInRegionNoZ` on live `PathfindingGrid` world bounds.
    pub(super) fn leftover_in_live_world_extent(&self, pos: Vec3) -> bool {
        let lo_x = self.grid.origin.x;
        let lo_z = self.grid.origin.z;
        let hi_x = lo_x + self.grid.world_extent_w;
        let hi_z = lo_z + self.grid.world_extent_h;
        gamelogic::object::unit::leftover_is_in_region_no_z(
            &gamelogic::common::Region3D::new(
                gamelogic::common::Coord3D::new(lo_x, lo_z, 0.0),
                gamelogic::common::Coord3D::new(hi_x, hi_z, 0.0),
            ),
            &Self::leftover_coord_from_host(pos),
        )
    }

    /// Leftover `should_use_direct_path_for_line_passable_non_final_goal` leftover-installed
    /// onto live terrain (pinched + passable, no occupancy). Does not consult leftover
    /// `THE_AI` pathfinder — that grid is not the live map.
    pub fn leftover_should_use_direct_path_for_line_passable_non_final_goal(
        &self,
        is_final_goal: bool,
        start: Vec3,
        dest: Vec3,
        surfaces: u32,
        _ignore_obstacle: Option<ObjectId>,
    ) -> bool {
        if is_final_goal || surfaces == 0 {
            return false;
        }
        let from = self.grid.world_to_grid(start);
        let to = self.grid.world_to_grid(dest);
        // C++ computePath gate (AIUpdate.cpp:1691-1694) calls isLinePassable
        // with allowPinched=false — a straight quick path must not cross
        // pinched cells.
        self.grid
            .leftover_is_line_passable_for_surfaces(from, to, surfaces, false)
    }

    /// C++ `computeQuickPath` two-node start+dest leftover-installed on host Y-up.
    pub fn leftover_compute_quick_path_nodes(start: Vec3, dest: Vec3) -> Vec<Vec3> {
        let [a, b] = gamelogic::object::unit::leftover_compute_quick_path_coords(
            &Self::leftover_coord_from_host(start),
            &Self::leftover_coord_from_host(dest),
        );
        vec![
            Self::leftover_host_from_coord(a),
            Self::leftover_host_from_coord(b),
        ]
    }
}
