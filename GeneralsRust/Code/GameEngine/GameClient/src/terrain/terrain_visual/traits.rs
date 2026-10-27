// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl Default for TerrainVisualImpl {
    fn default() -> Self {
        Self::new()
    }
}

impl SubsystemInterface for TerrainVisualImpl {
    fn init(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Initializing TerrainVisual subsystem");

        self.ensure_default_textures();

        // Initialize subsystems
        self.texture_system.init()?;
        self.water_system.init()?;
        self.road_system.init()?;
        self.chunk_manager.init()?;
        self.set_terrain_tracks_detail();

        Ok(())
    }

    fn reset(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        log::info!("Resetting TerrainVisual subsystem");

        self.filename.clear();
        self.skybox_enabled = false;
        self.missing_skybox_textures.clear();
        self.skybox_view_faces = (0, 0);
        self.dynamic_lights.clear();
        self.loaded_terrain_sources.clear();
        self.height_map = None;
        self.source_tile_classes.clear();
        self.ranked_source_tile_classes = None;

        self.reset_draw_area_state();
        self.seismic_simulations.clear();

        // Reset subsystems
        self.texture_system.reset()?;
        self.water_system.reset()?;
        self.road_system.reset()?;
        self.chunk_manager.reset()?;
        self.terrain_tracks.reset();
        self.chunk_meshes.clear();
        self.texture_rules.clear();
        self.chunk_texture_bindings.clear();
        self.active_chunk_texture_ids = None;
        self.terrain_sampler = None;
        self.terrain_sampler_mode = None;
        self.water_plane = None;
        self.water_track_meshes.clear();
        self.last_water_tracks_flush = crate::terrain::WaterTracksFlush::default();
        self.water_tracks = crate::terrain::WaterTracksRenderSystem::new(
            crate::terrain::DEFAULT_WATER_TRACK_MODULES,
        );
        self.water_named_bind_groups.clear();
        self.road_named_bind_groups.clear();
        self.river_gpu.bind_group = None;
        self.shroud_gpu.bind_group = None;
        self.shroud_gpu.dest_texture = None;
        self.shroud_gpu.dest_view = None;

        self.terrain_props.clear();
        self.construction_removals.clear();
        self.road_meshes.clear();
        self.bridge_meshes.clear();
        self.scorch_meshes.clear();
        // C++ BaseHeightMap.cpp:618 reset → clearAllScorches.
        self.scorches.clear();
        self.overlay_gpu_meshes_dirty = true;

        self.overlay = OverlayGpuState::default();
        self.shoreline_meshes.clear();
        self.water_grid_mesh = None;
        self.polygon_water_meshes.clear();
        self.bib_meshes.clear();
        self.tank_track_meshes.clear();
        self.custom_edge_meshes.clear();
        self.snow_mesh = None;
        self.smudge_mesh = None;
        self.flat_lod_meshes.clear();
        self.tree_meshes.clear();
        self.last_tree_gpu_vertices.clear();
        self.last_tree_atlas_mips.clear();
        self.tree_atlas_texture = None;
        self.tree_atlas_bind_group = None;
        self.tree_buffer.clear_all_trees();
        self.skybox_background_view = None;
        self.skybox_background_bind_group = None;
        self.restore_initial_skybox_textures()?;
        self.ensure_default_textures();

        self.stats = TerrainStats::default();

        Ok(())
    }

    fn update(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        self.update_with_motion(None, false)
    }
}

impl TerrainTrackHeightProvider for TerrainVisualImpl {
    fn ground_height_and_normal(&self, x: f32, y: f32) -> (f32, Vec3) {
        if let Some(heightmap) = self.height_map.as_ref() {
            (heightmap.get_height_at(x, y), heightmap.get_normal_at(x, y))
        } else {
            (0.0, Vec3::Z)
        }
    }
}

impl TerrainVisualImpl {
    /// Native Main submits Y-up matrices explicitly, including steep FX pitch.
    pub fn render_native_y_up(
        &mut self,
        view: &Mat4,
        projection: &Mat4,
        min_height: f32,
        snow: &crate::snow::SnowRenderState,
    ) -> Result<(), TerrainError> {
        self.set_skybox_view(view);
        let camera = snow_camera::SnowCamera::native_y_up(*view, *projection, min_height);
        self.render_with_snow_camera(view, projection, camera, Some(snow), None, false)
    }

