//! TerrainLogic terrain ops behavior.

use super::*;

impl TerrainLogic {
    pub(super) fn checked_height_sample_len(
        width: i32,
        height: i32,
        border: i32,
    ) -> Result<usize, String> {
        if width <= 0
            || height <= 0
            || border < 0
            || width
                .checked_add(border)
                .and_then(|n| n.checked_add(border))
                .and_then(|n| n.checked_add(132))
                .is_none()
            || height
                .checked_add(border)
                .and_then(|n| n.checked_add(border))
                .and_then(|n| n.checked_add(132))
                .is_none()
        {
            return Err("invalid terrain height dimensions/border".into());
        }
        let len = (width as usize)
            .checked_mul(height as usize)
            .ok_or("terrain height sample count overflow")?;
        if len > isize::MAX as usize {
            return Err("terrain height allocation exceeds addressable slice".into());
        }
        Ok(len)
    }
    /// Every admitted map is either the canonical unloaded tuple or a full grid.
    pub fn validate_height_map(&self) -> Result<(), String> {
        if self.map_dx == 0 && self.map_dy == 0 && self.border_size == 0 && self.map_data.is_empty()
        {
            return Ok(());
        }
        let expected = Self::checked_height_sample_len(self.map_dx, self.map_dy, self.border_size)?;
        if expected != self.map_data.len() {
            return Err("terrain height sample count disagrees with dimensions".into());
        }
        Ok(())
    }

    pub fn height_revision(&self) -> u64 {
        self.height_revision
    }
    pub fn logic_height_map_border_size(&self) -> i32 {
        self.border_size
    }
    pub(super) fn bump_height_revision(&mut self) {
        self.height_revision = self.height_revision.wrapping_add(1);
    }

    /// Get active boundary
    pub fn get_active_boundary(&self) -> i32 {
        self.active_boundary
    }

    /// C++ `WorldHeightMap` extent used by `W3DTerrainVisual::xfer` v>=2.
    pub fn logic_height_map_extents(&self) -> (i32, i32) {
        (self.map_dx, self.map_dy)
    }

    /// Raw logic-height bytes (`m_logicHeightMap->getDataPtr()`).
    pub fn logic_height_map_bytes(&self) -> &[u8] {
        &self.map_data
    }

    /// Apply C++ `W3DTerrainVisual::xfer` v>=2 height-map payload.
    /// Copies `min(len, xferLen)` into the existing buffer, then notifies the
    /// visual (`staticLightingChanged`).
    pub fn apply_logic_height_map_bytes(&mut self, data: &[u8]) {
        self.apply_logic_height_map_bytes_with_effects(data, &mut || {
            if let Some(visual) = crate::helpers::TheTerrainVisual::get() {
                visual.static_lighting_changed();
            }
        });
    }
    /// Owned Xfer admission: same prefix copy and synchronous lighting phase;
    /// a detached candidate supplies its own sink, never a published visual.
    pub fn apply_logic_height_map_bytes_with_effects(
        &mut self,
        data: &[u8],
        effect: &mut impl FnMut(),
    ) {
        if let Err(error) = self.try_apply_logic_height_map_bytes_with_effects(data, effect) {
            log::error!("terrain height admission: {error}");
        }
    }
    pub fn try_apply_logic_height_map_bytes_with_effects(
        &mut self,
        data: &[u8],
        effect: &mut impl FnMut(),
    ) -> Result<(), String> {
        self.validate_height_map()?;
        if self.map_data.is_empty() {
            if data.is_empty() {
                return Ok(());
            }
            return Err("cannot apply height bytes without an admitted receiving grid".into());
        }
        let mut len = (self.map_dx as usize).saturating_mul(self.map_dy as usize);
        len = len.min(self.map_data.len()).min(data.len());
        if self.map_data[..len] != data[..len] {
            self.bump_height_revision();
        }
        self.map_data[..len].copy_from_slice(&data[..len]);
        self.cliff_state
            .rebuild(&self.map_data, self.map_dx, self.map_dy);

        effect();

        Ok(())
    }

