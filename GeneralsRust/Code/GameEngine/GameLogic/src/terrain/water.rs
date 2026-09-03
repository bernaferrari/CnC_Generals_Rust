//! TerrainLogic water behavior.

use super::*;

impl TerrainLogic {
    /// Check if point is underwater
    pub fn is_underwater(
        &self,
        x: f32,
        y: f32,
        water_z: Option<&mut f32>,
        terrain_z: Option<&mut f32>,
    ) -> bool {
        let terrain_height = self.get_ground_height(x, y, None);

        if let Some(tz) = terrain_z {
            *tz = terrain_height;
        }

        let Some(water_handle) = self.get_water_handle(x, y) else {
            return false;
        };

        let is_grid = std::ptr::eq(water_handle, &self.grid_water_handle);
        let w_z = if is_grid {
            crate::terrain_water::get_water_grid_height(x, y).unwrap_or(0.0)
        } else {
            self.get_water_height(water_handle)
        };

        if let Some(wz) = water_z {
            *wz = w_z;
        }

        terrain_height < w_z
    }

    /// Check if cell is cliff
    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        crate::terrain_cliff::is_cliff_cell(
            x,
            y,
            &self.cliff_state,
            self.map_dx,
            self.map_dy,
            self.border_size,
        )
    }

    /// Get water handle at location
    pub fn get_water_handle(&self, x: f32, y: f32) -> Option<&WaterHandle> {
        let query = ICoord3D::new((x + 0.5).floor() as Int, (y + 0.5).floor() as Int, 0);

        let mut best_trigger_id: Option<Int> = None;
        let mut best_water_z = 0.0f32;

        for trigger in self.trigger_areas.get_triggers() {
            if !trigger.is_water_area() || !trigger.point_in_trigger_int(&query) {
                continue;
            }

            let Some(point0) = trigger.get_point(0) else {
                continue;
            };
            let trigger_water_z = point0.z as f32;
            if trigger_water_z >= best_water_z {
                best_water_z = trigger_water_z;
                best_trigger_id = Some(trigger.get_id());
            }
        }

        // C++ `TheTerrainVisual->getWaterGridHeight`: on-mesh only, not AABB.
        if let Some(mesh_z) = crate::terrain_water::get_water_grid_height(x, y) {
            if mesh_z >= best_water_z {
                return Some(&self.grid_water_handle);
            }
        }

        if let Some(trigger_id) = best_trigger_id {
            return self.water_handles_by_trigger_id.get(&trigger_id);
        }
        None
    }

    /// Get water handle by name
    pub fn get_water_handle_by_name(&self, name: &AsciiString) -> Option<&WaterHandle> {
        if Self::is_grid_water_name(name) {
            return Some(&self.grid_water_handle);
        }

        let trigger_id = self.resolve_water_trigger_id(name);
        if trigger_id >= 0 {
            return self.water_handles_by_trigger_id.get(&trigger_id);
        }

        None
    }

    /// Get water height
    pub fn get_water_height(&self, water: &WaterHandle) -> f32 {
        water.get_current_height()
    }

    fn is_grid_water_name(name: &AsciiString) -> bool {
        name.as_str().eq_ignore_ascii_case(WATER_GRID_NAME_CPP)
            || name.as_str().eq_ignore_ascii_case(WATER_GRID_NAME_LEGACY)
    }

    fn water_bounds_from_trigger(trigger: &PolygonTrigger, height: f32) -> Region3D {
        let bounds = trigger.get_bounds();
        Region3D::new(
            Coord3D::new(bounds.lo.x as f32, bounds.lo.y as f32, height),
            Coord3D::new(bounds.hi.x as f32, bounds.hi.y as f32, height),
        )
    }

    fn resolve_water_height_for_entry(
        &self,
        trigger_id: Int,
        water_name: &AsciiString,
    ) -> Option<f32> {
        if Self::is_grid_water_name(water_name) {
            return Some(self.grid_water_handle.get_current_height());
        }

        if trigger_id >= 0 {
            if let Some(handle) = self.water_handles_by_trigger_id.get(&trigger_id) {
                return Some(handle.get_current_height());
            }
            if let Some(trigger) = self.trigger_areas.get_by_id(trigger_id) {
                if trigger.is_water_area() {
                    if let Some(point) = trigger.get_point(0) {
                        return Some(point.z as f32);
                    }
                }
            }
        }

        None
    }

    fn update_polygon_water_height_by_id(
        &mut self,
        trigger_id: Int,
        height: f32,
    ) -> Option<(AsciiString, Region3D)> {
        let trigger = self.trigger_areas.get_by_id_mut(trigger_id)?;
        if !trigger.is_water_area() {
            return None;
        }

        let point_count = trigger.get_num_points();
        for idx in 0..point_count {
            if let Some(mut point) = trigger.get_point(idx).cloned() {
                point.z = height as Int;
                trigger.set_point(point, idx);
            }
        }

        let trigger_name = trigger.get_trigger_name().clone();
        let bounds = Self::water_bounds_from_trigger(trigger, height);
        Some((trigger_name, bounds))
    }

    fn update_polygon_water_height_by_name(
        &mut self,
        water_name: &AsciiString,
        height: f32,
    ) -> Option<(Int, AsciiString, Region3D)> {
        let trigger = self.trigger_areas.get_by_name_mut(water_name.as_str())?;
        if !trigger.is_water_area() {
            return None;
        }
        let trigger_id = trigger.get_id();

        let point_count = trigger.get_num_points();
        for idx in 0..point_count {
            if let Some(mut point) = trigger.get_point(idx).cloned() {
                point.z = height as Int;
                trigger.set_point(point, idx);
            }
        }

        let trigger_name = trigger.get_trigger_name().clone();
        let bounds = Self::water_bounds_from_trigger(trigger, height);
        Some((trigger_id, trigger_name, bounds))
    }

    fn sync_water_handle_for_trigger(
        &mut self,
        trigger_id: Int,
        trigger_name: &AsciiString,
        height: f32,
        bounds: Region3D,
    ) {
        if let Some(handle) = self.water_handles_by_trigger_id.get_mut(&trigger_id) {
            handle.name = trigger_name.clone();
            handle.set_height(height);
            handle.bounds = bounds;
        } else {
            self.water_handles_by_trigger_id.insert(
                trigger_id,
                WaterHandle::new(trigger_name.clone(), height, bounds),
            );
        }

        // Keep the name-keyed cache aligned with C++ first-match lookup behavior.
        if self.resolve_water_trigger_id(trigger_name) == trigger_id {
            if let Some(name_handle) = self.water_handles.get_mut(trigger_name) {
                name_handle.set_height(height);
                name_handle.bounds = bounds;
            } else if let Some(trigger_handle) =
                self.water_handles_by_trigger_id.get(&trigger_id).cloned()
            {
                self.water_handles
                    .insert(trigger_name.clone(), trigger_handle);
            }
        }
    }

    fn sync_named_water_handle(
        &mut self,
        water_name: &AsciiString,
        height: f32,
        bounds: Option<Region3D>,
    ) {
        if let Some(handle) = self.water_handles.get_mut(water_name) {
            handle.set_height(height);
            if let Some(region) = bounds {
                handle.bounds = region;
            }
        } else if let Some(region) = bounds {
            self.water_handles.insert(
                water_name.clone(),
                WaterHandle::new(water_name.clone(), height, region),
            );
        }
    }

    fn apply_water_rise_damage(&self, affected_region: &Region3D, damage_amount: f32) {
        // Wave 341: empty dual-world → host GameLogic applies DAMAGE_WATER.
        if dual_world_registry_unavailable() {
            queue_host_water_rise_damage(damage_amount);
            return;
        }

        if damage_amount <= 0.0 {
            return;
        }

        let center = Coord3D::new(
            (affected_region.lo.x + affected_region.hi.x) * 0.5,
            (affected_region.lo.y + affected_region.hi.y) * 0.5,
            0.0,
        );
        let width = affected_region.hi.x - affected_region.lo.x;
        let height = affected_region.hi.y - affected_region.lo.y;
        let max_dist = (width * width + height * height).sqrt();

        let Some(partition) = crate::helpers::ThePartitionManager::get() else {
            return;
        };

        for object_id in partition.get_objects_in_range(&center, max_dist) {
            let underwater = OBJECT_REGISTRY.with_object(object_id, |obj_guard| {
                let pos = *obj_guard.get_position();
                self.is_underwater(pos.x, pos.y, None, None)
            });
            if underwater != Some(true) {
                continue;
            }
            let _ = OBJECT_REGISTRY.with_object_mut(object_id, |obj_guard| {
                let mut damage = DamageInfo::with_simple(
                    damage_amount,
                    crate::common::INVALID_ID,
                    DamageType::Water,
                    DeathType::Normal,
                );
                let _ = obj_guard.attempt_damage(&mut damage);
            });
        }
    }

    fn request_pathfind_recalculation(&self) {
        // C++ TerrainLogic.cpp:2331-2338 forceMapRecalculation — live host
        // restamps Water cells even when the crate pathfinder is empty.
        queue_host_pathfind_recalculation();
        let ai_store = the_ai(); let pathfinder = if let Ok(ai_guard) = ai_store.read() {
            ai_guard.pathfinder()
        } else {
            None
        };
        let Some(pathfinder) = pathfinder else {
            return;
        };
        let mut pathfinder_guard = match pathfinder.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        pathfinder_guard.rebuild_from_terrain(self);
    }

    fn set_water_height_internal(
        &mut self,
        trigger_id: Int,
        water_name: &AsciiString,
        height: f32,
        damage_amount: f32,
        force_pathfind_update: bool,
    ) {
        if Self::is_grid_water_name(water_name) {
            let previous_height = crate::terrain_water::get_transform_z();
            crate::terrain_water::set_transform_z(height);
            self.grid_water_handle.set_height(height);

            if damage_amount > 0.0 && height > previous_height {
                let affected = self.grid_water_handle.get_bounds();
                self.apply_water_rise_damage(affected, damage_amount);
            }
            if force_pathfind_update || (previous_height - height).abs() > f32::EPSILON {
                self.request_pathfind_recalculation();
            }
            return;
        }

        let previous_height = self
            .resolve_water_height_for_entry(trigger_id, water_name)
            .unwrap_or(height);

        let mut resolved_name = water_name.clone();
        let mut resolved_trigger_id = trigger_id;
        let mut affected_region = None;
        if trigger_id >= 0 {
            if let Some((name, bounds)) = self.update_polygon_water_height_by_id(trigger_id, height)
            {
                resolved_name = name;
                resolved_trigger_id = trigger_id;
                affected_region = Some(bounds);
            }
        } else if let Some((id, name, bounds)) =
            self.update_polygon_water_height_by_name(water_name, height)
        {
            resolved_trigger_id = id;
            resolved_name = name;
            affected_region = Some(bounds);
        }

        if let Some(bounds) = affected_region {
            if resolved_trigger_id >= 0 {
                self.sync_water_handle_for_trigger(
                    resolved_trigger_id,
                    &resolved_name,
                    height,
                    bounds,
                );
            } else {
                self.sync_named_water_handle(&resolved_name, height, Some(bounds));
            }
        } else {
            if trigger_id >= 0 {
                log::warn!(
                    "TerrainLogic::set_water_height_internal missing water trigger id {}",
                    trigger_id
                );
            }
            self.sync_named_water_handle(&resolved_name, height, None);
        }

        if damage_amount > 0.0 && height > previous_height {
            if let Some(region) = affected_region {
                self.apply_water_rise_damage(&region, damage_amount);
            }
        }

        if force_pathfind_update || (previous_height - height).abs() > f32::EPSILON {
            self.request_pathfind_recalculation();
        }
    }

    fn resolve_named_water_handle_identity(
        &self,
        water_name: &AsciiString,
    ) -> Option<(Int, &WaterHandle)> {
        if Self::is_grid_water_name(water_name) {
            return Some((-1, &self.grid_water_handle));
        }
        let trigger_id = self.resolve_water_trigger_id(water_name);
        if trigger_id >= 0 {
            return self
                .water_handles_by_trigger_id
                .get(&trigger_id)
                .map(|handle| (trigger_id, handle));
        }
        None
    }

    /// Set water height
    pub fn set_water_height(
        &mut self,
        water_name: &AsciiString,
        height: f32,
        damage_amount: f32,
        force_pathfind_update: bool,
    ) {
        self.set_water_height_internal(
            self.resolve_water_trigger_id(water_name),
            water_name,
            height,
            damage_amount,
            force_pathfind_update,
        );
    }

    /// Change water height over time
    pub fn change_water_height_over_time(
        &mut self,
        water_name: &AsciiString,
        final_height: f32,
        transition_time_seconds: f32,
        damage_amount: f32,
    ) {
        let Some((trigger_id, water_handle)) = self.resolve_named_water_handle_identity(water_name)
        else {
            return;
        };
        let resolved_name = water_handle.get_name().clone();
        let current_height = water_handle.get_current_height();

        // C++ parity: remove existing transition for this water handle before adding a new one.
        self.water_to_update.retain(|entry| {
            if trigger_id >= 0 && entry.trigger_id >= 0 {
                entry.trigger_id != trigger_id
            } else {
                !entry
                    .water_name
                    .as_str()
                    .eq_ignore_ascii_case(resolved_name.as_str())
            }
        });

        // C++ parity: fixed-size dynamic water transition list.
        if self.water_to_update.len() >= MAX_DYNAMIC_WATER_ENTRIES {
            log::warn!(
                "TerrainLogic dynamic water transition limit ({}) reached",
                MAX_DYNAMIC_WATER_ENTRIES
            );
            return;
        }

        let frames_to_complete = (transition_time_seconds * LOGICFRAMES_PER_SECOND as f32) as i32;
        if frames_to_complete <= 0 {
            // C++ TerrainLogic::changeWaterHeightOverTime (TerrainLogic.cpp:2439-2448)
            // divides by (LOGICFRAMES_PER_SECOND * seconds). A 0/sub-frame time
            // yields an infinite changePerFrame; the next update snaps via
            // setWaterHeight with damage + pathfind recalc (TerrainLogic.cpp:1057).
            self.set_water_height_internal(
                trigger_id,
                &resolved_name,
                final_height,
                damage_amount,
                true,
            );
            return;
        }

        let change_per_frame = (final_height - current_height) / frames_to_complete as f32;
        self.water_to_update.push(DynamicWaterEntry {
            trigger_id,
            water_name: resolved_name,
            change_per_frame,
            target_height: final_height,
            damage_amount,
            current_height,
        });
    }

    fn resolve_water_trigger_id(&self, water_name: &AsciiString) -> Int {
        for trigger in self.trigger_areas.get_triggers() {
            if trigger.is_water_area() && trigger.get_trigger_name() == water_name {
                return trigger.get_id();
            }
        }
        -1
    }

    pub fn snapshot_dynamic_water_entries(&self) -> Vec<TerrainDynamicWaterSnapshotEntry> {
        let mut entries = Vec::with_capacity(self.water_to_update.len());
        for entry in &self.water_to_update {
            entries.push(TerrainDynamicWaterSnapshotEntry {
                trigger_id: if entry.trigger_id >= 0 {
                    entry.trigger_id
                } else {
                    self.resolve_water_trigger_id(&entry.water_name)
                },
                water_name: entry.water_name.clone(),
                change_per_frame: entry.change_per_frame,
                target_height: entry.target_height,
                damage_amount: entry.damage_amount,
                current_height: entry.current_height,
            });
        }
        entries
    }

    pub fn restore_dynamic_water_entries(
        &mut self,
        entries: Vec<TerrainDynamicWaterSnapshotEntry>,
    ) -> Result<(), String> {
        self.water_to_update.clear();
        for mut entry in entries {
            if self.water_to_update.len() >= MAX_DYNAMIC_WATER_ENTRIES {
                return Err(format!(
                    "TerrainLogic::restore_dynamic_water_entries exceeds max dynamic entries ({})",
                    MAX_DYNAMIC_WATER_ENTRIES
                ));
            }
            if entry.trigger_id >= 0 {
                let trigger = self
                    .trigger_areas
                    .get_by_id(entry.trigger_id)
                    .ok_or_else(|| {
                        format!(
                            "TerrainLogic::restore_dynamic_water_entries missing trigger id '{}'",
                            entry.trigger_id
                        )
                    })?;
                if trigger.get_water_handle().is_none() {
                    return Err(format!(
                        "TerrainLogic::restore_dynamic_water_entries trigger '{}' has no water handle",
                        entry.trigger_id
                    ));
                }
                if !self
                    .water_handles_by_trigger_id
                    .contains_key(&entry.trigger_id)
                {
                    return Err(format!(
                        "TerrainLogic::restore_dynamic_water_entries missing water handle for trigger id '{}'",
                        entry.trigger_id
                    ));
                }
                entry.water_name = trigger.get_trigger_name().clone();
            }

            if entry.water_name.is_empty() {
                return Err(
                    "TerrainLogic::restore_dynamic_water_entries missing water handle name"
                        .to_string(),
                );
            }

            let is_grid_name = entry
                .water_name
                .as_str()
                .eq_ignore_ascii_case(WATER_GRID_NAME_CPP)
                || entry
                    .water_name
                    .as_str()
                    .eq_ignore_ascii_case(WATER_GRID_NAME_LEGACY)
                || entry.water_name == *self.grid_water_handle.get_name();

            if !is_grid_name
                && entry.trigger_id < 0
                && self.get_water_handle_by_name(&entry.water_name).is_none()
            {
                return Err(format!(
                    "TerrainLogic::restore_dynamic_water_entries missing water handle '{}'",
                    entry.water_name
                ));
            }

            self.water_to_update.push(DynamicWaterEntry {
                trigger_id: entry.trigger_id,
                water_name: entry.water_name,
                change_per_frame: entry.change_per_frame,
                target_height: entry.target_height,
                damage_amount: entry.damage_amount,
                current_height: entry.current_height,
            });
        }
        Ok(())
    }

    /// Enable/disable water grid
    pub fn enable_water_grid(&mut self, enable: bool) {
        self.water_grid_enabled = enable;
        let _ = crate::terrain_water::enable_water_grid(enable);
    }

    /// Update dynamic water tables
    pub(super) fn update_dynamic_water(&mut self) {
        let do_damage_this_frame =
            crate::helpers::TheGameLogic::get_frame() % LOGICFRAMES_PER_SECOND == 0;
        let mut retained = Vec::with_capacity(self.water_to_update.len());
        let mut entries = std::mem::take(&mut self.water_to_update);
        for mut entry in entries.drain(..) {
            entry.current_height += entry.change_per_frame;

            let reached_target = if entry.change_per_frame > 0.0 {
                entry.current_height >= entry.target_height
            } else {
                entry.current_height <= entry.target_height
            };

            if reached_target {
                entry.current_height = entry.target_height;
                self.set_water_height_internal(
                    entry.trigger_id,
                    &entry.water_name,
                    entry.current_height,
                    entry.damage_amount,
                    true,
                );
            } else {
                let per_frame_damage = if do_damage_this_frame {
                    entry.damage_amount
                } else {
                    0.0
                };
                self.set_water_height_internal(
                    entry.trigger_id,
                    &entry.water_name,
                    entry.current_height,
                    per_frame_damage,
                    false,
                );
                retained.push(entry);
            }
        }
        self.water_to_update = retained;
    }
}
