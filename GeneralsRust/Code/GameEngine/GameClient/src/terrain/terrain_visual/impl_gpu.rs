// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl TerrainVisualImpl {
    /// Initialize WGPU resources
    pub fn init_gpu_resources(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> TerrainResult<()> {
        self.device = Some(Arc::clone(&device));
        self.queue = Some(Arc::clone(&queue));

        // Create terrain render pipeline
        self.create_terrain_pipeline(device.as_ref())?;
        self.create_extra_blend_pipeline(device.as_ref())?;

        // Create skybox background pipeline before terrain/water draws.
        self.create_skybox_background_pipeline(device.as_ref())?;

        // Create water render pipeline
        self.create_water_pipeline(device.as_ref())?;

        // Create road render pipeline
        self.create_road_pipeline(device.as_ref())?;
        self.ensure_road_texture_bind_group(device.as_ref());
        self.create_tree_pipeline(device.as_ref())?;

        self.sync_global_water_plane(device.as_ref())?;
        self.upload_extra_blend_overlay();

        // Create uniform buffer
        self.uniform_buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Terrain Uniform Buffer"),
            size: std::mem::size_of::<TerrainUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        if let (Some(layout), Some(buffer)) =
            (&self.terrain_camera_bind_group_layout, &self.uniform_buffer)
        {
            self.terrain_camera_bind_group =
                Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Terrain Camera Bind Group"),
                    layout: layout.as_ref(),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    }],
                }));
        }

        Ok(())
    }

    fn update_chunk_meshes(&mut self) -> TerrainResult<()> {
        let started = std::time::Instant::now();
        let device = match &self.device {
            Some(device) => Arc::clone(device),
            None => return Ok(()),
        };

        let queue = match &self.queue {
            Some(queue) => Arc::clone(queue),
            None => return Ok(()),
        };

        let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();
        let refresh_texture_slots = self.active_chunk_texture_ids.is_none();

        let select_slots_started = std::time::Instant::now();
        let stable_texture_ids = if refresh_texture_slots {
            let ids = self.select_stable_chunk_texture_ids(&visible_chunk_ids);
            self.active_chunk_texture_ids = Some(ids);
            ids
        } else {
            self.active_chunk_texture_ids
                .unwrap_or([0; MAX_TEXTURES_PER_CHUNK])
        };
        let select_slots_elapsed = select_slots_started.elapsed();

        let texture_slots_changed = refresh_texture_slots;

        let mut shared_slot_map: HashMap<TextureId, usize> = HashMap::new();
        for (slot, texture_id) in stable_texture_ids.iter().enumerate() {
            shared_slot_map.entry(*texture_id).or_insert(slot);
        }

        let mut binding_updates = 0usize;
        let mut mesh_uploads = 0usize;
        let mut vertices_uploaded = 0usize;
        let mut indices_uploaded = 0usize;
        let mut binding_prep_elapsed = std::time::Duration::ZERO;
        let mut mesh_upload_elapsed = std::time::Duration::ZERO;
        for &chunk_id in &visible_chunk_ids {
            let (chunk_revision, has_chunk_geometry) = match self.chunk_manager.get_chunk(chunk_id)
            {
                Some(chunk) => (
                    chunk.geometry_revision,
                    !(chunk.vertices.is_empty() || chunk.indices.is_empty()),
                ),
                None => continue,
            };

            let binding_up_to_date = self
                .chunk_texture_bindings
                .get(&chunk_id)
                .map(|binding| binding.texture_ids == stable_texture_ids)
                .unwrap_or(false);
            if !binding_up_to_date {
                let binding_started = std::time::Instant::now();
                self.prepare_chunk_texture_binding(
                    chunk_id,
                    &stable_texture_ids,
                    &shared_slot_map,
                    device.as_ref(),
                    queue.as_ref(),
                )?;
                binding_prep_elapsed += binding_started.elapsed();
                binding_updates += 1;
            }

            let needs_mesh_upload = match self.chunk_meshes.get(&chunk_id) {
                Some(mesh) => mesh.revision != chunk_revision || texture_slots_changed,
                None => true,
            };

            if needs_mesh_upload {
                if !has_chunk_geometry {
                    continue;
                }
                let upload_started = std::time::Instant::now();

                let (chunk_vertices, chunk_indices) = match self.chunk_manager.get_chunk(chunk_id) {
                    Some(chunk) => (chunk.vertices.clone(), chunk.indices.clone()),
                    None => continue,
                };

                let mut gpu_vertices = chunk_vertices;
                let mut vertex_weights = Vec::with_capacity(gpu_vertices.len());

                for vertex in &gpu_vertices {
                    let position =
                        Vec3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
                    let normal_vec =
                        Vec3::new(vertex.normal[0], vertex.normal[1], vertex.normal[2]);
                    let mut normal = if normal_vec.length_squared() > f32::EPSILON {
                        normal_vec.normalize()
                    } else {
                        Vec3::Y
                    };
                    if !normal.y.is_finite() {
                        normal = Vec3::Y;
                    }

                    let height = position.y;
                    let slope = normal.dot(Vec3::Y).clamp(-1.0, 1.0).acos();

                    // C++ WorldHeightMap samples BlendTileData tileNdx, not
                    // height/slope procedural weights (hq-32be).
                    let map_tile = self.height_map.as_ref().map(|hm| {
                        hm.get_packed_terrain_tile_at_world(position.x, position.z)
                    });
                    let blended = if let Some(tile) = map_tile.filter(|&t| t != 0) {
                        let mut weights = self.texture_system.generate_texture_weights(
                            height,
                            slope,
                            vertex.tex_coords,
                            &self.texture_rules,
                        );
                        let slot = (tile as usize) % MAX_TEXTURES_PER_CHUNK;
                        for (i, w) in weights.weights.iter_mut().enumerate() {
                            *w = if i == slot { 1.0 } else { 0.0 };
                        }
                        weights.indices = [slot as u32, 0, 0, 0];


                        weights
                    } else {
                        let base_weights = self.texture_system.generate_texture_weights(
                            height,
                            slope,
                            vertex.tex_coords,
                            &self.texture_rules,
                        );
                        self.texture_system.blend_textures_at_position(
                            position,
                            height,
                            normal,
                            vertex.tex_coords,
                            &base_weights,
                            &self.texture_rules,
                        )
                    };


                    vertex_weights.push(blended);
                }

                for (vertex, blended) in gpu_vertices.iter_mut().zip(vertex_weights.iter()) {
                    let mut packed_indices = [0u16; MAX_BLEND_WEIGHTS];
                    let mut packed_weights = [0.0f32; MAX_BLEND_WEIGHTS];

                    let mut insert_count = 0usize;
                    for (texture_id, weight) in blended.iter_pairs() {
                        if weight <= 0.0 {
                            break;
                        }

                        let slot_idx = shared_slot_map
                            .get(&texture_id)
                            .copied()
                            .or_else(|| {
                                shared_slot_map
                                    .get(stable_texture_ids.first().unwrap())
                                    .copied()
                            })
                            .unwrap_or(0);

                        if insert_count < MAX_BLEND_WEIGHTS {
                            packed_indices[insert_count] = slot_idx as u16;
                            packed_weights[insert_count] = weight;
                            insert_count += 1;
                        } else {
                            let mut weakest = 0usize;
                            let mut weakest_weight = packed_weights[0];
                            for idx in 1..MAX_BLEND_WEIGHTS {
                                if packed_weights[idx] < weakest_weight {
                                    weakest_weight = packed_weights[idx];
                                    weakest = idx;
                                }
                            }

                            if weight > weakest_weight {
                                packed_indices[weakest] = slot_idx as u16;
                                packed_weights[weakest] = weight;
                            }
                        }
                    }

                    let sum: f32 = packed_weights.iter().sum();
                    if sum > f32::EPSILON {
                        for weight in &mut packed_weights {
                            *weight /= sum;
                        }
                    } else {
                        packed_weights[0] = 1.0;
                        packed_indices[0] = shared_slot_map
                            .get(stable_texture_ids.first().unwrap())
                            .copied()
                            .unwrap_or(0) as u16;
                    }

                    vertex.blend_indices = packed_indices;
                    vertex.blend_weights = packed_weights;
                }

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Chunk {} Vertex Buffer", chunk_id)),
                    contents: cast_slice(&gpu_vertices),
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("Terrain Chunk {} Index Buffer", chunk_id)),
                    contents: cast_slice(&chunk_indices),
                    usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
                });

                self.chunk_meshes.insert(
                    chunk_id,
                    GpuChunkMesh {
                        vertex_buffer,
                        index_buffer,
                        index_count: chunk_indices.len() as u32,
                        revision: chunk_revision,
                    },
                );
                mesh_upload_elapsed += upload_started.elapsed();
                mesh_uploads += 1;
                vertices_uploaded += gpu_vertices.len();
                indices_uploaded += chunk_indices.len();
            }
        }

        self.chunk_meshes
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        self.chunk_texture_bindings
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        self.upload_extra_blend_overlay();

        self.stats.rendered_chunks = visible_chunk_ids.len();
        self.stats.triangles_rendered = visible_chunk_ids
            .iter()
            .filter_map(|chunk_id| self.chunk_manager.get_chunk(*chunk_id))
            .map(|chunk| chunk.stats.triangle_count as usize)
            .sum();

        let elapsed = started.elapsed();
        if elapsed >= std::time::Duration::from_millis(200) {
            warn!(
                "TerrainVisual::update_chunk_meshes breakdown: total={:?} visible={} refresh_texture_slots={} select_slots={:?} binding_updates={} binding_prep={:?} mesh_uploads={} uploaded_vertices={} uploaded_indices={} mesh_upload={:?} pending_visible={}",
                elapsed,
                visible_chunk_ids.len(),
                refresh_texture_slots,
                select_slots_elapsed,
                binding_updates,
                binding_prep_elapsed,
                mesh_uploads,
                vertices_uploaded,
                indices_uploaded,
                mesh_upload_elapsed,
                self.chunk_manager.pending_visible_chunk_count()
            );
        }

        Ok(())
    }

    fn select_stable_chunk_texture_ids(
        &mut self,
        _visible_chunk_ids: &[ChunkId],
    ) -> [TextureId; MAX_TEXTURES_PER_CHUNK] {
        let mut selected_textures: Vec<TextureId> = Vec::new();
        for rule in &self.texture_rules {
            let texture_id = rule.texture_id;
            if texture_id == 0 || self.texture_system.get_texture(texture_id).is_none() {
                continue;
            }
            if selected_textures
                .iter()
                .all(|candidate| *candidate != texture_id)
            {
                selected_textures.push(texture_id);
                if selected_textures.len() == MAX_TEXTURES_PER_CHUNK {
                    break;
                }
            }
        }

        if selected_textures.is_empty() {
            if let Some(rule) = self.texture_rules.first() {
                selected_textures.push(rule.texture_id);
            } else if let Some(texture_id) = self.texture_system.first_texture_id() {
                selected_textures.push(texture_id);
            }
        }

        if selected_textures.is_empty() {
            return [0; MAX_TEXTURES_PER_CHUNK];
        }

        while selected_textures.len() < MAX_TEXTURES_PER_CHUNK {
            if let Some(&fallback) = selected_textures.first() {
                selected_textures.push(fallback);
            } else {
                break;
            }
        }

        let mut stable_texture_ids = [0; MAX_TEXTURES_PER_CHUNK];
        for (idx, texture_id) in selected_textures
            .iter()
            .enumerate()
            .take(MAX_TEXTURES_PER_CHUNK)
        {
            stable_texture_ids[idx] = *texture_id;
        }
        stable_texture_ids
    }

    fn update_road_meshes(&mut self) -> TerrainResult<()> {
        // Retry Roads.ini / Art/Terrain/Road lookup when overlays rebuild
        // (map load) or the bind group was never created. Do not rebuild
        // GPU road meshes unless overlay_gpu_meshes_dirty.
        let retry_road_texture =
            self.road_texture_bind_group.is_none() || self.overlay_gpu_meshes_dirty;
        if retry_road_texture {
            if let Some(device) = self.device.clone() {
                self.ensure_road_texture_bind_group(device.as_ref());
            }
        }
        if !self.overlay_gpu_meshes_dirty {
            return Ok(());
        }


        let Some(device) = self.device.as_ref().cloned() else {
            self.road_meshes.clear();
            self.bridge_meshes.clear();
            self.scorch_meshes.clear();
            self.overlay_gpu_meshes_dirty = true;
            return Ok(());
        };


        let mut road_meshes = Vec::new();
        let mut bridge_meshes = Vec::new();
        let height_map = self.height_map.as_ref();
        let height_at = |x: f32, y: f32| {
            height_map
                .map(|height_map| height_map.get_height_at(x, y))
                .unwrap_or(0.0)
        };

        self.road_system
            .for_each_visible_overlay_source(|road, segment| {
                if matches!(road.road_type, RoadType::StoneBridge { .. }) {
                    let width = segment.width.max(0.1);
                    let scale = (width / 10.0).max(0.01);
                    let (left, section, right) = default_sectional_bridge_model(scale);
                    let from = [
                        segment.start.x,
                        segment.start.z,
                        segment.start.y + BRIDGE_FLOAT_AMT,
                    ];
                    let to = [
                        segment.end.x,
                        segment.end.z,
                        segment.end.y + BRIDGE_FLOAT_AMT,
                    ];
                    let baked = bake_bridge_span(
                        from,
                        to,
                        true,
                        left,
                        Some(section),
                        Some(right),
                        0xffff_ffff,
                    );
                    if baked.vertices.is_empty() || baked.indices.is_empty() {
                        return;
                    }
                    let gpu_vertices = fill_bridge_gpu_upload_vertices(&baked.vertices);
                    let gpu_indices: Vec<u32> =
                        baked.indices.iter().map(|index| *index as u32).collect();
                    if let Some(mesh) = Self::upload_overlay_mesh(
                        &device,
                        "Bridge Mesh",
                        &gpu_vertices,
                        &gpu_indices,
                    ) {
                        bridge_meshes.push(mesh);
                    }
                    return;
                }

                let kind = segment
                    .properties
                    .texture_override
                    .as_deref()
                    .unwrap_or("Kind=SEGMENT");
                let use_cpp_float4 = !kind.contains("TEE")
                    && !kind.contains("FOUR_WAY")
                    && !kind.contains("CURVE")
                    && !kind.contains("ALPHA_JOIN")
                    && !kind.contains("SyntheticIntersection");

                if use_cpp_float4 {
                    if let Some((verts, indices)) = bake_straight_road_segment(
                        [segment.start.x, segment.start.z],
                        [segment.end.x, segment.end.z],
                        segment.width,
                        0.0,
                        85.0 / 512.0,
                        segment.width.max(DEFAULT_ROAD_SCALE),
                        height_at,
                    ) {
                        if !verts.is_empty() && !indices.is_empty() {
                            let gpu_vertices = fill_road_gpu_upload_vertices(&verts);
                            let gpu_indices: Vec<u32> =
                                indices.iter().map(|index| *index as u32).collect();
                            if let Some(mesh) = Self::upload_overlay_mesh(
                                &device,
                                "Road Mesh",
                                &gpu_vertices,
                                &gpu_indices,
                            ) {
                                road_meshes.push(mesh);
                            }
                            return;
                        }
                    }
                }

                let Some(geometry) = segment.geometry.as_ref() else {
                    return;
                };
                if geometry.vertices.is_empty() || geometry.indices.is_empty() {
                    return;
                }
                let gpu_vertices: Vec<RoadVertex> = geometry
                    .vertices
                    .iter()
                    .map(|vertex| OverlayGpuVertex {
                        position: vertex.position,
                        color: [
                            vertex.color[0],
                            vertex.color[1],
                            vertex.color[2],
                        ],
                        tex_coords: vertex.tex_coords,
                        road_width: 1.0,
                        diffuse: 0xFFFF_FFFF,
                    })
                    .collect();
                if let Some(mesh) = Self::upload_overlay_mesh(
                    &device,
                    "Road Mesh",
                    &gpu_vertices,
                    &geometry.indices,
                ) {
                    road_meshes.push(mesh);
                }
            });

        self.road_meshes = road_meshes;
        self.bridge_meshes = bridge_meshes;
        self.update_scorch_meshes(&device);
        self.overlay_gpu_meshes_dirty = false;
        Ok(())

    }

    fn update_scorch_meshes(&mut self, device: &wgpu::Device) {
        let Some(height_map) = self.height_map.as_ref() else {
            self.scorch_meshes.clear();
            return;
        };
        let baked = bake_terrain_scorch_gpu_mesh(height_map, 0xffff_ffff);
        if baked.vertices.is_empty() || baked.indices.is_empty() {
            self.scorch_meshes.clear();
            return;
        }
        let gpu_vertices: Vec<RoadVertex> = baked
            .vertices
            .iter()
            .map(|vertex| {
                OverlayGpuVertex::from_cpp_xyzduv(
                    vertex.x,
                    vertex.y,
                    vertex.z,
                    vertex.diffuse,
                    vertex.u1,
                    vertex.v1,
                )
            })
            .collect();
        let gpu_indices: Vec<u32> = baked.indices.iter().map(|index| *index as u32).collect();
        self.scorch_meshes =
            Self::upload_overlay_mesh(device, "Scorch Mesh", &gpu_vertices, &gpu_indices)
                .into_iter()
                .collect();
    }

    fn upload_overlay_mesh(
        device: &wgpu::Device,
        label: &str,
        vertices: &[RoadVertex],
        indices: &[u32],
    ) -> Option<GpuRoadMesh> {
        if vertices.is_empty() || indices.is_empty() {
            return None;
        }
        Some(GpuRoadMesh {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
        })
    }

    pub fn record_chunk_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.enabled {
            return;
        }

        self.record_skybox_background_draw(pass);

        if let Some(pipeline) = &self.terrain_pipeline {
            pass.set_pipeline(pipeline);
            if let Some(camera_bg) = &self.terrain_camera_bind_group {
                pass.set_bind_group(0, camera_bg, &[]);
            }

            let chunk_meshes = &self.chunk_meshes;
            let chunk_texture_bindings = &self.chunk_texture_bindings;
            let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();

            let _ = self.chunk_manager.render_pass_draw(
                pass,
                |chunk_id| {
                    chunk_texture_bindings
                        .get(&chunk_id)
                        .map(|binding| binding.bind_group.clone())
                },
                |chunk_id| {
                    let mesh = chunk_meshes.get(&chunk_id)?;
                    if !visible_chunk_ids.contains(&chunk_id) {
                        return None;
                    }
                    Some((
                        mesh.vertex_buffer.slice(..),
                        mesh.index_buffer.slice(..),
                        mesh.index_count,
                    ))
                },
            );
        }

        self.record_extra_blend_pass(pass);
        self.record_road_draws(pass);
        self.record_tree_draws(pass);
        self.record_water_draws(pass);
    }

    pub fn record_chunk_depth_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.enabled {
            return;
        }

        if let Some(pipeline) = &self.terrain_depth_pipeline {
            pass.set_pipeline(pipeline);
            if let Some(camera_bg) = &self.terrain_camera_bind_group {
                pass.set_bind_group(0, camera_bg, &[]);
            }

            let chunk_meshes = &self.chunk_meshes;
            let visible_chunk_ids = self.visible_chunk_ids_for_draw_area();

            let _ = self.chunk_manager.render_pass_draw(
                pass,
                |_| None,
                |chunk_id| {
                    let mesh = chunk_meshes.get(&chunk_id)?;
                    if !visible_chunk_ids.contains(&chunk_id) {
                        return None;
                    }
                    Some((
                        mesh.vertex_buffer.slice(..),
                        mesh.index_buffer.slice(..),
                        mesh.index_count,
                    ))
                },
            );
        }
    }

    fn record_skybox_background_draw<'pass>(&self, pass: &mut RenderPass<'pass>) {
        let (Some(pipeline), Some(bind_group)) = (
            self.skybox_background_pipeline.as_ref(),
            self.skybox_background_bind_group.as_ref(),
        ) else {
            return;
        };

        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, bind_group, &[]);
        pass.draw(0..3, 0..1);
    }

    /// Ensure the GlobalData water mesh exists (honors `water_position_z` / extents).
    pub fn ensure_global_water_plane(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        self.sync_global_water_plane(device)
    }

    pub fn record_water_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(water_pipeline), Some(camera_bg)) = (
            self.water_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };

        pass.set_pipeline(water_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);

        if let Some(water_plane) = self.water_plane.as_ref() {
            pass.set_vertex_buffer(0, water_plane.vertex_buffer.slice(..));
            pass.set_index_buffer(
                water_plane.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..water_plane.index_count, 0, 0..1);
        }

        // C++ WaterTracksRenderSystem::flush / render after the water plane.
        for mesh in &self.water_track_meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }


    fn record_road_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(road_pipeline), Some(camera_bg), Some(road_bg)) = (
            self.road_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.road_texture_bind_group.as_ref(),
        ) else {
            return;
        };
        if self.road_meshes.is_empty() && self.bridge_meshes.is_empty() && self.scorch_meshes.is_empty()
        {
            return;
        }

        pass.set_pipeline(road_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_bind_group(1, road_bg, &[]);
        for mesh in self
            .road_meshes
            .iter()
            .chain(self.bridge_meshes.iter())
            .chain(self.scorch_meshes.iter())
        {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn ensure_road_texture_bind_group(&mut self, device: &wgpu::Device) {
        if self.road_texture_bind_group.is_some() && !self.road_texture_is_fallback {
            return;
        }
        let Some(layout) = self.road_texture_bind_group_layout.clone() else {
            return;
        };
        let Some(queue) = self.queue.clone() else {
            return;
        };

        let (texture, used_fallback, source_name) =
            match self.load_first_available_road_texture(device) {
                Some((texture, name)) => (texture, false, name),
                None => (
                    Self::create_fallback_road_texture(device, queue.as_ref()),
                    true,
                    "fallback-gravel-2x2".to_string(),
                ),
            };

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        if self.road_sampler.is_none() {
            self.road_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Road Texture Sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let Some(sampler) = self.road_sampler.as_ref() else {
            return;
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Road Texture Bind Group"),
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

        if used_fallback {
            warn!("Road texture missing; using repeatable 2x2 gravel fallback");
        } else {
            info!("Road texture bind group ready using {}", source_name);
        }
        self.road_texture = Some(texture);
        self.road_texture_bind_group = Some(bind_group);
        self.road_texture_is_fallback = used_fallback;
    }

    fn load_first_available_road_texture(
        &self,
        device: &wgpu::Device,
    ) -> Option<(wgpu::Texture, String)> {
        for name in self.road_texture_name_candidates() {
            for path in Self::road_texture_path_candidates(&name) {
                if TerrainTextures::is_available_terrain_texture_path(&path)
                    || TerrainTextures::can_resolve_texture_path(&path)
                {
                    if let Ok(texture) = self.load_texture_from_path(device, &path) {
                        return Some((texture, path));
                    }
                }
                if let Ok(texture) = self.load_texture_from_path(device, &path) {
                    return Some((texture, path));
                }
            }
        }
        None
    }

    fn road_texture_name_candidates(&self) -> Vec<String> {
        let mut names = Vec::new();
        let mut push_name = |name: &str| {
            let trimmed = name.trim();
            if trimmed.is_empty() {
                return;
            }
            if !names
                .iter()
                .any(|existing: &String| existing.eq_ignore_ascii_case(trimmed))
            {
                names.push(trimmed.to_string());
            }
        };

        if let Some(roads) = game_engine::common::ini::try_get_terrain_roads() {
            for road in roads.iter_roads() {
                push_name(road.texture.as_str());
            }
        }

        // Retail Roads.ini Texture names (W3DRoadBuffer.cpp loadRoads firstRoad).
        for name in [
            "TRTwoLane.tga",
            "TRDirtRoad.tga",
            "TRGravelRoad.tga",
            "TRCobbRoad.tga",
            "TRFourLane.tga",
            "TRDirtPath.tga",
            "TRSidewalk.tga",
        ] {
            push_name(name);
        }
        names
    }

    fn road_texture_path_candidates(name: &str) -> Vec<String> {
        let basename = Path::new(name)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(name);
        let mut paths = vec![
            format!("{TERRAIN_TGA_DIR_PATH}Road/{basename}"),
            format!("ART/Terrain/Road/{basename}"),
            format!("{TERRAIN_TGA_DIR_PATH}{basename}"),
            format!("ART/Terrain/{basename}"),
            format!("{TGA_DIR_PATH}{basename}"),
            format!("art/textures/{basename}"),
            basename.to_string(),
        ];
        if name != basename {
            paths.insert(0, name.replace('\\', "/"));
        }
        paths
    }

    fn create_fallback_road_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        // Repeatable 2x2 gravel last resort after Art/Terrain/Road + Roads.ini probe.
        const PIXELS: [u8; 16] = [
            110, 100, 86, 255, 138, 128, 112, 255, 138, 128, 112, 255, 110, 100, 86, 255,
        ];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Road Fallback Gravel 2x2"),
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
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
            &PIXELS,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(8),
                rows_per_image: Some(2),
            },
            wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
        );
        texture
    }


    fn record_tree_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(tree_pipeline), Some(camera_bg)) = (
            self.tree_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };
        if self.tree_meshes.is_empty() {
            return;
        }

        pass.set_pipeline(tree_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        for mesh in &self.tree_meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn current_tree_object_lights(&self) -> [TreeObjectLight; TREE_MAX_GLOBAL_LIGHTS] {
        let mut lights = [TreeObjectLight::default(); TREE_MAX_GLOBAL_LIGHTS];
        lights[0] = TreeObjectLight {
            ambient: self.ambient_color,
            diffuse: self.sun_color,
            // wgpu Y-up sun → C++ Z-up lightPos used by W3DTreeBuffer::doLighting.
            light_pos: [
                self.sun_direction.x,
                self.sun_direction.z,
                self.sun_direction.y,
            ],
        };
        lights
    }

    /// C++ `W3DTreeBuffer::drawTrees` VB fill + wgpu upload. Called every update/draw.
    pub fn update_tree_meshes(&mut self) {
        // C++ WaterRenderObjClass::render recenters `new_skybox` on the camera
        // each frame (W3DWater.cpp:1702-1707). Rebind the N/E/S/W/T face here
        // because this is the live per-frame `&mut` hook from `render`/`update`.
        self.rebind_skybox_background_for_camera();

        // C++ `loadTreesInVertexAndIndexBuffers` returns when nothing changed.
        if !self.tree_meshes.is_empty()
            && !self.tree_buffer.need_to_update_texture()
            && !self.tree_buffer.anything_changed()
        {
            return;
        }

        // C++ `drawTrees`: if m_needToUpdateTexture then updateTexture (blit+mip+SetLOD).
        // GlobalData::m_textureReductionFactor when present; else last SetLOD / atlas_lod.
        let lod = get_global_data()
            .map(|global_data| global_data.read().texture_reduction_factor.clamp(0, 4))
            .unwrap_or_else(|| self.tree_buffer.atlas_lod());
        self.tree_buffer.sync_tree_atlas_for_draw(lod);
        let lights = self.current_tree_object_lights();
        self.tree_buffer
            .draw_trees_fill_vertex_buffer(&lights, Vec3::Z, |_| true);
        let gpu_vertices = fill_tree_gpu_upload_vertices(self.tree_buffer.cpu_vertices());
        let gpu_indices: Vec<u32> = self
            .tree_buffer
            .cpu_indices()
            .iter()
            .map(|index| *index as u32)
            .collect();
        self.last_tree_gpu_vertices = gpu_vertices.clone();
        self.last_tree_atlas_mips = self.tree_buffer.atlas_upload_levels().to_vec();
        self.upload_tree_atlas_texture();

        let Some(device) = self.device.as_ref().cloned() else {
            self.tree_meshes.clear();
            return;
        };
        if gpu_vertices.is_empty() || gpu_indices.is_empty() {
            self.tree_meshes.clear();
            return;
        }
        self.tree_meshes = Self::upload_tree_mesh(&device, &gpu_vertices, &gpu_indices)
            .into_iter()
            .collect();
    }

    fn upload_tree_mesh(
        device: &wgpu::Device,
        vertices: &[TreeGpuVertex],
        indices: &[u32],
    ) -> Option<GpuTreeMesh> {
        if vertices.is_empty() || indices.is_empty() {
            return None;
        }
        Some(GpuTreeMesh {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Mesh"),
                contents: cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Mesh Indices"),
                contents: cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            }),
            index_count: indices.len() as u32,
        })
    }

    pub fn tree_buffer_mut(&mut self) -> &mut W3DTreeBuffer {
        &mut self.tree_buffer
    }

    pub fn last_tree_gpu_vertices(&self) -> &[TreeGpuVertex] {
        &self.last_tree_gpu_vertices
    }

    pub fn last_tree_atlas_mips(&self) -> &[Vec<u8>] {
        &self.last_tree_atlas_mips
    }

    fn upload_tree_atlas_texture(&mut self) {
        let (Some(device), Some(queue)) = (self.device.as_ref(), self.queue.as_ref()) else {
            self.tree_atlas_texture = None;
            return;
        };
        let levels = self.tree_buffer.atlas_upload_levels();
        if levels.is_empty() {
            self.tree_atlas_texture = None;
            return;
        }
        let lod = self.tree_buffer.atlas_upload_mip_index() as u32;
        let (full_w, _) = self.tree_buffer.texture_size();
        let top_w = (full_w.max(1) as u32 >> lod).max(1);
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("W3D Tree Atlas"),
            size: wgpu::Extent3d {
                width: top_w,
                height: top_w,
                depth_or_array_layers: 1,
            },
            mip_level_count: levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        for (mip, data) in levels.iter().enumerate() {
            let mip_w = (top_w >> mip as u32).max(1);
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: mip as u32,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                data,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(mip_w * 4),
                    rows_per_image: Some(mip_w),
                },
                wgpu::Extent3d {
                    width: mip_w,
                    height: mip_w,
                    depth_or_array_layers: 1,
                },
            );
        }
        self.tree_atlas_texture = Some(texture);
    }

    fn sync_global_water_plane(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(global_data) = get_global_data() else {
            self.water_plane = None;
            return Ok(());
        };
        let global = global_data.read();

        if !self.config.water_enabled
            || global.water_extent_x <= 0.0
            || global.water_extent_y <= 0.0
        {
            self.water_plane = None;
            return Ok(());
        }

        let water_z = global.water_position_z;
        let half_extent_x = global.water_extent_x * 0.5;
        let half_extent_y = global.water_extent_y * 0.5;
        let min_x = global.water_position_x - half_extent_x;
        let min_y = global.water_position_y - half_extent_y;
        let max_x = global.water_position_x + half_extent_x;
        let max_y = global.water_position_y + half_extent_y;

        // Tile 15×15 patches across GlobalData extents (do not stretch one sheet).
        let (patch_vertices, list_indices) =
            bake_water_tiles_world(min_x, min_y, max_x, max_y, water_z, 0xffff_ffff);
        if patch_vertices.is_empty() || list_indices.is_empty() {
            self.water_plane = None;
            return Ok(());
        }
        let vertices = fill_water_gpu_upload_vertices(&patch_vertices);

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Water Plane Vertex Buffer"),
            contents: cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Global Water Plane Index Buffer"),
            contents: cast_slice(&list_indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        self.water_plane = Some(GpuWaterPlane {
            vertex_buffer,
            index_buffer,
            index_count: list_indices.len() as u32,
        });

        Ok(())
    }

    /// Rebuild C++ extra-blend tile positions from the loaded heightmap.
    pub fn rebuild_extra_blend_gpu_state(&mut self) {
        self.extra_blend_tile_positions = self
            .height_map
            .as_ref()
            .map(|height_map| height_map.collect_extra_blend_tile_positions())
            .unwrap_or_default();
    }

    /// C++ extra-blend second pass: two triangles per packed tile, honoring
    /// `getExtraAlphaUVData` U/V, per-corner alpha, and `need_flip`.
    pub fn build_extra_blend_draw_mesh(&self) -> ExtraBlendDrawMesh {
        let Some(height_map) = self.height_map.as_ref() else {
            return ExtraBlendDrawMesh::default();
        };
        height_map.build_extra_blend_draw_mesh_for_window(
            &self.extra_blend_tile_positions,
            self.draw_origin_x,
            self.draw_origin_y,
            self.draw_width,
            self.draw_height,
        )
    }

    fn extra_blend_mesh_to_terrain_vertices(
        &self,
        mesh: &ExtraBlendDrawMesh,
    ) -> Vec<TerrainVertex> {
        mesh.vertices
            .iter()
            .map(|vertex| {
                let position = Vec3::from_array(vertex.position);
                let normal = self
                    .height_map
                    .as_ref()
                    .map(|height_map| height_map.get_normal_at(position.x, position.z))
                    .unwrap_or(Vec3::Y);
                let mut color = Self::terrain_static_diffuse_from_normal(
                    normal,
                    self.sun_direction,
                    self.sun_color,
                    self.ambient_color,
                );
                color[3] = vertex.color[3];
                TerrainVertex {
                    position: vertex.position,
                    normal: [normal.x, normal.y, normal.z],
                    tex_coords: vertex.tex_coords,
                    // C++ second pass samples extra-blend tile art (blend.blend_ndx),
                    // not the base slot-0 procedural texture (hq-qar9).
                    blend_indices: [1, 0, 0, 0],
                    blend_weights: [1.0, 0.0, 0.0, 0.0],
                    color,
                }
            })
            .collect()
    }

    /// Stage extra-blend overlay data and upload packed positions + draw verts
    /// when a GPU device is present. C++ draws these as a second 3-way blend pass.
    pub fn upload_extra_blend_overlay(&mut self) {
        self.rebuild_extra_blend_gpu_state();
        let positions = self.extra_blend_tile_positions.clone();
        let mesh = self.build_extra_blend_draw_mesh();
        self.extra_blend_gpu_upload = ExtraBlendGpuUpload {
            tile_count: positions.len(),
            positions: positions.clone(),
            vertex_count: mesh.vertex_count(),
            index_count: mesh.index_count(),
        };
        self.extra_blend_draw_mesh = mesh;
        self.extra_blend_vertex_count = self.extra_blend_draw_mesh.vertex_count() as u32;
        self.extra_blend_index_count = self.extra_blend_draw_mesh.index_count() as u32;

        let Some(device) = self.device.as_ref() else {
            self.extra_blend_position_buffer = None;
            self.extra_blend_vertex_buffer = None;
            self.extra_blend_index_buffer = None;
            return;
        };
        if positions.is_empty() {
            self.extra_blend_position_buffer = None;
            self.extra_blend_vertex_buffer = None;
            self.extra_blend_index_buffer = None;
            return;
        }
        self.extra_blend_position_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Terrain Extra Blend Positions"),
                contents: cast_slice(&positions),
                usage: wgpu::BufferUsages::VERTEX
                    | wgpu::BufferUsages::STORAGE
                    | wgpu::BufferUsages::COPY_DST,
            },
        ));

        if self.extra_blend_draw_mesh.is_empty() {
            self.extra_blend_vertex_buffer = None;
            self.extra_blend_index_buffer = None;
            return;
        }
        let gpu_vertices = self.extra_blend_mesh_to_terrain_vertices(&self.extra_blend_draw_mesh);
        self.extra_blend_vertex_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Terrain Extra Blend Vertices"),
                contents: cast_slice(&gpu_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            },
        ));
        self.extra_blend_index_buffer = Some(device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Terrain Extra Blend Indices"),
                contents: cast_slice(&self.extra_blend_draw_mesh.indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            },
        ));
    }

    /// Increment the shipped extra-blend draw counter when overlay tiles exist.
    /// Returns whether a draw was recorded.
    pub fn extra_blend_draw(&self) -> bool {
        if self.extra_blend_tile_count() == 0
            && self.extra_blend_gpu_upload.is_empty()
            && self.extra_blend_draw_mesh.is_empty()
        {
            return false;
        }
        self.extra_blend_draw_count
            .fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Second extra-blend pass over the base terrain (alpha overlay, no Z write).
    pub fn record_extra_blend_pass<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        if !self.extra_blend_draw() {
            return;
        }
        let Some(vertex_buffer) = self.extra_blend_vertex_buffer.as_ref() else {
            return;
        };
        let Some(index_buffer) = self.extra_blend_index_buffer.as_ref() else {
            return;
        };
        if self.extra_blend_index_count == 0 {
            return;
        }
        let Some(pipeline) = self
            .extra_blend_pipeline
            .as_ref()
            .or(self.terrain_pipeline.as_ref())
        else {
            return;
        };
        let Some(texture_binding) = self.chunk_texture_bindings.values().next() else {
            return;
        };

        pass.set_pipeline(pipeline);
        if let Some(camera_bg) = &self.terrain_camera_bind_group {
            pass.set_bind_group(0, camera_bg, &[]);
        }
        pass.set_bind_group(1, &texture_binding.bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        pass.draw_indexed(0..self.extra_blend_index_count, 0, 0..1);
    }

    pub fn extra_blend_tile_positions(&self) -> &[u32] {
        &self.extra_blend_tile_positions
    }

    pub fn extra_blend_tile_count(&self) -> usize {
        self.extra_blend_tile_positions.len()
    }

    pub fn last_extra_blend_gpu_upload(&self) -> &ExtraBlendGpuUpload {
        &self.extra_blend_gpu_upload
    }

    pub fn last_extra_blend_draw_mesh(&self) -> &ExtraBlendDrawMesh {
        &self.extra_blend_draw_mesh
    }

    pub fn extra_blend_draw_count(&self) -> u32 {
        self.extra_blend_draw_count.load(Ordering::Relaxed)
    }
}