    /// Restore host `TerrainSnapshot` logic heights after load.
    pub fn restore_logic_height_map(&mut self, width: i32, height: i32, data: &[u8]) {
        self.restore_logic_height_map_with_effects(width, height, data, &mut || {
            if let Some(visual) = crate::helpers::TheTerrainVisual::get() {
                visual.static_lighting_changed();
            }
        });
    }
    /// Owned snapshot admission; zero-fill/prefix-copy semantics stay unchanged.
    pub fn restore_logic_height_map_with_effects(
        &mut self,
        width: i32,
        height: i32,
        data: &[u8],
        effect: &mut impl FnMut(),
    ) {
        if let Err(error) =
            self.try_restore_logic_height_map_with_effects(width, height, data, effect)
        {
            log::error!("terrain height admission: {error}");
        }
    }
    pub fn try_restore_logic_height_map_with_effects(
        &mut self,
        width: i32,
        height: i32,
        data: &[u8],
        effect: &mut impl FnMut(),
    ) -> Result<(), String> {
        self.validate_height_map()?;
        let expected = Self::checked_height_sample_len(width, height, self.border_size)?;
        // Reserve the entire replacement before touching the receiving owner.
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(expected)
            .map_err(|_| "terrain height allocation failed")?;
        replacement.resize(expected, 0);
        let n = expected.min(data.len());
        replacement[..n].copy_from_slice(&data[..n]);
        self.bump_height_revision();
        self.map_dx = width;
        self.map_dy = height;
        self.map_data = replacement;
        if let Some((&min_height, &max_height)) =
            self.map_data.iter().min().zip(self.map_data.iter().max())
        {
            self.map_min_z = min_height as f32 * MAP_HEIGHT_SCALE;
            self.map_max_z = max_height as f32 * MAP_HEIGHT_SCALE;
        }
        self.cliff_state
            .rebuild(&self.map_data, self.map_dx, self.map_dy);

        effect();

        Ok(())
    }

    /// Set active boundary and rebuild partition/shroud/radar like C++ TerrainLogic.
    /// C++ `TerrainLogic::setActiveBoundary` (TerrainLogic.cpp:2545-2615).
    pub fn set_active_boundary(&mut self, new_active_boundary: i32) {
        if new_active_boundary < 0 || new_active_boundary as usize >= self.boundaries.len() {
            return;
        }
        if new_active_boundary == self.active_boundary {
            return;
        }
        let boundary = self.boundaries[new_active_boundary as usize];
        if boundary.x == 0 || boundary.y == 0 {
            return;
        }

        if let Ok(mut shroud) = crate::system::shroud_manager::get_shroud_manager().lock() {
            shroud.process_entire_pending_undo_shroud_reveal_queue();
        }

        let object_ids = crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .map(|logic| logic.get_all_object_ids().to_vec())
            .unwrap_or_default();

        // C++ storeFoggedCells(partitionStore, TRUE) — snapshot FOGGED cells
        // before objects detach. Also keep the existing store_fogged_cells hook.
        let fog_store = crate::system::game_logic::get_game_logic()
            .lock()
            .ok()
            .map(|logic| {
                Self::capture_shroud_status_store(
                    logic.partition_manager(),
                    &self.get_maximum_pathfind_extent(),
                )
            });
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            let pm = logic.partition_manager_mut();
            for player in 0..crate::common::MAX_PLAYER_COUNT {
                pm.store_fogged_cells(player, true);
            }
        }

        self.active_boundary = new_active_boundary;

        // C++ TheGhostObjectManager->releasePartitionData()
        if let Ok(mut ghosts) = THE_GHOST_OBJECT_MANAGER.write() {
            ghosts.release_partition_data();
        }
        if let Ok(mut ghosts) = THE_W3D_GHOST_OBJECT_MANAGER.write() {
            ghosts.release_partition_data();
        }

