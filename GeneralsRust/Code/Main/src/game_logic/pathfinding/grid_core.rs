use super::*;

impl PathfindingGrid {
    pub fn new(world_width: f32, world_height: f32, grid_size: f32) -> Self {
        Self::new_with_origin(Vec3::ZERO, world_width, world_height, grid_size)
    }

    pub fn new_with_origin(
        origin: Vec3,
        world_width: f32,
        world_height: f32,
        grid_size: f32,
    ) -> Self {
        let width = (world_width / grid_size).ceil() as i32;
        let height = (world_height / grid_size).ceil() as i32;
        let cells = (width.max(0) as usize).saturating_mul(height.max(0) as usize);
        let words = cells.div_ceil(64);
        let mut grid = Self {
            width,
            height,
            grid_size,
            origin,
            blocked_bits: vec![0u64; words],
            dynamic_bits: vec![0u64; words],
            cell_types: vec![PathfindCellType::Clear as u8; cells],
            pinched_bits: vec![0u64; words],
            terrain_zones: vec![0u16; cells],
            occ_fixed_mask: vec![0u16; cells],
            occ_goal_unit: vec![0u32; cells],
            occ_goal_aircraft: vec![0u32; cells],
            occ_moving_mask: vec![0u16; cells],
            fence_bits: vec![0u64; words],
            transparent_bits: vec![0u64; words],
            occ_goal_mask: vec![0u16; cells],
            occ_infantry_mask: vec![0u16; cells],
            occ_fixed_max_crushable: vec![0u8; cells],
            path_zones: vec![0u16; cells],
            ground_water_zones: Vec::new(),
            ground_cliff_zones: Vec::new(),
            ground_rubble_zones: Vec::new(),
            crusher_zones: Vec::new(),
            player_ally_masks: [0u16; 16],
            ground_connect: vec![0u8; cells],
            query_layer: PathfindLayerEnum::Ground as u8,
            query_seeker_id: 0,
            query_check_for_aircraft: false,
            query_from: None,
            query_orig_dest: None,
            logical_extent_lo: GridPos::new(0, 0),
            logical_extent_hi: GridPos::new(
                ((world_width / grid_size).floor() as i32 - 1)
                    .min(width.saturating_sub(1))
                    .max(0),
                ((world_height / grid_size).floor() as i32 - 1)
                    .min(height.saturating_sub(1))
                    .max(0),
            ),
            world_extent_w: world_width,
            world_extent_h: world_height,
            query_is_human: false,
            occ_obstacle_id: vec![0u32; cells],
            occ_obstacle_owner: vec![0xFFu8; cells],
            occ_obstacle_team: vec![0xFFu8; cells],
            permanent_blast_crater_cells: HashSet::new(),
            bridge_layers: Vec::new(),
            layer_occ: HashMap::new(),
            wall_pieces: Vec::new(),
            wall_cells: HashMap::new(),
            wall_height: 0.0,
            terrain_gen: 1,
            query_path_diameter: 1,
            query_is_crusher: false,
        };
        grid.refresh_logical_extent();
        grid
    }

    /// C++ `Pathfinder::processPathfindQueue` m_logicalExtent refresh
    /// (AIPathfind.cpp:5887-5897).
    pub fn refresh_logical_extent(&mut self) {
        let cell = self.grid_size.max(1.0);
        let Some(ext) = gamelogic::terrain::get_terrain_logic()
            .read()
            .ok()
            .map(|t| t.get_extent())
            .filter(|e| e.hi.x > e.lo.x && e.hi.y > e.lo.y)
        else {
            return;
        };
        // Leftover/C++ terrain is XY; host grid is XZ with origin.
        let mut lo_x = ((ext.lo.x - self.origin.x) / cell).floor() as i32;
        let mut lo_y = ((ext.lo.y - self.origin.z) / cell).floor() as i32;
        let mut hi_x = ((ext.hi.x - self.origin.x) / cell).floor() as i32 - 1;
        let mut hi_y = ((ext.hi.y - self.origin.z) / cell).floor() as i32 - 1;
        lo_x = lo_x.max(0);
        lo_y = lo_y.max(0);
        hi_x = hi_x.min(self.width.saturating_sub(1)).max(lo_x);
        hi_y = hi_y.min(self.height.saturating_sub(1)).max(lo_y);
        self.logical_extent_lo = GridPos::new(lo_x, lo_y);
        self.logical_extent_hi = GridPos::new(hi_x, hi_y);
    }

    #[inline]
    pub fn in_logical_extent(&self, cell: GridPos) -> bool {
        cell.x >= self.logical_extent_lo.x
            && cell.y >= self.logical_extent_lo.y
            && cell.x <= self.logical_extent_hi.x
            && cell.y <= self.logical_extent_hi.y
    }

    pub fn set_logical_extent(&mut self, lo: GridPos, hi: GridPos) {
        self.logical_extent_lo = lo;
        self.logical_extent_hi = hi;
    }

    pub fn set_query_is_human(&mut self, is_human: bool) {
        self.query_is_human = is_human;
    }

    /// C++ Player relationship ALLIES bits for occupancy crush-through.
    pub fn set_player_ally_masks(&mut self, masks: [u16; 16]) {
        self.player_ally_masks = masks;
    }