    /// Actual Main frame supplies its receiving script and physics observations.
    pub fn render_native_y_up_with_motion(
        &mut self,
        view: &Mat4,
        projection: &Mat4,
        min_height: f32,
        snow: &crate::snow::SnowRenderState,
        motion: TerrainVisualMotion,
    ) -> Result<(), TerrainError> {
        self.set_skybox_view(view);
        let camera = snow_camera::SnowCamera::native_y_up(*view, *projection, min_height);
        self.render_with_snow_camera(view, projection, camera, Some(snow), Some(motion), false)
    }

    /// The native render driver has completed each original tree callback.
    /// Consume the same render algorithms without advancing tree CPU again.
    pub fn render_native_y_up_after_tree_cpu(
        &mut self,
        view: &Mat4,
        projection: &Mat4,
        min_height: f32,
        snow: &crate::snow::SnowRenderState,
        motion: TerrainVisualMotion,
        receipt: Option<&crate::terrain::TreeCpuDrawReceipt>,
    ) -> Result<(), TerrainError> {
        self.validate_owned_tree_draw(receipt)
            .map_err(|error| TerrainError::InvalidData(error.into()))?;
        self.set_skybox_view(view);
        let camera = snow_camera::SnowCamera::native_y_up(*view, *projection, min_height);
        self.render_with_snow_camera(view, projection, camera, Some(snow), Some(motion), true)
    }
    pub fn update_native_after_tree_cpu(
        &mut self,
        motion: TerrainVisualMotion,
        receipt: Option<&crate::terrain::TreeCpuDrawReceipt>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.validate_owned_tree_draw(receipt)
            .map_err(|error| Box::<dyn std::error::Error>::from(error))?;
        self.update_with_motion(Some(motion), true)
    }
    fn validate_owned_tree_draw(
        &self,
        receipt: Option<&crate::terrain::TreeCpuDrawReceipt>,
    ) -> Result<(), &'static str> {
        if !self.enabled {
            return Ok(());
        }
        self.tree_buffer.validate_cpu_draw_receipt(
            receipt.ok_or("enabled native tree draw requires its completion receipt")?,
        )
    }
    pub fn begin_owned_tree_cpu_draw(
        &mut self,
        motion: TerrainVisualMotion,
        rng: &mut impl crate::terrain::TreeRandom,
    ) -> Option<crate::terrain::TreeCpuDrawCursor> {
        if !self.enabled {
            return None;
        }
        self.rebind_skybox_background_for_camera();
        // Original drawTrees updates breeze before any toppling/bounce callback.
        if !motion.paused && self.tree_buffer.cur_sway_version() != motion.breeze.breeze_version {
            self.tree_buffer.update_sway(motion.breeze, rng);
            self.overlay.last_sway_version = motion.breeze.breeze_version;
        }
        Some(self.tree_buffer.begin_cpu_draw(motion.paused))
    }

    /// Receiving standalone Client keeps its historical source-axis inference.
    pub fn render_client_view(
        &mut self,
        view: &Mat4,
        projection: &Mat4,
        snow: &crate::snow::SnowRenderState,
    ) -> Result<(), TerrainError> {
        self.set_client_skybox_view(view);
        let camera =
            snow_camera::SnowCamera::from_client_view(*view, *projection, self.snow_min_height());
        self.render_with_snow_camera(view, projection, camera, Some(snow), None, false)
    }

    fn snow_min_height(&self) -> f32 {
        self.height_map
            .as_ref()
            .map(|map| map.min_height)
            .unwrap_or(0.)
    }

    fn render_with_snow_camera(
        &mut self,
        view_matrix: &Mat4,
        projection_matrix: &Mat4,
        snow_camera: Option<snow_camera::SnowCamera>,
        snow: Option<&crate::snow::SnowRenderState>,
        motion: Option<TerrainVisualMotion>,
        tree_cpu_completed: bool,
    ) -> Result<(), TerrainError> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(device) = self.device.as_ref().cloned() {
            self.sync_water_plane(device.as_ref())?;
            if self.water_texture_bind_group.is_none() || self.water_texture_is_fallback {
                self.ensure_water_texture_bind_group(device.as_ref());
            }
        }

        if tree_cpu_completed {
            self.update_tree_meshes_after_cpu(motion);
        } else {
            match motion {
                Some(motion) => self.update_tree_meshes_for_motion(motion),
                None => self.update_tree_meshes(),
            }
        }
        self.time += 1.0 / 30.0;

        let view_proj = *projection_matrix * *view_matrix;
        let camera_inverse = view_matrix.inverse();
        let camera_position = camera_inverse.transform_point3(Vec3::ZERO);
        if let Some(device) = self.device.clone() {
            if let Some(snow) = snow {
                self.ensure_snow_texture_bind_group_owned(
                    device.as_ref(),
                    &snow.definition.snow_texture,
                );
            }
            self.apply_water_transparency_map_overrides(device.as_ref());
        }
        self.upload_snow_mesh(snow_camera.as_ref(), snow);
        if self.overlay.overlays_dirty {
            self.rebuild_all_overlays();
        }
        self.recenter_draw_area_for_view(*view_matrix, *projection_matrix);
        self.chunk_manager.set_camera(camera_position);
        self.chunk_manager.set_view_frustum(ViewFrustum {
            planes: [Vec3::ZERO; 6],
            view_matrix: *view_matrix,
            projection_matrix: *projection_matrix,
        });
        // Visibility must see this frame's view/proj. Subsystem update() ran
        // earlier with a stale/identity frustum and left visible_chunks=0.
        let _ = self
            .chunk_manager
            .update_with_dynamic_lights(&self.dynamic_lights);

        // Update uniforms
        if let (Some(queue), Some(uniform_buffer)) = (self.queue.as_ref(), &self.uniform_buffer) {
            let uniforms = TerrainUniforms {
                view_proj: matrix4_to_array(&view_proj),
                view_matrix: matrix4_to_array(view_matrix),
                projection_matrix: matrix4_to_array(projection_matrix),
                camera_position: {
                    let global = game_engine::common::global_data::read();
                    let do_cloud = global.use_cloud_map
                        && global.time_of_day != game_engine::common::global_data::TimeOfDay::Night;
                    let do_noise = global.use_light_map;
                    let mode =
                        (if do_cloud { 1.0 } else { 0.0 }) + (if do_noise { 2.0 } else { 0.0 });
                    [
                        camera_position.x,
                        camera_position.y,
                        camera_position.z,
                        mode,
                    ]
                },
                time: self.time,
                _time_padding: [0.0; 3],
                fog_color: self.terrain_fog_uniforms().0,
                fog_range: self.terrain_fog_uniforms().1,
            };

            queue.write_buffer(uniform_buffer, 0, cast_slice(&[uniforms]));
        }

        // Render water
        self.water_system.render(view_matrix, projection_matrix)?;

        // Render roads
        self.road_system.render(view_matrix, projection_matrix)?;

        Ok(())
    }
}

