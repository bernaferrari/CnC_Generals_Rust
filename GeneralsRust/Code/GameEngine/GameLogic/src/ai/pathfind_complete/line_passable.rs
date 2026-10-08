use super::*;

impl PathfindingSystem {
    /// Optimize path using line-of-sight checks
    pub(crate) fn optimize_path(
        &self,
        waypoints: &[Coord3D],
        layers: &[PathfindLayerEnum],
        request: &PathRequest,
    ) -> (Vec<Coord3D>, Vec<PathfindLayerEnum>) {
        self.optimize_path_blocked(waypoints, layers, request, false)
    }

    /// C++ `Path::optimize(obj, surfaces, blocked)`.
    pub(crate) fn optimize_path_blocked(
        &self,
        waypoints: &[Coord3D],
        layers: &[PathfindLayerEnum],
        request: &PathRequest,
        blocked: bool,
    ) -> (Vec<Coord3D>, Vec<PathfindLayerEnum>) {
        let ignore_cells = ignored_obstacle_cells(request.ignore_obstacle_id);
        let obj_id = request.object_id;

        // Line passability checker — C++ isLinePassable(..., blocked, false).
        let passability = |from: &Coord3D, to: &Coord3D, layer: PathfindLayerEnum| {
            self.is_line_passable_for_object_with_ignore_id(
                obj_id,
                from,
                to,
                request.surfaces,
                request.is_crusher,
                layer,
                ignore_cells.as_ref(),
                false,   // allow_pinched
                blocked, // consider_transient / blocked ally handling
                0,
                true,
                Some(request.ignore_obstacle_id.unwrap_or(INVALID_ID)),
            )
        };

        // C++ buildActualPath calls Path::optimize only. optimizeGroundPath
        // belongs to buildGroundPath and must not run over this already-pruned
        // node sequence, where it could collapse the bridge-layer span again.
        self.optimizer.optimize(waypoints, layers, passability)
    }

    pub(crate) fn world_pos_for_coord(
        &self,
        coord: GridCoord,
        layer: PathfindLayerEnum,
    ) -> Coord3D {
        Self::cell_center_with_height(coord, layer, |x, y, common_layer| {
            TheTerrainLogic::get()
                .map(|terrain| terrain.get_layer_height(x, y, common_layer))
                .unwrap_or(0.0)
        })
    }

    /// Preserve the exact C++ bridge slot when sampling terrain, not generic Top.
    pub(super) fn cell_center_with_height(
        coord: GridCoord,
        layer: PathfindLayerEnum,
        sample: impl FnOnce(f32, f32, CommonPathfindLayerEnum) -> f32,
    ) -> Coord3D {
        let mut pos = coord.to_world(0.0);
        let common_layer = CommonPathfindLayerEnum::from_u32(layer as u32);
        pos.z = sample(pos.x, pos.y, common_layer);
        pos
    }

    /// C++ `Path::computePointOnPath` checks each segment using the node's
    /// stored path layer. Unlike the compatibility helper, this must not
    /// silently substitute Ground for bridge/air/other layers.
    pub fn is_line_passable_for_surfaces_on_layer(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        layer: PathfindLayerEnum,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let ignore_cells = ignored_obstacle_cells(ignore_obstacle_id);
        self.is_line_passable(
            from,
            to,
            surfaces,
            false,
            layer,
            ignore_cells.as_ref(),
            false,
        )
    }

    /// Check if line between points is passable
    /// Matches C++ Pathfinder::isLinePassable() at AIPathfind.cpp:3989-4090
    /// C++ `linePassableCallback` core used by `isLinePassable`.
    pub(crate) fn is_line_passable(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        layer: PathfindLayerEnum,
        ignore_cells: Option<&HashSet<GridCoord>>,
        allow_pinched: bool,
    ) -> bool {
        self.is_line_passable_for_object_inner(
            INVALID_ID,
            from,
            to,
            surfaces,
            is_crusher,
            layer,
            ignore_cells,
            allow_pinched,
            false,
            0,
            true,
        )
    }