        for object_id in &object_ids {
            if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(mut guard) = obj.write() {
                    guard.friend_prepare_for_map_boundary_adjust();
                }
            }
        }

        // C++ storeFoggedCells(partitionStore, FALSE) — permanently revealed.
        // The typed store already captured both buckets; this second call
        // keeps the existing PartitionManager snapshot API in C++ order.
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            let pm = logic.partition_manager_mut();
            for player in 0..crate::common::MAX_PLAYER_COUNT {
                pm.store_fogged_cells(player, false);
            }
            // C++ ThePartitionManager->reset(); ThePartitionManager->init();
            pm.clear();
        }

        if let Some(radar) = crate::helpers::TheRadar::get() {
            radar.refresh_terrain();
        }

        // Restore permanently revealed cells (C++ restoreFoggedCells(..., FALSE)).
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            let pm = logic.partition_manager_mut();
            if let Some(store) = &fog_store {
                for &(player, x, y) in &store.revealed {
                    pm.add_looker(player, x, y);
                }
            }
            for player in 0..crate::common::MAX_PLAYER_COUNT {
                pm.restore_fogged_cells(player, false);
            }
        }

        // C++ TheGhostObjectManager->lockGhostObjects(TRUE)
        if let Ok(mut ghosts) = THE_GHOST_OBJECT_MANAGER.write() {
            ghosts.lock_ghost_objects(true);
        }
        if let Ok(mut ghosts) = THE_W3D_GHOST_OBJECT_MANAGER.write() {
            ghosts.set_lock_ghost_objects(true);
        }

        for object_id in &object_ids {
            if let Some(obj) = crate::helpers::TheGameLogic::find_object_by_id(*object_id) {
                if let Ok(mut guard) = obj.write() {
                    guard.friend_notify_of_new_map_boundary();
                }
            }
        }

        // Restore fogged cells (C++ restoreFoggedCells(..., TRUE)).
        if let Ok(mut logic) = crate::system::game_logic::get_game_logic().lock() {
            let pm = logic.partition_manager_mut();
            if let Some(store) = &fog_store {
                for &(player, x, y) in &store.fogged {
                    pm.add_looker(player, x, y);
                    pm.remove_looker(player, x, y);
                }
            }
            for player in 0..crate::common::MAX_PLAYER_COUNT {
                pm.restore_fogged_cells(player, true);
            }
        }

        // C++ restorePartitionData + lockGhostObjects(FALSE)
        if let Ok(mut ghosts) = THE_GHOST_OBJECT_MANAGER.write() {
            ghosts.restore_partition_data();
            ghosts.lock_ghost_objects(false);
        }
        if let Ok(mut ghosts) = THE_W3D_GHOST_OBJECT_MANAGER.write() {
            ghosts.restore_partition_data();
            ghosts.set_lock_ghost_objects(false);
        }

        crate::helpers::TheTacticalView::force_camera_constraint_recalc();
    }

    fn capture_shroud_status_store(
        pm: &crate::system::game_logic::PartitionManager,
        extent: &Region3D,
    ) -> BoundaryShroudStore {
        let cell = crate::object::collide::partition_manager::PARTITION_CELL_SIZE;
        let min_x = (extent.lo.x / cell).floor() as i32;
        let min_y = (extent.lo.y / cell).floor() as i32;
        let max_x = (extent.hi.x / cell).ceil() as i32;
        let max_y = (extent.hi.y / cell).ceil() as i32;
        let mut store = BoundaryShroudStore::default();
        for y in min_y..max_y.max(min_y) {
            for x in min_x..max_x.max(min_x) {
                for player in 0..crate::common::MAX_PLAYER_COUNT as i32 {
                    match pm.get_shroud_status_for_player_cell(player, x, y) {
                        game_engine::common::system::radar::CellShroudStatus::Fogged => {
                            store.fogged.push((player, x, y));
                        }
                        game_engine::common::system::radar::CellShroudStatus::Clear => {
                            store.revealed.push((player, x, y));
                        }
                        _ => {}
                    }
                }
            }
        }
        store
    }

    /// Flatten terrain under a building/object.
    /// Reference: C++ TerrainLogic::flattenTerrain() in TerrainLogic.cpp
    ///
    /// Computes the average height under the object's footprint, then lowers
    /// all terrain cells within the footprint to that average. Only lowers,
    /// never raises — matching C++ setRawMapHeight behavior.
    pub fn flatten_terrain(&mut self, obj: &Arc<RwLock<Object>>) {
        if let Err(error) = self.flatten_terrain_with_effects(obj, &mut compatibility_height_change)
        {
            log::error!("standalone terrain flatten: {error}");
        }
    }
    pub fn flatten_terrain_with_effects(
        &mut self,
        obj: &Arc<RwLock<Object>>,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) -> Result<(), String> {
        let obj_guard = obj.read().unwrap();
        if obj_guard.get_geometry_info().get_is_small() {
            return Ok(());
        }

        let pos = obj_guard.get_position();
        let geom = obj_guard.get_geometry_info();

        match geom.get_geometry_type() {
            EngineGeometryType::Box => {
                self.flatten_terrain_box_at_with_effects(
                    pos.x,
                    pos.y,
                    obj_guard.get_orientation(),
                    geom.get_major_radius(),
                    geom.get_minor_radius(),
                    effect,
                )?;
            }
            EngineGeometryType::Sphere | EngineGeometryType::Cylinder => {
                self.flatten_terrain_at_with_effects(
                    pos.x,
                    pos.y,
                    geom.get_major_radius(),
                    effect,
                )?;
            }
        }

        Ok(())
    }

    /// C++ `TerrainLogic::flattenTerrain` GEOMETRY_BOX (TerrainLogic.cpp:2620-2746).
    /// `x`/`y` are C++ ground-plane coords (live host XZ → leftover XY).
    pub fn flatten_terrain_box_at(
        &mut self,
        x: f32,
        y: f32,
        angle: f32,
        halfsize_x: f32,
        halfsize_y: f32,
    ) {
        if let Err(error) = self.flatten_terrain_box_at_with_effects(
            x,
            y,
            angle,
            halfsize_x,
            halfsize_y,
            &mut compatibility_height_change,
        ) {
            log::error!("standalone terrain flatten: {error}");
        }
    }
    pub fn flatten_terrain_box_at_with_effects(
        &mut self,
        x: f32,
        y: f32,
        angle: f32,
        halfsize_x: f32,
        halfsize_y: f32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) -> Result<(), String> {
        if halfsize_x <= 0.0 && halfsize_y <= 0.0 {
            return Ok(());
        }
        let c = angle.cos();
        let s = angle.sin();

        let top_left_x = x - halfsize_x * c - halfsize_y * s;
        let top_left_y = y + halfsize_y * c - halfsize_x * s;
        let top_right_x = x + halfsize_x * c - halfsize_y * s;
        let top_right_y = y + halfsize_y * c + halfsize_x * s;
        let bottom_right_x = x + halfsize_x * c + halfsize_y * s;
        let bottom_right_y = y - halfsize_y * c + halfsize_x * s;
        let bottom_left_x = x - halfsize_x * c + halfsize_y * s;
        let bottom_left_y = y - halfsize_y * c - halfsize_x * s;

        let min_x = top_left_x
            .min(top_right_x)
            .min(bottom_right_x)
            .min(bottom_left_x);
        let max_x = top_left_x
            .max(top_right_x)
            .max(bottom_right_x)
            .max(bottom_left_x);
        let min_y = top_left_y
            .min(top_right_y)
            .min(bottom_right_y)
            .min(bottom_left_y);
        let max_y = top_left_y
            .max(top_right_y)
            .max(bottom_right_y)
            .max(bottom_left_y);

        let i_min_x = super::terrain_ray::source_floor((min_x / MAP_XY_FACTOR))?;
        let _i_min_y = super::terrain_ray::source_floor((min_y / MAP_XY_FACTOR))?;
        let i_max_x = super::terrain_ray::source_floor((max_x / MAP_XY_FACTOR))?;
        let i_max_y = super::terrain_ray::source_floor((max_y / MAP_XY_FACTOR))?;

        let mut total_height: f32 = 0.0;
        let mut num_samples: i32 = 0;
        for i in i_min_x..=i_max_x {
            // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let match_tri = Self::point_in_triangle_2d(
                    top_left_x,
                    top_left_y,
                    top_right_x,
                    top_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                ) || Self::point_in_triangle_2d(
                    top_right_x,
                    top_right_y,
                    bottom_right_x,
                    bottom_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                );
                if match_tri {
                    total_height += self.get_ground_height(test_pt_x, test_pt_y, None);
                    num_samples += 1;
                }
            }
        }
        if num_samples == 0 {
            return Ok(());
        }
        let avg_height = total_height / num_samples as f32;
        let mut raw_data_height =
            super::terrain_ray::source_floor((0.5 + avg_height / MAP_HEIGHT_SCALE))?;

        let center_height = super::terrain_ray::source_floor(
            (self.get_ground_height(x, y, None) / MAP_HEIGHT_SCALE),
        )?;
        if raw_data_height > center_height {
            raw_data_height = center_height;
        }

        for i in i_min_x..=i_max_x {
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let match_tri = Self::point_in_triangle_2d(
                    top_left_x,
                    top_left_y,
                    top_right_x,
                    top_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                ) || Self::point_in_triangle_2d(
                    top_right_x,
                    top_right_y,
                    bottom_right_x,
                    bottom_right_y,
                    bottom_left_x,
                    bottom_left_y,
                    test_pt_x,
                    test_pt_y,
                );
                if match_tri {
                    self.lower_footprint_cell_with_effects(i, j, raw_data_height, effect);
                }
            }
        }

        Ok(())
    }

    /// Flatten a circular footprint at world XY (C++ ground plane).
    /// Used by host dozer placement when the crate Object is not in the registry.
    /// Matches TerrainLogic::flattenTerrain cylinder path (TerrainLogic.cpp:2620).
    pub fn flatten_terrain_at(&mut self, x: f32, y: f32, radius: f32) {
        if let Err(error) =
            self.flatten_terrain_at_with_effects(x, y, radius, &mut compatibility_height_change)
        {
            log::error!("standalone terrain flatten: {error}");
        }
    }
    pub fn flatten_terrain_at_with_effects(
        &mut self,
        x: f32,
        y: f32,
        radius: f32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) -> Result<(), String> {
        if radius <= 0.0 {
            return Ok(());
        }
        let radius_sqr = radius * radius;
        let i_min_x = super::terrain_ray::source_floor(((x - radius) / MAP_XY_FACTOR))?;
        let i_max_x = super::terrain_ray::source_floor(((x + radius) / MAP_XY_FACTOR))?;
        let i_max_y = super::terrain_ray::source_floor(((y + radius) / MAP_XY_FACTOR))?;
        let mut total_height: f32 = 0.0;
        let mut num_samples: i32 = 0;
        for i in i_min_x..=i_max_x {
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let dx = test_pt_x - x;
                let dy = test_pt_y - y;
                if dx * dx + dy * dy < radius_sqr {
                    total_height += self.get_ground_height(test_pt_x, test_pt_y, None);
                    num_samples += 1;
                }
            }
        }
        if num_samples == 0 {
            return Ok(());
        }
        let avg_height = total_height / num_samples as f32;
        let raw_data_height =
            super::terrain_ray::source_floor((0.5 + avg_height / MAP_HEIGHT_SCALE))?;
        for i in i_min_x..=i_max_x {
            for j in 0..=i_max_y {
                let test_pt_x = i as f32 * MAP_XY_FACTOR;
                let test_pt_y = j as f32 * MAP_XY_FACTOR;
                let dx = test_pt_x - x;
                let dy = test_pt_y - y;
                if dx * dx + dy * dy < radius_sqr {
                    self.lower_footprint_cell_with_effects(i, j, raw_data_height, effect);
                }
            }
        }

        Ok(())
    }

    /// C++ `MAX(1, getRawMapHeight(&gridPos) - displacementAmount)`:
    /// Int promotes to Real, subtract, then truncate toward zero to Int.
    pub(super) fn crater_raw_target_height(current_height: i32, displacement_amount: f32) -> i32 {
        1i32.max((current_height as f32 - displacement_amount) as i32)
    }

    /// Dig a deep circular gorge into the terrain beneath an object.
    /// Reference: C++ TerrainLogic::createCraterInTerrain() in TerrainLogic.cpp
    ///
    /// Creates a crater with radial displacement — deepest at center,
    /// tapering to zero at the edge of the object's radius.
    pub fn create_crater_in_terrain(&mut self, obj: &Arc<RwLock<Object>>) {
        let obj_guard = obj.read().unwrap();
        if obj_guard.get_geometry_info().get_is_small() {
            return;
        }

        let pos = obj_guard.get_position();
        let radius = obj_guard.get_geometry_info().get_major_radius();
        if radius <= 0.0 {
            return;
        }

        let i_min_x = ((pos.x - radius) / MAP_XY_FACTOR).floor() as i32;
        let _i_min_y = ((pos.y - radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_x = ((pos.x + radius) / MAP_XY_FACTOR).floor() as i32;
        let i_max_y = ((pos.y + radius) / MAP_XY_FACTOR).floor() as i32;

        for i in i_min_x..=i_max_x {
            // C++ bug: j starts at 0, not iMin.y — we match C++ exactly
            for j in 0..=i_max_y {
                let delta_x = i as f32 * MAP_XY_FACTOR - pos.x;
                let delta_y = j as f32 * MAP_XY_FACTOR - pos.y;
                let distance = (delta_x * delta_x + delta_y * delta_y).sqrt();

                if distance < radius {
                    let displacement_amount = radius * (1.0 - distance / radius);
                    let current_height = self.get_raw_map_height(i, j);
                    let target_height =
                        Self::crater_raw_target_height(current_height, displacement_amount);
                    self.set_raw_map_height(i, j, target_height);
                }
            }
        }
    }

    /// Set raw map height at grid position — only lowers, never raises.
    /// Reference: C++ W3DTerrainVisual::setRawMapHeight() in W3DTerrainVisual.cpp:909-937
    ///
    /// C++ adds getBorderSizeInline() to the incoming playable-grid coords
    /// before indexing the full (playable + 2*border) height buffer.
    pub(super) fn set_raw_map_height(&mut self, x: i32, y: i32, height: i32) {
        self.set_raw_map_height_with_effects(x, y, height, &mut compatibility_height_change);
    }
    /// Apply one strict lower-write and notify only after canonical bytes and
    /// cliff bits have changed. The sink belongs to the driving world.
    pub fn set_raw_map_height_with_effects(
        &mut self,
        x: i32,
        y: i32,
        height: i32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) {
        let Some(idx) = self.raw_map_height_index(x, y) else {
            return;
        };
        // Original compares Int before narrowing its accepted value to u8.
        if self.map_data[idx] as i32 > height {
            let accepted = height as u8;
            let changed = self.map_data[idx] != accepted;
            self.map_data[idx] = accepted;
            if changed {
                self.bump_height_revision();
            }
            let grid_x = (idx % self.map_dx as usize) as i32;
            let grid_y = (idx / self.map_dx as usize) as i32;
            self.cliff_state.refresh_vertex(
                &self.map_data,
                self.map_dx,
                self.map_dy,
                grid_x,
                grid_y,
            );
            effect(TerrainHeightChange {
                playable_x: x,
                playable_y: y,
                raw_height: accepted,
                requested_height: height,
            });
        }
    }

    // WorldHeightMap::getHeight/setRawHeight validate the flattened index,
    // not X and Y independently; edge aliases are intentional source behavior.
    fn raw_map_height_index(&self, x: i32, y: i32) -> Option<usize> {
        if self.map_dx <= 0 {
            return None;
        }
        let x = i64::from(x) + i64::from(self.border_size);
        let y = i64::from(y) + i64::from(self.border_size);
        let index = y.checked_mul(i64::from(self.map_dx))?.checked_add(x)?;
        usize::try_from(index)
            .ok()
            .filter(|&index| index < self.map_data.len())
    }

    fn lower_footprint_cell_with_effects(
        &mut self,
        x: i32,
        y: i32,
        height: i32,
        effect: &mut impl FnMut(TerrainHeightChange),
    ) {
        for (dx, dy) in [
            (0, 0),
            (-1, 0),
            (1, 0),
            (0, -1),
            (0, 1),
            (-1, -1),
            (1, 1),
            (1, -1),
            (-1, 1),
        ] {
            if let Some((x, y)) = x.checked_add(dx).zip(y.checked_add(dy)) {
                self.set_raw_map_height_with_effects(x, y, height, effect);
            }
        }
    }

    /// Get raw map height at grid position.
    /// Reference: C++ W3DTerrainVisual::getRawMapHeight() in W3DTerrainVisual.cpp:941-951
    pub(super) fn get_raw_map_height(&self, x: i32, y: i32) -> i32 {
        self.raw_map_height_index(x, y)
            .map_or(0, |index| self.map_data[index] as i32)
    }

    /// 2D point-in-triangle test using cross products.
    /// Reference: C++ Point_In_Triangle_2D
    fn point_in_triangle_2d(
        v0x: f32,
        v0y: f32,
        v1x: f32,
        v1y: f32,
        v2x: f32,
        v2y: f32,
        px: f32,
        py: f32,
    ) -> bool {
        let d1 = (px - v1x) * (v0y - v1y) - (v0x - v1x) * (py - v1y);
        let d2 = (px - v2x) * (v1y - v2y) - (v1x - v2x) * (py - v2y);
        let d3 = (px - v0x) * (v2y - v0y) - (v2x - v0x) * (py - v0y);

        let has_neg = (d1 < 0.0) || (d2 < 0.0) || (d3 < 0.0);
        let has_pos = (d1 > 0.0) || (d2 > 0.0) || (d3 > 0.0);

        !(has_neg && has_pos)
    }
}

// Standalone Core compatibility only. Main passes its receiving effect sink.
fn compatibility_height_change(change: TerrainHeightChange) {
    if let Some(visual) = crate::helpers::TheTerrainVisual::get() {
        visual.set_raw_map_height(
            change.playable_x,
            change.playable_y,
            change.requested_height,
        );
    }
}
