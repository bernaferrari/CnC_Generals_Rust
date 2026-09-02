// C++ ST_SHROUD_TEXTURE / setShroudTex projected dest texture.
// Extra multiplicative pass after water / wakes / trees / bridges.



impl TerrainVisualImpl {
    fn create_shroud_overlay_pipelines(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(camera_layout) = self.terrain_camera_bind_group_layout.as_ref() else {
            return Err(TerrainError::GPUError(
                "camera layout required for shroud overlay".into(),
            ));
        };
        let bind_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("ST_SHROUD dest bind layout"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            },
        ));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ST_SHROUD pipeline layout"),
            bind_group_layouts: &[camera_layout.as_ref(), bind_layout.as_ref()],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("ST_SHROUD overlay"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/shroud_overlay.wgsl").into()),
        });
        let multiply = Some(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::Dst,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::Zero,
                operation: wgpu::BlendOperation::Add,
            },
        });
        let make = |label: &str, stride: u64, z_compare: wgpu::CompareFunction| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: stride,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        }],
                    }],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: multiply,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: TERRAIN_PIPELINES_DEPTH_FORMAT,
                    depth_write_enabled: false,
                    depth_compare: z_compare,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        // Water does not write Z — C++ forces LESSEQUAL on the extra shroud pass.
        self.shroud_gpu.water_pipeline = Some(make(
            "ST_SHROUD water/wakes",
            std::mem::size_of::<WaterGpuVertex>() as u64,
            wgpu::CompareFunction::LessEqual,
        ));
        self.shroud_gpu.road_pipeline = Some(make(
            "ST_SHROUD bridges",
            std::mem::size_of::<OverlayGpuVertex>() as u64,
            wgpu::CompareFunction::LessEqual,
        ));
        self.shroud_gpu.tree_pipeline = Some(make(
            "ST_SHROUD trees",
            std::mem::size_of::<TreeGpuVertex>() as u64,
            wgpu::CompareFunction::LessEqual,
        ));
        self.shroud_gpu.bind_layout = Some(bind_layout);
        self.shroud_gpu.params = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ST_SHROUD params"),
            size: 32,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        self.shroud_gpu.sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("ST_SHROUD dest sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        }));
        Ok(())
    }

    fn sync_shroud_dest_texture(&mut self) {
        let Some(device) = self.device.as_ref() else {
            return;
        };
        let Some(queue) = self.queue.as_ref() else {
            return;
        };
        let width = self.overlay.shroud_width.max(0) as u32;
        let height = self.overlay.shroud_height.max(0) as u32;
        let cells = &self.overlay.shroud_cells;
        let (w, h, pixels) = if width == 0 || height == 0 || cells.is_empty() {
            (1u32, 1u32, vec![255u8, 255, 255, 255])
        } else {
            let mut rgba = vec![255u8; (width * height * 4) as usize];
            let n = cells.len().min((width * height) as usize);
            for i in 0..n {
                let a = cells[i];
                let o = i * 4;
                rgba[o] = a;
                rgba[o + 1] = a;
                rgba[o + 2] = a;
                rgba[o + 3] = 255;
            }
            (width, height, rgba)
        };
        let need_new = self
            .shroud_gpu
            .dest_texture
            .as_ref()
            .map(|t| t.width() != w || t.height() != h)
            .unwrap_or(true);
        if need_new {
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("W3DShroud dest"),
                size: wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            self.shroud_gpu.dest_view =
                Some(tex.create_view(&wgpu::TextureViewDescriptor::default()));
            self.shroud_gpu.dest_texture = Some(tex);
            self.shroud_gpu.bind_group = None;
        }
        if let Some(tex) = self.shroud_gpu.dest_texture.as_ref() {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: tex,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &pixels,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * 4),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: 1,
                },
            );
        }
        let enabled = if width > 0 && height > 0 && !cells.is_empty() {
            1.0f32
        } else {
            0.0
        };
        let params = [
            self.overlay.shroud_origin[0],
            self.overlay.shroud_origin[1],
            self.overlay.shroud_cell_size.max(1.0),
            enabled,
            w as f32,
            h as f32,
            0.0,
            0.0,
        ];
        if let Some(buf) = self.shroud_gpu.params.as_ref() {
            queue.write_buffer(buf, 0, bytemuck::cast_slice(&params));
        }
        if self.shroud_gpu.bind_group.is_none() {
            let (Some(layout), Some(view), Some(sampler), Some(buf)) = (
                self.shroud_gpu.bind_layout.as_ref(),
                self.shroud_gpu.dest_view.as_ref(),
                self.shroud_gpu.sampler.as_ref(),
                self.shroud_gpu.params.as_ref(),
            ) else {
                return;
            };
            self.shroud_gpu.bind_group = Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("ST_SHROUD dest bind"),
                layout: layout.as_ref(),
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: buf.as_entire_binding(),
                    },
                ],
            }));
        }
        self.shroud_gpu.uploaded_len = cells.len();
    }

    fn record_shroud_water_pass<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(pipeline), Some(camera), Some(shroud)) = (
            self.shroud_gpu.water_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.shroud_gpu.bind_group.as_ref(),
        ) else {
            return;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, camera, &[]);
        pass.set_bind_group(1, shroud, &[]);
        let meshes = self
            .water_plane
            .iter()
            .chain(self.water_track_meshes.iter())
            .chain(self.shoreline_meshes.iter())
            .chain(self.polygon_water_meshes.iter())
            .chain(self.water_grid_mesh.iter());
        for mesh in meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn record_shroud_tree_pass<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(pipeline), Some(camera), Some(shroud)) = (
            self.shroud_gpu.tree_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.shroud_gpu.bind_group.as_ref(),
        ) else {
            return;
        };
        if self.tree_meshes.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, camera, &[]);
        pass.set_bind_group(1, shroud, &[]);
        for mesh in &self.tree_meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }

    fn record_shroud_bridge_pass<'pass>(&'pass self, pass: &mut RenderPass<'pass>) {
        let (Some(pipeline), Some(camera), Some(shroud)) = (
            self.shroud_gpu.road_pipeline.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
            self.shroud_gpu.bind_group.as_ref(),
        ) else {
            return;
        };
        if self.bridge_meshes.is_empty() {
            return;
        }
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, camera, &[]);
        pass.set_bind_group(1, shroud, &[]);
        for mesh in &self.bridge_meshes {
            pass.set_vertex_buffer(0, mesh.vertex_buffer.slice(..));
            pass.set_index_buffer(mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
            pass.draw_indexed(0..mesh.index_count, 0, 0..1);
        }
    }
}
