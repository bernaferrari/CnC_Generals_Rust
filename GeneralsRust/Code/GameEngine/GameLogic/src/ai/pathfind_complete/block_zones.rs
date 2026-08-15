use super::*;

/// Per-block surface combiners — C++ ZoneBlock (AIPathfind.cpp ZoneBlock).
#[derive(Debug, Clone)]
pub(crate) struct BlockCombiner {
    pub(crate) first_zone: u16,
    pub(crate) num_zones: u16,
    pub(crate) ground_cliff: Vec<u16>,
    pub(crate) ground_water: Vec<u16>,
    pub(crate) ground_rubble: Vec<u16>,
    pub(crate) crusher: Vec<u16>,
    pub(crate) interacts_with_bridge: bool,
    pub(crate) marked_passable: bool,
}

impl BlockCombiner {
    pub(crate) fn identity(first: u16, num: u16) -> Self {
        let n = num.max(1) as usize;
        let table = || {
            (0..n)
                .map(|i| first.saturating_add(i as u16))
                .collect::<Vec<_>>()
        };
        Self {
            first_zone: first,
            num_zones: num.max(1),
            ground_cliff: table(),
            ground_water: table(),
            ground_rubble: table(),
            crusher: table(),
            interacts_with_bridge: false,
            marked_passable: true,
        }
    }

    /// C++ ZoneBlock::getEffectiveZone — local index into block tables.
    pub(crate) fn get_effective_zone(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        mut zone: u16,
    ) -> u16 {
        if zone == UNINITIALIZED_ZONE {
            return zone;
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
        if self.num_zones < 2 {
            return self.first_zone;
        }
        if zone < self.first_zone || zone >= self.first_zone.saturating_add(self.num_zones) {
            return self.first_zone;
        }
        let mut idx = (zone - self.first_zone) as usize;
        if crusher {
            if let Some(&z) = self.crusher.get(idx) {
                if z >= self.first_zone {
                    idx = (z - self.first_zone) as usize;
                    zone = z;
                }
            }
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_CLIFF) != 0 {
            return self.ground_cliff.get(idx).copied().unwrap_or(zone);
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_WATER) != 0 {
            return self.ground_water.get(idx).copied().unwrap_or(zone);
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_RUBBLE) != 0 {
            return self.ground_rubble.get(idx).copied().unwrap_or(zone);
        }
        self.first_zone.saturating_add(idx as u16)
    }
}

/// Zone manager for hierarchical pathfinding
/// Matches C++ PathfindZoneManager at AIPathfind.h:475-531
pub(crate) struct ZoneManager {
    pub(crate) zones: Vec<Vec<u16>>,
    pub(crate) width: usize,
    pub(crate) height: usize,
    pub(crate) next_zone: u16,
    /// C++ needToCalculateZones / markZonesDirty.
    pub(crate) zones_dirty: bool,
    /// C++ m_crusherZones — filled by build_surface_combiners / identity fallback.
    pub(crate) crusher_zones: Vec<u16>,
    /// C++ m_groundCliffZones — filled by build_surface_combiners / identity fallback.
    pub(crate) ground_cliff_zones: Vec<u16>,
    pub(crate) ground_water_zones: Vec<u16>,
    pub(crate) ground_rubble_zones: Vec<u16>,
    /// C++ m_hierarchicalZones — same-type connectivity across the map.
    pub(crate) hierarchical_zones: Vec<u16>,
    /// C++ m_terrainZones — obstacle treated as clear for terrain connectivity.
    pub(crate) terrain_zones: Vec<u16>,
    /// C++ m_zoneBlocks[x][y] — per ZONE_BLOCK_SIZE combiners.
    pub(crate) zone_blocks: Vec<Vec<BlockCombiner>>,
    pub(crate) blocks_x: usize,
    pub(crate) blocks_y: usize,
}

impl ZoneManager {
    pub(crate) fn new(width: usize, height: usize) -> Self {
        let blocks_x = (width + ZONE_BLOCK_SIZE as usize - 1) / ZONE_BLOCK_SIZE as usize;
        let blocks_y = (height + ZONE_BLOCK_SIZE as usize - 1) / ZONE_BLOCK_SIZE as usize;
        Self {
            zones: vec![vec![0; height]; width],
            width,
            height,
            next_zone: 1,
            zones_dirty: true,
            crusher_zones: Vec::new(),
            ground_cliff_zones: Vec::new(),
            ground_water_zones: Vec::new(),
            ground_rubble_zones: Vec::new(),
            hierarchical_zones: Vec::new(),
            terrain_zones: Vec::new(),
            zone_blocks: vec![
                vec![BlockCombiner::identity(1, 1); blocks_y.max(1)];
                blocks_x.max(1)
            ],
            blocks_x: blocks_x.max(1),
            blocks_y: blocks_y.max(1),
        }
    }