    #[inline]
    pub(super) fn ally_mask_for(&self, player: u32) -> u16 {
        self.player_ally_masks[player.min(15) as usize]
    }
    #[inline]
    pub(super) fn bit_index(&self, pos: GridPos) -> Option<usize> {
        if !self.is_valid_pos(pos) {
            return None;
        }
        Some(pos.y as usize * self.width as usize + pos.x as usize)
    }

    #[inline]
    pub(super) fn bit_test(bits: &[u64], idx: usize) -> bool {
        let w = idx >> 6;
        let b = idx & 63;
        bits.get(w)
            .map(|word| (word >> b) & 1 == 1)
            .unwrap_or(false)
    }

    #[inline]
    pub(super) fn bit_set(bits: &mut [u64], idx: usize, on: bool) {
        let w = idx >> 6;
        let b = idx & 63;
        if let Some(word) = bits.get_mut(w) {
            if on {
                *word |= 1u64 << b;
            } else {
                *word &= !(1u64 << b);
            }
        }
    }

    pub fn is_valid_pos(&self, pos: GridPos) -> bool {
        pos.x >= 0 && pos.x < self.width && pos.y >= 0 && pos.y < self.height
    }

    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    pub fn world_to_grid(&self, world_pos: Vec3) -> GridPos {
        // C++ Pathfinder::worldToGrid REAL_TO_INT truncate-toward-zero
        // (AIPathfind.h:856-858, BaseType.h:213). Host ground plane is XZ.
        GridPos {
            x: ((world_pos.x - self.origin.x) / self.grid_size) as i32,
            y: ((world_pos.z - self.origin.z) / self.grid_size) as i32,
        }
    }

    pub fn grid_to_world(&self, pos: GridPos) -> Vec3 {
        let x = self.origin.x + pos.x as f32 * self.grid_size;
        let z = self.origin.z + pos.y as f32 * self.grid_size;
        Vec3::new(x, self.cell_world_height(pos, x, z), z)
    }

    /// C++ `adjustCoordToCell` / `TerrainLogic::getLayerHeight` on a known layer.
    pub fn grid_to_world_on_layer(&self, pos: GridPos, layer: PathfindLayerEnum) -> Vec3 {
        let x = self.origin.x + pos.x as f32 * self.grid_size;
        let z = self.origin.z + pos.y as f32 * self.grid_size;
        Vec3::new(x, self.layer_world_height(pos, layer, x, z), z)
    }

    /// C++ `Pathfinder::adjustCoordToCell` (AIPathfind.cpp:8936-8946).
    /// Infantry / odd-diameter: cell center (+0.5). Vehicles / even: +0.05 inset.
    pub fn adjust_coord_to_cell(&self, pos: GridPos, center_in_cell: bool) -> Vec3 {
        let bias = if center_in_cell { 0.5 } else { 0.05 };
        let x = self.origin.x + (pos.x as f32 + bias) * self.grid_size;
        let z = self.origin.z + (pos.y as f32 + bias) * self.grid_size;
        Vec3::new(x, self.cell_world_height(pos, x, z), z)
    }

    /// `adjustCoordToCell` on a known pathfind layer.
    pub fn adjust_coord_to_cell_on_layer(
        &self,
        pos: GridPos,
        center_in_cell: bool,
        layer: PathfindLayerEnum,
    ) -> Vec3 {
        let bias = if center_in_cell { 0.5 } else { 0.05 };
        let x = self.origin.x + (pos.x as f32 + bias) * self.grid_size;
        let z = self.origin.z + (pos.y as f32 + bias) * self.grid_size;
        Vec3::new(x, self.layer_world_height(pos, layer, x, z), z)
    }

    pub(super) fn cell_world_height(&self, pos: GridPos, x: f32, z: f32) -> f32 {
        if self.wall_cells.contains_key(&(pos.x, pos.y)) && self.wall_height > 0.0 {
            return self.wall_height;
        }
        if let Some(id) = self.host_deck_layer_at(pos) {
            return self.layer_world_height(pos, PathfindLayerEnum::from_u32(id as u32), x, z);
        }
        sample_host_ground_height(x, z)
    }

    pub(super) fn layer_world_height(
        &self,
        pos: GridPos,
        layer: PathfindLayerEnum,
        x: f32,
        z: f32,
    ) -> f32 {
        let _ = pos;
        match layer {
            PathfindLayerEnum::Wall => {
                if self.wall_height > 0.0 {
                    self.wall_height
                } else {
                    sample_host_ground_height(x, z)
                }
            }
            PathfindLayerEnum::Ground | PathfindLayerEnum::Invalid => {
                sample_host_ground_height(x, z)
            }
            _ => {
                if let Some(bridge) = self.bridge_layers.iter().find(|l| l.id == layer as u8) {
                    let corners = [
                        bridge.from_left,
                        bridge.from_right,
                        bridge.to_right,
                        bridge.to_left,
                    ];
                    return bridge_deck_height(&corners, x, z);
                }
                sample_host_ground_height(x, z)
            }
        }
    }

    pub fn is_blocked(&self, pos: GridPos) -> bool {
        self.is_static_blocked(pos)
            || self
                .bit_index(pos)
                .is_some_and(|idx| Self::bit_test(&self.dynamic_bits, idx))
    }

