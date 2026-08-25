use super::*;

impl PathfindingSystem {
    // ========================================================================
    // GROUP B – Destination adjustment
    // ========================================================================

    /// Snap destination to the nearest passable cell using spiral search.
    /// C++ `Pathfinder::adjustDestination` (AIPathfind.cpp:5331-5407).
    ///
    /// Returns `true` if adjustment succeeded (dest was modified in-place).
    /// Spiral: right, down, left, up, expanding (matches C++).
    pub fn adjust_destination(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        dest: &mut Coord3D,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        self.adjust_destination_from(
            None,
            surfaces,
            is_crusher,
            dest,
            unit_radius,
            ignore_obstacle_id,
        )
    }

    /// C++ `adjustDestination` with optional unit position for path-existence gate
    /// (`clientSafeQuickDoesPathExist` in `checkForAdjust`).
    pub fn adjust_destination_from(
        &self,
        from: Option<&Coord3D>,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        dest: &mut Coord3D,
        unit_radius: f32,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        // C++: if (!center) adjustDest += PATHFIND_CELL_SIZE_F/2 before worldToCell.
        let mut adjust_dest = *dest;
        if !center_in_cell {
            adjust_dest.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_dest.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let cell = GridCoord::from_world(&adjust_dest);
        // C++: layer = TheTerrainLogic->getLayerForDestination(dest)
        let layer = self.get_layer_for_coord(cell);

        // Exact cell first (C++ checkForAdjust on seed cell).
        if self.try_adjust_cell(
            cell.x,
            cell.y,
            layer,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            ignore_obstacle_id,
            from,
            dest,
        ) {
            return true;
        }

        // Spiral search - matches C++ at AIPathfind.cpp:5366-5399
        const MAX_CELLS_TO_TRY: i32 = 400;
        let mut limit = MAX_CELLS_TO_TRY;
        let mut i = cell.x;
        let mut j = cell.y;
        let mut delta = 1;

        while limit > 0 {
            // Right
            for _ in 0..delta {
                i += 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    from,
                    dest,
                ) {
                    return true;
                }
            }
            // Down
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    from,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
            // Left
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    from,
                    dest,
                ) {
                    return true;
                }
            }
            // Up
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.try_adjust_cell(
                    i,
                    j,
                    layer,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    ignore_obstacle_id,
                    from,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
        }

