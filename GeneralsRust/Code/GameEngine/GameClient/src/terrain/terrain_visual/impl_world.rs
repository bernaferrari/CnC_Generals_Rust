// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl TerrainVisualImpl {
    /// Load terrain heightmap from file
    pub fn load_heightmap(&mut self, path: &str) -> TerrainResult<()> {
        self.load_heightmap_with_world_size(path, None)
    }

    /// Load terrain heightmap from runtime map data (C++ parity fallback when no external hint exists).
    pub fn load_heightmap_from_data(
        &mut self,
        mut heightmap: HeightMap,
        source_hint: Option<&Path>,
        world_size: Option<(f32, f32)>,
    ) -> TerrainResult<()> {
        if heightmap.width == 0 || heightmap.height == 0 {
            return Err(TerrainError::HeightmapError(
                "Runtime heightmap has invalid dimensions".to_string(),
            ));
        }
        if heightmap.heights.len()
            != (heightmap.width as usize).saturating_mul(heightmap.height as usize)
        {
            return Err(TerrainError::HeightmapError(
                "Runtime heightmap sample count does not match dimensions".to_string(),
            ));
        }

        self.filename = source_hint
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "<runtime_heightmap>".to_string());
        self.ensure_terrain_definitions(source_hint)?;
        if self.texture_rules.is_empty() {
            self.ensure_default_textures();
        }

        // C++ MapObject.h: MAP_XY_FACTOR = 10 is the cell size.
        // Playable world_size / (full_grid-1) is not that scale — Alpine
        // playable 1750 / 314 ≈ 5.6–6.4 and left border_size at 0.
        // Live pipeline always passes world_size; tests pass None and keep
        // the caller's HeightMap::new scale/border.
        if world_size.is_some() {
            apply_cpp_visual_heightmap_scale(&mut heightmap);
        }

        self.config.heightmap_resolution = (heightmap.width, heightmap.height);
        self.config.world_size = world_size.unwrap_or((
            heightmap.width as f32 * heightmap.scale,
            heightmap.height as f32 * heightmap.scale,
        ));
        self.chunk_manager.set_config(self.config.clone());
        self.chunk_manager
            .load_heightmap(&heightmap, &self.config)?;
        self.height_map = Some(heightmap);
        self.reset_draw_area_state();
        self.upload_extra_blend_overlay();
        self.apply_tree_world_bounds();
        self.load_water_tracks_from_map();
        self.rebuild_shoreline();
        self.overlay.overlays_dirty = true;
        self.overlay.water_grid_dirty = true;
        Ok(())
    }

    pub fn load_heightmap_with_world_size(
        &mut self,
        path: &str,
        world_size: Option<(f32, f32)>,
    ) -> TerrainResult<()> {
        log::info!("Loading terrain heightmap: {}", path);
        self.filename = path.to_string();
        self.ensure_terrain_definitions(Some(Path::new(path)))?;
        if self.texture_rules.is_empty() {
            self.ensure_default_textures();
        }

        // Load heightmap using the appropriate loader based on file extension
        let mut heightmap = if path.ends_with(".hmp") {
            HeightMap::load_hmp(path)?
        } else if path.ends_with(".tga") {
            HeightMap::load_tga(path)?
        } else if path.ends_with(".raw") {
            HeightMap::load_raw(path)?
        } else {
            return Err(TerrainError::HeightmapError(format!(
                "Unsupported heightmap format: {}",
                path
            )));
        };

        // C++ MapObject.h MAP_XY_FACTOR=10. File loaders leave scale=1 /
        // border=0; never derive playable_world/(full_grid-1) (~6.4).
        apply_cpp_visual_heightmap_scale(&mut heightmap);

        // Update terrain configuration based on heightmap
        self.config.heightmap_resolution = (heightmap.width, heightmap.height);
        self.config.world_size = world_size.unwrap_or((
            heightmap.width as f32 * heightmap.scale,
            heightmap.height as f32 * heightmap.scale,
        ));

        self.chunk_manager.set_config(self.config.clone());

        // Initialize chunk system with heightmap data
        self.chunk_manager
            .load_heightmap(&heightmap, &self.config)?;

        self.height_map = Some(heightmap);
        self.reset_draw_area_state();
        self.upload_extra_blend_overlay();
        self.apply_tree_world_bounds();
        self.load_water_tracks_from_map();
        self.rebuild_shoreline();
        self.overlay.overlays_dirty = true;
        self.overlay.water_grid_dirty = true;

        log::info!("Terrain heightmap loaded successfully");
        Ok(())
    }

    /// Load terrain textures
    pub fn load_textures(&mut self, texture_paths: &[&str]) -> TerrainResult<()> {
        self.ensure_terrain_definitions(None)?;
        let normalized_paths: Vec<String> = texture_paths
            .iter()
            .filter_map(|path| Self::normalize_terrain_texture_path(path))
            .collect();
        let normalized_refs: Vec<&str> = normalized_paths.iter().map(String::as_str).collect();
        let ids = self.texture_system.load_textures(&normalized_refs)?;

        if ids.is_empty() {
            if self.texture_rules.is_empty() {
                self.ensure_default_textures();
            }
            return Ok(());
        }

        self.build_rules_from_textures(&ids);
        Ok(())
    }

    /// Update seismic simulations
    pub fn update_seismic_simulations(&mut self) {
        let mut active_simulations = Vec::new();
        let simulations = std::mem::take(&mut self.seismic_simulations);
        for mut simulation in simulations {
            simulation.life += 1;

            // Apply seismic effects to terrain
            if let Some(heightmap) = self.height_map.as_mut() {
                let chunk_manager = &mut self.chunk_manager;
                Self::apply_seismic_effect(chunk_manager, &simulation, heightmap);
            }

            // Keep simulation if it's still active
            if simulation.life < 15 {
                active_simulations.push(simulation);
            }
        }

        self.seismic_simulations = active_simulations;
    }

    /// Apply seismic effect to heightmap
    fn apply_seismic_effect(
        chunk_manager: &mut ChunkManager,
        simulation: &SeismicSimulationNode,
        heightmap: &mut HeightMap,
    ) {
        let center_x = simulation.center.x;
        let center_z = simulation.center.z;
        let radius = simulation.radius;
        let magnitude = simulation.magnitude;

        if simulation.life == 0 || simulation.life >= 15 {
            return;
        }

        let effect_magnitude = magnitude / simulation.life as f32;

        // Apply dome-style seismic effect
        for y in 0..heightmap.height as i32 {
            for x in 0..heightmap.width as i32 {
                let world_x = x as f32 * heightmap.scale;
                let world_z = y as f32 * heightmap.scale;

                let dx = world_x - center_x;
                let dz = world_z - center_z;
                let distance = (dx * dx + dz * dz).sqrt();

                if distance < radius {
                    let distance_factor = (1.0 - distance / radius).max(0.0);
                    let height_offset = effect_magnitude
                        * distance_factor
                        * (std::f32::consts::PI * distance / radius / 2.0).cos();

                    // Modify heightmap
                    let index = (y as u32 * heightmap.width + x as u32) as usize;
                    if index < heightmap.heights.len() {
                        heightmap.heights[index] += height_offset;
                        heightmap.heights[index] =
                            heightmap.heights[index].clamp(0.0, heightmap.max_height);
                    }
                }
            }
        }

        // Mark affected chunks as dirty
        chunk_manager.mark_region_dirty(
            simulation.region.0.x,
            simulation.region.0.z,
            simulation.region.1.x,
            simulation.region.1.z,
        );
        chunk_manager.refresh_dirty_chunks(heightmap);
    }

    /// Add seismic simulation
    pub fn add_seismic_simulation(&mut self, simulation: SeismicSimulationNode) {
        self.seismic_simulations.push(simulation);
    }

    /// Get terrain color at position
    pub fn get_terrain_color_at(&self, x: f32, y: f32) -> Result<[f32; 3], TerrainError> {
        if let Some(height_map) = self.height_map.as_ref() {
            return Ok(height_map.get_terrain_color_at_world(x, y, &self.source_tiles));
        }

        self.texture_system.sample_color_at(x, y)
    }

    /// `TheTerrainVisual->getTerrainColorAt` restricted to REAL tile art
    /// (C++ `W3DRadar::buildTerrainTexture` → `WorldHeightMap::
    /// getTerrainColorAt`, WorldHeightMap.cpp:2347-2356: a null
    /// `getSourceTile` leaves the color unset). Stand-in (missing-art)
    /// tiles report `None` so the radar software path shades its fallback
    /// base color instead of sampling the hash placeholder as terrain.
    pub fn radar_terrain_color_at(&self, x: f32, y: f32) -> Option<[f32; 3]> {
        let height_map = self.height_map.as_ref()?;
        let tile_ndx = height_map.tile_ndx_at_world(x, y)?;
        if self.stand_in_source_tiles.get(tile_ndx).copied().unwrap_or(true) {
            return None;
        }
        let tile = self.source_tiles.get(tile_ndx)?.as_ref()?;
        let pixel = tile.get_rgb_data_for_width(1);
        if pixel.len() < 3 {
            return None;
        }
        Some([
            pixel[2] as f32 / 255.0,
            pixel[1] as f32 / 255.0,
            pixel[0] as f32 / 255.0,
        ])
    }

    /// Get terrain tile type at position
    pub fn get_terrain_tile(&self, x: f32, y: f32) -> Result<u32, TerrainError> {
        if let Some(height_map) = self.height_map.as_ref() {
            Ok(height_map.get_packed_terrain_tile_at_world(x, y))
        } else {
            self.texture_system.get_terrain_type_at(x, y)
        }
    }

    /// Ray-terrain intersection
    pub fn intersect_terrain(&self, ray_start: Vec3, ray_end: Vec3) -> Option<Vec3> {
        if let Some(ref heightmap) = self.height_map {
            heightmap.intersect_ray(ray_start, ray_end)
        } else {
            None
        }
    }

    /// Enable/disable water grid
    pub fn enable_water_grid(&mut self, enable: bool) {
        self.water_grid_enabled = enable;
        self.water_system.set_enabled(enable);
        self.overlay.water_grid_dirty = true;
        self.overlay.overlays_dirty = true;
    }

    /// Current C++ water-grid enabled flag.
    pub fn water_grid_enabled(&self) -> bool {
        self.water_grid_enabled
    }

    /// CPU water-grid state.
    pub fn water_grid_state(&self) -> &WaterGridCpuState {
        &self.water_grid
    }

    /// C++ `setWaterGridHeightClamps`.
    pub fn set_water_grid_height_clamps(&mut self, low: f32, high: f32) {
        self.water_grid.height_clamps = (low, high);
    }

    /// C++ `setWaterAttenuationFactors`.
    pub fn set_water_attenuation_factors(&mut self, a: f32, b: f32, c: f32, range: f32) {
        self.water_grid.attenuation = (a, b, c, range);
    }

    /// C++ `setWaterTransform(angle, x, y, z)`.
    pub fn set_water_transform(&mut self, angle: f32, x: f32, y: f32, z: f32) {
        self.water_grid.transform =
            Mat4::from_translation(Vec3::new(x, y, z)) * Mat4::from_rotation_z(angle);
    }

    /// C++ `setWaterTransform(Matrix3D*)`.
    pub fn set_water_transform_matrix(&mut self, transform: Mat4) {
        self.water_grid.transform = transform;
    }

    /// C++ `getWaterTransform`.
    pub fn water_transform(&self) -> Mat4 {
        self.water_grid.transform
    }

    /// C++ `setWaterGridResolution`.
    pub fn set_water_grid_resolution(
        &mut self,
        grid_cells_x: f32,
        grid_cells_y: f32,
        cell_size: f32,
    ) {
        let old_resolution = self.water_grid.resolution;
        let cell_size = cell_size.max(f32::EPSILON);
        self.water_grid.resolution.2 = cell_size;
        // C++ W3DWater.cpp compares `m_gridCellsY != m_gridCellsY`, so y-only
        // resolution changes do not reallocate or update the stored y count.
        if old_resolution.0 != grid_cells_x {
            self.water_grid.resolution.0 = grid_cells_x;
            self.water_grid.resolution.1 = grid_cells_y;
            self.water_grid.height_deltas.clear();
            self.water_grid.point_motions.clear();
            self.water_grid.velocity_events.clear();
        }
    }

    /// C++ `getWaterGridResolution`.
    pub fn water_grid_resolution(&self) -> (f32, f32, f32) {
        self.water_grid.resolution
    }

    /// C++ `changeWaterHeight`.
    pub fn change_water_height(&mut self, world_x: f32, world_y: f32, delta: f32) -> bool {
        let Some((grid_x, grid_y)) = self.water_grid_space(world_x, world_y) else {
            return false;
        };
        let (grid_cells_x, grid_cells_y, cell_size) = self.water_grid.resolution;
        let (att0, att1, att2, range) = self.water_grid.attenuation;
        let range = range / cell_size;
        let min_x = (grid_x - range).floor().max(0.0) as i32;
        let max_x = (grid_x + range).ceil().min(grid_cells_x) as i32;
        let min_y = (grid_y - range).floor().max(0.0) as i32;
        let max_y = (grid_y + range).ceil().min(grid_cells_y) as i32;
        let (min_height, max_height) = self.water_grid.height_clamps;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let distance = ((grid_x - x as f32).powi(2) + (grid_y - y as f32).powi(2)).sqrt();
                let denominator = att0 + att1 * distance + distance * distance * att2;
                if denominator == 0.0 {
                    continue;
                }
                let old_height = self
                    .water_grid
                    .height_deltas
                    .get(&(x, y))
                    .copied()
                    .unwrap_or(0.0);
                let new_height = (old_height + delta / denominator).clamp(min_height, max_height);
                self.water_grid.height_deltas.insert((x, y), new_height);
            }
        }
        true
    }

    /// C++ `addWaterVelocity`.
    pub fn add_water_velocity(
        &mut self,
        world_x: f32,
        world_y: f32,
        velocity: f32,
        preferred_height: f32,
    ) {
        if !self.water_grid_enabled {
            return;
        }
        let Some((grid_x, grid_y)) = self.water_grid_space(world_x, world_y) else {
            return;
        };
        let (grid_cells_x, grid_cells_y, cell_size) = self.water_grid.resolution;
        let range = self.water_grid.attenuation.3 / cell_size;
        let min_x = (grid_x - range).floor().max(0.0) as i32;
        let max_x = (grid_x + range).ceil().min(grid_cells_x) as i32;
        let min_y = (grid_y - range).floor().max(0.0) as i32;
        let max_y = (grid_y + range).ceil().min(grid_cells_y) as i32;

        for y in min_y..=max_y {
            for x in min_x..=max_x {
                let motion =
                    self.water_grid
                        .point_motions
                        .entry((x, y))
                        .or_insert(WaterGridPointMotion {
                            velocity: 0.0,
                            preferred_height,
                            in_motion: false,
                        });
                motion.preferred_height = preferred_height;
                motion.velocity += velocity;
                motion.in_motion = true;
            }
        }

        self.water_grid
            .velocity_events
            .push(WaterGridVelocityEvent {
                world_x,
                world_y,
                velocity,
                preferred_height,
            });
    }

    /// C++ `getWaterGridHeight`.
    pub fn get_water_grid_height(&self, world_x: f32, world_y: f32) -> Option<f32> {
        if !self.water_grid_enabled {
            return None;
        }
        let (grid_x, grid_y) = self.water_grid_indices(world_x, world_y)?;
        let base_height = self.water_grid.transform.w_axis.z;
        Some(
            base_height
                + self
                    .water_grid
                    .height_deltas
                    .get(&(grid_x, grid_y))
                    .copied()
                    .unwrap_or(0.0),
        )
    }

    fn water_grid_space(&self, world_x: f32, world_y: f32) -> Option<(f32, f32)> {
        let (grid_cells_x, grid_cells_y, cell_size) = self.water_grid.resolution;
        if grid_cells_x < 1.0 || grid_cells_y < 1.0 || cell_size <= 0.0 {
            return None;
        }
        let local = self
            .water_grid
            .transform
            .inverse()
            .transform_point3(Vec3::new(world_x, world_y, 0.0));
        let grid_x = local.x / cell_size;
        let grid_y = local.y / cell_size;
        if grid_x < 0.0
            || grid_y < 0.0
            || grid_x > grid_cells_x - 1.0
            || grid_y > grid_cells_y - 1.0
        {
            return None;
        }
        Some((grid_x, grid_y))
    }

    fn water_grid_indices(&self, world_x: f32, world_y: f32) -> Option<(i32, i32)> {
        let (grid_x, grid_y) = self.water_grid_space(world_x, world_y)?;
        Some((grid_x as i32, grid_y as i32))
    }

    /// Current C++ terrain bib records.
    pub fn terrain_bibs(&self) -> &[TerrainBibRecord] {
        &self.terrain_bibs
    }

    /// Current C++ terrain prop records.
    pub fn terrain_props(&self) -> &[TerrainPropRecord] {
        &self.terrain_props
    }

    /// Current C++ construction tree/prop removal requests.
    pub fn construction_removals(&self) -> &[TerrainConstructionRemoval] {
        &self.construction_removals
    }

    /// C++ terrain-track render object system owned by `W3DTerrainVisual`.
    pub fn terrain_tracks(&self) -> &TerrainTracksRenderObjClassSystem {
        &self.terrain_tracks
    }

    /// Mutable access for callers that bind and append track marks.
    pub fn terrain_tracks_mut(&mut self) -> &mut TerrainTracksRenderObjClassSystem {
        &mut self.terrain_tracks
    }

    /// C++ `setTerrainTracksDetail`.
    pub fn set_terrain_tracks_detail(&mut self) {
        self.set_terrain_tracks_detail_with_config(Self::terrain_tracks_config());
    }

    /// Forward a concrete config into the terrain-track system.
    pub fn set_terrain_tracks_detail_with_config(&mut self, config: TerrainTracksConfig) {
        self.terrain_tracks.set_detail(config);
    }

    /// C++ `addFactionBib`/`addFactionBibDrawable` geometry wrapper.
    pub fn add_faction_bib(
        &mut self,
        owner_id: u32,
        owner_kind: TerrainBibOwnerKind,
        transform: Mat4,
        major_radius: f32,
        minor_radius: f32,
        geometry_is_box: bool,
        factory_exit_width: f32,
        factory_extra_bib_width: f32,
        highlight: bool,
        extra: f32,
    ) -> bool {
        if self.height_map.is_none() {
            return false;
        }

        let size_x = major_radius;
        let size_y = if geometry_is_box {
            minor_radius
        } else {
            major_radius
        };
        let extra_width = factory_extra_bib_width + extra;
        let corners = [
            Vec3::new(-size_x - extra_width, -size_y - extra_width, 0.0),
            Vec3::new(
                size_x + factory_exit_width + extra_width,
                -size_y - extra_width,
                0.0,
            ),
            Vec3::new(
                size_x + factory_exit_width + extra_width,
                size_y + extra_width,
                0.0,
            ),
            Vec3::new(-size_x - extra_width, size_y + extra_width, 0.0),
        ]
        .map(|corner| transform.transform_point3(corner).to_array());

        if let Some(existing) = self
            .terrain_bibs
            .iter_mut()
            .find(|bib| bib.owner_id == owner_id && bib.owner_kind == owner_kind)
        {
            existing.corners = corners;
            existing.highlight = highlight;
        } else {
            self.terrain_bibs.push(TerrainBibRecord {
                owner_id,
                owner_kind,
                corners,
                highlight,
            });
        }
        self.overlay.overlays_dirty = true;
        true
    }

    /// C++ `removeFactionBib`/`removeFactionBibDrawable`.
    pub fn remove_faction_bib(&mut self, owner_id: u32, owner_kind: TerrainBibOwnerKind) {
        self.terrain_bibs
            .retain(|bib| bib.owner_id != owner_id || bib.owner_kind != owner_kind);
        self.overlay.overlays_dirty = true;
    }

    /// C++ `removeAllBibs`.
    pub fn remove_all_bibs(&mut self) {
        self.terrain_bibs.clear();
        self.overlay.overlays_dirty = true;
    }

    /// C++ `removeBibHighlighting`.
    pub fn remove_bib_highlighting(&mut self) {
        for bib in &mut self.terrain_bibs {
            bib.highlight = false;
        }
        self.overlay.overlays_dirty = true;
    }

    /// C++ `removeTreesAndPropsForConstruction`.
    pub fn remove_trees_and_props_for_construction(
        &mut self,
        position: [f32; 3],
        major_radius: f32,
        minor_radius: f32,
        geometry_is_box: bool,
        angle: f32,
    ) {
        self.construction_removals.push(TerrainConstructionRemoval {
            position,
            major_radius,
            minor_radius,
            geometry_is_box,
            angle,
        });
        self.tree_buffer.remove_trees_for_construction(
            crate::terrain::TreeConstructionGeometry {
                position: Vec3::from_array(position),
                major_radius,
                minor_radius,
                geometry_type: if geometry_is_box {
                    crate::terrain::TreeGeometryType::Box
                } else {
                    crate::terrain::TreeGeometryType::Cylinder
                },
                angle,
            },
        );
        self.tree_buffer.force_vertex_rebuild();
        self.terrain_props.retain(|prop| {
            !Self::point_inside_construction_footprint(
                prop.position,
                position,
                major_radius,
                minor_radius,
                geometry_is_box,
                angle,
            )
        });
    }

    /// C++ `addProp` terminal terrain-render-object call.
    pub fn add_prop(
        &mut self,
        position: [f32; 3],
        angle: f32,
        scale: f32,
        model_name: &str,
    ) -> bool {
        if model_name.is_empty() {
            return false;
        }
        self.terrain_props.push(TerrainPropRecord {
            position,
            angle,
            scale,
            model_name: model_name.to_string(),
        });
        true
    }

    fn point_inside_construction_footprint(
        point: [f32; 3],
        center: [f32; 3],
        major_radius: f32,
        minor_radius: f32,
        geometry_is_box: bool,
        angle: f32,
    ) -> bool {
        let dx = point[0] - center[0];
        let dy = point[1] - center[1];
        let (sin, cos) = angle.sin_cos();
        let local_x = dx * cos + dy * sin;
        let local_y = -dx * sin + dy * cos;
        let size_y = if geometry_is_box {
            minor_radius
        } else {
            major_radius
        };

        local_x.abs() <= major_radius && local_y.abs() <= size_y
    }

    /// Replace skybox textures
    pub fn replace_skybox_textures(
        &mut self,
        old_names: &[&str; 5],
        new_names: &[&str; 5],
    ) -> TerrainResult<()> {
        for (i, old_name) in old_names.iter().enumerate() {
            if self.initial_skybox_texture_names[i].is_none() {
                let old_name = (*old_name).to_string();
                self.initial_skybox_texture_names[i] = Some(old_name.clone());
                self.current_skybox_texture_names[i] = Some(old_name);
            }
        }

        let mut replacements = Vec::new();
        for (i, new_name) in new_names.iter().enumerate() {
            let should_replace = self.current_skybox_texture_names[i].as_deref() != Some(*new_name);
            if should_replace {
                replacements.push((i, (*new_name).to_string()));
            }
        }

        if replacements.is_empty() {
            // C++ `W3DTerrainVisual::replaceSkyboxTextures` still keeps the
            // already-bound skybox; refresh so a later GPU init cannot leave peach.
            return self.refresh_skybox_background_binding_if_ready();
        }

        let loaded_textures = self.load_skybox_replacement_textures(&replacements);

        for (i, new_name) in replacements {
            self.current_skybox_texture_names[i] = Some(new_name);
        }
        self.install_loaded_skybox_faces(loaded_textures);

        self.refresh_skybox_background_binding_if_ready()?;
        Ok(())
    }

    /// Initial/default skybox names captured by C++ `replaceSkyboxTextures`.
    pub fn initial_skybox_texture_names(&self) -> &[Option<String>; 5] {
        &self.initial_skybox_texture_names
    }

    pub fn current_skybox_texture_names(&self) -> &[Option<String>; 5] {
        &self.current_skybox_texture_names
    }

    /// Last face name that produced a GPU bind, or the horizon-gradient tag.
    pub fn last_skybox_face_bind(&self) -> Option<&str> {
        self.last_skybox_face_bind.as_deref()
    }

    /// C++ `new_skybox` has five faces (N/E/S/W/T). The live wgpu path is a
    /// fullscreen triangle, so pick the face the camera is looking at.
    ///
    /// Yaw sectors match `View` / `W3DView` Z-up: angle 0 looks +Y (north).
    /// Top (`T`, index 4) only when the look vector points upward — a high
    /// RTS pitch looks *down* at the map and must keep the side faces.
    pub fn skybox_face_from_yaw_pitch(yaw: f32, look_pitch: f32) -> usize {
        const TOP_LOOK_PITCH: f32 = 0.55;
        if look_pitch > TOP_LOOK_PITCH {
            return 4;
        }
        let two_pi = 2.0 * PI;
        let yaw = yaw.rem_euclid(two_pi);
        let sector = ((yaw + PI * 0.25) / (PI * 0.5)).floor() as i32;
        match sector.rem_euclid(4) {
            0 => 0,
            1 => 1,
            2 => 2,
            _ => 3,
        }
    }

    /// Pick N/E/S/W/T from a wgpu/glam view matrix (Y-up or Z-up).
    pub fn skybox_face_from_view_matrix(view: &Mat4) -> usize {
        let inv = view.inverse();
        let forward = inv.transform_vector3(-Vec3::Z);
        let world_up = inv.transform_vector3(Vec3::Y);
        let up = if world_up.length_squared() < 1.0e-8 {
            Vec3::Z
        } else {
            world_up.normalize()
        };
        let fwd = if forward.length_squared() < 1.0e-8 {
            if up.z.abs() >= up.y.abs() {
                Vec3::Y
            } else {
                Vec3::Z
            }
        } else {
            forward.normalize()
        };
        let look_pitch = fwd.dot(up).clamp(-1.0, 1.0).asin();
        let north = if up.z.abs() >= up.y.abs() {
            Vec3::Y
        } else {
            Vec3::Z
        };
        let east = up.cross(north);
        let horiz = fwd - up * fwd.dot(up);
        let yaw = if horiz.length_squared() < 1.0e-8 {
            0.0
        } else {
            let h = horiz.normalize();
            h.dot(east).atan2(h.dot(north))
        };
        Self::skybox_face_from_yaw_pitch(yaw, look_pitch)
    }

    /// Rebind the visible skybox face from the live camera. Called every
    /// `TerrainVisual::render` / `update` (via `update_tree_meshes`).
    pub fn rebind_skybox_background_for_camera(&mut self) {
        let _ = self.refresh_skybox_background_binding_if_ready();
    }

    fn skybox_camera_yaw_and_look_pitch() -> (f32, f32) {
        crate::display::view::with_tactical_view_ref(|view| {
            let eye = view.get_3d_camera_position();
            let target = view.position();
            let forward = Vec3::new(target.x - eye.x, target.y - eye.y, target.z - eye.z);
            let len = forward.length();
            let look_pitch = if len > 1.0e-5 { (forward.z / len).asin() } else { 0.0 };
            (view.angle(), look_pitch)
        })
    }

    fn is_loaded_skybox_face(&self, index: usize) -> bool {
        let Some(name) = self
            .current_skybox_texture_names
            .get(index)
            .and_then(|name| name.as_deref())
        else {
            return false;
        };
        if is_synthetic_skybox_bind(name) {
            return false;
        }
        if self.skybox_textures.get(index).is_none_or(Option::is_none) {
            return false;
        }
        // Synthetic 1×N gradient lives in slot 0 only when no real face loaded.
        if index == 0 && is_synthetic_skybox_bind(self.last_skybox_face_bind.as_deref().unwrap_or(""))
        {
            return false;
        }
        true
    }

    fn resolve_skybox_face_index(&self, preferred: usize) -> Option<usize> {
        if self.is_loaded_skybox_face(preferred) {
            return Some(preferred);
        }
        if preferred == 4 {
            let (yaw, _) = Self::skybox_camera_yaw_and_look_pitch();
            let side = Self::skybox_face_from_yaw_pitch(yaw, 0.0);
            if self.is_loaded_skybox_face(side) {
                return Some(side);
            }
        }
        [0usize, 1, 2, 3, 4]
            .into_iter()
            .find(|&idx| self.is_loaded_skybox_face(idx))
    }

    pub fn has_skybox_background_bind_group(&self) -> bool {
        self.skybox_background_bind_group.is_some()
    }

    /// C++ `W3DFileSystem` + `DDSFileClass` search list (tga↔dds, Art/Textures, map dir).
    pub fn skybox_texture_search_candidates(&self, path: &str) -> Vec<PathBuf> {
        self.runtime_texture_candidates(path)
    }

    fn load_skybox_replacement_textures(
        &self,
        replacements: &[(usize, String)],
    ) -> Vec<(usize, Texture)> {
        let Some(device) = self.device.as_ref().cloned() else {
            return Vec::new();
        };

        let mut loaded = Vec::new();
        for (i, texture_path) in replacements {
            match self.load_texture_from_path(device.as_ref(), texture_path) {
                Ok(texture) => {
                    info!(
                        "Skybox face {} bound from '{}'",
                        i, texture_path
                    );
                    loaded.push((*i, texture));
                }
                Err(err) => {
                    warn!(
                        "Skybox face {} '{}' failed to load; keeping remaining faces: {}",
                        i, texture_path, err
                    );
                }
            }
        }
        loaded
    }

    fn refresh_skybox_background_binding_if_ready(&mut self) -> TerrainResult<()> {
        if let Some(device) = self.device.as_ref().cloned() {
            self.refresh_skybox_background_binding(device.as_ref())?;
        }
        Ok(())
    }

    fn restore_initial_skybox_textures(&mut self) -> TerrainResult<()> {
        let mut replacements = Vec::new();
        for i in 0..5 {
            let Some(initial_name) = self.initial_skybox_texture_names[i].clone() else {
                continue;
            };
            if self.current_skybox_texture_names[i].as_deref() != Some(initial_name.as_str()) {
                replacements.push((i, initial_name));
            }
        }

        if replacements.is_empty() {
            return Ok(());
        }

        let loaded_textures = self.load_skybox_replacement_textures(&replacements);

        for (i, initial_name) in replacements {
            self.current_skybox_texture_names[i] = Some(initial_name);
        }
        self.install_loaded_skybox_faces(loaded_textures);

        self.refresh_skybox_background_binding_if_ready()
    }

    fn refresh_skybox_background_binding(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(layout) = self.skybox_background_bind_group_layout.clone() else {
            return Ok(());
        };

        if self.skybox_sampler.is_none() {
            self.skybox_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Terrain Skybox Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }

        self.ensure_water_ini_skybox_faces_loaded();

        let (yaw, look_pitch) = Self::skybox_camera_yaw_and_look_pitch();
        let preferred = Self::skybox_face_from_yaw_pitch(yaw, look_pitch);
        let mut used_gradient = false;
        let selected_index = match self.resolve_skybox_face_index(preferred) {
            Some(index) => index,
            None => {
                // Live path is a fullscreen triangle (no `new_skybox` W3D mesh).
                // Horizon gradient when no N/E/S/W/T face loaded — never a fog card.
                if !self.ensure_skybox_horizon_gradient_texture(device) {
                    self.skybox_background_view = None;
                    self.skybox_background_bind_group = None;
                    self.last_skybox_face_bind = None;
                    return Ok(());
                }
                used_gradient = true;
                0
            }
        };

        if is_synthetic_skybox_bind(self.last_skybox_face_bind.as_deref().unwrap_or(""))
            && selected_index != 0
        {
            self.skybox_textures[0] = None;
        }

        let face_name = if used_gradient {
            HORIZON_GRADIENT_BIND.to_string()
        } else {
            self.current_skybox_texture_names[selected_index]
                .clone()
                .unwrap_or_else(|| format!("face-{selected_index}"))
        };
        if self.skybox_background_bind_group.is_some()
            && self.last_skybox_face_bind.as_deref() == Some(face_name.as_str())
        {
            return Ok(());
        }

        let view = self.skybox_textures[selected_index]
            .as_ref()
            .expect("selected skybox texture must exist")
            .create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self
            .skybox_sampler
            .as_ref()
            .expect("skybox sampler should be initialised");

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Terrain Skybox Background Bind Group"),
            layout: layout.as_ref(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        info!("Skybox background bind group ready using {}", face_name);
        self.last_skybox_face_bind = Some(face_name);
        self.skybox_background_view = Some(view);
        self.skybox_background_bind_group = Some(bind_group);
        Ok(())
    }

    fn install_loaded_skybox_faces(&mut self, loaded_textures: Vec<(usize, Texture)>) {
        if loaded_textures.is_empty() {
            return;
        }
        if is_synthetic_skybox_bind(self.last_skybox_face_bind.as_deref().unwrap_or("")) {
            self.last_skybox_face_bind = None;
            self.skybox_background_bind_group = None;
            self.skybox_background_view = None;
        }
        for (i, texture) in loaded_textures {
            self.skybox_textures[i] = Some(texture);
        }
    }

    fn ensure_water_ini_skybox_faces_loaded(&mut self) {
        if (0..5).any(|i| self.is_loaded_skybox_face(i)) {
            return;
        }
        if is_synthetic_skybox_bind(self.last_skybox_face_bind.as_deref().unwrap_or("")) {
            return;
        }

        let water_names = water_ini_or_default_skybox_names();
        let mut replacements = Vec::new();
        for i in 0..5 {
            let name = self
                .current_skybox_texture_names
                .get(i)
                .and_then(|name| name.as_deref())
                .filter(|name| !name.is_empty() && !is_synthetic_skybox_bind(name))
                .map(str::to_string)
                .unwrap_or_else(|| water_names[i].clone());
            if name.is_empty() {
                continue;
            }
            if self.initial_skybox_texture_names[i].is_none() {
                self.initial_skybox_texture_names[i] = Some(name.clone());
            }
            self.current_skybox_texture_names[i] = Some(name.clone());
            replacements.push((i, name));
        }
        if replacements.is_empty() {
            return;
        }
        let loaded = self.load_skybox_replacement_textures(&replacements);
        self.install_loaded_skybox_faces(loaded);

        if (0..5).any(|i| self.is_loaded_skybox_face(i)) {
            return;
        }

        let mut extra = Vec::new();
        for i in 0..5 {
            let water_name = &water_names[i];
            if water_name.is_empty() {
                continue;
            }
            if self.current_skybox_texture_names[i].as_deref() == Some(water_name.as_str()) {
                continue;
            }
            extra.push((i, water_name.clone()));
        }
        if extra.is_empty() {
            return;
        }
        let loaded = self.load_skybox_replacement_textures(&extra);
        for (i, name) in &extra {
            if loaded.iter().any(|(li, _)| li == i) {
                self.current_skybox_texture_names[*i] = Some(name.clone());
            }
        }
        self.install_loaded_skybox_faces(loaded);
    }

    fn ensure_skybox_horizon_gradient_texture(&mut self, device: &wgpu::Device) -> bool {
        if (0..5).any(|i| self.is_loaded_skybox_face(i)) {
            return false;
        }
        let Some(queue) = self.queue.as_ref() else {
            return false;
        };
        const WIDTH: u32 = 64;
        const HEIGHT: u32 = 128;
        let mut rgba = vec![0u8; (WIDTH * HEIGHT * 4) as usize];
        for y in 0..HEIGHT {
            let t = y as f32 / (HEIGHT - 1) as f32;
            let base = horizon_sky_color(t);
            for x in 0..WIDTH {
                let sun = (x as f32 / (WIDTH - 1) as f32 - 0.5) * 0.08;
                let r = (base[0] + sun * 0.85).clamp(0.0, 1.0);
                let g = (base[1] + sun * 0.35).clamp(0.0, 1.0);
                let b = (base[2] - sun * 0.12).clamp(0.0, 1.0);
                let i = ((y * WIDTH + x) * 4) as usize;
                rgba[i] = (r * 255.0) as u8;
                rgba[i + 1] = (g * 255.0) as u8;
                rgba[i + 2] = (b * 255.0) as u8;
                rgba[i + 3] = 255;
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Skybox Horizon Gradient"),
            size: wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH * 4),
                rows_per_image: Some(HEIGHT),
            },
            wgpu::Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.skybox_textures[0] = Some(texture);
        self.last_skybox_face_bind = Some(HORIZON_GRADIENT_BIND.to_string());
        true
    }

    /// Load texture from path
    fn load_texture_from_path(
        &self,
        device: &wgpu::Device,
        path: &str,
    ) -> TerrainResult<wgpu::Texture> {
        let queue = self.queue.as_ref().ok_or_else(|| {
            TerrainError::GPUError("TerrainVisual queue not initialised for texture upload".into())
        })?;

        let dyn_image = self.load_runtime_texture_image(path)?;
        let rgba = dyn_image.to_rgba8();
        let (width, height) = rgba.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(&format!("Texture: {}", path)),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        Ok(texture)
    }

    fn load_runtime_texture_image(&self, path: &str) -> TerrainResult<image::DynamicImage> {
        for candidate in self.runtime_texture_candidates(path) {
            match self.try_load_image_from_filesystem(&candidate) {
                Ok(Some(image)) => return Ok(image),
                Ok(None) => {}
                Err(err) => {
                    warn!(
                        "Skipping undecodable skybox candidate '{}': {}",
                        candidate.display(),
                        err
                    );
                }
            }
        }

        Err(TerrainError::TextureError(GameImageError::LoadError {
            path: path.to_string(),
            source: Box::new(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                format!("runtime texture '{}' not found", path),
            )),
        }))
    }

    fn try_load_image_from_filesystem(
        &self,
        candidate: &Path,
    ) -> TerrainResult<Option<image::DynamicImage>> {
        let resource_name = candidate.to_string_lossy().replace('\\', "/");
        let fs = get_file_system();
        let bytes = {
            let Ok(mut guard) = fs.lock() else {
                return Ok(None);
            };
            let access = FileAccess::READ.combine(FileAccess::BINARY);
            let Some(mut file) = guard.open_file(resource_name.as_str(), access) else {
                return Ok(None);
            };
            match file.read_entire_and_close() {
                Ok(bytes) => bytes,
                Err(err) => {
                    return Err(TerrainError::TextureError(GameImageError::LoadError {
                        path: resource_name,
                        source: Box::new(err),
                    }));
                }
            }
        };

        let extension = candidate
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase());
        let decoded = match extension.as_deref() {
            Some("tga") => image::load_from_memory_with_format(&bytes, ImageFormat::Tga),
            Some("dds") => image::load_from_memory_with_format(&bytes, ImageFormat::Dds),
            Some("png") => image::load_from_memory_with_format(&bytes, ImageFormat::Png),
            Some("jpg") | Some("jpeg") => {
                image::load_from_memory_with_format(&bytes, ImageFormat::Jpeg)
            }
            Some("bmp") => image::load_from_memory_with_format(&bytes, ImageFormat::Bmp),
            _ => image::load_from_memory(&bytes),
        }
        .map_err(|err| {
            TerrainError::TextureError(GameImageError::LoadError {
                path: candidate.display().to_string(),
                source: Box::new(err),
            })
        })?;

        Ok(Some(decoded))
    }

    fn runtime_texture_candidates(&self, path: &str) -> Vec<PathBuf> {
        let normalized = path.replace('\\', "/");
        let bare = normalized.trim_start_matches("./").to_string();
        if bare.is_empty() {
            return Vec::new();
        }

        let mut candidates = Vec::<PathBuf>::new();
        let mut seen: HashSet<PathBuf> = HashSet::new();
        let mut push_unique = |candidate: PathBuf| {
            if seen.insert(candidate.clone()) {
                candidates.push(candidate);
            }
        };

        let basename = Path::new(&bare)
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(bare.as_str())
            .to_string();
        let names = if basename != bare {
            vec![bare.clone(), basename.clone()]
        } else {
            vec![bare.clone()]
        };

        let language = get_registry_language().as_str().to_string();
        for name in &names {
            if !language.is_empty() {
                push_unique(PathBuf::from(format!(
                    "Data/{language}/{TGA_DIR_PATH}{name}"
                )));
            }
            // C++ W3DFileSystem.cpp:197-201 — TGA_DIR_PATH ("Art/Textures/").
            push_unique(PathBuf::from(format!("{TGA_DIR_PATH}{name}")));
            push_unique(PathBuf::from(format!("art/textures/{name}")));
            push_unique(PathBuf::from(format!("Data/Art/Textures/{name}")));
            push_unique(PathBuf::from(name.clone()));
        }

        // Custom-map directory next to the loaded .map (C++ AssetManager local lookup).
        if let Some(map_dir) = Path::new(&self.filename).parent() {
            if !map_dir.as_os_str().is_empty() {
                for name in &names {
                    push_unique(map_dir.join(name));
                }
            }
        }

        let global_data = global_data::read();
        let user_data = global_data.get_user_data_dir();
        if !user_data.is_empty() {
            for name in &names {
                let user_textures = Path::new(&user_data)
                    .join(USER_TGA_DIR_PATH.replace("%s", ""))
                    .join(name);
                push_unique(user_textures);

                let extension = Path::new(name)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.to_ascii_lowercase());
                if matches!(extension.as_deref(), Some("tga") | Some("dds")) {
                    let user_previews = Path::new(&user_data)
                        .join(MAP_PREVIEW_DIR_PATH.replace("%s", ""))
                        .join(name);
                    push_unique(user_previews);
                }
            }
        }
        drop(push_unique);

        // C++ DDSFileClass rewrites .tga → .dds before opening (ddsfile.cpp:33-37).
        let current = candidates.clone();
        for candidate in current {
            if let Some(swapped) = swapped_skybox_texture_extension(&candidate) {
                if seen.insert(swapped.clone()) {
                    candidates.push(swapped);
                }
            }
        }

        candidates
    }

}

