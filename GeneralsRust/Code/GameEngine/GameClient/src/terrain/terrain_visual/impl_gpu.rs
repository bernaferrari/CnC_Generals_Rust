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
        self.ensure_water_texture_bind_group(device.as_ref());
        self.create_river_pipeline(device.as_ref())?;
        self.ensure_river_bind_group(device.as_ref());
        self.create_shroud_overlay_pipelines(device.as_ref())?;
        self.sync_shroud_dest_texture();

        // Create road render pipeline
        self.create_road_pipeline(device.as_ref())?;
        self.ensure_road_texture_bind_group(device.as_ref());
        self.ensure_snow_texture_bind_group(device.as_ref());
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

    /// Chunks overlapping the camera XZ window (draw-area plus live look-at).
    /// Used when frustum visibility is empty so the ground under the camera
    /// still gets mesh uploads and draws.
    fn chunk_ids_intersecting_camera_xz_window(&self) -> Vec<ChunkId> {
        let (draw_min_x, draw_min_z, draw_max_x, draw_max_z) = self.draw_area_bounds_world();
        let scale = self.map_scale().max(f32::EPSILON);
        let half_x = (self.draw_width.max(1) as f32) * scale * 0.5;
        let half_z = (self.draw_height.max(1) as f32) * scale * 0.5;
        let (look_x, look_z) = crate::display::view::with_tactical_view_ref(|view| {
            let look = view.position();
            // Terrain GPU is Y-up (x,z). Tactical View may still store C++ XY ground.
            if look.z.abs() > look.y.abs() {
                (look.x, look.z)
            } else {
                (look.x, look.y)
            }
        });
        let cam_min_x = look_x - half_x;
        let cam_max_x = look_x + half_x;
        let cam_min_z = look_z - half_z;
        let cam_max_z = look_z + half_z;

        let intersects = |min_x: f32, min_z: f32, max_x: f32, max_z: f32, chunk: &crate::terrain::chunk::TerrainChunk| {
            chunk.bounds.max.x > min_x
                && chunk.bounds.min.x < max_x
                && chunk.bounds.max.z > min_z
                && chunk.bounds.min.z < max_z
        };

        let mut ids: Vec<ChunkId> = self
            .chunk_manager
            .iter_chunks()
            .filter(|chunk| {
                intersects(draw_min_x, draw_min_z, draw_max_x, draw_max_z, chunk)
                    || intersects(cam_min_x, cam_min_z, cam_max_x, cam_max_z, chunk)
            })
            .map(|chunk| chunk.id)
            .collect();
        ids.sort_unstable();
        ids
    }

    fn chunk_ids_for_gpu_draw(&self) -> Vec<ChunkId> {
        let mut ids = self.visible_chunk_ids_for_draw_area();
        // Frustum-visible can be a stale/identity set at the origin. Always
        // union the live camera XZ window so Alpine under the CC still uploads.
        for id in self.chunk_ids_intersecting_camera_xz_window() {
            if !ids.contains(&id) {
                ids.push(id);
            }
        }
        ids.sort_unstable();
        ids.dedup();
        ids
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

        let visible_chunk_ids = self.chunk_ids_for_gpu_draw();
        let select_slots_started = std::time::Instant::now();
        // Recompute every pass so late-loaded source_tile_classes (map BlendTileData)
        // remesh instead of keeping the first cobble-only slot set.
        let stable_texture_ids = self.select_stable_chunk_texture_ids(&visible_chunk_ids);
        let refresh_texture_slots = self.active_chunk_texture_ids != Some(stable_texture_ids);
        if refresh_texture_slots {
            self.active_chunk_texture_ids = Some(stable_texture_ids);
        }
        let select_slots_elapsed = select_slots_started.elapsed();

        let texture_slots_changed = refresh_texture_slots;
        let scene_lights = scene_dynamic_lights();
        let lights_active = scene_lights.iter().any(|light| light.enabled);
        // C++ HeightMap::doTheDynamicLight rebakes every VB fill. Live GPU
        // meshes only remesh on revision/texture change, so force one rebuild
        // while pulses are live and one more after they expire.
        let needs_light_rebake = lights_active || self.had_dynamic_lights;

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
                Some(mesh) => {
                    mesh.revision != chunk_revision
                        || texture_slots_changed
                        || needs_light_rebake
                }
                None => true,
            };

            if needs_mesh_upload {
                if !has_chunk_geometry {
                    continue;
                }
                let upload_started = std::time::Instant::now();

                let (chunk_vertices, chunk_indices, chunk_base_colors) =
                    match self.chunk_manager.get_chunk(chunk_id) {
                        Some(chunk) => (
                            chunk.vertices.clone(),
                            chunk.indices.clone(),
                            chunk.base_colors.clone(),
                        ),
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

                    // C++ WorldHeightMap::getTextureClassFromNdx maps tileNdx>>2
                    // through m_textureClasses firstTile/numTiles. Weights carry
                    // TextureIds so shared_slot_map (keyed by TextureId, not
                    // tile%4) can place adjacent vertices on different bound
                    // slots. tile 0 is a valid class — do not treat it as empty.
                    // Prefer blend_tile_ndxes when the map wired them.
                    let tile_sample = self.height_map.as_ref().map(|hm| {
                        let packed =
                            hm.get_packed_terrain_tile_at_world(position.x, position.z);
                        let (ix, iy) = height_map_cell_at_world(hm, position.x, position.z);
                        let blend_i = hm.get_blend_tile_index(ix, iy);
                        let other_packed = if blend_i != 0 {
                            hm.blended_tiles.get(blend_i as usize).map(|blend| {
                                (blend.blend_ndx >> 2).max(0) as u32
                            })
                        } else {
                            None
                        };
                        (packed, blend_i, other_packed)
                    });

                    let base_weights = self.texture_system.generate_texture_weights(
                        height,
                        slope,
                        vertex.tex_coords,
                        &self.texture_rules,
                    );

                    let blended = if let Some((packed, blend_i, other_packed)) = tile_sample {
                        let base_id = bound_texture_id_for_source_tile(
                            packed,
                            &self.source_tile_classes,
                            &self.texture_system,
                            &stable_texture_ids,
                            &shared_slot_map,
                        );
                        if blend_i != 0 {
                            let other_id = other_packed.and_then(|other| {
                                bound_texture_id_for_source_tile(
                                    other,
                                    &self.source_tile_classes,
                                    &self.texture_system,
                                    &stable_texture_ids,
                                    &shared_slot_map,
                                )
                            });
                            match (base_id, other_id) {
                                (Some(a), Some(b)) if a != b => {
                                    TextureWeights::blend_two(a, b, 0.5)
                                }
                                (Some(a), _) => TextureWeights::single(a),
                                (_, Some(b)) => TextureWeights::single(b),
                                _ => self.texture_system.blend_textures_at_position(
                                    position,
                                    height,
                                    normal,
                                    vertex.tex_coords,
                                    &base_weights,
                                    &self.texture_rules,
                                ),
                            }
                        } else if let Some(id) = base_id {
                            TextureWeights::single(id)
                        } else {
                            self.texture_system.blend_textures_at_position(
                                position,
                                height,
                                normal,
                                vertex.tex_coords,
                                &base_weights,
                                &self.texture_rules,
                            )
                        }
                    } else {
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

                for (vert_index, (vertex, blended)) in
                    gpu_vertices.iter_mut().zip(vertex_weights.iter()).enumerate()
                {
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
                    if let Some(hm) = self.height_map.as_ref() {
                        vertex.tex_coords =
                            hm.cell_uv_at_world(vertex.position[0], vertex.position[2]);
                    }
                    let shroud = self
                        .shroud_alpha_at_world(vertex.position[0], vertex.position[2]);
                    // C++ vbMirror / getStaticDiffuse, then doTheDynamicLight.
                    // Leftover chunk.vertices may already be pulse-lit; start
                    // from base_colors so a live remesh does not double-apply.
                    let mut color = chunk_base_colors
                        .get(vert_index)
                        .copied()
                        .unwrap_or(vertex.color);
                    color[0] *= shroud;
                    color[1] *= shroud;
                    color[2] *= shroud;
                    vertex.color = Self::bake_terrain_vertex_dynamic_light(
                        vertex.position,
                        vertex.normal,
                        color,
                        &scene_lights,
                    );
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
        self.had_dynamic_lights = lights_active;

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

        // Bind map texture classes first so adjacent firstTile/numTiles classes
        // land on different TextureId keys (not tile%4).
        for class in &self.source_tile_classes {
            if selected_textures.len() == MAX_TEXTURES_PER_CHUNK {
                break;
            }
            if let Some(texture_id) = self.texture_id_for_class_name(&class.name) {
                if selected_textures
                    .iter()
                    .all(|candidate| *candidate != texture_id)
                {
                    selected_textures.push(texture_id);
                }
            }
        }

        for rule in &self.texture_rules {
            if selected_textures.len() == MAX_TEXTURES_PER_CHUNK {
                break;
            }
            let texture_id = rule.texture_id;
            if texture_id == 0 || self.texture_system.get_texture(texture_id).is_none() {
                continue;
            }
            if selected_textures
                .iter()
                .all(|candidate| *candidate != texture_id)
            {
                selected_textures.push(texture_id);
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

    fn texture_id_for_class_name(&self, class_name: &str) -> Option<TextureId> {
        for rule in &self.texture_rules {
            if let Some(texture) = self.texture_system.get_texture(rule.texture_id) {
                if texture_matches_class_name(texture, class_name) {
                    return Some(rule.texture_id);
                }
            }
        }
        let Some(first) = self.texture_system.first_texture_id() else {
            return None;
        };
        for id in first..=first.saturating_add(255) {
            if let Some(texture) = self.texture_system.get_texture(id) {
                if texture_matches_class_name(texture, class_name) {
                    return Some(id);
                }
            }
        }
        None
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

        // Curve/join geometry is authored on the map plane. C++
        // `loadFloat4PtSection` / `updateSegLighting` project those verts with
        // `getMaxCellHeight`. `apply_terrain_heights_and_normals` is the live
        // equivalent and otherwise has no production callers.
        self.road_system.invalidate_terrain_lighting();
        if let Some(height_map) = self.height_map.as_ref() {
            self.road_system.apply_terrain_heights_and_normals(
                |pos| height_map.get_height_at(pos.x, pos.z),
                |pos| height_map.get_normal_at(pos.x, pos.z),
            );
        }

        let mut road_meshes = Vec::new();
        let mut bridge_meshes = Vec::new();
        let height_map = self.height_map.as_ref();
        let height_at = |x: f32, y: f32| {
            height_map
                .map(|height_map| height_map.get_height_at(x, y))
                .unwrap_or(0.0)
        };
        let sun_direction = self.sun_direction;
        let sun_color = self.sun_color;
        let ambient_color = self.ambient_color;



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
                            let mut gpu_vertices = fill_road_gpu_upload_vertices(&verts);
                            Self::apply_road_vertex_static_diffuse(
                                &mut gpu_vertices,
                                height_map,
                                sun_direction,
                                sun_color,
                                ambient_color,
                            );
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
                let mut gpu_vertices: Vec<RoadVertex> = geometry
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
                // Belt-and-suspenders: keep join/curve strips on the height
                // field even if RoadManager geometry was authored at Y=0.
                for vertex in &mut gpu_vertices {
                    vertex.position[1] =
                        height_at(vertex.position[0], vertex.position[2]) + ROAD_FLOAT_AMOUNT;
                }

                Self::apply_road_vertex_static_diffuse(
                    &mut gpu_vertices,
                    height_map,
                    sun_direction,
                    sun_color,
                    ambient_color,
                );
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
        self.ensure_scorch_texture_bind_group(device);
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

    /// C++ `RoadSegment::updateSegLighting`: `getStaticDiffuse` into vertex color.
    /// `loadFloat4PtSection` leaves `diffuse=0`, which unpacks to black and
    /// produced the pitch-black road strips. Floor at 0.35 so unlit verts stay visible.
    fn apply_road_vertex_static_diffuse(
        vertices: &mut [OverlayGpuVertex],
        height_map: Option<&HeightMap>,
        sun_direction: Vec3,
        sun_color: [f32; 3],
        ambient_color: [f32; 3],
    ) {
        const COLOR_FLOOR: f32 = 0.35;
        for vertex in vertices {
            let normal = height_map
                .map(|height_map| height_map.get_normal_at(vertex.position[0], vertex.position[2]))
                .unwrap_or(Vec3::Y);
            let lit = Self::terrain_static_diffuse_from_normal(
                normal,
                sun_direction,
                sun_color,
                ambient_color,
            );
            vertex.color = [
                lit[0].max(COLOR_FLOOR),
                lit[1].max(COLOR_FLOOR),
                lit[2].max(COLOR_FLOOR),
            ];
            let r = (vertex.color[0] * 255.0).round() as u32;
            let g = (vertex.color[1] * 255.0).round() as u32;
            let b = (vertex.color[2] * 255.0).round() as u32;
            vertex.diffuse = 0xFF00_0000 | (r << 16) | (g << 8) | b;
        }
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

            let draw_ids = self.chunk_ids_for_gpu_draw();
            for chunk_id in draw_ids {
                let Some(binding) = self.chunk_texture_bindings.get(&chunk_id) else {
                    continue;
                };
                let Some(mesh) = self.chunk_meshes.get(&chunk_id) else {
                    continue;
                };

                pass.set_bind_group(1, &binding.bind_group, &[]);
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(
                    mesh.index_buffer.slice(..),
                    wgpu::IndexFormat::Uint32,
                );
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }

        }
        self.record_extra_blend_pass(pass);
        self.record_road_draws(pass);
        self.record_overlay_draws(pass);
        self.record_tree_draws(pass);
        self.record_water_draws(pass);
        self.record_extra_water_draws(pass);
        self.record_shroud_water_pass(pass);
        self.record_shroud_tree_pass(pass);
        self.record_shroud_bridge_pass(pass);
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
        let pipeline = if self.water_additive_blend {
            self.water_additive_pipeline
                .as_ref()
                .or(self.water_pipeline.as_ref())
        } else {
            self.water_pipeline.as_ref()
        };
        let (Some(water_pipeline), Some(camera_bg), Some(water_bg)) = (
            pipeline,
            self.terrain_camera_bind_group.as_ref(),
            self.water_texture_bind_group.as_ref(),
        ) else {
            return;
        };

        pass.set_pipeline(water_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        pass.set_bind_group(1, water_bg, &[]);

        if let Some(water_plane) = self.water_plane.as_ref() {
            pass.set_vertex_buffer(0, water_plane.vertex_buffer.slice(..));
            pass.set_index_buffer(
                water_plane.index_buffer.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..water_plane.index_count, 0, 0..1);
        }

        // C++ WaterTracksRenderSystem::flush switches stage-0 texture per type.
        let mut last_tex = String::new();
        for mesh in &self.water_track_meshes {
            if mesh.texture_name != last_tex {
                if let Some(named) = self.water_named_bind_groups.get(&mesh.texture_name) {
                    pass.set_bind_group(1, &named.bind_group, &[]);
                }
                last_tex = mesh.texture_name.clone();
            }
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn record_road_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(road_pipeline), Some(camera_bg)) = (
            self.road_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };

        if (!self.road_meshes.is_empty() || !self.bridge_meshes.is_empty())
            && let Some(road_bg) = self.road_texture_bind_group.as_ref()
        {
            pass.set_pipeline(road_pipeline);
            pass.set_bind_group(0, camera_bg, &[]);
            pass.set_bind_group(1, road_bg, &[]);
            for mesh in self.road_meshes.iter().chain(self.bridge_meshes.iter()) {
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }

        // C++ drawScorches: Set_Texture(0, m_scorchTexture) = EXScorch01.tga.
        if !self.scorch_meshes.is_empty()
            && let Some(scorch_bg) = self.scorch_texture_bind_group.as_ref()
        {
            pass.set_pipeline(road_pipeline);
            pass.set_bind_group(0, camera_bg, &[]);
            pass.set_bind_group(1, scorch_bg, &[]);
            for mesh in &self.scorch_meshes {
                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..mesh.index_count, 0, 0..1);
            }
        }
    }

    fn ensure_road_texture_bind_group(&mut self, device: &wgpu::Device) {
        if self.road_texture_bind_group.is_some() && !self.road_texture_is_fallback {
            return;
        }
        // Same negative cache as water: after one failed search the gravel
        // fallback stays bound instead of re-probing Roads.ini candidates
        // every frame.
        if self.road_texture_search_exhausted {
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
        self.road_texture_search_exhausted = used_fallback;

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

    fn wanted_snow_texture_name() -> String {
        crate::snow::get_weather_setting()
            .and_then(|s| s.read().ok().map(|g| g.snow_texture.clone()))
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "EXSnowFlake.tga".to_string())
    }

    fn ensure_snow_texture_bind_group(&mut self, device: &wgpu::Device) {
        let wanted = Self::wanted_snow_texture_name();
        if self.snow_texture_bind_group.is_some()
            && !self.snow_texture_is_fallback
            && self.snow_texture_name.eq_ignore_ascii_case(&wanted)
        {
            return;
        }
        // Negative cache for the current wanted name: a failed search keeps
        // the white fallback bound instead of re-probing per frame. A changed
        // Weather.ini snow texture name re-opens the search.
        if self.snow_texture_search_exhausted && self.snow_texture_name.eq_ignore_ascii_case(&wanted)
        {
            return;
        }
        let Some(layout) = self.road_texture_bind_group_layout.clone() else {
            return;
        };
        let Some(queue) = self.queue.clone() else {
            return;
        };

        let (texture, used_fallback, source_name) =
            match self.load_first_available_named_overlay_texture(device, &wanted) {
                Some((texture, name)) => (texture, false, name),
                None => (
                    Self::create_fallback_snow_texture(device, queue.as_ref()),
                    true,
                    "fallback-snowflake-1x1".to_string(),
                ),
            };
        self.snow_texture_search_exhausted = used_fallback;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        if self.snow_sampler.is_none() {
            self.snow_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Snow Texture Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let Some(sampler) = self.snow_sampler.as_ref() else {
            return;
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Snow Texture Bind Group"),
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
        self.snow_texture = Some(texture);
        self.snow_texture_bind_group = Some(bind_group);
        self.snow_texture_name = wanted;
        self.snow_texture_is_fallback = used_fallback;
        if used_fallback {
            warn!("Snow texture missing; using white 1x1 flake fallback ({source_name})");
        }
    }

    fn wanted_scorch_texture_name() -> &'static str {
        "EXScorch01.tga"
    }

    fn ensure_scorch_texture_bind_group(&mut self, device: &wgpu::Device) {
        let wanted = Self::wanted_scorch_texture_name();
        if self.scorch_texture_bind_group.is_some()
            && !self.scorch_texture_is_fallback
            && self.scorch_texture_name.eq_ignore_ascii_case(wanted)
        {
            return;
        }
        // Negative cache for the (static) wanted scorch name, same shape as snow.
        if self.scorch_texture_search_exhausted && self.scorch_texture_name.eq_ignore_ascii_case(wanted)
        {
            return;
        }
        let Some(layout) = self.road_texture_bind_group_layout.clone() else {
            return;
        };
        let Some(queue) = self.queue.clone() else {
            return;
        };

        let (texture, used_fallback, source_name) =
            match self.load_first_available_named_overlay_texture(device, wanted) {
                Some((texture, name)) => (texture, false, name),
                None => (
                    Self::create_fallback_scorch_texture(device, queue.as_ref()),
                    true,
                    "fallback-exscorch-4x4".to_string(),
                ),
            };
        self.scorch_texture_search_exhausted = used_fallback;
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        if self.scorch_sampler.is_none() {
            self.scorch_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Scorch Texture Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let Some(sampler) = self.scorch_sampler.as_ref() else {
            return;
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Scorch EXScorch01 Bind Group"),
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
        self.scorch_texture = Some(texture);
        self.scorch_texture_bind_group = Some(bind_group);
        self.scorch_texture_name = wanted.to_string();
        self.scorch_texture_is_fallback = used_fallback;
        if used_fallback {
            warn!("EXScorch01 missing; using 4x4 burn-atlas fallback ({source_name})");
        }
    }

    fn create_fallback_scorch_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        // 4x4 3x3-atlas analog (C++ SCORCH_PER_ROW+1). Dark brown, not road gravel.
        let mut pixels = [0u8; 4 * 4 * 4];
        for y in 0..4 {
            for x in 0..4 {
                let i = (y * 4 + x) * 4;
                let in_tile = x < 3 && y < 3;
                if in_tile {
                    pixels[i] = 42 + (x as u8) * 8;
                    pixels[i + 1] = 28 + (y as u8) * 6;
                    pixels[i + 2] = 18;
                    pixels[i + 3] = 210;
                } else {
                    pixels[i + 3] = 0;
                }
            }
        }
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Scorch Fallback EXScorch 4x4"),
            size: wgpu::Extent3d {
                width: 4,
                height: 4,
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
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(16),
                rows_per_image: Some(4),
            },
            wgpu::Extent3d {
                width: 4,
                height: 4,
                depth_or_array_layers: 1,
            },
        );
        texture
    }

    fn load_first_available_named_overlay_texture(
        &self,
        device: &wgpu::Device,
        name: &str,
    ) -> Option<(wgpu::Texture, String)> {
        for path in Self::water_texture_path_candidates(name) {
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
        None
    }

    fn create_fallback_snow_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        const PIXELS: [u8; 4] = [255, 255, 255, 220];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Snow Fallback White 1x1"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
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
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        texture
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

    fn ensure_water_texture_bind_group(&mut self, device: &wgpu::Device) {
        if self.water_texture_bind_group.is_some() && !self.water_texture_is_fallback {
            return;
        }
        // A failed search stays failed: the teal fallback bind group is bound
        // and re-probing every frame re-read candidate paths per render call.
        // `apply_water_transparency_map_overrides` clears this when the INI
        // standing-water name changes.
        if self.water_texture_search_exhausted {
            return;
        }
        let Some(layout) = self.water_texture_bind_group_layout.clone() else {
            return;
        };
        let Some(queue) = self.queue.clone() else {
            return;
        };

        let (texture, used_fallback, source_name) =
            match self.load_first_available_water_texture(device) {
                Some((texture, name)) => (texture, false, name),
                None => (
                    Self::create_fallback_water_texture(device, queue.as_ref()),
                    true,
                    "fallback-teal-1x1".to_string(),
                ),
            };
        self.water_texture_search_exhausted = used_fallback;

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        if self.water_sampler.is_none() {
            self.water_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Water Texture Sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let Some(sampler) = self.water_sampler.as_ref() else {
            return;
        };
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water Texture Bind Group"),
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
            warn!("Water texture missing; using teal-alpha 1x1 fallback");
        } else {
            info!("Water texture bind group ready using {}", source_name);
        }
        self.water_texture = Some(texture);
        self.water_texture_bind_group = Some(bind_group);
        self.water_texture_is_fallback = used_fallback;
        self.bound_standing_water_texture = source_name;
    }

    /// C++ `replaceSkyboxTextures` + `WaterRenderObjClass::updateMapOverrides`.
    fn apply_water_transparency_map_overrides(&mut self, device: &wgpu::Device) {
        if let Some((old, new)) = game_engine::common::ini::ini_water::take_pending_skybox_replace()
        {
            let old_refs = [
                old[0].as_str(),
                old[1].as_str(),
                old[2].as_str(),
                old[3].as_str(),
                old[4].as_str(),
            ];
            let new_refs = [
                new[0].as_str(),
                new[1].as_str(),
                new[2].as_str(),
                new[3].as_str(),
                new[4].as_str(),
            ];
            if let Err(err) = self.replace_skybox_textures(&old_refs, &new_refs) {
                warn!("WaterTransparency skybox replace failed: {err}");
            }
        }

        game_engine::common::ini::ini_water::initialize_water_settings();
        let wanted = game_engine::common::ini::ini_water::get_water_transparency()
            .and_then(|lock| {
                lock.read().ok().map(|g| {
                    g.get_final_override()
                        .standing_water_texture
                        .to_string()
                })
            })
            .unwrap_or_default();
        // C++ applies the INI standing-water override when the map/Water.ini
        // changes it, not per frame. Rebind only when the requested name
        // differs from the last attempt: the loaded texture path (e.g.
        // Art/Textures/water01.dds) rarely contains the INI name
        // (TWWater01.tga), so the old contains() test forced a full
        // DDS re-decode + upload every frame (~220 ms of the terrain pass).
        if !wanted.is_empty() && wanted != self.requested_standing_water_texture {
            self.requested_standing_water_texture = wanted;
            self.water_texture_bind_group = None;
            self.water_texture_is_fallback = true;
            self.water_texture_search_exhausted = false;
            self.ensure_water_texture_bind_group(device);
        }
    }


    fn load_first_available_water_texture(
        &self,
        device: &wgpu::Device,
    ) -> Option<(wgpu::Texture, String)> {
        for name in self.water_texture_name_candidates() {
            for path in Self::water_texture_path_candidates(&name) {
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

    fn water_texture_name_candidates(&self) -> Vec<String> {
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

        // C++ Water.h / Water.ini StandingWaterTexture default, then WaterSet WaterTexture.
        game_engine::common::ini::ini_water::initialize_water_settings();
        if let Some(lock) = game_engine::common::ini::ini_water::get_water_transparency() {
            if let Ok(guard) = lock.read() {
                push_name(guard.get_final_override().standing_water_texture.as_str());
            }
        }
        if let Some(global_data) = get_global_data() {
            let tod = game_engine::common::ini::ini_water::TimeOfDay::from_index(
                global_data.read().time_of_day as usize,
            );
            if let Some(lock) = game_engine::common::ini::ini_water::get_water_setting(tod) {
                if let Ok(guard) = lock.read() {
                    push_name(guard.texture_name.as_str());
                }
            }
        }
        for name in ["TWWater01.tga", "TWWater01.dds", "water01.dds", "Water01.tga"] {
            push_name(name);
        }
        names
    }

    fn water_texture_path_candidates(name: &str) -> Vec<String> {
        let basename = Path::new(name)
            .file_name()
            .and_then(|file| file.to_str())
            .unwrap_or(name);
        let mut paths = vec![
            format!("{TGA_DIR_PATH}{basename}"),
            format!("art/textures/{basename}"),
            format!("ART/Textures/{basename}"),
            format!("{TERRAIN_TGA_DIR_PATH}{basename}"),
            basename.to_string(),
        ];
        if name != basename {
            paths.insert(0, name.replace('\\', "/"));
        }
        paths
    }

    fn create_fallback_water_texture(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        // Teal-alpha 1x1 so the plane is visible when TWWater01 is missing.
        const PIXELS: [u8; 4] = [33, 107, 128, 178];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Water Fallback Teal 1x1"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
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
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        texture
    }


    fn record_tree_draws<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(tree_pipeline), Some(camera_bg), Some(atlas_bg)) = (
            self.tree_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.tree_atlas_bind_group.as_ref(),
        ) else {
            return;
        };
        if self.tree_meshes.is_empty() {
            return;
        }

        pass.set_pipeline(tree_pipeline);
        pass.set_bind_group(0, camera_bg, &[]);
        // Own atlas group. Do not overwrite road/terrain bind group 1 after this pass.
        pass.set_bind_group(1, atlas_bg, &[]);
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
        self.rebind_skybox_background_for_camera();
        self.tree_buffer.tick_cpu(false, |_| TreeShroudStatus::Clear);
        if self.tree_buffer.take_any_push_changed() {
            self.tree_buffer.force_vertex_rebuild();
        }
        // C++ `W3DTreeBuffer::drawTrees`: ScriptEngine breeze + versioned updateSway.
        let pause = gamelogic::helpers::TheScriptEngine::is_time_frozen()
            || gamelogic::helpers::TheGameLogic::is_game_paused();
        if !pause {
            let breeze = script_tree_breeze();
            if self.tree_buffer.cur_sway_version() != breeze.breeze_version {
                self.tree_buffer
                    .update_sway(breeze, &mut GameClientSwayRng);
                self.overlay.last_sway_version = breeze.breeze_version;
            }
        }
        if self.tree_meshes.is_empty() {
            if self
                .tree_buffer
                .trees()
                .iter()
                .any(|tree| tree.tree_type >= 0)
            {
                self.tree_buffer.force_vertex_rebuild();
            } else if !self.tree_buffer.need_to_update_texture() {
                return;
            }
        }

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
            self.tree_atlas_bind_group = None;
            return;
        };
        let levels = self.tree_buffer.atlas_upload_levels();
        if levels.is_empty() {
            self.tree_atlas_texture = Some(Self::create_fallback_tree_atlas(device, queue));
            self.bind_tree_atlas_group();
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
        self.bind_tree_atlas_group();
    }

    fn create_fallback_tree_atlas(device: &wgpu::Device, queue: &wgpu::Queue) -> wgpu::Texture {
        const PIXELS: [u8; 16] = [
            40, 90, 28, 255, 32, 78, 22, 255, 36, 84, 24, 255, 28, 70, 18, 255,
        ];
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("W3D Tree Atlas Fallback"),
            size: wgpu::Extent3d {
                width: 2,
                height: 2,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8Unorm,
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

    fn bind_tree_atlas_group(&mut self) {
        let (Some(device), Some(layout), Some(texture)) = (
            self.device.as_ref(),
            self.tree_atlas_bind_group_layout.as_ref(),
            self.tree_atlas_texture.as_ref(),
        ) else {
            self.tree_atlas_bind_group = None;
            return;
        };
        if self.tree_atlas_sampler.is_none() {
            self.tree_atlas_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Tree Atlas Sampler"),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::FilterMode::Linear,
                ..Default::default()
            }));
        }
        let Some(sampler) = self.tree_atlas_sampler.as_ref() else {
            self.tree_atlas_bind_group = None;
            return;
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        self.tree_atlas_bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Tree Atlas Bind Group"),
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
        }));
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

        // C++ WaterRenderObjClass::getClippedWaterPlane (W3DWater.cpp:1735-1738)
        // spans local [0..m_dx] x [0..m_dy] at m_level, then Set_Position
        // (waterPositionX, waterPositionY, waterPositionZ). Not centered
        // [-extent/2, +extent/2] — Alpine camera x=1362 must sit over the plane.
        let water_y = global.water_position_z;
        let min_x = global.water_position_x;
        let min_z = global.water_position_y;
        let max_x = global.water_position_x + global.water_extent_x;
        let max_z = global.water_position_y + global.water_extent_y;
        // Tile 15×15 patches across GlobalData extents (do not stretch one sheet).
        let (standing_color, additive, standing_tex) = {
            game_engine::common::ini::ini_water::initialize_water_settings();
            game_engine::common::ini::ini_water::get_water_transparency()
                .and_then(|lock| lock.read().ok().map(|g| {
                    let final_s = g.get_final_override();
                    (
                        [
                            final_s.standing_water_color.0,
                            final_s.standing_water_color.1,
                            final_s.standing_water_color.2,
                        ],
                        final_s.additive_blending,
                        final_s.standing_water_texture.to_string(),
                    )
                }))
                .unwrap_or(([1.0, 1.0, 1.0], false, String::new()))
        };
        self.water_additive_blend = additive;

        let tod = game_engine::common::ini::ini_water::TimeOfDay::from_index(
            global.time_of_day as usize,
        );
        let water_set = game_engine::common::ini::ini_water::get_water_setting(tod)
            .and_then(|lock| lock.read().ok().map(|g| g.clone()));
        let water_diffuse = water_set
            .as_ref()
            .map(|s| pack_water_rgba_int(s.surface_color))
            .unwrap_or(0xffff_ffff);
        let packed = compute_standing_water_diffuse(
            standing_color,
            water_diffuse,
            self.ambient_color,
            &[WaterTerrainLight {
                light_pos: [self.sun_direction.x, self.sun_direction.z, self.sun_direction.y],
                diffuse: self.sun_color,
            }],
        );

        let (mut patch_vertices, list_indices) =
            bake_water_tiles_world(min_x, min_z, max_x, max_z, water_y, packed);
        if let Some(setting) = water_set.as_ref() {
            let time_ms = self.time * 1000.0;
            let repeat = if setting.water_repeat_count > 0 {
                setting.water_repeat_count as f32 / 16.0
            } else {
                1.0
            };
            let su = setting.u_scroll_per_ms * time_ms;
            let sv = setting.v_scroll_per_ms * time_ms;
            for vertex in &mut patch_vertices {
                vertex.tu = vertex.tu * repeat + su;
                vertex.tv = vertex.tv * repeat + sv;
            }
        }
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
            texture_name: standing_tex,
            jba: false,
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
                color = Self::bake_terrain_vertex_dynamic_light(
                    vertex.position,
                    [normal.x, normal.y, normal.z],
                    color,
                    &scene_dynamic_lights(),
                );
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

    /// C++ `HeightMapRenderObjClass::doTheDynamicLight` on Y-up wgpu verts.
    /// Map coords are C++ Z-up: `(x, y, z) = (wgpu.x, wgpu.z, wgpu.y)`.
    pub(crate) fn bake_terrain_vertex_dynamic_light(
        position_yup: [f32; 3],
        normal_yup: [f32; 3],
        static_rgba: [f32; 4],
        lights: &[DisplayDynamicLight],
    ) -> [f32; 4] {
        if lights.is_empty() {
            return static_rgba;
        }
        let xyz = [position_yup[0], position_yup[2], position_yup[1]];
        let nrm = [normal_yup[0], normal_yup[2], normal_yup[1]];
        let packed = rgba_f32_to_bgra_u32(static_rgba);
        bgra_u32_to_rgba_f32(do_the_dynamic_light(xyz, nrm, packed, lights))
    }

    /// C++ `drawScorches` → `updateScorches` when `m_scorchesInBuffer == 0`.
    pub(crate) fn scorches_need_gpu_rebuild(&self) -> bool {
        let in_buffer = terrain_scorches_in_buffer();
        let count = terrain_scorch_count();
        if count == 0 {
            return !self.scorch_meshes.is_empty();
        }
        in_buffer == 0 || self.scorch_meshes.is_empty()
    }

    pub(crate) fn scorch_overlay_texture_name() -> &'static str {
        Self::wanted_scorch_texture_name()
    }

    pub(crate) fn scorch_bind_group_ready(&self) -> bool {
        self.scorch_texture_bind_group.is_some()
    }

    pub(crate) fn last_scorch_texture_name(&self) -> &str {
        &self.scorch_texture_name
    }
}

fn rgba_f32_to_bgra_u32(color: [f32; 4]) -> u32 {
    let r = (color[0].clamp(0.0, 1.0) * 255.0) as u32;
    let g = (color[1].clamp(0.0, 1.0) * 255.0) as u32;
    let b = (color[2].clamp(0.0, 1.0) * 255.0) as u32;
    let a = (color[3].clamp(0.0, 1.0) * 255.0) as u32;
    b | (g << 8) | (r << 16) | (a << 24)
}

fn bgra_u32_to_rgba_f32(packed: u32) -> [f32; 4] {
    let b = (packed & 0xFF) as f32 / 255.0;
    let g = ((packed >> 8) & 0xFF) as f32 / 255.0;
    let r = ((packed >> 16) & 0xFF) as f32 / 255.0;
    let a = ((packed >> 24) & 0xFF) as f32 / 255.0;
    [r, g, b, a]
}

/// Live `GameClientRandomValue` stream for C++ `W3DTreeBuffer::updateSway`.
struct GameClientSwayRng;

impl crate::terrain::TreeRandom for GameClientSwayRng {
    fn int_range(&mut self, min: i32, max: i32) -> i32 {
        crate::client_random_value::get_game_client_random_value(min, max, file!(), line!())
    }

    fn real_range(&mut self, min: f32, max: f32) -> f32 {
        crate::client_random_value::get_game_client_random_value_real(min, max, file!(), line!())
    }
}

/// C++ `TheScriptEngine->getBreezeInfo()`, or ScriptEngine reset defaults.
fn script_tree_breeze() -> crate::terrain::BreezeInfo {
    if let Ok(guard) = gamelogic::get_script_engine().read() {
        if let Some(engine) = guard.as_ref() {
            return tree_breeze_from_script(&engine.get_breeze_info());
        }
    }
    tree_breeze_from_script(&gamelogic::scripting::engine::BreezeInfo::new())
}

fn tree_breeze_from_script(
    info: &gamelogic::scripting::engine::BreezeInfo,
) -> crate::terrain::BreezeInfo {
    crate::terrain::BreezeInfo {
        breeze_version: i32::from(info.breeze_version),
        lean: info.lean,
        intensity: info.intensity,
        direction_vec: glam::Vec2::new(info.direction_vec[0], info.direction_vec[1]),
        randomness: info.randomness,
        breeze_period: i32::from(info.breeze_period.max(1)),
    }
}

fn height_map_cell_at_world(hm: &HeightMap, world_x: f32, world_z: f32) -> (i32, i32) {
    let max_x = hm.width.saturating_sub(1) as i32;
    let max_y = hm.height.saturating_sub(1) as i32;
    let scale = if hm.scale.abs() <= f32::EPSILON {
        1.0
    } else {
        hm.scale
    };
    let x = ((world_x / scale).floor() as i32 + hm.border_size).clamp(0, max_x);
    let y = ((world_z / scale).floor() as i32 + hm.border_size).clamp(0, max_y);
    (x, y)
}

fn unique_bound_texture_ids(
    stable_ids: &[TextureId; MAX_TEXTURES_PER_CHUNK],
    shared_slot_map: &HashMap<TextureId, usize>,
) -> Vec<TextureId> {
    let mut unique = Vec::new();
    for &id in stable_ids {
        if !shared_slot_map.contains_key(&id) {
            continue;
        }
        if unique.iter().all(|existing| *existing != id) {
            unique.push(id);
        }
    }
    unique
}

fn texture_class_index_from_ndx(
    packed_tile: u32,
    classes: &[TerrainSourceTileClass],
) -> Option<usize> {
    // packed_tile is already tileNdx>>2 (HeightMap::get_packed_terrain_tile_at_world).
    // C++ WorldHeightMap::getTextureClassFromNdx walks firstTile/numTiles;
    // tile 0 is a valid class when firstTile == 0.
    let tile = packed_tile as i32;
    for (idx, class) in classes.iter().enumerate() {
        if class.first_tile < 0 || class.num_tiles <= 0 {
            continue;
        }
        if tile >= class.first_tile && tile < class.first_tile + class.num_tiles {
            return Some(idx);
        }
    }
    None
}

#[allow(dead_code)]
fn source_tile_class_contains(classes: &[TerrainSourceTileClass], packed_tile: u32) -> bool {
    texture_class_index_from_ndx(packed_tile, classes).is_some()
}

fn texture_matches_class_name(texture: &TerrainTexture, class_name: &str) -> bool {
    let class = class_name.trim();
    if class.is_empty() {
        return false;
    }
    if texture.name.eq_ignore_ascii_case(class) {
        return true;
    }
    let class_l = class.to_ascii_lowercase();
    let path = texture.diffuse_path.replace('\\', "/").to_ascii_lowercase();
    if path.contains(&class_l) {
        return true;
    }
    std::path::Path::new(&texture.diffuse_path)
        .file_stem()
        .and_then(|stem| stem.to_str())
        .is_some_and(|stem| stem.eq_ignore_ascii_case(class))
}

fn bound_texture_id_for_source_tile(
    packed_tile: u32,
    classes: &[TerrainSourceTileClass],
    textures: &TerrainTextures,
    stable_ids: &[TextureId; MAX_TEXTURES_PER_CHUNK],
    shared_slot_map: &HashMap<TextureId, usize>,
) -> Option<TextureId> {
    let unique = unique_bound_texture_ids(stable_ids, shared_slot_map);
    if unique.is_empty() {
        return None;
    }

    let Some(class_idx) = texture_class_index_from_ndx(packed_tile, classes) else {
        // Do not treat the tile index as a slot (tile%4 / tile/4). Unmapped
        // tiles leave TextureId unset so the caller keeps generated weights.
        return None;
    };

    if let Some(class) = classes.get(class_idx) {
        for &id in &unique {
            if let Some(texture) = textures.get_texture(id) {
                if texture_matches_class_name(texture, &class.name) {
                    return Some(id);
                }
            }
        }
    }

    // Map the texture class onto the 4 bound slots via TextureId keys.
    Some(unique[class_idx % unique.len()])
}

/// C++ `loadSetting` packs `RGBAColorInt` as A<<24 | R<<16 | G<<8 | B.
fn pack_water_rgba_int(color: (f32, f32, f32, f32)) -> u32 {
    let r = color.0.round().clamp(0.0, 255.0) as u32;
    let g = color.1.round().clamp(0.0, 255.0) as u32;
    let b = color.2.round().clamp(0.0, 255.0) as u32;
    let a = color.3.round().clamp(0.0, 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

#[cfg(test)]
mod splat_texture_class_tests {
    use super::*;

    fn class(first_tile: i32, num_tiles: i32, name: &str) -> TerrainSourceTileClass {
        TerrainSourceTileClass {
            first_tile,
            num_tiles,
            width: 2,
            name: name.to_string(),
        }
    }

    fn bound_fixture() -> (
        TerrainTextures,
        Vec<TerrainSourceTileClass>,
        [TextureId; MAX_TEXTURES_PER_CHUNK],
        HashMap<TextureId, usize>,
    ) {
        let mut textures = TerrainTextures::new();
        let grass = textures.register_texture(TerrainTexture::new(
            0,
            "Grass".to_string(),
            "Art/Terrain/Grass.tga".to_string(),
        ));
        let rock = textures.register_texture(TerrainTexture::new(
            0,
            "Rock".to_string(),
            "Art/Terrain/Rock.tga".to_string(),
        ));
        let snow = textures.register_texture(TerrainTexture::new(
            0,
            "Snow".to_string(),
            "Art/Terrain/Snow.tga".to_string(),
        ));
        let dirt = textures.register_texture(TerrainTexture::new(
            0,
            "Dirt".to_string(),
            "Art/Terrain/Dirt.tga".to_string(),
        ));
        let stable = [grass, rock, snow, dirt];
        let mut slot_map = HashMap::new();
        for (slot, id) in stable.iter().enumerate() {
            slot_map.entry(*id).or_insert(slot);
        }
        let classes = vec![
            class(0, 4, "Grass"),
            class(4, 4, "Rock"),
            class(8, 4, "Snow"),
            class(12, 4, "Dirt"),
        ];
        (textures, classes, stable, slot_map)
    }

    #[test]
    fn get_texture_class_from_ndx_includes_tile_zero() {
        // C++ WorldHeightMap::getTextureClassFromNdx (WorldHeightMap.cpp:2308)
        // after tileNdx>>2. firstTile==0 is a valid class.
        let classes = [class(0, 4, "Grass"), class(4, 4, "Rock")];
        assert_eq!(texture_class_index_from_ndx(0, &classes), Some(0));
        assert!(source_tile_class_contains(&classes, 0));
        assert_eq!(texture_class_index_from_ndx(3, &classes), Some(0));
        assert_eq!(texture_class_index_from_ndx(4, &classes), Some(1));
    }

    #[test]
    fn adjacent_classes_bind_different_texture_id_slots() {
        let (textures, classes, stable, slot_map) = bound_fixture();
        let grass = bound_texture_id_for_source_tile(
            0, &classes, &textures, &stable, &slot_map,
        );
        let rock = bound_texture_id_for_source_tile(
            4, &classes, &textures, &stable, &slot_map,
        );
        let snow = bound_texture_id_for_source_tile(
            8, &classes, &textures, &stable, &slot_map,
        );
        assert_eq!(grass, Some(stable[0]));
        assert_eq!(rock, Some(stable[1]));
        assert_eq!(snow, Some(stable[2]));
        assert_ne!(grass, rock);
        assert_ne!(rock, snow);
        // tile%4 would collapse 0/4/8 onto the same slot.
        assert_ne!(slot_map[&grass.unwrap()], slot_map[&rock.unwrap()]);
        assert_ne!(slot_map[&rock.unwrap()], slot_map[&snow.unwrap()]);
    }

    #[test]
    fn unmapped_tile_is_not_used_as_slot_index() {
        let (textures, classes, stable, slot_map) = bound_fixture();
        // packed 99 is outside firstTile/numTiles — must not become 99%4.
        assert_eq!(
            bound_texture_id_for_source_tile(99, &classes, &textures, &stable, &slot_map),
            None
        );
    }
}

