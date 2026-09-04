//! WGPU pipeline manager bridging WW3D shader/material state to wgpu::RenderPipeline
use crate::rendering::shader_system::shader::{
    AlphaTestType, DepthCompareType, DepthMaskType, DstBlendFuncType, ShaderClass,
    SrcBlendFuncType,
};
use std::collections::HashMap;
use std::sync::Arc;
use ww3d_gpu::device::GpuDevice;

/// Maximum number of texture stages that the fixed-function compatibility shaders support.
///
/// The classic renderer exposed eight texture stages. The WGSL pipelines mirror that layout by
/// packing two stages per bind group (2×2D + 2×cube + samplers).
pub const MAX_TEXTURE_STAGES: usize = 8;
pub const TEXTURES_PER_GROUP: usize = 2;
pub const MAX_TEXTURE_STAGE_GROUPS: usize = MAX_TEXTURE_STAGES.div_ceil(TEXTURES_PER_GROUP);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VertexFormat {
    Basic,   // position, normal, texture coordinates
    Skinned, // position, normal, texture coordinates, bone indices, bone weights
    Line,    // position, color
    /// C++ `W3DShroudMaterialPass` rigid geometry.  This has a dedicated
    /// shader because its texture coordinates are projected from world X/Z,
    /// not read from authored mesh UVs.
    ProjectedShroudBasic,
    /// C++ `W3DShroudMaterialPass` skinned geometry.  The vertex stage keeps
    /// the source bone palette path, then projects the resulting world X/Z.
    ProjectedShroudSkinned,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct PipelineKey {
    pub shader_bits: u32,
    pub skinned: bool,
    pub has_lighting: bool,
    pub has_fog: bool,
    pub primitive_topology: u8, // 0=TriangleList, 1=LineList, 2=TriangleStrip
    pub color_format: wgpu::TextureFormat,
    pub depth_format: Option<wgpu::TextureFormat>,
    pub vertex_format: u8, // 0=basic (pos,normal,uv), 1=skinned, 2=line (pos,color)
    pub depth_bias: i32,
    pub force_two_sided: bool,
}

#[derive(Debug)]
pub struct WgpuPipelineManager {
    gpu_device: Arc<GpuDevice>,
    cache: HashMap<PipelineKey, Arc<wgpu::RenderPipeline>>,
}

impl WgpuPipelineManager {
    pub fn new(gpu_device: Arc<GpuDevice>) -> Self {
        Self {
            gpu_device,
            cache: HashMap::new(),
        }
    }

    pub fn get_or_create(
        &mut self,
        shader: &ShaderClass,
        _texture_stage_mask: u8,
        skinned: bool,
        has_lighting: bool,
        has_fog: bool,
        primitive_topology: wgpu::PrimitiveTopology,
        vertex_format: VertexFormat,
        color_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        depth_bias: i32,
        force_two_sided: bool,
    ) -> Arc<wgpu::RenderPipeline> {
        let topology_id = match primitive_topology {
            wgpu::PrimitiveTopology::TriangleList => 0,
            wgpu::PrimitiveTopology::LineList => 1,
            wgpu::PrimitiveTopology::TriangleStrip => 2,
            _ => 0,
        };

        let vertex_format_id = match vertex_format {
            VertexFormat::Basic => 0,
            VertexFormat::Skinned => 1,
            VertexFormat::Line => 2,
            VertexFormat::ProjectedShroudBasic => 3,
            VertexFormat::ProjectedShroudSkinned => 4,
        };

        let key = PipelineKey {
            shader_bits: shader.get_bits(),
            skinned,
            has_lighting,
            has_fog,
            primitive_topology: topology_id,
            color_format,
            depth_format,
            vertex_format: vertex_format_id,
            depth_bias,
            force_two_sided,
        };
        if let Some(p) = self.cache.get(&key) {
            return p.clone();
        }

        // Load WGSL based on shader flags and vertex format
        let device = self.gpu_device.wgpu_device();

        let shader_source: std::borrow::Cow<'static, str> = match vertex_format {
            VertexFormat::Line => include_str!("../shader_system/line.wgsl").into(),
            VertexFormat::Skinned => include_str!("../shader_system/skinned.wgsl").into(),
            VertexFormat::ProjectedShroudSkinned => {
                include_str!("../shader_system/projected_shroud_skinned.wgsl").into()
            }
            VertexFormat::Basic => {
                if shader.get_src_blend_func() == SrcBlendFuncType::SrcAlpha
                    && shader.get_dst_blend_func() == DstBlendFuncType::InvSrcAlpha
                {
                    include_str!("../shader_system/alpha.wgsl").into()
                } else if shader.get_src_blend_func() == SrcBlendFuncType::One
                    && shader.get_dst_blend_func() == DstBlendFuncType::One
                {
                    include_str!("../shader_system/additive.wgsl").into()
                } else if shader.get_depth_compare() == DepthCompareType::Always
                    && shader.get_depth_mask() == DepthMaskType::Disable
                {
                    include_str!("../shader_system/decal.wgsl").into()
                } else {
                    include_str!("../shader_system/opaque.wgsl").into()
                }
            }
            VertexFormat::ProjectedShroudBasic => {
                include_str!("../shader_system/projected_shroud_basic.wgsl").into()
            }
        };
        // C++ parity: W3D applies the alpha test only when the ShaderClass
        // ALPHATEST bit is authored — ShaderClass::Apply drives
        // D3DRS_ALPHATESTENABLE from BOOL(Get_Alpha_Test()) (shader.cpp:998)
        // with a 0x60 reference (shader.cpp:427), and the default device state
        // is ALPHATESTENABLE=FALSE with ALPHAREF=0 (dx8wrapper.cpp:3682/3688).
        // Gate the alpha.wgsl/decal.wgsl discard on that authored bit:
        // materials without it compile with a 0.0 threshold, which
        // `final_alpha < alpha_threshold` can never fire on (default
        // no-discard); materials with the bit keep the 96/255 reference.
        let shader_source = gate_alpha_test_discard(&shader, shader_source);

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("WW3D Shader Module"),
            source: wgpu::ShaderSource::Wgsl(shader_source.into()),
        });

        // Bind group layouts:
        // - Line shaders: 0 camera, 1 model+lighting
        // - Mesh shaders: 0 camera, 1 model+lighting, 2 (uv or bones+uv), 3..6 textures, 7 colors
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Camera BGL"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let model_bgl = if matches!(vertex_format, VertexFormat::Line) {
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Model BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
            })
        } else {
            // Live forward pass: model + lighting + cascade shadow map (PCF).
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Model+CSM BGL"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2Array,
                            sample_type: wgpu::TextureSampleType::Depth,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 4,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Comparison),
                        count: None,
                    },
                ],
            })
        };
        let group2_bgl = match vertex_format {
            VertexFormat::Line => None,
            VertexFormat::Skinned | VertexFormat::ProjectedShroudSkinned => Some(
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Skinned Group2 BGL"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                }),
            ),
            VertexFormat::Basic | VertexFormat::ProjectedShroudBasic => Some(
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("UV Transform BGL"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    }],
                }),
            ),
        };
        let texture_bgls: Vec<wgpu::BindGroupLayout> =
            if matches!(vertex_format, VertexFormat::Line) {
                Vec::new()
            } else {
                (0..MAX_TEXTURE_STAGE_GROUPS)
                    .map(|group_index| {
                        let mut entries = Vec::with_capacity(TEXTURES_PER_GROUP * 3);
                        for i in 0..TEXTURES_PER_GROUP {
                            let binding_base = (i * 3) as u32;
                            entries.push(wgpu::BindGroupLayoutEntry {
                                binding: binding_base,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::D2,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                },
                                count: None,
                            });
                            entries.push(wgpu::BindGroupLayoutEntry {
                                binding: binding_base + 1,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Texture {
                                    multisampled: false,
                                    view_dimension: wgpu::TextureViewDimension::Cube,
                                    sample_type: wgpu::TextureSampleType::Float {
                                        filterable: true,
                                    },
                                },
                                count: None,
                            });
                            entries.push(wgpu::BindGroupLayoutEntry {
                                binding: binding_base + 2,
                                visibility: wgpu::ShaderStages::FRAGMENT,
                                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                                count: None,
                            });
                        }
                        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                            label: Some(&format!("Texture BGL Group {group_index}")),
                            entries: &entries,
                        })
                    })
                    .collect()
            };

        let color_bgl = if matches!(vertex_format, VertexFormat::Line) {
            None
        } else {
            Some(
                device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("Vertex Color BGL"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Storage { read_only: true },
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                    ],
                }),
            )
        };

        let mut bgls: Vec<&wgpu::BindGroupLayout> = vec![&camera_bgl, &model_bgl];
        if let Some(ref b) = group2_bgl {
            bgls.push(b);
        }
        for texture_layout in &texture_bgls {
            bgls.push(texture_layout);
        }
        if let Some(ref color_layout) = color_bgl {
            bgls.push(color_layout);
        }

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("WW3D Pipeline Layout"),
            bind_group_layouts: &bgls,
            push_constant_ranges: &[],
        });

        // Vertex formats based on vertex format type
        let vertex_layout = match vertex_format {
            VertexFormat::Line => {
                wgpu::VertexBufferLayout {
                    array_stride: (std::mem::size_of::<[f32; 3]>()
                        + std::mem::size_of::<[f32; 4]>()) as u64, // pos + color
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }
            }
            VertexFormat::Skinned | VertexFormat::ProjectedShroudSkinned => {
                // MeshRenderManager packs four UV sets, followed by the four
                // bone indices and four weights.  Texture *stages* may be
                // eight, but the vertex ABI is four UV attributes (the same
                // ABI used by shader.rs and the WGSL inputs).
                let stride_bytes = (3 + 3 + (4 * 2) + 4 + 4) * std::mem::size_of::<f32>();
                wgpu::VertexBufferLayout {
                    array_stride: stride_bytes as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 56,
                            shader_location: 6,
                            format: wgpu::VertexFormat::Uint32x4,
                        },
                        wgpu::VertexAttribute {
                            offset: 72,
                            shader_location: 7,
                            format: wgpu::VertexFormat::Float32x4,
                        },
                    ],
                }
            }
            VertexFormat::Basic | VertexFormat::ProjectedShroudBasic => {
                let stride_bytes = (3 + 3 + (4 * 2)) * std::mem::size_of::<f32>();
                wgpu::VertexBufferLayout {
                    array_stride: stride_bytes as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            offset: 0,
                            shader_location: 0,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 12,
                            shader_location: 1,
                            format: wgpu::VertexFormat::Float32x3,
                        },
                        wgpu::VertexAttribute {
                            offset: 24,
                            shader_location: 2,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 32,
                            shader_location: 3,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 40,
                            shader_location: 4,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                        wgpu::VertexAttribute {
                            offset: 48,
                            shader_location: 5,
                            format: wgpu::VertexFormat::Float32x2,
                        },
                    ],
                }
            }
        };

        let blend = create_blend_state_from_shader(shader);
        let bias_state = if depth_bias != 0 {
            wgpu::DepthBiasState {
                constant: depth_bias,
                slope_scale: 0.0,
                clamp: 0.0,
            }
        } else {
            wgpu::DepthBiasState::default()
        };

        let depth_format = depth_format.unwrap_or(wgpu::TextureFormat::Depth32Float);
        let mut depth_stencil = Some(create_depth_stencil_state_from_shader(
            shader,
            depth_format,
            bias_state,
        ));

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("WW3D Render Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_main"),
                buffers: &[vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(blend),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: primitive_topology,
                strip_index_format: if matches!(
                    primitive_topology,
                    wgpu::PrimitiveTopology::TriangleStrip
                ) {
                    Some(wgpu::IndexFormat::Uint16)
                } else {
                    None
                },
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: if force_two_sided
                    || shader.get_cull_mode()
                        != crate::rendering::shader_system::shader::CullModeType::Enable
                {
                    None
                } else {
                    Some(wgpu::Face::Back)
                },
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        let arc = Arc::new(pipeline);
        self.cache.insert(key, arc.clone());
        arc
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rendering::projected_shroud::ProjectedShroudMaterialPassContract;
    use std::sync::Arc;
    use std::sync::mpsc;
    use wgpu::util::DeviceExt;

    /// Exercise the real WGPU shader/pipeline validator when a software or
    /// hardware adapter is available.  Headless CI is allowed to skip this
    /// test, but environments that can render must validate both projected
    /// shader modules and their bind-group/vertex ABI rather than only
    /// compiling the Rust source that includes them.
    #[test]
    fn projected_shroud_pipelines_validate_on_available_adapter() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
            .ok()
        else {
            return;
        };
        // STANDALONE DEVICE: #[cfg(test)] pipeline ABI test, not on the game path.
        let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: Some("projected-shroud-pipeline-test"),
                ..Default::default()
            }))
        else {
            return;
        };
        let gpu = Arc::new(GpuDevice::from_shared(Arc::new(device), Arc::new(queue)));
        let mut manager = WgpuPipelineManager::new(gpu);
        let shader = ProjectedShroudMaterialPassContract::CXX.shader();
        let _rigid = manager.get_or_create(
            &shader,
            0,
            false,
            false,
            false,
            wgpu::PrimitiveTopology::TriangleList,
            VertexFormat::ProjectedShroudBasic,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            Some(wgpu::TextureFormat::Depth32Float),
            0,
            false,
        );
        let _skinned = manager.get_or_create(
            &shader,
            0,
            true,
            false,
            false,
            wgpu::PrimitiveTopology::TriangleList,
            VertexFormat::ProjectedShroudSkinned,
            wgpu::TextureFormat::Bgra8UnormSrgb,
            Some(wgpu::TextureFormat::Depth32Float),
            0,
            false,
        );
    }

    /// Render one projected pass into a one-pixel target and read it back.
    ///
    /// This is intentionally a tiny, renderer-owned target rather than a
    /// Main integration test: it proves the actual shader/bind layout and
    /// `Zero / SrcColor` blend operation multiply an existing destination by
    /// the frozen R8 shroud level.  Environments without a software adapter
    /// may skip the test, just like the pipeline ABI test above.
    #[test]
    fn projected_shroud_headless_target_is_multiplied_by_frozen_level() {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });
        let Some(adapter) =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::LowPower,
                compatible_surface: None,
                force_fallback_adapter: true,
            }))
            .ok()
        else {
            return;
        };
        // STANDALONE DEVICE: #[cfg(test)] headless shroud test, not on the game path.
        let Ok((device, queue)) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: Some("projected-shroud-headless-test"),
                ..Default::default()
            }))
        else {
            return;
        };

        let device = Arc::new(device);
        let queue = Arc::new(queue);
        let gpu = Arc::new(GpuDevice::from_shared(device.clone(), queue.clone()));
        let mut manager = WgpuPipelineManager::new(gpu);
        let shader = ProjectedShroudMaterialPassContract::CXX.shader();
        let pipeline = manager.get_or_create(
            &shader,
            0,
            false,
            false,
            false,
            wgpu::PrimitiveTopology::TriangleList,
            VertexFormat::ProjectedShroudBasic,
            wgpu::TextureFormat::Rgba8Unorm,
            Some(wgpu::TextureFormat::Depth32Float),
            0,
            true,
        );

        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("projected-shroud-color"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("projected-shroud-depth"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());

        let shroud = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("projected-shroud-r8"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &shroud,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[128],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(1),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        let shroud_view = shroud.create_view(&wgpu::TextureViewDescriptor::default());
        let cube = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("projected-shroud-cube-fallback"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
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
                texture: &cube,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255; 24],
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 6,
            },
        );
        let cube_view = cube.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::Cube),
            ..Default::default()
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let mut camera = [0u32; 52];
        for base in [0usize, 16, 32] {
            camera[base] = 1.0f32.to_bits();
            camera[base + 5] = 1.0f32.to_bits();
            camera[base + 10] = 1.0f32.to_bits();
            camera[base + 15] = 1.0f32.to_bits();
        }
        camera[48..52].copy_from_slice(&[0.0f32, 0.0, 0.0, 1.0].map(f32::to_bits));
        let mut model = [0u32; 60];
        for base in [0usize, 16] {
            model[base] = 1.0f32.to_bits();
            model[base + 5] = 1.0f32.to_bits();
            model[base + 10] = 1.0f32.to_bits();
            model[base + 15] = 1.0f32.to_bits();
        }
        model[40..44].copy_from_slice(&[1.0f32, 1.0, 0.0, 0.0].map(f32::to_bits));
        model[52..56].copy_from_slice(&[1.0f32, 1.0, 1.0, 1.0].map(f32::to_bits));
        model[56..60].copy_from_slice(&[1.0f32, 1.0, 1.0, 0.0].map(f32::to_bits));
        let uv = [0u32; 16];

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("projected-shroud-camera"),
            contents: bytemuck::cast_slice(&camera),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let model_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("projected-shroud-model"),
            contents: bytemuck::cast_slice(&model),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let lighting_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("projected-shroud-lighting"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let csm_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("projected-shroud-csm"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        let uv_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("projected-shroud-uv"),
            contents: bytemuck::cast_slice(&uv),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let csm_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("projected-shroud-csm-depth"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let csm_view = csm_texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });
        let csm_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            compare: Some(wgpu::CompareFunction::LessEqual),
            ..Default::default()
        });

        let camera_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("projected-shroud-camera-group"),
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });
        let model_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("projected-shroud-model-group"),
            layout: &pipeline.get_bind_group_layout(1),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: model_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: lighting_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: csm_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&csm_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::Sampler(&csm_sampler),
                },
            ],
        });
        let uv_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("projected-shroud-uv-group"),
            layout: &pipeline.get_bind_group_layout(2),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uv_buffer.as_entire_binding(),
            }],
        });
        let texture_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("projected-shroud-texture-group"),
            layout: &pipeline.get_bind_group_layout(3),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&shroud_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(&shroud_view),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: wgpu::BindingResource::TextureView(&cube_view),
                },
                wgpu::BindGroupEntry {
                    binding: 5,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let mut vertices = Vec::with_capacity(3 * 14);
        for position in [[-1.0f32, -1.0, 0.5], [3.0, -1.0, 0.5], [-1.0, 3.0, 0.5]] {
            vertices.extend_from_slice(&position);
            vertices.extend_from_slice(&[0.0, 0.0, 1.0]);
            vertices.extend_from_slice(&[0.0; 8]);
        }
        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("projected-shroud-triangle"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("projected-shroud-headless-encoder"),
        });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("projected-shroud-headless-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.8,
                            g: 0.4,
                            b: 0.2,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.5),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &camera_group, &[]);
            pass.set_bind_group(1, &model_group, &[]);
            pass.set_bind_group(2, &uv_group, &[]);
            pass.set_bind_group(3, &texture_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.draw(0..3, 0..1);
        }

        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("projected-shroud-readback"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &color,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));
        let slice = readback.slice(..);
        let (sender, receiver) = mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        receiver
            .recv()
            .expect("readback callback")
            .expect("readback map");
        let bytes = slice.get_mapped_range();
        let expected = [102u8, 51u8, 26u8, 255u8];
        assert!(
            bytes[..4]
                .iter()
                .zip(expected)
                .all(|(actual, expected)| (*actual as i16 - expected as i16).abs() <= 2),
            "projected pass output {:?} did not multiply destination {:?}",
            &bytes[..4],
            expected
        );
        drop(bytes);
        readback.unmap();
    }
}