    pub(crate) fn reset(&mut self) {
        for column in self.zones.iter_mut() {
            for zone in column.iter_mut() {
                *zone = 0;
            }
        }
        self.next_zone = 1;
    }

    pub(crate) fn zone_at(&self, cell: GridCoord) -> u16 {
        if cell.x < 0
            || cell.y < 0
            || cell.x as usize >= self.width
            || cell.y as usize >= self.height
        {
            return 0;
        }
        self.zones[cell.x as usize][cell.y as usize]
    }

    /// C++ hierarchical connectivity via `getEffectiveZone` (not raw cell zones).
    ///
    /// Ground+cliff locomotors share ground_cliff combiners; crushers share
    /// crusher combiners, etc. Identity residual only when combiners unbuilt.
    pub(crate) fn are_connected(
        &self,
        start: GridCoord,
        goal: GridCoord,
        surfaces: LocomotorSurfaceTypeMask,
        is_crusher: bool,
    ) -> bool {
        if start.x < 0
            || start.x >= self.width as i32
            || start.y < 0
            || start.y >= self.height as i32
        {
            return false;
        }

        if goal.x < 0 || goal.x >= self.width as i32 || goal.y < 0 || goal.y >= self.height as i32 {
            return false;
        }

        let start_zone = self.zones[start.x as usize][start.y as usize];
        let goal_zone = self.zones[goal.x as usize][goal.y as usize];

        if start_zone == 0 || goal_zone == 0 {
            // Unzoned (dirty/partial) — don't hard-reject hierarchical precheck.
            return true;
        }

        let z1 = self.get_effective_zone(surfaces, is_crusher, start_zone);
        let z2 = self.get_effective_zone(surfaces, is_crusher, goal_zone);
        z1 == z2
    }

    /// C++ `PathfindZoneManager::getBlockZone`.
    pub(crate) fn get_block_zone(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        cell_x: i32,
        cell_y: i32,
    ) -> u16 {
        if cell_x < 0
            || cell_y < 0
            || cell_x as usize >= self.width
            || cell_y as usize >= self.height
        {
            return 0;
        }
        let bx = (cell_x / ZONE_BLOCK_SIZE) as usize;
        let by = (cell_y / ZONE_BLOCK_SIZE) as usize;
        let zone = self.zones[cell_x as usize][cell_y as usize];
        if let Some(block) = self.zone_blocks.get(bx).and_then(|col| col.get(by)) {
            let z = block.get_effective_zone(surfaces, crusher, zone);
            if z != 0 && z < self.next_zone.max(2) {
                return z;
            }
            if z >= self.next_zone && self.next_zone > 1 {
                return UNINITIALIZED_ZONE;
            }
            return z;
        }
        self.get_effective_zone(surfaces, crusher, zone)
    }

    /// C++ `PathfindZoneManager::getEffectiveZone` (AIPathfind.cpp:3118+).
    pub(crate) fn get_effective_zone(
        &self,
        surfaces: LocomotorSurfaceTypeMask,
        crusher: bool,
        mut zone: u16,
    ) -> u16 {
        if zone == 0 {
            return 0;
        }
        // AIR → single zone
        if (surfaces & SURFACE_AIR) != 0 {
            return 1;
        }
        // ground+water+cliff → 1
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
            if let Some(&z) = self.ground_cliff_zones.get(zone as usize) {
                return z;
            }
            return zone;
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_WATER) != 0 {
            if let Some(&z) = self.ground_water_zones.get(zone as usize) {
                return z;
            }
            return zone;
        }
        if (surfaces & SURFACE_GROUND) != 0 && (surfaces & SURFACE_RUBBLE) != 0 {
            if let Some(&z) = self.ground_rubble_zones.get(zone as usize) {
                return z;
            }
            return zone;
        }
        // C++ default: zone = m_hierarchicalZones[zone]
        if let Some(&z) = self.hierarchical_zones.get(zone as usize) {
            return z;
        }
        zone
    }

