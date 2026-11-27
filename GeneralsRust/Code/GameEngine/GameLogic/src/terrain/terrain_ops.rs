//! TerrainLogic terrain ops behavior.

use super::*;

impl TerrainLogic {
    /// Every admitted map is either the canonical unloaded tuple or a full grid.
    pub fn validate_height_map(&self) -> Result<(), String> {
        self.height_map.validate()
    }

    pub fn height_revision(&self) -> u64 {
        self.height_map.revision()
    }
    pub fn logic_height_map_border_size(&self) -> i32 {
        self.height_map.border_size()
    }

    /// Get active boundary
    pub fn get_active_boundary(&self) -> i32 {
        self.active_boundary
    }

    /// C++ `WorldHeightMap` extent used by `W3DTerrainVisual::xfer` v>=2.
    pub fn logic_height_map_extents(&self) -> (i32, i32) {
        self.height_map.extents()
    }

    /// Raw logic-height bytes (`m_logicHeightMap->getDataPtr()`).
    pub fn logic_height_map_bytes(&self) -> &[u8] {
        self.height_map.samples()
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
        if self.height_map.apply_snapshot_prefix(data)? {
            effect();
        }
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
        self.height_map.restore_snapshot(width, height, data)?;
        effect();
        Ok(())
    }

    /// Restore only the terrain value for a detached load candidate. The
    /// caller must restore that candidate's partition, shroud and objects
    /// from the same snapshot; no ambient world is notified here.
    pub fn restore_active_boundary_from_snapshot(&mut self, boundary: i32) {
        if boundary < 0 {
            return;
        }
        if let Some(candidate) = self.boundaries.get(boundary as usize) {
            if candidate.x != 0 && candidate.y != 0 {
                self.active_boundary = boundary;
            }
        }
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
        self.height_map
            .flatten_terrain_box_at_with_effects(x, y, angle, halfsize_x, halfsize_y, effect)
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
        self.height_map
            .flatten_terrain_at_with_effects(x, y, radius, effect)
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
        self.height_map
            .create_crater(pos.x, pos.y, radius, &mut compatibility_height_change);
    }

    /// Set raw map height at grid position — only lowers, never raises.
    /// Reference: C++ W3DTerrainVisual::setRawMapHeight() in W3DTerrainVisual.cpp:909-937
    ///
    /// C++ adds getBorderSizeInline() to the incoming playable-grid coords
    /// before indexing the full (playable + 2*border) height buffer.
    #[cfg(test)]
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
        self.height_map
            .set_raw_map_height_with_effects(x, y, height, effect);
    }

    /// Get raw map height at grid position.
    /// Reference: C++ W3DTerrainVisual::getRawMapHeight() in W3DTerrainVisual.cpp:941-951
    #[cfg(test)]
    pub(super) fn get_raw_map_height(&self, x: i32, y: i32) -> i32 {
        self.height_map.get_raw_map_height(x, y)
    }
}

// Legacy native compatibility delivery. Main flatten_and_snap still reaches
// this wrapper; explicit receiving native effects remain tracked by hq-x3l7v.
fn compatibility_height_change(change: TerrainHeightChange) {
    if let Some(visual) = crate::helpers::TheTerrainVisual::get() {
        visual.set_raw_map_height(
            change.playable_x,
            change.playable_y,
            change.requested_height,
        );
    }
}