fn swapped_skybox_texture_extension(path: &Path) -> Option<PathBuf> {
    let ext = path.extension()?.to_str()?;
    let stem = path.file_stem()?.to_str().filter(|stem| !stem.is_empty())?;
    let swapped = if ext.eq_ignore_ascii_case("tga") {
        "dds"
    } else if ext.eq_ignore_ascii_case("dds") {
        "tga"
    } else {
        return None;
    };
    Some(match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent.join(format!("{stem}.{swapped}")),
        _ => PathBuf::from(format!("{stem}.{swapped}")),
    })
}

const HORIZON_GRADIENT_BIND: &str = "horizon-gradient";
const FOG_FALLBACK_BIND: &str = "fog-fallback";

fn is_synthetic_skybox_bind(name: &str) -> bool {
    name.eq_ignore_ascii_case(HORIZON_GRADIENT_BIND) || name.eq_ignore_ascii_case(FOG_FALLBACK_BIND)
}

fn water_ini_or_default_skybox_names() -> [String; 5] {
    const DEFAULT: [&str; 5] = [
        "TSMorningN.tga",
        "TSMorningE.tga",
        "TSMorningS.tga",
        "TSMorningW.tga",
        "TSMorningT.tga",
    ];
    game_engine::common::ini::ini_water::initialize_water_settings();
    if let Some(lock) = game_engine::common::ini::ini_water::get_water_transparency() {
        if let Ok(guard) = lock.read() {
            let setting = guard.get_final_override();
            let names = [
                setting.skybox_texture_n.as_str().to_string(),
                setting.skybox_texture_e.as_str().to_string(),
                setting.skybox_texture_s.as_str().to_string(),
                setting.skybox_texture_w.as_str().to_string(),
                setting.skybox_texture_t.as_str().to_string(),
            ];
            if names.iter().any(|name| !name.is_empty()) {
                return names;
            }
        }
    }
    DEFAULT.map(str::to_string)
}

