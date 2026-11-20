// C++ setupJbaWaterShader / m_riverWaterPixelShader / m_trapezoidWaterPixelShader.

const RIVER_NOISE_REPEAT: f32 = 1.0 / 16.0;
const RIVER_REFLECTION: f32 = 0.1;

fn river_params(phase: f32, is_trapezoid: f32) -> [f32; 4] {
    [phase, RIVER_NOISE_REPEAT, RIVER_REFLECTION, is_trapezoid]
}

impl TerrainVisualImpl {
    fn create_river_pipeline(&mut self, device: &wgpu::Device) -> TerrainResult<()> {
        let Some(camera_layout) = self.terrain_camera_bind_group_layout.as_ref() else {
            return Err(TerrainError::GPUError(
                "camera layout required for river pipeline".into(),
            ));
        };
        let Some(shroud_layout) = self.shroud_gpu.bind_layout.as_ref() else {
            return Err(TerrainError::GPUError(
                "shroud layout required for trapezoid water".into(),
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
        let river_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("JBA river pipeline layout"),
            bind_group_layouts: &[Some(camera_layout.as_ref()), Some(bind_layout.as_ref())],
            immediate_size: 0,
        });
        let trapezoid_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("JBA trapezoid pipeline layout"),
            bind_group_layouts: &[
                Some(camera_layout.as_ref()),
                Some(bind_layout.as_ref()),
                Some(shroud_layout.as_ref()),
            ],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("JBA river shader"),
            source: wgpu::ShaderSource::Wgsl(
                concat!(
                    include_str!("../shaders/projected_shroud_sample.wgsl"),
                    include_str!("../shaders/water_river.wgsl"),
                )
                .into(),
            ),
        });
        let make_pipeline = |label: &str,
                             layout: &wgpu::PipelineLayout,
                             fragment: &str,
                             blend: wgpu::BlendState| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(WaterGpuVertex::desc())],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(fragment),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: wgpu::TextureFormat::Bgra8Unorm,
                        blend: Some(blend),
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
                    depth_write_enabled: Some(false),
                    depth_compare: Some(wgpu::CompareFunction::LessEqual),
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        self.river_gpu.pipeline = Some(make_pipeline(
            "JBA river pipeline",
            &river_layout,
            "fs_river",
            wgpu::BlendState::ALPHA_BLENDING,
        ));
        self.river_gpu.trapezoid_pipeline = Some(make_pipeline(
            "JBA trapezoid pipeline",
            &trapezoid_layout,
            "fs_trapezoid",
            wgpu::BlendState::ALPHA_BLENDING,
        ));
        // C++ drawRiverWater additive override (W3DWater.cpp:2882-2883):
        // SRCBLEND=SRCALPHA on top of PresetAdditiveShader => SRCALPHA, ONE.
        self.river_gpu.river_additive_pipeline = Some(make_pipeline(
            "JBA river additive pipeline",
            &river_layout,
            "fs_river",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        ));
        // C++ setupFlatWaterShader additive (PresetAdditiveShader): ONE, ONE.
        self.river_gpu.trapezoid_additive_pipeline = Some(make_pipeline(
            "JBA trapezoid additive pipeline",
            &trapezoid_layout,
            "fs_trapezoid",
            wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        ));
        self.river_gpu.bind_layout = Some(bind_layout);
        let phase = self.overlay.river_v_origin;
        let make_params = |label: &str, is_trapezoid| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::bytes_of(&river_params(phase, is_trapezoid)),
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            })
        };
        self.river_gpu.params = Some(make_params("JBA river params", 0.0));
        self.river_gpu.trapezoid_params = Some(make_params("JBA trapezoid params", 1.0));
        self.river_gpu.uploaded_phase = Some(phase.to_bits());
        // Both groups retain the buffers they were made with.
        self.river_gpu.bind_group = None;
        self.river_gpu.trapezoid_bind_group = None;
        Ok(())
    }

    // C++ WaterRenderObjClass::updateMapOverrides changes m_riverTexture to
    // the owner's StandingWaterTexture (W3DWater.cpp:1110–1117).
    fn ensure_water_materials(&mut self, device: &wgpu::Device) {
        self.ensure_water_texture_bind_group(device);
        let (Some(surface), Some(sampler), Some(layout), Some(params), Some(trapezoid_params)) = (
            self.water_texture.as_ref(),
            self.water_sampler.as_ref(),
            self.river_gpu.bind_layout.as_ref(),
            self.river_gpu.params.as_ref(),
            self.river_gpu.trapezoid_params.as_ref(),
        ) else {
            return;
        };
        if self.river_gpu.bind_group.is_some()
            && self.river_gpu.trapezoid_bind_group.is_some()
            && self.river_gpu.river_tex.as_ref() == Some(surface)
        {
            return;
        }
        if self.river_gpu.detail_views.is_none() {
            let Some(queue) = self.queue.as_ref() else {
                return;
            };
            let load_view = |name, rgba, fallback_label| {
                self.load_first_available_named_texture(device, &[name], 1)
                    .unwrap_or_else(|| {
                        Self::create_solid_rgba_texture(device, queue, rgba, fallback_label)
                    })
                    .create_view(&wgpu::TextureViewDescriptor::default())
            };
            self.river_gpu.detail_views = Some(RiverDetailViews {
                sparkle: load_view(
                    "WaterSurfaceBubbles.tga",
                    [220, 230, 255, 80],
                    "sparkle fallback",
                ),
                noise: load_view("Noise0000.tga", [90, 90, 90, 255], "noise fallback"),
                edge: load_view("TWAlphaEdge.tga", [255, 255, 255, 255], "alpha-edge fallback"),
            });
        }
        let Some(details) = self.river_gpu.detail_views.as_ref() else {
            return;
        };
        let river_view = surface.create_view(&wgpu::TextureViewDescriptor::default());
        let make_bind = |label: &str, params: &wgpu::Buffer| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
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
                        resource: wgpu::BindingResource::TextureView(&details.sparkle),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::TextureView(&details.noise),
                    },
                    wgpu::BindGroupEntry {
                        binding: 4,
                        resource: wgpu::BindingResource::TextureView(&details.edge),
                    },
                    wgpu::BindGroupEntry {
                        binding: 5,
                        resource: params.as_entire_binding(),
                    },
                ],
            })
        };
        let bind = make_bind("JBA river bind", params);
        let trapezoid_bind = make_bind("JBA trapezoid bind", trapezoid_params);
        self.river_gpu.river_tex = Some(surface.clone());
        self.river_gpu.bind_group = Some(bind);
        self.river_gpu.trapezoid_bind_group = Some(trapezoid_bind);
    }

    /// Upload the new phase to both materials. Returns whether writes were queued.
    fn sync_river_params(&mut self) -> bool {
        let phase = self.overlay.river_v_origin;
        if self.river_gpu.uploaded_phase == Some(phase.to_bits()) {
            return false;
        }
        let (Some(queue), Some(river), Some(trapezoid)) = (
            self.queue.as_ref(),
            self.river_gpu.params.as_ref(),
            self.river_gpu.trapezoid_params.as_ref(),
        ) else {
            return false;
        };
        for (buffer, is_trapezoid) in [(river, 0.0), (trapezoid, 1.0)] {
            queue.write_buffer(buffer, 0, bytemuck::bytes_of(&river_params(phase, is_trapezoid)));
        }
        self.river_gpu.uploaded_phase = Some(phase.to_bits());
        true
    }

    fn load_first_available_named_texture(
        &self,
        device: &wgpu::Device,
        names: &[&str],
        mip_levels: u32,
    ) -> Option<wgpu::Texture> {
        for name in names {
            for path in Self::water_texture_path_candidates(name) {
                if let Ok(texture) = self.load_texture_from_path_with_mips(device, &path, mip_levels) {
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

    /// Resolve the mounted stage-zero wake materials before the receiving
    /// map's first water draw. The W3D texture loader prefers a DDS sibling
    /// and WaterTracksObj::init requests all levels retained by DDSFileClass.
    pub fn hydrate_mounted_water_track_textures(
        &mut self,
        mut read_texture: impl FnMut(&str) -> Option<Vec<u8>>,
    ) {
        let (Some(device), Some(queue), Some(layout)) = (
            self.device.as_ref(),
            self.queue.as_ref(),
            self.water_texture_bind_group_layout.as_ref(),
        ) else {
            return;
        };
        let mut seen = HashSet::new();
        let mut textures = Vec::new();
        for name in self.water_tracks.active_texture_names() {
            if !seen.insert(name) || self.water_named_bind_groups.contains_key(name) {
                continue;
            }
            let path = name.replace('\\', "/");
            let texture = ["dds", "tga"].into_iter().find_map(|extension| {
                let candidate = Path::new(&path).with_extension(extension);
                let bytes = read_texture(candidate.to_str()?)?;
                Self::upload_runtime_texture(device, queue, &candidate, &bytes, u32::MAX).ok()
            });
            if let Some(texture) = texture {
                textures.push((name.to_owned(), texture));
            }
        }
        if !textures.is_empty() {
            let device = Arc::clone(device);
            let layout = Arc::clone(layout);
            for (name, texture) in textures {
                self.bind_water_track_texture(&device, &layout, &name, texture);
            }
        }
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
            .load_first_available_named_texture(device, &[name], u32::MAX)
            .unwrap_or_else(|| {
                Self::create_solid_rgba_texture(
                    device,
                    queue.as_ref(),
                    [210, 230, 245, 200],
                    "wave256 fallback",
                )
            });
        self.bind_water_track_texture(device, &layout, name, texture);
    }

    fn bind_water_track_texture(
        &mut self,
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        name: &str,
        texture: wgpu::Texture,
    ) {
        let sampler = self.water_sampler.get_or_insert_with(|| {
            device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("Water Track Sampler"),
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            })
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Water track named bind"),
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
