use super::*;

impl PathfindingGrid {
    /// Clamp a grid position into the playable rectangle.
    pub fn clamp_pos(&self, pos: GridPos) -> GridPos {
        GridPos::new(
            pos.x.clamp(0, self.width.saturating_sub(1).max(0)),
            pos.y.clamp(0, self.height.saturating_sub(1).max(0)),
        )
    }

    /// Nearest non-blocked cell around `pos` (spiral search). Returns None if none found.
    pub fn nearest_open(&self, pos: GridPos, max_radius: i32) -> Option<GridPos> {
        let origin = self.clamp_pos(pos);
        if self.is_valid_pos(origin) && !self.is_blocked(origin) {
            return Some(origin);
        }
        for r in 1..=max_radius {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let candidate = GridPos::new(origin.x + dx, origin.y + dy);
                    if self.is_valid_pos(candidate) && !self.is_blocked(candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    /// Find path using A* algorithm.
    ///
    /// Start/goal are clamped into the grid. If the goal cell is blocked (building
    /// footprint etc.), the nearest open cell is used so infantry can still approach.
    ///
    /// Parity notes vs C++ examineNeighboringCells (host simplified grid):
    /// - static blocks hard-reject; dynamic unit occupancy is a soft cost (allyFixed-like)
    /// - diagonal steps require both orthogonal legs open (no corner cut)
    pub fn find_path(&self, start: GridPos, goal: GridPos) -> Option<Vec<Vec3>> {
        if self.width <= 0 || self.height <= 0 {
            return None;
        }

        let start = self
            .nearest_static_open(self.clamp_pos(start), 16)
            .unwrap_or_else(|| self.clamp_pos(start));
        // Prefer static-open goal; dynamic occupancy near goal is soft-costed below.
        let goal = self
            .nearest_static_open(self.clamp_pos(goal), 16)
            .unwrap_or_else(|| self.clamp_pos(goal));

        // Either endpoint still static-blocked and no open neighbor — cannot plan.
        if self.is_static_blocked(start) || self.is_static_blocked(goal) {
            return None;
        }

        // Trivial same-cell path.
        if start == goal {
            return Some(vec![self.grid_to_world(start)]);
        }

        let mut open_set = BinaryHeap::new();
        let mut came_from: HashMap<GridPos, GridPos> = HashMap::new();
        let mut g_score: HashMap<GridPos, f32> = HashMap::new();
        // Closed bitset keeps large open-field A* from revisiting nodes forever.
        let mut closed = vec![0u64; self.blocked_bits.len().max(1)];

        g_score.insert(start, 0.0);
        open_set.push(PathNode::new(start, 0.0, start.distance(goal), None));

        while let Some(current) = open_set.pop() {
            if current.pos == goal {
                // Reconstruct path
                return Some(self.reconstruct_path(&came_from, current.pos));
            }

            let Some(cidx) = self.bit_index(current.pos) else {
                continue;
            };
            if Self::bit_test(&closed, cidx) {
                continue;
            }
            Self::bit_set(&mut closed, cidx, true);

            for neighbor in current.pos.neighbors() {
                if !self.is_valid_pos(neighbor) || self.is_static_blocked(neighbor) {
                    continue;
                }
                if !self.diameter_allows(self.query_is_crusher, neighbor) {
                    continue;
                }
                if self
                    .bit_index(neighbor)
                    .is_some_and(|idx| Self::bit_test(&closed, idx))
                {
                    continue;
                }

                let dx = neighbor.x - current.pos.x;
                let dy = neighbor.y - current.pos.y;
                let is_diag = dx.abs() == 1 && dy.abs() == 1;

                // C++ diagonal corner-cut: both orthogonal legs must be open.
                if is_diag {
                    let ortho_a = GridPos::new(current.pos.x + dx, current.pos.y);
                    let ortho_b = GridPos::new(current.pos.x, current.pos.y + dy);
                    if !self.is_valid_pos(ortho_a)
                        || !self.is_valid_pos(ortho_b)
                        || self.is_static_blocked(ortho_a)
                        || self.is_static_blocked(ortho_b)
                    {
                        continue;
                    }
                }

                // Base ortho/diag cost (COST_ORTHOGONAL=1, COST_DIAGONAL≈1.414).
                let mut movement_cost = if is_diag { 1.414_213_5 } else { 1.0 };
                // C++ costSoFar pinched surcharge (AIPathfind.cpp:1701-1703).
                if self.is_pinched(neighbor) {
                    movement_cost += 1.414_213_5;
                }
                match self.occupancy_cost(neighbor, None, false, 0, 0, Some(start)) {
                    None => continue, // enemyFixed abort
                    Some(extra) => movement_cost += extra,
                }

                let tentative_g_score = current.g_cost + movement_cost;

                if tentative_g_score < *g_score.get(&neighbor).unwrap_or(&f32::INFINITY) {
                    came_from.insert(neighbor, current.pos);
                    g_score.insert(neighbor, tentative_g_score);

                    open_set.push(PathNode::new(
                        neighbor,
                        tentative_g_score,
                        neighbor.distance(goal),
                        Some(current.pos),
                    ));
                }
            }
        }

        None // No path found
    }

    /// Like nearest_open but only considers static blocks (dynamic is soft in A*).
    pub(super) fn nearest_static_open(&self, origin: GridPos, max_radius: i32) -> Option<GridPos> {
        if self.is_valid_pos(origin) && !self.is_static_blocked(origin) {
            return Some(origin);
        }
        for r in 1..=max_radius {
            for dx in -r..=r {
                for dy in -r..=r {
                    if dx.abs() != r && dy.abs() != r {
                        continue;
                    }
                    let candidate = GridPos::new(origin.x + dx, origin.y + dy);
                    if self.is_valid_pos(candidate) && !self.is_static_blocked(candidate) {
                        return Some(candidate);
                    }
                }
            }
        }
        None
    }

    pub(super) fn reconstruct_path(
        &self,
        came_from: &HashMap<GridPos, GridPos>,
        mut current: GridPos,
    ) -> Vec<Vec3> {
        let mut path = vec![self.grid_to_world(current)];

        while let Some(&parent) = came_from.get(&current) {
            current = parent;
            path.push(self.grid_to_world(current));
        }

        path.reverse();
        path
    }

    /// Update dynamic obstacles based on unit positions
    pub fn update_dynamic_obstacles(&mut self, objects: &HashMap<ObjectId, Object>) {
        self.update_dynamic_obstacles_ignoring(objects, None);
    }

    /// Same occupancy stamp, skipping `ignore` (C++ `ignoreObstacle(goalObject)`).
    pub fn update_dynamic_obstacles_ignoring(
        &mut self,
        objects: &HashMap<ObjectId, Object>,
        ignore: Option<ObjectId>,
    ) {
        self.clear_dynamic_blocks();

        // C++ iterates TheGameLogic's insertion-ordered object list
        // (getFirstObject/getNextObject); HashMap order would randomize which
        // unit owns a contested cell's single m_posUnitID. Sort by ObjectID.
        let mut stamp_order: Vec<&Object> = objects.values().collect();
        stamp_order.sort_by_key(|o| o.id);
        for obj in stamp_order {
            if !obj.is_alive() {
                continue;
            }
            if ignore == Some(obj.id) {
                continue;
            }
            let is_aircraft = obj.is_kind_of(KindOf::Aircraft)
                || obj.object_type == crate::game_logic::ObjectType::Aircraft
                || obj.chinook_ai.is_some();
            if is_aircraft {
                if Self::is_aircraft_that_adjusts_destination(obj) {
                    self.stamp_aircraft_goal_from_object(obj);
                }
                // C++ updatePos: !isDoingGroundMovement → removePos and return.
                // JetAIUpdate never grounds; parked/taxiing jets do not stamp UNIT_PRESENT.
                if !Self::is_doing_ground_movement(obj) {
                    continue;
                }
            }
            // C++ examineNeighboringCells occupancy: infantry + vehicles + structures.
            if !(obj.is_kind_of(KindOf::Vehicle)
                || obj.is_kind_of(KindOf::Structure)
                || obj.is_kind_of(KindOf::Infantry))
            {
                continue;
            }

            let player = obj.owner_player_id.unwrap_or(obj.team as u32);
            let moving = !obj.is_kind_of(KindOf::Structure)
                && (!obj.movement.path.is_empty() || obj.movement.velocity.length_squared() > 0.25);
            let infantry = obj.is_kind_of(KindOf::Infantry);
            let (radius, center_in_cell) =
                Self::radius_and_center(obj.selection_radius, self.grid_size);
            let mut num_above = radius;
            if center_in_cell {
                num_above += 1;
            }
            let pos = obj.get_position();
            let pos_layer = self.layer_for_destination(pos);
            let pos_at_end =
                self.object_interacts_with_bridge_end(pos, obj.selection_radius, pos_layer);
            let pos_do_layer = pos_layer != PathfindLayerEnum::Ground;
            let pos_do_ground = !pos_do_layer || pos_at_end;
            let grid_pos = self.cell_for_unit_position(pos, center_in_cell);
            for i in (grid_pos.x - radius)..(grid_pos.x + num_above) {
                for j in (grid_pos.y - radius)..(grid_pos.y + num_above) {
                    let p = GridPos::new(i, j);
                    if self.is_valid_pos(p) {
                        // C++ canCrushOrSquish TEST_CRUSH_OR_SQUISH: module
                        // presence is crush-through even at CrushableLevel 255.
                        let crushable = if obj.has_squish_collide {
                            0
                        } else {
                            obj.crushable_level
                        };
                        if pos_do_ground {
                            self.mark_occupancy(
                                p,
                                player,
                                moving,
                                infantry,
                                false,
                                crushable,
                                obj.id.0,
                                PathfindLayerEnum::Ground,
                            );
                        }
                        if pos_do_layer {
                            self.mark_occupancy(
                                p, player, moving, infantry, false, crushable, obj.id.0, pos_layer,
                            );
                        }
                    }
                }
            }
            // C++ Pathfinder::updateGoal stamps UNIT_GOAL on the destination cell.
            if !is_aircraft
                && !obj.is_kind_of(KindOf::Immobile)
                && !obj.is_kind_of(KindOf::Structure)
            {
                let dest = obj
                    .movement
                    .path
                    .last()
                    .copied()
                    .or(obj.movement.target_position);
                if let Some(goal) = dest {
                    let goal_layer = self.layer_for_destination(goal);
                    let goal_at_end = self.object_interacts_with_bridge_end(
                        pos,
                        obj.selection_radius,
                        goal_layer,
                    );
                    let goal_do_layer = goal_layer != PathfindLayerEnum::Ground;
                    let goal_do_ground = !goal_do_layer || goal_at_end;
                    let goal_cell = self.cell_for_unit_position(goal, center_in_cell);
                    for i in (goal_cell.x - radius)..(goal_cell.x + num_above) {
                        for j in (goal_cell.y - radius)..(goal_cell.y + num_above) {
                            let p = GridPos::new(i, j);
                            if self.is_valid_pos(p) {
                                if goal_do_ground {
                                    self.mark_occupancy(
                                        p,
                                        player,
                                        false,
                                        false,
                                        true,
                                        255,
                                        obj.id.0,
                                        PathfindLayerEnum::Ground,
                                    );
                                }
                                if goal_do_layer {
                                    self.mark_occupancy(
                                        p, player, false, false, true, 255, obj.id.0, goal_layer,
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    pub fn occupancy_extra_cost(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
    ) -> u32 {
        let ally_mask = seeker_player.map(|p| self.ally_mask_for(p)).unwrap_or(0);
        match self.occupancy_cost(
            pos,
            seeker_player,
            seeker_is_infantry,
            crusher_level,
            ally_mask,
            None,
        ) {
            None => u32::MAX / 8,
            Some(c) => (c * 10.0) as u32, // crate A* uses integer COST_DIAGONAL=14
        }
    }

    pub fn has_allied_goal(&self, pos: GridPos, seeker_player: Option<u32>) -> bool {
        self.has_allied_goal_on(pos, seeker_player, self.query_layer_enum())
    }

    pub fn has_allied_goal_on(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.goal == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        // C++ checkDestination: own UNIT_GOAL is skipped (goalUnitID==objID).
        if self.query_seeker_id != 0 && bits.goal_unit == self.query_seeker_id {
            return false;
        }
        let bit = 1u16 << player.min(15);
        let ally = self.ally_mask_for(player);
        // Refuse allies (other players + same-player siblings). Own reservation
        // already excluded above.
        (bits.goal & (ally | bit)) != 0
    }

    /// C++ `checkDestination` occupancy (AIPathfind.cpp:4946-4953).
    pub(super) fn has_blocking_fixed_occupant(&self, pos: GridPos, crusher_level: u8) -> bool {
        self.has_blocking_fixed_occupant_on(pos, crusher_level, self.query_layer_enum())
    }

    pub(super) fn has_blocking_fixed_occupant_on(
        &self,
        pos: GridPos,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.fixed == 0 {
            return false;
        }
        crusher_level == 0 || crusher_level <= bits.crushable
    }

    /// C++ `checkDestination` single-cell residual used by adjustDestination.
    pub(super) fn destination_cell_ok(
        &self,
        pos: GridPos,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        // C++ checkForAdjust isHuman: reject cells outside m_logicalExtent.
        if self.query_is_human && !self.in_logical_extent(pos) {
            return false;
        }
        if self.query_check_for_aircraft {
            return !self.has_other_aircraft_goal(pos);
        }
        if !self.cell_passable_for_layer(pos, layer, surfaces, is_crusher) {
            return false;
        }
        if self.resolved_cell_type(layer, pos) == PathfindCellType::Cliff {
            return false;
        }
        if self.has_allied_goal_on(pos, seeker_player, layer) {
            return false;
        }
        if self.has_blocking_fixed_occupant_on(pos, crusher_level, layer) {
            return false;
        }
        if !self.diameter_allows(is_crusher, pos) {
            return false;
        }
        // Leftover `check_for_adjust_ex` zone path-exists gate (AIPathfind.cpp:5198-5208).
        if (surfaces & SURFACE_AIR) == 0 {
            if let Some(from) = self.query_from {
                let from_w = self.grid_to_world(from);
                let dest_w = self.grid_to_world(pos);
                let orig_w = self
                    .query_orig_dest
                    .map(|p| self.grid_to_world(p))
                    .unwrap_or(dest_w);
                let path_exists =
                    self.quick_path_exists_for_crusher(from_w, orig_w, surfaces, is_crusher);
                let adjusted_path_exists =
                    self.quick_path_exists_for_crusher(from_w, dest_w, surfaces, is_crusher);
                let mut ok = adjusted_path_exists;
                if !path_exists
                    && self.quick_path_exists_for_crusher(orig_w, dest_w, surfaces, is_crusher)
                {
                    ok = true;
                }
                if !ok {
                    return false;
                }
            }
        }
        true
    }

    /// C++ `AIUpdateInterface::isAircraftThatAdjustsDestination` (HOVER/WINGS).
    pub fn is_aircraft_that_adjusts_destination(obj: &Object) -> bool {
        if matches!(obj.loco_appearance, LocomotorAppearance::Thrust) {
            return false;
        }
        matches!(
            obj.loco_appearance,
            LocomotorAppearance::Hover | LocomotorAppearance::Wings
        ) || obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft
            || obj.chinook_ai.is_some()
    }

    /// C++ `AIUpdateInterface::isDoingGroundMovement` (AIUpdate.cpp:2347-2361).
    /// Air-only / current-AIR locos never stamp UNIT_PRESENT.
    pub fn is_doing_ground_movement(obj: &Object) -> bool {
        use crate::game_logic::object::LOCO_SURFACE_AIR;
        if matches!(
            obj.loco_appearance,
            LocomotorAppearance::Wings | LocomotorAppearance::Thrust
        ) {
            return false;
        }
        if obj.locomotor_surfaces != 0 {
            if obj.locomotor_surfaces == LOCO_SURFACE_AIR {
                return false;
            }
            if (obj.locomotor_surfaces & LOCO_SURFACE_AIR) != 0 {
                return false;
            }
        }
        if obj.is_kind_of(KindOf::Aircraft)
            || obj.object_type == crate::game_logic::ObjectType::Aircraft
        {
            return false;
        }
        true
    }

    pub fn goal_aircraft(&self, pos: GridPos) -> u32 {
        self.bit_index(pos)
            .and_then(|idx| self.occ_goal_aircraft.get(idx).copied())
            .unwrap_or(0)
    }

    pub fn has_other_aircraft_goal(&self, pos: GridPos) -> bool {
        let id = self.goal_aircraft(pos);
        id != 0 && (self.query_seeker_id == 0 || id != self.query_seeker_id)
    }

    pub(super) fn stamp_aircraft_goal_cell(&mut self, pos: GridPos, unit_id: u32) {
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        if let Some(slot) = self.occ_goal_aircraft.get_mut(idx) {
            if *slot == 0 || *slot == unit_id {
                *slot = unit_id;
            }
        }
    }

    pub(crate) fn aircraft_goal_dest(obj: &Object) -> Option<Vec3> {
        if let Some(ai) = obj.chinook_ai.as_ref() {
            if matches!(
                ai.flight_status,
                crate::game_logic::host_combat_chinook::HostChinookFlightStatus::Landing
                    | crate::game_logic::host_combat_chinook::HostChinookFlightStatus::TakingOff
            ) {
                return Some(Vec3::new(ai.dest[0], ai.dest[2], ai.dest[1]));
            }
        }
        obj.movement
            .path
            .last()
            .copied()
            .or(obj.movement.target_position)
    }

    /// C++ `Pathfinder::updateAircraftGoal`.
    pub(super) fn stamp_aircraft_goal_from_object(&mut self, obj: &Object) {
        let Some(goal) = Self::aircraft_goal_dest(obj) else {
            return;
        };
        let (radius, center_in_cell) =
            Self::radius_and_center(obj.selection_radius, self.grid_size);
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        let cell = self.world_to_grid(goal);
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                self.stamp_aircraft_goal_cell(GridPos::new(i, j), obj.id.0);
            }
        }
        let ai_store = gamelogic::ai::the_ai(); if let Ok(ai) = ai_store.read() {
            if let Some(pf) = ai.pathfinder() {
                if let Ok(pf) = pf.read() {
                    let dest = gamelogic::common::Coord3D::new(goal.x, goal.z, goal.y);
                    pf.update_aircraft_goal(&dest, obj.id.0, radius, center_in_cell);
                }
            }
        }
    }

    pub(super) fn check_for_landing(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        match self.resolved_cell_type(layer, cell) {
            PathfindCellType::Cliff | PathfindCellType::Water | PathfindCellType::Impassable => {
                return false;
            }
            _ => {}
        }
        if self.has_other_aircraft_goal(cell) {
            return false;
        }
        // C++ checkDestination(NULL, cell, layer, iRadius, center): footprint
        // refuses CELL_OBSTACLE / IS_IMPASSABLE / any UNIT_GOAL / off-map.
        self.check_destination_null(cell, layer, radius, center_in_cell)
    }

    pub(super) fn check_destination_null(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                let p = GridPos::new(i, j);
                if !self.is_valid_pos(p) {
                    return false;
                }
                match self.resolved_cell_type(layer, p) {
                    PathfindCellType::Obstacle
                    | PathfindCellType::Impassable
                    | PathfindCellType::BridgeImpassable => {
                        return false;
                    }
                    _ => {}
                }
                if self.has_other_aircraft_goal(p) {
                    return false;
                }
                if self.has_allied_goal_on(p, None, layer) {
                    return false;
                }
                if self.has_blocking_fixed_occupant_on(p, 0, layer) {
                    return false;
                }
            }
        }
        true
    }

    /// C++ `Pathfinder::adjustToLandingDestination` spiral.
    pub fn adjust_to_landing_destination(
        &self,
        dest: GridPos,
        max_cells: i32,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        self.adjust_to_landing_destination_for(dest, max_cells, layer, 0, true)
    }

    pub fn adjust_to_landing_destination_for(
        &self,
        dest: GridPos,
        max_cells: i32,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
    ) -> Option<GridPos> {
        if !self.is_valid_pos(dest) {
            return None;
        }
        if self.check_for_landing(dest, layer, radius, center_in_cell) {
            return Some(dest);
        }
        let mut i = dest.x;
        let mut j = dest.y;
        let mut delta = 1;
        let mut limit = max_cells.max(1);
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_landing(c, layer, radius, center_in_cell) {
                    return Some(c);
                }
            }
            delta += 1;
        }
        None
    }

    /// C++ `Pathfinder::checkForTarget` (AIPathfind.cpp:5409-5421).
    /// Aircraft `checkDestination` only refuses another unit's goalAircraft.
    pub fn check_for_target(
        &self,
        cell_x: i32,
        cell_y: i32,
        radius: i32,
        center_in_cell: bool,
        seeker_id: u32,
        claimed: &HashSet<GridPos>,
        in_range: impl Fn(Vec3) -> bool,
        dest: &mut Vec3,
    ) -> bool {
        let cell = GridPos::new(cell_x, cell_y);
        if !self.is_valid_pos(cell) {
            return false;
        }
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                let p = GridPos::new(i, j);
                if !self.is_valid_pos(p) {
                    return false;
                }
                if claimed.contains(&p) {
                    return false;
                }
                let id = self.goal_aircraft(p);
                if id != 0 && id != seeker_id {
                    return false;
                }
            }
        }
        let size = self.grid_size;
        let o = self.origin;
        let adjust = if center_in_cell {
            Vec3::new(
                o.x + cell.x as f32 * size + size * 0.5,
                dest.y,
                o.z + cell.y as f32 * size + size * 0.5,
            )
        } else {
            Vec3::new(
                o.x + cell.x as f32 * size + 0.05,
                dest.y,
                o.z + cell.y as f32 * size + 0.05,
            )
        };
        if !in_range(adjust) {
            return false;
        }
        *dest = adjust;
        true
    }

    /// C++ `Pathfinder::adjustTargetDestination` (AIPathfind.cpp:5428-5487).
    pub fn adjust_target_destination(
        &self,
        dest: &mut Vec3,
        unit_radius: f32,
        seeker_id: u32,
        claimed: &HashSet<GridPos>,
        in_range: impl Fn(Vec3) -> bool,
    ) -> bool {
        let (radius, center_in_cell) = Self::radius_and_center(unit_radius, self.grid_size);
        let mut adjust_dest = *dest;
        if !center_in_cell {
            let half = self.grid_size * 0.5;
            adjust_dest.x += half;
            adjust_dest.z += half;
        }
        let cell = self.world_to_grid(adjust_dest);
        if !self.is_valid_pos(cell) {
            return false;
        }
        if self.check_for_target(
            cell.x,
            cell.y,
            radius,
            center_in_cell,
            seeker_id,
            claimed,
            &in_range,
            dest,
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
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.check_for_target(
                    i,
                    j,
                    radius,
                    center_in_cell,
                    seeker_id,
                    claimed,
                    &in_range,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
        }
        false
    }

    /// C++ linePassableCallback occupancy + pinched (AIPathfind.cpp:9553-9591).
    pub(super) fn occupancy_blocks_line(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        let bits = self.occupancy_bits(pos, self.query_layer_enum());
        if bits.fixed == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | self.ally_mask_for(player);
        if (bits.fixed & friend) != 0 {
            return true;
        }
        if (bits.fixed & !friend) != 0 {
            return crusher_level == 0 || crusher_level <= bits.crushable;
        }
        false
    }

    pub(super) fn occupancy_blocks_line_on(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.fixed == 0 {
            return false;
        }
        let Some(player) = seeker_player else {
            return true;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | self.ally_mask_for(player);
        if (bits.fixed & friend) != 0 {
            return true;
        }
        if (bits.fixed & !friend) != 0 {
            return crusher_level == 0 || crusher_level <= bits.crushable;
        }
        false
    }

    pub(super) fn line_cell_ok_on(
        &self,
        cell: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
        layer: PathfindLayerEnum,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        if !allow_pinched && self.is_pinched(cell) {
            return false;
        }
        if self.occupancy_blocks_line_on(cell, seeker_player, crusher_level, layer) {
            return false;
        }
        if !self.diameter_allows(is_crusher, cell) {
            return false;
        }
        if unpinched_cliff_passable
            && self.resolved_cell_type(layer, cell) == PathfindCellType::Cliff
            && !self.is_pinched(cell)
        {
            return true;
        }
        self.cell_passable_for_layer(cell, layer, surfaces, is_crusher)
    }

    /// C++ `isLinePassable` on the **anchor node's layer** (AIPathfind.cpp:501-502).
    pub(super) fn line_passable_on_layer(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
        layer: PathfindLayerEnum,
    ) -> bool {
        if from == to {
            return true;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if !self.line_cell_ok_on(
                cell,
                surfaces,
                is_crusher,
                allow_pinched,
                seeker_player,
                crusher_level,
                unpinched_cliff_passable,
                layer,
            ) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// C++ `validLocomotorSurfacesForCellType` + fence crusher exception.
    pub fn cell_passable_for(&self, pos: GridPos, surfaces: u32, is_crusher: bool) -> bool {
        self.cell_passable_for_layer(pos, PathfindLayerEnum::Ground, surfaces, is_crusher)
    }

    /// C++ `Pathfinder::getRadiusAndCenter` (AIPathfind.cpp:9670-9696). MAX_RADIUS=2.
    pub fn radius_and_center(unit_radius: f32, grid_size: f32) -> (i32, bool) {
        let cell = grid_size.max(1.0);
        let mut diameter = 2.0 * unit_radius;
        if diameter > cell && diameter < 2.0 * cell {
            diameter = 2.0 * cell;
        }
        let mut radius = (diameter / cell + 0.3).floor() as i32;
        let mut center_in_cell = false;
        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        const MAX_RADIUS: i32 = 2;
        if radius > MAX_RADIUS {
            radius = MAX_RADIUS;
            center_in_cell = true;
        }
        (radius, center_in_cell)
    }

    /// C++ getRadiusAndCenter + worldToCell (AIPathfind.cpp:9748-9753).
    pub fn cell_for_unit_position(&self, pos: Vec3, center_in_cell: bool) -> GridPos {
        if center_in_cell {
            self.world_to_grid(pos)
        } else {
            GridPos {
                x: ((pos.x - self.origin.x) / self.grid_size + 0.5).floor() as i32,
                y: ((pos.z - self.origin.z) / self.grid_size + 0.5).floor() as i32,
            }
        }
    }

    pub(super) fn check_destination_for(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for i in (cell.x - radius)..(cell.x + num_above) {
            for j in (cell.y - radius)..(cell.y + num_above) {
                let p = GridPos::new(i, j);
                if !self.destination_cell_ok(
                    p,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return false;
                }
                match self.resolved_cell_type(layer, p) {
                    PathfindCellType::Obstacle
                    | PathfindCellType::Impassable
                    | PathfindCellType::BridgeImpassable => {
                        return false;
                    }
                    _ => {}
                }
            }
        }
        true
    }

    /// Vehicle path width in cells. Infantry stay single-cell.
    pub fn path_diameter_for_unit(unit_radius: f32, grid_size: f32, is_vehicle: bool) -> i32 {
        if !is_vehicle {
            return 1;
        }
        let (radius, _) = Self::radius_and_center(unit_radius, grid_size);
        (2 * radius.max(1)).min(4)
    }

    pub fn set_query_footprint(&mut self, path_diameter: i32, is_crusher: bool) {
        self.query_path_diameter = path_diameter.max(1);
        self.query_is_crusher = is_crusher;
    }

    pub fn query_seeker_id(&self) -> u32 {
        self.query_seeker_id
    }

    pub fn set_query_seeker_id(&mut self, id: u32) {
        self.query_seeker_id = id;
    }

    /// C++ `Pathfinder::clearCellForDiameter` (AIPathfind.cpp:6700-6759).
    pub fn clear_cell_for_diameter(&self, crusher: bool, cell: GridPos, path_diameter: i32) -> i32 {
        clear_cell_for_diameter_impl(
            self.width,
            self.height,
            &self.cell_types,
            &self.fence_bits,
            &self.occ_fixed_mask,
            &self.occ_fixed_max_crushable,
            crusher,
            cell,
            path_diameter,
        )
    }

    pub(super) fn diameter_allows(&self, crusher: bool, cell: GridPos) -> bool {
        let d = self.query_path_diameter;
        d < 2 || self.clear_cell_for_diameter(crusher, cell, d) == d
    }

    /// C++ `Pathfinder::adjustDestination` spiral (AIPathfind.cpp:5331-5407).
    pub fn adjust_destination(
        &self,
        dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        max_cells: i32,
    ) -> Option<GridPos> {
        self.adjust_destination_ex(
            dest,
            surfaces,
            is_crusher,
            max_cells,
            None,
            if is_crusher { 1 } else { 0 },
        )
    }

    pub fn adjust_destination_ex(
        &self,
        dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        max_cells: i32,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> Option<GridPos> {
        self.adjust_destination_on_layer(
            dest,
            surfaces,
            is_crusher,
            max_cells,
            seeker_player,
            crusher_level,
            PathfindLayerEnum::Ground,
        )
    }

    /// C++ `adjustDestination` spiral on `layer` (`getCell(layer, i, j)`).
    pub fn adjust_destination_on_layer(
        &self,
        dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        max_cells: i32,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        let origin = self.clamp_pos(dest);
        if self.destination_cell_ok(
            origin,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return Some(origin);
        }
        let mut i = origin.x;
        let mut j = origin.y;
        let mut delta = 1;
        let mut limit = max_cells.max(1);
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.destination_cell_ok(
                    c,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
        }
        None
    }

    /// C++ `Pathfinder::checkForPossible` (AIPathfind.cpp:5489-5504).
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
        if !self.is_valid_pos(cell) {
            return false;
        }
        if matches!(
            self.resolved_cell_type(layer, cell),
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        ) {
            return false;
        }
        let z = self.path_zone(cell);
        let mut z2 = self.get_effective_zone(surfaces, is_crusher, z);
        // Leftover `get_effective_terrain_zone` when starting in an obstacle.
        if starting_in_obstacle {
            z2 = self.terrain_zone_equiv(z2);
        }
        if from_zone != z2 {
            return false;
        }
        *dest = self.adjust_coord_to_cell_on_layer(cell, center, layer);
        true
    }

    /// C++ `PathfindZoneManager::getEffectiveTerrainZone` residual.
    pub(super) fn terrain_zone_equiv(&self, zone: u16) -> u16 {
        if zone == 0 {
            return 0;
        }
        zone
    }

    /// C++ `Pathfinder::adjustToPossibleDestination` (AIPathfind.cpp:5510-5617).
    /// Weaker than `adjustDestination` / checkForAdjust — same-zone spiral.
    pub fn adjust_to_possible_destination(
        &self,
        start: Vec3,
        dest: &mut Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        self.adjust_to_possible_destination_ex(
            start,
            dest,
            surfaces,
            is_crusher,
            unit_radius,
            None,
            if is_crusher { 1 } else { 0 },
        )
    }

    pub fn adjust_to_possible_destination_ex(
        &self,
        start: Vec3,
        dest: &mut Vec3,
        surfaces: u32,
        is_crusher: bool,
        unit_radius: f32,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> bool {
        let (radius, center_in_cell) = Self::radius_and_center(unit_radius, self.grid_size);
        // C++: if (!center) adjustDest += PATHFIND_CELL_SIZE_F/2 before worldToCell.
        let mut adjust_dest = *dest;
        if !center_in_cell {
            let half = self.grid_size * 0.5;
            adjust_dest.x += half;
            adjust_dest.z += half;
        }
        let goal_cell = self.world_to_grid(adjust_dest);
        if !self.is_valid_pos(goal_cell) {
            return false;
        }
        let destination_layer = self.layer_for_destination(*dest);
        let start_cell = self.world_to_grid(start);
        let same_zone = self.zones_connected(start_cell, goal_cell, surfaces, is_crusher);
        if same_zone
            && self.destination_cell_ok(
                goal_cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                destination_layer,
            )
        {
            // C++ returns true without rewriting dest when seed is already valid.
            return true;
        }

        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = goal_cell.x;
        let mut j = goal_cell.y;
        let mut delta = 1;
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.try_zone_adjust(
                    GridPos::new(i, j),
                    start_cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    dest,
                    center_in_cell,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.try_zone_adjust(
                    GridPos::new(i, j),
                    start_cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    dest,
                    center_in_cell,
                ) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.try_zone_adjust(
                    GridPos::new(i, j),
                    start_cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    dest,
                    center_in_cell,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.try_zone_adjust(
                    GridPos::new(i, j),
                    start_cell,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    dest,
                    center_in_cell,
                ) {
                    return true;
                }
            }
            delta += 1;
        }
        let _ = radius;
        false
    }

    /// C++ checkForPossible + checkDestination for adjustToPossibleDestination spiral.
    pub(super) fn try_zone_adjust(
        &self,
        cell: GridPos,
        start_cell: GridPos,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        dest: &mut Vec3,
        center_in_cell: bool,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        if !self.zones_connected(start_cell, cell, surfaces, is_crusher) {
            return false;
        }
        let layer = self.layer_for_destination(self.grid_to_world(cell));
        if !self.destination_cell_ok(
            cell,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return false;
        }
        *dest = self.adjust_coord_to_cell_on_layer(cell, center_in_cell, layer);
        true
    }

    /// C++ `Pathfinder::tightenPath` (AIPathfind.cpp:8414-8421).
    /// Walk Bresenham from `from` toward `to`; advance `from` to the last
    /// cell that still passes `destination_cell_ok`.
    pub fn tighten_path(
        &self,
        from: &mut Vec3,
        to: Vec3,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) {
        let start = *from;
        let start_cell = self.world_to_grid(start);
        let goal_cell = self.world_to_grid(to);
        let mut found = false;
        let mut dest_pos = start;
        let mut x0 = start_cell.x;
        let mut y0 = start_cell.y;
        let x1 = goal_cell.x;
        let y1 = goal_cell.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if self.layer_for_destination(self.grid_to_world(cell)) != layer {
                break;
            }
            if self.destination_cell_ok(
                cell,
                surfaces,
                is_crusher,
                seeker_player,
                crusher_level,
                layer,
            ) {
                found = true;
                dest_pos = self.grid_to_world(cell);
            } else {
                break;
            }
            if x0 == x1 && y0 == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
        if found {
            *from = dest_pos;
        }
    }

    /// C++ `Pathfinder::checkPathCost` (AIPathfind.cpp:8338+).
    /// Limited A*; returns `0x7fff0000` when no path.
    pub fn check_path_cost(&self, surfaces: u32, is_crusher: bool, from: Vec3, to: Vec3) -> f32 {
        const MAX_COST: f32 = 0x7fff_0000u32 as f32;
        const MAX_CELL_COUNT: i32 = 500;
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if !self.is_valid_pos(start) || !self.is_valid_pos(goal) {
            return MAX_COST;
        }
        if !self.cell_passable_for(start, surfaces, is_crusher)
            && !(self.is_obstacle_fence(start) && is_crusher)
        {
            return MAX_COST;
        }
        if start == goal {
            return 0.0;
        }
        let heuristic = |c: GridPos| -> i32 {
            let dx = (goal.x - c.x).abs();
            let dy = (goal.y - c.y).abs();
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
        let mut open: BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32)>> = BinaryHeap::new();
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
                return g as f32;
            }
            if cell_count > MAX_CELL_COUNT {
                continue;
            }
            let parent = GridPos::new(cx, cy);
            if let Some(link) = self.change_layer_xy(parent) {
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
                let nc = GridPos::new(nx, ny);
                if !self.is_valid_pos(nc) || closed.contains(&(nx, ny)) {
                    continue;
                }
                if Self::skip_diagonal_if_squeezed(i, &neighbor_flags) {
                    continue;
                }
                if !self.cell_passable_for(nc, surfaces, is_crusher)
                    && !(self.is_obstacle_fence(nc) && is_crusher)
                {
                    continue;
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

    #[inline]
    pub(super) fn skip_diagonal_if_squeezed(i: usize, neighbor_flags: &[bool; 8]) -> bool {
        const FIRST_DIAGONAL: usize = 4;
        const ADJACENT: [usize; 5] = [0, 1, 2, 3, 0];
        i >= FIRST_DIAGONAL && !neighbor_flags[ADJACENT[i - 4]] && !neighbor_flags[ADJACENT[i - 3]]
    }

    /// C++ `checkChangeLayers`: same-XY connect-layer hop (2D residual).
    pub(super) fn change_layer_xy(&self, cell: GridPos) -> Option<GridPos> {
        let connect = self.ground_connect_layer(cell);
        if connect == 0 {
            return None;
        }
        Some(cell)
    }

    pub(super) fn human_extent_allows(&self, pos: GridPos, is_human: bool) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        if !is_human {
            return true;
        }
        self.in_logical_extent(pos)
    }

    /// C++ `checkForAdjust` groupDest tighten + `checkPathCost` gate, then
    /// spiral. Falls back to simple `adjustDestination` when the group
    /// cost check rejects every candidate (AIPathfind.cpp:5210-5403).
    pub fn adjust_destination_for_group(
        &self,
        dest: GridPos,
        group_dest: GridPos,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        if let Some(adj) = self.adjust_destination_on_layer_group(
            dest,
            Some(group_dest),
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return Some(adj);
        }
        self.adjust_destination_on_layer(
            dest,
            surfaces,
            is_crusher,
            400,
            seeker_player,
            crusher_level,
            layer,
        )
    }

    pub(super) fn check_for_adjust_group(
        &self,
        cell: GridPos,
        group_dest: Option<GridPos>,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> bool {
        if !self.destination_cell_ok(
            cell,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return false;
        }
        let Some(gd) = group_dest else {
            return true;
        };
        let mut adjust_dest = self.grid_to_world(cell);
        let group_w = self.grid_to_world(gd);
        self.tighten_path(
            &mut adjust_dest,
            group_w,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        );
        let cost = self.check_path_cost(surfaces, is_crusher, group_w, adjust_dest);
        let dx = (group_w.x - adjust_dest.x).abs();
        let dy = (group_w.z - adjust_dest.z).abs();
        if cost > 0.0 && 1.4 * (dx + dy) < cost {
            return false;
        }
        true
    }

    pub(super) fn adjust_destination_on_layer_group(
        &self,
        dest: GridPos,
        group_dest: Option<GridPos>,
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<GridPos> {
        let origin = self.clamp_pos(dest);
        if self.check_for_adjust_group(
            origin,
            group_dest,
            surfaces,
            is_crusher,
            seeker_player,
            crusher_level,
            layer,
        ) {
            return Some(origin);
        }
        let mut i = origin.x;
        let mut j = origin.y;
        let mut delta = 1;
        let mut limit = 400i32;
        while limit > 0 {
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                let c = GridPos::new(i, j);
                if self.check_for_adjust_group(
                    c,
                    group_dest,
                    surfaces,
                    is_crusher,
                    seeker_player,
                    crusher_level,
                    layer,
                ) {
                    return Some(c);
                }
            }
            delta += 1;
        }
        None
    }

    pub(super) fn line_cell_ok(
        &self,
        cell: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
    ) -> bool {
        if !self.is_valid_pos(cell) {
            return false;
        }
        if !allow_pinched && self.is_pinched(cell) {
            return false;
        }
        if self.occupancy_blocks_line(cell, seeker_player, crusher_level) {
            return false;
        }
        if !self.diameter_allows(is_crusher, cell) {
            return false;
        }
        if unpinched_cliff_passable
            && self.cell_type(cell) == PathfindCellType::Cliff
            && !self.is_pinched(cell)
        {
            return true;
        }
        self.cell_passable_for(cell, surfaces, is_crusher)
    }

    pub(super) fn line_passable(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        self.line_passable_ex(from, to, surfaces, is_crusher, true, None, 0, false)
    }

    pub(super) fn line_passable_ex(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        is_crusher: bool,
        allow_pinched: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
        unpinched_cliff_passable: bool,
    ) -> bool {
        if from == to {
            return true;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if !self.line_cell_ok(
                cell,
                surfaces,
                is_crusher,
                allow_pinched,
                seeker_player,
                crusher_level,
                unpinched_cliff_passable,
            ) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
    /// Leftover `Pathfinder::isLinePassable` (allowPinched=true variant)
    /// (occupancy.rs:265-273): is_crusher=false, no object occupancy.
    /// `allow_pinched` mirrors C++ isLinePassable's allowPinched flag
    /// (AIUpdate.cpp:1692 passes false; the leftover direct-path probe
    /// passed true, which let a straight line cross pinched cells).
    pub(super) fn leftover_is_line_passable_for_surfaces(
        &self,
        from: GridPos,
        to: GridPos,
        surfaces: u32,
        allow_pinched: bool,
    ) -> bool {
        if from == to {
            return true;
        }
        let mut x0 = from.x;
        let mut y0 = from.y;
        let x1 = to.x;
        let y1 = to.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if !self.is_valid_pos(cell) {
                return false;
            }
            if self.is_pinched(cell) {
                return false;
            }
            if !self.cell_passable_for(cell, surfaces, false) {
                return false;
            }
            if x0 == x1 && y0 == y1 {
                return true;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }

    /// C++ `Path::optimize` / `optimizeGroundPath` LOS-shortcut + jig removal.
    pub fn optimize_ground_path(
        &self,
        waypoints: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
    ) -> Vec<Vec3> {
        self.optimize_ground_path_ex(
            waypoints,
            surfaces,
            is_crusher,
            None,
            if is_crusher { 1 } else { 0 },
        )
    }

    pub fn optimize_ground_path_ex(
        &self,
        waypoints: &[Vec3],
        surfaces: u32,
        is_crusher: bool,
        seeker_player: Option<u32>,
        crusher_level: u8,
    ) -> Vec<Vec3> {
        if waypoints.len() <= 2 {
            return waypoints.to_vec();
        }
        // C++ Path::optimize ALLOWED_STEPS = 3 past a layer change
        // (AIPathfind.cpp:472-485). Deck shortcuts stay on the deck.
        const ALLOWED_STEPS: usize = 3;
        let layers: Vec<PathfindLayerEnum> = waypoints
            .iter()
            .map(|p| self.layer_for_destination(*p))
            .collect();
        let mut optimized = vec![waypoints[0]];
        let mut anchor = 0usize;
        let mut first_node = true;
        let first_layer = layers[0];
        while anchor < waypoints.len() - 1 {
            let mut layer = layers[anchor];
            let mut cur_layer = layers[anchor];
            let mut count = 0usize;
            let mut node_idx = anchor + 1;
            while node_idx + 1 < waypoints.len() {
                count += 1;
                if cur_layer == PathfindLayerEnum::Ground {
                    if layers[node_idx] != cur_layer {
                        layer = layers[node_idx];
                        cur_layer = layer;
                        if count > ALLOWED_STEPS {
                            break;
                        }
                    }
                } else if layers[node_idx + 1] != cur_layer && count > ALLOWED_STEPS {
                    break;
                }
                cur_layer = layers[node_idx];
                node_idx += 1;
            }
            if first_node {
                layer = first_layer;
                first_node = false;
            }
            let mut found = false;
            let mut far = node_idx;
            while far > anchor {
                let a = self.world_to_grid(waypoints[anchor]);
                let b = self.world_to_grid(waypoints[far]);
                // C++ groundPathPassableCallback: the H/V/diag "A* already
                // walked it" bypass only applies within one layer's corridor;
                // a ground anchor must LOS across a deck span on `layer`,
                // never shortcut across water via collinearity.
                let layers_uniform = (anchor..=far)
                    .all(|i| self.layer_for_destination(waypoints[i]) == layer);
                if self.line_passable_on_layer(
                    a,
                    b,
                    surfaces,
                    is_crusher,
                    false,
                    seeker_player,
                    crusher_level,
                    true,
                    layer,
                ) || (layers_uniform
                    // C++ optimizeGroundPath LOS is isGroundPathPassable →
                    // clearCellForDiameter (AIPathfind.cpp:9602-9613, 6721-6733):
                    // any solid CELL_OBSTACLE in the span refuses the shortcut;
                    // crusher fences stay exempt. The H/V/diag "A* already
                    // walked it" bypass (Path::optimize AIPathfind.cpp:511-551)
                    // must not erase detour waypoints across buildings.
                    && !self.span_crosses_solid_obstacle(waypoints, anchor, far)
                    && self.collinear_cells_force_passable(waypoints, anchor, far))
                {
                    if far == anchor {
                        break;
                    }
                    optimized.push(waypoints[far]);
                    anchor = far;
                    found = true;
                    break;
                }
                if far == 0 {
                    break;
                }
                far -= 1;
            }
            if !found {
                optimized.push(waypoints[anchor + 1]);
                anchor += 1;
            }
        }
        // C++ jig-jog removal: drop very short mid segments.
        let cell = self.grid_size;
        let thresh = cell * cell * 3.9;
        let mut i = 0;
        while i + 2 < optimized.len() {
            let dx = optimized[i + 1].x - optimized[i].x;
            let dz = optimized[i + 1].z - optimized[i].z;
            if dx * dx + dz * dz < thresh {
                optimized.remove(i + 1);
            } else {
                i += 1;
            }
        }
        optimized
    }

    /// C++ `Path::optimize` H/V/diag force-passable (AIPathfind.cpp:511-551).
    /// A* already walked these cells; pinched occupancy on a straight jog
    /// must not keep every cell center.
    pub(super) fn collinear_cells_force_passable(
        &self,
        waypoints: &[Vec3],
        from: usize,
        to: usize,
    ) -> bool {
        if to <= from {
            return true;
        }
        let cell = self.grid_size;
        let eps = cell * 0.15;
        let bx = waypoints[to].x - waypoints[from].x;
        let bz = waypoints[to].z - waypoints[from].z;
        if (bx.abs() - cell).abs() < eps && (bz.abs() - cell).abs() < eps {
            return true;
        }
        let horiz = bx.abs() < eps;
        let vert = bz.abs() < eps;
        let diag_pos = (bx - bz).abs() < eps;
        let diag_neg = (bx + bz).abs() < eps;
        if !horiz && !vert && !diag_pos && !diag_neg {
            return false;
        }
        for i in from..to {
            let dx = waypoints[i + 1].x - waypoints[i].x;
            let dz = waypoints[i + 1].z - waypoints[i].z;
            if horiz && dx.abs() >= eps {
                return false;
            }
            if vert && dz.abs() >= eps {
                return false;
            }
            if diag_pos && (dx - dz).abs() >= eps {
                return false;
            }
            if diag_neg && (dx + dz).abs() >= eps {
                return false;
            }
        }
        true
    }

    /// C++ `groundPathPassableCallback` / `clearCellForDiameter`
    /// (AIPathfind.cpp:9602-9613, 6721-6733): the optimize LOS walk refuses
    /// any solid CELL_OBSTACLE cell in the span. Crusher fences stay exempt.
    pub(super) fn span_crosses_solid_obstacle(
        &self,
        waypoints: &[Vec3],
        from: usize,
        to: usize,
    ) -> bool {
        let a = self.world_to_grid(waypoints[from]);
        let b = self.world_to_grid(waypoints[to]);
        let (mut x0, mut y0) = (a.x, a.y);
        let (x1, y1) = (b.x, b.y);
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let cell = GridPos::new(x0, y0);
            if self.cell_type(cell) == PathfindCellType::Obstacle && !self.is_obstacle_fence(cell)
            {
                return true;
            }
            if x0 == x1 && y0 == y1 {
                return false;
            }
            let e2 = 2 * err;
            if e2 >= dy {
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                err += dx;
                y0 += sy;
            }
        }
    }
}
