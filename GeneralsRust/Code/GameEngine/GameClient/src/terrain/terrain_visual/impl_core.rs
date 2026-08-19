// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl TerrainVisualImpl {
    pub fn new() -> Self {
        Self {
            config: TerrainConfig::default(),
            stats: TerrainStats::default(),
            enabled: true,
            lod_setting: TerrainVisualLOD::default(),
            filename: String::new(),
            loaded_terrain_sources: Vec::new(),
            height_map: None,
            chunk_manager: ChunkManager::new(),
            texture_system: TerrainTextures::new(),
            source_tiles: vec![None; NUM_SOURCE_TILES],
            water_system: WaterSystem::new(),
            road_system: RoadSystem::new(),
            terrain_tracks: TerrainTracksRenderObjClassSystem::new(Self::terrain_tracks_config()),
            water_tracks: crate::terrain::WaterTracksRenderSystem::new(
                crate::terrain::DEFAULT_WATER_TRACK_MODULES,
            ),
            last_water_tracks_flush: crate::terrain::WaterTracksFlush::default(),

            device: None,
            queue: None,
            uniform_buffer: None,
            terrain_pipeline: None,
            terrain_depth_pipeline: None,
            water_pipeline: None,
            road_pipeline: None,
            tree_pipeline: None,
            heightmap_texture: None,
            blend_texture: None,
            detail_textures: Vec::new(),
            skybox_textures: [None, None, None, None, None],
            initial_skybox_texture_names: [None, None, None, None, None],
            current_skybox_texture_names: [None, None, None, None, None],
            skybox_background_view: None,
            skybox_background_bind_group: None,
            skybox_background_pipeline: None,
            skybox_background_bind_group_layout: None,
            skybox_sampler: None,
            last_skybox_face_bind: None,
            seismic_simulations: Vec::new(),
            water_grid_enabled: false,
            grid_water_handle: WaterHandle(0),
            water_grid: WaterGridCpuState::default(),
            terrain_bibs: Vec::new(),
            terrain_props: Vec::new(),
            construction_removals: Vec::new(),
            chunk_meshes: HashMap::new(),
            texture_rules: Vec::new(),
            water_plane: None,
            water_track_meshes: Vec::new(),
            road_meshes: Vec::new(),
            bridge_meshes: Vec::new(),
            scorch_meshes: Vec::new(),

            tree_buffer: W3DTreeBuffer::new(),
            last_tree_gpu_vertices: Vec::new(),
            last_tree_atlas_mips: Vec::new(),
            tree_meshes: Vec::new(),
            tree_atlas_texture: None,
            terrain_camera_bind_group_layout: None,
            terrain_texture_bind_group_layout: None,
            terrain_camera_bind_group: None,
            terrain_sampler: None,
            terrain_sampler_mode: None,
            chunk_texture_bindings: HashMap::new(),
            active_chunk_texture_ids: None,
            sun_direction: Vec3::new(0.0, -1.0, 0.0),
            sun_color: [1.0, 0.9, 0.8],
            ambient_color: [0.2, 0.2, 0.2],
            fog_color: [0.5, 0.6, 0.7],
            // C++ SceneClass: FogEnabled(false), FogStart(0), FogEnd(1000).
            // A degenerate span (end <= start) keeps the shader from
            // distance-fogging every fragment when live lighting has
            // fog_range=None.
            fog_start: 0.0,
            fog_end: 0.0,
            time: 0.0,
            oversize_amount: 0,
            draw_width: NORMAL_DRAW_WIDTH,
            draw_height: NORMAL_DRAW_HEIGHT,
            draw_origin_x: 0,
            draw_origin_y: 0,
            extra_blend_tile_positions: Vec::new(),
            extra_blend_gpu_upload: ExtraBlendGpuUpload::default(),
            extra_blend_draw_mesh: ExtraBlendDrawMesh::default(),
            extra_blend_position_buffer: None,
            extra_blend_vertex_buffer: None,
            extra_blend_index_buffer: None,
            extra_blend_index_count: 0,
            extra_blend_vertex_count: 0,
            extra_blend_pipeline: None,
            extra_blend_draw_count: AtomicU32::new(0),
        }
    }

    fn terrain_tracks_config() -> TerrainTracksConfig {
        let from_values = |max_terrain_tracks: i32,
                           max_tank_track_edges: i32,
                           max_tank_track_opaque_edges: i32,
                           max_tank_track_fade_delay: i32,
                           make_track_marks: bool| {
            let defaults = TerrainTracksConfig::default();
            TerrainTracksConfig {
                max_terrain_tracks: positive_usize(max_terrain_tracks)
                    .unwrap_or(defaults.max_terrain_tracks),
                max_tank_track_edges: positive_usize(max_tank_track_edges)
                    .unwrap_or(defaults.max_tank_track_edges),
                max_tank_track_opaque_edges: positive_usize(max_tank_track_opaque_edges)
                    .unwrap_or(defaults.max_tank_track_opaque_edges),
                max_tank_track_fade_delay: if max_tank_track_fade_delay > 0 {
                    max_tank_track_fade_delay
                } else {
                    defaults.max_tank_track_fade_delay
                },
                make_track_marks,
            }
        };

        if let Some(global_data) = get_global_data() {
            let data = global_data.read();
            return from_values(
                data.max_terrain_tracks,
                data.max_tank_track_edges,
                data.max_tank_track_opaque_edges,
                data.max_tank_track_fade_delay,
                data.make_track_marks,
            );
        }

        let data = global_data::read();
        from_values(
            data.max_terrain_tracks,
            data.max_tank_track_edges,
            data.max_tank_track_opaque_edges,
            data.max_tank_track_fade_delay,
            data.make_track_marks,
        )
    }

    /// Expose chunk manager for renderer passes.
    pub fn chunk_manager(&self) -> &ChunkManager {
        &self.chunk_manager
    }

    /// Number of visible chunks; used to accumulate draw-call stats.
    pub fn chunk_draw_count(&self) -> usize {
        self.visible_chunk_ids_for_draw_area().len()
    }

    /// Apply texture-LOD side effects immediately after a runtime LOD adjustment.
    ///
    /// Matches the intent of C++ `TheTerrainRenderObject->setTextureLOD(...)` called from
    /// `W3DGameClient::adjustLOD`.
    pub fn apply_texture_lod_reduction(&mut self, _reduction: i32) {
        self.terrain_sampler = None;
        self.terrain_sampler_mode = None;
        self.chunk_texture_bindings.clear();
    }

    fn map_sample_dimensions(&self) -> Option<(i32, i32)> {
        self.height_map
            .as_ref()
            .map(|height_map| (height_map.width as i32, height_map.height as i32))
    }

    fn map_scale(&self) -> f32 {
        self.map_sample_dimensions()
            .map(|(width, _height)| {
                (self.config.world_size.0 / width.max(1) as f32).max(f32::EPSILON)
            })
            .or_else(|| {
                self.height_map
                    .as_ref()
                    .map(|height_map| height_map.scale.max(f32::EPSILON))
            })
            .unwrap_or(1.0)
    }

    fn reset_draw_area_state(&mut self) {
        self.oversize_amount = 0;

        if let Some((map_width, map_height)) = self.map_sample_dimensions() {
            self.draw_width = NORMAL_DRAW_WIDTH.min(map_width).max(1);
            self.draw_height = NORMAL_DRAW_HEIGHT.min(map_height).max(1);
            self.draw_origin_x = ((map_width - self.draw_width) / 2).max(0);
            self.draw_origin_y = ((map_height - self.draw_height) / 2).max(0);
        } else {
            self.draw_width = NORMAL_DRAW_WIDTH;
            self.draw_height = NORMAL_DRAW_HEIGHT;
            self.draw_origin_x = 0;
            self.draw_origin_y = 0;
        }

        self.clamp_draw_area_to_map();
    }

    fn recenter_draw_area_on_world_position(&mut self, world_x: f32, world_z: f32) {
        let Some((_map_width, _map_height)) = self.map_sample_dimensions() else {
            return;
        };

        let scale = self.map_scale().max(f32::EPSILON);
        let sample_x = (world_x / scale).floor() as i32;
        let sample_y = (world_z / scale).floor() as i32;
        self.draw_origin_x = sample_x - (self.draw_width / 2);
        self.draw_origin_y = sample_y - (self.draw_height / 2);
        self.clamp_draw_area_to_map();
    }

    fn clamp_draw_area_to_map(&mut self) {
        if let Some((map_width, map_height)) = self.map_sample_dimensions() {
            if map_width > 0 && map_height > 0 {
                self.draw_width = self
                    .draw_width
                    .clamp(0, map_width)
                    .max(1)
                    .min(map_width.max(1));
                self.draw_height = self
                    .draw_height
                    .clamp(0, map_height)
                    .max(1)
                    .min(map_height.max(1));

                let max_origin_x = (map_width - self.draw_width).max(0);
                let max_origin_y = (map_height - self.draw_height).max(0);
                if self.draw_origin_x < 0 {
                    self.draw_origin_x = 0;
                }
                if self.draw_origin_y < 0 {
                    self.draw_origin_y = 0;
                }
                if self.draw_origin_x > max_origin_x {
                    self.draw_origin_x = max_origin_x;
                }
                if self.draw_origin_y > max_origin_y {
                    self.draw_origin_y = max_origin_y;
                }
            }
        }
    }

    fn draw_area_bounds_world(&self) -> (f32, f32, f32, f32) {
        let Some((map_width, map_height)) = self.map_sample_dimensions() else {
            return (0.0, 0.0, self.config.world_size.0, self.config.world_size.1);
        };

        let scale = self.map_scale();
        let width = (map_width.max(1) as f32) * scale;
        let height = (map_height.max(1) as f32) * scale;

        let origin_x = (self.draw_origin_x.max(0) as f32) * self.map_scale();
        let origin_y = (self.draw_origin_y.max(0) as f32) * self.map_scale();
        let max_x = (((self.draw_origin_x + self.draw_width).max(self.draw_origin_x) as f32)
            * scale)
            .max(0.0)
            .min(width);
        let max_y = (((self.draw_origin_y + self.draw_height).max(self.draw_origin_y) as f32)
            * scale)
            .max(0.0)
            .min(height);
        (origin_x, origin_y, max_x, max_y)
    }

    fn chunk_intersects_draw_area(&self, chunk: &crate::terrain::chunk::TerrainChunk) -> bool {
        let (min_x, min_y, max_x, max_y) = self.draw_area_bounds_world();
        chunk.bounds.max.x > min_x
            && chunk.bounds.min.x < max_x
            && chunk.bounds.max.z > min_y
            && chunk.bounds.min.z < max_y
    }

    fn visible_chunk_ids_for_draw_area(&self) -> Vec<ChunkId> {
        let chunks = self.chunk_manager.get_visible_chunks();
        let mut chunk_ids: Vec<ChunkId> = match self.map_sample_dimensions() {
            Some(_) => chunks
                .into_iter()
                .filter(|chunk| self.chunk_intersects_draw_area(chunk))
                .map(|chunk| chunk.id)
                .collect(),
            None => chunks.into_iter().map(|chunk| chunk.id).collect(),
        };
        chunk_ids.sort_unstable();
        chunk_ids
    }

    /// Current world size in world units.
    pub fn world_size(&self) -> (f32, f32) {
        self.config.world_size
    }

    pub fn set_world_size(&mut self, width: f32, height: f32) {
        self.config.world_size = (width.max(1.0), height.max(1.0));
        self.chunk_manager.set_config(self.config.clone());
        self.reset_draw_area_state();
    }

    pub fn debug_heightmap_loaded(&self) -> bool {
        self.height_map.is_some()
    }

    /// C++ `W3DTerrainVisual::getRawMapHeight`.
    pub fn get_raw_map_height(&self, grid_x: i32, grid_y: i32) -> i32 {
        let Some(height_map) = self.height_map.as_ref() else {
            return 0;
        };
        let x = grid_x + height_map.border_size;
        let y = grid_y + height_map.border_size;
        height_map.get_raw_height(x, y) as i32
    }

    /// C++ `W3DTerrainVisual::setRawMapHeight`.
    pub fn set_raw_map_height(&mut self, grid_x: i32, grid_y: i32, height: i32) {
        let Some(height_map) = self.height_map.as_mut() else {
            return;
        };
        let x = grid_x + height_map.border_size;
        let y = grid_y + height_map.border_size;
        if height_map.get_raw_height(x, y) as i32 > height {
            height_map.set_raw_height(x, y, height.clamp(0, u8::MAX as i32) as u8);
            self.static_lighting_changed();
        }
    }

    /// C++ `HeightMapRenderObjClass::staticLightingChanged`.
    pub fn static_lighting_changed(&mut self) {
        let Some(height_map) = self.height_map.as_ref() else {
            return;
        };
        self.chunk_manager.mark_region_dirty(
            0.0,
            0.0,
            self.config.world_size.0,
            self.config.world_size.1,
        );
        self.chunk_manager.refresh_dirty_chunks(height_map);
        self.chunk_meshes.clear();
        self.road_system.invalidate_terrain_lighting();
    }

    /// Apply C++ `W3DTerrainVisual::xfer` v>=2 raw height-map bytes.
    pub fn apply_logic_height_map_bytes(&mut self, data: &[u8]) {
        let Some(height_map) = self.height_map.as_mut() else {
            return;
        };
        let expected = (height_map.width as usize).saturating_mul(height_map.height as usize);
        if expected == 0 {
            return;
        }
        let n = expected.min(data.len()).min(height_map.heights.len());
        for i in 0..n {
            height_map.heights[i] = data[i] as f32 / crate::terrain::height_map::K_MAX_HEIGHT as f32;
        }
        self.static_lighting_changed();
    }

    /// C++ `WaterTracksRenderSystem::flush` from the live water record.
    pub fn flush_water_tracks(&mut self) {
        struct SampledWaterHeight {
            height: f32,
        }
        impl crate::terrain::WaterTrackHeightProvider for SampledWaterHeight {
            fn water_height(&self, _x: f32, _y: f32) -> f32 {
                self.height
            }
        }
        let height = self
            .get_water_grid_height(0.0, 0.0)
            .or_else(|| self.get_height_at(0.0, 0.0).ok())
            .unwrap_or(0.0);
        let flush = self.water_tracks.flush(&SampledWaterHeight { height });
        self.last_water_tracks_flush = flush;
        self.upload_water_track_meshes();
    }


    fn upload_water_track_meshes(&mut self) {
        let Some(device) = self.device.as_ref().cloned() else {
            self.water_track_meshes.clear();
            return;
        };
        let flush = &self.last_water_tracks_flush;
        if flush.vertices.is_empty() || flush.indices.is_empty() {
            self.water_track_meshes.clear();
            return;
        }
        let gpu_vertices: Vec<WaterGpuVertex> = flush
            .vertices
            .iter()
            .map(|v| {
                let rgba = crate::terrain::unpack_bgra_rgba(v.diffuse);
                WaterGpuVertex {
                    // C++ water-track verts are Z-up world; wgpu water pipeline is Y-up.
                    position: [v.x, v.z, v.y],
                    color: [rgba[0], rgba[1], rgba[2]],
                    tex_coords: [v.u1, v.v1],
                    alpha: rgba[3],
                    packed_c: v.diffuse,
                }
            })
            .collect();
        let gpu_indices: Vec<u32> = crate::terrain::triangle_list_from_strip(&flush.indices);
        if gpu_vertices.is_empty() || gpu_indices.is_empty() {
            self.water_track_meshes.clear();
            return;
        }
        self.water_track_meshes = vec![GpuWaterPlane {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Water Tracks Vertex Buffer"),
                contents: bytemuck::cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Water Tracks Index Buffer"),
                contents: bytemuck::cast_slice(&gpu_indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: gpu_indices.len() as u32,
        }];
    }

    pub fn water_tracks_mut(&mut self) -> &mut crate::terrain::WaterTracksRenderSystem {
        &mut self.water_tracks
    }

    pub fn last_water_tracks_flush(&self) -> &crate::terrain::WaterTracksFlush {
        &self.last_water_tracks_flush
    }


    pub fn debug_total_chunk_count(&self) -> usize {
        self.chunk_manager.total_chunk_count()
    }

    pub fn debug_visible_chunk_count(&self) -> usize {
        self.chunk_manager.get_visible_chunks().len()
    }

    pub fn debug_renderable_visible_chunk_count(&self) -> usize {
        self.chunk_manager.renderable_chunk_count()
    }

    pub fn debug_pending_visible_chunk_count(&self) -> usize {
        self.chunk_manager.pending_visible_chunk_count()
    }

    pub fn debug_chunk_summary(&self) -> String {
        self.chunk_manager.render_diagnostic_summary()
    }

    pub fn debug_total_chunk_geometry_revision(&self) -> u64 {
        self.chunk_manager.total_geometry_revision()
    }

    pub fn debug_dirty_chunk_count(&self) -> usize {
        self.chunk_manager.dirty_chunk_count()
    }

    pub fn debug_clear_dirty_chunks(&mut self) {
        self.chunk_manager.clear_dirty_flags_for_diagnostics();
    }

    pub fn debug_roads_need_terrain_normal_reprojection(&self) -> bool {
        self.road_system.needs_terrain_normal_reprojection()
    }

    pub fn debug_set_source_tile(&mut self, index: usize, tile: TileData) {
        if index >= self.source_tiles.len() {
            self.source_tiles.resize_with(index + 1, || None);
        }
        self.source_tiles[index] = Some(tile);
    }

    pub fn load_source_tiles_from_texture_classes(
        &mut self,
        classes: &[TerrainSourceTileClass],
    ) -> TerrainResult<usize> {
        let mut loaded = 0usize;
        for class in classes {
            loaded += self.load_source_tiles_for_class(class)?;
        }
        Ok(loaded)
    }

    fn load_source_tiles_for_class(
        &mut self,
        class: &TerrainSourceTileClass,
    ) -> TerrainResult<usize> {
        let first_tile = match usize::try_from(class.first_tile) {
            Ok(value) => value,
            Err(_) => return Ok(0),
        };
        let num_tiles = match usize::try_from(class.num_tiles) {
            Ok(value) if value > 0 => value,
            _ => return Ok(0),
        };
        let Some(path) = Self::resolve_source_tile_texture_path(&class.name) else {
            return Ok(0);
        };

        let image = image::open(&path)
            .map_err(|err| {
                TerrainError::TextureError(GameImageError::LoadError {
                    path: path.display().to_string(),
                    source: Box::new(err),
                })
            })?
            .to_rgba8();
        let image_width = image.width() as usize;
        let image_height = image.height() as usize;
        if image_width < 64 || image_height < 64 {
            return Ok(0);
        }

        let available_columns = image_width / 64;
        let available_rows = image_height / 64;
        let available_tiles = available_columns.saturating_mul(available_rows);
        let read_tiles = num_tiles.min(available_tiles);
        let rows = Self::source_tile_square_width(read_tiles, class.width);
        if rows == 0 {
            return Ok(0);
        }

        let needed_len = first_tile.saturating_add(rows.saturating_mul(rows));
        if needed_len > self.source_tiles.len() {
            self.source_tiles.resize_with(needed_len, || None);
        }

        let mut count = 0usize;
        for tile_row in 0..rows {
            for tile_col in 0..rows {
                let mut tile = TileData::new();
                for y in 0..64usize {
                    for x in 0..64usize {
                        let src_x = tile_col * 64 + x;
                        let src_y = tile_row * 64 + y;
                        let rgba = image.get_pixel(src_x as u32, src_y as u32).0;
                        let dst = (y * 64 + x) * 4;
                        tile.data[dst] = rgba[2];
                        tile.data[dst + 1] = rgba[1];
                        tile.data[dst + 2] = rgba[0];
                        tile.data[dst + 3] = rgba[3];
                    }
                }
                tile.tile_location_in_texture = ((tile_col * 64) as i32, (tile_row * 64) as i32);
                tile.update_mips();
                self.source_tiles[first_tile + count] = Some(tile);
                count += 1;
            }
        }

        Ok(count)
    }

    fn source_tile_square_width(num_tiles: usize, class_width: i32) -> usize {
        if let Ok(width) = usize::try_from(class_width) {
            if width > 0 && width <= 10 && num_tiles >= width.saturating_mul(width) {
                return width;
            }
        }
        for width in (1usize..=10).rev() {
            if num_tiles >= width.saturating_mul(width) {
                return width;
            }
        }
        0
    }

    fn resolve_source_tile_texture_path(class_name: &str) -> Option<PathBuf> {
        let mut candidates = Vec::new();
        if let Some(registry) = ini_terrain::get_terrain_types() {
            let guard = registry.read();
            let key = AsciiString::from(class_name);
            if let Some(terrain) = guard.find_terrain(&key) {
                let texture = terrain.texture_name.as_str().trim();
                if !texture.is_empty() {
                    candidates.push(format!("{TERRAIN_TGA_DIR_PATH}{texture}"));
                    candidates.push(texture.to_string());
                }
            }
        }
        candidates.push(class_name.to_string());

        for candidate in candidates {
            let path = Path::new(&candidate);
            if path.is_file() {
                return Some(path.to_path_buf());
            }
            if let Some(path) = TerrainTextures::resolve_texture_path(&candidate) {
                return Some(path);
            }
        }

        None
    }
}
