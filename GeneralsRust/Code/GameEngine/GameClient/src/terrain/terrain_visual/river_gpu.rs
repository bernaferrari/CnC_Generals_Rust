// C++ setupJbaWaterShader / m_riverWaterPixelShader / m_trapezoidWaterPixelShader.

const RIVER_NOISE_REPEAT: f32 = 1.0 / 16.0;
const RIVER_REFLECTION: f32 = 0.1;



impl TerrainVisualImpl {
    fn create_river_pipeline(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(camera_layout) = self.terrain_camera_bind_group_layout.as_ref() else {
            return Err(TerrainError::GPUError(
                "camera layout required for river pipeline".into(),
            ));
        };
        let bind_layout = Arc::new(device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                label: Some("JBA river bind layout"),
                entries: &[
                    tex_entry(0),
                    sampler_entry(1),
                    tex_entry(2),
                    tex_entry(3),
                    tex_entry(4),
                    wgpu::BindGroupLayoutEntry {
                        binding: 5,
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
            label: Some("JBA river pipeline layout"),
            bind_group_layouts: &[camera_layout.as_ref(), bind_layout.as_ref()],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("JBA river shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../shaders/water_river.wgsl").into()),
        });
        self.river_gpu.pipeline = Some(device.create_render_pipeline(
            &wgpu::RenderPipelineDescriptor {
                label: Some("JBA river/trapezoid pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[WaterGpuVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8UnormSrgb,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            },
        ));
        self.river_gpu.bind_layout = Some(bind_layout);
        self.river_gpu.params = Some(device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("JBA river params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));
        Ok(())
    }

    fn ensure_river_bind_group(&mut self, device: &wgpu::Device) {
        if self.river_gpu.bind_group.is_some() {
            return;
        }
        let Some(layout) = self.river_gpu.bind_layout.clone() else {
            return;
        };
        let Some(queue) = self.queue.clone() else {
            return;
        };
        let Some(params) = self.river_gpu.params.as_ref() else {
            return;
        };
        // C++ m_riverTexture is standing water; extras are named TGA assets.
        let river_tex = self
            .load_first_available_named_texture(device, &["TWWater01.tga", "TWWater01.dds"])
            .unwrap_or_else(|| Self::create_fallback_water_texture(device, queue.as_ref()));
        let sparkle = self
            .load_first_available_named_texture(device, &["WaterSurfaceBubbles.tga"])
            .unwrap_or_else(|| {
                Self::create_solid_rgba_texture(
                    device,
                    queue.as_ref(),
                    [220, 230, 255, 80],
                    "sparkle fallback",
                )
            });
        let noise = self
            .load_first_available_named_texture(device, &["Noise0000.tga"])
            .unwrap_or_else(|| {
                Self::create_solid_rgba_texture(
                    device,
                    queue.as_ref(),
                    [90, 90, 90, 255],
                    "noise fallback",
                )
            });
        let edge = self
            .load_first_available_named_texture(device, &["TWAlphaEdge.tga"])
            .unwrap_or_else(|| {
                Self::create_solid_rgba_texture(
                    device,
                    queue.as_ref(),
                    [255, 255, 255, 255],
                    "alpha-edge fallback",
                )
            });
        if self.water_sampler.is_none() {
            self.water_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("River Texture Sampler"),
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
        let river_view = river_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sparkle_view = sparkle.create_view(&wgpu::TextureViewDescriptor::default());
        let noise_view = noise.create_view(&wgpu::TextureViewDescriptor::default());
        let edge_view = edge.create_view(&wgpu::TextureViewDescriptor::default());
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("JBA river bind"),
            layout: layout.as_ref(),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&river_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(&sparkle_view),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&noise_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&edge_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: params.as_entire_binding(),
                },
            ],
        });
        self.river_gpu.river_tex = Some(river_tex);
        self.river_gpu.sparkle_tex = Some(sparkle);
        self.river_gpu.noise_tex = Some(noise);
        self.river_gpu.edge_tex = Some(edge);
        self.river_gpu.bind_group = Some(bind);
    }

    fn sync_river_params(&self) {
        let Some(queue) = self.queue.as_ref() else {
            return;
        };
        let Some(buf) = self.river_gpu.params.as_ref() else {
            return;
        };
        let params = [
            self.overlay.river_v_origin,
            RIVER_NOISE_REPEAT,
            RIVER_REFLECTION,
            0.0f32,
        ];
        queue.write_buffer(buf, 0, bytemuck::cast_slice(&params));
    }

    fn load_first_available_named_texture(
        &self,
        device: &wgpu::Device,
        names: &[&str],
    ) -> Option<wgpu::Texture> {
        for name in names {
            for path in Self::water_texture_path_candidates(name) {
                if let Ok(texture) = self.load_texture_from_path(device, &path) {
                    return Some(texture);
                }
            }
        }
        None
    }

    fn create_solid_rgba_texture(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        rgba: [u8; 4],
        label: &str,
    ) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
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
            &rgba,
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

    fn ensure_named_water_bind_group(&mut self, device: &wgpu::Device, name: &str) {
        if name.is_empty() || self.water_named_bind_groups.contains_key(name) {
            return;
        }
        let Some(layout) = self.water_texture_bind_group_layout.clone() else {
            return;
        };
        let Some(queue) = self.queue.clone() else {
            return;
        };
        let texture = self
            .load_first_available_named_texture(device, &[name])
            .unwrap_or_else(|| {
                Self::create_solid_rgba_texture(
                    device,
                    queue.as_ref(),
                    [210, 230, 245, 200],
                    "wave256 fallback",
                )
            });
        if self.water_sampler.is_none() {
            self.water_sampler = Some(device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Water Track Sampler"),
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
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water track named bind"),
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
        self.water_named_bind_groups.insert(
            name.to_string(),
            NamedWaterBind {
                _texture: texture,
                bind_group,
            },
        );
    }
}

fn tex_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}
