//! TerrainLogic water behavior.

use super::*;
use generals_sim::terrain::{WaterSurface, is_grid_water_name, select_water_surface};

/// Effects of one completed in-place water-height mutation.
#[derive(Debug, Clone, Copy)]
pub struct WaterHeightChange {
    pub affected_region: Region3D,
    pub reclassify: bool,
    pub damage_amount: f32,
}

impl TerrainLogic {
    /// Check if point is underwater
    pub fn is_underwater(
        &self,
        x: f32,
        y: f32,
        water_z: Option<&mut f32>,
        terrain_z: Option<&mut f32>,
    ) -> bool {
        let Some(w_z) = self.water_height_at_location(x, y) else {
            // TerrainLogic.cpp:2128–2133 still reports an explicitly requested
            // terrain height on dry ground. Predicate-only callers can stop here.
            if let Some(tz) = terrain_z {
                *tz = self.get_ground_height(x, y, None);
            }
            return false;
        };

        if let Some(wz) = water_z {
            *wz = w_z;
        }

        let terrain_height = self.get_ground_height(x, y, None);
        if let Some(tz) = terrain_z {
            *tz = terrain_height;
        }
        terrain_height < w_z
    }

    /// Check if cell is cliff
    pub fn is_cliff_cell(&self, x: f32, y: f32) -> bool {
        self.height_map.is_cliff_cell(x, y)
    }

    /// Sample the authoritative logical water height in one search.
    pub fn water_height_at_location(&self, x: f32, y: f32) -> Option<f32> {
        match self.water_surface_at(x, y, || self.water_grid.height_at(x, y))? {
            WaterSurface::Grid(height) => Some(height),
            WaterSurface::Polygon { trigger_id, .. } => self
                .water_handles_by_trigger_id
                .get(&trigger_id)
                .map(|handle| self.get_water_height(handle)),
        }
    }

    /// Get water handle at location
    pub fn get_water_handle(&self, x: f32, y: f32) -> Option<&WaterHandle> {
        match self.water_surface_at(x, y, || self.water_grid.height_at(x, y))? {
            WaterSurface::Grid(_) => Some(&self.grid_water_handle),
            WaterSurface::Polygon { trigger_id, .. } => {
                self.water_handles_by_trigger_id.get(&trigger_id)
            }
        }
    }

    /// The W3D wake uses the receiving polygon table and the rendered water grid.
    /// It needs the selected surface height, without a ground height lookup.
    pub fn water_surface_height_with_visual_grid(
        &self,
        x: f32,
        y: f32,
        visual_grid_height: impl FnOnce() -> Option<f32>,
    ) -> Option<f32> {
        let height = match self.water_surface_at(x, y, visual_grid_height)? {
            WaterSurface::Grid(height) | WaterSurface::Polygon { height, .. } => height,
        };
        Some(height)
    }

    fn water_surface_at(
        &self,
        x: f32,
        y: f32,
        grid_height: impl FnOnce() -> Option<f32>,
    ) -> Option<WaterSurface> {
        let query = ICoord3D::new((x + 0.5).floor() as Int, (y + 0.5).floor() as Int, 0);
        let polygons = self.trigger_areas.get_triggers().iter().filter_map(|trigger| {
            if trigger.is_water_area() && trigger.point_in_trigger_int(&query) {
                let point = trigger.get_point(0)?;
                Some((trigger.get_id(), point.z as f32))
            } else {
                None
            }
        });
        select_water_surface(polygons, grid_height)
    }

