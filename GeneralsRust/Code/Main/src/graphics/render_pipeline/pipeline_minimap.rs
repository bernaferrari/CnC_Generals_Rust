#![allow(
    unused_imports,
    unused_variables,
    dead_code,
    non_snake_case,
    unused_mut,
    unused_assignments,
    clippy::all
)]
use super::*;

impl RenderPipeline {
    /// Initialize minimap FOW texture renderer
    ///
    /// Creates the minimap texture renderer for displaying FOW on the minimap UI
    ///
    /// # Arguments
    ///
    /// * `device` - WGPU device
    /// * `queue` - WGPU queue
    /// * `world_bounds` - World coordinate bounds (min, max)
    pub fn initialize_minimap_renderer(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        world_bounds: (Vec3, Vec3),
    ) -> Result<()> {
        // Use default minimap dimensions (256x256)
        let dimensions = crate::graphics::minimap_renderer::MinimapDimensions::standard();

        let renderer = MinimapTextureRenderer::new(device, queue, dimensions, world_bounds)?;

        self.minimap_renderer = Some(renderer);
        self.minimap_base_needs_refresh = true;
        info!("Initialized minimap FOW texture renderer");
        Ok(())
    }

    /// Record an optional heightmap path hint to be consumed by the terrain subsystem when plumbed.
    pub fn set_heightmap_hint(&mut self, path: Option<String>) {
        self.pending_heightmap_hint_load = path.is_some();
        self.heightmap_path_hint = path;
    }

    /// Retrieve the current heightmap hint (if any).
    pub fn heightmap_hint(&self) -> Option<&str> {
        self.heightmap_path_hint.as_deref()
    }

    /// Record a skybox texture hint array.
    pub fn set_skybox_hint(&mut self, textures: [String; 5]) {
        self.skybox_textures_hint = Some(textures);
    }

    pub fn set_skybox_enabled(&mut self, enabled: bool) {
        self.skybox_enabled = enabled;
    }

    pub fn skybox_hint(&self) -> Option<&[String; 5]> {
        self.skybox_textures_hint.as_ref()
    }

    pub(super) fn resolved_skybox_hint(&self) -> [String; 5] {
        self.skybox_textures_hint
            .clone()
            .unwrap_or_else(|| DEFAULT_SKYBOX_TEXTURES.map(|name| name.to_string()))
    }

    pub(super) fn has_explicit_skybox_hint(&self) -> bool {
        self.skybox_textures_hint.is_some()
    }

    pub(super) fn terrain_clear_color(&self) -> wgpu::Color {
        // C++ W3DDisplay.cpp:1859 `WW3D::Begin_Render(true, true, Vector3(0,0,0), …)`
        // always clears BLACK, then sky+terrain draw. Uncovered pixels stay
        // black (or skybox), never map fog/sun peach.
        if std::env::var_os("GENERALS_DEBUG_CLEAR_COLOR").is_some() {
            return wgpu::Color {
                r: 0.0,
                g: 0.55,
                b: 0.0,
                a: 1.0,
            };
        }
        wgpu::Color::BLACK
    }

    /// Cache map lighting for terrain/sky consumers and push to terrain if ready.
    pub fn set_environment_lighting(
        &mut self,
        sun_direction: Option<[f32; 3]>,
        sun_color: Option<[f32; 3]>,
        ambient_color: Option<[f32; 3]>,
        fog_color: Option<[f32; 3]>,
        fog_range: Option<(f32, f32)>,
    ) {
        let lighting = CachedLighting {
            sun_direction,
            sun_color,
            ambient_color,
            fog_color,
            fog_range,
            fogged_light_fraction: None,
        };
        self.set_environment_lighting_with_terrain(Some(lighting.clone()), Some(lighting));
    }