impl TerrainVisual for TerrainVisualImpl {
    fn render(&mut self, view_matrix: &Mat4, projection_matrix: &Mat4) -> Result<(), TerrainError> {
        self.set_client_skybox_view(view_matrix);
        let camera = snow_camera::SnowCamera::from_client_view(
            *view_matrix,
            *projection_matrix,
            self.snow_min_height(),
        );
        let snow = crate::snow::SnowRenderState::from_standalone();
        self.render_with_snow_camera(
            view_matrix,
            projection_matrix,
            camera,
            snow.as_ref(),
            None,
            false,
        )
    }

    fn get_height_at(&self, x: f32, y: f32) -> Result<f32, TerrainError> {
        if let Some(ref heightmap) = self.height_map {
            Ok(heightmap.get_height_at(x, y))
        } else {
            Ok(0.0)
        }
    }

    fn get_normal_at(&self, x: f32, y: f32) -> Result<Vec3, TerrainError> {
        if let Some(ref heightmap) = self.height_map {
            Ok(heightmap.get_normal_at(x, y))
        } else {
            Ok(Vec3::new(0.0, 0.0, 1.0))
        }
    }

    fn is_valid_position(&self, x: f32, y: f32) -> bool {
        x >= 0.0 && y >= 0.0 && x < self.config.world_size.0 && y < self.config.world_size.1
    }

    fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    fn chunk_draw_count(&self) -> usize {
        self.chunk_draw_count()
    }

    fn oversize_terrain(&mut self, amount: i32) {
        TerrainVisualImpl::oversize_terrain(self, amount);
    }

    fn set_terrain_tracks_detail(&mut self) {
        TerrainVisualImpl::set_terrain_tracks_detail(self);
    }
}

/// Terrain uniform data for shaders
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TerrainUniforms {
    view_proj: [[f32; 4]; 4],
    view_matrix: [[f32; 4]; 4],
    projection_matrix: [[f32; 4]; 4],
    camera_position: [f32; 4],
    time: f32,
    _time_padding: [f32; 3],
    fog_color: [f32; 4],
    fog_range: [f32; 4],
}

impl TerrainVisualImpl {
    /// Immutable receiving inputs for the actual GPU Camera block.
    /// Other lighting is already baked into terrain vertex diffuse.
    fn terrain_fog_uniforms(&self) -> ([f32; 4], [f32; 4]) {
        (
            [self.fog_color[0], self.fog_color[1], self.fog_color[2], 1.0],
            [self.fog_start, self.fog_end, 0.0, 0.0],
        )
    }
}

#[cfg(test)]
#[path = "terrain_fog_uniform_tests.rs"]
mod terrain_fog_uniform_tests;

/// Shipped wgpu water overlay vertex. `packed_c` is C++ `SEA_PATCH_VERTEX.c`.
pub type WaterVertex = WaterGpuVertex;

/// Shipped wgpu overlay vertex (roads/bridges/scorch). Packed `diffuse` is C++ BGRA.
pub type RoadVertex = OverlayGpuVertex;

impl TreeGpuVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TreeGpuVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        }
    }
}

