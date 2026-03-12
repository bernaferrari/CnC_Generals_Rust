use std::collections::HashMap;
use std::sync::Arc;

use glam::Vec2;
use log::warn;

use crate::material_system::{TextureMipStrategy, TextureStageSettings};
use crate::rendering::render2d::Vertex2D;
use crate::rendering::shader_system::{DstBlendFuncType, SrcBlendFuncType, TexturingType};
use crate::rendering::texture_system::texture_base::{TextureAddressMode, TextureFilterMode};
use crate::texture_system::TextureClass;

/// GPU state manager for the 2D renderer. Responsible for caching pipelines,
/// samplers, and fallback textures so we can translate Render2D batches into
/// WGPU draw calls without rebuilding state every frame.
pub struct Render2DGpuContext {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    target_format: wgpu::TextureFormat,
    shader_module: wgpu::ShaderModule,
    textured_layout: wgpu::BindGroupLayout,
    textured_pipeline_layout: wgpu::PipelineLayout,
    solid_pipeline_layout: wgpu::PipelineLayout,
    pipeline_cache: HashMap<PipelineKey, Arc<wgpu::RenderPipeline>>,
    sampler_cache: HashMap<SamplerKey, Arc<wgpu::Sampler>>,
    fallback_texture: wgpu::Texture,
    default_sampler: Arc<wgpu::Sampler>,
}

impl Render2DGpuContext {
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        target_format: wgpu::TextureFormat,
    ) -> Self {
        let shader_module =
            device.create_shader_module(wgpu::include_wgsl!("../shader_system/render2d.wgsl"));

        let textured_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render2D Texture Bind Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let textured_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render2D Pipeline Layout (Textured)"),
                bind_group_layouts: &[&textured_layout],
                push_constant_ranges: &[],
            });

        let solid_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render2D Pipeline Layout (Solid)"),
                bind_group_layouts: &[],
                push_constant_ranges: &[],
            });

        // Create a 1x1 white fallback texture for uninitialised assets.
        let fallback_texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Render2D Fallback Texture"),
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
                texture: &fallback_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &[255, 255, 255, 255],
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

        let default_sampler = Arc::new(device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Render2D Default Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: f32::MAX,
            compare: None,
            anisotropy_clamp: 1,
            border_color: None,
        }));

        Self {
            device,
            queue,
            target_format,
            shader_module,
            textured_layout,
            textured_pipeline_layout,
            solid_pipeline_layout,
            pipeline_cache: HashMap::new(),
            sampler_cache: HashMap::new(),
            fallback_texture,
            default_sampler,
        }
    }

    #[inline]
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Retrieve (or create) a render pipeline matching the requested blending
    /// and shading configuration.
    pub fn pipeline_for(
        &mut self,
        textured: bool,
        grayscale: bool,
        src: SrcBlendFuncType,
        dst: DstBlendFuncType,
    ) -> Arc<wgpu::RenderPipeline> {
        let key = PipelineKey {
            textured,
            grayscale: textured && grayscale,
            src: src as u8,
            dst: dst as u8,
        };

        if !self.pipeline_cache.contains_key(&key) {
            let pipeline = Arc::new(self.build_pipeline(key));
            self.pipeline_cache.insert(key, pipeline);
        }

        self.pipeline_cache
            .get(&key)
            .cloned()
            .expect("pipeline cache entry")
    }

    fn build_pipeline(&self, key: PipelineKey) -> wgpu::RenderPipeline {
        let vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex2D>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 8,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: 16,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Uint32,
                },
            ],
        };

        let fragment_entry = match (key.textured, key.grayscale) {
            (true, true) => "fs_grayscale_main",
            (true, false) => "fs_main",
            (false, _) => "fs_solid_main",
        };

        let blend = wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: map_src_factor(key.src),
                dst_factor: map_dst_factor(key.dst),
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: map_src_factor(key.src),
                dst_factor: map_dst_factor(key.dst),
                operation: wgpu::BlendOperation::Add,
            },
        };

        self.device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render2D Pipeline"),
                layout: Some(if key.textured {
                    &self.textured_pipeline_layout
                } else {
                    &self.solid_pipeline_layout
                }),
                vertex: wgpu::VertexState {
                    module: &self.shader_module,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_layout],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader_module,
                    entry_point: Some(fragment_entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: self.target_format,
                        blend: Some(blend),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
    }

    /// Create a bind group for the provided texture (or a fallback). When the
    /// texture is missing GPU data a shared 1x1 white texture is used instead
    /// so the draw call still succeeds.
    pub fn create_texture_bind_group(
        &mut self,
        texture: Option<&mut TextureClass>,
    ) -> wgpu::BindGroup {
        let (view, sampler) = match texture {
            Some(texture) => {
                if texture.gpu_texture.is_none() && !texture.data.is_empty() {
                    // Upload CPU texture data on demand.
                    if let Err(err) = texture.create_wgpu_texture(self.device(), self.queue()) {
                        warn!(
                            "failed to upload texture {} for Render2D: {err}",
                            texture.name
                        );
                    }
                }

                let sampler = self.sampler_for(texture.stage_settings());
                let view = texture.get_texture_view().unwrap_or_else(|| {
                    self.fallback_texture
                        .create_view(&wgpu::TextureViewDescriptor::default())
                });
                (view, sampler)
            }
            None => (
                self.fallback_texture
                    .create_view(&wgpu::TextureViewDescriptor::default()),
                Arc::clone(&self.default_sampler),
            ),
        };

        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render2D Texture Bind Group"),
            layout: &self.textured_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&*sampler),
                },
            ],
        })
    }

    fn sampler_for(&mut self, settings: TextureStageSettings) -> Arc<wgpu::Sampler> {
        let key = SamplerKey::from_settings(settings);

        if !self.sampler_cache.contains_key(&key) {
            let sampler = build_sampler(&self.device, key);
            self.sampler_cache.insert(key, Arc::new(sampler));
        }

        self.sampler_cache
            .get(&key)
            .expect("sampler cache entry")
            .clone()
    }
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct PipelineKey {
    textured: bool,
    grayscale: bool,
    src: u8,
    dst: u8,
}