    /// Install separately authored object-scene and terrain lighting.
    ///
    /// C++ `W3DDisplay::setTimeOfDay` reads `TerrainObjectsLighting[tod][0]`
    /// for the scene and `TerrainLighting[tod][0]` for TerrainVisual. Main's
    /// forward pass and GraphicsSystem consume the object record, while the
    /// terrain bridge consumes the terrain record.
    pub fn set_environment_lighting_with_terrain(
        &mut self,
        object_lighting: Option<CachedLighting>,
        terrain_lighting: Option<CachedLighting>,
    ) {
        if let Some(object_lighting) = object_lighting {
            self.cached_lighting = Some(object_lighting);
        }
        if let Some(terrain_lighting) = terrain_lighting {
            self.cached_terrain_lighting = Some(terrain_lighting.clone());
            self.apply_cached_lighting_to_terrain(&terrain_lighting);
        }
    }

    /// Clear any cached lighting state.
    pub fn clear_environment_lighting(&mut self) {
        self.cached_lighting = None;
        self.cached_terrain_lighting = None;
    }

    #[cfg(feature = "game_client")]
    pub(super) fn apply_cached_lighting_to_terrain(&self, lighting: &CachedLighting) {
        if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.set_lighting(
                    lighting.sun_direction,
                    lighting.sun_color,
                    lighting.ambient_color,
                    lighting.fog_color,
                    lighting.fog_range,
                );
            }
        }
    }

    #[cfg(not(feature = "game_client"))]
    pub(super) fn apply_cached_lighting_to_terrain(&self, _lighting: &CachedLighting) {}

    /// Attempt to load the heightmap hinted by the map metadata into the TerrainVisual singleton.
    pub fn load_heightmap_from_hint(
        &mut self,
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
        world_bounds: Option<(Vec3, Vec3)>,
    ) -> Result<()> {
        let Some(path) = self.heightmap_hint() else {
            return Ok(());
        };

        info!("Loading heightmap from map hint: {}", path);
        #[cfg(feature = "game_client")]
        {
            game_client::terrain::terrain_visual::init_terrain_visual()
                .map_err(|e| anyhow::anyhow!("Terrain visual init failed: {}", e))?;
            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    let explicit_world_size = world_bounds.map(|bounds| {
                        (
                            (bounds.1.x - bounds.0.x).abs().max(1.0),
                            (bounds.1.z - bounds.0.z).abs().max(1.0),
                        )
                    });
                    visual
                        .init_gpu_resources(device.clone(), queue.clone())
                        .map_err(|e| anyhow::anyhow!("Terrain GPU init failed: {}", e))?;
                    visual
                        .load_heightmap_with_world_size(path, explicit_world_size)
                        .map_err(|e| anyhow::anyhow!("Terrain heightmap load failed: {}", e))?;
                    self.heightmap_world_size = Some(visual.world_size());

                    // Apply skybox textures if provided.
                    if self.skybox_enabled {
                        let textures = self.resolved_skybox_hint();
                        let borrowed: [&str; 5] = [
                            textures[0].as_str(),
                            textures[1].as_str(),
                            textures[2].as_str(),
                            textures[3].as_str(),
                            textures[4].as_str(),
                        ];
                        if let Err(err) = visual.replace_skybox_textures(&[""; 5], &borrowed) {
                            if self.has_explicit_skybox_hint() {
                                warn!("Failed to apply skybox textures from map/defaults: {}", err);
                            } else {
                                debug!(
                                    "Skipping default skybox texture override because mounted assets do not expose the legacy fallback set: {}",
                                    err
                                );
                            }
                        }
                    }

                    self.pending_heightmap_hint_load = false;

                    // Push lighting into the terrain visual if available.
                    if let Some(lighting) = self
                        .cached_terrain_lighting
                        .as_ref()
                        .or(self.cached_lighting.as_ref())
                    {
                        visual.set_lighting(
                            lighting.sun_direction,
                            lighting.sun_color,
                            lighting.ambient_color,
                            lighting.fog_color,
                            lighting.fog_range,
                        );
                    }
                }
            }
        }
        #[cfg(not(feature = "game_client"))]
        {
            debug!("Terrain visual bridge disabled; skipping heightmap hint load.");
        }
        Ok(())
    }

    /// Load terrain visual data from already-parsed runtime terrain (C++ parity fallback when no hint path exists).
    pub fn load_heightmap_from_runtime_terrain(
        &mut self,
        device: &Arc<wgpu::Device>,
        queue: &Arc<wgpu::Queue>,
    ) -> Result<bool> {
        #[cfg(feature = "game_client")]
        {
            // Presentation-frozen heightmap only (no live GameLogic dual-read).
            let Some(pres) = self.presentation_frame.as_ref() else {
                return Ok(false);
            };
            let Some(frozen) = pres
                .world_env
                .runtime_heightmap
                .as_ref()
                .filter(|h| h.is_usable())
            else {
                return Ok(false);
            };
            let heightmap = frozen.to_height_map();
            let heightmap_resolution = (heightmap.width, heightmap.height);

            game_client::terrain::terrain_visual::init_terrain_visual()
                .map_err(|e| anyhow::anyhow!("Terrain visual init failed: {}", e))?;

            let source_hint_owned: Option<std::path::PathBuf> = pres
                .world_env
                .heightmap_hint
                .as_ref()
                .map(std::path::PathBuf::from);
            let source_hint_ref = source_hint_owned.as_deref();
            let world_bounds = pres.world_env.world_bounds_vec3();
            let world_size = (
                (world_bounds.1.x - world_bounds.0.x).abs().max(1.0),
                (world_bounds.1.z - world_bounds.0.z).abs().max(1.0),
            );

            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    visual
                        .init_gpu_resources(device.clone(), queue.clone())
                        .map_err(|e| anyhow::anyhow!("Terrain GPU init failed: {}", e))?;
                    visual
                        .load_heightmap_from_data(heightmap, source_hint_ref, Some(world_size))
                        .map_err(|e| {
                            anyhow::anyhow!("Terrain runtime heightmap load failed: {}", e)
                        })?;
                    let source_tile_classes: Vec<
                        game_client::terrain::terrain_visual::TerrainSourceTileClass,
                    > = pres
                        .world_env
                        .terrain_texture_classes
                        .iter()
                        .map(
                            |class| game_client::terrain::terrain_visual::TerrainSourceTileClass {
                                first_tile: class.first_tile,
                                num_tiles: class.num_tiles,
                                width: class.width,
                                name: class.name.clone(),
                            },
                        )
                        .collect();
                    if !source_tile_classes.is_empty() {
                        match visual.load_source_tiles_from_texture_classes(&source_tile_classes) {
                            Ok(loaded) => debug!(
                                "Loaded {} terrain source tiles from {} texture classes",
                                loaded,
                                source_tile_classes.len()
                            ),
                            Err(err) => warn!("Terrain source tile load failed: {}", err),
                        }
                    }
                    self.heightmap_world_size = Some(visual.world_size());
                    if self.skybox_enabled {
                        let textures = self.resolved_skybox_hint();
                        let borrowed: [&str; 5] = [
                            textures[0].as_str(),
                            textures[1].as_str(),
                            textures[2].as_str(),
                            textures[3].as_str(),
                            textures[4].as_str(),
                        ];
                        if let Err(err) = visual.replace_skybox_textures(&[""; 5], &borrowed) {
                            if self.has_explicit_skybox_hint() {
                                warn!(
                                    "Failed to apply skybox textures from runtime terrain: {}",
                                    err
                                );
                            } else {
                                debug!(
                                    "Skipping default skybox texture override because mounted assets do not expose the legacy fallback set: {}",
                                    err
                                );
                            }
                        }
                    }

                    self.pending_heightmap_hint_load = false;

                    if let Some(lighting) = self
                        .cached_terrain_lighting
                        .as_ref()
                        .or(self.cached_lighting.as_ref())
                    {
                        visual.set_lighting(
                            lighting.sun_direction,
                            lighting.sun_color,
                            lighting.ambient_color,
                            lighting.fog_color,
                            lighting.fog_range,
                        );
                    }

                    info!(
                        "Loaded terrain visual from presentation heightmap ({}x{}, world_size=({:.1}, {:.1}))",
                        heightmap_resolution.0, heightmap_resolution.1, world_size.0, world_size.1
                    );
                    return Ok(true);
                }
            }

            Ok(false)
        }

        #[cfg(not(feature = "game_client"))]
        {
            let _ = (device, queue);
            Ok(false)
        }
    }

    /// Sync map roads/bridges into the terrain-road render path.
    /// Prefers frozen `PresentationWorldEnv` road/bridge segments when present.
    pub fn sync_runtime_map_roads(&mut self) -> Result<()> {
        #[cfg(feature = "game_client")]
        {
            // When presentation is installed, roads/bridges are snapshot-owned even if
            // empty (fail-closed: no live dual-read mid-frame). Live GameLogic residual
            // only for boot/loading without a presentation frame.
            let (road_segments, bridge_segments) =
                if let Some(env) = self.presentation_frame.as_ref().map(|p| &p.world_env) {
                    let roads: Vec<game_client::terrain::terrain_visual::RuntimeRoadVisualSegment> =
                        env.road_segments
                            .iter()
                            .map(|segment| {
                                game_client::terrain::terrain_visual::RuntimeRoadVisualSegment {
                                    // Presentation stores [x,y,z]; visual wants [x,z,y] like live path.
                                    start: [segment.from[0], segment.from[2], segment.from[1]],
                                    end: [segment.to[0], segment.to[2], segment.to[1]],
                                    width: segment.width,
                                    template_name: segment.template_name.clone(),
                                    width_in_texture: segment.width_in_texture,
                                    road_type_id: segment.road_type_id,
                                    start_is_angled: segment.start_is_angled,
                                    start_is_join: segment.start_is_join,
                                    end_is_angled: segment.end_is_angled,
                                    end_is_join: segment.end_is_join,
                                    curve_radius: segment.curve_radius,
                                }
                            })
                            .collect();
                    let bridges: Vec<([f32; 3], [f32; 3], f32, String)> = env
                        .bridge_segments
                        .iter()
                        .map(|b| (b.start, b.end, b.width, b.template_name.clone()))
                        .collect();
                    (roads, bridges)
                } else {
                    return Ok(());
                };
            // Always push roads/bridges (even empty) so scorches bake like C++
            // HeightMapRenderObjClass::updateScorches — independent of road presence.

            if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_mut() {
                    visual
                        .set_runtime_map_road_segments(&road_segments, &bridge_segments)
                        .map_err(|e| anyhow::anyhow!("Terrain map-road sync failed: {}", e))?;
                }
            }
        }

        #[cfg(not(feature = "game_client"))]
        {}

        Ok(())
    }

    /// World size from the loaded heightmap, if available.
    pub fn heightmap_world_size(&self) -> Option<(f32, f32)> {
        self.heightmap_world_size
    }

    pub fn sync_heightmap_world_bounds(&mut self, world_bounds: (Vec3, Vec3)) {
        let width = (world_bounds.1.x - world_bounds.0.x).abs().max(1.0);
        let height = (world_bounds.1.z - world_bounds.0.z).abs().max(1.0);
        self.heightmap_world_size = Some((width, height));

        #[cfg(feature = "game_client")]
        if let Ok(mut guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
            if let Some(visual) = guard.as_mut() {
                visual.set_world_size(width, height);
            }
        }
    }

    /// Update minimap FOW texture
    ///
    /// Updates the minimap texture with FOW state. Prefer the presentation
    /// frame's frozen `fow_grid` when available so terrain/minimap overlay does
    /// not re-query the live shroud manager mid-render.
    pub fn update_minimap_fow_texture(&mut self) -> Result<()> {
        // Clone grid before mutably borrowing minimap_renderer (split-borrow).
        let grid = self
            .presentation_frame
            .as_ref()
            .map(|f| f.fow_grid().clone());
        let player_id = self.current_player_id as usize;
        let frame_number = self.frame_number;
        if let Some(ref mut minimap_renderer) = self.minimap_renderer {
            minimap_renderer.update_texture_from_fow_with_grid(
                player_id,
                frame_number,
                grid.as_ref(),
            )?;

            trace!(
                "Updated minimap FOW texture for player {} at frame {} (grid_active={})",
                player_id,
                frame_number,
                grid.as_ref().map(|g| g.active).unwrap_or(false)
            );
        }
        Ok(())
    }

    /// R8 terrain FOW overlay payload from the presentation snapshot (no live shroud).
    ///
    /// Feed into `FowTerrainOverlay::update_texture` when the GPU overlay is bound.
    /// Returns `None` when inactive / fail-open (skip overlay upload).
    pub fn presentation_terrain_fow_r8(&self) -> Option<Vec<u8>> {
        self.presentation_frame
            .as_ref()
            .and_then(|f| f.terrain_fow_r8())
    }

    /// Pack presentation laser Line3D segments into a CPU vertex buffer for WGPU.
    ///
    /// Pack itself is CPU-only; `RenderPipeline::execute` then calls
    /// `ForwardPass::upload_laser_segments` → `Queue::write_buffer`.

    pub fn debug_last_laser_segments_packed(&self) -> u32 {
        self.debug_last_laser_segments_packed
    }

    pub fn debug_last_laser_pack_ok(&self) -> bool {
        self.debug_last_laser_pack_ok
    }

    pub fn debug_last_laser_gpu_write_ok(&self) -> bool {
        self.debug_last_laser_gpu_write_ok
    }

    pub fn debug_last_projectile_segments_packed(&self) -> u32 {
        self.debug_last_projectile_segments_packed
    }

    pub fn debug_last_projectile_pack_ok(&self) -> bool {
        self.debug_last_projectile_pack_ok
    }

    pub fn debug_last_move_lines_packed(&self) -> u32 {
        self.debug_last_move_lines_packed
    }

    pub fn debug_last_attack_lines_packed(&self) -> u32 {
        self.debug_last_attack_lines_packed
    }

    pub fn debug_last_floating_texts_packed(&self) -> u32 {
        self.debug_last_floating_texts_packed
    }

    pub fn debug_last_floating_text_pack_ok(&self) -> bool {
        self.debug_last_floating_text_pack_ok
    }

    pub fn debug_last_world_anims_packed(&self) -> u32 {
        self.debug_last_world_anims_packed
    }

    pub fn debug_last_world_anim_pack_ok(&self) -> bool {
        self.debug_last_world_anim_pack_ok
    }

    pub fn debug_last_particle_systems_packed(&self) -> u32 {
        self.debug_last_particle_systems_packed
    }

    pub fn debug_last_particle_pack_ok(&self) -> bool {
        self.debug_last_particle_pack_ok
    }

    /// Pack presentation floating-text captions into CPU layout (no live GameLogic).
    pub fn pack_presentation_floating_texts(
        &self,
    ) -> crate::graphics::floating_text_layout::FloatingTextLayout {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::floating_text_layout::FloatingTextLayout::pack_from_presentation(
                    frame,
                )
            }
            None => crate::graphics::floating_text_layout::FloatingTextLayout::empty(),
        }
    }

    /// Pack presentation world-anim (MoneyPickUp) samples into CPU layout.
    pub fn pack_presentation_world_anims(
        &self,
    ) -> crate::graphics::world_anim_layout::WorldAnimLayout {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::world_anim_layout::WorldAnimLayout::pack_from_presentation(frame)
            }
            None => crate::graphics::world_anim_layout::WorldAnimLayout::empty(),
        }
    }

    /// Pack presentation combat particle systems into CPU layout (no live GameLogic).
    pub fn pack_presentation_particle_systems(
        &self,
    ) -> crate::graphics::particle_system_upload::ParticleSystemUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::particle_system_upload::pack_and_mark_upload_ready(frame)
            }
            None => crate::graphics::particle_system_upload::ParticleSystemUpload::empty(),
        }
    }

    /// Pack presentation laser Line3D segments into CPU buffer (no live GameLogic).
    /// so SegLine upload does not re-read live GameLogic mid-render.
    pub fn pack_presentation_laser_segments(
        &self,
    ) -> crate::graphics::laser_segment_upload::LaserSegmentUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => crate::graphics::laser_segment_upload::pack_and_mark_upload_ready(frame),
            None => crate::graphics::laser_segment_upload::LaserSegmentUpload::empty(),
        }
    }

    /// Get minimap texture ID for UI rendering.
    /// Pack presentation projectiles into CPU trail buffer (no live GameLogic).
    pub fn pack_presentation_projectiles(
        &self,
    ) -> crate::graphics::projectile_segment_upload::ProjectileSegmentUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::pack_from_presentation(
                    frame,
                )
            }
            None => crate::graphics::projectile_segment_upload::ProjectileSegmentUpload::empty(),
        }
    }

    /// Pack presentation move-order lines into CPU buffer (no live GameLogic).
    pub fn pack_presentation_move_lines(
        &self,
    ) -> crate::graphics::move_line_upload::MoveLineUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::move_line_upload::MoveLineUpload::pack_from_presentation(frame)
            }
            None => crate::graphics::move_line_upload::MoveLineUpload::empty(),
        }
    }

    /// Pack presentation attack-order lines into CPU buffer (no live GameLogic).
    pub fn pack_presentation_attack_lines(
        &self,
    ) -> crate::graphics::attack_line_upload::AttackLineUpload {
        match self.presentation_frame.as_ref() {
            Some(frame) => {
                crate::graphics::attack_line_upload::AttackLineUpload::pack_from_presentation(frame)
            }
            None => crate::graphics::attack_line_upload::AttackLineUpload::empty(),
        }
    }

    pub fn get_minimap_texture_id(&self) -> Option<UiTextureId> {
        self.minimap_renderer.as_ref()?.get_texture_id()
    }

    /// Get minimap coordinates for click handling
    pub fn get_minimap_coordinates(&self) -> Option<&MinimapCoordinates> {
        self.minimap_renderer.as_ref().map(|r| r.get_coordinates())
    }

    /// Update minimap coordinate mapping after world bounds change.
    pub fn update_minimap_world_bounds(&mut self, world_bounds: (Vec3, Vec3)) {
        if let Some(renderer) = self.minimap_renderer.as_mut() {
            renderer.set_world_bounds(world_bounds);
            self.minimap_base_needs_refresh = true;
        }
    }

    /// Inform the minimap renderer about the latest on-screen rectangle.
    pub fn update_minimap_screen_rect(&mut self, top_left: Vec2, size: Vec2) {
        if let Some(renderer) = self.minimap_renderer.as_mut() {
            renderer.set_screen_rect(top_left, size);
        }
    }

    pub(super) fn refresh_minimap_terrain_base(&mut self) -> Result<()> {
        let Some(renderer) = self.minimap_renderer.as_mut() else {
            return Ok(());
        };
        if !self.minimap_base_needs_refresh {
            return Ok(());
        }

        let dimensions = renderer.dimensions();
        // Prefer presentation-owned bounds + coarse height grid (no live height re-sample).
        let (bounds, height_env) = if let Some(pres) = self.presentation_frame.as_ref() {
            (
                Some(pres.world_env.world_bounds_vec3()),
                Some(&pres.world_env),
            )
        } else {
            (None, None)
        };
        let base_texture = Self::build_minimap_terrain_base_texture(dimensions, bounds, height_env);
        renderer.set_base_terrain_texture(base_texture)?;
        self.minimap_base_needs_refresh = false;
        Ok(())
    }

    pub(super) fn build_minimap_terrain_base_texture(
        dimensions: MinimapDimensions,
        bounds_override: Option<(Vec3, Vec3)>,
        height_env: Option<&crate::presentation_frame::PresentationWorldEnv>,
    ) -> Vec<u8> {
        let width = dimensions.width.max(1);
        let height = dimensions.height.max(1);
        let pixel_count = (width * height) as usize;
        let mut heights = vec![0.0f32; pixel_count];
        let mut has_sample = false;

        let (world_min, world_max) = bounds_override
            .unwrap_or((Vec3::new(-500.0, 0.0, -500.0), Vec3::new(500.0, 0.0, 500.0)));
        let world_span_x = (world_max.x - world_min.x).max(1.0);
        let world_span_z = (world_max.z - world_min.z).max(1.0);

        let idx = |x: u32, y: u32| -> usize { (y * width + x) as usize };

        let use_pres_heights = height_env
            .map(|e| e.height_samples_from_terrain && !e.height_samples.is_empty())
            .unwrap_or(false);

        for y in 0..height {
            for x in 0..width {
                let u = (x as f32 + 0.5) / width as f32;
                let v = (y as f32 + 0.5) / height as f32;
                let world = Vec3::new(
                    world_min.x + u * world_span_x,
                    0.0,
                    world_min.z + v * world_span_z,
                );
                let sample = if height_env.is_some() {
                    // Presentation installed: use coarse grid only (None sample = empty
                    // cell). Do not dual-read live terrain_height_at.
                    if use_pres_heights {
                        height_env.and_then(|e| e.sample_height(world.x, world.z))
                    } else {
                        None
                    }
                } else {
                    // Boot/loading without presentation: live residual.
                    None
                };
                if let Some(h) = sample {
                    heights[idx(x, y)] = h;
                    has_sample = true;
                }
            }
        }

        if !has_sample {
            return vec![255u8; pixel_count * 4];
        }

        let (mut min_h, mut max_h) = (f32::MAX, f32::MIN);
        for h in &heights {
            min_h = min_h.min(*h);
            max_h = max_h.max(*h);
        }
        let range_h = (max_h - min_h).max(1.0);
        let waterline = min_h + range_h * 0.14;
        let light_dir = Vec3::new(0.45, 0.70, 0.55).normalize();

        let mut texture = vec![0u8; pixel_count * 4];
        for y in 0..height {
            for x in 0..width {
                let x0 = x.saturating_sub(1);
                let x1 = (x + 1).min(width - 1);
                let y0 = y.saturating_sub(1);
                let y1 = (y + 1).min(height - 1);
                let h = heights[idx(x, y)];
                let left = heights[idx(x0, y)];
                let right = heights[idx(x1, y)];
                let up = heights[idx(x, y0)];
                let down = heights[idx(x, y1)];

                let dx = (right - left) / range_h;
                let dz = (down - up) / range_h;
                let normal = Vec3::new(-dx, 1.0, -dz).normalize_or_zero();
                let shade = normal.dot(light_dir).clamp(0.2, 1.0);

                let elevation = ((h - min_h) / range_h).clamp(0.0, 1.0);
                let mut r = 48.0 + (201.0 - 48.0) * elevation;
                let mut g = 62.0 + (177.0 - 62.0) * elevation;
                let mut b = 44.0 + (128.0 - 44.0) * elevation;

                if h <= waterline {
                    let t = ((waterline - h) / range_h / 0.14).clamp(0.0, 1.0);
                    r = r * (1.0 - 0.55 * t) + 55.0 * 0.55 * t;
                    g = g * (1.0 - 0.55 * t) + 92.0 * 0.55 * t;
                    b = b * (1.0 - 0.55 * t) + 140.0 * 0.55 * t;
                }

                let base = idx(x, y) * 4;
                texture[base] = (r * shade).clamp(0.0, 255.0) as u8;
                texture[base + 1] = (g * shade).clamp(0.0, 255.0) as u8;
                texture[base + 2] = (b * shade).clamp(0.0, 255.0) as u8;
                texture[base + 3] = 255;
            }
        }

        #[cfg(feature = "game_client")]
        {
            if let Ok(guard) = game_client::terrain::terrain_visual::get_terrain_visual() {
                if let Some(visual) = guard.as_ref() {
                    let samples = visual.minimap_road_samples(10);
                    let span_norm = world_span_x.max(world_span_z).max(1.0);

                    for sample in samples {
                        let nx = ((sample.position.x - world_min.x) / world_span_x).clamp(0.0, 1.0);
                        let nz = ((sample.position.z - world_min.z) / world_span_z).clamp(0.0, 1.0);
                        let cx = (nx * (width - 1) as f32).round() as i32;
                        let cy = (nz * (height - 1) as f32).round() as i32;
                        let radius = ((sample.width / span_norm) * width.max(height) as f32 * 0.55)
                            .clamp(1.0, 4.0) as i32;
                        let blend =
                            (0.30 + (sample.width / 14.0).clamp(0.0, 0.28)).clamp(0.22, 0.60);
                        Self::paint_minimap_circle(
                            &mut texture,
                            width,
                            height,
                            cx,
                            cy,
                            radius,
                            sample.tint_rgb,
                            blend,
                        );
                    }
                }
            }
        }

        texture
    }

    /// Handle minimap click - convert to world position
    ///
    /// # Arguments
    ///
    /// * `screen_pos` - Screen position of the click
    ///
    /// # Returns
    ///
    /// World position if click was on minimap and area is visible
    pub fn handle_minimap_click(&self, screen_pos: Vec2) -> Option<Vec3> {
        let minimap_renderer = self.minimap_renderer.as_ref()?;
        minimap_renderer.screen_to_world(screen_pos)
    }

    /// Bind minimap texture to the active UI renderer.
    ///
    /// Makes the minimap texture available for UI rendering.
    ///
    /// # Arguments
    ///
    /// * `renderer` - UI texture registrar/renderer
    pub fn bind_minimap_texture_to_ui<T: UiTextureRegistrar>(
        &mut self,
        renderer: &mut T,
    ) -> Result<UiTextureId> {
        if let Some(ref mut minimap_renderer) = self.minimap_renderer {
            minimap_renderer.bind_to_ui_renderer(renderer)
        } else {
            Err(anyhow::anyhow!("Minimap renderer not initialized"))
        }
    }

    /// Ensure the minimap texture is registered with the active UI renderer.
    pub fn ensure_minimap_texture_bound<T: UiTextureRegistrar>(
        &mut self,
        renderer: &mut T,
    ) -> Result<()> {
        if let Some(ref mut minimap_renderer) = self.minimap_renderer {
            if minimap_renderer.get_texture_id().is_none() {
                minimap_renderer.bind_to_ui_renderer(renderer)?;
            }
            Ok(())
        } else {
            Err(anyhow::anyhow!("Minimap renderer not initialized"))
        }
    }

    /// Schedule a callback to run after the WW3D renderer finishes its main passes.
    pub fn enqueue_post_frame_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.forward_pass.enqueue_post_frame_callback(callback);
    }

    pub fn enqueue_pre_scene_callback<F>(&mut self, callback: F)
    where
        F: FnOnce(&mut ww3d_engine::RenderFrame) -> RendererResult<()> + Send + 'static,
    {
        self.forward_pass.enqueue_pre_scene_callback(callback);
    }
}
