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
        self.loaded_terrain_sources.clear();
        self.height_map = None;
        self.source_tile_classes.clear();

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
        crate::terrain::clear_terrain_scorches();
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
        self.simulate_water_grid(1.0 / 30.0);
        self.overlay.river_v_origin = (self.overlay.river_v_origin + 0.002) % 1.0;
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
                let light_pos = self.sun_direction;
                let sun_color = self.sun_color;
                let ambient_color = self.ambient_color;
                self.road_system.apply_terrain_heights_normals_and_diffuse(
                    |pos| height_map.get_height_at(pos.x, pos.z),
                    |pos| height_map.get_normal_at(pos.x, pos.z),
                    |pos| {
                        let normal = height_map.get_normal_at(pos.x, pos.z);
                        Self::terrain_static_diffuse_from_normal(
                            normal,
                            light_pos,
                            sun_color,
                            ambient_color,
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
        self.update_tree_meshes();
        let road_meshes_elapsed = road_meshes_started.elapsed();

        let chunk_manager_started = std::time::Instant::now();
        self.chunk_manager.update()?;
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

impl TerrainTrackHeightProvider for TerrainVisualImpl {
    fn ground_height_and_normal(&self, x: f32, y: f32) -> (f32, Vec3) {
        if let Some(heightmap) = self.height_map.as_ref() {
            (heightmap.get_height_at(x, y), heightmap.get_normal_at(x, y))
        } else {
            (0.0, Vec3::Z)
        }
    }
}

impl TerrainVisual for TerrainVisualImpl {
    fn render(&mut self, view_matrix: &Mat4, projection_matrix: &Mat4) -> Result<(), TerrainError> {
        if !self.enabled {
            return Ok(());
        }

        if let Some(device) = self.device.as_ref().cloned() {
            self.sync_global_water_plane(device.as_ref())?;
            if self.water_texture_bind_group.is_none() || self.water_texture_is_fallback {
                self.ensure_water_texture_bind_group(device.as_ref());
            }
        }

        self.update_tree_meshes();
        self.time += 1.0 / 30.0;

        let view_proj = *projection_matrix * *view_matrix;
        let camera_inverse = view_matrix.inverse();
        let camera_position = camera_inverse.transform_point3(Vec3::ZERO);
        if let Some(device) = self.device.clone() {
            self.ensure_snow_texture_bind_group(device.as_ref());
            self.apply_water_transparency_map_overrides(device.as_ref());
        }
        self.upload_snow_mesh(camera_position, view_matrix);
        if self.overlay.overlays_dirty {
            self.rebuild_all_overlays();
        }
        self.recenter_draw_area_on_world_position(camera_position.x, camera_position.z);
        self.chunk_manager.set_camera(camera_position);
        self.chunk_manager.set_view_frustum(ViewFrustum {
            planes: [Vec3::ZERO; 6],
            view_matrix: *view_matrix,
            projection_matrix: *projection_matrix,
        });
        // Visibility must see this frame's view/proj. Subsystem update() ran
        // earlier with a stale/identity frustum and left visible_chunks=0.
        let _ = self.chunk_manager.update();


        // Update uniforms
        if let (Some(queue), Some(uniform_buffer)) = (self.queue.as_ref(), &self.uniform_buffer) {
            let uniforms = TerrainUniforms {
                view_proj: matrix4_to_array(&view_proj),
                view_matrix: matrix4_to_array(view_matrix),
                projection_matrix: matrix4_to_array(projection_matrix),
                camera_position: {
                    let global = game_engine::common::global_data::read();
                    let do_cloud = global.use_cloud_map
                        && global.time_of_day
                            != game_engine::common::global_data::TimeOfDay::Night;
                    let do_noise = global.use_light_map;
                    let mode = (if do_cloud { 1.0 } else { 0.0 })
                        + (if do_noise { 2.0 } else { 0.0 });
                    [camera_position.x, camera_position.y, camera_position.z, mode]
                },
                time: self.time,
                sun_direction: self.sun_direction.to_array(),
                sun_color: self.sun_color,
                ambient_color: self.ambient_color,
                fog_color: self.fog_color,
                fog_start: self.fog_start,
                fog_end: self.fog_end,
                _padding: [0.0; 2],
            };

            queue.write_buffer(uniform_buffer, 0, cast_slice(&[uniforms]));
        }

        // Render water
        self.water_system.render(view_matrix, projection_matrix)?;

        // Render roads
        self.road_system.render(view_matrix, projection_matrix)?;

        Ok(())
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
#[derive(Debug, Copy, Clone)]
struct TerrainUniforms {
    view_proj: [[f32; 4]; 4],
    view_matrix: [[f32; 4]; 4],
    projection_matrix: [[f32; 4]; 4],
    camera_position: [f32; 4],
    time: f32,
    sun_direction: [f32; 3],
    sun_color: [f32; 3],
    ambient_color: [f32; 3],
    fog_color: [f32; 3],
    fog_start: f32,
    fog_end: f32,
    _padding: [f32; 2],
}

// SAFETY: `#[repr(C)]` uniform block of matrices/vectors/f32s with explicit
// SAFETY: _padding; uploaded to wgpu as raw bytes, never reinterpreted otherwise.
unsafe impl bytemuck::Pod for TerrainUniforms {}
// SAFETY: Zero bits are valid uniform values (identity-adjacent defaults).
unsafe impl bytemuck::Zeroable for TerrainUniforms {}

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