/// Create WGPU blend state from shader configuration with validation and fallback logic
///
/// Reference: GeneralsMD/Code/Libraries/Source/WWVegas/WW3D2/shader.cpp lines 438-463
///
/// This function mirrors the C++ Apply() method's blend state logic:
/// 1. Extract source and destination blend functions from shader bits
/// 2. Check if color mask is disabled (forces ZERO, ONE for safety)
/// 3. Apply blend enable logic: blending is enabled if (src != ONE || dst != ZERO)
/// 4. Handle alpha channel separately from color channels
///
/// # Validation Rules (from C++ lines 455-463)
/// - If color mask is disabled: force src=ZERO, dst=ONE to prevent writes
/// - If src==ONE and dst==ZERO: blending is considered disabled (opaque)
/// - Otherwise: blending is enabled
fn create_blend_state_from_shader(shader: &ShaderClass) -> wgpu::BlendState {
    let mut src_func = shader.get_src_blend_func();
    let mut dst_func = shader.get_dst_blend_func();

    // Validation: If color mask is disabled, force safe blend mode
    // Reference: shader.cpp lines 442-446
    if shader.get_color_mask() == crate::rendering::shader_system::shader::ColorMaskType::Disable {
        src_func = SrcBlendFuncType::Zero;
        dst_func = DstBlendFuncType::One;
    }

    // Convert source blend function to WGPU
    let src = match src_func {
        SrcBlendFuncType::Zero => wgpu::BlendFactor::Zero,
        SrcBlendFuncType::One => wgpu::BlendFactor::One,
        SrcBlendFuncType::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
        SrcBlendFuncType::InvSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        _ => {
            // Fallback for invalid values
            log::warn!("Invalid source blend function, defaulting to One");
            wgpu::BlendFactor::One
        }
    };

    // Convert destination blend function to WGPU
    let dst = match dst_func {
        DstBlendFuncType::Zero => wgpu::BlendFactor::Zero,
        DstBlendFuncType::One => wgpu::BlendFactor::One,
        DstBlendFuncType::SrcColor => wgpu::BlendFactor::Src,
        DstBlendFuncType::InvSrcColor => wgpu::BlendFactor::OneMinusSrc,
        DstBlendFuncType::SrcAlpha => wgpu::BlendFactor::SrcAlpha,
        DstBlendFuncType::InvSrcAlpha => wgpu::BlendFactor::OneMinusSrcAlpha,
        _ => {
            // Fallback for invalid values
            log::warn!("Invalid destination blend function, defaulting to Zero");
            wgpu::BlendFactor::Zero
        }
    };

    // Check if blending should be enabled
    // Reference: shader.cpp lines 455-463
    // "if(sf != D3DBLEND_ONE || df != D3DBLEND_ZERO) { blendOn = TRUE; }"
    let blend_enabled = src != wgpu::BlendFactor::One || dst != wgpu::BlendFactor::Zero;

    if !blend_enabled {
        // Opaque rendering: use REPLACE blend mode
        wgpu::BlendState::REPLACE
    } else {
        // Transparent rendering: configure blend state
        wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: src,
                dst_factor: dst,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                // Alpha channel typically uses same factors as color
                // but could be configured differently for special effects
                src_factor: src,
                dst_factor: dst,
                operation: wgpu::BlendOperation::Add,
            },
        }
    }
}