    pub(crate) fn block_index(cell_x: i32, cell_y: i32) -> (i32, i32) {
        (cell_x / ZONE_BLOCK_SIZE, cell_y / ZONE_BLOCK_SIZE)
    }

    /// Rebuild identity combiner tables when cell types are unavailable.
    pub(crate) fn rebuild_combiner_identity(&mut self) {
        let n = self.next_zone as usize + 1;
        self.crusher_zones = (0..n).map(|i| i as u16).collect();
        self.ground_cliff_zones = (0..n).map(|i| i as u16).collect();
        self.ground_water_zones = (0..n).map(|i| i as u16).collect();
        self.ground_rubble_zones = (0..n).map(|i| i as u16).collect();
        self.hierarchical_zones = (0..n).map(|i| i as u16).collect();
        self.terrain_zones = (0..n).map(|i| i as u16).collect();
    }

    /// Calculate zones using flood-fill on the pathfinder grid.
    /// Matches C++ PathfindZoneManager::calculateZones().
    pub(crate) fn mark_zones_dirty(&mut self, _insert: bool) {
        // C++ PathfindZoneManager::markZonesDirty — force recalculation next frame.
        self.zones_dirty = true;
    }

    /// C++ `PathfindZoneManager::setAllPassable` residual — clear dirty gate.
    pub(crate) fn set_all_passable(&mut self) {
        self.zones_dirty = false;
        for col in &mut self.zone_blocks {
            for b in col.iter_mut() {
                b.marked_passable = true;
            }
        }
    }

    /// C++ `PathfindZoneManager::clearPassableFlags` residual.
    pub(crate) fn clear_passable_flags(&mut self) {
        self.zones_dirty = true;
        for col in &mut self.zone_blocks {
            for b in col.iter_mut() {
                b.marked_passable = false;
            }
        }
    }

    /// C++ `PathfindZoneManager::getEffectiveTerrainZone`.
    pub(crate) fn get_effective_terrain_zone(&self, zone: u16) -> u16 {
        if zone == 0 {
            return 0;
        }
        let t = self
            .terrain_zones
            .get(zone as usize)
            .copied()
            .unwrap_or(zone);
        self.hierarchical_zones
            .get(t as usize)
            .copied()
            .unwrap_or(t)
    }

    /// C++ `PathfindZoneManager::setPassable` residual — mark cell zone usable.
    pub(crate) fn set_passable(&mut self, cell_x: i32, cell_y: i32, passable: bool) {
        if cell_x < 0
            || cell_y < 0
            || cell_x as usize >= self.width
            || cell_y as usize >= self.height
        {
            return;
        }
        if passable && self.zones[cell_x as usize][cell_y as usize] == 0 {
            self.zones[cell_x as usize][cell_y as usize] = 1;
        }
        let bx = (cell_x / ZONE_BLOCK_SIZE) as usize;
        let by = (cell_y / ZONE_BLOCK_SIZE) as usize;
        if let Some(b) = self.zone_blocks.get_mut(bx).and_then(|c| c.get_mut(by)) {
            b.marked_passable = passable;
        }
    }

    pub(crate) fn is_block_passable(&self, cell_x: i32, cell_y: i32) -> bool {
        if cell_x < 0 || cell_y < 0 {
            return false;
        }
        let bx = (cell_x / ZONE_BLOCK_SIZE) as usize;
        let by = (cell_y / ZONE_BLOCK_SIZE) as usize;
        self.zone_blocks
            .get(bx)
            .and_then(|c| c.get(by))
            .map(|b| b.marked_passable)
            .unwrap_or(true)
    }

    /// C++ `PathfindZoneManager::setBridge`.
    pub(crate) fn set_bridge(&mut self, cell_x: i32, cell_y: i32, bridge: bool) {
        if cell_x < 0 || cell_y < 0 {
            return;
        }
        let bx = (cell_x / ZONE_BLOCK_SIZE) as usize;
        let by = (cell_y / ZONE_BLOCK_SIZE) as usize;
        if let Some(b) = self.zone_blocks.get_mut(bx).and_then(|c| c.get_mut(by)) {
            b.interacts_with_bridge = bridge;
        }
    }