    pub fn is_static_blocked(&self, pos: GridPos) -> bool {
        let Some(idx) = self.bit_index(pos) else {
            return true;
        };
        if Self::bit_test(&self.blocked_bits, idx) {
            return true;
        }
        matches!(
            self.cell_type_at_index(idx),
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        )
    }

    #[inline]
    pub(super) fn cell_type_at_index(&self, idx: usize) -> PathfindCellType {
        match self.cell_types.get(idx).copied().unwrap_or(0) {
            0x01 => PathfindCellType::Water,
            0x02 => PathfindCellType::Cliff,
            0x03 => PathfindCellType::Rubble,
            0x04 => PathfindCellType::Obstacle,
            0x05 => PathfindCellType::BridgeImpassable,
            0x06 => PathfindCellType::Impassable,
            _ => PathfindCellType::Clear,
        }
    }

    /// C++ PathfindCell::getType residual (AIPathfind.h:233-242).
    pub fn cell_type(&self, pos: GridPos) -> PathfindCellType {
        match self.bit_index(pos) {
            Some(idx) => self.cell_type_at_index(idx),
            None => PathfindCellType::Impassable,
        }
    }

    /// Classify a cell without claiming full locomotor surfaces.
    /// Water/Cliff stay walk-costed (not hard-blocked); Impassable/Obstacle set bits.
    /// Fence/transparent bits are independent flags (C++ PathfindCellInfo).
    pub fn set_cell_type(&mut self, pos: GridPos, ty: PathfindCellType) {
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        if let Some(slot) = self.cell_types.get_mut(idx) {
            *slot = ty as u8;
        }
        let hard = matches!(
            ty,
            PathfindCellType::Impassable
                | PathfindCellType::Obstacle
                | PathfindCellType::BridgeImpassable
        );
        Self::bit_set(&mut self.blocked_bits, idx, hard);
        if !matches!(ty, PathfindCellType::Obstacle) {
            Self::bit_set(&mut self.fence_bits, idx, false);
            Self::bit_set(&mut self.transparent_bits, idx, false);
            if let Some(slot) = self.occ_obstacle_id.get_mut(idx) {
                *slot = 0;
            }
            if let Some(slot) = self.occ_obstacle_owner.get_mut(idx) {
                *slot = 0xFF;
            }
            if let Some(slot) = self.occ_obstacle_team.get_mut(idx) {
                *slot = 0xFF;
            }
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }

    /// C++ `PathfindCell::setTypeAsObstacle(..., isFence)`.
    pub fn set_cell_obstacle(&mut self, pos: GridPos, is_fence: bool, is_transparent: bool) {
        self.set_cell_obstacle_owned(pos, is_fence, is_transparent, 0, None, None);
    }

    /// C++ `setTypeAsObstacle` + `getObstacleID` owner for dozerHack.
    pub fn set_cell_obstacle_owned(
        &mut self,
        pos: GridPos,
        is_fence: bool,
        is_transparent: bool,
        object_id: u32,
        owner_player: Option<u32>,
        team: Option<Team>,
    ) {
        self.set_cell_type(pos, PathfindCellType::Obstacle);
        let Some(idx) = self.bit_index(pos) else {
            return;
        };
        Self::bit_set(&mut self.fence_bits, idx, is_fence);
        Self::bit_set(&mut self.transparent_bits, idx, is_transparent);
        if let Some(slot) = self.occ_obstacle_id.get_mut(idx) {
            *slot = object_id;
        }
        if let Some(slot) = self.occ_obstacle_owner.get_mut(idx) {
            *slot = owner_player
                .filter(|&p| p <= 15)
                .map(|p| p as u8)
                .unwrap_or(0xFF);
        }
        if let Some(slot) = self.occ_obstacle_team.get_mut(idx) {
            *slot = team.map(|t| t as u8).unwrap_or(0xFF);
        }
    }

    /// C++ `PathfindCell::removeObstacle` (AIPathfind.cpp:1473-1483).
    pub fn clear_cell_obstacle_owned(&mut self, pos: GridPos, object_id: u32) -> bool {
        if self.cell_type(pos) == PathfindCellType::Rubble {
            self.set_cell_type(pos, PathfindCellType::Clear);
        }
        let Some((id, _, _)) = self.obstacle_owner(pos) else {
            return false;
        };
        if id != object_id {
            return false;
        }
        self.set_cell_type(pos, PathfindCellType::Clear);
        true
    }

    pub(super) fn obstacle_owner(&self, pos: GridPos) -> Option<(u32, Option<u32>, Option<Team>)> {
        let idx = self.bit_index(pos)?;
        let id = *self.occ_obstacle_id.get(idx)?;
        if id == 0 {
            return None;
        }
        let owner = self.occ_obstacle_owner.get(idx).copied().and_then(|p| {
            if p == 0xFF {
                None
            } else {
                Some(p as u32)
            }
        });
        let team = self
            .occ_obstacle_team
            .get(idx)
            .copied()
            .and_then(|t| match t {
                0 => Some(Team::GLA),
                1 => Some(Team::USA),
                2 => Some(Team::China),
                3 => Some(Team::Neutral),
                _ => None,
            });
        Some((id, owner, team))
    }

    pub fn is_obstacle_fence(&self, pos: GridPos) -> bool {
        self.bit_index(pos)
            .is_some_and(|idx| Self::bit_test(&self.fence_bits, idx))
    }

    pub fn is_obstacle_transparent(&self, pos: GridPos) -> bool {
        self.bit_index(pos)
            .is_some_and(|idx| Self::bit_test(&self.transparent_bits, idx))
    }

    /// C++ Pathfinder::isAttackViewBlockedByObstacle residual (static obstacles only).
    /// Bresenham walk from `from`→`to`; only CELL_OBSTACLE blocks, after identity
    /// / transparent / victim-cell skips. No Chebyshev-4 blind zone.
    pub fn is_attack_view_blocked_static(&self, from: Vec3, to: Vec3) -> bool {
        self.is_attack_view_blocked_static_ex(from, to, 0, &[])
    }

    /// C++ `attackBlockedByObstacleCallback` + leftover skip-3 / identity / victim-cell.
    pub(super) fn is_attack_view_blocked_static_ex(
        &self,
        from: Vec3,
        to: Vec3,
        mut skip_count: i32,
        skip_ids: &[u32],
    ) -> bool {
        let start = self.world_to_grid(from);
        let goal = self.world_to_grid(to);
        if start == goal {
            return false;
        }
        if start.manhattan_distance(goal) <= 1 {
            return false;
        }
        let victim_obs = self.obstacle_owner(goal).map(|(id, _, _)| id).unwrap_or(0);
        let mut x0 = start.x;
        let mut y0 = start.y;
        let x1 = goal.x;
        let y1 = goal.y;
        let dx = (x1 - x0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let dy = -(y1 - y0).abs();
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx + dy;
        loop {
            let e2 = 2 * err;
            if e2 >= dy {
                if x0 == x1 {
                    break;
                }
                err += dy;
                x0 += sx;
            }
            if e2 <= dx {
                if y0 == y1 {
                    break;
                }
                err += dx;
                y0 += sy;
            }
            let cell = GridPos::new(x0, y0);
            if cell == goal {
                break;
            }
            if !self.is_valid_pos(cell) {
                continue;
            }
            // C++ skipCount: first N cells off a bridge/rooftop are see-through.
            if skip_count > 0 {
                skip_count -= 1;
                continue;
            }
            // C++ attackBlockedByObstacleCallback: only CELL_OBSTACLE.
            if self.cell_type(cell) != PathfindCellType::Obstacle {
                continue;
            }
            if self.is_obstacle_transparent(cell) {
                continue;
            }
            let obs_id = self.obstacle_owner(cell).map(|(id, _, _)| id).unwrap_or(0);
            if obs_id != 0 {
                if skip_ids.contains(&obs_id) {
                    continue;
                }
                if victim_obs != 0 && obs_id == victim_obs {
                    continue;
                }
            }
            return true;
        }
        false
    }

    /// Live-host name used by attack/mood/save. Same residual as static LOS.
    pub fn is_attack_view_blocked(&self, from: Vec3, to: Vec3) -> bool {
        self.is_attack_view_blocked_static(from, to)
    }

    pub fn set_blocked(&mut self, pos: GridPos, blocked: bool) {
        if blocked {
            self.set_cell_type(pos, PathfindCellType::Obstacle);
        } else {
            self.set_cell_type(pos, PathfindCellType::Clear);
        }
    }

    /// Mark a structure footprint as static-blocked (C++ pathfind obstacle residual).
    /// `radius_cells` is half-extent in grid cells (1 => 3×3).
    pub fn block_structure_footprint(&mut self, center: GridPos, radius_cells: i32) {
        self.block_structure_footprint_ex(center, radius_cells, false, false);
    }

    /// Block a structure footprint from a world position (cell radius).
    pub fn block_structure_at_world(&mut self, pos: Vec3, radius_cells: i32) {
        let center = self.world_to_grid(pos);
        self.block_structure_footprint(center, radius_cells);
    }

    pub fn block_structure_footprint_ex(
        &mut self,
        center: GridPos,
        radius_cells: i32,
        is_fence: bool,
        is_transparent: bool,
    ) {
        let r = radius_cells.max(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let p = GridPos::new(center.x + dx, center.y + dy);
                if self.is_valid_pos(p) {
                    self.set_cell_obstacle(p, is_fence, is_transparent);
                }
            }
        }
    }

    /// C++ `setTypeAsObstacle` BODY_RUBBLE → CELL_RUBBLE.
    pub fn stamp_rubble_footprint(&mut self, center: GridPos, radius_cells: i32) {
        let r = radius_cells.max(0);
        for dy in -r..=r {
            for dx in -r..=r {
                let p = GridPos::new(center.x + dx, center.y + dy);
                if self.is_valid_pos(p) {
                    self.set_cell_type(p, PathfindCellType::Rubble);
                }
            }
        }
    }

    pub(super) fn object_is_pathfind_rubble(obj: &Object) -> bool {
        use crate::game_logic::host_enum_table_residual::HostBodyDamageType;
        obj.status.keep_as_rubble
            || obj.body_damage_state == HostBodyDamageType::Rubble
            || obj.keep_object_die.as_ref().is_some_and(|d| d.is_rubble)
    }

    /// C++ `Pathfinder::classifyFence` raster (AIPathfind.cpp:3983+).
    pub fn classify_fence_world(
        &mut self,
        world: Vec3,
        orientation: f32,
        fence_width: f32,
        fence_x_offset: f32,
        is_transparent: bool,
        object_id: u32,
        owner_player: Option<u32>,
        team: Option<Team>,
    ) -> Option<(GridPos, GridPos)> {
        if fence_width <= 0.0 {
            return None;
        }
        let halfsize_x = fence_width * 0.5;
        let halfsize_y = self.grid_size / 10.0;
        let (s, c) = orientation.sin_cos();
        let step = self.grid_size * 0.5;
        let ydx = s * step;
        let ydy = -c * step;
        let xdx = c * step;
        let xdy = s * step;
        let num_steps_x = ((2.0 * halfsize_x / step).ceil() as i32).max(1);
        let num_steps_y = ((2.0 * halfsize_y / step).ceil() as i32).max(1);
        let mut tl_x = world.x - fence_x_offset * c - halfsize_y * s;
        let mut tl_z = world.z + halfsize_y * c - fence_x_offset * s;
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        for _iy in 0..num_steps_y {
            let mut x = tl_x;
            let mut z = tl_z;
            for _ix in 0..num_steps_x {
                let cell = self.classify_world_to_cell(x, z);
                if self.is_valid_pos(cell) {
                    self.set_cell_obstacle_owned(
                        cell,
                        true,
                        is_transparent,
                        object_id,
                        owner_player,
                        team,
                    );
                    Self::expand_stamp_bounds(&mut lo, &mut hi, cell);
                    did = true;
                }
                x += xdx;
                z += xdy;
            }
            tl_x += ydx;
            tl_z += ydy;
        }
        if did {
            Some((lo, hi))
        } else {
            None
        }
    }

    /// C++ `REAL_TO_INT_FLOOR((x+0.5)/PATHFIND_CELL_SIZE)` used by classify raster.
    pub(super) fn classify_world_to_cell(&self, x: f32, z: f32) -> GridPos {
        GridPos {
            x: ((x - self.origin.x + 0.5) / self.grid_size).floor() as i32,
            y: ((z - self.origin.z + 0.5) / self.grid_size).floor() as i32,
        }
    }

    pub(super) fn expand_stamp_bounds(lo: &mut GridPos, hi: &mut GridPos, cell: GridPos) {
        lo.x = lo.x.min(cell.x);
        lo.y = lo.y.min(cell.y);
        hi.x = hi.x.max(cell.x);
        hi.y = hi.y.max(cell.y);
    }

    /// Leftover `classify.rs` / C++ `internal_classifyObjectFootprint` (AIPathfind.cpp:4175-4290).
    /// Oriented GEOMETRY_BOX raster or cylinder/sphere disc. Returns stamped bounds.
    pub fn classify_object_footprint(
        &mut self,
        obj: &Object,
        as_rubble: bool,
    ) -> Option<(GridPos, GridPos)> {
        if obj.is_kind_of(KindOf::Mine)
            || obj.is_kind_of(KindOf::Projectile)
            || obj.is_kind_of(KindOf::BridgeTower)
        {
            return None;
        }
        let fence_width = obj.thing.template.fence_width;
        if fence_width > 0.0 && !obj.is_kind_of(KindOf::DefensiveWall) {
            return self.classify_fence_world(
                obj.get_position(),
                obj.get_orientation(),
                fence_width,
                obj.thing.template.fence_x_offset,
                obj.is_kind_of(KindOf::CanSeeThrough),
                obj.id.0,
                obj.owner_player_id,
                Some(obj.team),
            );
        }
        if !obj.is_kind_of(KindOf::Structure) {
            return None;
        }
        if obj.is_mobile() {
            return None;
        }
        let geom = obj.thing.template.geometry_info;
        if geom.authored && geom.is_small {
            return None;
        }
        let pos = obj.get_position();
        let height_above = pos.y - sample_host_ground_height(pos.x, pos.z);
        let is_blast_crater = obj.is_kind_of(KindOf::BlastCrater);
        // C++ skips airborne bounds unless KINDOF_BLAST_CRATER (AIPathfind.cpp:4168-4171).
        if height_above > self.grid_size && !is_blast_crater {
            return None;
        }
        let is_transparent = obj.is_kind_of(KindOf::CanSeeThrough);
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        let stamp = |grid: &mut Self, cell: GridPos, lo: &mut GridPos, hi: &mut GridPos| {
            if !grid.is_valid_pos(cell) {
                return false;
            }
            if as_rubble {
                grid.set_cell_type(cell, PathfindCellType::Rubble);
            } else {
                grid.set_cell_obstacle_owned(
                    cell,
                    false,
                    is_transparent,
                    obj.id.0,
                    obj.owner_player_id,
                    Some(obj.team),
                );
            }
            if is_blast_crater {
                grid.permanent_blast_crater_cells.insert((cell.x, cell.y));
            }
            Self::expand_stamp_bounds(lo, hi, cell);
            true
        };
        let geom_type = if geom.authored {
            geom.geom_type
        } else {
            crate::game_logic::HostGeometryType::Cylinder
        };
        let major = if geom.authored && geom.major_radius > 0.0 {
            geom.major_radius
        } else {
            obj.selection_radius.max(self.grid_size * 0.5)
        };
        let minor = if geom.authored && geom.minor_radius > 0.0 {
            geom.minor_radius
        } else {
            major
        };
        match geom_type {
            crate::game_logic::HostGeometryType::Box => {
                let angle = obj.get_orientation();
                let (s, c) = angle.sin_cos();
                let step = self.grid_size * 0.5;
                let ydx = s * step;
                let ydy = -c * step;
                let xdx = c * step;
                let xdy = s * step;
                let num_steps_x = ((2.0 * major / step).ceil() as i32).max(1);
                let num_steps_y = ((2.0 * minor / step).ceil() as i32).max(1);
                let mut tl_x = pos.x - major * c - minor * s;
                let mut tl_z = pos.z + minor * c - major * s;
                for _iy in 0..num_steps_y {
                    let mut x = tl_x;
                    let mut z = tl_z;
                    for _ix in 0..num_steps_x {
                        let cell = self.classify_world_to_cell(x, z);
                        if stamp(self, cell, &mut lo, &mut hi) {
                            did = true;
                        }
                        x += xdx;
                        z += xdy;
                    }
                    tl_x += ydx;
                    tl_z += ydy;
                }
            }
            crate::game_logic::HostGeometryType::Sphere
            | crate::game_logic::HostGeometryType::Cylinder => {
                // C++ cylinder: cell-center disc, size = radius/cell + 0.4.
                let size = major / self.grid_size + 0.4;
                let r2 = size * size;
                let center_x = (pos.x - self.origin.x) / self.grid_size;
                let center_y = (pos.z - self.origin.z) / self.grid_size;
                let top_left_x =
                    ((pos.x - self.origin.x - major) / self.grid_size + 0.5).floor() as i32 - 1;
                let top_left_y =
                    ((pos.z - self.origin.z - major) / self.grid_size + 0.5).floor() as i32 - 1;
                let bottom_right_x = top_left_x + (2.0 * size) as i32 + 2;
                let bottom_right_y = top_left_y + (2.0 * size) as i32 + 2;
                for j in top_left_y..bottom_right_y {
                    for i in top_left_x..bottom_right_x {
                        let dx = i as f32 + 0.5 - center_x;
                        let dy = j as f32 + 0.5 - center_y;
                        if dx * dx + dy * dy > r2 {
                            continue;
                        }
                        if stamp(self, GridPos::new(i, j), &mut lo, &mut hi) {
                            did = true;
                        }
                    }
                }
            }
        }
        if did {
            Some((lo, hi))
        } else {
            None
        }
    }

    /// C++ `internal_classifyObjectFootprint` (AIPathfind.cpp:4175).
    /// `createAWallFromMyFootprint` / `removeWallFromMyFootprint` — no structure/mobile skip.
    pub fn internal_classify_object_footprint(
        &mut self,
        obj: &Object,
        insert: bool,
    ) -> Option<(GridPos, GridPos)> {
        let geom = obj.thing.template.geometry_info;
        let pos = obj.get_position();
        let is_transparent = obj.is_kind_of(KindOf::CanSeeThrough);
        let mut lo = GridPos::new(i32::MAX, i32::MAX);
        let mut hi = GridPos::new(i32::MIN, i32::MIN);
        let mut did = false;
        let stamp = |grid: &mut Self, cell: GridPos, lo: &mut GridPos, hi: &mut GridPos| {
            if !grid.is_valid_pos(cell) {
                return false;
            }
            let changed = if insert {
                grid.set_cell_obstacle_owned(
                    cell,
                    false,
                    is_transparent,
                    obj.id.0,
                    obj.owner_player_id,
                    Some(obj.team),
                );
                true
            } else {
                grid.clear_cell_obstacle_owned(cell, obj.id.0)
            };
            if changed {
                Self::expand_stamp_bounds(lo, hi, cell);
            }
            changed
        };
        let geom_type = if geom.authored {
            geom.geom_type
        } else {
            crate::game_logic::HostGeometryType::Cylinder
        };
        let major = if geom.authored && geom.major_radius > 0.0 {
            geom.major_radius
        } else {
            obj.selection_radius.max(self.grid_size * 0.5)
        };
        let minor = if geom.authored && geom.minor_radius > 0.0 {
            geom.minor_radius
        } else {
            major
        };
        match geom_type {
            crate::game_logic::HostGeometryType::Box => {
                let angle = obj.get_orientation();
                let (s, c) = angle.sin_cos();
                let step = self.grid_size * 0.5;
                let ydx = s * step;
                let ydy = -c * step;
                let xdx = c * step;
                let xdy = s * step;
                let num_steps_x = ((2.0 * major / step).ceil() as i32).max(1);
                let num_steps_y = ((2.0 * minor / step).ceil() as i32).max(1);
                let mut tl_x = pos.x - major * c - minor * s;
                let mut tl_z = pos.z + minor * c - major * s;
                for _iy in 0..num_steps_y {
                    let mut x = tl_x;
                    let mut z = tl_z;
                    for _ix in 0..num_steps_x {
                        let cell = self.classify_world_to_cell(x, z);
                        if stamp(self, cell, &mut lo, &mut hi) {
                            did = true;
                        }
                        x += xdx;
                        z += xdy;
                    }
                    tl_x += ydx;
                    tl_z += ydy;
                }
            }
            crate::game_logic::HostGeometryType::Sphere
            | crate::game_logic::HostGeometryType::Cylinder => {
                let size = major / self.grid_size + 0.4;
                let r2 = size * size;
                let center_x = (pos.x - self.origin.x) / self.grid_size;
                let center_y = (pos.z - self.origin.z) / self.grid_size;
                let top_left_x =
                    ((pos.x - self.origin.x - major) / self.grid_size + 0.5).floor() as i32 - 1;
                let top_left_y =
                    ((pos.z - self.origin.z - major) / self.grid_size + 0.5).floor() as i32 - 1;
                let bottom_right_x = top_left_x + (2.0 * size) as i32 + 2;
                let bottom_right_y = top_left_y + (2.0 * size) as i32 + 2;
                for j in top_left_y..bottom_right_y {
                    for i in top_left_x..bottom_right_x {
                        let dx = i as f32 + 0.5 - center_x;
                        let dy = j as f32 + 0.5 - center_y;
                        if dx * dx + dy * dy > r2 {
                            continue;
                        }
                        if stamp(self, GridPos::new(i, j), &mut lo, &mut hi) {
                            did = true;
                        }
                    }
                }
            }
        }
        if did {
            Some((lo, hi))
        } else {
            None
        }
    }

    /// C++ `Pathfinder::createAWallFromMyFootprint`.
    pub fn create_wall_from_object(&mut self, obj: &Object) -> Option<(GridPos, GridPos)> {
        let bounds = self.internal_classify_object_footprint(obj, true)?;
        self.refresh_pinched_bounds(bounds.0, bounds.1);
        Some(bounds)
    }

    /// C++ `Pathfinder::removeWallFromMyFootprint`.
    pub fn remove_wall_from_object(&mut self, obj: &Object) -> Option<(GridPos, GridPos)> {
        let bounds = self.internal_classify_object_footprint(obj, false)?;
        self.refresh_pinched_bounds(bounds.0, bounds.1);
        Some(bounds)
    }

    /// C++ never-remove: re-OR BLAST_CRATER cells after terrain/object rebuild.
    pub(super) fn restamp_permanent_blast_craters(
        &mut self,
        lo: &mut GridPos,
        hi: &mut GridPos,
        did: &mut bool,
    ) {
        let cells: Vec<(i32, i32)> = self.permanent_blast_crater_cells.iter().copied().collect();
        for (x, y) in cells {
            let cell = GridPos::new(x, y);
            self.set_cell_obstacle(cell, false, false);
            Self::expand_stamp_bounds(lo, hi, cell);
            *did = true;
        }
    }

    /// Leftover `AStarPathfinder::refresh_pinched_cells_in_bounds` (C++ AIPathfind.cpp:4404-4477).
    pub fn refresh_pinched_cells_in_bounds(&mut self, lo: GridPos, hi: GridPos) {
        let min_x = lo.x.max(0);
        let min_y = lo.y.max(0);
        let max_x = hi.x.min(self.width.saturating_sub(1));
        let max_y = hi.y.min(self.height.saturating_sub(1));
        if min_x > max_x || min_y > max_y {
            return;
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.cell_type(p) == PathfindCellType::Impassable {
                    self.set_cell_type(p, PathfindCellType::Clear);
                }
                self.set_pinched(p, false);
            }
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.cell_type(p) != PathfindCellType::Clear {
                    continue;
                }
                let mut total_count = 0;
                let mut orthogonal_count = 0;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        let n = GridPos::new(x + dx, y + dy);
                        if !self.is_valid_pos(n) {
                            continue;
                        }
                        if self.cell_type(n) == PathfindCellType::Clear {
                            total_count += 1;
                            if dx == 0 || dy == 0 {
                                orthogonal_count += 1;
                            }
                        }
                    }
                }
                if orthogonal_count < 2 || total_count < 4 {
                    self.set_pinched(p, true);
                }
            }
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.is_pinched(p) && self.cell_type(p) == PathfindCellType::Clear {
                    self.set_cell_type(p, PathfindCellType::Impassable);
                    self.set_pinched(p, false);
                }
            }
        }
        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let p = GridPos::new(x, y);
                if self.cell_type(p) != PathfindCellType::Clear {
                    continue;
                }
                let mut obstacle_adjacent = false;
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if dx == 0 && dy == 0 {
                            continue;
                        }
                        if dx != 0 && dy != 0 {
                            continue;
                        }
                        let n = GridPos::new(x + dx, y + dy);
                        if !self.is_valid_pos(n) {
                            continue;
                        }
                        if self.cell_type(n) == PathfindCellType::Obstacle {
                            obstacle_adjacent = true;
                            break;
                        }
                    }
                    if obstacle_adjacent {
                        break;
                    }
                }
                if obstacle_adjacent {
                    self.set_pinched(p, true);
                }
            }
        }
    }

    /// Leftover `refresh_pinched_bounds`: expand stamped region by 2 cells then pinch.
    pub fn refresh_pinched_bounds(&mut self, lo: GridPos, hi: GridPos) {
        if lo.x == i32::MAX {
            return;
        }
        let lo = GridPos::new((lo.x - 2).max(0), (lo.y - 2).max(0));
        let hi = GridPos::new(
            (hi.x + 2).min(self.width.saturating_sub(1)),
            (hi.y + 2).min(self.height.saturating_sub(1)),
        );
        self.refresh_pinched_cells_in_bounds(lo, hi);
    }

    pub fn clear_static_blocks(&mut self) {
        self.blocked_bits.fill(0);
        self.cell_types.fill(PathfindCellType::Clear as u8);
        self.pinched_bits.fill(0);
        self.terrain_zones.fill(0);
        self.path_zones.fill(0);
        self.ground_water_zones.clear();
        self.ground_cliff_zones.clear();
        self.ground_rubble_zones.clear();
        self.crusher_zones.clear();
        self.fence_bits.fill(0);
        self.transparent_bits.fill(0);
        self.ground_connect.fill(0);
        self.bridge_layers.clear();
        // C++ classifyMap keeps m_layers[LAYER_WALL] when pieces remain
        // (AIPathfind.cpp:4650-4651). Do not drop the deck on terrain rebuild.
        self.allocate_and_classify_wall_layer();
    }

    pub fn export_static_block_mask(&self) -> Vec<bool> {
        let mut mask = vec![false; (self.width.max(0) * self.height.max(0)) as usize];
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                mask[idx] = self.is_static_blocked(GridPos::new(x, y));
            }
        }
        mask
    }

    pub fn import_static_block_mask(&mut self, width: i32, height: i32, mask: &[bool]) -> bool {
        if width != self.width || height != self.height {
            return false;
        }

        let expected_len = (self.width * self.height) as usize;
        if mask.len() != expected_len {
            return false;
        }

        self.clear_static_blocks();
        for y in 0..self.height {
            for x in 0..self.width {
                let idx = (y * self.width + x) as usize;
                if mask[idx] {
                    self.set_blocked(GridPos::new(x, y), true);
                }
            }
        }
        true
    }

    pub fn grid_size(&self) -> f32 {
        self.grid_size
    }

    pub fn width(&self) -> i32 {
        self.width
    }

    pub fn height(&self) -> i32 {
        self.height
    }

    pub fn set_dynamic_blocked(&mut self, pos: GridPos, blocked: bool) {
        if let Some(idx) = self.bit_index(pos) {
            Self::bit_set(&mut self.dynamic_bits, idx, blocked);
        }
    }

    pub fn clear_dynamic_blocks(&mut self) {
        self.dynamic_bits.fill(0);
        self.occ_fixed_mask.fill(0);
        self.occ_moving_mask.fill(0);
        self.occ_goal_mask.fill(0);
        self.occ_goal_unit.fill(0);
        self.occ_goal_aircraft.fill(0);
        self.occ_infantry_mask.fill(0);
        self.occ_fixed_max_crushable.fill(0);
        self.layer_occ.clear();
    }

    pub fn is_pinched(&self, pos: GridPos) -> bool {
        self.bit_index(pos)
            .is_some_and(|idx| Self::bit_test(&self.pinched_bits, idx))
    }

    pub fn set_pinched(&mut self, pos: GridPos, pinched: bool) {
        if let Some(idx) = self.bit_index(pos) {
            Self::bit_set(&mut self.pinched_bits, idx, pinched);
        }
    }

    /// C++ Pathfinder::classifyMap cliff expand (AIPathfind.cpp:4591-4632).
    pub fn pinch_tighten_cliffs(&mut self) {
        let w = self.width;
        let h = self.height;
        if w <= 0 || h <= 0 {
            return;
        }
        self.pinched_bits.fill(0);
        let mut first_ring = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let pos = GridPos::new(x, y);
                if self.cell_type(pos) != PathfindCellType::Cliff {
                    continue;
                }
                for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
                    for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                        let n = GridPos::new(nx, ny);
                        if self.cell_type(n) == PathfindCellType::Clear {
                            first_ring.push(n);
                        }
                    }
                }
            }
        }
        for pos in &first_ring {
            self.set_pinched(*pos, true);
        }
        for pos in first_ring {
            if self.cell_type(pos) == PathfindCellType::Clear {
                self.set_cell_type(pos, PathfindCellType::Cliff);
            }
        }
        let mut second_ring = Vec::new();
        for y in 0..h {
            for x in 0..w {
                let pos = GridPos::new(x, y);
                if self.cell_type(pos) != PathfindCellType::Cliff {
                    continue;
                }
                for ny in (y - 1).max(0)..=(y + 1).min(h - 1) {
                    for nx in (x - 1).max(0)..=(x + 1).min(w - 1) {
                        let n = GridPos::new(nx, ny);
                        if self.cell_type(n) == PathfindCellType::Clear {
                            second_ring.push(n);
                        }
                    }
                }
            }
        }
        for pos in second_ring {
            self.set_pinched(pos, true);
        }
        self.terrain_gen = self.terrain_gen.wrapping_add(1);
    }
}