#[derive(Clone, Copy, Hash, PartialEq, Eq)]
struct SamplerKey {
    address_u: u8,
    address_v: u8,
    filter: u8,
    mipmap: u8,
    anisotropy: u8,
    lod_max_milli: u16,
}

impl SamplerKey {
    fn from_settings(settings: TextureStageSettings) -> Self {
        let (mipmap, lod_max_milli) = match settings.mip_strategy {
            TextureMipStrategy::Full => (wgpu::FilterMode::Linear, u16::MAX),
            TextureMipStrategy::NoMips => (wgpu::FilterMode::Nearest, 0),
            TextureMipStrategy::MaxLevels(levels) => (
                wgpu::FilterMode::Linear,
                (levels as u16).saturating_mul(1000),
            ),
        };

        let filter_code = match settings.filter {
            TextureFilterMode::Point | TextureFilterMode::Nearest => 0,
            TextureFilterMode::Linear => 1,
            TextureFilterMode::Anisotropic => 2,
        };

        let anisotropy = match settings.filter {
            TextureFilterMode::Anisotropic => settings.anisotropy.clamp(1, 16) as u8,
            _ => 1,
        };

        Self {
            address_u: encode_address(settings.address_u),
            address_v: encode_address(settings.address_v),
            filter: filter_code,
            mipmap: if mipmap == wgpu::FilterMode::Linear {
                1
            } else {
                0
            },
            anisotropy,
            lod_max_milli,
        }
    }
}

