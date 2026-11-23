// Split from `terrain/terrain_visual.rs` dump. Included by `terrain_visual/mod.rs`.

impl TerrainVisualImpl {
    /// Initialize WGPU resources
    pub fn init_gpu_resources(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> TerrainResult<()> {
        let device_changed = self
            .device
            .as_ref()
            .is_none_or(|current| !Arc::ptr_eq(current, &device));
        if device_changed {
            self.tree_buffer.force_vertex_rebuild();
        }
        if device_changed && self.device.is_some() {
            self.water_sampler = None;
            self.water_texture = None;
            self.water_texture_bind_group = None;
            self.water_texture_search_exhausted = false;
            self.water_named_bind_groups.clear();
            self.river_gpu = RiverGpuState::default();
            self.tree_meshes.clear();
            self.tree_atlas_sampler = None;
            self.tree_atlas_texture = None;
            self.tree_atlas_bind_group = None;
            self.tree_sway_uniform_buffer = None;
            self.tree_sway_bind_group = None;
        }
        self.device = Some(Arc::clone(&device));
        self.queue = Some(Arc::clone(&queue));
        self.shoreline_meshes.clear();
        self.polygon_water_meshes.clear();
        self.overlay.polygon_water_mesh_stamp = None;
        self.overlay.polygon_water_indices.clear();
        self.overlay.overlays_dirty = true;
        self.terrain_material_bind_groups.clear();
        self.chunk_texture_bindings.clear();
        self.chunk_meshes.clear();
        self.water_track_meshes.clear();
        self.water_track_index_buffer = None;
        self.extra_blend_batches.clear();
        // Skybox face textures (incl. the synthetic horizon gradient) and
        // their bind group belong to the previous device after a re-init;
        // drop them so the next refresh rebuilds against this device instead
        // of binding stale textures (wgpu cross-device bind is invalid).
        self.skybox_textures = [None, None, None, None, None];
        self.road_named_bind_groups.clear();
        self.projected_shadow_gpu = None;
        self.tree_decal_texture_requested = false;
        self.tank_track_draws = TankTrackDraws::default();
        self.road_texture = None;
        self.road_sampler = None;
        self.road_texture_bind_group = None;
        self.road_texture_search_exhausted = false;
        self.skybox_background_bind_group = None;
        self.skybox_background_view = None;
        self.last_skybox_face_bind = None;

        // Create terrain render pipeline
        self.create_terrain_pipeline(device.as_ref())?;

        // Create skybox background pipeline before terrain/water draws.
        self.create_skybox_background_pipeline(device.as_ref())?;

        // Create water render pipeline
        self.create_water_pipeline(device.as_ref())?;
        self.create_shroud_overlay_pipelines(device.as_ref())?;
        self.create_river_pipeline(device.as_ref())?;
        self.ensure_water_materials(device.as_ref());
        self.sync_shroud_dest_texture();

        // Create road render pipeline
        self.create_road_pipeline(device.as_ref())?;
        self.ensure_road_texture_bind_group(device.as_ref());
        self.ensure_snow_texture_bind_group(device.as_ref());
        self.create_tree_pipeline(device.as_ref())?;
        self.bind_tree_atlas_group();

        self.sync_water_plane(device.as_ref())?;
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

        let intersects = |min_x: f32,
                          min_z: f32,
                          max_x: f32,
                          max_z: f32,
                          chunk: &crate::terrain::chunk::TerrainChunk| {
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
        let scene_lights = self.dynamic_lights.clone();
        let lights_active = scene_lights.iter().any(|light| light.enabled);
        // C++ HeightMap::doTheDynamicLight rebakes every VB fill. Refill
        // existing meshes while pulses are live and once after they expire.
        let needs_light_rebake = lights_active || self.had_dynamic_lights;

        let mut shared_slot_map: HashMap<TextureId, usize> = HashMap::new();
        for (slot, texture_id) in stable_texture_ids.iter().enumerate() {
            shared_slot_map.entry(*texture_id).or_insert(slot);
        }

        let mut binding_updates = 0usize;
        let mut mesh_uploads = 0usize;
        let mut light_rebakes = 0usize;
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
                let binding = self.prepare_chunk_texture_binding(
                    chunk_id,
                    &stable_texture_ids,
                    &shared_slot_map,
                    device.as_ref(),
                    queue.as_ref(),
                )?;
                self.chunk_texture_bindings.insert(chunk_id, binding);
                binding_prep_elapsed += binding_started.elapsed();
                binding_updates += 1;
            }

            let needs_mesh_upload = match self.chunk_meshes.get_mut(&chunk_id) {
                Some(mesh) if mesh.revision == chunk_revision && !texture_slots_changed => {
                    // Only the lighting changed: refill the existing vertex
                    // buffer like C++ doTheDynamicLight refills its VB.
                    if needs_light_rebake || (mesh.uploaded_with_dynamic_lights && !lights_active) {
                        mesh.rebake_lights(queue.as_ref(), &scene_lights, lights_active);
                        light_rebakes += 1;
                    }
                    false
                }
                _ => true,
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

                // C++ HeightMapRenderObjClass fills four vertices per cell, so
                // each cell carries its own tile UVs and texture class. The
                // shared-vertex grid interpolated UVs across neighbouring
                // cells (mirrored quarters, one tile repeated everywhere).
                let (mut gpu_vertices, chunk_indices, chunk_base_colors, cell_centers) =
                    expand_cells_to_unique_vertices(
                        chunk_vertices,
                        chunk_indices,
                        chunk_base_colors,
                    );
                // Keep whole cells in draw order while admitting all base and
                // first-blend classes needed by each material batch.
                let class_ids: Vec<_> = self.source_tile_classes.iter()
                    .map(|class| self.texture_id_for_class_name(&class.name))
                    .collect();
                let seed: Vec<TextureId> = self.texture_rules.iter()
                    .map(|rule| rule.texture_id)
                    .filter(|id| *id != 0 && self.texture_system.get_texture(*id).is_some())
                    .collect();
                let cells_expanded = gpu_vertices.len() % 4 == 0
                    && chunk_indices.len() == gpu_vertices.len() / 4 * 6;
                let mut materials: Vec<(usize, usize, [TextureId; MAX_TEXTURES_PER_CHUNK], HashMap<TextureId, usize>)> = Vec::new();
                if cells_expanded {
                    let mut start = 0;
                    let mut ids = seed.clone();
                    if ids.is_empty() { ids.push(stable_texture_ids[0]); }
                    let finish = |start: usize, end: usize, ids: &[TextureId]| {
                        let mut slots = HashMap::new();
                        let mut packed = [ids[0]; MAX_TEXTURES_PER_CHUNK];
                        for (slot, &id) in ids.iter().enumerate() {
                            packed[slot] = id;
                            slots.entry(id).or_insert(slot);
                        }
                        (start, end, packed, slots)
                    };
                    for (cell, center) in cell_centers.chunks_exact(4).map(|v| v[0]).enumerate() {
                        let mut needed = Vec::with_capacity(2);
                        if let Some(hm) = self.height_map.as_ref() {
                            let (x, y) = height_map_cell_at_world(hm, center[0], center[1]);
                            let base = hm.get_packed_terrain_tile_at_world(center[0], center[1]);
                            let blend = hm.get_blend_tile_index(x, y);
                            let other = (blend > 0).then(|| hm.blended_tiles.get(blend as usize)).flatten()
                                .map(|info| (info.blend_ndx >> 2).max(0) as u32);
                            for tile in std::iter::once(base).chain(other) {
                                if let Some(id) = texture_class_index_from_ndx(tile, &self.source_tile_classes)
                                    .and_then(|index| class_ids[index]) {
                                    if !needed.contains(&id) { needed.push(id); }
                                }
                            }
                        }
                        let additional = needed.iter().filter(|id| !ids.contains(id)).count();
                        if ids.len() + additional > MAX_TEXTURES_PER_CHUNK {
                            materials.push(finish(start, cell, &ids));
                            start = cell;
                            ids = seed.clone();
                            if ids.is_empty() { ids.push(stable_texture_ids[0]); }
                        }
                        for id in needed {
                            if !ids.contains(&id) { ids.push(id); }
                        }
                    }
                    materials.push(finish(start, gpu_vertices.len() / 4, &ids));
                } else {
                    materials.push((0, gpu_vertices.len(), stable_texture_ids, shared_slot_map.clone()));
                }
                let batch_for_vertex: Vec<usize> = materials.iter().enumerate()
                    .flat_map(|(batch, (start, end, _, _))| {
                        let count = if cells_expanded { (end - start) * 4 } else { end - start };
                        std::iter::repeat(batch).take(count)
                    }).collect();
                let mut vertex_weights = Vec::with_capacity(gpu_vertices.len());
                for ((vertex, center), batch) in gpu_vertices.iter().zip(&cell_centers).zip(&batch_for_vertex) {
                    let (_, _, stable_texture_ids, shared_slot_map) = &materials[*batch];
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
                        let packed = hm.get_packed_terrain_tile_at_world(center[0], center[1]);
                        let (ix, iy) = height_map_cell_at_world(hm, center[0], center[1]);
                        let blend_i = hm.get_blend_tile_index(ix, iy);
                        let blend = if blend_i != 0 {
                            hm.blended_tiles.get(blend_i as usize)
                        } else {
                            None
                        };
                        let other_packed = blend.map(|blend| (blend.blend_ndx >> 2).max(0) as u32);
                        // Vertex fraction inside the cell for the C++
                        // per-corner alpha table (cell_uv_at_world math).
                        let scale = if hm.scale.abs() > f32::EPSILON {
                            hm.scale
                        } else {
                            gamelogic::common::types::MAP_XY_FACTOR
                        };
                        let frac_x = (position.x / scale + hm.border_size as f32 - ix as f32)
                            .clamp(0.0, 1.0);
                        let frac_y = (position.z / scale + hm.border_size as f32 - iy as f32)
                            .clamp(0.0, 1.0);
                        (packed, blend, other_packed, frac_x, frac_y)
                    });

                    let base_weights = self.texture_system.generate_texture_weights(
                        height,
                        slope,
                        vertex.tex_coords,
                        &self.texture_rules,
                    );

                    let blended = if let Some((packed, blend, other_packed, frac_x, frac_y)) =
                        tile_sample
                    {
                        let base_id = bound_texture_id_for_source_tile(
                            packed,
                            &self.source_tile_classes,
                            &self.texture_system,
                            &stable_texture_ids,
                            &shared_slot_map,
                        );
                        if blend.is_some() {
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
                                    // C++ getAlphaUVData
                                    // (WorldHeightMap.cpp:2042-2134): the
                                    // blend tile covers the cell with a
                                    // directional per-corner alpha; the
                                    // flat 50/50 painted band-shaped 50%
                                    // stripes on every blend edge.
                                    let alpha = blend
                                        .map(|flags| {
                                            cpp_blend_corner_alpha(flags).0
                                                [cell_corner(frac_x, frac_y)]
                                        })
                                        .unwrap_or(0.5);
                                    TextureWeights::blend_two(a, b, (1.0 - alpha).clamp(0.0, 1.0))
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
                let mut chunk_indices = chunk_indices;
                if let Some(map) = self.height_map.as_ref() {
                    let ground = TerrainReceiverHeight {
                        map,
                        classes: &self.source_tile_classes,
                    };
                    for (quad, center) in chunk_indices.chunks_exact_mut(6)
                        .zip(cell_centers.iter().step_by(4))
                    {
                        let (x, y) = height_map_cell_at_world(map, center[0], center[1]);
                        if quad[3] == quad[0] && quad[4] == quad[2] && ground.flip_state(x, y) {
                            let base = quad[0];
                            quad.copy_from_slice(&[base, base + 1, base + 3, base + 1, base + 2, base + 3]);
                        }
                    }
                }

                // Bound slot per weight. The fragment shader reads the slots
                // flat, so every corner of a cell gets the same slot set (the
                // union over the cell, strongest four, sorted by slot) and its
                // own weight for each slot.
                let fallback_slot = 0u16;
                let slot_weights: Vec<Vec<(u16, f32)>> = vertex_weights
                    .iter()
                    .enumerate()
                    .map(|(vertex_index, blended)| {
                        let shared_slot_map = &materials[batch_for_vertex[vertex_index]].3;
                        let mut pairs: Vec<(u16, f32)> = Vec::new();
                        for (texture_id, weight) in blended.iter_pairs() {
                            let slot = shared_slot_map
                                .get(&texture_id)
                                .copied()
                                .map(|slot| slot as u16)
                                .unwrap_or(fallback_slot);
                            match pairs.iter_mut().find(|(s, _)| *s == slot) {
                                Some(entry) => entry.1 += weight,
                                None => pairs.push((slot, weight)),
                            }
                        }
                        pairs
                    })
                    .collect();
                let mut packed_per_vertex =
                    vec![([0u16; MAX_BLEND_WEIGHTS], [0.0f32; MAX_BLEND_WEIGHTS]); gpu_vertices.len()];
                let mut group_start = 0usize;
                while group_start < gpu_vertices.len() {
                    let mut group_end = group_start + 1;
                    while group_end < gpu_vertices.len()
                        && cell_centers[group_end] == cell_centers[group_start]
                        && batch_for_vertex[group_end] == batch_for_vertex[group_start]
                    {
                        group_end += 1;
                    }
                    let mut union: Vec<(u16, f32)> = Vec::new();
                    for pairs in &slot_weights[group_start..group_end] {
                        for &(slot, weight) in pairs {
                            match union.iter_mut().find(|(s, _)| *s == slot) {
                                Some(entry) => entry.1 += weight,
                                None => union.push((slot, weight)),
                            }
                        }
                    }
                    union.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
                    union.truncate(MAX_BLEND_WEIGHTS);
                    union.sort_by_key(|(slot, _)| *slot);
                    if union.is_empty() {
                        union.push((fallback_slot, 1.0));
                    }
                    for vert in group_start..group_end {
                        let (indices, weights) = &mut packed_per_vertex[vert];
                        for (k, &(slot, _)) in union.iter().enumerate() {
                            indices[k] = slot;
                            weights[k] = slot_weights[vert]
                                .iter()
                                .find(|(s, _)| *s == slot)
                                .map_or(0.0, |(_, w)| *w);
                        }
                        let sum: f32 = weights.iter().sum();
                        if sum > f32::EPSILON {
                            for weight in weights.iter_mut() {
                                *weight /= sum;
                            }
                        } else {
                            *weights = [0.0; MAX_BLEND_WEIGHTS];
                            weights[0] = 1.0;
                        }
                    }
                    group_start = group_end;
                }

                for (vert_index, vertex) in gpu_vertices.iter_mut().enumerate() {
                    let (_, _, stable_texture_ids, shared_slot_map) = &materials[batch_for_vertex[vert_index]];
                    let (packed_indices, packed_weights) = packed_per_vertex[vert_index];
                    vertex.blend_indices = packed_indices;
                    vertex.blend_weights = packed_weights;
                    if let Some(hm) = self.height_map.as_ref() {
                        vertex.tex_coords = cell_class_uv(
                            hm,
                            &self.source_tile_classes,
                            cell_centers[vert_index],
                            [vertex.position[0], vertex.position[2]],
                        );
                        if let Some((base_slot, blend_slot, alpha, blend_uv)) =
                            authored_blend_vertex(
                                hm,
                                &self.source_tile_classes,
                                &self.texture_system,
                                &stable_texture_ids,
                                &shared_slot_map,
                                cell_centers[vert_index],
                                [vertex.position[0], vertex.position[2]],
                            )
                        {
                            vertex.blend_indices = [base_slot, blend_slot, 0, 0];
                            vertex.blend_weights = [1.0 - alpha, alpha, 0.0, 0.0];
                            vertex.blend_tex_coords = blend_uv;
                            vertex.blend_texture_slot = 1;
                        }
                    }
                    // C++ vbMirror / getStaticDiffuse, then doTheDynamicLight.
                    // Leftover chunk.vertices may already be pulse-lit; start
                    // from base_colors so a live remesh does not double-apply.
                    vertex.color = chunk_base_colors
                        .get(vert_index)
                        .copied()
                        .unwrap_or(vertex.color);
                }
                let static_colors: Vec<[f32; 4]> =
                    gpu_vertices.iter().map(|vertex| vertex.color).collect();
                for vertex in &mut gpu_vertices {
                    vertex.color = Self::bake_terrain_vertex_dynamic_light(
                        vertex.position,
                        vertex.normal,
                        vertex.color,
                        &scene_lights,
                    );
                }

                vertices_uploaded += gpu_vertices.len();
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
                let mut batches = Vec::with_capacity(materials.len());
                for (start, end, ids, slots) in &materials {
                    let binding = self.prepare_chunk_texture_binding(
                        chunk_id, ids, slots, device.as_ref(), queue.as_ref(),
                    )?;
                    let indices = if cells_expanded {
                        (*start as u32 * 6)..(*end as u32 * 6)
                    } else {
                        0..chunk_indices.len() as u32
                    };
                    batches.push(GpuTerrainBatch { indices, bind_group: binding.bind_group });
                }

                self.chunk_meshes.insert(
                    chunk_id,
                    GpuChunkMesh {
                        vertex_buffer,
                        index_buffer,
                        index_count: chunk_indices.len() as u32,
                        batches,
                        revision: chunk_revision,
                        uploaded_with_dynamic_lights: lights_active,
                        vertices: gpu_vertices,
                        static_colors,
                    },
                );
                mesh_upload_elapsed += upload_started.elapsed();
                mesh_uploads += 1;
                indices_uploaded += chunk_indices.len();
            }
        }

        self.chunk_meshes
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        self.chunk_texture_bindings
            .retain(|id, _| self.chunk_manager.has_chunk(*id));

        // The extra-blend overlay depends on the draw window, the bound
        // classes, cell data (which bumps chunk revisions) and lighting.
        let extra_blend_key = ExtraBlendUploadKey {
            draw_origin: (self.draw_origin_x, self.draw_origin_y),
            draw_size: (self.draw_width, self.draw_height),
            texture_ids: self.active_chunk_texture_ids,
        };
        if mesh_uploads > 0
            || light_rebakes > 0
            || needs_light_rebake
            || self.extra_blend_upload_key != Some(extra_blend_key)
        {
            self.upload_extra_blend_overlay();
            if self.extra_blend_draw_mesh.is_empty()
                || (self.extra_blend_vertex_buffer.is_some()
                    && self.extra_blend_index_buffer.is_some())
            {
                self.extra_blend_upload_key = Some(extra_blend_key);
            }
        }
        self.had_dynamic_lights = lights_active;

        self.stats.rendered_chunks = visible_chunk_ids.len();
        self.stats.triangles_rendered = visible_chunk_ids
            .iter()
            .filter_map(|chunk_id| self.chunk_manager.get_chunk(*chunk_id))
            .map(|chunk| chunk.stats.triangle_count as usize)
            .sum();

        let elapsed = started.elapsed();
        if self.terrain_metrics_enabled {
            self.terrain_metrics_frame = self.terrain_metrics_frame.wrapping_add(1);
            if self.terrain_metrics_frame % 60 == 1 {
                let mut lod_chunks = [0usize; 5];
                let mut visible_vertices = 0usize;
                let mut visible_indices = 0usize;
                let mut draw_calls = 0usize;
                let mut shrouded_chunk_centers = 0usize;
                for &chunk_id in &visible_chunk_ids {
                    let Some(chunk) = self.chunk_manager.get_chunk(chunk_id) else {
                        continue;
                    };
                    lod_chunks[usize::from(chunk.lod_level).min(4)] += 1;
                    if self.chunk_meshes.contains_key(&chunk_id) {
                        visible_vertices += chunk.vertices.len();
                        visible_indices += chunk.indices.len();
                        draw_calls += 1;
                        let center_x = (chunk.bounds.min.x + chunk.bounds.max.x) * 0.5;
                        let center_z = (chunk.bounds.min.z + chunk.bounds.max.z) * 0.5;
                        shrouded_chunk_centers +=
                            usize::from(self.shroud_alpha_at_world(center_x, center_z) < 0.01);
                    }
                }
                let shroud_zero_cells = self
                    .overlay
                    .shroud_cells
                    .iter()
                    .filter(|&&alpha| alpha == 0)
                    .count();
                let mut shroud_seen_bounds = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];
                if self.overlay.shroud_width > 0 {
                    for (index, &alpha) in self.overlay.shroud_cells.iter().enumerate() {
                        if alpha == 0 {
                            continue;
                        }
                        let x = index as i32 % self.overlay.shroud_width;
                        let y = index as i32 / self.overlay.shroud_width;
                        shroud_seen_bounds[0] = shroud_seen_bounds[0].min(x);
                        shroud_seen_bounds[1] = shroud_seen_bounds[1].min(y);
                        shroud_seen_bounds[2] = shroud_seen_bounds[2].max(x);
                        shroud_seen_bounds[3] = shroud_seen_bounds[3].max(y);
                    }
                }
                let shroud_center_x = (self.draw_origin_x + self.draw_width / 2
                    - self.height_map.as_ref().map_or(0, |hm| hm.border_size))
                    as f32
                    * self.map_scale();
                let shroud_center_z = (self.draw_origin_y + self.draw_height / 2
                    - self.height_map.as_ref().map_or(0, |hm| hm.border_size))
                    as f32
                    * self.map_scale();
                info!(
                    "terrain_metrics frame={} origin=({}, {}) draw_samples={}x{} lod_chunks={:?} draw_calls={} visible_vertices={} visible_triangles={} uploaded_vertices={} uploaded_indices={} mesh_uploads={} shroud_grid={}x{} shroud_zero_cells={} shroud_seen_bounds={:?} shrouded_chunk_centers={} shroud_center_alpha={:.3} update_ms={:.3} upload_ms={:.3}",
                    self.terrain_metrics_frame,
                    self.draw_origin_x,
                    self.draw_origin_y,
                    self.draw_width,
                    self.draw_height,
                    lod_chunks,
                    draw_calls,
                    visible_vertices,
                    visible_indices / 3,
                    vertices_uploaded,
                    indices_uploaded,
                    mesh_uploads,
                    self.overlay.shroud_width,
                    self.overlay.shroud_height,
                    shroud_zero_cells,
                    shroud_seen_bounds,
                    shrouded_chunk_centers,
                    self.shroud_alpha_at_world(shroud_center_x, shroud_center_z),
                    elapsed.as_secs_f64() * 1000.0,
                    mesh_upload_elapsed.as_secs_f64() * 1000.0
                );
            }
        }
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
        // C++ composes every texture class into one combiner atlas
        // (WorldHeightMap::getTerrainTexture), so all classes stay addressable.
        // The live path binds only MAX_TEXTURES_PER_CHUNK slots per chunk, so
        // rank the map's classes by actual tile usage instead of taking the
        // first four in map order — a >4-class map no longer collapses every
        // under-represented class onto the fallback slot.
        // WorldHeightMap.cpp:2142 builds resident texture data only when absent.
        // Cache map-only ranking; texture lookup below must still see late assets.
        if self.ranked_source_tile_classes.is_none() {
            #[cfg(test)]
            {
                self.texture_class_rank_builds += 1;
            }
            self.ranked_source_tile_classes = Some(self.rank_source_tile_classes());
        }

        let mut selected_textures: Vec<TextureId> = Vec::new();
        for class_idx in self
            .ranked_source_tile_classes
            .as_deref()
            .unwrap_or_default()
        {
            if selected_textures.len() == MAX_TEXTURES_PER_CHUNK {
                break;
            }
            let class_name = &self.source_tile_classes[*class_idx].name;
            if let Some(texture_id) = self.texture_id_for_class_name(class_name) {
                if selected_textures
                    .iter()
                    .all(|candidate| *candidate != texture_id)
                {
                    selected_textures.push(texture_id);
                }
            }
        }

        // Classes the map never places still keep their rule/textures eligible
        // (paint rules, fallback terrain) before the global first-texture sink.
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

    fn rank_source_tile_classes(&self) -> Vec<usize> {
        let mut class_usage: HashMap<usize, u32> = HashMap::new();
        if let Some(height_map) = self.height_map.as_ref() {
            for &tile_ndx in &height_map.tile_ndxes {
                let packed_tile = ((tile_ndx >> 2).max(0)) as u32;
                if let Some(class_idx) =
                    texture_class_index_from_ndx(packed_tile, &self.source_tile_classes)
                {
                    *class_usage.entry(class_idx).or_insert(0) += 1;
                }
            }
            for &blend_index in height_map.blend_tile_ndxes.iter().chain(&height_map.extra_blend_tile_ndxes) {
                if blend_index <= 0 {
                    continue;
                }
                if let Some(blend) = height_map.blended_tiles.get(blend_index as usize) {
                    let packed_tile = (blend.blend_ndx >> 2).max(0) as u32;
                    if let Some(class_idx) =
                        texture_class_index_from_ndx(packed_tile, &self.source_tile_classes)
                    {
                        *class_usage.entry(class_idx).or_insert(0) += 1;
                    }
                }
            }
        }
        let mut ranked_classes: Vec<usize> = class_usage.keys().copied().collect();
        ranked_classes.sort_by(|a, b| {
            class_usage[b].cmp(&class_usage[a]).then_with(|| {
                self.source_tile_classes[*a]
                    .first_tile
                    .cmp(&self.source_tile_classes[*b].first_tile)
            })
        });
        ranked_classes
    }

    fn texture_id_for_class_name(&self, class_name: &str) -> Option<TextureId> {
        if let Some(id) = self.texture_system.registered_textures()
            .filter(|texture| texture.name.eq_ignore_ascii_case(class_name.trim()))
            .map(|texture| texture.id)
            .min()
        {
            return Some(id);
        }
        for rule in &self.texture_rules {
            if let Some(texture) = self.texture_system.get_texture(rule.texture_id) {
                if texture_matches_class_name(texture, class_name) {
                    return Some(rule.texture_id);
                }
            }
        }
        self.texture_system
            .registered_textures()
            .filter(|texture| texture_matches_class_name(texture, class_name))
            .map(|texture| texture.id)
            .min()
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

        let mut road_batches = RoadMeshBatches::new(&device);
        let mut bridge_meshes = Vec::new();
        let authored_roads = &self.road_presentation;
        let height_map = self.height_map.as_ref();
        let height_at = |x: f32, y: f32| {
            height_map
                .map(|height_map| height_map.get_max_cell_height(x, y))
                .unwrap_or(0.0)
        };
        // C++ updateSegLighting shades with all lights admitted by this world.
        let lighting = self.chunk_manager.static_lighting();
        let global_lights = &lighting.lights;
        let ambient_color = lighting.ambient;
        let dynamic_lights = &self.dynamic_lights;

        self.road_system
            .for_each_visible_overlay_source(|road, segment| {
                let texture_name = authored_road_texture_name(
                    road.name.as_str(),
                    segment.properties.texture_override.as_deref(),
                    authored_roads,
                );
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
                    if let Some(mut mesh) = Self::upload_overlay_mesh(
                        &device,
                        "Bridge Mesh",
                        &gpu_vertices,
                        &gpu_indices,
                    ) {
                        mesh.texture_name = texture_name.to_owned();
                        bridge_meshes.push(mesh);
                    }
                    return;
                }

                let diffuse_mode = if texture_name.is_empty()
                    || matches!(road.road_type, RoadType::WoodenBridge { .. })
                {
                    RoadDiffuseMode::Compatibility
                } else {
                    RoadDiffuseMode::AuthoredRoad
                };
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
                    let width_in_texture = segment
                        .runtime_texture_override_f32("WidthInTexture")
                        .unwrap_or(1.0);
                    if let Some((verts, indices)) = bake_straight_road_section(
                        [segment.start.x, segment.start.z],
                        [segment.end.x, segment.end.z],
                        segment.width,
                        width_in_texture,
                        0.0,
                        85.0 / 512.0,
                        segment.width,
                        segment.properties.w3d_miters,
                        height_at,
                        &[],
                    ) {
                        if !verts.is_empty() && !indices.is_empty() {
                            let mut gpu_vertices = fill_road_gpu_upload_vertices(&verts);
                            Self::apply_road_vertex_static_diffuse(
                                &mut gpu_vertices,
                                height_map,
                                global_lights,
                                ambient_color,
                                dynamic_lights,
                                diffuse_mode,
                            );
                            road_batches.append(
                                texture_name,
                                diffuse_mode,
                                &gpu_vertices,
                                indices.iter().map(|&index| u32::from(index)),
                            );
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
                let projected = height_map
                    .and_then(|_| segment.project_overlay_strip(|pos| height_at(pos.x, pos.z)));
                let (vertices, indices) = projected.as_ref().map_or(
                    (geometry.vertices.as_slice(), geometry.indices.as_slice()),
                    |(vertices, indices)| (vertices.as_slice(), indices.as_slice()),
                );
                let mut gpu_vertices: Vec<RoadVertex> = vertices
                    .iter()
                    .map(|vertex| OverlayGpuVertex {
                        position: vertex.position,
                        color: [vertex.color[0], vertex.color[1], vertex.color[2]],
                        tex_coords: vertex.tex_coords,
                        road_width: 1.0,
                        diffuse: 0xFFFF_FFFF,
                    })
                    .collect();
                if height_map.is_none() {
                    for vertex in &mut gpu_vertices {
                        vertex.position[1] = ROAD_FLOAT_AMOUNT;
                    }
                }

                Self::apply_road_vertex_static_diffuse(
                    &mut gpu_vertices,
                    height_map,
                    global_lights,
                    ambient_color,
                    dynamic_lights,
                    diffuse_mode,
                );
                road_batches.append(
                    texture_name,
                    diffuse_mode,
                    &gpu_vertices,
                    indices.iter().copied(),
                );
            });

        self.road_meshes = road_batches.finish();
        self.bridge_meshes = bridge_meshes;
        let mut road_texture_names = Vec::new();
        for mesh in self.road_meshes.iter().chain(&self.bridge_meshes) {
            if !mesh.texture_name.is_empty()
                && !road_texture_names
                    .iter()
                    .any(|name: &String| name.eq_ignore_ascii_case(&mesh.texture_name))
            {
                road_texture_names.push(mesh.texture_name.clone());
            }
        }
        for name in road_texture_names {
            self.ensure_named_road_texture(&device, &name);
        }
        self.update_scorch_meshes(&device);
        // The receiving visual already owns the authored road art.
        self.overlay_gpu_meshes_dirty = false;
        Ok(())
    }

    fn update_scorch_meshes(&mut self, device: &wgpu::Device) {
        self.ensure_scorch_texture_bind_group(device);
        let Some(height_map) = self.height_map.as_ref() else {
            self.scorch_meshes.clear();
            return;
        };
        let lighting = self.chunk_manager.static_lighting();
        let diffuse = crate::terrain::scorch_mesh::scorch_diffuse_bgra(
            lighting.ambient,
            lighting.scorch_diffuse,
        );
        let ground = TerrainReceiverHeight {
            map: height_map,
            classes: &self.source_tile_classes,
        };
        let baked = self.scorches.update_scorches(&ground, diffuse);
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
            texture_name: String::new(),
            diffuse_mode: RoadDiffuseMode::Compatibility,
            bib_highlight: false,
        })
    }