/// Gates the WGSL alpha-test discard on the authored ShaderClass ALPHATEST bit.
///
/// C++ parity: `ShaderClass::Apply` enables `D3DRS_ALPHATESTENABLE` only when
/// `Get_Alpha_Test() != ALPHATEST_DISABLE` (GeneralsMD shader.cpp:998) with a
/// 0x60 reference (shader.cpp:427); the default device state is
/// ALPHATESTENABLE=FALSE / ALPHAREF=0 (dx8wrapper.cpp:3682/3688). Materials
/// without the bit compile with a 0.0 threshold so the
/// `final_alpha < alpha_threshold` discard in alpha.wgsl/decal.wgsl can never
/// fire (default no-discard); materials with the bit keep the 96/255 reference.
pub fn gate_alpha_test_discard(
    shader: &ShaderClass,
    source: std::borrow::Cow<'static, str>,
) -> std::borrow::Cow<'static, str> {
    if shader.get_alpha_test() == AlphaTestType::Enable {
        return source;
    }
    if source.contains("96.0 / 255.0") {
        std::borrow::Cow::Owned(source.replacen("96.0 / 255.0", "0.0", 1))
    } else {
        source
    }
}

fn to_compare_func(cmp: DepthCompareType) -> wgpu::CompareFunction {
    match cmp {
        DepthCompareType::Never => wgpu::CompareFunction::Never,
        DepthCompareType::Less => wgpu::CompareFunction::Less,
        DepthCompareType::Equal => wgpu::CompareFunction::Equal,
        DepthCompareType::Lequal => wgpu::CompareFunction::LessEqual,
        DepthCompareType::Greater => wgpu::CompareFunction::Greater,
        DepthCompareType::Notequal => wgpu::CompareFunction::NotEqual,
        DepthCompareType::Gequal => wgpu::CompareFunction::GreaterEqual,
        DepthCompareType::Always => wgpu::CompareFunction::Always,
        _ => wgpu::CompareFunction::LessEqual,
    }
}


fn create_depth_stencil_state_from_shader(
    shader: &ShaderClass,
    format: wgpu::TextureFormat,
    bias: wgpu::DepthBiasState,
) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format,
        depth_write_enabled: shader.get_depth_mask() == DepthMaskType::Enable,
        depth_compare: to_compare_func(shader.get_depth_compare()),
        stencil: wgpu::StencilState::default(),
        bias,
    }
}