        false
    }

    /// C++ `Pathfinder::checkForAdjust` core (no groupDest tighten).
    pub(crate) fn try_adjust_cell(
        &self,
        cx: i32,
        cy: i32,
        layer: PathfindLayerEnum,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        radius: i32,
        center_in_cell: bool,
        ignore_obstacle_id: Option<ObjectID>,
        from: Option<&Coord3D>,
        dest: &mut Coord3D,
    ) -> bool {
        let coord = GridCoord::new(cx, cy);
        if !self.is_valid_coord(coord) {
            return false;
        }
        // C++: no final destinations on cliffs.
        let world = coord.to_world(layer);
        if self.get_cell_type(&world) == Some(PathfindCellType::Cliff) {
            return false;
        }
        if !self.is_destination_valid(
            coord,
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
        if let Some(terrain) = TheTerrainLogic::get() {
            adjust_dest.z =
                terrain.get_layer_height(world.x, world.y, CommonPathfindLayerEnum::Ground);
        }

        // C++ checkForAdjust path gate via clientSafeQuickDoesPathExist.
        if let Some(from_pos) = from {
            let path_exists = self.client_safe_quick_does_path_exist(surfaces, from_pos, dest);
            let adjusted_path_exists =
                self.client_safe_quick_does_path_exist(surfaces, from_pos, &adjust_dest);
            let mut ok = adjusted_path_exists;
            if !path_exists {
                // C++: if (!pathExists) { if (clientSafeQuick(dest, adjustDest)) ok }
                if self.client_safe_quick_does_path_exist(surfaces, dest, &adjust_dest) {
                    ok = true;
                }
            }
            if !ok {
                return false;
            }
        }

        dest.x = adjust_dest.x;
        dest.y = adjust_dest.y;
        dest.z = adjust_dest.z;
        true
    }

    /// Check if a cell is a valid destination for the given parameters.
    /// Matches C++ Pathfinder::checkDestination() logic.
    pub(crate) fn is_destination_valid(
        &self,
        cell: GridCoord,
        _layer: PathfindLayerEnum,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        radius: i32,
        center_in_cell: bool,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        if !self.is_valid_coord(cell) {
            return false;
        }

        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        let pathfinder = self.pathfinder.lock().unwrap();

        // Check all cells in the unit's footprint
        let mut num_cells_above = radius;
        if center_in_cell {
            num_cells_above += 1;
        }
        let start_x = cell.x - radius;
        let end_x = cell.x + num_cells_above;
        let start_y = cell.y - radius;
        let end_y = cell.y + num_cells_above;

        for x in start_x..end_x {
            for y in start_y..end_y {
                let coord = GridCoord::new(x, y);
                if !pathfinder.is_passable_with_ignore(
                    coord,
                    surfaces,
                    is_crusher,
                    ignore_cells.as_ref(),
                ) {
                    return false;
                }
            }
        }
        true
    }

    /// C++ PathfindCell flags derived from goal/pos unit IDs (setGoalUnit/setPosUnit).
    #[inline]
    pub(crate) fn cell_occupancy_flags(goal_u: ObjectID, pos_u: ObjectID) -> u8 {
        // Matches AIPathfind.h CellFlags + setGoalUnit/setPosUnit transitions.
        const NO_UNITS: u8 = 0x00;
        const UNIT_GOAL: u8 = 0x01;
        const UNIT_PRESENT_MOVING: u8 = 0x02;
        const UNIT_PRESENT_FIXED: u8 = 0x03;
        const UNIT_GOAL_OTHER_MOVING: u8 = 0x05;
        if goal_u == INVALID_ID && pos_u == INVALID_ID {
            NO_UNITS
        } else if goal_u != INVALID_ID && pos_u == INVALID_ID {
            UNIT_GOAL
        } else if goal_u == INVALID_ID && pos_u != INVALID_ID {
            UNIT_PRESENT_MOVING
        } else if goal_u == pos_u {
            UNIT_PRESENT_FIXED
        } else {
            UNIT_GOAL_OTHER_MOVING
        }
    }

    /// C++ `Pathfinder::checkForMovement` (AIPathfind.cpp:4971-5076).
    ///
    /// Footprint scan of goal/pos occupancy. Populates ally/enemy fixed counts.
    /// Returns false if off-map or blocked by non-AI ally fixed unit.
    pub fn check_for_movement(&self, obj_id: ObjectID, info: &mut CheckMovementInfo) -> bool {
        info.ally_fixed_count = 0;
        info.ally_moving = false;
        info.ally_goal = false;
        info.enemy_fixed = false;

        if obj_id == INVALID_ID {
            return true;
        }

        let Some(ignore_id) = OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
            let mut id = INVALID_ID;
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                if let Ok(ai_g) = ai.lock() {
                    id = ai_g.get_ignored_obstacle_id();
                }
            }
            id
        }) else {
            return true;
        };

        let mut num_cells_above = info.radius;
        if info.center_in_cell {
            num_cells_above += 1;
        }

        const MAX_ALLY: usize = 5;
        let mut allies: [ObjectID; MAX_ALLY] = [INVALID_ID; MAX_ALLY];
        let mut num_ally = 0usize;

        const UNIT_GOAL: u8 = 0x01;
        const UNIT_PRESENT_MOVING: u8 = 0x02;
        const UNIT_PRESENT_FIXED: u8 = 0x03;
        const UNIT_GOAL_OTHER_MOVING: u8 = 0x05;

        let Ok(goals) = self.goal_cells.lock() else {
            return true;
        };

        for i in (info.cell.x - info.radius)..(info.cell.x + num_cells_above) {
            for j in (info.cell.y - info.radius)..(info.cell.y + num_cells_above) {
                let coord = GridCoord::new(i, j);
                if !self.is_valid_coord(coord) {
                    return false; // off the map
                }
                let Some(row) = goals.get(coord.x as usize) else {
                    continue;
                };
                let Some(gc) = row.get(coord.y as usize) else {
                    continue;
                };
                let goal_u = gc.get_goal_unit(info.layer);
                let pos_u = gc.get_pos_unit(info.layer);
                let flags = Self::cell_occupancy_flags(goal_u, pos_u);

                // C++: UNIT_GOAL | UNIT_GOAL_OTHER_MOVING → allyGoal.
                if flags == UNIT_GOAL || flags == UNIT_GOAL_OTHER_MOVING {
                    info.ally_goal = true;
                }

                // C++ NO_UNITS continue.
                if flags == 0x00 {
                    continue;
                }

                // C++ uses getPosUnit for the occupying unit identity.
                let pos_unit = pos_u;
                if pos_unit == INVALID_ID {
                    // Goal-only cell: no present unit to collide with for fixed/moving checks.
                    continue;
                }
                if pos_unit == obj_id || pos_unit == ignore_id {
                    continue;
                }

                let mut check = false;
                if flags == UNIT_PRESENT_MOVING || flags == UNIT_GOAL_OTHER_MOVING {
                    let is_ally = OBJECT_REGISTRY
                        .with_object(obj_id, |obj_guard| {
                            OBJECT_REGISTRY.with_object(pos_unit, |unit_guard| {
                                obj_guard.relationship_to(&unit_guard) == Relationship::Allies
                            })
                        })
                        .flatten()
                        .unwrap_or(false);
                    if is_ally {
                        info.ally_moving = true;
                    }
                    if info.consider_transient {
                        check = true;
                    }
                }
                if flags == UNIT_PRESENT_FIXED {
                    check = true;
                }

                if !check {
                    continue;
                }

                // C++ INFANTRY_MOVES_THROUGH_INFANTRY (AIPathfind.cpp:5031-5036).
                let infantry_through = OBJECT_REGISTRY
                    .with_object(obj_id, |obj_guard| {
                        OBJECT_REGISTRY.with_object(pos_unit, |unit_guard| {
                            obj_guard.is_kind_of(KindOf::Infantry)
                                && unit_guard.is_kind_of(KindOf::Infantry)
                        })
                    })
                    .flatten()
                    .unwrap_or(false);
                if infantry_through {
                    continue;
                }

                // order matters: obj considers unit relationship.
                let Some((rel, unit_has_ai, can_crush)) = OBJECT_REGISTRY
                    .with_object(obj_id, |obj_guard| {
                        OBJECT_REGISTRY.with_object(pos_unit, |unit_guard| {
                            let rel = obj_guard.relationship_to(&unit_guard);
                            let unit_has_ai = unit_guard.get_ai_update_interface().is_some();
                            let can_crush = obj_guard.can_crush_or_squish(
                                &unit_guard,
                                CrushSquishTestType::TestCrushOrSquish,
                            );
                            (rel, unit_has_ai, can_crush)
                        })
                    })
                    .flatten()
                else {
                    continue;
                };

                if rel == Relationship::Allies {
                    // C++: can't path through non-AI allies.
                    if !unit_has_ai {
                        return false;
                    }
                    let mut found = false;
                    for k in 0..num_ally {
                        if allies[k] == pos_unit {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        info.ally_fixed_count += 1;
                        if num_ally < MAX_ALLY {
                            allies[num_ally] = pos_unit;
                            num_ally += 1;
                        }
                    }
                } else if !can_crush {
                    // C++ obj->canCrushOrSquish(unit, TEST_CRUSH_OR_SQUISH).
                    info.enemy_fixed = true;
                }
            }
        }

        true
    }

    /// Find a pathable spot near the destination.
    /// C++ `Pathfinder::adjustToPossibleDestination` (AIPathfind.cpp:5510-5617).
    ///
    /// Same-zone passable destination via spiral; half-cell bias when not centered.
    /// C++ `Pathfinder::checkForPossible` (AIPathfind.cpp:5489-5504).
    pub fn check_for_possible(
        &self,
        is_crusher: bool,
        from_zone: u16,
        center: bool,
        surfaces: LocomotorSurfaceTypeMask,
        cell_x: i32,
        cell_y: i32,
        layer: PathfindLayerEnum,
        dest: &mut Coord3D,
        starting_in_obstacle: bool,
    ) -> bool {
        let cell = GridCoord::new(cell_x, cell_y);
        if !self.is_valid_coord(cell) {
            return false;
        }
        {
            let Ok(pf) = self.pathfinder.lock() else {
                return false;
            };
            if let Some(ct) = pf.get_cell_type(cell) {
                if matches!(
                    ct,
                    PathfindCellType::Impassable
                        | PathfindCellType::Obstacle
                        | PathfindCellType::BridgeImpassable
                ) {
                    return false;
                }
            }
        }
        let mut zone2 = if let Ok(zones) = self.zones.lock() {
            let z = zones.zone_at(cell);
            let mut z2 = zones.get_effective_zone(surfaces, is_crusher, z);
            if starting_in_obstacle {
                z2 = zones.get_effective_terrain_zone(z2);
            }
            z2
        } else {
            0
        };
        let _ = layer;
        if from_zone == zone2 {
            self.adjust_coord_to_cell(cell_x, cell_y, center, dest, layer);
            return true;
        }
        let _ = &mut zone2;
        false
    }

    pub fn adjust_to_possible_destination(
        &self,
        start: &Coord3D,
        dest: &mut Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        unit_radius: f32,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        // C++: if (!center) adjustDest += PATHFIND_CELL_SIZE_F/2 before worldToCell.
        let mut adjust_dest = *dest;
        if !center_in_cell {
            adjust_dest.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_dest.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let goal_cell = GridCoord::from_world(&adjust_dest);
        // C++ worldToCell returns true when outside bounds → fail.
        if !self.is_valid_coord(goal_cell) {
            return false;
        }
        let destination_layer = self.get_layer_for_coord(goal_cell);

        let start_cell = GridCoord::from_world(start);
        let same_zone = if let Ok(zones) = self.zones.lock() {
            zones.are_connected(start_cell, goal_cell, surfaces, is_crusher)
        } else {
            true
        };

        if same_zone {
            if self.is_destination_valid(
                goal_cell,
                destination_layer,
                surfaces,
                is_crusher,
                radius,
                center_in_cell,
                None,
            ) {
                // C++ returns true without rewriting dest when seed is already valid.
                return true;
            }
        }

        // Spiral search
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
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j += 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
            for _ in 0..delta {
                i -= 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            for _ in 0..delta {
                j -= 1;
                limit -= 1;
                if self.try_zone_adjust(
                    i,
                    j,
                    start_cell,
                    surfaces,
                    is_crusher,
                    radius,
                    center_in_cell,
                    dest,
                ) {
                    return true;
                }
            }
            delta += 1;
        }

        false
    }

    /// C++ checkForPossible + checkDestination for adjustToPossibleDestination spiral.
    pub(crate) fn try_zone_adjust(
        &self,
        cx: i32,
        cy: i32,
        start_cell: GridCoord,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        radius: i32,
        center_in_cell: bool,
        dest: &mut Coord3D,
    ) -> bool {
        let coord = GridCoord::new(cx, cy);
        if !self.is_valid_coord(coord) {
            return false;
        }
        let layer = self.get_layer_for_coord(coord);

        let connected = if let Ok(zones) = self.zones.lock() {
            zones.are_connected(start_cell, coord, surfaces, is_crusher)
        } else {
            true
        };
        if !connected {
            return false;
        }

        if !self.is_destination_valid(
            coord,
            layer,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            None,
        ) {
            return false;
        }

        self.adjust_coord_to_cell(cx, cy, center_in_cell, dest, layer);
        true
    }

    /// C++ `Pathfinder::checkForTarget` (AIPathfind.cpp:5409-5421).
    ///
    /// Valid destination cell that is within weapon attack range of the target.
    pub fn check_for_target(
        &self,
        cell_x: i32,
        cell_y: i32,
        radius: i32,
        center_in_cell: bool,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        ignore_obstacle_id: Option<ObjectID>,
        in_range: impl Fn(&Coord3D) -> bool,
        dest: &mut Coord3D,
    ) -> bool {
        let coord = GridCoord::new(cell_x, cell_y);
        if !self.is_valid_coord(coord) {
            return false;
        }
        if !self.is_destination_valid(
            coord,
            PathfindLayerEnum::Ground,
            surfaces,
            is_crusher,
            radius,
            center_in_cell,
            ignore_obstacle_id,
        ) {
            return false;
        }
        // C++ checkDestination aircraft branch: refuse another unit's goalAircraft.
        let mut num_above = radius;
        if center_in_cell {
            num_above += 1;
        }
        for x in (cell_x - radius)..(cell_x + num_above) {
            for y in (cell_y - radius)..(cell_y + num_above) {
                let goal_ac = self.get_goal_aircraft(GridCoord::new(x, y));
                if goal_ac != INVALID_ID && ignore_obstacle_id != Some(goal_ac) {
                    return false;
                }
            }
        }

        let mut adjust_dest = Coord3D::new(0.0, 0.0, 0.0);
        self.adjust_coord_to_cell(
            cell_x,
            cell_y,
            center_in_cell,
            &mut adjust_dest,
            PathfindLayerEnum::Ground,
        );
        if !in_range(&adjust_dest) {
            return false;
        }
        *dest = adjust_dest;
        true
    }

    /// C++ `Pathfinder::adjustTargetDestination` (AIPathfind.cpp:5428-5487).
    ///
    /// Spiral-search an unoccupied spot that can fire at the victim.
    /// `in_range(goal)` should implement weapon isGoalPosWithinAttackRange.
    pub fn adjust_target_destination(
        &self,
        dest: &mut Coord3D,
        unit_radius: f32,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        ignore_obstacle_id: Option<ObjectID>,
        in_range: impl Fn(&Coord3D) -> bool,
    ) -> bool {
        let (radius, center_in_cell) = Self::compute_radius_and_center(unit_radius);
        let mut adjust_dest = *dest;
        if !center_in_cell {
            adjust_dest.x += PATHFIND_CELL_SIZE_F * 0.5;
            adjust_dest.y += PATHFIND_CELL_SIZE_F * 0.5;
        }
        let cell = GridCoord::from_world(&adjust_dest);
        // C++ worldToCell returns true when outside bounds → fail.
        if !self.is_valid_coord(cell) {
            return false;
        }

        if self.check_for_target(
            cell.x,
            cell.y,
            radius,
            center_in_cell,
            surfaces,
            is_crusher,
            ignore_obstacle_id,
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
                    surfaces,
                    is_crusher,
                    ignore_obstacle_id,
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
                    surfaces,
                    is_crusher,
                    ignore_obstacle_id,
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
                    surfaces,
                    is_crusher,
                    ignore_obstacle_id,
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
                    surfaces,
                    is_crusher,
                    ignore_obstacle_id,
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

    /// C++ `Pathfinder::moveAlliesAwayFromDestination` (AIPathfind.cpp:6911-6922).
    ///
    /// Bresenham walk from unit to destination; for each allied idle unit
    /// occupying a cell, issue `aiMoveAwayFromUnit`. Returns ids nudged.
    pub fn move_allies_away_from_destination(
        &self,
        obj_id: ObjectID,
        from: &Coord3D,
        destination: &Coord3D,
    ) -> Vec<ObjectID> {
        // Wave 262: empty dual-world → empty vec.
        if dual_world_registry_unavailable() {
            return Vec::new();
        }

        let mut nudged = Vec::new();
        if obj_id == INVALID_ID {
            return nudged;
        }
        let Some(ignore_id) = OBJECT_REGISTRY.with_object(obj_id, |obj_guard| {
            let mut id = INVALID_ID;
            if let Some(ai) = obj_guard.get_ai_update_interface() {
                if let Ok(ai_g) = ai.lock() {
                    id = ai_g.get_ignored_obstacle_id();
                }
            }
            id
        }) else {
            return nudged;
        };
        let layer = self.get_layer_for_coord(GridCoord::from_world(from));
        let _ = self.iterate_cells_along_line_world(
            from,
            destination,
            layer,
            |_from_c, to_c, _x, _y| {
                let Ok(goals) = self.goal_cells.lock() else {
                    return 0;
                };
                let Some(row) = goals.get(to_c.x as usize) else {
                    return 0;
                };
                let Some(gc) = row.get(to_c.y as usize) else {
                    return 0;
                };
                let cell_layer = self.get_layer_for_coord(to_c);
                let pos_unit = gc.get_pos_unit(cell_layer);
                drop(goals);
                if pos_unit == INVALID_ID || pos_unit == obj_id || pos_unit == ignore_id {
                    return 0;
                }
                let Some(other_ai) = OBJECT_REGISTRY
                    .with_object(pos_unit, |other_guard| {
                        let is_ally = OBJECT_REGISTRY
                            .with_object(obj_id, |obj_guard| {
                                obj_guard.relationship_to(&other_guard) == Relationship::Allies
                            })
                            .unwrap_or(false);
                        if !is_ally {
                            return None;
                        }
                        let other_ai = other_guard.get_ai_update_interface()?;
                        {
                            let Ok(other_ai_g) = other_ai.lock() else {
                                return None;
                            };
                            if !other_ai_g.is_idle() {
                                return None;
                            }
                        }
                        Some(other_ai)
                    })
                    .flatten()
                else {
                    return 0;
                };
                use crate::modules::AIUpdateInterfaceExt;
                other_ai.ai_move_away_from_unit(obj_id, crate::common::CommandSourceType::FromAi);
                if !nudged.contains(&pos_unit) {
                    nudged.push(pos_unit);
                }
                0 // keep going
            },
        );
        nudged
    }
}