    /// C++ `isLinePassable` / `linePassableCallback` with optional object occupancy.
    pub(crate) fn is_line_passable_for_object_inner(
        &self,
        obj_id: ObjectID,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        layer: PathfindLayerEnum,
        ignore_cells: Option<&HashSet<GridCoord>>,
        allow_pinched: bool,
        consider_transient: bool,
        footprint_radius: i32,
        center_in_cell: bool,
    ) -> bool {
        self.is_line_passable_for_object_with_ignore_id(
            obj_id,
            from,
            to,
            surfaces,
            is_crusher,
            layer,
            ignore_cells,
            allow_pinched,
            consider_transient,
            footprint_radius,
            center_in_cell,
            None,
        )
    }

    fn is_line_passable_for_object_with_ignore_id(
        &self,
        obj_id: ObjectID,
        from: &Coord3D,
        to: &Coord3D,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
        layer: PathfindLayerEnum,
        ignore_cells: Option<&HashSet<GridCoord>>,
        allow_pinched: bool,
        consider_transient: bool,
        footprint_radius: i32,
        center_in_cell: bool,
        ignore_obstacle_id: Option<ObjectID>,
    ) -> bool {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = (dx * dx + dy * dy).sqrt();
        if distance < 0.1 {
            return true;
        }

        let steps = (distance / (PATHFIND_CELL_SIZE_F * 0.5)).ceil() as i32;
        let steps = steps.max(1);

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let sample = Coord3D::new(from.x + dx * t, from.y + dy * t, 0.0);
            let coord = GridCoord::from_world(&sample);

            {
                let pathfinder = &self.pathfinder;
                // C++: if (!allowPinched && to->getPinched()) bail.
                if !allow_pinched && pathfinder.is_pinched(coord) == Some(true) {
                    return false;
                }
                if !pathfinder.is_passable_with_ignore(coord, surfaces, is_crusher, ignore_cells) {
                    return false;
                }
            }

            // C++ checkForMovement; bail on allyFixedCount || enemyFixed.
            if obj_id != INVALID_ID {
                let mut info = CheckMovementInfo {
                    cell: coord,
                    layer,
                    center_in_cell,
                    radius: footprint_radius,
                    consider_transient,
                    acceptable_surfaces: surfaces,
                    ..Default::default()
                };
                let can_move = match ignore_obstacle_id {
                    Some(ignore_id) => {
                        self.check_for_movement_with_ignore_id(obj_id, &mut info, ignore_id)
                    }
                    None => self.check_for_movement(obj_id, &mut info),
                };
                if !can_move {
                    return false;
                }
                if info.ally_fixed_count > 0 || info.enemy_fixed {
                    return false;
                }
            }
        }