    /// C++ `PathfindZoneManager::interactsWithBridge`.
    pub(crate) fn interacts_with_bridge(&self, cell_x: i32, cell_y: i32) -> bool {
        if cell_x < 0 || cell_y < 0 {
            return false;
        }
        let bx = (cell_x / ZONE_BLOCK_SIZE) as usize;
        let by = (cell_y / ZONE_BLOCK_SIZE) as usize;
        self.zone_blocks
            .get(bx)
            .and_then(|c| c.get(by))
            .map(|b| b.interacts_with_bridge)
            .unwrap_or(false)
    }

    /// Clear all bridge interaction flags (before re-stamp from layers).
    pub(crate) fn clear_bridge_flags(&mut self) {
        for col in &mut self.zone_blocks {
            for b in col.iter_mut() {
                b.interacts_with_bridge = false;
            }
        }
    }

    /// Calculate zones using flood-fill by cell type, then build surface combiners.
    /// Matches C++ PathfindZoneManager::calculateZones + ZoneBlock combiners.
    /// Flood-fill zones from a cell-type grid (no combiners).
    pub(crate) fn flood_fill_from_types(&mut self, types: &[Vec<PathfindCellType>]) {
        for col in self.zones.iter_mut() {
            for zone in col.iter_mut() {
                *zone = 0;
            }
        }
        self.next_zone = 1;
        for x in 0..self.width {
            for y in 0..self.height {
                if self.zones[x][y] == 0 {
                    let ct = types
                        .get(x)
                        .and_then(|col| col.get(y))
                        .copied()
                        .unwrap_or(PathfindCellType::Clear);
                    self.flood_fill_type(x, y, ct, types);
                }
            }
        }
        if self.next_zone == 0 {
            self.next_zone = 1;
        }
    }

    /// Allocate a fresh zone id (C++ PathfindLayer zone assignment).
    pub(crate) fn allocate_zone_id(&mut self) -> u16 {
        let z = self.next_zone;
        self.next_zone = self.next_zone.saturating_add(1).max(1);
        if self.next_zone == 0 {
            self.next_zone = 1;
        }
        z
    }

    pub(crate) fn calculate_zones(&mut self) {
        // Without cell types, identity flood-fill.
        self.calculate_zones_with_types(None);
    }

    pub(crate) fn calculate_zones_with_types(
        &mut self,
        cell_types: Option<&[Vec<PathfindCellType>]>,
    ) {
        self.calculate_zones_with_types_and_fences(cell_types, None, None, None);
    }

    pub(crate) fn calculate_zones_with_types_and_fences(
        &mut self,
        cell_types: Option<&[Vec<PathfindCellType>]>,
        fence_flags: Option<&[Vec<bool>]>,
        connect_layers: Option<&[Vec<u8>]>,
        layer_zones: Option<&[u16]>,
    ) {
        for col in self.zones.iter_mut() {
            for zone in col.iter_mut() {
                *zone = 0;
            }
        }
        self.next_zone = 1;

        if let Some(types) = cell_types {
            for x in 0..self.width {
                for y in 0..self.height {
                    if self.zones[x][y] == 0 {
                        let ct = types
                            .get(x)
                            .and_then(|col| col.get(y))
                            .copied()
                            .unwrap_or(PathfindCellType::Clear);
                        self.flood_fill_type(x, y, ct, types);
                    }
                }
            }
            self.build_surface_combiners(types, fence_flags, connect_layers, layer_zones);
            self.rebuild_zone_blocks(Some(types), fence_flags);
        } else {
            for x in 0..self.width {
                for y in 0..self.height {
                    if self.zones[x][y] == 0 {
                        self.flood_fill(x, y);
                    }
                }
            }
            self.rebuild_combiner_identity();
            self.rebuild_zone_blocks(None, None);
        }
        self.zones_dirty = false;
    }

    pub(crate) fn flood_fill_type(
        &mut self,
        start_x: usize,
        start_y: usize,
        cell_type: PathfindCellType,
        types: &[Vec<PathfindCellType>],
    ) {
        let zone_id = self.next_zone;
        self.next_zone = self.next_zone.saturating_add(1).max(1);
        if self.next_zone == 0 {
            self.next_zone = 1;
        }
        let mut stack = vec![(start_x, start_y)];
        while let Some((x, y)) = stack.pop() {
            if x >= self.width || y >= self.height {
                continue;
            }
            if self.zones[x][y] != 0 {
                continue;
            }
            let ct = types
                .get(x)
                .and_then(|col| col.get(y))
                .copied()
                .unwrap_or(PathfindCellType::Clear);
            if ct != cell_type {
                continue;
            }
            self.zones[x][y] = zone_id;
            if x > 0 {
                stack.push((x - 1, y));
            }
            if x + 1 < self.width {
                stack.push((x + 1, y));
            }
            if y > 0 {
                stack.push((x, y - 1));
            }
            if y + 1 < self.height {
                stack.push((x, y + 1));
            }
        }
    }