fn horizon_sky_color(t: f32) -> [f32; 3] {
    let t = t.clamp(0.0, 1.0);
    let zenith = [0.18, 0.38, 0.78];
    let mid = [0.45, 0.66, 0.92];
    let horizon = [0.93, 0.84, 0.68];
    let haze = [0.62, 0.68, 0.74];
    if t < 0.52 {
        let u = t / 0.52;
        [
            zenith[0] + (mid[0] - zenith[0]) * u,
            zenith[1] + (mid[1] - zenith[1]) * u,
            zenith[2] + (mid[2] - zenith[2]) * u,
        ]
    } else if t < 0.78 {
        let u = (t - 0.52) / 0.26;
        [
            mid[0] + (horizon[0] - mid[0]) * u,
            mid[1] + (horizon[1] - mid[1]) * u,
            mid[2] + (horizon[2] - mid[2]) * u,
        ]
    } else {
        let u = (t - 0.78) / 0.22;
        [
            horizon[0] + (haze[0] - horizon[0]) * u,
            horizon[1] + (haze[1] - horizon[1]) * u,
            horizon[2] + (haze[2] - horizon[2]) * u,
        ]
    }
}

/// C++ `MapObject.h` `MAP_XY_FACTOR` (10) is the cell size.
/// `WorldHeightMap::m_borderSize` is map-authored (`K_HEIGHT_MAP_VERSION_3+`);
/// ZH Alpine is 70. Loaders that omit border stay at 0, so default 70.
fn apply_cpp_visual_heightmap_scale(heightmap: &mut HeightMap) {
    heightmap.scale = game_engine::map_object::MAP_XY_FACTOR;
    if heightmap.border_size == 0 {
        heightmap.border_size = 70;
    }
}

