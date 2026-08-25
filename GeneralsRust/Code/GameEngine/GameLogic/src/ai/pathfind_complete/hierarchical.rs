use super::*;

impl PathfindingSystem {
    /// C++ `Pathfinder::tightenPath` (AIPathfind.cpp:8414-8421).
    ///
    /// Walk cells from `from` toward `to` via Bresenham; advance `from` to the
    /// last position that still passes destination adjust (checkForAdjust).
    pub fn tighten_path(
        &self,
        from: &mut Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
    ) {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let layer = self.get_layer_for_coord(GridCoord::from_world(from));
        let start = *from;
        let mut found = false;
        let mut dest_pos = start;
        let _ = self.iterate_cells_along_line_world(&start, to, layer, |_from_c, to_c, cx, cy| {
            // C++ layer change aborts tighten walk when layer differs.
            if self.get_layer_for_coord(to_c) != layer {
                return 1;
            }
            let mut adjust = to_c.to_world(layer);
            if self.try_adjust_cell(
                cx,
                cy,
                layer,
                surfaces,
                is_crusher,
                radius,
                center_in_cell,
                ignore_obstacle_id,
                Some(&start),
                &mut adjust,
            ) {
                found = true;
                dest_pos = adjust;
                0 // keep going (C++ keeps walking while adjust succeeds)
            } else {
                1 // bail early
            }
        });
        if found {
            *from = dest_pos;
        }
    }

    /// C++ `Pathfinder::checkForLanding` (AIPathfind.cpp:5228-5247).
    /// `unit_id` skips that unit's own goalAircraft reservation.
    pub(crate) fn check_for_landing(
        &self,
        cell_x: i32,
        cell_y: i32,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
        dest: &mut Coord3D,
        unit_id: ObjectID,
    ) -> bool {
        let coord = GridCoord::new(cell_x, cell_y);
        if !self.is_valid_coord(coord) {
            return false;
        }
        let world = coord.to_world(layer);
        match self.get_cell_type(&world) {
            Some(PathfindCellType::Cliff)
            | Some(PathfindCellType::Water)
            | Some(PathfindCellType::Impassable) => return false,
            _ => {}
        }
        let goal_ac = self.get_goal_aircraft(coord);
        if goal_ac != INVALID_ID && (unit_id == INVALID_ID || goal_ac != unit_id) {
            return false;
        }
        // C++ checkDestination(NULL, ...) — no object occupancy special-case.
        if !self.is_destination_valid(
            coord,
            layer,
            SURFACE_GROUND,
            false,
            radius,
            center_in_cell,
            None,
        ) {
            return false;
        }
        self.adjust_coord_to_cell(cell_x, cell_y, center_in_cell, dest, layer);
        true
    }

    /// C++ `Pathfinder::adjustToLandingDestination` (AIPathfind.cpp:5253-5320).
    ///
    /// Spiral-search an unoccupied landing cell. Off-map object + off-map dest
    /// is treated as scripted success (leave dest unchanged).
    pub fn adjust_to_landing_destination(
        &self,
        from: &Coord3D,
        dest: &mut Coord3D,
        unit_radius: f32,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);

        // C++: if dest off map and unit off map → true (scripted).
        let dest_in = self.is_valid_coord(GridCoord::from_world(dest));
        let from_in = self.is_valid_coord(GridCoord::from_world(from));
        if !dest_in {
            if !from_in {
                return true;
            }
            // Dest off map but unit on map — still try spiral from clamped? C++ still
            // worldToCells the half-biased dest; out-of-bounds cells fail checkForLanding.
        }