/// Immutable observations from the session driving this native frame.
#[derive(Debug, Clone, Copy)]
pub struct TerrainVisualMotion {
    pub paused: bool,
    pub breeze: crate::terrain::BreezeInfo,
    pub gravity: f32,
}
impl TerrainVisualImpl {
    pub fn update_native(
        &mut self,
        motion: TerrainVisualMotion,
    ) -> Result<(), Box<dyn std::error::Error>> {
        self.update_with_motion(Some(motion), false)
    }
    fn update_with_motion(
        &mut self,
        motion: Option<TerrainVisualMotion>,
        tree_cpu_completed: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if !self.enabled {
            return Ok(());
        }

        let update_started = std::time::Instant::now();

        // Update seismic simulations
        self.update_seismic_simulations();

        // Update subsystems
        let texture_started = std::time::Instant::now();
        self.texture_system.update()?;
        let texture_elapsed = texture_started.elapsed();

        let water_started = std::time::Instant::now();
        self.water_system.update()?;
        match motion {
            Some(motion) => self.simulate_water_grid_with_gravity(motion.gravity),
            None => self.simulate_water_grid(1.0 / 30.0),
        }
        // C++ WaterRenderObjClass::update (W3DWater.cpp:1201) accumulates
        // m_riverVOrigin without wrapping; the wave phases (sin(25v + x),
        // cos(11v)) would visibly pop on every wrap.
        self.overlay.river_v_origin += 0.002;
        // C++ rebuilds the trapezoid/river vertex buffers every draw with the
        // current phase; rebake so polygon water actually scrolls/wobbles.
        if let Some(device) = self.device.clone() {
            let _ = self.sync_polygon_water_meshes(device.as_ref());
        }
        self.overlay.cloud_map.update(1000.0 / 30.0);
        // C++ GameClient.cpp:560 is the only SnowManager::UPDATE; TerrainVisual
        // does not tick snow (GameClient.cpp:719-722).
        self.flush_water_tracks();
        if self.overlay.overlays_dirty || self.overlay.water_grid_dirty {
            self.rebuild_all_overlays();
        }
        if let Some(device) = self.device.clone() {
            if self.water_grid_enabled && self.overlay.water_grid_dirty {
                self.upload_water_grid_mesh(device.as_ref());
            }
            self.ensure_river_bind_group(device.as_ref());
        }
        self.sync_river_params();
        self.sync_shroud_dest_texture();
        let water_elapsed = water_started.elapsed();

        let road_started = std::time::Instant::now();
        self.road_system.update()?;
        if let Some(height_map) = self.height_map.as_ref() {
            if self.road_system.needs_terrain_normal_reprojection() {
                let lighting = self.chunk_manager.static_lighting();
                self.road_system.apply_terrain_heights_normals_and_diffuse(
                    |pos| height_map.get_height_at(pos.x, pos.z),
                    |pos| height_map.get_normal_at(pos.x, pos.z),
                    |pos| {
                        let normal = height_map.get_normal_at(pos.x, pos.z);
                        Self::terrain_static_diffuse_from_normal(
                            normal,
                            &lighting.lights,
                            lighting.ambient,
                        )
                    },
                );
                self.overlay_gpu_meshes_dirty = true;
            }
        }
        let road_elapsed = road_started.elapsed();

        let road_meshes_started = std::time::Instant::now();
        self.update_road_meshes()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        // C++ drawScorches calls updateScorches every frame; addScorch only
        // zeros m_scorchesInBuffer. Do not wait for overlay_gpu_meshes_dirty
        // (that rebuilds all roads).
        if self.scorches_need_gpu_rebuild() {
            if let Some(device) = self.device.clone() {
                self.update_scorch_meshes(&device);
            }
        }
        if tree_cpu_completed {
            self.update_tree_meshes_after_cpu(motion);
        } else {
            match motion {
                Some(motion) => self.update_tree_meshes_for_motion(motion),
                None => self.update_tree_meshes(),
            }
        }
        let road_meshes_elapsed = road_meshes_started.elapsed();

        let chunk_manager_started = std::time::Instant::now();
        self.chunk_manager
            .update_with_dynamic_lights(&self.dynamic_lights)?;
        let chunk_manager_elapsed = chunk_manager_started.elapsed();

        let chunk_meshes_started = std::time::Instant::now();
        self.update_chunk_meshes()
            .map_err(|e| Box::new(e) as Box<dyn std::error::Error>)?;
        let chunk_meshes_elapsed = chunk_meshes_started.elapsed();

        self.stats.update_time_ms =
            self.chunk_manager.get_stats().update_time.as_secs_f64() * 1000.0;

        let update_elapsed = update_started.elapsed();
        if update_elapsed >= std::time::Duration::from_millis(200) {
            warn!(
                "TerrainVisual::update breakdown: total={:?} texture={:?} water={:?} roads={:?} road_meshes={:?} chunk_manager={:?} chunk_meshes={:?} visible={} pending_visible={} total_chunks={}",
                update_elapsed,
                texture_elapsed,
                water_elapsed,
                road_elapsed,
                road_meshes_elapsed,
                chunk_manager_elapsed,
                chunk_meshes_elapsed,
                self.chunk_manager.get_visible_chunks().len(),
                self.chunk_manager.pending_visible_chunk_count(),
                self.chunk_manager.total_chunk_count()
            );
        }

        Ok(())
    }
}

#[cfg(test)]
mod native_motion_owner_tests {
    use super::*;
    fn water_owner() -> TerrainVisualImpl {
        let mut visual = TerrainVisualImpl::new();
        visual.set_water_grid_resolution(2.0, 2.0, 1.0);
        visual.set_water_transform(0.0, 0.0, 0.0, 0.0);
        visual.set_water_attenuation_factors(1.0, 0.0, 0.0, 10.0);
        visual.enable_water_grid(true);
        visual.add_water_velocity(0.0, 0.0, 4.0, 0.0);
        visual
    }
    #[test]
    fn actual_native_updates_keep_receiving_water_gravity_independent() {
        let mut first = water_owner();
        let mut second = water_owner();
        let motion = |gravity| TerrainVisualMotion {
            paused: false,
            gravity,
            breeze: Default::default(),
        };
        first.update_native(motion(-0.25)).unwrap();
        second.update_native(motion(-2.0)).unwrap();
        let a = first.get_water_grid_height(0.0, 0.0).unwrap();
        let b = second.get_water_grid_height(0.0, 0.0).unwrap();
        assert!((a - 2.97).abs() < 0.00001, "receiving gravity: {a}");
        assert!((b + 2.28).abs() < 0.00001, "foreign gravity: {b}");
        second.clear_scorches();
        drop(second);
        assert_eq!(first.get_water_grid_height(0.0, 0.0), Some(a));
    }
}