#[cfg(test)]
mod visual_heightmap_scale_tests {
    use super::*;

    #[test]
    fn live_world_size_uses_map_xy_factor_not_playable_over_grid() {
        // C++ MapObject.h MAP_XY_FACTOR=10; WorldHeightMap.cpp m_borderSize.
        // Alpine-shaped 32 samples + playable 200 would derive 200/31 ≈ 6.45.
        let mut heightmap = HeightMap::new(32, 32, 255.0, 1.0);
        heightmap.border_size = 70;
        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(heightmap, None, Some((200.0, 200.0)))
            .expect("runtime heightmap should load");
        let loaded = visual.height_map.as_ref().expect("loaded");
        assert_eq!(loaded.scale, game_engine::map_object::MAP_XY_FACTOR);
        assert_eq!(loaded.border_size, 70);
        assert!(
            (loaded.scale - 200.0 / 31.0).abs() > 1.0,
            "must not use playable_world/(full_grid-1)"
        );
    }

    #[test]
    fn live_world_size_defaults_zh_alpine_border_when_map_omits() {
        let heightmap = HeightMap::new(32, 32, 255.0, 1.0);
        let mut visual = TerrainVisualImpl::new();
        visual
            .load_heightmap_from_data(heightmap, None, Some((200.0, 200.0)))
            .expect("runtime heightmap should load");
        let loaded = visual.height_map.as_ref().expect("loaded");
        assert_eq!(loaded.scale, 10.0);
        assert_ne!(loaded.border_size, 0);
        assert_eq!(loaded.border_size, 70);
    }

    #[test]
    fn file_world_size_uses_map_xy_factor_not_derived_scale() {
        let width: u32 = 32;
        let height: u32 = 32;
        let mut bytes = Vec::with_capacity(8 + (width * height * 2) as usize);
        bytes.extend_from_slice(&width.to_le_bytes());
        bytes.extend_from_slice(&height.to_le_bytes());
        bytes.extend(std::iter::repeat_n(0u8, (width * height * 2) as usize));
        let path = std::env::temp_dir().join("hq_zxgm_visual_heightmap.hmp");
        std::fs::write(&path, bytes).expect("write temp hmp");
        let mut visual = TerrainVisualImpl::new();
        let load = visual.load_heightmap_with_world_size(
            path.to_str().expect("utf8 path"),
            Some((200.0, 200.0)),
        );
        let _ = std::fs::remove_file(&path);
        load.expect("file heightmap should load");
        let loaded = visual.height_map.as_ref().expect("loaded");
        assert_eq!(loaded.scale, game_engine::map_object::MAP_XY_FACTOR);
        assert_eq!(loaded.border_size, 70);
    }
}