    /// C++ RoadSegment::updateSegLighting writes getStaticDiffuse into vertex color.
    fn apply_road_vertex_static_diffuse(
        vertices: &mut [OverlayGpuVertex],
        height_map: Option<&HeightMap>,
        global_lights: &[(Vec3, [f32; 3])],
        ambient_color: [f32; 3],
        dynamic_lights: &[DisplayDynamicLight],
        diffuse_mode: RoadDiffuseMode,
    ) {
        let minimum_diffuse = match diffuse_mode {
            RoadDiffuseMode::Compatibility => 0.35,
            RoadDiffuseMode::AuthoredRoad => 0.0,
        };
        for vertex in vertices {
            let normal = height_map.map_or(Vec3::Y, |height_map| {
                // W3DRoadBuffer.cpp updateSegLighting rounds each coordinate
                // by adding 0.5 before truncating, then adds the map border.
                let sample = |value: f32| {
                    let index = (value / height_map.scale + 0.5) as i32;
                    index.saturating_add(height_map.border_size).max(0) as u32
                };
                height_map.render_normal_at_sample(
                    sample(vertex.position[0]),
                    sample(vertex.position[2]),
                )
            });
            let lit =
                Self::terrain_static_diffuse_from_normal(normal, global_lights, ambient_color);
            vertex.color = [
                lit[0].max(minimum_diffuse),
                lit[1].max(minimum_diffuse),
                lit[2].max(minimum_diffuse),
            ];
            let r = (vertex.color[0] * 255.0).round() as u32;
            let g = (vertex.color[1] * 255.0).round() as u32;
            let b = (vertex.color[2] * 255.0).round() as u32;
            let base = 0xFF00_0000 | (r << 16) | (g << 8) | b;
            vertex.diffuse = super::w3d_overlay_mesh::do_road_dynamic_light(
                [vertex.position[0], vertex.position[2], vertex.position[1]],
                base,
                dynamic_lights,
            );
            vertex.color = super::w3d_overlay_mesh::unpack_bgra_rgb(vertex.diffuse);
        }
    }