    pub(crate) fn flood_fill(&mut self, start_x: usize, start_y: usize) {
        let zone_id = self.next_zone;
        self.next_zone = self.next_zone.saturating_add(1).max(1);
        if self.next_zone == 0 {
            self.next_zone = 1;
        }
        let mut stack = vec![(start_x, start_y)];
        while let Some((x, y)) = stack.pop() {
            if x >= self.width || y >= self.height {
                continue;
            }
            if self.zones[x][y] != 0 {
                continue;
            }
            self.zones[x][y] = zone_id;
            if x > 0 {
                stack.push((x - 1, y));
            }
            if x + 1 < self.width {
                stack.push((x + 1, y));
            }
            if y > 0 {
                stack.push((x, y - 1));
            }
            if y + 1 < self.height {
                stack.push((x, y + 1));
            }
        }
    }

    /// Build ground/cliff, ground/water, ground/rubble, crusher combiner tables.
    /// C++ ZoneBlock::blockCalculateZones / PathfindZoneManager global tables.
    pub(crate) fn build_surface_combiners(
        &mut self,
        types: &[Vec<PathfindCellType>],
        fence_flags: Option<&[Vec<bool>]>,
        connect_layers: Option<&[Vec<u8>]>,
        layer_zones: Option<&[u16]>,
    ) {
        let n = (self.next_zone as usize).max(2);
        // Index by zone id; unused 0 slot identity.
        let mut cliff = (0..n).map(|i| i as u16).collect::<Vec<_>>();
        let mut water = (0..n).map(|i| i as u16).collect::<Vec<_>>();
        let mut rubble = (0..n).map(|i| i as u16).collect::<Vec<_>>();
        let mut crusher = (0..n).map(|i| i as u16).collect::<Vec<_>>();
        let mut hierarchical = (0..n).map(|i| i as u16).collect::<Vec<_>>();
        let mut terrain = (0..n).map(|i| i as u16).collect::<Vec<_>>();

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

        let ct = |x: usize, y: usize| -> PathfindCellType {
            types
                .get(x)
                .and_then(|c| c.get(y))
                .copied()
                .unwrap_or(PathfindCellType::Clear)
        };
        let is_fence_obs = |x: usize, y: usize| -> bool {
            fence_flags
                .and_then(|f| f.get(x))
                .and_then(|col| col.get(y))
                .copied()
                .unwrap_or(false)
        };

        // C++: clear cells with connectLayer > LAYER_GROUND resolve into hierarchical
        // with PathfindLayer::getZone() (bridge layer zone).
        if let (Some(connects), Some(lz)) = (connect_layers, layer_zones) {
            for x in 0..self.width {
                for y in 0..self.height {
                    let cl = connects.get(x).and_then(|c| c.get(y)).copied().unwrap_or(0);
                    // PathfindLayerEnum::Ground = 1; only layers above ground.
                    if cl <= PathfindLayerEnum::Ground as u8 {
                        continue;
                    }
                    if ct(x, y) != PathfindCellType::Clear {
                        continue;
                    }
                    let cell_z = self.zones[x][y];
                    if cell_z == 0 {
                        continue;
                    }
                    let layer_z = lz.get(cl as usize).copied().unwrap_or(0);
                    if layer_z != 0 {
                        resolve(&mut hierarchical, cell_z, layer_z);
                    }
                }
            }
        }

        for x in 0..self.width {
            for y in 0..self.height {
                let z1 = self.zones[x][y];
                let t1 = ct(x, y);
                // left neighbor
                if x > 0 {
                    let z0 = self.zones[x - 1][y];
                    let t0 = ct(x - 1, y);
                    if z0 != z1 && z0 != 0 && z1 != 0 {
                        // C++ horizontal: same type → hierarchical only; else terrain/crusher,
                        // then water/rubble/cliff only if neither terrain nor crusher matched.
                        if t0 == t1 {
                            resolve(&mut hierarchical, z0, z1);
                        } else {
                            let mut not_terrain_or_crusher = true;
                            if Self::pair_terrain(t0, t1) {
                                resolve(&mut terrain, z0, z1);
                                not_terrain_or_crusher = false;
                            }
                            if Self::pair_crusher_ground(
                                t0,
                                t1,
                                is_fence_obs(x - 1, y),
                                is_fence_obs(x, y),
                            ) {
                                resolve(&mut crusher, z0, z1);
                                not_terrain_or_crusher = false;
                            }
                            if not_terrain_or_crusher {
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
                if y > 0 {
                    let z0 = self.zones[x][y - 1];
                    let t0 = ct(x, y - 1);
                    if z0 != z1 && z0 != 0 && z1 != 0 {
                        // C++ vertical: same type → hierarchical; else terrain + crusher +
                        // water/rubble/cliff ladder (not gated by terrain/crusher in C++).
                        if t0 == t1 {
                            resolve(&mut hierarchical, z0, z1);
                        } else {
                            if Self::pair_terrain(t0, t1) {
                                resolve(&mut terrain, z0, z1);
                            }
                            if Self::pair_crusher_ground(
                                t0,
                                t1,
                                is_fence_obs(x, y - 1),
                                is_fence_obs(x, y),
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
        // Flatten hierarchical (C++ pathfind zone flatten loop).
        for i in 1..n {
            let z = hierarchical[i] as usize;
            if z < n {
                hierarchical[i] = hierarchical[z];
            }
        }
        // C++ flattenZones(surface, hierarchical) — compose surface through hierarchical.
        let flatten = |surface: &mut [u16], hier: &[u16]| {
            for i in 0..surface.len() {
                let z1 = surface[i] as usize;
                if z1 >= hier.len() {
                    continue;
                }
                let z2 = hier[z1] as usize;
                if z2 < surface.len() {
                    let z3 = surface[z2] as usize;
                    if z3 < hier.len() {
                        surface[i] = hier[z3];
                    } else {
                        surface[i] = hier[z1];
                    }
                } else {
                    surface[i] = hier[z1];
                }
            }
        };
        flatten(&mut cliff, &hierarchical);
        flatten(&mut water, &hierarchical);
        flatten(&mut rubble, &hierarchical);
        flatten(&mut terrain, &hierarchical);
        flatten(&mut crusher, &hierarchical);

        self.ground_cliff_zones = cliff;
        self.ground_water_zones = water;
        self.ground_rubble_zones = rubble;
        self.crusher_zones = crusher;
        self.hierarchical_zones = hierarchical;
        self.terrain_zones = terrain;
    }

    /// C++ allocateBlocks + blockCalculateZones for each ZONE_BLOCK_SIZE tile.
    pub(crate) fn rebuild_zone_blocks(
        &mut self,
        types: Option<&[Vec<PathfindCellType>]>,
        fence_flags: Option<&[Vec<bool>]>,
    ) {
        self.blocks_x = (self.width + ZONE_BLOCK_SIZE as usize - 1) / ZONE_BLOCK_SIZE as usize;
        self.blocks_y = (self.height + ZONE_BLOCK_SIZE as usize - 1) / ZONE_BLOCK_SIZE as usize;
        self.blocks_x = self.blocks_x.max(1);
        self.blocks_y = self.blocks_y.max(1);
        self.zone_blocks = vec![vec![BlockCombiner::identity(1, 1); self.blocks_y]; self.blocks_x];

        for bx in 0..self.blocks_x {
            for by in 0..self.blocks_y {
                let lo_x = bx * ZONE_BLOCK_SIZE as usize;
                let lo_y = by * ZONE_BLOCK_SIZE as usize;
                let hi_x = (lo_x + ZONE_BLOCK_SIZE as usize - 1).min(self.width.saturating_sub(1));
                let hi_y = (lo_y + ZONE_BLOCK_SIZE as usize - 1).min(self.height.saturating_sub(1));

                let mut min_z = u16::MAX;
                let mut max_z = 0u16;
                for x in lo_x..=hi_x {
                    for y in lo_y..=hi_y {
                        let z = self.zones[x][y];
                        if z == 0 {
                            continue;
                        }
                        min_z = min_z.min(z);
                        max_z = max_z.max(z);
                    }
                }
                if min_z == u16::MAX {
                    self.zone_blocks[bx][by] = BlockCombiner::identity(1, 1);
                    continue;
                }
                let num = max_z.saturating_sub(min_z).saturating_add(1);
                let mut block = BlockCombiner::identity(min_z, num);

                if num > 1 {
                    if let Some(types) = types {
                        let resolve = |table: &mut [u16], a: u16, b: u16, first: u16| {
                            if a < first || b < first {
                                return;
                            }
                            let ia = (a - first) as usize;
                            let ib = (b - first) as usize;
                            if ia >= table.len() || ib >= table.len() {
                                return;
                            }
                            let za = table[ia];
                            let zb = table[ib];
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
                        let ct = |x: usize, y: usize| {
                            types
                                .get(x)
                                .and_then(|c| c.get(y))
                                .copied()
                                .unwrap_or(PathfindCellType::Clear)
                        };
                        let fence = |x: usize, y: usize| {
                            fence_flags
                                .and_then(|f| f.get(x))
                                .and_then(|c| c.get(y))
                                .copied()
                                .unwrap_or(false)
                        };
                        for x in lo_x..=hi_x {
                            for y in lo_y..=hi_y {
                                let z1 = self.zones[x][y];
                                let t1 = ct(x, y);
                                if x > lo_x {
                                    let z0 = self.zones[x - 1][y];
                                    let t0 = ct(x - 1, y);
                                    if z0 != z1 && z0 != 0 && z1 != 0 {
                                        if Self::pair_water_ground(t0, t1) {
                                            resolve(&mut block.ground_water, z0, z1, min_z);
                                        }
                                        if Self::pair_ground_rubble(t0, t1) {
                                            resolve(&mut block.ground_rubble, z0, z1, min_z);
                                        }
                                        if Self::pair_ground_cliff(t0, t1) {
                                            resolve(&mut block.ground_cliff, z0, z1, min_z);
                                        }
                                        if Self::pair_crusher_ground(
                                            t0,
                                            t1,
                                            fence(x - 1, y),
                                            fence(x, y),
                                        ) {
                                            resolve(&mut block.crusher, z0, z1, min_z);
                                        }
                                    }
                                }
                                if y > lo_y {
                                    let z0 = self.zones[x][y - 1];
                                    let t0 = ct(x, y - 1);
                                    if z0 != z1 && z0 != 0 && z1 != 0 {
                                        if Self::pair_water_ground(t0, t1) {
                                            resolve(&mut block.ground_water, z0, z1, min_z);
                                        }
                                        if Self::pair_ground_rubble(t0, t1) {
                                            resolve(&mut block.ground_rubble, z0, z1, min_z);
                                        }
                                        if Self::pair_ground_cliff(t0, t1) {
                                            resolve(&mut block.ground_cliff, z0, z1, min_z);
                                        }
                                        if Self::pair_crusher_ground(
                                            t0,
                                            t1,
                                            fence(x, y - 1),
                                            fence(x, y),
                                        ) {
                                            resolve(&mut block.crusher, z0, z1, min_z);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                self.zone_blocks[bx][by] = block;
            }
        }
    }

    /// C++ `terrain()` — treat obstacle as clear, then types must match.
    pub(crate) fn pair_terrain(a: PathfindCellType, b: PathfindCellType) -> bool {
        let ta = if a == PathfindCellType::Obstacle {
            PathfindCellType::Clear
        } else {
            a
        };
        let tb = if b == PathfindCellType::Obstacle {
            PathfindCellType::Clear
        } else {
            b
        };
        ta == tb
    }

    pub(crate) fn pair_water_ground(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Water)
                | (PathfindCellType::Water, PathfindCellType::Clear)
        )
    }
    pub(crate) fn pair_ground_rubble(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Rubble)
                | (PathfindCellType::Rubble, PathfindCellType::Clear)
        )
    }
    pub(crate) fn pair_ground_cliff(a: PathfindCellType, b: PathfindCellType) -> bool {
        matches!(
            (a, b),
            (PathfindCellType::Clear, PathfindCellType::Cliff)
                | (PathfindCellType::Cliff, PathfindCellType::Clear)
        )
    }
    pub(crate) fn pair_crusher_ground(
        a: PathfindCellType,
        b: PathfindCellType,
        a_fence: bool,
        b_fence: bool,
    ) -> bool {
        (a == PathfindCellType::Obstacle && a_fence && b == PathfindCellType::Clear)
            || (b == PathfindCellType::Obstacle && b_fence && a == PathfindCellType::Clear)
    }
}