        let mut adjust_dest = *dest;
        if !center_in_cell {
            adjust_dest.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_dest.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let cell = GridCoord::from_world(&adjust_dest);
        let layer = self.get_layer_for_coord(GridCoord::from_world(dest));

        if self.check_for_landing(
            cell.x,
            cell.y,
            layer,
            radius,
            center_in_cell,
            dest,
            INVALID_ID,
        ) {
            return true;
        }

        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = cell.x;
        let mut j = cell.y;
        let mut delta = 1;
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, INVALID_ID) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, INVALID_ID) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, INVALID_ID) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, INVALID_ID) {
                    return true;
                }
            }
            delta += 1;
        }
        false
    }

    /// Landing dest for a known aircraft: skip that unit's own goalAircraft.
    pub fn adjust_to_landing_destination_for(
        &self,
        from: &Coord3D,
        dest: &mut Coord3D,
        unit_radius: f32,
        unit_id: ObjectID,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let dest_in = self.is_valid_coord(GridCoord::from_world(dest));
        let from_in = self.is_valid_coord(GridCoord::from_world(from));
        if !dest_in && !from_in {
            return true;
        }
        let mut adjust_dest = *dest;
        if !center_in_cell {
            adjust_dest.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_dest.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let cell = GridCoord::from_world(&adjust_dest);
        let layer = self.get_layer_for_coord(GridCoord::from_world(dest));
        if self.check_for_landing(cell.x, cell.y, layer, radius, center_in_cell, dest, unit_id) {
            return true;
        }
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = cell.x;
        let mut j = cell.y;
        let mut delta = 1;
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, unit_id) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, unit_id) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, unit_id) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.check_for_landing(i, j, layer, radius, center_in_cell, dest, unit_id) {
                    return true;
                }
            }
            delta += 1;
        }
        false
    }

    /// Full adjustment pipeline combining adjustDestination and zone check.
    /// Matches C++ Pathfinder::checkForAdjust() at AIPathfind.cpp ~5300.
    /// C++ `Pathfinder::checkForAdjust` (AIPathfind.cpp:5175-5226).
    ///
    /// Thin wrapper used by older callers — assumes human player and no group dest.
    pub fn check_for_adjust(
        &self,
        dest: &mut Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        self.check_for_adjust_ex(
            dest,
            surfaces,
            is_crusher,
            unit_radius,
            ignore_obstacle_id,
            true, // is_human default (safe for player units)
            None, // from
            None, // group_dest
        )
    }

    /// Full C++ `checkForAdjust` with human logical-extent clamp, optional
    /// path-existence gate from unit position, and groupDest tighten/cost.
    pub fn check_for_adjust_ex(
        &self,
        dest: &mut Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
        is_human: bool,
        from: Option<&Coord3D>,
        group_dest: Option<&Coord3D>,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let mut adjust_seed = *dest;
        if !center_in_cell {
            adjust_seed.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_seed.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let cell = GridCoord::from_world(&adjust_seed);
        let layer = self.get_layer_for_coord(cell);
        if !self.is_valid_coord(cell) {
            return false;
        }
        // C++: no final destinations on cliffs.
        let world = cell.to_world(layer);
        if self.get_cell_type(&world) == Some(PathfindCellType::Cliff) {
            return false;
        }
        // C++: human must stay inside m_logicalExtent.
        if is_human && !self.in_logical_extent(cell) {
            return false;
        }
        if !self.is_destination_valid(
            cell,
            layer,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            ignore_obstacle_id,
        ) {
            return false;
        }

        let mut adjust_dest = world;
        self.adjust_coord_to_cell(cell.x, cell.y, center_in_cell, &mut adjust_dest, layer);
        if let Some(terrain) = TheTerrainLogic::get() {
            adjust_dest.z = terrain.get_layer_height(
                adjust_dest.x,
                adjust_dest.y,
                CommonPathfindLayerEnum::Ground,
            );
        }

        // C++ path existence gate when unit position known.
        if let Some(from_pos) = from {
            let path_exists = self.client_safe_quick_does_path_exist(surfaces, from_pos, dest);
            let adjusted_path_exists =
                self.client_safe_quick_does_path_exist(surfaces, from_pos, &adjust_dest);
            let mut ok = adjusted_path_exists;
            if !path_exists {
                if self.client_safe_quick_does_path_exist(surfaces, dest, &adjust_dest) {
                    ok = true;
                }
            }
            if !ok {
                return false;
            }
        }

        // C++: if groupDest, tightenPath + checkPathCost gate.
        if let Some(gd) = group_dest {
            self.tighten_path(
                &mut adjust_dest,
                gd,
                surfaces,
                is_crusher,
                unit_radius,
                ignore_obstacle_id,
            );
            let cost = self.check_path_cost(surfaces, is_crusher, gd, &adjust_dest);
            let dx = (gd.x - adjust_dest.x).abs();
            let dy = (gd.y - adjust_dest.y).abs();
            // C++: if (1.4f*(dx+dy) < cost) return false;
            if cost > 0.0 && 1.4 * (dx + dy) < cost {
                return false;
            }
        }

        *dest = adjust_dest;
        true
    }

    /// Validate a destination cell is passable for the given parameters.
    /// Matches C++ Pathfinder::checkDestination() at AIPathfind.cpp ~5200.
    pub fn validate_destination(
        &self,
        dest: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let cell = GridCoord::from_world(dest);
        self.is_destination_valid(
            cell,
            PathfindLayerEnum::Ground,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            None,
        )
    }

    // ========================================================================
    // GROUP C – Hierarchical pathfinding
    // ========================================================================

    /// C++ `Pathfinder::processHierarchicalCell` (AIPathfind.cpp:7322+).
    ///
    /// Expand from parent zone-block cell into an adjacent block cell when both
    /// share the same effective global zone. Returns true if adj was enqueued.
    pub fn process_hierarchical_cell(
        &self,
        scan_cell: GridCoord,
        delta: (i32, i32),
        parent_zone: u16,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        examined_zones: &mut Vec<u16>,
    ) -> Option<(GridCoord, u16)> {
        if parent_zone == UNINITIALIZED_ZONE || parent_zone == 0 {
            return None;
        }
        if !self.is_valid_coord(scan_cell) {
            return None;
        }
        let Ok(zones) = self.zones.lock() else {
            return None;
        };
        let scan_block = zones.get_block_zone(surfaces, crusher, scan_cell.x, scan_cell.y);
        if scan_block != parent_zone {
            return None;
        }
        let adj = GridCoord::new(scan_cell.x + delta.0, scan_cell.y + delta.1);
        if !self.is_valid_coord(adj) {
            return None;
        }
        // C++ hierarchical: skip pinched cells when expanding neighbors.
        if let Ok(pf) = self.pathfinder.lock() {
            if pf.is_pinched(adj) == Some(true) {
                return None;
            }
        }
        let new_zone = zones.get_block_zone(surfaces, crusher, adj.x, adj.y);
        let parent_global = zones.get_effective_zone(surfaces, crusher, parent_zone);
        let new_global = zones.get_effective_zone(surfaces, crusher, new_zone);
        if new_global != parent_global {
            // Orthogonal neighbors must share effective zone. Bridge jumps use
            // `hierarchical_bridge_jumps` (C++ interactsWithBridge layer scan).
            return None;
        }
        if examined_zones.contains(&new_zone) {
            return None;
        }
        examined_zones.push(new_zone);
        Some((adj, new_zone))
    }

    /// C++ hierarchical bridge expansion (AIPathfind.cpp ~7595-7650).
    ///
    /// When the parent cell's zone block interacts with a bridge, enqueue the
    /// far-end ground cell of each live bridge attached to that block.
    pub fn hierarchical_bridge_jumps(
        &self,
        parent_cell: GridCoord,
        parent_zone: u16,
        goal_zone: u16,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        examined_zones: &mut Vec<u16>,
    ) -> Vec<(GridCoord, u16, bool)> {
        // Returns (far_end_cell, far_block_zone, reached_goal).
        let mut out = Vec::new();
        if parent_zone == 0 || parent_zone == UNINITIALIZED_ZONE {
            return out;
        }
        let Ok(zones) = self.zones.lock() else {
            return out;
        };
        if !zones.interacts_with_bridge(parent_cell.x, parent_cell.y) {
            return out;
        }
        let block_x = parent_cell.x.div_euclid(ZONE_BLOCK_SIZE);
        let block_y = parent_cell.y.div_euclid(ZONE_BLOCK_SIZE);

        for bridge in &self.bridges {
            if bridge.destroyed {
                continue;
            }
            // C++: pick orientation so start (ndx) is in parent block.
            let (near, far) = {
                let s = bridge.start_cell;
                let e = bridge.end_cell;
                let sbx = s.x.div_euclid(ZONE_BLOCK_SIZE);
                let sby = s.y.div_euclid(ZONE_BLOCK_SIZE);
                let ebx = e.x.div_euclid(ZONE_BLOCK_SIZE);
                let eby = e.y.div_euclid(ZONE_BLOCK_SIZE);
                if sbx == block_x && sby == block_y {
                    (s, e)
                } else if ebx == block_x && eby == block_y {
                    (e, s)
                } else {
                    // Also accept ground_connect_cells in this block.
                    let mut found_near = None;
                    let mut found_far = None;
                    for c in &bridge.ground_connect_cells {
                        let bx = c.x.div_euclid(ZONE_BLOCK_SIZE);
                        let by = c.y.div_euclid(ZONE_BLOCK_SIZE);
                        if bx == block_x && by == block_y {
                            found_near = Some(*c);
                        } else {
                            found_far = Some(*c);
                        }
                    }
                    match (found_near, found_far) {
                        (Some(n), Some(f)) => (n, f),
                        _ => continue,
                    }
                }
            };
            if near.x < 0 || near.y < 0 || far.x < 0 || far.y < 0 {
                continue;
            }
            if !self.is_valid_coord(near) || !self.is_valid_coord(far) {
                continue;
            }
            let near_zone = zones.get_block_zone(surfaces, crusher, near.x, near.y);
            if near_zone != parent_zone {
                continue;
            }
            // Goal via bridge layer zone.
            if bridge.zone != 0 && bridge.zone == goal_zone {
                out.push((
                    far,
                    zones.get_block_zone(surfaces, crusher, far.x, far.y),
                    true,
                ));
                continue;
            }
            let far_zone = zones.get_block_zone(surfaces, crusher, far.x, far.y);
            if far_zone == 0 || examined_zones.contains(&far_zone) {
                continue;
            }
            examined_zones.push(far_zone);
            out.push((far, far_zone, false));
        }
        out
    }

    /// BFS over hierarchical bridge jumps to see if start can reach goal zone.
    pub(crate) fn hierarchical_zones_join_via_bridge(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
    ) -> bool {
        let Ok(zones) = self.zones.lock() else {
            return false;
        };
        let start_z = zones.get_block_zone(surfaces, crusher, start.x, start.y);
        let goal_z = zones.get_block_zone(surfaces, crusher, goal.x, goal.y);
        drop(zones);
        if start_z == 0 || goal_z == 0 {
            return false;
        }
        if start_z == goal_z {
            return true;
        }
        let mut examined = vec![start_z];
        let mut queue = vec![start];
        let mut seen_cells = std::collections::HashSet::new();
        seen_cells.insert(start);
        let mut steps = 0;
        while let Some(cell) = queue.pop() {
            steps += 1;
            if steps > 256 {
                break;
            }
            let parent_z = {
                let Ok(zones) = self.zones.lock() else {
                    break;
                };
                zones.get_block_zone(surfaces, crusher, cell.x, cell.y)
            };
            if parent_z == goal_z {
                return true;
            }
            for (far, far_z, reached) in self.hierarchical_bridge_jumps(
                cell,
                parent_z,
                goal_z,
                surfaces,
                crusher,
                &mut examined,
            ) {
                if reached || far_z == goal_z {
                    return true;
                }
                if seen_cells.insert(far) {
                    queue.push(far);
                }
            }
        }
        false
    }

    /// Long-distance hierarchical path check using zone connectivity.
    /// Matches C++ Pathfinder::findHierarchicalPath() concept.
    ///
    /// Uses the zone manager to verify that start and end are in connected
    /// zones, then delegates to the full A* pathfinder.
    /// C++ `Pathfinder::findHierarchicalPath` → internal_findHierarchicalPath(closestOK=false).
    pub fn find_hierarchical_path(
        &self,
        start: Coord3D,
        end: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> Option<PathResult> {
        self.internal_find_hierarchical_path(start, end, surfaces, is_crusher, false, false)
    }

    /// C++ `Pathfinder::findClosestHierarchicalPath` → closestOK=true.
    pub fn find_closest_hierarchical_path(
        &self,
        start: Coord3D,
        end: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> Option<PathResult> {
        self.internal_find_hierarchical_path(start, end, surfaces, is_crusher, true, false)
    }

    /// C++ `Pathfinder::internal_findHierarchicalPath` (AIPathfind.cpp:7434+).
    ///
    /// Zone-block A* using processHierarchicalCell + bridge jumps. On success
    /// builds a cell path via find_path_internal from start to the reached cell
    /// (exact goal or closest block when `closest_ok`).
    pub fn internal_find_hierarchical_path(
        &self,
        start: Coord3D,
        end: Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        closest_ok: bool,
        is_human: bool,
    ) -> Option<PathResult> {
        const COST_ORTHO: i32 = 10;
        const MAX_CELLS: i32 = 5000;

        if !self.is_map_ready {
            return None;
        }
        // C++ rejects path to 0,0 as generally a bug.
        if end.x == 0.0 && end.y == 0.0 {
            return None;
        }

        let start_cell = GridCoord::from_world(&start);
        let end_cell = GridCoord::from_world(&end);
        if !self.is_valid_coord(start_cell) || !self.is_valid_coord(end_cell) {
            return None;
        }
        if is_human && (!self.in_logical_extent(start_cell) || !self.in_logical_extent(end_cell)) {
            if is_human && !self.in_logical_extent(start_cell) {
                return None;
            }
        }

        // Effective zone equality gate (C++ zone1 != zone2 early out).
        let (z1, z2) = {
            let Ok(zones) = self.zones.lock() else {
                return None;
            };
            let a = zones.get_effective_zone(surfaces, is_crusher, zones.zone_at(start_cell));
            let b = zones.get_effective_zone(surfaces, is_crusher, zones.zone_at(end_cell));
            (a, b)
        };
        if z1 != 0 && z2 != 0 && z1 != z2 {
            if !self.hierarchical_zones_join_via_bridge(start_cell, end_cell, surfaces, is_crusher)
            {
                return None;
            }
        }

        let goal_block_zone = {
            let Ok(zones) = self.zones.lock() else {
                return None;
            };
            zones.get_block_zone(surfaces, is_crusher, end_cell.x, end_cell.y)
        };
        let goal_block_ndx = (
            end_cell.x.div_euclid(ZONE_BLOCK_SIZE),
            end_cell.y.div_euclid(ZONE_BLOCK_SIZE),
        );

        // Hierarchical open list: f = g + h_to_goal_block, store (f, g, x, y).
        let hier_h = |c: GridCoord| -> i32 {
            let bx = c.x.div_euclid(ZONE_BLOCK_SIZE);
            let by = c.y.div_euclid(ZONE_BLOCK_SIZE);
            let dx = (goal_block_ndx.0 - bx).abs();
            let dy = (goal_block_ndx.1 - by).abs();
            // Block-scale orthogonal cost.
            (dx + dy) * COST_ORTHO * ZONE_BLOCK_SIZE
        };

        let mut open: std::collections::BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> =
            std::collections::BinaryHeap::new();
        let mut g_score: HashMap<(i32, i32), i32> = HashMap::new();
        let mut closed: HashSet<(i32, i32)> = HashSet::new();
        let mut came_from: HashMap<(i32, i32), (i32, i32)> = HashMap::new();
        open.push(std::cmp::Reverse((
            hier_h(start_cell),
            0,
            start_cell.x,
            start_cell.y,
        )));
        g_score.insert((start_cell.x, start_cell.y), 0);

        let mut closest: Option<(GridCoord, f32)> = None;
        let mut cell_count = 0i32;
        let mut reached: Option<GridCoord> = None;

        while let Some(std::cmp::Reverse((_f, g, cx, cy))) = open.pop() {
            if closed.contains(&(cx, cy)) {
                continue;
            }
            closed.insert((cx, cy));
            let parent = GridCoord::new(cx, cy);
            cell_count += 1;
            if cell_count > MAX_CELLS {
                break;
            }

            let parent_zone = {
                let Ok(zones) = self.zones.lock() else {
                    break;
                };
                zones.get_block_zone(surfaces, is_crusher, cx, cy)
            };

            let block_x = cx.div_euclid(ZONE_BLOCK_SIZE);
            let block_y = cy.div_euclid(ZONE_BLOCK_SIZE);
            let mut at_goal = parent_zone == goal_block_zone
                && block_x == goal_block_ndx.0
                && block_y == goal_block_ndx.1;
            // Exact cell match also counts.
            if cx == end_cell.x && cy == end_cell.y {
                at_goal = true;
            }

            if at_goal {
                reached = Some(parent);
                break;
            }

            // Track closest for closestOK.
            if closest_ok {
                let dx = (end_cell.x - cx).abs() as f32;
                let dy = (end_cell.y - cy).abs() as f32;
                let d2 = dx * dx + dy * dy;
                if closest.map(|(_, bd)| d2 < bd).unwrap_or(true) {
                    closest = Some((parent, d2));
                }
            }

            // Expand hierarchical neighbors (orthogonal zone-block steps +
            // same-block cell steps for denser open-list like C++ block scan).
            let mut examined = Vec::new();
            let expand_deltas: [(i32, i32); 8] = [
                (ZONE_BLOCK_SIZE, 0),
                (0, ZONE_BLOCK_SIZE),
                (-ZONE_BLOCK_SIZE, 0),
                (0, -ZONE_BLOCK_SIZE),
                (1, 0),
                (0, 1),
                (-1, 0),
                (0, -1),
            ];
            for &(dx, dy) in &expand_deltas {
                // C++ processHierarchicalCell(scan = parent, delta) expands into adj.
                if let Some((adj, _nz)) = self.process_hierarchical_cell(
                    parent,
                    (dx, dy),
                    parent_zone,
                    surfaces,
                    is_crusher,
                    &mut examined,
                ) {
                    let key = (adj.x, adj.y);
                    if closed.contains(&key) {
                        continue;
                    }
                    if is_human && !self.in_logical_extent(adj) {
                        continue;
                    }
                    let step = COST_ORTHO * ZONE_BLOCK_SIZE;
                    let ng = g + step;
                    if g_score.get(&key).is_some_and(|&og| ng >= og) {
                        continue;
                    }
                    g_score.insert(key, ng);
                    came_from.insert(key, (cx, cy));
                    let f = ng + hier_h(adj);
                    open.push(std::cmp::Reverse((f, ng, adj.x, adj.y)));
                }
            }

            // Bridge jumps from this cell.
            for (far, _far_z, hit_goal) in self.hierarchical_bridge_jumps(
                parent,
                parent_zone,
                goal_block_zone,
                surfaces,
                is_crusher,
                &mut examined,
            ) {
                if hit_goal {
                    reached = Some(far);
                    // continue processing but mark goal
                }
                let key = (far.x, far.y);
                if closed.contains(&key) {
                    continue;
                }
                let step = COST_ORTHO * ZONE_BLOCK_SIZE;
                let ng = g + step;
                if g_score.get(&key).is_some_and(|&og| ng >= og) {
                    continue;
                }
                g_score.insert(key, ng);
                came_from.insert(key, (cx, cy));
                let f = ng + hier_h(far);
                open.push(std::cmp::Reverse((f, ng, far.x, far.y)));
            }

            if reached.is_some() {
                break;
            }
        }

        let dest_cell = if let Some(c) = reached {
            c
        } else if closest_ok {
            closest.map(|(c, _)| c).unwrap_or(end_cell)
        } else {
            // Zone-block A* found no block path — if effective zones still connect
            // (open ground single zone), fall through to cell A* like prior residual.
            let connected = self
                .zones
                .lock()
                .map(|z| z.are_connected(start_cell, end_cell, surfaces, is_crusher))
                .unwrap_or(false)
                || self
                    .hierarchical_zones_join_via_bridge(start_cell, end_cell, surfaces, is_crusher);
            if !connected {
                return None;
            }
            end_cell
        };

        // Prefer exact goal world pos when we landed in goal block.
        let to = if dest_cell.x == end_cell.x && dest_cell.y == end_cell.y {
            end
        } else if reached.is_some()
            && dest_cell.x.div_euclid(ZONE_BLOCK_SIZE) == goal_block_ndx.0
            && dest_cell.y.div_euclid(ZONE_BLOCK_SIZE) == goal_block_ndx.1
        {
            end
        } else {
            dest_cell.to_world(PathfindLayerEnum::Ground)
        };

        let request = PathRequest {
            object_id: INVALID_ID,
            from: start,
            to,
            surfaces,
            is_crusher,
            unit_radius: 0.0,
            allow_partial: closest_ok,
            move_allies: false,
            ignore_obstacle_id: None,
            is_human,
        };
        let result = self.find_path_internal(request);
        if result.success { Some(result) } else { None }
    }

    // ========================================================================
    // GROUP D – Path utilities and dynamic map updates
    // ========================================================================

    /// Quick path existence check (for UI feedback).
    /// Matches C++ Pathfinder::quickDoesPathExist() concept.
    ///
    /// Uses zone connectivity as a fast heuristic. Does not run full A*.
    pub fn quick_does_path_exist(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> bool {
        let start_cell = GridCoord::from_world(start);
        let end_cell = GridCoord::from_world(end);

        // Bounds check
        if !self.is_valid_coord(start_cell) || !self.is_valid_coord(end_cell) {
            return false;
        }

        // Quick passability check on start/end
        let pathfinder = self.pathfinder.lock().unwrap();
        if !pathfinder.is_passable(start_cell, surfaces, is_crusher) {
            return false;
        }
        if !pathfinder.is_passable(end_cell, surfaces, is_crusher) {
            return false;
        }
        drop(pathfinder);

        // Zone connectivity check
        if let Ok(zones) = self.zones.lock() {
            zones.are_connected(start_cell, end_cell, surfaces, is_crusher)
        } else {
            true
        }
    }

    /// Full path existence check (runs actual A*).
    /// C++ `Pathfinder::slowDoesPathExist(obj, from, to, ignoreObject)`.
    pub fn slow_does_path_exist(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> bool {
        self.slow_does_path_exist_ex(start, end, surfaces, is_crusher, None, INVALID_ID)
    }

    /// C++ `slowDoesPathExist` with ignore obstacle + optional object id for radius.
    pub fn slow_does_path_exist_ex(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        ignore_obstacle_id: Option<ObjectID>,
        object_id: ObjectID,
    ) -> bool {
        // C++ temporarily sets m_ignoreObstacleID around findPath.
        let request = PathRequest {
            object_id,
            from: *start,
            to: *end,
            surfaces,
            is_crusher,
            unit_radius: 0.0,
            allow_partial: false,
            move_allies: false,
            ignore_obstacle_id,
            is_human: false,
        };
        self.find_path(request).success
    }

    /// Check if a ground path is passable between two points.
    /// Matches C++ Pathfinder::isGroundPathPassable().
    pub fn is_ground_path_passable(
        &self,
        start: &Coord3D,
        end: &Coord3D,
        is_crusher: bool,
        diameter: i32,
    ) -> bool {
        self.is_ground_line_passable(start, end, is_crusher, diameter, None)
    }

    /// C++ `Pathfinder::clip` (AIPathfind.cpp) — clip from/to cells to map extent.
    ///
    /// When an endpoint cell is outside `m_extent`, move that world point onto the
    /// clipped cell ( + 0.05 like C++ ).
    pub fn clip(&self, from: &mut Coord3D, to: &mut Coord3D) {
        let from_cell = GridCoord::from_world(from);
        let to_cell = GridCoord::from_world(to);
        let extent = (
            GridCoord::new(0, 0),
            GridCoord::new(
                self.width.saturating_sub(1) as i32,
                self.height.saturating_sub(1) as i32,
            ),
        );
        if let Some((cf, ct)) = clip_line_cells(from_cell, to_cell, extent) {
            if cf != from_cell {
                from.x = cf.x as f32 * PATHFIND_CELL_SIZE_F + 0.05;
                from.y = cf.y as f32 * PATHFIND_CELL_SIZE_F + 0.05;
            }
            if ct != to_cell {
                to.x = ct.x as f32 * PATHFIND_CELL_SIZE_F + 0.05;
                to.y = ct.y as f32 * PATHFIND_CELL_SIZE_F + 0.05;
            }
        }
    }
}