    /// Get water handle by name
    pub fn get_water_handle_by_name(&self, name: &AsciiString) -> Option<&WaterHandle> {
        if is_grid_water_name(name.as_str()) {
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
        if is_grid_water_name(water_name.as_str()) {
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

    fn apply_water_rise_damage(&self, affected_region: &Region3D, damage_amount: f32) {
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
        let ai_store = the_ai();
        let pathfinder = ai_store.read().ok().and_then(|ai_guard| ai_guard.pathfinder());
        self.rebuild_pathfinder(pathfinder.as_ref());
    }

    fn rebuild_pathfinder(
        &self,
        pathfinder: Option<&Arc<RwLock<crate::ai::Pathfinder>>>,
    ) {
        let Some(pathfinder) = pathfinder else {
            return;
        };
        let mut pathfinder_guard = match pathfinder.write() {
            Ok(guard) => guard,
            Err(_) => return,
        };
        pathfinder_guard.rebuild_from_terrain(self);
    }

    /// Mutate only this terrain. The driving world classifies its own pathfinder
    /// before admitting damage (TerrainLogic.cpp:2331-2381).
    fn mutate_water_height(
        &mut self,
        trigger_id: Int,
        water_name: &AsciiString,
        height: f32,
        damage_amount: f32,
        force_pathfind_update: bool,
    ) -> Option<WaterHeightChange> {
        let previous = self.resolve_water_height_for_entry(trigger_id, water_name)?;
        let (height, affected_region) = if is_grid_water_name(water_name.as_str()) {
            let previous = self.water_grid.transform.w_axis.z;
            self.water_grid.set_height(height);
            self.grid_water_handle.set_height(height);
            let (lo, hi) = self.water_grid.bounds();
            return Some(WaterHeightChange {
                affected_region: Region3D::new(lo, hi),
                reclassify: force_pathfind_update || previous != height,
                damage_amount: if damage_amount > 0.0 && height > previous {
                    damage_amount
                } else {
                    0.0
                },
            });
        } else {
            // PolygonTrigger stores integer Z; compare and cache the actual
            // height, not the requested fractional accumulator.
            let actual = (height as Int) as f32;
            let (id, name, bounds) = if trigger_id >= 0 {
                let (name, bounds) = self.update_polygon_water_height_by_id(trigger_id, actual)?;
                (trigger_id, name, bounds)
            } else {
                self.update_polygon_water_height_by_name(water_name, actual)?
            };
            self.sync_water_handle_for_trigger(id, &name, actual, bounds);
            (actual, bounds)
        };
        Some(WaterHeightChange {
            affected_region,
            reclassify: force_pathfind_update || previous != height,
            damage_amount: if damage_amount > 0.0 && height > previous {
                damage_amount
            } else {
                0.0
            },
        })
    }

    pub fn mutate_named_water_height(
        &mut self,
        name: &AsciiString,
        height: f32,
        damage: f32,
        force: bool,
    ) -> Option<WaterHeightChange> {
        self.mutate_water_height(
            self.resolve_water_trigger_id(name),
            name,
            height,
            damage,
            force,
        )
    }

    fn apply_standalone_water_change(&self, change: WaterHeightChange) {
        self.apply_standalone_water_change_with_owner(change, None, true);
    }

    fn apply_standalone_water_change_with_owner(
        &self,
        change: WaterHeightChange,
        owner_pathfinder: Option<&Arc<RwLock<crate::ai::Pathfinder>>>,
        allow_legacy_owner: bool,
    ) {
        if change.reclassify {
            if allow_legacy_owner {
                self.request_pathfind_recalculation();
            } else {
                self.rebuild_pathfinder(owner_pathfinder);
            }
        }
        self.apply_water_rise_damage(&change.affected_region, change.damage_amount);
        crate::terrain_water::publish_water_grid(&self.water_grid);
    }

    pub fn water_grid_state(&self) -> &generals_sim::terrain::WaterGridState {
        &self.water_grid
    }
    pub fn water_grid_state_mut(&mut self) -> &mut generals_sim::terrain::WaterGridState {
        &mut self.water_grid
    }
    pub fn restore_water_grid_state(&mut self, state: generals_sim::terrain::WaterGridState) {
        self.water_grid_enabled = state.enabled;
        self.grid_water_handle.set_height(state.transform.w_axis.z);
        self.water_grid = state;
    }

    fn resolve_named_water_handle_identity(
        &self,
        water_name: &AsciiString,
    ) -> Option<(Int, &WaterHandle)> {
        if is_grid_water_name(water_name.as_str()) {
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
        if let Some(change) =
            self.mutate_named_water_height(water_name, height, damage_amount, force_pathfind_update)
        {
            self.apply_standalone_water_change(change);
        }
    }

    /// Schedule the original float accumulator; even a zero duration completes
    /// on the next terrain phase, never inside the requesting script action.
    pub fn change_water_height_over_time(
        &mut self,
        water_name: &AsciiString,
        final_height: f32,
        transition_time_seconds: f32,
        damage_amount: f32,
    ) {
        // C++ checks the fixed capacity before resolving a handle or removing duplicates.
        if self.water_to_update.is_full() {
            return;
        }
        let Some((trigger_id, handle)) = self.resolve_named_water_handle_identity(water_name)
        else {
            return;
        };
        let name = handle.get_name().clone();
        let current_height = if is_grid_water_name(name.as_str()) {
            self.water_grid.transform.w_axis.z
        } else {
            handle.get_current_height()
        };
        self.water_to_update.schedule(
            trigger_id,
            name,
            current_height,
            final_height,
            transition_time_seconds,
            damage_amount,
        );
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
        self.water_to_update
            .entries()
            .iter()
            .map(|entry| {
                let mut entry = entry.clone();
                if entry.trigger_id < 0 {
                    entry.trigger_id = self.resolve_water_trigger_id(&entry.water_name);
                }
                entry
            })
            .collect()
    }

    pub fn restore_dynamic_water_entries(
        &mut self,
        entries: Vec<TerrainDynamicWaterSnapshotEntry>,
    ) -> Result<(), String> {
        let candidate = WaterTransitions::from_snapshot(entries, |entry| {
            self.validate_restored_water_transition(entry)
        })?;
        self.water_to_update = candidate;
        Ok(())
    }

    fn validate_restored_water_transition(
        &self,
        entry: &mut TerrainDynamicWaterSnapshotEntry,
    ) -> Result<(), String> {
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

        let is_grid_name = is_grid_water_name(entry.water_name.as_str())
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
        Ok(())
    }

    pub fn dynamic_water_count(&self) -> usize {
        self.water_to_update.entries().len()
    }

    /// One bounded original update phase. Terminal removal happens only after
    /// the driving world has classified and delivered this change's damage.
    pub fn advance_dynamic_water(
        &mut self,
        index: usize,
        frame: u32,
    ) -> Option<(WaterHeightChange, bool)> {
        let step = self.water_to_update.advance(index, frame)?;
        self.mutate_water_height(
            step.trigger_id,
            &step.name,
            step.height,
            step.damage,
            step.terminal,
        )
        .map(|change| (change, step.terminal))
    }

    pub fn finish_dynamic_water(&mut self, index: usize, terminal: bool) {
        self.water_to_update.finish(index, terminal);
    }

    pub(super) fn update_dynamic_water(&mut self) {
        let frame = crate::helpers::TheGameLogic::get_frame();
        self.update_dynamic_water_with_owner_and_legacy(None, true, frame);
    }

    pub(super) fn update_dynamic_water_with_owner(
        &mut self,
        owner_pathfinder: Option<&Arc<RwLock<crate::ai::Pathfinder>>>,
        frame: u32,
    ) {
        self.update_dynamic_water_with_owner_and_legacy(owner_pathfinder, false, frame);
    }

    fn update_dynamic_water_with_owner_and_legacy(
        &mut self,
        owner_pathfinder: Option<&Arc<RwLock<crate::ai::Pathfinder>>>,
        allow_legacy_owner: bool,
        frame: u32,
    ) {
        for index in (0..self.dynamic_water_count()).rev() {
            if let Some((change, terminal)) = self.advance_dynamic_water(index, frame) {
                self.apply_standalone_water_change_with_owner(
                    change,
                    owner_pathfinder,
                    allow_legacy_owner,
                );
                self.finish_dynamic_water(index, terminal);
            }
        }
    }
}