    pub fn record_chunk_draws(&self, pass: &mut RenderPass<'_>) {
        self.record_chunk_draws_impl(pass, true, true);
    }

    /// Main completes the waypoint overlay after shroud and before the bibs.
    pub fn record_chunk_draws_before_bibs(&self, pass: &mut RenderPass<'_>) {
        self.record_chunk_draws_impl(pass, false, true);
    }

    pub fn record_native_terrain_ground(&self, pass: &mut RenderPass<'_>, inline_bibs: bool) {
        self.record_chunk_draws_impl(pass, inline_bibs, false);
    }

    fn record_chunk_draws_impl(
        &self,
        pass: &mut RenderPass<'_>,
        inline_bibs: bool,
        inline_trees: bool,
    ) {
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
                let Some(mesh) = self.chunk_meshes.get(&chunk_id) else {
                    continue;
                };

                pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
                pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                for batch in &mesh.batches {
                    pass.set_bind_group(1, &batch.bind_group, &[]);
                    pass.draw_indexed(batch.indices.clone(), 0, 0..1);
                }
            }
        }
        // HeightMap.cpp draws feathered shorelines before the 3-way overlay.
        self.record_shoreline_draws(pass);
        self.record_extra_blend_pass(pass);
        self.record_road_draws(pass);
        self.record_overlay_draws(pass, inline_bibs);
        self.record_tank_track_draws(pass);
        // C++ HeightMap.cpp:2138-2144 replays terrain after roads and tracks
        // with W3DShroud's projected destination texture. Keep the base mesh
        // independent of changing fog cells and darken the road depth itself.
        self.record_shroud_ground_pass(pass);
        if inline_trees {
            self.record_tree_draws(pass);
        }
        self.record_shroud_water_pass(pass, true);
    }

    /// C++ static sort level 2: the whole water scene renders after opaque
    /// objects and shadows (W3DScene.cpp:833 Render_And_Clear_Static_Sort_Lists).
    /// GlobalData sea plane (drawSea) + water tracks + polygon rivers/trapezoids
    /// (renderWater) + water grid mesh (renderWaterMesh), and the separate
    /// shroud passes for water that uses them.
    pub fn record_water_scene_draws(&self, pass: &mut RenderPass<'_>) {
        self.record_water_draws(pass);
        self.record_extra_water_draws(pass);
        self.record_shroud_water_pass(pass, false);
    }

    pub fn record_chunk_depth_draws(&self, pass: &mut RenderPass<'_>) {
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
        // C++ WaterRenderObjClass::draw (W3DWater.cpp:1702-1708) renders the
        // skybox only while TheGlobalData->m_drawSkyBox stands (GameData.ini
        // `DrawSkyBox`; script DRAW_SKYBOX_BEGIN/END -> doSkyBoxSet,
        // W3DWater.cpp:150-154).
        if !self.skybox_enabled {
            return;
        }
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
    pub fn ensure_water_plane(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        self.sync_water_plane(device)
    }

    pub fn record_water_draws(&self, pass: &mut RenderPass<'_>) {
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
            pass.set_vertex_buffer(0, water_plane.vertex_slice());
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
            pass.set_vertex_buffer(0, mesh.vertex_slice());
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn record_road_draws(&self, pass: &mut RenderPass<'_>) {
        let (Some(road_pipeline), Some(camera_bg)) = (
            self.road_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };

        if (!self.road_meshes.is_empty() || !self.bridge_meshes.is_empty())
            && let Some(road_bg) = self.road_texture_bind_group.as_ref()
        {
            pass.set_bind_group(0, camera_bg, &[]);
            let mut current_pipeline: Option<&wgpu::RenderPipeline> = None;
            for mesh in self.road_meshes.iter().chain(self.bridge_meshes.iter()) {
                let texture = self
                    .road_named_bind_groups
                    .get(&mesh.texture_name.to_ascii_lowercase());
                let pipeline = match (mesh.diffuse_mode, texture) {
                    (RoadDiffuseMode::AuthoredRoad, Some(_)) => self
                        .authored_road_pipeline
                        .as_ref()
                        .unwrap_or(road_pipeline),
                    _ => road_pipeline,
                };
                if current_pipeline.is_none_or(|current| !std::ptr::eq(current, pipeline)) {
                    pass.set_pipeline(pipeline);
                    current_pipeline = Some(pipeline);
                }
                let texture_bg = texture.map_or(road_bg, |named| &named.bind_group);
                pass.set_bind_group(1, texture_bg, &[]);
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
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
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

    fn ensure_named_road_texture(&mut self, device: &wgpu::Device, name: &str) {
        let key = name.to_ascii_lowercase();
        if self.road_named_bind_groups.contains_key(&key) {
            return;
        }
        let (Some(layout), Some(sampler)) = (
            self.road_texture_bind_group_layout.as_ref(),
            self.road_sampler.as_ref(),
        ) else {
            return;
        };
        let texture = Self::road_texture_path_candidates(name)
            .into_iter()
            .find_map(|path| {
                self.load_road_texture(device, &path)
                    .ok()
                    .map(|texture| (texture, path))
            });
        let Some((texture, source_path)) = texture else {
            warn!(
                "Roads.ini texture '{}' missing; retaining road fallback",
                name
            );
            return;
        };
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Authored Road Texture Bind Group"),
            layout,
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
        self.road_named_bind_groups.insert(
            key,
            NamedRoadBind {
                _texture: texture,
                bind_group,
            },
        );
        info!("Roads.ini texture '{}' bound from {}", name, source_path);
    }

    fn wanted_snow_texture_name() -> String {
        crate::snow::get_weather_setting()
            .and_then(|s| s.read().ok().map(|g| g.snow_texture.clone()))
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| "EXSnowFlake.tga".to_string())
    }

    fn ensure_snow_texture_bind_group(&mut self, device: &wgpu::Device) {
        let wanted = Self::wanted_snow_texture_name();
        self.ensure_snow_texture_bind_group_owned(device, &wanted);
    }

    fn ensure_snow_texture_bind_group_owned(&mut self, device: &wgpu::Device, wanted: &str) {
        let wanted = wanted.to_owned();
        if self.snow_texture_bind_group.is_some()
            && !self.snow_texture_is_fallback
            && self.snow_texture_name.eq_ignore_ascii_case(&wanted)
        {
            return;
        }
        // Negative cache for the current wanted name: a failed search keeps
        // the white fallback bound instead of re-probing per frame. A changed
        // Weather.ini snow texture name re-opens the search.
        if self.snow_texture_search_exhausted
            && self.snow_texture_name.eq_ignore_ascii_case(&wanted)
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
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
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
        if self.scorch_texture_search_exhausted
            && self.scorch_texture_name.eq_ignore_ascii_case(wanted)
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
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
                    if let Ok(texture) = self.load_road_texture(device, &path) {
                        return Some((texture, path));
                    }
                }
                if let Ok(texture) = self.load_road_texture(device, &path) {
                    return Some((texture, path));
                }
            }
        }
        None
    }

    fn load_road_texture(&self, device: &wgpu::Device, path: &str) -> TerrainResult<wgpu::Texture> {
        // W3DRoadBuffer.cpp RoadType::loadTexture requests MIP_LEVELS_3.
        self.load_texture_from_path_with_mips(device, path, 3)
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

        for road in &self.road_presentation.roads {
            push_name(&road.texture);
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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
        // Water.ini admission clears this when the owner's effective settings change.
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
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
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
        push_name(&self.water_presentation.standing_water_texture);
        push_name(
            &self
                .water_presentation
                .setting_at(self.water_rules.time_of_day)
                .texture_name,
        );
        for name in [
            "TWWater01.tga",
            "TWWater01.dds",
            "water01.dds",
            "Water01.tga",
        ] {
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
            format: wgpu::TextureFormat::Rgba8Unorm,
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

    /// W3DScene::Flush calls DoTrees after opaque/occluded meshes. Its decal
    /// multiplies the receiver before any tree vertices enter this pass.
    pub fn has_native_tree_scene_draws(&self) -> bool {
        self.enabled && (!self.tree_meshes.is_empty() || self.has_projected_tree_decals())
    }

    pub fn record_native_tree_scene(&self, pass: &mut RenderPass<'_>) {
        if self.enabled {
            self.record_projected_tree_decals(pass);
            self.record_tree_draws(pass);
        }
    }

    fn record_tree_draws(&self, pass: &mut RenderPass<'_>) {
        let shroud = self
            .shroud_gpu
            .bind_group
            .as_ref()
            .or(self.shroud_gpu.neutral_bind_group.as_ref());
        let (Some(tree_pipeline), Some(camera_bg), Some(atlas_bg), Some(shroud), Some(sway_bg)) = (
            self.tree_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.tree_atlas_bind_group.as_ref(),
            shroud,
            self.tree_sway_bind_group.as_ref(),
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
        pass.set_bind_group(2, shroud, &[]);
        pass.set_bind_group(3, sway_bg, &[]);
        for mesh in &self.tree_meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_vertex_buffer(1, mesh.dynamic_vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn has_visible_tree_mesh(&self) -> bool {
        self.tree_meshes.first().is_some_and(|mesh| mesh.index_count != 0)
    }

    pub(super) fn advance_tree_sway_uniform_for_draw(&mut self, paused: bool) {
        if paused {
            return;
        }
        let factors = self.tree_buffer.advance_sway_for_draw(false);
        if let (Some(queue), Some(buffer)) =
            (self.queue.as_ref(), self.tree_sway_uniform_buffer.as_ref())
        {
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&factors));
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
    pub fn update_tree_meshes(&mut self, client_random: Option<&mut game_engine::common::random_value::RandomState>) {
        self.update_tree_meshes_observed(None, client_random);
    }
    pub fn update_tree_meshes_for_motion(
        &mut self,
        motion: TerrainVisualMotion,
        client_random: Option<&mut game_engine::common::random_value::RandomState>,
    ) {
        self.update_tree_meshes_observed(Some(motion), client_random);
    }
    fn update_tree_meshes_observed(
        &mut self,
        motion: Option<TerrainVisualMotion>,
        client_random: Option<&mut game_engine::common::random_value::RandomState>,
    ) {
        self.rebind_skybox_background_for_camera();
        self.tree_buffer
            .tick_cpu(false, |_| TreeShroudStatus::Clear);
        self.finish_tree_meshes(motion, false, client_random);
    }
    pub(super) fn update_tree_meshes_after_cpu(
        &mut self,
        motion: Option<TerrainVisualMotion>,
        client_random: Option<&mut game_engine::common::random_value::RandomState>,
    ) {
        self.finish_tree_meshes(motion, true, client_random);
    }
    fn finish_tree_meshes(
        &mut self,
        motion: Option<TerrainVisualMotion>,
        sway_completed: bool,
        client_random: Option<&mut game_engine::common::random_value::RandomState>,
    ) {
        let moving_vertices_changed = self.tree_buffer.take_any_push_changed();
        if !sway_completed {
            // C++ `W3DTreeBuffer::drawTrees`: ScriptEngine breeze + versioned updateSway.
            let pause = motion.map(|motion| motion.paused).unwrap_or_else(|| {
                gamelogic::helpers::TheScriptEngine::is_time_frozen()
                    || gamelogic::helpers::TheGameLogic::is_game_paused()
            });
            if !pause {
                let breeze = motion
                    .map(|motion| motion.breeze)
                    .unwrap_or_else(script_tree_breeze);
                if let (true, Some(client_random)) = (
                    self.tree_buffer.cur_sway_version() != breeze.breeze_version,
                    client_random,
                ) {
                    self.tree_buffer.update_sway(breeze, &mut GameClientSwayRng(client_random));
                    self.overlay.last_sway_version = breeze.breeze_version;
                }
            }
        }
        let native_view = sway_completed && self.tree_buffer.has_native_view();
        if !native_view && !self.has_visible_tree_mesh() {
            if self
                .tree_buffer
                .trees()
                .iter()
                .any(|tree| tree.tree_type >= 0)
            {
                self.tree_buffer.force_vertex_rebuild();
            } else if !self.tree_buffer.need_to_update_texture()
                && self.tree_buffer.num_tiles() == 0
            {
                // C++ drawTrees processes an existing atlas even with no tree instances.
                // A prepared tile pack still needs the normal mip/LOD upload route.
                return;
            }
        }

        if (native_view || self.has_visible_tree_mesh())
            && !self.tree_buffer.need_to_update_texture()
            && !self.tree_buffer.anything_changed()
        {
            if moving_vertices_changed {
                self.update_tree_motion_gpu_ranges();
            }
            return;
        }

        // C++ `drawTrees`: if m_needToUpdateTexture then updateTexture (blit+mip+SetLOD).
        // GlobalData::m_textureReductionFactor when present; else last SetLOD / atlas_lod.
        let lod = get_global_data()
            .map(|global_data| global_data.read().texture_reduction_factor.clamp(0, 4))
            .unwrap_or_else(|| self.tree_buffer.atlas_lod());
        let repack_atlas = self.tree_buffer.need_to_update_texture()
            || self.tree_buffer.atlas_mips().is_empty();
        self.tree_buffer.sync_tree_atlas_for_draw(lod);
        let levels = self.tree_buffer.atlas_upload_levels();
        let (atlas_width, _) = self.tree_buffer.texture_size();
        let first_mip = self.tree_buffer.atlas_upload_mip_index();
        let upload_width = (atlas_width.max(1) as u32 >> first_mip).max(1);
        if repack_atlas
            || self.tree_atlas_texture.as_ref().is_none_or(|atlas| {
                atlas.width() != upload_width || atlas.mip_level_count() as usize != levels.len()
            })
        {
            self.upload_tree_atlas_texture();
        }
        if native_view && !self.tree_buffer.anything_changed() {
            return;
        }
        let lights = self.current_tree_object_lights();
        if native_view {
            // W3DTreeBuffer.cpp:675–685: the first full dirty vertex/index
            // build requests the shared decal after this pass captured its casters.
            if !self.tree_buffer.tree_types().is_empty() {
                self.request_projected_tree_decal_texture();
            }
            self.tree_buffer.load_trees_in_vertex_and_index_buffers(&lights);
        } else {
            self.tree_buffer
                .draw_trees_fill_vertex_buffer(&lights, Vec3::Z, |_| true);
        }
        self.last_tree_gpu_vertices = fill_tree_gpu_upload_vertices(self.tree_buffer.cpu_vertices());
        self.write_tree_gpu_geometry();
        self.tree_buffer.discard_pending_vertex_updates();
    }

    fn write_tree_gpu_geometry(&mut self) {
        let indices = self.tree_buffer.cpu_indices();
        let vertices = &self.last_tree_gpu_vertices;
        if vertices.is_empty() || indices.is_empty() {
            if let Some(mesh) = self.tree_meshes.first_mut() {
                mesh.index_count = 0;
            }
            return;
        }
        let (Some(device), Some(queue)) = (self.device.as_ref(), self.queue.as_ref()) else {
            return;
        };
        let static_bytes = (vertices.len() * std::mem::size_of::<TreeGpuVertexStatic>()) as u64;
        let dynamic_bytes = (vertices.len() * std::mem::size_of::<TreeGpuVertexDynamic>()) as u64;
        let index_bytes =
            (std::mem::size_of_val(indices) as u64).next_multiple_of(wgpu::COPY_BUFFER_ALIGNMENT);
        if let Some(mesh) = self.tree_meshes.first_mut()
            && mesh.vertex_buffer.size() >= static_bytes
            && mesh.dynamic_vertex_buffer.size() >= dynamic_bytes
            && mesh.index_buffer.size() >= index_bytes
        {
            let static_vertices: Vec<_> = vertices
                .iter()
                .copied()
                .map(TreeGpuVertex::static_attributes)
                .collect();
            let dynamic_vertices: Vec<_> = vertices
                .iter()
                .copied()
                .map(TreeGpuVertex::dynamic_attributes)
                .collect();
            queue.write_buffer(&mesh.vertex_buffer, 0, cast_slice(&static_vertices));
            queue.write_buffer(&mesh.dynamic_vertex_buffer, 0, cast_slice(&dynamic_vertices));

            // Uint16 triangle lists can contain an odd number of indices.
            // WGPU copies are four-byte aligned, while only index_count is drawn.
            let bytes = cast_slice(indices);
            if index_bytes == bytes.len() as u64 {
                queue.write_buffer(&mesh.index_buffer, 0, bytes);
            } else if let Some(mut upload) = queue.write_buffer_with(
                &mesh.index_buffer,
                0,
                wgpu::BufferSize::new(index_bytes).expect("nonempty tree index upload"),
            ) {
                upload.slice(..bytes.len()).copy_from_slice(bytes);
                upload
                    .slice(bytes.len()..)
                    .copy_from_slice(bytemuck::bytes_of(&0u16));
            } else {
                mesh.index_count = 0;
                return;
            }
            mesh.index_count = indices.len() as u32;
            return;
        }
        self.tree_meshes.clear();
        self.tree_meshes.extend(Self::upload_tree_mesh(device, vertices, indices));
    }

    fn update_tree_motion_gpu_ranges(&mut self) {
        let updates = self.tree_buffer.update_moving_tree_vertex_ranges();
        let Some(queue) = self.queue.as_ref().cloned() else {
            return;
        };
        let Some(mesh) = self.tree_meshes.first() else {
            return;
        };
        for update in updates {
            let end_vertex = update.first_vertex + update.dynamic_vertices.len();
            if end_vertex > self.last_tree_gpu_vertices.len() {
                continue;
            }
            let offset = update.first_vertex * std::mem::size_of::<TreeGpuVertexDynamic>();
            queue.write_buffer(
                &mesh.dynamic_vertex_buffer,
                offset as u64,
                cast_slice(&update.dynamic_vertices),
            );
            for (cached, dynamic) in self.last_tree_gpu_vertices
                [update.first_vertex..end_vertex]
                .iter_mut()
                .zip(update.dynamic_vertices)
            {
                cached.position = dynamic.position;
                cached.tree_effects[1] = dynamic.darkening;
            }
        }
    }

    fn upload_tree_mesh(
        device: &wgpu::Device,
        vertices: &[TreeGpuVertex],
        indices: &[u16],
    ) -> Option<GpuTreeMesh> {
        if vertices.is_empty() || indices.is_empty() {
            return None;
        }
        let static_vertices: Vec<_> = vertices
            .iter()
            .copied()
            .map(TreeGpuVertex::static_attributes)
            .collect();
        let dynamic_vertices: Vec<_> = vertices
            .iter()
            .copied()
            .map(TreeGpuVertex::dynamic_attributes)
            .collect();
        Some(GpuTreeMesh {
            vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Static Attributes"),
                contents: cast_slice(&static_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            dynamic_vertex_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Dynamic Position Attributes"),
                contents: cast_slice(&dynamic_vertices),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            }),
            index_buffer: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Tree Mesh Indices"),
                contents: cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
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
        self.tree_buffer.atlas_upload_levels()
    }

    fn upload_tree_atlas_texture(&mut self) {
        let (Some(device), Some(queue)) = (self.device.as_ref(), self.queue.as_ref()) else {
            self.tree_atlas_texture = None;
            self.tree_atlas_bind_group = None;
            return;
        };
        let levels = self.tree_buffer.atlas_upload_levels();
        if levels.is_empty() {
            self.tree_atlas_texture = None;
            self.tree_atlas_bind_group = None;
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
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
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

    fn sync_water_plane(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        if !self.config.water_enabled {
            self.water_plane = None;
            return Ok(());
        }
        // C++ Render() only draws the GlobalData-extent sea plane for
        // WATER_TYPE_2_PVSHADER (drawSea, W3DWater.cpp:1540-1543); the shipped
        // WATER_TYPE_0 path renders polygon-trigger water only.
        let Some(([min_x, min_z], [max_x, max_z], water_y)) = self.water_rules.sea_plane_bounds()
        else {
            self.water_plane = None;
            return Ok(());
        };

        // C++ WaterRenderObjClass::getClippedWaterPlane (W3DWater.cpp:1735-1738)
        // spans local [0..m_dx] x [0..m_dy] at m_level, then Set_Position
        // (waterPositionX, waterPositionY, waterPositionZ). Not centered
        // [-extent/2, +extent/2] — Alpine camera x=1362 must sit over the plane.
        // Tile 15×15 patches across GlobalData extents (do not stretch one sheet).
        let additive = self.water_presentation.additive_blending;
        let standing_tex = self.water_presentation.standing_water_texture.clone();
        self.water_additive_blend = additive;

        // C++ type-2 sea vertices use the ini TransparentDiffuseColor directly
        // (generateVertexBuffer, W3DWater.cpp:667), not the standing-light blend.
        let packed = pack_water_rgba_int(
            self.water_presentation
                .setting_at(self.water_rules.time_of_day)
                .transparent_diffuse_color,
        );

        let (mut patch_vertices, list_indices) = bake_water_tiles_world(
            min_x,
            min_z,
            max_x,
            max_z,
            water_y,
            packed,
        );
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
            vertex_byte_range: None,
            index_buffer,
            index_count: list_indices.len() as u32,
            texture_name: standing_tex,
            jba: false,
            river: false,
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
        let mut mesh = height_map.build_extra_blend_draw_mesh_for_window(
            &self.extra_blend_tile_positions,
            self.draw_origin_x,
            self.draw_origin_y,
            self.draw_width,
            self.draw_height,
        );
        for vertices in mesh.vertices.chunks_exact_mut(6) {
            let origin = vertices.iter().fold([f32::INFINITY; 2], |p, v| {
                [p[0].min(v.position[0]), p[1].min(v.position[2])]
            });
            let (x, y) = height_map_cell_at_world(
                height_map, origin[0] + height_map.scale * 0.5,
                origin[1] + height_map.scale * 0.5,
            );
            let index = height_map.get_extra_blend_tile_index(x, y);
            let Some(blend) = usize::try_from(index).ok()
                .and_then(|index| height_map.blended_tiles.get(index)) else { continue };
            let tile = blend.blend_ndx as i16;
            let cliff = class_tile_stretched(height_map, &self.source_tile_classes, x, y, tile);
            let mut flip = cpp_blend_corner_alpha(blend).1;
            if cliff {
                let h0 = height_map.get_raw_height(x, y) as i32;
                let h1 = height_map.get_raw_height(x + 1, y) as i32;
                let h2 = height_map.get_raw_height(x + 1, y + 1) as i32;
                let h3 = height_map.get_raw_height(x, y + 1) as i32;
                flip |= (h0 - h2).abs() > (h1 - h3).abs();
            }
            let mut corners = [vertices[0]; 4];
            for &vertex in vertices.iter() {
                let fx = if vertex.position[0] > origin[0] { 1.0 } else { 0.0 };
                let fy = if vertex.position[2] > origin[1] { 1.0 } else { 0.0 };
                corners[cell_corner(fx, fy)] = vertex;
            }
            let order = if flip { [1, 3, 0, 1, 2, 3] } else { [0, 2, 3, 0, 1, 2] };
            for (vertex, corner) in vertices.iter_mut().zip(order) {
                *vertex = corners[corner];
            }
        }
        mesh
    }

    fn extra_blend_mesh_to_terrain_vertices(
        &self,
        mesh: &ExtraBlendDrawMesh,
        tile_slots: Option<&[u16]>,
    ) -> Vec<TerrainVertex> {
        // C++ extra-blend tiles bake through getStaticDiffuse (doTheLight):
        // every global light contributes clamped N·L diffuse.
        let lighting = self.chunk_manager.static_lighting();
        let stable_ids = self.active_chunk_texture_ids.unwrap_or([0; MAX_TEXTURES_PER_CHUNK]);
        let mut slots = HashMap::new();
        for (slot, id) in stable_ids.iter().enumerate() {
            slots.entry(*id).or_insert(slot);
        }
        mesh.vertices
            .iter()
            .enumerate()
            .map(|(vertex_index, vertex)| {
                let position = Vec3::from_array(vertex.position);
                let mut normal = Vec3::Y;
                let mut uv = vertex.tex_coords;
                let mut slot = 0;
                if let Some(hm) = self.height_map.as_ref() {
                    // The draw mesh expands each tile to six indexed vertices.
                    let first = vertex_index / 6 * 6;
                    let origin = mesh.vertices[first..first + 6].iter().fold(
                        [f32::INFINITY; 2],
                        |p, v| [p[0].min(v.position[0]), p[1].min(v.position[2])],
                    );
                    let (x, y) = height_map_cell_at_world(
                        hm, origin[0] + hm.scale * 0.5, origin[1] + hm.scale * 0.5,
                    );
                    let corner_x = u32::from(position.x > origin[0]);
                    let corner_y = u32::from(position.z > origin[1]);
                    normal = hm.render_normal_at_sample(x as u32 + corner_x, y as u32 + corner_y);
                    let index = hm.get_extra_blend_tile_index(x, y);
                    if index > 0 {
                        if let Some(blend) = hm.blended_tiles.get(index as usize) {
                            let tile = blend.blend_ndx as i16;
                            let packed = ((blend.blend_ndx >> 2).max(0)) as u32;
                            if let Some(id) = bound_texture_id_for_source_tile(
                                packed, &self.source_tile_classes, &self.texture_system,
                                &stable_ids, &slots,
                            ) {
                                slot = slots.get(&id).copied().unwrap_or(0) as u16;
                            }
                            let fx = ((vertex.position[0] - origin[0]) / hm.scale).clamp(0.0, 1.0);
                            let fy = ((vertex.position[2] - origin[1]) / hm.scale).clamp(0.0, 1.0);
                            uv = class_uv_for_tile(hm, &self.source_tile_classes, x, y, tile, fx, fy);
                        }
                    }
                }
                if let Some(slots) = tile_slots {
                    slot = slots[vertex_index / 6];
                }
                let mut color = Self::terrain_static_diffuse_from_normal(
                    normal,
                    &lighting.lights,
                    lighting.ambient,
                );
                color[3] = vertex.color[3];
                TerrainVertex {
                    position: vertex.position,
                    normal: [normal.x, normal.y, normal.z],
                    tex_coords: uv,
                    blend_indices: [slot, 0, 0, 0],
                    blend_weights: [1.0, 0.0, 0.0, 0.0],
                    blend_tex_coords: vertex.tex_coords,
                    blend_texture_slot: u32::MAX,
                    color,
                }
            })
            .collect()
    }

    /// Stage extra-blend overlay data and upload its draw geometry.
    /// C++ draws this geometry as a second 3-way blend pass.
    pub fn upload_extra_blend_overlay(&mut self) {
        self.extra_blend_upload_key = None;
        self.rebuild_extra_blend_gpu_state();
        let mesh = self.build_extra_blend_draw_mesh();
        self.extra_blend_gpu_upload = ExtraBlendGpuUpload {
            tile_count: self.extra_blend_tile_positions.len(),
            positions: self.extra_blend_tile_positions.clone(),
            vertex_count: mesh.vertex_count(),
            index_count: mesh.index_count(),
        };
        self.extra_blend_draw_mesh = mesh;
        self.extra_blend_vertex_count = self.extra_blend_draw_mesh.vertex_count() as u32;
        self.extra_blend_index_count = self.extra_blend_draw_mesh.index_count() as u32;
        self.extra_blend_batches.clear();

        let Some(device) = self.device.clone() else {
            self.extra_blend_vertex_buffer = None;
            self.extra_blend_index_buffer = None;
            return;
        };
        if self.extra_blend_tile_positions.is_empty() || self.extra_blend_draw_mesh.is_empty() {
            self.extra_blend_vertex_buffer = None;
            self.extra_blend_index_buffer = None;
            return;
        }
        let mut tile_slots = Vec::new();
        if let Some(queue) = self.queue.clone() {
            let class_ids: Vec<_> = self.source_tile_classes.iter()
                .map(|class| self.texture_id_for_class_name(&class.name))
                .collect();
            let fallback = self.active_chunk_texture_ids.map_or(0, |ids| ids[0]);
            let mut ids = Vec::new();
            let mut groups: Vec<(usize, usize, Vec<TextureId>)> = Vec::new();
            let mut start = 0;
            for (tile, vertices) in self.extra_blend_draw_mesh.vertices.chunks_exact(6).enumerate() {
                let id = self.height_map.as_ref().and_then(|hm| {
                    let origin = vertices.iter().fold([f32::INFINITY; 2], |p, vertex| {
                        [p[0].min(vertex.position[0]), p[1].min(vertex.position[2])]
                    });
                    let (x, y) = height_map_cell_at_world(
                        hm, origin[0] + hm.scale * 0.5, origin[1] + hm.scale * 0.5,
                    );
                    let index = hm.get_extra_blend_tile_index(x, y);
                    let blend = hm.blended_tiles.get(index as usize)?;
                    let packed = (blend.blend_ndx >> 2).max(0) as u32;
                    let class = texture_class_index_from_ndx(packed, &self.source_tile_classes)?;
                    class_ids[class]
                }).unwrap_or(fallback);
                if !ids.contains(&id) && ids.len() == MAX_TEXTURES_PER_CHUNK {
                    groups.push((start, tile, std::mem::take(&mut ids)));
                    start = tile;
                }
                let slot = match ids.iter().position(|candidate| *candidate == id) {
                    Some(slot) => slot,
                    None => { ids.push(id); ids.len() - 1 }
                };
                tile_slots.push(slot as u16);
            }
            if !ids.is_empty() {
                groups.push((start, tile_slots.len(), ids));
            }
            for (start, end, ids) in groups {
                let mut packed = [ids[0]; MAX_TEXTURES_PER_CHUNK];
                let mut slots = HashMap::new();
                for (slot, id) in ids.into_iter().enumerate() {
                    packed[slot] = id;
                    slots.insert(id, slot);
                }
                match self.prepare_chunk_texture_binding(0, &packed, &slots, device.as_ref(), queue.as_ref()) {
                    Ok(binding) => self.extra_blend_batches.push(GpuTerrainBatch {
                        indices: (start as u32 * 6)..(end as u32 * 6),
                        bind_group: binding.bind_group,
                    }),
                    Err(error) => {
                        warn!("Failed to prepare extra terrain blend materials: {}", error);
                        self.extra_blend_batches.clear();
                        self.extra_blend_vertex_buffer = None;
                        self.extra_blend_index_buffer = None;
                        return;
                    }
                }
            }
        }
        let gpu_vertices = self.extra_blend_mesh_to_terrain_vertices(
            &self.extra_blend_draw_mesh,
            (!tile_slots.is_empty()).then_some(tile_slots.as_slice()),
        );
        Self::write_reusable_terrain_buffer(
            &device,
            self.queue.as_deref(),
            &mut self.extra_blend_vertex_buffer,
            "Terrain Extra Blend Vertices",
            cast_slice(&gpu_vertices),
            wgpu::BufferUsages::VERTEX,
        );
        Self::write_reusable_terrain_buffer(
            &device,
            self.queue.as_deref(),
            &mut self.extra_blend_index_buffer,
            "Terrain Extra Blend Indices",
            cast_slice(&self.extra_blend_draw_mesh.indices),
            wgpu::BufferUsages::INDEX,
        );
    }

    fn write_reusable_terrain_buffer(
        device: &wgpu::Device,
        queue: Option<&wgpu::Queue>,
        buffer: &mut Option<wgpu::Buffer>,
        label: &str,
        contents: &[u8],
        usage: wgpu::BufferUsages,
    ) {
        if let (Some(queue), Some(existing)) = (queue, buffer.as_ref()) {
            if existing.size() >= contents.len() as u64 {
                queue.write_buffer(existing, 0, contents);
                return;
            }
        }
        *buffer = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some(label),
            contents,
            usage: usage | wgpu::BufferUsages::COPY_DST,
        }));
    }

    /// Increment the shipped extra-blend draw counter when overlay tiles exist.
    /// Returns whether a draw was recorded.
    pub fn extra_blend_draw(&self) -> bool {
        if self.use_3way_terrain_blends == 0 {
            return false;
        }
        if self.extra_blend_tile_count() == 0
            && self.extra_blend_gpu_upload.is_empty()
            && self.extra_blend_draw_mesh.is_empty()
        {
            return false;
        }
        self.extra_blend_draw_count.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// Second extra-blend pass over the base terrain (alpha overlay, no Z write).
    pub fn record_extra_blend_pass(&self, pass: &mut RenderPass<'_>) {
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

        pass.set_pipeline(pipeline);
        if let Some(camera_bg) = &self.terrain_camera_bind_group {
            pass.set_bind_group(0, camera_bg, &[]);
        }
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        if self.extra_blend_batches.is_empty() {
            // The device-only test seam can supply an already-bound material.
            if let Some(binding) = self.chunk_texture_bindings.values().next() {
                pass.set_bind_group(1, &binding.bind_group, &[]);
                pass.draw_indexed(0..self.extra_blend_index_count, 0, 0..1);
            }
        } else {
            for batch in &self.extra_blend_batches {
                pass.set_bind_group(1, &batch.bind_group, &[]);
                pass.draw_indexed(batch.indices.clone(), 0, 0..1);
            }
        }
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
        let in_buffer = self.scorches.scorches_in_buffer;
        let count = self.scorches.len();
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
struct GameClientSwayRng<'a>(&'a mut game_engine::common::random_value::RandomState);

impl crate::terrain::TreeRandom for GameClientSwayRng<'_> {
    fn int_range(&mut self, min: i32, max: i32) -> i32 {
        self.0.next_int(min, max)
    }

    fn real_range(&mut self, min: f32, max: f32) -> f32 {
        self.0.next_real(min, max)
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
/// C++ `WorldHeightMap::getAlphaUVData` (WorldHeightMap.cpp:2042-2134) corner
/// coverage table: 255 = the blend texture covers the corner, 0 = the base
/// texture shows. `BlendTileInfo.inverted` packs INVERTED_MASK 0x1 /
/// FLIPPED_MASK 0x2 (TileData.h:27-28). Returns the blend-texture coverage
/// (0..1) at the vertex sitting at fraction (fx, fy) inside the cell, with
/// corner 0=(x0,z0), 1=(x1,z0), 2=(x1,z1), 3=(x0,z1) matching the C++
/// U/V corner ordering. Edge gradients stand in for the alpha-edge texture
/// art (the diagonal alpha halves across the cell, flipped covers invert);
/// `longDiagonal` doubles the gradient so the blend spans the whole cell.
/// `customBlendEdgeClass >= 0` disables the alpha blend entirely (C++ :2023).
/// Cell corner (C++ vertex order 0..3: (x0,y0), (x1,y0), (x1,y1), (x0,y1))
/// of a vertex at fraction `(fx, fy)` inside its cell.
fn cell_corner(fx: f32, fy: f32) -> usize {
    match (fx > 0.5, fy > 0.5) {
        (false, false) => 0,
        (true, false) => 1,
        (true, true) => 2,
        (false, true) => 3,
    }
}

/// C++ `WorldHeightMap::getAlphaUVData` (WorldHeightMap.cpp:2042-2134): the
/// blend layer's per-corner alpha (0..1) and whether the cell's triangles
/// are split along the other diagonal (`needFlip`).
fn cpp_blend_corner_alpha(blend: &crate::terrain::textures::BlendTileInfo) -> ([f32; 4], bool) {
    use crate::terrain::textures::{FLIPPED_MASK, INVERTED_MASK};
    let inverted = blend.inverted & INVERTED_MASK != 0;
    let mut alpha = [0u8; 4];
    let mut flip = false;
    if blend.horiz != 0 {
        flip = blend.inverted & FLIPPED_MASK != 0;
        if inverted {
            alpha[0] = 255;
            alpha[3] = 255;
        } else {
            alpha[1] = 255;
            alpha[2] = 255;
        }
    }
    if blend.vert != 0 {
        flip = blend.inverted & FLIPPED_MASK != 0;
        if inverted {
            alpha[0] = 255;
            alpha[1] = 255;
        } else {
            alpha[2] = 255;
            alpha[3] = 255;
        }
    }
    if blend.right_diagonal != 0 {
        if inverted {
            alpha[1] = 255;
            if blend.long_diagonal != 0 {
                alpha[0] = 255;
                alpha[2] = 255;
            }
        } else {
            flip = true;
            alpha[2] = 255;
            if blend.long_diagonal != 0 {
                alpha[1] = 255;
                alpha[3] = 255;
            }
        }
    }
    if blend.left_diagonal != 0 {
        if inverted {
            flip = true;
            alpha[0] = 255;
            if blend.long_diagonal != 0 {
                alpha[1] = 255;
                alpha[3] = 255;
            }
        } else {
            alpha[3] = 255;
            if blend.long_diagonal != 0 {
                alpha[0] = 255;
                alpha[2] = 255;
            }
        }
    }
    if blend.custom_blend_edge_class >= 0 {
        alpha = [0; 4];
        flip = false;
    }
    (alpha.map(|a| f32::from(a) / 255.0), flip)
}

fn unique_bound_texture_ids(
    stable_ids: &[TextureId; MAX_TEXTURES_PER_CHUNK],
    shared_slot_map: &HashMap<TextureId, usize>,
) -> ([TextureId; MAX_TEXTURES_PER_CHUNK], usize) {
    let mut unique = [0; MAX_TEXTURES_PER_CHUNK];
    let mut len = 0;
    for &id in stable_ids {
        if shared_slot_map.contains_key(&id) && !unique[..len].contains(&id) {
            unique[len] = id;
            len += 1;
        }
    }
    (unique, len)
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

/// Expand the shared-vertex cell grid (`p0 p1 p2 / p0 p2 p3` per cell) into
/// four vertices per cell. Returns the cell centre (world x/z) per vertex.
fn expand_cells_to_unique_vertices(
    vertices: Vec<TerrainVertex>,
    indices: Vec<u32>,
    colors: Vec<[f32; 4]>,
) -> (Vec<TerrainVertex>, Vec<u32>, Vec<[f32; 4]>, Vec<[f32; 2]>) {
    let is_cell_list = indices.len() % 6 == 0
        && indices
            .chunks_exact(6)
            .all(|q| q[3] == q[0] && q[4] == q[2]);
    if !is_cell_list {
        let centers = vertices
            .iter()
            .map(|v| [v.position[0], v.position[2]])
            .collect();
        return (vertices, indices, colors, centers);
    }
    let cells = indices.len() / 6;
    let mut out_vertices = Vec::with_capacity(cells * 4);
    let mut out_colors = Vec::with_capacity(cells * 4);
    let mut centers = Vec::with_capacity(cells * 4);
    let mut out_indices = Vec::with_capacity(indices.len());
    for quad in indices.chunks_exact(6) {
        let corners = [quad[0], quad[1], quad[2], quad[5]];
        let Some(corner_vertices) = corners
            .iter()
            .map(|&i| vertices.get(i as usize).copied())
            .collect::<Option<Vec<_>>>()
        else {
            continue;
        };
        let center = corner_vertices.iter().fold([0.0f32; 2], |acc, v| {
            [acc[0] + v.position[0] * 0.25, acc[1] + v.position[2] * 0.25]
        });
        let base = u32::try_from(out_vertices.len()).unwrap_or(u32::MAX);
        for (&i, vertex) in corners.iter().zip(corner_vertices) {
            out_colors.push(colors.get(i as usize).copied().unwrap_or(vertex.color));
            out_vertices.push(vertex);
            centers.push(center);
        }
        out_indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }
    (out_vertices, out_indices, out_colors, centers)
}

/// C++ getAlphaUVData supplies a second tile's UVs independently of the base.
/// Keep both weight entries even when the tiles share a texture class.
fn authored_blend_vertex(
    hm: &HeightMap,
    classes: &[TerrainSourceTileClass],
    textures: &TerrainTextures,
    stable_ids: &[TextureId; MAX_TEXTURES_PER_CHUNK],
    slots: &HashMap<TextureId, usize>,
    center: [f32; 2],
    position: [f32; 2],
) -> Option<(u16, u16, f32, [f32; 2])> {
    let (x, y) = height_map_cell_at_world(hm, center[0], center[1]);
    let blend_index = hm.get_blend_tile_index(x, y);
    if blend_index <= 0 {
        return None;
    }
    let blend = hm.blended_tiles.get(blend_index as usize)?;
    let base = hm.get_packed_terrain_tile_at_world(center[0], center[1]);
    let other = (blend.blend_ndx >> 2).max(0) as u32;
    let base_id = bound_texture_id_for_source_tile(base, classes, textures, stable_ids, slots)?;
    let blend_id = bound_texture_id_for_source_tile(other, classes, textures, stable_ids, slots)?;
    let base_slot = *slots.get(&base_id)? as u16;
    let blend_slot = *slots.get(&blend_id)? as u16;
    let scale = if hm.scale.abs() > f32::EPSILON {
        hm.scale
    } else {
        gamelogic::common::types::MAP_XY_FACTOR
    };
    let fx = (position[0] / scale + hm.border_size as f32 - x as f32).clamp(0.0, 1.0);
    let fy = (position[1] / scale + hm.border_size as f32 - y as f32).clamp(0.0, 1.0);
    let alpha = cpp_blend_corner_alpha(blend).0[cell_corner(fx, fy)];
    let uv = class_uv_for_tile(hm, classes, x, y, blend.blend_ndx as i16, fx, fy);
    Some((base_slot, blend_slot, alpha, uv))
}

/// C++ WorldHeightMap caches primary getAlphaUVData's triangle orientation
/// for the receiver meshes (WorldHeightMap.cpp:2168–2181). Derive the same
/// orientation from the admitted map and classes so ground and overlays
/// agree, including after height edits and texture-class replacement.
struct TerrainReceiverHeight<'a> {
    map: &'a HeightMap,
    classes: &'a [TerrainSourceTileClass],
}

impl ScorchHeightSource for TerrainReceiverHeight<'_> {
    fn x_extent(&self) -> i32 {
        self.map.x_extent()
    }

    fn y_extent(&self) -> i32 {
        self.map.y_extent()
    }

    fn border_size(&self) -> i32 {
        self.map.border_size()
    }

    fn clip_height_world(&self, x: i32, y: i32) -> f32 {
        self.map.clip_height_world(x, y)
    }

    fn flip_state(&self, x: i32, y: i32) -> bool {
        let blend_index = self.map.get_blend_tile_index(x, y);
        let blend = (blend_index > 0)
            .then(|| self.map.blended_tiles.get(blend_index as usize))
            .flatten();
        let tile = blend.map_or_else(|| self.map.get_tile_index(x, y), |info| info.blend_ndx as i16);
        authored_cell_flip(self.map, self.classes, x, y, tile, blend)
    }
}

fn authored_cell_flip(
    hm: &HeightMap,
    classes: &[TerrainSourceTileClass],
    x: i32,
    y: i32,
    tile: i16,
    blend: Option<&crate::terrain::textures::BlendTileInfo>,
) -> bool {
    let flip = blend.is_some_and(|blend| cpp_blend_corner_alpha(blend).1);
    let stretched = class_tile_stretched(hm, classes, x, y, tile);
    if !stretched {
        return flip;
    }
    let h0 = hm.get_raw_height(x, y) as i32;
    let h1 = hm.get_raw_height(x + 1, y) as i32;
    let h2 = hm.get_raw_height(x + 1, y + 1) as i32;
    let h3 = hm.get_raw_height(x, y + 1) as i32;
    (h0 - h2).abs() > (h1 - h3).abs()
}

/// Whether getUVForTileIndex returned TRUE, rather than merely having
/// produced UV coordinates. A missing source class cannot be stretched.
fn class_tile_stretched(hm: &HeightMap, classes: &[TerrainSourceTileClass], x: i32, y: i32, tile: i16) -> bool {
    let packed = ((tile as i32 >> 2).max(0)) as u32;
    if texture_class_index_from_ndx(packed, classes).is_none() {
        return false;
    }
    match cliff_info_for_tile(hm, classes, x, y, tile) {
        Some(info) => info.flip,
        None => hm.get_uv_for_tile_index((y * hm.width as i32 + x) as usize, tile, false).stretched,
    }
}

/// An authored cliff applies to every tile in its texture class, not just
/// the exact source tile (WorldHeightMap.cpp:1724-1751).
fn cliff_info_for_tile<'a>(
    hm: &'a HeightMap,
    classes: &[TerrainSourceTileClass],
    x: i32,
    y: i32,
    tile: i16,
) -> Option<&'a crate::terrain::TCliffInfo> {
    if !hm.adjust_cliff_textures { return None; }
    let index = hm.cliff_info_ndxes.get((y * hm.width as i32 + x) as usize).copied()?;
    if index <= 0 { return None; }
    let info = hm.cliff_info.get(index as usize)?;
    let tile_class = texture_class_index_from_ndx((i32::from(tile) >> 2).max(0) as u32, classes)?;
    let cliff_class = texture_class_index_from_ndx((info.tile_index >> 2).max(0) as u32, classes)?;
    (tile_class == cliff_class).then_some(info)
}

fn class_uv_for_tile(
    hm: &HeightMap,
    classes: &[TerrainSourceTileClass],
    x: i32,
    y: i32,
    tile: i16,
    fx: f32,
    fy: f32,
) -> [f32; 2] {
    if let Some(info) = cliff_info_for_tile(hm, classes, x, y, tile) {
        let class_index = texture_class_index_from_ndx((i32::from(tile) >> 2).max(0) as u32, classes).unwrap();
        let class_width = classes[class_index].width.max(1) as f32;
        let factor = crate::terrain::textures::TEXTURE_WIDTH as f32
            / (class_width * crate::terrain::textures::TILE_PIXEL_EXTENT as f32);
        let interpolate = |values: [f32; 4]| {
            values[0] * (1.0 - fx) * (1.0 - fy)
                + values[1] * fx * (1.0 - fy)
                + values[2] * fx * fy
                + values[3] * (1.0 - fx) * fy
        };
        return [
            interpolate([info.u0, info.u1, info.u2, info.u3]) * factor,
            1.0 + interpolate([info.v0, info.v1, info.v2, info.v3]) * factor,
        ];
    }
    let packed = ((tile as i32 >> 2).max(0)) as u32;
    let class = texture_class_index_from_ndx(packed, classes).and_then(|i| classes.get(i));
    let uv = hm.get_uv_for_tile_index((y * hm.width as i32 + x) as usize, tile, false);
    let (mut u, mut v) = if class.is_some() {
        (uv.u, uv.v)
    } else {
        // No source class: C++ exits before the old cliff adjustment.
        let ou = if tile & 1 != 0 { 0.5 } else { 0.0 };
        let ov = if tile & 2 != 0 { 0.0 } else { 0.5 };
        ([ou, ou + 0.5, ou + 0.5, ou], [ov + 0.5, ov + 0.5, ov, ov])
    };
    if let Some(class) = class {
        let width = class.width.max(1);
        let index = (packed as i32 - class.first_tile).max(0);
        let column = (index % width) as f32;
        let row = (width - 1 - (index / width).min(width - 1)) as f32;
        for corner in 0..4 {
            u[corner] = (u[corner] + column) / width as f32;
            v[corner] = (v[corner] + row) / width as f32;
        }
        if uv.stretched {
            // The one-high/one-low branch uses the *whole class* V span,
            // not the 32-pixel quarter-tile span used by HeightMap's UVs.
            let heights = [
                hm.get_raw_height(x, y) as i32,
                hm.get_raw_height(x + 1, y) as i32,
                hm.get_raw_height(x + 1, y + 1) as i32,
                hm.get_raw_height(x, y + 1) as i32,
            ];
            let min = *heights.iter().min().unwrap();
            let max = *heights.iter().max().unwrap();
            let delta = max - min;
            let below = heights.iter().filter(|&&h| h < min + (2 * delta + 1) / 3).count();
            let above = heights.iter().filter(|&&h| h > min + (delta + 1) / 3).count();
            let extreme = if below == 1 || above > below {
                Some(min)
            } else if above == 1 || below > above {
                Some(max)
            } else {
                None
            };
            if let Some(extreme) = extreme {
                let corner = heights.iter().position(|&h| h == extreme).unwrap();
                let scale = gamelogic::common::types::MAP_HEIGHT_SCALE
                    / gamelogic::common::types::MAP_XY_FACTOR;
                let divisor = (4.0 / (delta as f32 * scale)).clamp(1.0, 4.0);
                let ov = if tile & 2 != 0 { 0.0 } else { 0.5 };
                let n_v = (row + ov) / width as f32;
                let x_v = (row + ov + 0.5) / width as f32;
                v[corner] = if corner < 2 { n_v + 1.0 / divisor } else { x_v - 1.0 / divisor };
            }
            // WorldHeightMap::getUVForTileIndex shifts the entire stretched
            // quad back inside its source class, preserving its shape.
            let below = v.iter().fold(0.0f32, |shift, &value| shift.max(-value));
            for value in &mut v { *value += below; }
            let right = u.iter().fold(0.0f32, |shift, &value| shift.max(value - 1.0));
            let top = v.iter().fold(0.0f32, |shift, &value| shift.max(value - 1.0));
            for value in &mut u { *value -= right; }
            for value in &mut v { *value -= top; }
        }
    }
    let interpolate = |values: [f32; 4]| {
        values[0] * (1.0 - fx) * (1.0 - fy)
            + values[1] * fx * (1.0 - fy)
            + values[2] * fx * fy
            + values[3] * (1.0 - fx) * fy
    };
    [interpolate(u), interpolate(v)]
}

/// UV of a cell vertex in its texture class's source image: the cell's
/// quarter of its 64-px tile (`getUVForNdx`), offset to that tile's place in
/// the class grid (`readTiles`: tile = firstTile + column + row * width).
fn cell_class_uv(
    hm: &crate::terrain::height_map::HeightMap,
    classes: &[TerrainSourceTileClass],
    center: [f32; 2],
    position: [f32; 2],
) -> [f32; 2] {
    let (ix, iy, _, _) = hm.cell_grid_at_world(center[0], center[1]);
    let scale = if hm.scale.abs() > f32::EPSILON {
        hm.scale
    } else {
        gamelogic::common::types::MAP_XY_FACTOR
    };
    // Quads that straddle a cell line keep extrapolating the centre cell's
    // UV (chunk grids are not cell-aligned) instead of folding at the edge.
    let fx = (position[0] / scale + hm.border_size as f32 - ix as f32).clamp(-0.5, 1.5);
    let fy = (position[1] / scale + hm.border_size as f32 - iy as f32).clamp(-0.5, 1.5);
    let index = (iy * hm.width as i32 + ix) as usize;
    match hm.tile_ndxes.get(index) {
        Some(&tile) => class_uv_for_tile(hm, classes, ix, iy, tile, fx, fy),
        None => hm.cell_uv(ix, iy, fx, fy),
    }
}

fn bound_texture_id_for_source_tile(
    packed_tile: u32,
    classes: &[TerrainSourceTileClass],
    textures: &TerrainTextures,
    stable_ids: &[TextureId; MAX_TEXTURES_PER_CHUNK],
    shared_slot_map: &HashMap<TextureId, usize>,
) -> Option<TextureId> {
    let (unique, len) = unique_bound_texture_ids(stable_ids, shared_slot_map);
    if len == 0 {
        return None;
    }

    let Some(class_idx) = texture_class_index_from_ndx(packed_tile, classes) else {
        // Do not treat the tile index as a slot (tile%4 / tile/4). Unmapped
        // tiles leave TextureId unset so the caller keeps generated weights.
        return None;
    };

    if let Some(class) = classes.get(class_idx) {
        for &id in &unique[..len] {
            if textures.get_texture(id).is_some_and(|texture| {
                texture.name.eq_ignore_ascii_case(class.name.trim())
            }) {
                return Some(id);
            }
        }
        for &id in &unique[..len] {
            if let Some(texture) = textures.get_texture(id) {
                if texture_matches_class_name(texture, &class.name) {
                    return Some(id);
                }
            }
        }
    }

    // Preserve the existing missing-art fallback when no class art resolved.
    Some(unique[class_idx % len])
}

/// C++ `loadSetting` packs `RGBAColorInt` as A<<24 | R<<16 | G<<8 | B.
fn pack_water_rgba_int(color: (f32, f32, f32, f32)) -> u32 {
    let r = color.0.round().clamp(0.0, 255.0) as u32;
    let g = color.1.round().clamp(0.0, 255.0) as u32;
    let b = color.2.round().clamp(0.0, 255.0) as u32;
    let a = color.3.round().clamp(0.0, 255.0) as u32;
    (a << 24) | (r << 16) | (g << 8) | b
}

/// Combine contiguous material runs without changing the triangle order of
/// translucent road sections. The color and shroud passes replay these buffers.
struct RoadMeshBatches<'a> {
    device: &'a wgpu::Device,
    meshes: Vec<GpuRoadMesh>,
    vertices: Vec<RoadVertex>,
    indices: Vec<u32>,
    texture_name: String,
    diffuse_mode: RoadDiffuseMode,
    vertex_limit: usize,
    index_limit: usize,
}

impl<'a> RoadMeshBatches<'a> {
    fn new(device: &'a wgpu::Device) -> Self {
        let max_bytes = device.limits().max_buffer_size;
        let element_limit = |size: u64| (max_bytes / size).min(u64::from(u32::MAX)) as usize;
        Self {
            device,
            meshes: Vec::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            texture_name: String::new(),
            diffuse_mode: RoadDiffuseMode::Compatibility,
            vertex_limit: element_limit(std::mem::size_of::<RoadVertex>() as u64),
            index_limit: element_limit(std::mem::size_of::<u32>() as u64),
        }
    }

    fn append(
        &mut self,
        texture_name: &str,
        diffuse_mode: RoadDiffuseMode,
        vertices: &[RoadVertex],
        indices: impl ExactSizeIterator<Item = u32>,
    ) {
        if !self.vertices.is_empty()
            && (!self.texture_name.eq_ignore_ascii_case(texture_name)
                || self.diffuse_mode != diffuse_mode
                || self.vertices.len().saturating_add(vertices.len()) > self.vertex_limit
                || self.indices.len().saturating_add(indices.len()) > self.index_limit)
        {
            self.flush();
        }
        if self.vertices.is_empty() {
            self.texture_name = texture_name.to_owned();
            self.diffuse_mode = diffuse_mode;
        }
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(vertices);
        self.indices.extend(indices.map(|index| base + index));
    }

    fn flush(&mut self) {
        if let Some(mut mesh) = TerrainVisualImpl::upload_overlay_mesh(
            self.device,
            "Road Mesh",
            &self.vertices,
            &self.indices,
        ) {
            mesh.texture_name = std::mem::take(&mut self.texture_name);
            mesh.diffuse_mode = self.diffuse_mode;
            self.meshes.push(mesh);
        }
        self.vertices.clear();
        self.indices.clear();
    }

    fn finish(mut self) -> Vec<GpuRoadMesh> {
        self.flush();
        self.meshes
    }
}

/// Resolve the exact Roads.ini texture for a map road or bridge. Synthetic
/// joins carry the source RoadTypeId, so they keep the same source art.
fn authored_road_texture_name<'a>(
    road_name: &str,
    metadata: Option<&str>,
    roads: &'a game_engine::common::ini::ini_road::TerrainRoadPresentation,
) -> &'a str {
    let road_type_id = metadata.and_then(|metadata| {
        metadata
            .split_whitespace()
            .find_map(|part| part.strip_prefix("RoadTypeId="))
            .and_then(|value| value.parse::<u32>().ok())
    });
    roads
        .roads
        .iter()
        .find(|road| road.name.eq_ignore_ascii_case(road_name))
        .or_else(|| road_type_id.and_then(|id| roads.roads.iter().find(|road| road.id == id)))
        .or_else(|| {
            roads
                .bridges
                .iter()
                .find(|bridge| bridge.name.eq_ignore_ascii_case(road_name))
        })
        .map(|road| road.texture.as_str().trim())
        .unwrap_or_default()
}

#[cfg(test)]
#[path = "authored_road_texture_tests.rs"]
mod authored_road_texture_tests;

#[cfg(test)]
#[path = "splat_texture_class_tests.rs"]
mod splat_texture_class_tests;

#[cfg(test)]
#[path = "cpp_blend_corner_alpha_tests.rs"]
mod cpp_blend_corner_alpha_tests;