        true
    }

    /// Check if ground path is passable
    /// Matches C++ Pathfinder::isGroundPathPassable() at AIPathfind.cpp:4065-4090
    pub(crate) fn is_ground_line_passable(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        is_crusher: bool,
        diameter: i32,
        ignore_cells: Option<&HashSet<GridCoord>>,
    ) -> bool {
        let pathfinder = &self.pathfinder;
        let radius = (diameter / 2).max(1);

        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let distance = (dx * dx + dy * dy).sqrt();

        if distance < 0.1 {
            return true;
        }

        let steps = (distance / PATHFIND_CELL_SIZE_F).ceil() as i32;

        for i in 0..=steps {
            let t = i as f32 / steps as f32;
            let center = Coord3D::new(from.x + dx * t, from.y + dy * t, 0.0);
            let center_grid = GridCoord::from_world(&center);

            // Check all cells in radius
            for rx in -radius..=radius {
                for ry in -radius..=radius {
                    let coord = GridCoord::new(center_grid.x + rx, center_grid.y + ry);
                    if !pathfinder.is_passable_with_ignore(
                        coord,
                        SURFACE_GROUND,
                        is_crusher,
                        ignore_cells,
                    ) {
                        return false;
                    }
                }
            }
        }

        true
    }

    /// Get layer for a grid coordinate (checks bridges)
    pub(crate) fn get_layer_for_coord(&self, coord: GridCoord) -> PathfindLayerEnum {
        // Check if coordinate is on a bridge
        for bridge in &self.bridges {
            if !bridge.destroyed && bridge.contains(coord) {
                return PathfindLayerEnum::Top; // Or specific layer ID
            }
        }

        PathfindLayerEnum::Ground
    }

    /// Calculate total path cost
    pub(crate) fn calculate_path_cost(&self, path: &[GridCoord]) -> u32 {
        let mut cost = 0;

        for i in 0..path.len() - 1 {
            if path[i] == path[i + 1] {
                // C++ checkChangeLayers enqueues the same XY cell at the
                // parent's cost; changing layers adds no movement distance.
                continue;
            }
            let dist = if path[i].is_diagonal(&path[i + 1]) {
                COST_DIAGONAL
            } else {
                COST_ORTHOGONAL
            };
            cost += dist;
        }

        cost
    }

    /// Check if coordinate is valid
    pub(crate) fn is_valid_coord(&self, coord: GridCoord) -> bool {
        coord.x >= 0 && coord.x < self.width as i32 && coord.y >= 0 && coord.y < self.height as i32
    }

    pub(crate) fn compute_radius_and_center(unit_radius: f32) -> (i32, bool) {
        let mut diameter = 2.0 * unit_radius;
        if diameter > PATHFIND_CELL_SIZE_F && diameter < 2.0 * PATHFIND_CELL_SIZE_F {
            diameter = 2.0 * PATHFIND_CELL_SIZE_F;
        }

        let mut radius = (diameter / PATHFIND_CELL_SIZE_F + 0.3).floor() as i32;
        let mut center_in_cell = false;
        if radius == 0 {
            radius = 1;
        }
        if (radius & 1) != 0 {
            center_in_cell = true;
        }
        radius /= 2;
        if radius > 2 {
            radius = 2;
            center_in_cell = true;
        }

        (radius, center_in_cell)
    }

    pub(crate) fn check_destination(
        &self,
        request: &PathRequest,
        cell: GridCoord,
        layer: PathfindLayerEnum,
        radius: i32,
        center_in_cell: bool,
        aircraft_goal_only: Option<bool>,
    ) -> bool {
        // Wave 262: empty dual-world → fail-closed.
        if dual_world_registry_unavailable() {
            return false;
        }

        let ignore_cells = ignored_obstacle_cells(request.ignore_obstacle_id);
        let pathfinder = &self.pathfinder;
        let center_cell = ICoord2D::new(cell.x, cell.y);
        let check_for_aircraft = aircraft_goal_only
            .unwrap_or_else(|| Self::object_uses_aircraft_goal_reservations(request.object_id));

        let mut ok = true;
        Self::for_goal_cells(center_cell, radius, center_in_cell, |coord| {
            if !ok {
                return;
            }
            if !self.is_valid_coord(coord) {
                ok = false;
                return;
            }

            if check_for_aircraft {
                let goal_aircraft = self.get_goal_aircraft(coord);
                if goal_aircraft == INVALID_ID || goal_aircraft == request.object_id {
                    return;
                }
                ok = false;
                return;
            }

            if !pathfinder.is_passable_with_ignore(
                coord,
                request.surfaces,
                request.is_crusher,
                ignore_cells.as_ref(),
            ) {
                ok = false;
                return;
            }

            let goal_unit = self.get_goal_unit(coord, layer);
            if goal_unit == INVALID_ID
                || goal_unit == request.object_id
                || request.ignore_obstacle_id == Some(goal_unit)
            {
                return;
            }

            if request.object_id == INVALID_ID {
                ok = false;
                return;
            }
            let Some(relationship) = OBJECT_REGISTRY
                .with_object(request.object_id, |obj_guard| {
                    OBJECT_REGISTRY.with_object(goal_unit, |goal_guard| {
                        obj_guard.relationship_to(&goal_guard)
                    })
                })
                .flatten()
            else {
                return;
            };
            if !request.move_allies && matches!(relationship, crate::common::Relationship::Allies) {
                ok = false;
                return;
            }
        });

        ok
    }

    pub(crate) fn for_goal_cells<F>(
        center_cell: ICoord2D,
        radius: i32,
        center_in_cell: bool,
        mut f: F,
    ) where
        F: FnMut(GridCoord),
    {
        let mut num_cells_above = radius;
        if center_in_cell {
            num_cells_above += 1;
        }

        let start_x = center_cell.x - radius;
        let end_x = center_cell.x + num_cells_above;
        let start_y = center_cell.y - radius;
        let end_y = center_cell.y + num_cells_above;

        for x in start_x..end_x {
            for y in start_y..end_y {
                f(GridCoord::new(x, y));
            }
        }
    }

    /// Add a bridge layer
    /// Matches C++ Pathfinder::addBridge() at AIPathfind.h:698
    /// C++ `Pathfinder::getAircraftPath` (AIPathfind.cpp:5781-5847).
    ///
    /// Trivial two-node path with tall-building detours for wing aircraft.
    pub fn get_aircraft_path(
        &self,
        from: &Coord3D,
        to: &Coord3D,
        check_clips: bool,
        avoid_object: ObjectID,
    ) -> PathResult {
        let radius = 100.0_f32;
        let mut adj_dest = *to;
        if check_clips {
            let mut adj = adj_dest;
            if self.circle_clips_tall_building(from, to, radius, avoid_object, &mut adj) {
                adj_dest = adj;
            }
        }
        let mut start = *from;
        start.z = to.z;
        let mut waypoints = vec![start, adj_dest];
        let mut layers = vec![PathfindLayerEnum::Ground, PathfindLayerEnum::Ground];
        let mut can_optimize = vec![true, true];

        let mut limit = 20i32;
        let mut idx = 0usize;
        while idx + 1 < waypoints.len() && limit >= 0 {
            let cur = waypoints[idx];
            let mut next = waypoints[idx + 1];
            let mut n1 = Coord3D::new(0.0, 0.0, 0.0);
            let mut n2 = Coord3D::new(0.0, 0.0, 0.0);
            let mut n3 = Coord3D::new(0.0, 0.0, 0.0);
            if self.segment_intersects_tall_building(
                &cur,
                &mut next,
                avoid_object,
                &mut n1,
                &mut n2,
                &mut n3,
            ) {
                // C++ appends n3, n2, n1 after cur before next — insert in path order n1,n2,n3
                // After cur->append(n3); append(n2); append(n1) on linked list with reverse prepend semantics...
                // Looking at C++: curNode->append(newNode3); append(newNode2); append(newNode1)
                // so order is cur -> n1 -> n2 -> n3 -> next (if append inserts after current sequentially
                // Actually in their PathNode, append likely adds as next of cur, so last append is closest next.
                // First append n3: cur->n3->oldNext
                // append n2 on cur: cur->n2->n3->oldNext
                // append n1 on cur: cur->n1->n2->n3->oldNext
                // So path order: cur, n1, n2, n3, next
                waypoints[idx + 1] = next; // may have been adjusted
                waypoints.insert(idx + 1, n3);
                waypoints.insert(idx + 1, n2);
                waypoints.insert(idx + 1, n1);
                layers.insert(idx + 1, PathfindLayerEnum::Ground);
                layers.insert(idx + 1, PathfindLayerEnum::Ground);
                layers.insert(idx + 1, PathfindLayerEnum::Ground);
                can_optimize.insert(idx + 1, true);
                can_optimize.insert(idx + 1, true);
                can_optimize.insert(idx + 1, true);
                // C++ continues from newNode2 which is n2 at idx+2 after inserts of n1,n2,n3
                idx += 2;
            } else {
                waypoints[idx + 1] = next;
                idx += 1;
            }
            limit -= 1;
        }

        PathResult {
            success: waypoints.len() >= 2,
            waypoints,
            layers,
            can_optimize,
            total_cost: 0,
            blocked_by_ally: false,
        }
    }

    pub fn add_bridge(&mut self, bounds: (GridCoord, GridCoord)) -> u32 {
        self.add_bridge_ex(bounds, INVALID_ID, bounds.0, bounds.1)
    }

    /// Add bridge with object id + attach cells (for findBrokenBridge / connectsZones).
    pub fn add_bridge_ex(
        &mut self,
        bounds: (GridCoord, GridCoord),
        bridge_object_id: ObjectID,
        start_cell: GridCoord,
        end_cell: GridCoord,
    ) -> u32 {
        // C++ Pathfinder::addBridge: first unused slot in 2..=14 (LAYER_GROUND+1 .. LAYER_WALL-1).
        let Some(layer_id) = (2u32..=14).find(|id| !self.bridges.iter().any(|b| b.layer_id == *id))
        else {
            return 1; // C++ returns LAYER_GROUND when out of bridge slots.
        };
        let mut layer =
            BridgeLayer::with_meta(layer_id, bounds, bridge_object_id, start_cell, end_cell);
        // C++ classifyCells entry points: bridge ends + edge spans (isCellEntryPoint).
        layer.set_ground_connect_cells(Self::bridge_entry_cells(bounds, start_cell, end_cell));
        self.bridges.push(layer);
        let idx = self.bridges.len() - 1;
        self.classify_bridge_cells(idx);
        self.sync_bridge_layers_to_pathfinder();
        layer_id
    }

    /// Build ground-connect cell list for a bridge layer.
    /// Prefer explicit start/end attach cells; also include end-edge cells in bounds
    /// so connectsZones scans more than two points (closer to full layer table).
    pub(crate) fn bridge_entry_cells(
        bounds: (GridCoord, GridCoord),
        start_cell: GridCoord,
        end_cell: GridCoord,
    ) -> Vec<GridCoord> {
        let mut cells = Vec::new();
        let push = |cells: &mut Vec<GridCoord>, c: GridCoord| {
            if !cells.contains(&c) {
                cells.push(c);
            }
        };
        push(&mut cells, start_cell);
        push(&mut cells, end_cell);
        let lo = bounds.0;
        let hi = bounds.1;
        // End rows (y = lo.y and y = hi.y) — typical bridge entry spans.
        for x in lo.x..=hi.x {
            push(&mut cells, GridCoord::new(x, lo.y));
            push(&mut cells, GridCoord::new(x, hi.y));
        }
        // End columns if bridge is axis-aligned the other way.
        for y in lo.y..=hi.y {
            push(&mut cells, GridCoord::new(lo.x, y));
            push(&mut cells, GridCoord::new(hi.x, y));
        }
        cells
    }

    pub(crate) fn zone_at_cell(&self, cell: GridCoord) -> u16 {
        let zones = &self.zones;
        zones.zone_at(cell)
    }

    /// C++ `Pathfinder::findBrokenBridge` layer pass (m_layers isDestroyed + connectsZones).
    /// C++ `Pathfinder::findBrokenBridge` layer scan body.
    ///
    /// zone1/zone2 from ground cells at from/to; if equal, no broken bridge.
    /// Else first destroyed layer with connectsZones(zone1,zone2) and a bridge id.
    pub fn find_broken_bridge_layer(&self, from: &Coord3D, to: &Coord3D) -> Option<ObjectID> {
        let from_c = GridCoord::from_world(from);
        let to_c = GridCoord::from_world(to);
        let zone1 = self.zone_at_cell(from_c);
        let zone2 = self.zone_at_cell(to_c);
        // C++: if (zone1 == zone2) return false;
        if zone1 == zone2 {
            return None;
        }
        for bridge in &self.bridges {
            if !bridge.destroyed {
                continue;
            }
            if !bridge.connects_zones(|c| self.zone_at_cell(c), zone1, zone2) {
                continue;
            }
            if bridge.bridge_object_id != INVALID_ID {
                return Some(bridge.bridge_object_id);
            }
        }
        None
    }
    pub fn set_bridge_destroyed(&mut self, layer_id: u32, destroyed: bool) {
        if let Some(bridge) = self.bridges.iter_mut().find(|b| b.layer_id == layer_id) {
            if bridge.destroyed != destroyed {
                bridge.destroyed = destroyed;
                bridge.reclassify_cells();
            }
        }
    }

    /// Find a bridge layer by its assigned pathfinder layer id.
    pub fn bridge_by_layer_id(&self, layer_id: u32) -> Option<&BridgeLayer> {
        self.bridges
            .iter()
            .find(|bridge| bridge.layer_id == layer_id)
    }

    /// Clear path cache
    pub fn clear_cache(&mut self) {
        self.path_cache.clear();
    }

    /// Set cell type at world position
    pub fn set_cell_type(&mut self, pos: &Coord3D, cell_type: PathfindCellType) {
        let coord = GridCoord::from_world(pos);
        self.pathfinder.set_cell_type(coord, cell_type);
    }

    /// Get cell type at world position.
    ///
    /// Ground A* grid via `GridCoord::from_world` (floor). Prefer
    /// `get_cell_type_at_layer` for C++ `getCell(layer, REAL_TO_INT(x/size))`.
    pub fn get_cell_type(&self, pos: &Coord3D) -> Option<PathfindCellType> {
        let coord = GridCoord::from_world(pos);
        let pathfinder = &self.pathfinder;
        pathfinder.get_cell_type(coord)
    }

    /// C++ `REAL_TO_INT(pos / PATHFIND_CELL_SIZE)` — truncate toward zero.
    ///
    /// Distinct from `GridCoord::from_world` (`REAL_TO_INT_FLOOR`).
    /// diesOnBadLand (ObjectCreationList.cpp) uses this index with `getCell`.
    #[inline]
    pub fn world_to_cell_trunc(pos: &Coord3D) -> GridCoord {
        GridCoord::new(
            (pos.x / PATHFIND_CELL_SIZE_F) as i32,
            (pos.y / PATHFIND_CELL_SIZE_F) as i32,
        )
    }

    /// C++ `Pathfinder::getCell(layer, cellX, cellY)->getType()`.
    ///
    /// Out-of-extent reads return `None`. Ground uses the A* grid. Elevated
    /// layers look up their exact `m_layers[layer]` slot; `Top` is the first
    /// bridge ID, not a wildcard. If that exact slot has no cell here, C++
    /// `getCell` falls back to the ground map cell.
    pub fn get_cell_type_at_cell(
        &self,
        layer: PathfindLayerEnum,
        cell_x: i32,
        cell_y: i32,
    ) -> Option<PathfindCellType> {
        let coord = GridCoord::new(cell_x, cell_y);
        if !self.is_valid_coord(coord) {
            return None;
        }
        if !matches!(
            layer,
            PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid
        ) {
            let layer_id = layer as u32;
            let bridge = self
                .bridges
                .iter()
                .find(|b| b.contains(coord) && b.layer_id == layer_id);
            if let Some(cell_type) = bridge.and_then(|b| b.cell_type_at(coord)) {
                return Some(cell_type);
            }
        }
        let pathfinder = &self.pathfinder;
        pathfinder.get_cell_type(coord)
    }

    /// C++ `getCell(layer, REAL_TO_INT(pos.x/SIZE), REAL_TO_INT(pos.y/SIZE))`.
    pub fn get_cell_type_at_layer(
        &self,
        pos: &Coord3D,
        layer: PathfindLayerEnum,
    ) -> Option<PathfindCellType> {
        let c = Self::world_to_cell_trunc(pos);
        self.get_cell_type_at_cell(layer, c.x, c.y)
    }

    /// Update a ground or bridge-layer cell without aliasing bridge IDs onto ground.
    pub fn set_cell_type_at_layer(
        &mut self,
        pos: &Coord3D,
        layer: PathfindLayerEnum,
        cell_type: PathfindCellType,
    ) {
        if matches!(
            layer,
            PathfindLayerEnum::Invalid | PathfindLayerEnum::Ground
        ) {
            self.set_cell_type(pos, cell_type);
        } else {
            let coord = Self::world_to_cell_trunc(pos);
            let layer_id = layer as u32;
            let bridge_updated = self
                .bridges
                .iter_mut()
                .find(|bridge| bridge.contains(coord) && bridge.layer_id == layer_id)
                .filter(|bridge| bridge.cell_type_at(coord).is_some())
                .map(|bridge| {
                    bridge.cell_types.insert(coord, cell_type);
                })
                .is_some();
            if bridge_updated {
                self.pathfinder.set_cell_type_on_layer(
                    coord,
                    PathfindLayerEnum::from_u32(layer_id),
                    cell_type,
                );
            } else {
                self.set_cell_type(pos, cell_type);
            }
        }
        self.clear_cache();
        self.mark_zones_dirty();
    }

    /// Pure zone connectivity (C++ zone1 == zone2 / UNINITIALIZED → true).
    pub fn zones_connected_for_surfaces(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        let from_c = GridCoord::from_world(from);
        let to_c = GridCoord::from_world(to);
        let zones = &self.zones;
        zones.are_connected(from_c, to_c, surfaces, false)
    }

    /// C++ `Pathfinder::clientSafeQuickDoesPathExist` (AIPathfind.cpp).
    ///
    /// Zone connectivity only — not a full A* path. False = impossible terrain;
    /// true = terrain-possible (units may still block).
    pub fn client_safe_quick_does_path_exist(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        // C++ validMovementPosition(false, destLayer, locoSet, to)
        if !self.valid_movement_position(surfaces, false, to, None) {
            return false;
        }
        // C++: no goals on cliffs
        if self.get_cell_type(to) == Some(PathfindCellType::Cliff) {
            return false;
        }
        self.zones_connected_for_surfaces(surfaces, from, to)
    }

    /// C++ `Pathfinder::clientSafeQuickDoesPathExistForUI`.
    ///
    /// Ignores structure obstacles for UI feedback (terrain zones only).
    pub fn client_safe_quick_does_path_exist_for_ui(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        from: &Coord3D,
        to: &Coord3D,
    ) -> bool {
        if self.get_cell_type(to) == Some(PathfindCellType::Cliff) {
            return false;
        }
        self.zones_connected_for_surfaces(surfaces, from, to)
    }

    /// C++ `computeNormalRadialOffset` helper (AIPathfind.cpp:9433-9458).
    pub fn compute_normal_radial_offset(
        from: &Coord3D,
        insert: &mut Coord3D,
        to: &Coord3D,
        obj_pos: &Coord3D,
        radius: f32,
    ) {
        let dx = to.x - from.x;
        let dy = to.y - from.y;
        let obj_dx = obj_pos.x - from.x;
        let obj_dy = obj_pos.y - from.y;
        let cross = dx * obj_dy - dy * obj_dx;
        let mut nx;
        let mut ny;
        if cross > 0.0 {
            nx = dy;
            ny = -dx;
        } else {
            nx = -dy;
            ny = dx;
        }
        let len = (nx * nx + ny * ny).sqrt();
        if len > 0.0001 {
            nx /= len;
            ny /= len;
        } else {
            nx = 1.0;
            ny = 0.0;
        }
        insert.x = obj_pos.x + nx * radius;
        insert.y = obj_pos.y + ny * radius;
        insert.z = obj_pos.z;
    }
}
