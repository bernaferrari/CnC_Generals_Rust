use super::*;

impl PathfindingGrid {
    pub(super) fn terrain_zone_passable(ty: PathfindCellType) -> bool {
        // Terrain-only: structures (Obstacle) are ignored for UI zones.
        !matches!(
            ty,
            PathfindCellType::Water
                | PathfindCellType::Cliff
                | PathfindCellType::Impassable
                | PathfindCellType::BridgeImpassable
        )
    }

    /// Flood-fill terrain zones ignoring structure obstacles (C++ effectiveTerrainZone).
    pub fn rebuild_terrain_zones(&mut self) {
        let cells = self.terrain_zones.len();
        self.terrain_zones.fill(0);
        let mut next_zone = 1u16;
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let start = GridPos::new(x, y);
                let Some(sidx) = self.bit_index(start) else {
                    continue;
                };
                if self.terrain_zones[sidx] != 0 {
                    continue;
                }
                if !Self::terrain_zone_passable(self.cell_type_at_index(sidx)) {
                    continue;
                }
                let zone = next_zone;
                next_zone = next_zone.saturating_add(1);
                let mut stack = vec![start];
                while let Some(cur) = stack.pop() {
                    let Some(idx) = self.bit_index(cur) else {
                        continue;
                    };
                    if self.terrain_zones[idx] != 0 {
                        continue;
                    }
                    if !Self::terrain_zone_passable(self.cell_type_at_index(idx)) {
                        continue;
                    }
                    self.terrain_zones[idx] = zone;
                    stack.push(GridPos::new(cur.x + 1, cur.y));
                    stack.push(GridPos::new(cur.x - 1, cur.y));
                    stack.push(GridPos::new(cur.x, cur.y + 1));
                    stack.push(GridPos::new(cur.x, cur.y - 1));
                }
                let _ = cells;
            }
        }
        let mut zones = std::mem::take(&mut self.terrain_zones);
        self.merge_zones_via_connect_layer(&mut zones);
        self.terrain_zones = zones;
    }

    pub fn terrain_zone(&self, pos: GridPos) -> u16 {
        self.bit_index(pos)
            .and_then(|idx| self.terrain_zones.get(idx).copied())
            .unwrap_or(0)
    }

    /// C++ Pathfinder::clientSafeQuickDoesPathExistForUI (AIPathfind.cpp:8055).
    pub fn quick_path_exists_for_ui(&self, from: Vec3, to: Vec3) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if self.cell_type(goal) == PathfindCellType::Cliff {
            return false;
        }
        let z1 = self.terrain_zone(start);
        let z2 = self.terrain_zone(goal);
        // C++ UNINITIALIZED_ZONE → false-positive true.
        if z1 == 0 || z2 == 0 {
            return true;
        }
        z1 == z2
    }

    /// Flood kind for structure-aware zones: same-type connectivity.
    /// C++ `calculateZones` joins only equal `getType()`; leftover
    /// `flood_fill_type`. Rubble is not Clear. Impassable / Obstacle /
    /// BridgeImpassable get real zone ids (not zone 0).
    pub(super) fn path_zone_flood_kind(ty: PathfindCellType) -> Option<u8> {
        Some(ty as u8)
    }

    pub(super) fn pair_water_ground(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Water)
                | (PathfindCellType::Water, PathfindCellType::Clear)
        )
    }

    pub(super) fn pair_ground_cliff(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Cliff)
                | (PathfindCellType::Cliff, PathfindCellType::Clear)
        )
    }

    pub(super) fn pair_ground_rubble(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Rubble)
                | (PathfindCellType::Rubble, PathfindCellType::Clear)
        )
    }

    /// C++ `crusherGround` — CELL_OBSTACLE-with-isObstacleFence <-> CELL_CLEAR.
    pub(super) fn pair_crusher_ground(
        a: PathfindCellType,
        b: PathfindCellType,
        a_fence: bool,
        b_fence: bool,
    ) -> bool {
        (a == PathfindCellType::Obstacle && a_fence && b == PathfindCellType::Clear)
            || (b == PathfindCellType::Obstacle && b_fence && a == PathfindCellType::Clear)
    }

    /// C++ `PathfindZoneManager::calculateZones` resolveZones
    /// (`AIPathfind.cpp:2629-2633`): CLEAR ground cells with
    /// `getConnectLayer() > LAYER_GROUND` merge with that bridge layer's
    /// zone in `m_hierarchicalZones`. Leftover `build_surface_combiners`
    /// already does this; live floods store the effective id on the cell.
    pub(super) fn merge_zones_via_connect_layer(&self, zones: &mut [u16]) {
        if zones.len() != self.ground_connect.len() {
            return;
        }
        let mut max_z = 0u16;
        for &z in zones.iter() {
            if z > max_z {
                max_z = z;
            }
        }
        if max_z == 0 {
            return;
        }
        let mut parent: Vec<u16> = (0..=max_z).collect();
        fn find(parent: &mut [u16], mut z: u16) -> u16 {
            while (z as usize) < parent.len() && parent[z as usize] != z {
                let p = parent[z as usize];
                if (p as usize) < parent.len() {
                    parent[z as usize] = parent[p as usize];
                }
                z = p;
            }
            z
        }
        fn union(parent: &mut [u16], a: u16, b: u16) {
            if a == 0 || b == 0 {
                return;
            }
            let pa = find(parent, a);
            let pb = find(parent, b);
            if pa == pb {
                return;
            }
            // C++ resolveZones keeps the lower zone id.
            if pa < pb {
                parent[pb as usize] = pa;
            } else {
                parent[pa as usize] = pb;
            }
        }
        // Union every CLEAR ground cell that shares a connectLayer
        // (equivalent to resolveZones(cell.zone, layer.zone)).
        let mut layer_rep = [0u16; 16];
        for (idx, &cl) in self.ground_connect.iter().enumerate() {
            if cl <= PathfindLayerEnum::Ground as u8 || (cl as usize) >= layer_rep.len() {
                continue;
            }
            if self.cell_type_at_index(idx) != PathfindCellType::Clear {
                continue;
            }
            let z = zones.get(idx).copied().unwrap_or(0);
            if z == 0 {
                continue;
            }
            let slot = cl as usize;
            if layer_rep[slot] == 0 {
                layer_rep[slot] = z;
            } else {
                union(&mut parent, layer_rep[slot], z);
            }
        }
        for z in zones.iter_mut() {
            if *z != 0 {
                *z = find(&mut parent, *z);
            }
        }
    }

    /// Structure-aware zones (C++ clientSafeQuickDoesPathExist).
    pub fn rebuild_path_zones(&mut self) {
        self.path_zones.fill(0);
        let mut next_zone = 1u16;
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let start = GridPos::new(x, y);
                let Some(sidx) = self.bit_index(start) else {
                    continue;
                };
                if self.path_zones[sidx] != 0 {
                    continue;
                }
                let Some(kind) = Self::path_zone_flood_kind(self.cell_type_at_index(sidx)) else {
                    continue;
                };
                let zone = next_zone;
                next_zone = next_zone.saturating_add(1);
                let mut stack = vec![start];
                while let Some(cur) = stack.pop() {
                    let Some(idx) = self.bit_index(cur) else {
                        continue;
                    };
                    if self.path_zones[idx] != 0 {
                        continue;
                    }
                    if Self::path_zone_flood_kind(self.cell_type_at_index(idx)) != Some(kind) {
                        continue;
                    }
                    self.path_zones[idx] = zone;
                    stack.push(GridPos::new(cur.x + 1, cur.y));
                    stack.push(GridPos::new(cur.x - 1, cur.y));
                    stack.push(GridPos::new(cur.x, cur.y + 1));
                    stack.push(GridPos::new(cur.x, cur.y - 1));
                }
            }
        }
        let mut zones = std::mem::take(&mut self.path_zones);
        self.merge_zones_via_connect_layer(&mut zones);
        self.path_zones = zones;
        self.build_surface_combiners();
    }

    /// True when `goal` is sealed off from `start` and every blocking cell on
    /// the separating boundary is a CELL_OBSTACLE structure (no terrain
    /// Impassable / BridgeImpassable). C++ `AIUpdateInterface::computePath`
    /// (AIUpdate.cpp:1713-1717) walks a unit to the closest reachable cell
    /// when `Pathfinder::findPath` fails — the classic click-on-a-building
    /// case `Pathfinder::findClosestPath` exists for. A boundary that mixes
    /// terrain walls with a doze-able obstacle gap is terrain-sealed: the
    /// pathfinder fails closed like `Pathfinder::findPath` itself
    /// (AIPathfind.cpp:6364-6436) so non-dozers cannot dozerHack the gap.
    pub(super) fn structure_sealed_goal(&self, start: GridPos, goal: GridPos) -> bool {
        if !self.is_valid_pos(start) || !self.is_valid_pos(goal) {
            return false;
        }
        let blocking = |p: GridPos| {
            matches!(
                self.cell_type(p),
                PathfindCellType::Obstacle
                    | PathfindCellType::Impassable
                    | PathfindCellType::BridgeImpassable
            )
        };
        let mut visited = vec![false; (self.width * self.height) as usize];
        let mut stack = vec![start];
        if let Some(idx) = self.bit_index(start) {
            visited[idx] = true;
        }
        let mut boundary: Vec<GridPos> = Vec::new();
        while let Some(cur) = stack.pop() {
            if cur == goal {
                return false;
            }
            for n in [
                GridPos::new(cur.x + 1, cur.y),
                GridPos::new(cur.x - 1, cur.y),
                GridPos::new(cur.x, cur.y + 1),
                GridPos::new(cur.x, cur.y - 1),
            ] {
                if !self.is_valid_pos(n) {
                    continue;
                }
                let Some(idx) = self.bit_index(n) else {
                    continue;
                };
                if visited[idx] {
                    continue;
                }
                if blocking(n) {
                    if !boundary.contains(&n) {
                        boundary.push(n);
                    }
                    continue;
                }
                visited[idx] = true;
                stack.push(n);
            }
        }
        !boundary.is_empty()
            && boundary
                .iter()
                .all(|c| self.cell_type(*c) == PathfindCellType::Obstacle)
    }


    /// Leftover `ZoneManager::build_surface_combiners` (GROUND+WATER / GROUND+CLIFF).
    pub(super) fn build_surface_combiners(&mut self) {
        let mut max_z = 0u16;
        for &z in &self.path_zones {
            if z > max_z {
                max_z = z;
            }
        }
        let n = max_z as usize + 1;
        let mut water: Vec<u16> = (0..n as u16).collect();
        let mut cliff: Vec<u16> = (0..n as u16).collect();
        let mut rubble: Vec<u16> = (0..n as u16).collect();
        let mut crusher: Vec<u16> = (0..n as u16).collect();
        let resolve = |table: &mut [u16], a: u16, b: u16| {
            if a == 0 || b == 0 || a == b {
                return;
            }
            let za = table.get(a as usize).copied().unwrap_or(a);
            let zb = table.get(b as usize).copied().unwrap_or(b);
            if za == zb {
                return;
            }
            let final_z = za.min(zb);
            for z in table.iter_mut() {
                if *z == za || *z == zb {
                    *z = final_z;
                }
            }
        };
        let w = self.width;
        let h = self.height;
        for y in 0..h {
            for x in 0..w {
                let Some(idx) = self.bit_index(GridPos::new(x, y)) else {
                    continue;
                };
                let z1 = self.path_zones[idx];
                if z1 == 0 {
                    continue;
                }
                let t1 = self.cell_type_at_index(idx);
                if x > 0 {
                    if let Some(nidx) = self.bit_index(GridPos::new(x - 1, y)) {
                        let z0 = self.path_zones[nidx];
                        let t0 = self.cell_type_at_index(nidx);
                        if z0 != 0 && z0 != z1 {
                            if Self::pair_crusher_ground(
                                t0,
                                t1,
                                Self::bit_test(&self.fence_bits, nidx),
                                Self::bit_test(&self.fence_bits, idx),
                            ) {
                                resolve(&mut crusher, z0, z1);
                            }
                            if Self::pair_water_ground(t0, t1) {
                                resolve(&mut water, z0, z1);
                            } else if Self::pair_ground_rubble(t0, t1) {
                                resolve(&mut rubble, z0, z1);
                            } else if Self::pair_ground_cliff(t0, t1) {
                                resolve(&mut cliff, z0, z1);
                            }
                        }
                    }
                }
                if y > 0 {
                    if let Some(nidx) = self.bit_index(GridPos::new(x, y - 1)) {
                        let z0 = self.path_zones[nidx];
                        let t0 = self.cell_type_at_index(nidx);
                        if z0 != 0 && z0 != z1 {
                            if Self::pair_crusher_ground(
                                t0,
                                t1,
                                Self::bit_test(&self.fence_bits, nidx),
                                Self::bit_test(&self.fence_bits, idx),
                            ) {
                                resolve(&mut crusher, z0, z1);
                            }
                            if Self::pair_water_ground(t0, t1) {
                                resolve(&mut water, z0, z1);
                            } else if Self::pair_ground_rubble(t0, t1) {
                                resolve(&mut rubble, z0, z1);
                            } else if Self::pair_ground_cliff(t0, t1) {
                                resolve(&mut cliff, z0, z1);
                            }
                        }
                    }
                }
            }
        }
        self.ground_water_zones = water;
        self.ground_cliff_zones = cliff;
        self.ground_rubble_zones = rubble;
        self.crusher_zones = crusher;
    }

    /// C++ `PathfindZoneManager::getEffectiveZone` (AIPathfind.cpp:3118).
    /// Crusher maps `m_crusherZones` BEFORE ground-cliff/water/rubble lookups.
    pub(super) fn get_effective_zone(&self, surfaces: u32, crusher: bool, mut zone: u16) -> u16 {
        if zone == 0 {
            return 0;
        }
        if (surfaces & SURFACE_AIR) != 0 {
            return 1;
        }
        if (surfaces & SURFACE_GROUND) != 0
            && (surfaces & SURFACE_WATER) != 0
            && (surfaces & SURFACE_CLIFF) != 0
        {
            return 1;
        }
        if crusher {
            if let Some(&z) = self.crusher_zones.get(zone as usize) {
                zone = z;
            }
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_CLIFF) != 0 {
            return self
                .ground_cliff_zones
                .get(zone as usize)
                .copied()
                .unwrap_or(zone);
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_WATER) != 0 {
            return self
                .ground_water_zones
                .get(zone as usize)
                .copied()
                .unwrap_or(zone);
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_RUBBLE) != 0 {
            return self
                .ground_rubble_zones
                .get(zone as usize)
                .copied()
                .unwrap_or(zone);
        }
        zone
    }

    pub fn path_zone(&self, pos: GridPos) -> u16 {
        self.bit_index(pos)
            .and_then(|idx| self.path_zones.get(idx).copied())
            .unwrap_or(0)
    }

    /// Leftover `ZoneManager::are_connected` — effective-zone compare.
    pub(super) fn zones_connected(
        &self,
        start: GridPos,
        goal: GridPos,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        if !self.is_valid_pos(start) || !self.is_valid_pos(goal) {
            return false;
        }
        let z1 = self.path_zone(start);
        let z2 = self.path_zone(goal);
        if z1 == 0 || z2 == 0 {
            return true;
        }
        self.get_effective_zone(surfaces, is_crusher, z1)
            == self.get_effective_zone(surfaces, is_crusher, z2)
    }

    /// C++ Pathfinder::clientSafeQuickDoesPathExist (structure-aware, ground).
    pub fn quick_path_exists(&self, from: Vec3, to: Vec3) -> bool {
        self.quick_path_exists_for(from, to, SURFACE_GROUND)
    }

    /// C++ `clientSafeQuickDoesPathExist` with locomotor surfaces.
    pub fn quick_path_exists_for(&self, from: Vec3, to: Vec3, surfaces: u32) -> bool {
        self.quick_path_exists_for_crusher(from, to, surfaces, false)
    }

    /// C++ `clientSafeQuickDoesPathExist` with surfaces + crusher combiners.
    pub fn quick_path_exists_for_crusher(
        &self,
        from: Vec3,
        to: Vec3,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if self.cell_type(goal) == PathfindCellType::Cliff {
            return false;
        }
        // C++ validMovementPosition: water dest needs WATER or AIR.
        if self.cell_type(goal) == PathfindCellType::Water
            && (surfaces & (SURFACE_WATER | SURFACE_AIR)) == 0
        {
            return false;
        }
        if self.cell_type(goal) == PathfindCellType::Obstacle && !self.is_obstacle_fence(goal) {
            return false;
        }
        let z1 = self.path_zone(start);
        let z2 = self.path_zone(goal);
        if z1 == 0 || z2 == 0 {
            // Uninitialized: treat as possible (C++ UNINITIALIZED_ZONE).
            return true;
        }
        self.get_effective_zone(surfaces, is_crusher, z1)
            == self.get_effective_zone(surfaces, is_crusher, z2)
    }

    /// C++ Pathfinder::classifyMapCell (AIPathfind.cpp:4491-4521).
    /// Cliff at the cell top-left; water if any of 4 corners — water wins.
    /// No terrain-slope Impassable gate.
    pub fn classify_map_cell(cliff_top_left: bool, water_any_corner: bool) -> PathfindCellType {
        let mut ty = PathfindCellType::Clear;
        if cliff_top_left {
            ty = PathfindCellType::Cliff;
        }
        if water_any_corner {
            ty = PathfindCellType::Water;
        }
        ty
    }

    /// C++ `PathfindLayer::classifyCells` / `classifyLayerMapCell`.
    /// Deck lives on its own layer (CLEAR); sides are BRIDGE_IMPASSABLE;
    /// only end/entry cells connect to LAYER_GROUND. Does **not** flatten
    /// the deck onto `m_map`. Low-clearance ground under the deck is stamped
    /// BRIDGE_IMPASSABLE. Destroyed: layer cells become BRIDGE_IMPASSABLE and
    /// ground connects are dropped — water/ground under the span stays.
    pub fn stamp_bridge_deck(
        &mut self,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
        destroyed: bool,
    ) {
        let corners = [from_left, from_right, to_right, to_left];
        let mut min_x = f32::MAX;
        let mut max_x = f32::MIN;
        let mut min_z = f32::MAX;
        let mut max_z = f32::MIN;
        for c in corners {
            min_x = min_x.min(c.x);
            max_x = max_x.max(c.x);
            min_z = min_z.min(c.z);
            max_z = max_z.max(c.z);
        }
        // Entry/side lines sit up to ~1 cell outside the deck AABB.
        let pad = self.grid_size;
        let lo = self.world_to_grid(Vec3::new(min_x - pad, 0.0, min_z - pad));
        let hi = self.world_to_grid(Vec3::new(max_x + pad, 0.0, max_z + pad));
        let layer_id = self.alloc_or_find_bridge_layer(from_left, from_right, to_left, to_right);
        self.disconnect_ground_from_layer(layer_id);

        let mut cells: HashMap<(i32, i32), (PathfindCellType, u8)> = HashMap::new();
        for y in lo.y.min(hi.y)..=lo.y.max(hi.y) {
            for x in lo.x.min(hi.x)..=lo.x.max(hi.x) {
                let pos = GridPos::new(x, y);
                if !self.is_valid_pos(pos) {
                    continue;
                }
                let Some((ty, connect)) = self.classify_layer_map_cell(
                    pos, &corners, from_left, from_right, to_left, to_right,
                ) else {
                    continue;
                };
                if connect == PathfindLayerEnum::Ground as u8 {
                    if let Some(idx) = self.bit_index(pos) {
                        if let Some(slot) = self.ground_connect.get_mut(idx) {
                            *slot = layer_id;
                        }
                    }
                }
                // C++ classifyLayerMapCell clearance (AIPathfind.cpp:3711-3721).
                if connect != PathfindLayerEnum::Ground as u8 {
                    let center = self.cell_center_xz(pos);
                    let deck_h = bridge_deck_height(&corners, center.0, center.1);
                    let ground_h = sample_host_ground_height(center.0, center.1);
                    if ground_h + LAYER_Z_CLOSE_ENOUGH_F > deck_h
                        && self.cell_type(pos) != PathfindCellType::Obstacle
                    {
                        self.set_cell_type(pos, PathfindCellType::BridgeImpassable);
                    }
                }
                cells.insert((x, y), (ty, connect));
            }
        }

        let ground_connect_cells: Vec<(i32, i32)> = cells
            .iter()
            .filter(|(_, (_, connect))| *connect == PathfindLayerEnum::Ground as u8)
            .map(|(&(x, y), _)| (x, y))
            .collect();

        if destroyed {
            // C++ classifyCells m_destroyed: every layer cell BRIDGE_IMPASSABLE,
            // drop ground connect (AIPathfind.cpp:3504-3519).
            self.disconnect_ground_from_layer(layer_id);
            for value in cells.values_mut() {
                *value = (PathfindCellType::BridgeImpassable, 0);
            }
        }

        if let Some(layer) = self
            .bridge_layers
            .iter_mut()
            .find(|layer| layer.id == layer_id)
        {
            layer.from_left = from_left;
            layer.from_right = from_right;
            layer.to_left = to_left;
            layer.to_right = to_right;
            layer.cells = cells;
            layer.destroyed = destroyed;
            if !ground_connect_cells.is_empty() {
                layer.ground_connect_cells = ground_connect_cells;
            }
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `PathfindLayer::getCell` type (NULL/IMPASSABLE → None).
    pub fn layer_cell_type(&self, layer: u8, pos: GridPos) -> Option<PathfindCellType> {
        if layer == LAYER_WALL_ID {
            return self.wall_cells.get(&(pos.x, pos.y)).copied();
        }
        self.bridge_layers
            .iter()
            .find(|l| l.id == layer)
            .and_then(|l| l.cells.get(&(pos.x, pos.y)).map(|(ty, _)| *ty))
    }

    /// C++ `Pathfinder::isPointOnWall` (AIPathfind.cpp:3929-3942).
    pub fn is_point_on_wall(&self, pos: Vec3) -> bool {
        if self.wall_pieces.is_empty() || self.wall_cells.is_empty() {
            return false;
        }
        let cell = self.world_to_grid(pos);
        matches!(
            self.wall_cells.get(&(cell.x, cell.y)),
            Some(PathfindCellType::Clear)
        )
    }

    pub fn wall_height(&self) -> f32 {
        self.wall_height
    }

    pub fn set_wall_height(&mut self, h: f32) {
        self.wall_height = h;
    }

    pub fn wall_piece_count(&self) -> usize {
        self.wall_pieces.len()
    }

    pub(super) fn wall_piece_contains(piece: &HostWallPiece, x: f32, z: f32) -> bool {
        // C++ PathfindLayer::isPointOnWall — Cos/Sin(-orientation).
        let ori = -piece.orientation;
        let (s, c) = ori.sin_cos();
        let ptx = x - piece.pos_x;
        let ptz = z - piece.pos_z;
        let ptx_new = (ptx * c - ptz * s).abs();
        let ptz_new = (ptx * s + ptz * c).abs();
        ptx_new <= piece.major && ptz_new <= piece.minor
    }

    pub(super) fn wall_piece_aabb(piece: &HostWallPiece) -> (f32, f32, f32, f32) {
        let (s, c) = piece.orientation.sin_cos();
        let mut lo_x = f32::MAX;
        let mut lo_z = f32::MAX;
        let mut hi_x = f32::MIN;
        let mut hi_z = f32::MIN;
        for &sx in &[-piece.major, piece.major] {
            for &sz in &[-piece.minor, piece.minor] {
                let x = piece.pos_x + sx * c - sz * s;
                let z = piece.pos_z + sx * s + sz * c;
                lo_x = lo_x.min(x);
                lo_z = lo_z.min(z);
                hi_x = hi_x.max(x);
                hi_z = hi_z.max(z);
            }
        }
        (lo_x, lo_z, hi_x, hi_z)
    }

    pub(super) fn wall_corner_count(&self, pos: GridPos, pieces: &[HostWallPiece]) -> u32 {
        let tl = self.grid_to_world(pos);
        let s = self.grid_size;
        let pts = [
            (tl.x, tl.z),
            (tl.x, tl.z + s),
            (tl.x + s, tl.z + s),
            (tl.x + s, tl.z),
        ];
        pts.iter()
            .filter(|(x, z)| pieces.iter().any(|p| Self::wall_piece_contains(p, *x, *z)))
            .count() as u32
    }

    /// C++ `allocateCellsForWallLayer` + `classifyWallCells` (AIPathfind.cpp:3386-3583).
    pub fn allocate_and_classify_wall_layer(&mut self) {
        self.wall_cells.clear();
        if self.wall_pieces.is_empty() {
            self.terrain_gen = self.terrain_gen.wrapping_add(1);
            return;
        }
        let mut lo_x = f32::MAX;
        let mut lo_z = f32::MAX;
        let mut hi_x = f32::MIN;
        let mut hi_z = f32::MIN;
        for piece in &self.wall_pieces {
            let (a, b, c, d) = Self::wall_piece_aabb(piece);
            lo_x = lo_x.min(a);
            lo_z = lo_z.min(b);
            hi_x = hi_x.max(c);
            hi_z = hi_z.max(d);
        }
        let pad = self.grid_size / 100.0;
        let mut min_cell = self.world_to_grid(Vec3::new(lo_x - pad, 0.0, lo_z - pad));
        let mut max_cell = self.world_to_grid(Vec3::new(hi_x + pad, 0.0, hi_z + pad));
        min_cell.x -= 1;
        min_cell.y -= 1;
        max_cell.x += 1;
        max_cell.y += 1;
        min_cell.x = min_cell.x.max(0);
        min_cell.y = min_cell.y.max(0);
        max_cell.x = max_cell.x.min(self.width.saturating_sub(1));
        max_cell.y = max_cell.y.min(self.height.saturating_sub(1));
        if max_cell.x < min_cell.x || max_cell.y < min_cell.y {
            self.terrain_gen = self.terrain_gen.wrapping_add(1);
            return;
        }
        let pieces = self.wall_pieces.clone();
        let mut raw: HashMap<(i32, i32), PathfindCellType> = HashMap::new();
        for y in min_cell.y..=max_cell.y {
            for x in min_cell.x..=max_cell.x {
                let count = self.wall_corner_count(GridPos::new(x, y), &pieces);
                let ty = if count == 4 {
                    PathfindCellType::Clear
                } else if count != 0 {
                    PathfindCellType::BridgeImpassable
                } else {
                    PathfindCellType::Impassable
                };
                if ty != PathfindCellType::Impassable {
                    raw.insert((x, y), ty);
                }
            }
        }
        // C++ pinch: any 3x3 neighbor not CLEAR → pinched; pinched CLEAR → CLIFF.
        let mut pinched = HashSet::new();
        for y in (min_cell.y + 1)..max_cell.y {
            for x in (min_cell.x + 1)..max_cell.x {
                let mut pinch = false;
                'adj: for dy in -1..=1 {
                    for dx in -1..=1 {
                        let ty = raw
                            .get(&(x + dx, y + dy))
                            .copied()
                            .unwrap_or(PathfindCellType::Impassable);
                        if ty != PathfindCellType::Clear {
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
        for (k, ty) in raw.iter_mut() {
            if pinched.contains(k) && *ty == PathfindCellType::Clear {
                *ty = PathfindCellType::Cliff;
            }
        }
        self.wall_cells = raw;
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `Pathfinder::addWallPiece` (AIPathfind.cpp:3885-3890).
    pub fn add_wall_piece(&mut self, id: u32, pos: Vec3, orientation: f32, major: f32, minor: f32) {
        if self.wall_pieces.len() >= MAX_WALL_PIECES.saturating_sub(1) {
            return;
        }
        if self.wall_pieces.iter().any(|p| p.id == id) {
            return;
        }
        self.wall_pieces.push(HostWallPiece {
            id,
            pos_x: pos.x,
            pos_z: pos.z,
            orientation,
            major: major.max(0.1),
            minor: minor.max(0.1),
        });
        self.allocate_and_classify_wall_layer();
    }

    /// C++ `Pathfinder::removeWallPiece` (AIPathfind.cpp:3896-3923).
    pub fn remove_wall_piece(&mut self, id: u32) {
        if let Some(i) = self.wall_pieces.iter().position(|p| p.id == id) {
            let last = self.wall_pieces.len() - 1;
            self.wall_pieces.swap(i, last);
            self.wall_pieces.pop();
            self.allocate_and_classify_wall_layer();
        }
    }

    /// C++ classifyObjectFootprint remove: `isPointOnWall(&curID, 1, pos)`.
    pub fn is_point_on_wall_piece(&self, piece_id: u32, pos: Vec3) -> bool {
        self.wall_pieces
            .iter()
            .find(|p| p.id == piece_id)
            .is_some_and(|p| Self::wall_piece_contains(p, pos.x, pos.z))
    }

    /// C++ `m_map[i][j].getConnectLayer()` (0 = LAYER_INVALID).
    pub fn ground_connect_layer(&self, pos: GridPos) -> u8 {
        self.bit_index(pos)
            .and_then(|idx| self.ground_connect.get(idx).copied())
            .unwrap_or(0)
    }

    pub fn first_bridge_layer_id(&self) -> Option<u8> {
        self.bridge_layers.first().map(|l| l.id)
    }

    /// Bind leftover/host bridge object id onto the matching span.
    pub fn bind_bridge_layer_object_id(
        &mut self,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
        object_id: u32,
    ) {
        if object_id == 0 {
            return;
        }
        if let Some(layer) = self.bridge_layers.iter_mut().find(|layer| {
            span_xz_eq(layer.from_left, from_left)
                && span_xz_eq(layer.from_right, from_right)
                && span_xz_eq(layer.to_left, to_left)
                && span_xz_eq(layer.to_right, to_right)
        }) {
            layer.object_id = object_id;
        }
    }

    /// C++ `Pathfinder::findBrokenBridge` layer scan (destroyed + connectsZones).
    pub fn find_broken_bridge(&self, from: Vec3, to: Vec3) -> Option<ObjectId> {
        let from_c = self.world_to_grid(from);
        let to_c = self.world_to_grid(to);
        let zone1 = self.path_zone(from_c);
        let zone2 = self.path_zone(to_c);
        if zone1 == zone2 {
            return None;
        }
        for layer in &self.bridge_layers {
            if !layer.destroyed || layer.object_id == 0 {
                continue;
            }
            let mut found1 = false;
            let mut found2 = false;
            for &(x, y) in &layer.ground_connect_cells {
                let z = self.path_zone(GridPos::new(x, y));
                if z == 0 {
                    continue;
                }
                if z == zone1 {
                    found1 = true;
                }
                if z == zone2 {
                    found2 = true;
                }
                if found1 && found2 {
                    return Some(ObjectId(layer.object_id));
                }
            }
        }
        None
    }

    /// C++ `TerrainLogic::getLayerForDestination` (host Y-up).
    /// Nearest deck/ground height among bridges whose quad covers XZ.
    pub fn layer_for_destination(&self, pos: Vec3) -> PathfindLayerEnum {
        let ground_y = sample_host_ground_height(pos.x, pos.z);
        let mut best_layer = PathfindLayerEnum::Ground;
        let mut best_distance = (pos.y - ground_y).abs();
        // C++ TerrainLogic::getLayerForDestination checks the wall first
        // when |z-ground| > wallHeight/2 (TerrainLogic.cpp:1674-1682).
        if best_distance > self.wall_height * 0.5 && self.is_point_on_wall(pos) {
            let delta = (pos.y - self.wall_height).abs();
            if delta < best_distance {
                best_layer = PathfindLayerEnum::Wall;
                best_distance = delta;
            }
        }
        let cell = self.world_to_grid(pos);
        for layer in &self.bridge_layers {
            let corners = [
                layer.from_left,
                layer.from_right,
                layer.to_right,
                layer.to_left,
            ];
            if point_in_bridge_quad(pos.x, pos.z, &corners) {
                let deck_y = bridge_deck_height(&corners, pos.x, pos.z);
                let delta = (pos.y - deck_y).abs();
                if delta < best_distance {
                    best_layer = PathfindLayerEnum::from_u32(layer.id as u32);
                    best_distance = delta;
                }
            }
        }
        if best_layer != PathfindLayerEnum::Ground {
            return best_layer;
        }
        if let Ok(tl) = gamelogic::terrain::get_terrain_logic().read() {
            let dest = gamelogic::common::Coord3D::new(pos.x, pos.z, pos.y);
            let leftover = tl.get_layer_for_destination(&dest);
            if leftover != gamelogic::path::PathfindLayerEnum::Ground {
                let id = leftover as u8;
                if self.layer_cell_type(id, cell).is_some() {
                    return PathfindLayerEnum::from_u32(id as u32);
                }
                if let Some(host) = self.host_deck_layer_at(cell) {
                    return PathfindLayerEnum::from_u32(host as u32);
                }
            }
        }
        // Click/unit on a river cell whose deck is Clear: prefer the deck
        // when ground is not a valid ground locomotor cell (Y may be 0).
        if !self.cell_passable_for(cell, SURFACE_GROUND, false) {
            if let Some(host) = self.host_deck_layer_at(cell) {
                return PathfindLayerEnum::from_u32(host as u32);
            }
        }
        PathfindLayerEnum::Ground
    }

    pub(super) fn host_deck_layer_at(&self, pos: GridPos) -> Option<u8> {
        self.bridge_layers.iter().find_map(|layer| {
            layer.cells.get(&(pos.x, pos.y)).and_then(|(ty, _)| {
                if matches!(
                    *ty,
                    PathfindCellType::Impassable | PathfindCellType::BridgeImpassable
                ) {
                    None
                } else {
                    Some(layer.id)
                }
            })
        })
    }

    /// C++ `Pathfinder::getCell(layer, x, y)` type (Impassable/missing → ground).
    pub fn resolved_cell_type(&self, layer: PathfindLayerEnum, pos: GridPos) -> PathfindCellType {
        if (layer as u8) > PathfindLayerEnum::Ground as u8 {
            if let Some(ty) = self.layer_cell_type(layer as u8, pos) {
                if ty != PathfindCellType::Impassable {
                    return ty;
                }
            }
        }
        self.cell_type(pos)
    }

    pub(super) fn type_passable_for(ty: PathfindCellType, surfaces: u32, is_crusher: bool) -> bool {
        let cell_surfaces = match ty {
            PathfindCellType::Obstacle
            | PathfindCellType::Impassable
            | PathfindCellType::BridgeImpassable => SURFACE_AIR,
            PathfindCellType::Clear => SURFACE_GROUND | SURFACE_AIR,
            PathfindCellType::Water => SURFACE_WATER | SURFACE_AIR,
            PathfindCellType::Rubble => SURFACE_RUBBLE | SURFACE_AIR,
            PathfindCellType::Cliff => SURFACE_CLIFF | SURFACE_AIR,
        };
        if (cell_surfaces & surfaces) != 0 {
            return true;
        }
        ty == PathfindCellType::Rubble && is_crusher
    }

    /// `cell_passable_for` on `layer` (C++ `validMovementPosition(..., layer, ...)`).
    pub fn cell_passable_for_layer(
        &self,
        pos: GridPos,
        layer: PathfindLayerEnum,
        surfaces: u32,
        is_crusher: bool,
    ) -> bool {
        if !self.is_valid_pos(pos) {
            return false;
        }
        if self.is_obstacle_fence(pos) && is_crusher {
            return true;
        }
        Self::type_passable_for(self.resolved_cell_type(layer, pos), surfaces, is_crusher)
    }

    pub(super) fn cell_center_xz(&self, pos: GridPos) -> (f32, f32) {
        let tl = self.grid_to_world(pos);
        (tl.x + self.grid_size * 0.5, tl.z + self.grid_size * 0.5)
    }

    pub(super) fn alloc_or_find_bridge_layer(
        &mut self,
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
    ) -> u8 {
        if let Some(existing) = self.bridge_layers.iter().find(|layer| {
            span_xz_eq(layer.from_left, from_left)
                && span_xz_eq(layer.from_right, from_right)
                && span_xz_eq(layer.to_left, to_left)
                && span_xz_eq(layer.to_right, to_right)
        }) {
            return existing.id;
        }
        let used: Vec<u8> = self.bridge_layers.iter().map(|l| l.id).collect();
        let id = (2u8..=14).find(|id| !used.contains(id)).unwrap_or(2);
        if used.contains(&id) {
            self.disconnect_ground_from_layer(id);
            if let Some(slot) = self.bridge_layers.iter_mut().find(|l| l.id == id) {
                slot.from_left = from_left;
                slot.from_right = from_right;
                slot.to_left = to_left;
                slot.to_right = to_right;
                slot.cells.clear();
                slot.destroyed = false;
                slot.ground_connect_cells.clear();
                return id;
            }
        }
        self.bridge_layers.push(HostBridgeLayer {
            id,
            from_left,
            from_right,
            to_left,
            to_right,
            cells: HashMap::new(),
            destroyed: false,
            object_id: 0,
            ground_connect_cells: Vec::new(),
        });
        id
    }

    pub(super) fn disconnect_ground_from_layer(&mut self, layer_id: u8) {
        for slot in &mut self.ground_connect {
            if *slot == layer_id {
                *slot = 0;
            }
        }
    }

    /// C++ `PathfindLayer::classifyLayerMapCell` (AIPathfind.cpp:3647-3724).
    pub(super) fn classify_layer_map_cell(
        &self,
        pos: GridPos,
        corners: &[Vec3; 4],
        from_left: Vec3,
        from_right: Vec3,
        to_left: Vec3,
        to_right: Vec3,
    ) -> Option<(PathfindCellType, u8)> {
        let tl = self.grid_to_world(pos);
        let s = self.grid_size;
        let pts = [
            (tl.x, tl.z),
            (tl.x, tl.z + s),
            (tl.x + s, tl.z + s),
            (tl.x + s, tl.z),
        ];
        let mut count = 0u32;
        for (px, pz) in pts {
            if point_in_bridge_quad(px, pz, corners) {
                count += 1;
            }
        }
        let bounds = CellXz {
            lo_x: tl.x,
            lo_z: tl.z,
            hi_x: tl.x + s,
            hi_z: tl.z + s,
        };
        let mut ty = PathfindCellType::Impassable;
        let mut connect = 0u8;
        if count == 4 {
            ty = PathfindCellType::Clear;
        } else {
            if count != 0 {
                ty = PathfindCellType::BridgeImpassable;
            }
            if cell_on_bridge_side(&bounds, from_left, from_right, to_left, to_right, s) {
                ty = PathfindCellType::BridgeImpassable;
            } else {
                if cell_on_bridge_end(&bounds, from_left, from_right, to_left, to_right, s) {
                    ty = PathfindCellType::Clear;
                }
                if cell_is_bridge_entry(&bounds, from_left, from_right, to_left, to_right, s) {
                    ty = PathfindCellType::Clear;
                    connect = PathfindLayerEnum::Ground as u8;
                }
            }
        }
        if ty == PathfindCellType::Impassable {
            None
        } else {
            Some((ty, connect))
        }
    }

    pub(super) fn query_layer_enum(&self) -> PathfindLayerEnum {
        PathfindLayerEnum::from_u32(self.query_layer as u32)
    }

    /// C++ `getCell(layer, x, y)` occupancy. Missing layer cells fall back to ground.
    pub(super) fn occupancy_bits(&self, pos: GridPos, layer: PathfindLayerEnum) -> OccBits {
        if (layer as u8) > PathfindLayerEnum::Ground as u8
            && self.layer_cell_type(layer as u8, pos).is_some()
        {
            if let Some(occ) = self.layer_occ.get(&(layer as u8)) {
                let key = (pos.x, pos.y);
                return OccBits {
                    fixed: occ.occ_fixed_mask.get(&key).copied().unwrap_or(0),
                    moving: occ.occ_moving_mask.get(&key).copied().unwrap_or(0),
                    goal: occ.occ_goal_mask.get(&key).copied().unwrap_or(0),
                    infantry: occ.occ_infantry_mask.get(&key).copied().unwrap_or(0),
                    crushable: occ.occ_fixed_max_crushable.get(&key).copied().unwrap_or(0),
                    goal_unit: occ.occ_goal_unit.get(&key).copied().unwrap_or(0),
                    pos_unit: occ.occ_pos_unit.get(&key).copied().unwrap_or(0),
                    pos_player: occ.occ_pos_player.get(&key).copied().unwrap_or(0),
                    pos_moving: occ.occ_pos_flags.get(&key).copied().unwrap_or(0) & 1 != 0,
                    pos_infantry: occ.occ_pos_flags.get(&key).copied().unwrap_or(0) & 2 != 0,
                    pos_crushable: occ.occ_pos_crushable.get(&key).copied().unwrap_or(0),
                };
            }
            return OccBits::default();
        }
        let Some(idx) = self.bit_index(pos) else {
            return OccBits::default();
        };
        OccBits {
            fixed: self.occ_fixed_mask.get(idx).copied().unwrap_or(0),
            moving: self.occ_moving_mask.get(idx).copied().unwrap_or(0),
            goal: self.occ_goal_mask.get(idx).copied().unwrap_or(0),
            infantry: self.occ_infantry_mask.get(idx).copied().unwrap_or(0),
            crushable: self.occ_fixed_max_crushable.get(idx).copied().unwrap_or(0),
            goal_unit: self.occ_goal_unit.get(idx).copied().unwrap_or(0),
            pos_unit: self.occ_pos_unit.get(idx).copied().unwrap_or(0),
            pos_player: self.occ_pos_player.get(idx).copied().unwrap_or(0),
            pos_moving: self.occ_pos_flags.get(idx).copied().unwrap_or(0) & 1 != 0,
            pos_infantry: self.occ_pos_flags.get(idx).copied().unwrap_or(0) & 2 != 0,
            pos_crushable: self.occ_pos_crushable.get(idx).copied().unwrap_or(0),
        }
    }
    pub(super) fn occupancy_cost(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
        ally_mask: u16,
        start: Option<GridPos>,
    ) -> Option<f32> {
        let bits = self.occupancy_bits(pos, self.query_layer_enum());
        if bits.fixed == 0 && bits.moving == 0 && bits.goal == 0 {
            return Some(0.0);
        }
        // C++ INFANTRY_MOVES_THROUGH_INFANTRY continue is unconditional
        // (AIPathfind.cpp:5031-5035) — goals do not block infantry stream-through.
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return Some(0.0);
        }
        // C++ checkForMovement keys on the cell's single m_posUnitID, not a
        // per-player bitset: posUnit==seeker / ignoreId skip, then one
        // ALLIES→allyFixedCount (or canCrushOrSquish→enemyFixed) verdict.
        let Some(player) = seeker_player else {
            return Some(3.0 * 1.414_213_5);
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | ally_mask;
        if seeker_is_infantry
            && (bits.infantry & !bit) != 0
            && (bits.fixed & !bit) == (bits.infantry & !bit)
        {
            let leftover_fixed = bits.fixed & !bits.infantry;
            let leftover_moving = bits.moving & !bits.infantry;
            if leftover_fixed == 0 && leftover_moving == 0 {
                return Some(0.0);
            }
        }
        // Single-occupant identity check (C++ PathfindCell::getPosUnit).
        if bits.pos_unit != 0 {
            let owner_friend = (bits.pos_player & friend) != 0;
            if !owner_friend {
                let enemy_crushable = crusher_level > 0 && crusher_level > bits.pos_crushable;
                if !enemy_crushable {
                    return None;
                }
            } else if !bits.pos_moving {
                // C++ ALLIES + UNIT_PRESENT_FIXED → allyFixedCount cost.
                return Some(3.0 * 1.414_213_5);
            }
        } else {
            // No single identity recorded — fall back to the bitset view so
            // legacy stamps still block.
            let enemy_fixed = (bits.fixed & !friend) != 0;
            if enemy_fixed && (crusher_level == 0 || crusher_level <= bits.crushable) {
                return None;
            }
        }
        let mut extra = 0.0;
        // C++ allyMoving +3*COST_DIAGONAL only within dx<10 && dy<10 of start
        // (AIPathfind.cpp:6260-6262). Moving enemies add no cost.
        if (bits.moving & friend) != 0 {
            if let Some(s) = start {
                if (pos.x - s.x).abs() < 10 && (pos.y - s.y).abs() < 10 {
                    extra += 3.0 * 1.414_213_5;
                }
            }
        }
        if (bits.fixed & friend) != 0 {
            extra += 3.0 * 1.414_213_5;
        }
        Some(extra)
    }

    /// C++ `examineCellsCallback` seed abort (AIPathfind.cpp:6034-6052).
    /// Allied-fixed or any enemy-fixed (crushable included) refuses the seed line.
    pub(super) fn seed_line_occupancy_ok(
        &self,
        pos: GridPos,
        seeker_player: Option<u32>,
        ally_mask: u16,
        seeker_is_infantry: bool,
        layer: PathfindLayerEnum,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return true;
        }
        let Some(player) = seeker_player else {
            return bits.fixed == 0 && bits.pos_unit == 0;
        };
        let bit = 1u16 << player.min(15);
        let friend = bit | ally_mask;
        // Single-occupant verdict first (C++ m_posUnitID), then the residual
        // bitset view for cells whose identity was never stamped.
        if bits.pos_unit != 0 {
            if (bits.pos_player & friend) == 0 {
                return false;
            }
            if !bits.pos_moving {
                return false;
            }
            return true;
        }
        if (bits.fixed & friend) != 0 {
            return false;
        }
        if (bits.fixed & !friend) != 0 {
            return false;
        }
        true
    }

    /// C++ `checkForMovement` occupancy used by `patchPath` (AIPathfind.cpp:10419-10442).
    pub(super) fn patch_cell_occupied(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        consider_transient: bool,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
        radius: i32,
        center_in_cell: bool,
    ) -> bool {
        let above = if center_in_cell { radius + 1 } else { radius };
        for y in (cell.y - radius)..(cell.y + above) {
            for x in (cell.x - radius)..(cell.x + above) {
                let p = GridPos::new(x, y);
                if !self.is_valid_pos(p) {
                    return true;
                }
                if self.patch_occupant_blocks(
                    p,
                    layer,
                    consider_transient,
                    seeker_player,
                    seeker_is_infantry,
                    crusher_level,
                ) {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn patch_occupant_blocks(
        &self,
        pos: GridPos,
        layer: PathfindLayerEnum,
        consider_transient: bool,
        seeker_player: Option<u32>,
        seeker_is_infantry: bool,
        crusher_level: u8,
    ) -> bool {
        let bits = self.occupancy_bits(pos, layer);
        if bits.fixed == 0 && (!consider_transient || bits.moving == 0) {
            return false;
        }
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return false;
        }
        // C++ checkForMovement (AIPathfind.cpp:10419-10442): the cell's single
        // m_posUnitID decides — ally fixed blocks, enemy blocks unless crushable.
        if let Some(player) = seeker_player {
            if bits.pos_unit != 0 {
                let friend = (1u16 << player.min(15)) | self.ally_mask_for(player);
                if (bits.pos_player & friend) == 0 {
                    return crusher_level == 0 || crusher_level <= bits.pos_crushable;
                }
                // Allied fixed occupant blocks the patch footprint.
                return true;
            }
            let present = bits.fixed | if consider_transient { bits.moving } else { 0 };
            let friend = (1u16 << player.min(15)) | self.ally_mask_for(player);
            if (present & friend) != 0 {
                return true;
            }
            let enemy = (present & !friend) != 0;
            return enemy && (crusher_level == 0 || crusher_level <= bits.crushable);
        }
        true
    }

    /// C++ `PathfindCell::getConnectLayer` hop target (same XY, other layer).
    pub(super) fn connect_layer_of(
        &self,
        pos: GridPos,
        current: PathfindLayerEnum,
    ) -> Option<PathfindLayerEnum> {
        if matches!(
            current,
            PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid
        ) {
            let cl = self.ground_connect_layer(pos);
            if cl != 0 {
                return Some(PathfindLayerEnum::from_u32(cl as u32));
            }
            return None;
        }
        if current as u8 == LAYER_WALL_ID {
            return Some(PathfindLayerEnum::Ground);
        }
        self.bridge_layers
            .iter()
            .find(|layer| layer.id == current as u8)
            .and_then(|layer| layer.cells.get(&(pos.x, pos.y)))
            .and_then(|(_, connect)| {
                if *connect != 0 && *connect != current as u8 {
                    Some(PathfindLayerEnum::from_u32(*connect as u32))
                } else {
                    None
                }
            })
    }

    /// C++ `Pathfinder::checkChangeLayers` (AIPathfind.cpp:5942-5981).
    /// Enqueue the same-XY connect-layer cell at parent cost if not closed.
    pub(super) fn enqueue_connect_layer(
        &self,
        cell: GridPos,
        layer: PathfindLayerEnum,
        g: i32,
        f: i32,
        closed: &HashSet<(i32, i32, u8)>,
        g_score: &mut HashMap<(i32, i32, u8), i32>,
        open: &mut BinaryHeap<std::cmp::Reverse<(i32, i32, i32, i32, u8)>>,
    ) -> bool {
        let Some(cl) = self.connect_layer_of(cell, layer) else {
            return false;
        };
        let lid = cl as u8;
        let key = (cell.x, cell.y, lid);
        if closed.contains(&key) {
            return false;
        }
        if g_score.get(&key).is_some_and(|&og| g >= og) {
            return false;
        }
        g_score.insert(key, g);
        open.push(std::cmp::Reverse((f, g, cell.x, cell.y, lid)));
        true
    }

    /// C++ `examineNeighboringCells` attackDistance occupancy (AIPathfind.cpp:6228-6300).
    /// `None` = enemyFixed skip. Extra is added to the ortho/diag step.
    pub(super) fn attack_step_occupancy(
        &self,
        pos: GridPos,
        start: GridPos,
        seeker_player: Option<u32>,
        ally_mask: u16,
        seeker_is_infantry: bool,
        is_vehicle: bool,
        crusher_level: u8,
        layer: PathfindLayerEnum,
    ) -> Option<i32> {
        const COST_ORTHO: i32 = 10;
        const COST_DIAG: i32 = 14;
        let bits = self.occupancy_bits(pos, layer);
        if seeker_is_infantry && bits.infantry != 0 && (bits.fixed | bits.moving) == bits.infantry {
            return Some(0);
        }
        let friend = match seeker_player {
            Some(player) => (1u16 << player.min(15)) | ally_mask,
            None => 0,
        };
        // C++ checkForMovement verdict on the cell's single m_posUnitID
        // (AIPathfind.cpp:5020-5066): ally fixed → allyFixedCount cost, enemy
        // uncrushable → skip. The bitset fallback keeps legacy stamps working.
        if seeker_player.is_some() {
            if bits.pos_unit != 0 {
                if (bits.pos_player & friend) == 0 {
                    let crushable = crusher_level > 0 && crusher_level > bits.pos_crushable;
                    if !crushable {
                        return None;
                    }
                } else if !bits.pos_moving {
                    // allyFixedCount>0 → +3*COST_DIAGONAL (AIPathfind.cpp:6278-6290).
                    return Some(3 * COST_DIAG);
                }
            } else {
                if (bits.fixed & !friend) != 0
                    && (crusher_level == 0 || crusher_level <= bits.crushable)
                {
                    return None;
                }
                if (bits.fixed & friend) != 0 {
                    return Some(3 * COST_DIAG);
                }
            }
        } else if bits.pos_unit != 0 || (bits.fixed & !friend) != 0 {
            if crusher_level == 0 || crusher_level <= {
                if bits.pos_unit != 0 {
                    bits.pos_crushable
                } else {
                    bits.crushable
                }
            } {
                return None;
            }
        }
        let mut extra = 0i32;
        if (bits.moving & friend) != 0
            && (pos.x - start.x).abs() < 10
            && (pos.y - start.y).abs() < 10
        {
            extra += 3 * COST_DIAG;
        }
        if (bits.goal & friend) != 0 {
            extra += if is_vehicle {
                3 * COST_ORTHO
            } else {
                COST_ORTHO
            };
        }
        Some(extra)
    }

    /// C++ `Pathfinder::updatePos` / `updateGoal` cell stamp.
    /// LAYER cells go on the layer map; `dynamic_bits` is ground-only.
    /// Missing layer cells fall back to ground (`getCell` residual).
    pub(super) fn mark_occupancy(
        &mut self,
        pos: GridPos,
        player: u32,
        moving: bool,
        infantry: bool,
        goal: bool,
        crushable_level: u8,
        unit_id: u32,
        layer: PathfindLayerEnum,
    ) {
        let bit = 1u16 << player.min(15);
        if (layer as u8) > PathfindLayerEnum::Ground as u8
            && self.layer_cell_type(layer as u8, pos).is_some()
        {
            let occ = self.layer_occ.entry(layer as u8).or_default();
            let key = (pos.x, pos.y);
            if goal {
                *occ.occ_goal_mask.entry(key).or_insert(0) |= bit;
                occ.occ_goal_unit.insert(key, unit_id);
                return;
            }
            // C++ setPosUnit: a single m_posUnitID per cell; the newest
            // stamp wins (updatePos iterates units in frame order).
            occ.occ_pos_unit.insert(key, unit_id);
            occ.occ_pos_player.insert(key, bit);
            occ.occ_pos_flags.insert(
                key,
                (moving as u8) | ((infantry as u8) << 1),
            );
            occ.occ_pos_crushable.insert(key, crushable_level);
            if infantry {
                *occ.occ_infantry_mask.entry(key).or_insert(0) |= bit;
            }
            if moving {
                *occ.occ_moving_mask.entry(key).or_insert(0) |= bit;
            } else {
                *occ.occ_fixed_mask.entry(key).or_insert(0) |= bit;
                let crush = occ.occ_fixed_max_crushable.entry(key).or_insert(0);
                *crush = (*crush).max(crushable_level);
            }
            return;
        }
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.dynamic_bits, idx, true);
        if goal {
            if let Some(slot) = self.occ_goal_mask.get_mut(idx) {
                *slot |= bit;
            }
            if let Some(slot) = self.occ_goal_unit.get_mut(idx) {
                *slot = unit_id;
            }
            return;
        }
        if let Some(slot) = self.occ_pos_unit.get_mut(idx) {
            *slot = unit_id;
        }
        if let Some(slot) = self.occ_pos_player.get_mut(idx) {
            *slot = bit;
        }
        if let Some(slot) = self.occ_pos_flags.get_mut(idx) {
            *slot = (moving as u8) | ((infantry as u8) << 1);
        }
        if let Some(slot) = self.occ_pos_crushable.get_mut(idx) {
            *slot = crushable_level;
        }
        // The pos stamp IS the fixed/moving record (C++ updatePos): keep the
        // bitset view in lockstep so identity-less readers still see it.
        if moving {
            if let Some(slot) = self.occ_moving_mask.get_mut(idx) {
                *slot |= bit;
            }
            if infantry {
                if let Some(slot) = self.occ_infantry_mask.get_mut(idx) {
                    *slot |= bit;
                }
            }
        } else {
            if let Some(slot) = self.occ_fixed_mask.get_mut(idx) {
                *slot |= bit;
            }
            if let Some(slot) = self.occ_fixed_max_crushable.get_mut(idx) {
                *slot = (*slot).max(crushable_level);
            }
            if infantry {
                if let Some(slot) = self.occ_infantry_mask.get_mut(idx) {
                    *slot |= bit;
                }
            }
        }
    }

    /// C++ `TerrainLogic::objectInteractsWithBridgeEnd` (TerrainLogic.cpp:1799).
    pub(super) fn object_interacts_with_bridge_end(
        &self,
        pos: Vec3,
        minor_radius: f32,
        layer: PathfindLayerEnum,
    ) -> bool {
        if layer == PathfindLayerEnum::Ground {
            return false;
        }
        let Some(bridge) = self
            .bridge_layers
            .iter()
            .find(|layer_rec| layer_rec.id == layer as u8)
        else {
            return false;
        };
        let r = minor_radius + self.grid_size * 0.5;
        let cell = CellXz {
            lo_x: pos.x - r,
            lo_z: pos.z - r,
            hi_x: pos.x + r,
            hi_z: pos.z + r,
        };
        if !cell_on_bridge_end(
            &cell,
            bridge.from_left,
            bridge.from_right,
            bridge.to_left,
            bridge.to_right,
            self.grid_size,
        ) {
            return false;
        }
        let corners = [
            bridge.from_left,
            bridge.from_right,
            bridge.to_right,
            bridge.to_left,
        ];
        let deck_h = bridge_deck_height(&corners, pos.x, pos.z);
        (pos.y - deck_h).abs() <= LAYER_Z_CLOSE_ENOUGH_F
    }
}