fn build_sampler(device: &wgpu::Device, key: SamplerKey) -> wgpu::Sampler {
    let anisotropy = if key.filter == 2 {
        key.anisotropy.max(1) as u16
    } else {
        1
    };

    device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("Render2D Cached Sampler"),
        address_mode_u: decode_address(key.address_u),
        address_mode_v: decode_address(key.address_v),
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: if key.filter == 0 {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        },
        min_filter: if key.filter == 0 {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        },
        mipmap_filter: if key.mipmap == 0 {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        },
        lod_min_clamp: 0.0,
        lod_max_clamp: if key.lod_max_milli == u16::MAX {
            f32::MAX
        } else {
            (key.lod_max_milli as f32) / 1000.0
        },
        compare: None,
        anisotropy_clamp: anisotropy,
        border_color: None,
    })
}

fn map_src_factor(src: u8) -> wgpu::BlendFactor {
    match src {
        x if x == SrcBlendFuncType::One as u8 => wgpu::BlendFactor::One,
        x if x == SrcBlendFuncType::Zero as u8 => wgpu::BlendFactor::Zero,
        x if x == SrcBlendFuncType::SrcColor as u8 => wgpu::BlendFactor::Src,
        x if x == SrcBlendFuncType::InvSrcColor as u8 => wgpu::BlendFactor::OneMinusSrc,
        x if x == SrcBlendFuncType::SrcAlpha as u8 => wgpu::BlendFactor::SrcAlpha,
        x if x == SrcBlendFuncType::InvSrcAlpha as u8 => wgpu::BlendFactor::OneMinusSrcAlpha,
        _ => wgpu::BlendFactor::One,
    }
}

fn map_dst_factor(dst: u8) -> wgpu::BlendFactor {
    match dst {
        x if x == DstBlendFuncType::One as u8 => wgpu::BlendFactor::One,
        x if x == DstBlendFuncType::Zero as u8 => wgpu::BlendFactor::Zero,
        x if x == DstBlendFuncType::SrcColor as u8 => wgpu::BlendFactor::Src,
        x if x == DstBlendFuncType::InvSrcColor as u8 => wgpu::BlendFactor::OneMinusSrc,
        x if x == DstBlendFuncType::SrcAlpha as u8 => wgpu::BlendFactor::SrcAlpha,
        x if x == DstBlendFuncType::InvSrcAlpha as u8 => wgpu::BlendFactor::OneMinusSrcAlpha,
        x if x == DstBlendFuncType::DstAlpha as u8 => wgpu::BlendFactor::DstAlpha,
        x if x == DstBlendFuncType::InvDstAlpha as u8 => wgpu::BlendFactor::OneMinusDstAlpha,
        x if x == DstBlendFuncType::DstColor as u8 => wgpu::BlendFactor::Dst,
        x if x == DstBlendFuncType::InvDstColor as u8 => wgpu::BlendFactor::OneMinusDst,
        _ => wgpu::BlendFactor::Zero,
    }
}

fn encode_address(address: TextureAddressMode) -> u8 {
    match address {
        TextureAddressMode::Wrap => 0,
        TextureAddressMode::Clamp => 1,
        TextureAddressMode::Mirror => 2,
        TextureAddressMode::Border => 3,
        TextureAddressMode::Repeat => 4,
    }
}

fn decode_address(code: u8) -> wgpu::AddressMode {
    match code {
        0 | 4 => wgpu::AddressMode::Repeat,
        1 => wgpu::AddressMode::ClampToEdge,
        2 => wgpu::AddressMode::MirrorRepeat,
        3 => wgpu::AddressMode::ClampToBorder,
        _ => wgpu::AddressMode::ClampToEdge,
    }
}

impl Render2DGpuContext {}

/// Helper to convert screen-space vertices into the clip space positions
/// expected by the WGSL shader.
pub fn screen_to_clip(position: Vec2, scale: Vec2, offset: Vec2) -> Vec2 {
    Vec2::new(
        position.x * scale.x + offset.x,
        position.y * scale.y + offset.y,
    )
}

/// Determine whether the shader configuration should treat the current batch as
/// textured based on both the shader bits and the presence of a bound texture.
pub fn is_textured(
    shader: &crate::rendering::shader_system::ShaderClass,
    has_texture: bool,
) -> bool {
    has_texture && matches!(shader.get_texturing(), TexturingType::Enable)
}
