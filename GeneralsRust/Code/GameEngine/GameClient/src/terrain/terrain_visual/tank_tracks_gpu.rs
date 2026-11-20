#[derive(Default)]
struct TankTrackDraws {
    materials: HashMap<String, Option<BindGroup>>,
    flush: crate::terrain::terrain_tracks::TerrainTracksFlush,
    vertices: Vec<RoadVertex>,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
}

impl TerrainVisualImpl {
    /// Age the receiving tread pool at the WW3D clock published before this Display.
    pub fn age_tank_tracks(&mut self, previous_sync_ms: u32) {
        self.terrain_tracks.update(previous_sync_ms as i32);
    }

    /// C++ W3DTerrainTracks::Render and ::flush. Build the bands in used-list
    /// order after the displayed world has completed its Drawable callbacks.
    pub fn prepare_tank_track_draws(
        &mut self,
        mut read_asset: impl FnMut(&str) -> Option<Vec<u8>>,
    ) {
        let draws = &mut self.tank_track_draws;
        draws.flush.ranges.clear();
        if !self.enabled || self.height_map.is_none() {
            return;
        }
        let (Some(device), Some(queue), Some(layout), Some(sampler)) = (
            self.device.as_deref(),
            self.queue.as_deref(),
            self.road_texture_bind_group_layout.as_deref(),
            self.road_sampler.as_ref(),
        ) else {
            return;
        };
        let lighting = self.chunk_manager.static_lighting();
        let diffuse = crate::terrain::scorch_mesh::scorch_diffuse_bgra(
            lighting.ambient,
            lighting.scorch_diffuse,
        );
        self.terrain_tracks
            .prepare_visible_draws(diffuse, &mut draws.flush);
        if draws.flush.ranges.is_empty() {
            return;
        }
        for range in &mut draws.flush.ranges {
            range.texture_name.make_ascii_lowercase();
            if draws.materials.contains_key(&range.texture_name) {
                continue;
            }
            let filename = range.texture_name.replace('\\', "/");
            let asset_path = if filename.contains('/') {
                filename
            } else {
                format!("Art/Textures/{filename}")
            };
            let path = std::path::Path::new(&asset_path);
            let texture = ["dds", "tga"].into_iter().find_map(|extension| {
                let path = path.with_extension(extension);
                let bytes = read_asset(path.to_str()?)?;
                Self::upload_runtime_texture(device, queue, &path, &bytes, u32::MAX).ok()
            });
            let material = texture.map(|texture| {
                let view = texture.create_view(&Default::default());
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Authored Tank Track Texture"),
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
                })
            });
            draws.materials.insert(range.texture_name.clone(), material);
        }
        draws.vertices.clear();
        draws
            .vertices
            .extend(draws.flush.vertices.iter().map(|vertex| {
                let mut gpu = OverlayGpuVertex::from_cpp_xyzduv(
                    vertex.x,
                    vertex.y,
                    vertex.z,
                    vertex.diffuse,
                    vertex.u1,
                    vertex.v1,
                );
                // Generic overlay conversion treats this slot as opaque. Track
                // coverage is the faded alpha packed by W3DTerrainTracks::flush.
                gpu.road_width = (vertex.diffuse >> 24) as f32 / 255.0;
                gpu
            }));
        Self::write_reusable_terrain_buffer(
            device,
            Some(queue),
            &mut draws.vertex_buffer,
            "Tank Track Vertices",
            cast_slice(&draws.vertices),
            wgpu::BufferUsages::VERTEX,
        );
        Self::write_reusable_terrain_buffer(
            device,
            Some(queue),
            &mut draws.index_buffer,
            "Tank Track Indices",
            cast_slice(&draws.flush.indices),
            wgpu::BufferUsages::INDEX,
        );
    }

    fn record_tank_track_draws(&self, pass: &mut RenderPass<'_>) {
        let draws = &self.tank_track_draws;
        let (Some(pipeline), Some(camera), Some(vertices), Some(indices)) = (
            self.tank_track_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            draws.vertex_buffer.as_ref(),
            draws.index_buffer.as_ref(),
        ) else {
            return;
        };
        if draws.flush.ranges.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, camera, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint16);
        for band in &draws.flush.ranges {
            if let Some(Some(material)) = draws.materials.get(&band.texture_name) {
                pass.set_bind_group(1, material, &[]);
                let first = band.first_index as u32;
                pass.draw_indexed(first..first + band.index_count as u32, 0, 0..1);
            }
        }
    }
}
