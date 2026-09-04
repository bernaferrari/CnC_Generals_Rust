//! # UI Renderer
//!
//! WGPU-based immediate mode UI rendering system that matches the original
//! Command & Conquer Generals GUI rendering capabilities.
//!
//! Features:
//! - Immediate mode rendering with retained batching
//! - Text rendering with font support
//! - Image/texture rendering with alpha blending
//! - Window hierarchy rendering with proper Z-ordering
//! - Animation support for transitions and effects
//! - Multi-sampling anti-aliasing support
//! - Hardware-accelerated rendering on all platforms

use bytemuck::{Pod, Zeroable};
use cosmic_text::{
    Attrs, Buffer as TextBuffer, Color as TextColor, Family, FontSystem, Metrics, Shaping, Stretch,
    Style, SwashCache, Weight, Wrap,
};
use fontdue::{Font, FontSettings};
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, Buffer, BufferBindingType, BufferDescriptor, BufferUsages,
    Color, ColorTargetState, ColorWrites, CommandEncoder, Device, FragmentState, LoadOp,
    MultisampleState, Operations, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, Queue, RenderPass, RenderPassColorAttachment, RenderPassDescriptor,
    RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModule, ShaderSource, ShaderStages, StoreOp, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureSampleType, TextureUsages, TextureView,
    TextureViewDescriptor, TextureViewDimension, VertexBufferLayout, VertexState, VertexStepMode,
};

/// UI Renderer errors
#[derive(Error, Debug)]
pub enum UIRendererError {
    #[error("WGPU error: {0}")]
    WgpuError(String),
    #[error("Font loading error: {0}")]
    FontError(String),
    #[error("Texture loading error: {0}")]
    TextureError(String),
    #[error("Shader compilation error: {0}")]
    ShaderError(String),
    #[error("Buffer creation error: {0}")]
    BufferError(String),
    #[error("Invalid parameters: {0}")]
    InvalidParameters(String),
}

type Result<T> = std::result::Result<T, UIRendererError>;

/// Vertex data for UI rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UIVertex {
    position: [f32; 3],  // xyz
    tex_coord: [f32; 2], // uv
    color: [f32; 4],     // rgba
}

/// Instance data for batched rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UIInstance {
    transform: [[f32; 4]; 4], // 4x4 transformation matrix
    color_modifier: [f32; 4], // rgba color modification
    texture_rect: [f32; 4],   // texture coordinates (uvst)
}

/// Uniform data for global rendering parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct UIUniforms {
    view_projection: [[f32; 4]; 4], // 4x4 view-projection matrix
    screen_size: [f32; 2],          // screen width, height
    time: f32,                      // current time for animations
    _padding: f32,
}

/// Drawing command for batched rendering
#[derive(Debug, Clone)]
pub struct UIDrawCommand {
    pub vertices: Vec<UIVertex>,
    pub indices: Vec<u32>,
    pub texture: Option<Arc<TextureView>>,
    pub blend_mode: UIBlendMode,
    pub scissor_rect: Option<UIRect>,
    pub z_order: f32,
}

/// Blend modes for UI rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UIBlendMode {
    Alpha,
    Additive,
    Multiply,
    Screen,
    None,
    Grayscale,
}

/// Rectangle for UI elements
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UIRect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl UIRect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub fn contains(&self, x: f32, y: f32) -> bool {
        x >= self.x && x <= self.x + self.width && y >= self.y && y <= self.y + self.height
    }

    pub fn intersects(&self, other: &UIRect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }
}

/// Text layout information
#[derive(Debug, Clone)]
pub struct TextLayout {
    pub text: String,
    /// Pixel em height (C++ GDI: MulDiv(point, 96, 72)).
    pub font_size: f32,
    pub color: [f32; 4],
    pub bounds: UIRect,
    pub alignment: TextAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub word_wrap: bool,
    pub single_line: bool,
}


/// Per-frame CPU-side UI command state.
///
/// The Main host updates some presentation UI (selection bars and drawable
/// icon UI) before the WGPU post-scene overlay pass starts.  Those commands
/// belong to the upcoming frame, not the previous one.  Keeping the frame
/// state separate makes that distinction explicit without borrowing the
/// renderer through a global/raw-pointer re-entry path.
#[derive(Default)]
struct UIFrameBuffers {
    draw_commands: Vec<UIDrawCommand>,
    vertex_data: Vec<UIVertex>,
    index_data: Vec<u32>,
    instance_data: Vec<UIInstance>,
    open: bool,
}

impl UIFrameBuffers {
    fn clear_scratch(&mut self) {
        self.vertex_data.clear();
        self.index_data.clear();
        self.instance_data.clear();
    }

    fn clear_all(&mut self) {
        self.draw_commands.clear();
        self.clear_scratch();
    }

    /// Start a self-contained frame, dropping all previously queued work.
    fn begin_fresh(&mut self) {
        self.clear_all();
        self.open = true;
    }

    /// Start Main's post-scene overlay frame.
    ///
    /// A closed frame may already contain presentation commands emitted during
    /// the update phase.  Preserve those commands so the WND traversal can
    /// append to them.  An already-open frame was abandoned (for example by a
    /// prior UI failure), so discard it rather than leaking stale commands.
    fn begin_overlay(&mut self) {
        if self.open {
            self.clear_all();
        } else {
            self.clear_scratch();
        }
        self.open = true;
    }

    fn end(&mut self) {
        self.clear_all();
        self.open = false;
    }
}

/// Text alignment options
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
    Justify,
}

/// Vertical text alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VerticalAlignment {
    Top,
    Middle,
    Bottom,
}

/// UI Renderer - main rendering system
pub struct UIRenderer {
    device: Arc<Device>,
    queue: Arc<Queue>,

    solid_pipeline: RenderPipeline,
    textured_pipeline: RenderPipeline,
    textured_additive_pipeline: RenderPipeline,
    textured_grayscale_pipeline: RenderPipeline,
    text_pipeline: RenderPipeline,

    // Shader modules
    ui_shader: ShaderModule,
    text_shader: ShaderModule,

    // Bind group layouts
    uniform_bind_group_layout: BindGroupLayout,
    texture_bind_group_layout: BindGroupLayout,

    // Buffers
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    instance_buffer: Buffer,
    uniform_buffer: Buffer,
    vertex_capacity: usize,
    index_capacity: usize,

    // Bind groups
    uniform_bind_group: BindGroup,

    // Text rendering system. cosmic_text types are `Send + !Sync`; wrap them
    // so UIRenderer can live in `Arc<RwLock<UIRenderer>>` without a lying
    // `unsafe impl Sync`.
    font_runtime: Mutex<FontRuntime>,
    font_cache: HashMap<String, Font>,
    /// C++ DisplayString retains rasterized glyphs. Re-shaping every label
    /// every frame created a wgpu texture per button and froze Menu.
    /// Value carries the placed quad — the atlas canvas may exceed the
    /// layout bounds so real glyph extents are never clipped.
    text_texture_cache: HashMap<u64, (Arc<TextureView>, UIRect)>,

    // Textures and samplers
    default_texture: Arc<TextureView>,
    default_texture_bind_group: BindGroup,
    linear_sampler: Sampler,
    nearest_sampler: Sampler,

    // Rendering state
    screen_size: (u32, u32),
    view_projection: Mat4,
    current_time: f32,

    // Command batching
    frame_buffers: UIFrameBuffers,

    // Performance statistics
    last_frame_stats: RenderStats,
}

/// cosmic_text layout state. `FontSystem` / `SwashCache` / `TextBuffer` are
/// `Send + !Sync` (interior `RefCell`). UIRenderer serializes them with
/// `Mutex<FontRuntime>` instead of an `unsafe impl Sync`.
struct FontRuntime {
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_buffer: TextBuffer,
}

/// Rendering performance statistics
#[derive(Debug, Clone, Default)]
pub struct RenderStats {
    pub draw_calls: u32,
    pub vertices_rendered: u32,
    pub triangles_rendered: u32,
    pub texture_switches: u32,
    pub render_time_ms: f32,
}

impl UIRenderer {
    /// Create a new UI renderer
    pub fn new(device: Arc<Device>, queue: Arc<Queue>, format: TextureFormat) -> Result<Self> {
        let mut font_system = FontSystem::new();
        // C++ GDI resolved font families by name (render2dsentence.cpp:1477
        // `Create_GDI_Font(font_name)`). cosmic-text defaults `Family::SansSerif`
        // to "Fira Sans" (cosmic-text-0.10.0/src/font/system.rs:64), which is not
        // installed on typical systems — glyphs then come from an arbitrary
        // fallback face with wrong advances (the Arial-10 glyph corruption).
        // Register the game's font files so `Family::Name("Arial")` resolves to
        // real Arial (the same files the measuring side prefers).
        for path in super::font::font_atlas_files() {
            if let Ok(bytes) = std::fs::read(&path) {
                let source: Arc<dyn AsRef<[u8]> + Send + Sync> = Arc::new(bytes);
                font_system
                    .db_mut()
                    .load_font_source(fontdb::Source::Binary(source));
            }
        }
        let text_buffer = TextBuffer::new(&mut font_system, Metrics::new(14.0, 16.0));

        // Create shader modules
        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/ui.wgsl").into()),
        });

        let text_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Text Shader"),
            source: ShaderSource::Wgsl(include_str!("shaders/text.wgsl").into()),
        });

        // Create bind group layouts
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("UI Uniform Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX | ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("UI Texture Bind Group Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Texture {
                            sample_type: TextureSampleType::Float { filterable: true },
                            view_dimension: TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::FRAGMENT,
                        ty: BindingType::Sampler(SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("UI Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create vertex buffer layout
        let vertex_buffer_layout = VertexBufferLayout {
            array_stride: std::mem::size_of::<UIVertex>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0, // position
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1, // tex_coord
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2, // color
                },
            ],
        };

        // Create render pipelines
        let solid_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("UI Solid Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_solid"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let textured_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("UI Textured Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_textured"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let textured_additive_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("UI Textured Additive Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &ui_shader,
                entry_point: Some("fs_textured"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState {
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
                    }),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let textured_grayscale_pipeline =
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("UI Textured Grayscale Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &ui_shader,
                    entry_point: Some("vs_main"),
                    buffers: &[vertex_buffer_layout.clone()],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module: &ui_shader,
                    entry_point: Some("fs_disabled"),
                    targets: &[Some(ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    polygon_mode: PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview: None,
                cache: None,
            });

        let text_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("UI Text Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &text_shader,
                entry_point: Some("vs_main"),
                buffers: &[vertex_buffer_layout.clone()],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &text_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create buffers
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Vertex Buffer"),
            size: (std::mem::size_of::<UIVertex>() * 65536) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Index Buffer"),
            size: (std::mem::size_of::<u32>() * 98304) as u64, // 1.5x vertex count
            usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let instance_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Instance Buffer"),
            size: (std::mem::size_of::<UIInstance>() * 16384) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("UI Uniform Buffer"),
            size: std::mem::size_of::<UIUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create uniform bind group
        let uniform_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("UI Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create default white texture
        let default_texture_data = [255u8; 4]; // White pixel
        let default_texture = device.create_texture(&TextureDescriptor {
            label: Some("UI Default Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &default_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &default_texture_data,
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

        let default_texture_view =
            Arc::new(default_texture.create_view(&TextureViewDescriptor::default()));

        // Create samplers
        let linear_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("UI Linear Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let nearest_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("UI Nearest Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let default_texture_bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("UI Default Texture Bind Group"),
            layout: &texture_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&default_texture_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&linear_sampler),
                },
            ],
        });

        Ok(Self {
            device,
            queue,
            solid_pipeline,
            textured_pipeline,
            textured_additive_pipeline,
            textured_grayscale_pipeline,
            text_pipeline,
            ui_shader,
            text_shader,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            font_runtime: Mutex::new(FontRuntime {
                font_system,
                swash_cache: SwashCache::new(),
                text_buffer,
            }),
            font_cache: HashMap::new(),
            text_texture_cache: HashMap::new(),
            vertex_buffer,
            index_buffer,
            instance_buffer,
            uniform_buffer,
            vertex_capacity: 65536,
            index_capacity: 98304,
            uniform_bind_group,
            default_texture: default_texture_view,
            default_texture_bind_group,
            linear_sampler,
            nearest_sampler,
            screen_size: (800, 600),
            view_projection: Mat4::IDENTITY,
            current_time: 0.0,
            frame_buffers: UIFrameBuffers::default(),
            last_frame_stats: RenderStats::default(),
        })
    }

    fn ensure_geometry_buffer_capacity(&mut self) {
        // Fail-closed caps: runaway draw command storms (bad sizes / reentry)
        // previously requested multi-GB UI buffers and aborted the process
        // (wgpu Validation Error: Buffer size > max buffer size).
        const MAX_UI_INDICES: usize = 1 << 21;
        let required_vertices = self.frame_buffers.vertex_data.len();
        if required_vertices > Self::MAX_UI_VERTICES {
            log::error!(
                "UI vertex flood ({} > {}); clearing draw geometry to fail closed",
                required_vertices,
                Self::MAX_UI_VERTICES
            );
            self.frame_buffers.clear_all();
            return;
        }
        if required_vertices > self.vertex_capacity {
            let new_capacity = required_vertices
                .next_power_of_two()
                .max(self.vertex_capacity * 2)
                .min(Self::MAX_UI_VERTICES);
            self.vertex_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("UI Vertex Buffer"),
                size: (std::mem::size_of::<UIVertex>() * new_capacity) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = new_capacity;
        }

        let required_indices = self.frame_buffers.index_data.len();
        if required_indices > MAX_UI_INDICES {
            log::error!(
                "UI index flood ({} > {}); clearing draw geometry to fail closed",
                required_indices,
                MAX_UI_INDICES
            );
            self.frame_buffers.clear_all();
            return;
        }
        if required_indices > self.index_capacity {
            let new_capacity = required_indices
                .next_power_of_two()
                .max(self.index_capacity * 2)
                .min(MAX_UI_INDICES);
            self.index_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("UI Index Buffer"),
                size: (std::mem::size_of::<u32>() * new_capacity) as u64,
                usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.index_capacity = new_capacity;
        }
    }

    /// Set the screen size and update projection matrix
    pub fn set_screen_size(&mut self, width: u32, height: u32) {
        self.screen_size = (width, height);

        // Create orthographic projection matrix (0,0 at top-left)
        self.view_projection =
            Mat4::orthographic_rh(0.0, width as f32, height as f32, 0.0, -1.0, 1.0);
    }

    /// Set the current time for animations
    pub fn set_time(&mut self, time: f32) {
        self.current_time = time;
    }

    /// Begin a new frame
    pub fn begin_frame(&mut self) {
        self.frame_buffers.begin_fresh();
    }

    /// Begin Main's WGPU overlay pass without discarding presentation commands
    /// queued earlier in this same app frame.
    ///
    /// The normal display path should use [`Self::begin_frame`].  This variant
    /// is for Main's sole-present path, where GameClient updates selection and
    /// drawable UI before the post-scene WND traversal happens.
    pub fn begin_overlay_frame(&mut self) {
        self.frame_buffers.begin_overlay();
    }

    /// Whether a UI frame is currently open and needs cleanup.
    pub fn is_frame_open(&self) -> bool {
        self.frame_buffers.open
    }

    /// GPU triangle list for a gadget fill rect (two triangles, C++ StretchRect).
    #[must_use]
    pub fn gadget_gpu_fill_rect_mesh(
        rect: UIRect,
        color: [f32; 4],
        z_order: f32,
    ) -> (Vec<[f32; 3]>, Vec<[f32; 2]>, Vec<[f32; 4]>, Vec<u32>) {
        (
            vec![
                [rect.x, rect.y, z_order],
                [rect.x + rect.width, rect.y, z_order],
                [rect.x + rect.width, rect.y + rect.height, z_order],
                [rect.x, rect.y + rect.height, z_order],
            ],
            vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]],
            vec![color, color, color, color],
            vec![0, 1, 2, 0, 2, 3],
        )
    }

    /// Hard cap on queued gadget draws per frame. A runaway border-tile or
    /// WND re-draw loop used to push hundreds of millions of verts and get
    /// the process SIGKILL'd (code=None) mid-InGame construct.
    pub const MAX_DRAW_COMMANDS_PER_FRAME: usize = 8_192;
    pub const MAX_UI_VERTICES: usize = 1 << 20;

    /// Textured rect with an explicit blend mode (C++ `DrawImageMode`).
    pub fn draw_textured_rect_ex(
        &mut self,
        rect: UIRect,
        texture: Arc<TextureView>,
        color: [f32; 4],
        tex_rect: Option<UIRect>,
        blend_mode: UIBlendMode,
        z_order: f32,
    ) {
        if self.frame_buffers.draw_commands.len() >= Self::MAX_DRAW_COMMANDS_PER_FRAME {
            return;
        }
        let tex_rect = tex_rect.unwrap_or(UIRect::new(0.0, 0.0, 1.0, 1.0));
        let vertices = vec![
            UIVertex {
                position: [rect.x, rect.y, z_order],
                tex_coord: [tex_rect.x, tex_rect.y],
                color,
            },
            UIVertex {
                position: [rect.x + rect.width, rect.y, z_order],
                tex_coord: [tex_rect.x + tex_rect.width, tex_rect.y],
                color,
            },
            UIVertex {
                position: [rect.x + rect.width, rect.y + rect.height, z_order],
                tex_coord: [tex_rect.x + tex_rect.width, tex_rect.y + tex_rect.height],
                color,
            },
            UIVertex {
                position: [rect.x, rect.y + rect.height, z_order],
                tex_coord: [tex_rect.x, tex_rect.y + tex_rect.height],
                color,
            },
        ];
        self.push_draw_command(UIDrawCommand {
            vertices,
            indices: vec![0, 1, 2, 0, 2, 3],
            texture: Some(texture),
            blend_mode,
            scissor_rect: None,
            z_order,
        });
    }

    /// Arbitrary textured mesh (C++ `Add_Tri` rotate-90 path).
    pub fn draw_textured_mesh(
        &mut self,
        positions: &[[f32; 2]],
        uvs: &[[f32; 2]],
        indices: &[u32],
        texture: Arc<TextureView>,
        color: [f32; 4],
        blend_mode: UIBlendMode,
        z_order: f32,
    ) {
        if positions.len() != uvs.len() || positions.is_empty() {
            return;
        }
        let vertices = positions
            .iter()
            .zip(uvs.iter())
            .map(|(pos, uv)| UIVertex {
                position: [pos[0], pos[1], z_order],
                tex_coord: *uv,
                color,
            })
            .collect();
        self.push_draw_command(UIDrawCommand {
            vertices,
            indices: indices.to_vec(),
            texture: Some(texture),
            blend_mode,
            scissor_rect: None,
            z_order,
        });
    }
    fn push_draw_command(&mut self, command: UIDrawCommand) {
        if self.frame_buffers.draw_commands.len() >= Self::MAX_DRAW_COMMANDS_PER_FRAME {
            return;
        }
        self.frame_buffers.draw_commands.push(command);
    }

    /// Add a rectangle draw command
    pub fn draw_rect(&mut self, rect: UIRect, color: [f32; 4], z_order: f32) {
        if self.frame_buffers.draw_commands.len() >= Self::MAX_DRAW_COMMANDS_PER_FRAME {
            return;
        }
        let (positions, uvs, colors, indices) =
            Self::gadget_gpu_fill_rect_mesh(rect, color, z_order);
        let vertices = positions
            .into_iter()
            .zip(uvs)
            .zip(colors)
            .map(|((position, tex_coord), color)| UIVertex {
                position,
                tex_coord,
                color,
            })
            .collect();

        self.push_draw_command(UIDrawCommand {
            vertices,
            indices,
            texture: None,
            blend_mode: UIBlendMode::Alpha,
            scissor_rect: None,
            z_order,
        });
    }

    /// Add a textured rectangle draw command
    pub fn draw_textured_rect(
        &mut self,
        rect: UIRect,
        texture: Arc<TextureView>,
        color: [f32; 4],
        tex_rect: Option<UIRect>,
        z_order: f32,
    ) {
        if self.frame_buffers.draw_commands.len() >= Self::MAX_DRAW_COMMANDS_PER_FRAME {
            return;
        }
        let tex_rect = tex_rect.unwrap_or(UIRect::new(0.0, 0.0, 1.0, 1.0));

        let vertices = vec![
            UIVertex {
                position: [rect.x, rect.y, z_order],
                tex_coord: [tex_rect.x, tex_rect.y],
                color,
            },
            UIVertex {
                position: [rect.x + rect.width, rect.y, z_order],
                tex_coord: [tex_rect.x + tex_rect.width, tex_rect.y],
                color,
            },
            UIVertex {
                position: [rect.x + rect.width, rect.y + rect.height, z_order],
                tex_coord: [tex_rect.x + tex_rect.width, tex_rect.y + tex_rect.height],
                color,
            },
            UIVertex {
                position: [rect.x, rect.y + rect.height, z_order],
                tex_coord: [tex_rect.x, tex_rect.y + tex_rect.height],
                color,
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        self.push_draw_command(UIDrawCommand {
            vertices,
            indices,
            texture: Some(texture),
            blend_mode: UIBlendMode::Alpha,
            scissor_rect: None,
            z_order,
        });
    }

    /// Create a transient RGBA texture for immediate use (e.g., video buffers).
    pub fn create_texture_from_rgba(
        &self,
        width: u32,
        height: u32,
        data: &[u8],
    ) -> Arc<TextureView> {
        if width == 0 || height == 0 {
            return self.default_texture.clone();
        }
        let expected_len = (width as usize)
            .saturating_mul(height as usize)
            .saturating_mul(4);
        if data.len() < expected_len {
            return self.default_texture.clone();
        }

        let texture = self.device.create_texture(&TextureDescriptor {
            label: Some("UI Video Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let bytes_per_row = width.saturating_mul(4);
        let aligned_bytes_per_row = (bytes_per_row + 255) & !255;
        if aligned_bytes_per_row == bytes_per_row {
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &data[..expected_len],
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bytes_per_row),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        } else {
            let mut padded = vec![0u8; aligned_bytes_per_row as usize * height as usize];
            let row_bytes = bytes_per_row as usize;
            let aligned_row_bytes = aligned_bytes_per_row as usize;
            for row in 0..height as usize {
                let src_start = row * row_bytes;
                let dst_start = row * aligned_row_bytes;
                padded[dst_start..dst_start + row_bytes]
                    .copy_from_slice(&data[src_start..src_start + row_bytes]);
            }
            self.queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                &padded,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(aligned_bytes_per_row),
                    rows_per_image: Some(height),
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
        }

        Arc::new(texture.create_view(&TextureViewDescriptor::default()))
    }

    /// Draw a rectangle outline
    pub fn draw_rect_outline(
        &mut self,
        rect: UIRect,
        thickness: f32,
        color: [f32; 4],
        z_order: f32,
    ) {
        // Top edge
        self.draw_rect(
            UIRect::new(rect.x, rect.y, rect.width, thickness),
            color,
            z_order,
        );
        // Bottom edge
        self.draw_rect(
            UIRect::new(
                rect.x,
                rect.y + rect.height - thickness,
                rect.width,
                thickness,
            ),
            color,
            z_order,
        );
        // Left edge
        self.draw_rect(
            UIRect::new(rect.x, rect.y, thickness, rect.height),
            color,
            z_order,
        );
        // Right edge
        self.draw_rect(
            UIRect::new(
                rect.x + rect.width - thickness,
                rect.y,
                thickness,
                rect.height,
            ),
            color,
            z_order,
        );
    }

    /// Draw a line segment with thickness.
    pub fn draw_line(
        &mut self,
        start: Vec2,
        end: Vec2,
        thickness: f32,
        color: [f32; 4],
        z_order: f32,
    ) {
        self.draw_line_gradient(start, end, thickness, color, color, z_order);
    }

    pub fn draw_line_gradient(
        &mut self,
        start: Vec2,
        end: Vec2,
        thickness: f32,
        start_color: [f32; 4],
        end_color: [f32; 4],
        z_order: f32,
    ) {
        let dir = end - start;
        let length = dir.length();
        if length <= f32::EPSILON {
            return;
        }
        let normal = Vec2::new(-dir.y, dir.x).normalize() * (thickness * 0.5);

        let p0 = start + normal;
        let p1 = start - normal;
        let p2 = end - normal;
        let p3 = end + normal;

        let vertices = vec![
            UIVertex {
                position: [p0.x, p0.y, z_order],
                tex_coord: [0.0, 0.0],
                color: start_color,
            },
            UIVertex {
                position: [p1.x, p1.y, z_order],
                tex_coord: [0.0, 1.0],
                color: start_color,
            },
            UIVertex {
                position: [p2.x, p2.y, z_order],
                tex_coord: [1.0, 1.0],
                color: end_color,
            },
            UIVertex {
                position: [p3.x, p3.y, z_order],
                tex_coord: [1.0, 0.0],
                color: end_color,
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        self.push_draw_command(UIDrawCommand {
            vertices,
            indices,
            texture: Some(self.default_texture.clone()),
            blend_mode: UIBlendMode::Alpha,
            scissor_rect: None,
            z_order,
        });
    }

    /// Draw a solid triangle.
    pub fn draw_triangle(&mut self, p0: Vec2, p1: Vec2, p2: Vec2, color: [f32; 4], z_order: f32) {
        let vertices = vec![
            UIVertex {
                position: [p0.x, p0.y, z_order],
                tex_coord: [0.0, 0.0],
                color,
            },
            UIVertex {
                position: [p1.x, p1.y, z_order],
                tex_coord: [0.0, 0.0],
                color,
            },
            UIVertex {
                position: [p2.x, p2.y, z_order],
                tex_coord: [0.0, 0.0],
                color,
            },
        ];
        let indices = vec![0u32, 1, 2];

        self.push_draw_command(UIDrawCommand {
            vertices,
            indices,
            texture: None,
            blend_mode: UIBlendMode::Alpha,
            scissor_rect: None,
            z_order,
        });
    }

    fn text_layout_cache_key(layout: &TextLayout, font_name: &str, bold: bool) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        layout.text.hash(&mut hasher);
        font_name.hash(&mut hasher);
        bold.hash(&mut hasher);
        layout.font_size.to_bits().hash(&mut hasher);
        for channel in layout.color {
            channel.to_bits().hash(&mut hasher);
        }
        layout.bounds.width.to_bits().hash(&mut hasher);
        layout.bounds.height.to_bits().hash(&mut hasher);
        std::mem::discriminant(&layout.alignment).hash(&mut hasher);
        std::mem::discriminant(&layout.vertical_alignment).hash(&mut hasher);
        layout.word_wrap.hash(&mut hasher);
        layout.single_line.hash(&mut hasher);
        hasher.finish()
    }
    /// Add a text draw command (default game font: Arial regular — the C++
    /// fallback family, render2dsentence.cpp:1481).
    pub fn draw_text(&mut self, layout: &TextLayout, z_order: f32) -> Result<()> {
        self.draw_text_with_font(layout, "Arial", false, z_order)
    }

    /// Add a text draw command with an explicit game font (C++ renders gadget
    /// text with the window's GameFont family and weight).
    pub fn draw_text_with_font(
        &mut self,
        layout: &TextLayout,
        font_name: &str,
        bold: bool,
        z_order: f32,
    ) -> Result<()> {
        if layout.text.is_empty() || layout.bounds.width <= 0.0 || layout.bounds.height <= 0.0 {
            return Ok(());
        }
        let cache_key = Self::text_layout_cache_key(layout, font_name, bold);
        if let Some((texture, quad)) = self.text_texture_cache.get(&cache_key).cloned() {
            self.draw_textured_rect(quad, texture, [1.0, 1.0, 1.0, 1.0], None, z_order);
            return Ok(());
        }

        let canvas_width = layout.bounds.width.ceil().max(1.0).min(2048.0) as u32;
        let canvas_height = layout.bounds.height.ceil().max(1.0).min(512.0) as u32;
        let mut canvas = vec![0u8; canvas_width as usize * canvas_height as usize * 4];

        let metrics = Metrics::new(layout.font_size.max(1.0), (layout.font_size * 1.2).max(1.0));
        let wrap_mode = if layout.word_wrap && !layout.single_line {
            Wrap::Word
        } else {
            Wrap::None
        };
        let text = if layout.single_line {
            layout.text.replace('\r', "").replace('\n', " ")
        } else {
            layout.text.clone()
        };

        // C++ renders gadget text with the window's GameFont family and weight
        // (Create_GDI_Font: `CreateFont(..., bold ? FW_BOLD : FW_NORMAL, ...,
        // font_name)`, render2dsentence.cpp:1507-1512).
        let attrs = Attrs::new()
            .family(if font_name.is_empty() {
                Family::SansSerif
            } else {
                Family::Name(font_name)
            })
            .weight(if bold {
                Weight::BOLD
            } else {
                Weight::NORMAL
            })
            .stretch(Stretch::Normal)
            .style(Style::Normal);
        let text_color = TextColor::rgba(
            (layout.color[0].clamp(0.0, 1.0) * 255.0) as u8,
            (layout.color[1].clamp(0.0, 1.0) * 255.0) as u8,
            (layout.color[2].clamp(0.0, 1.0) * 255.0) as u8,
            (layout.color[3].clamp(0.0, 1.0) * 255.0) as u8,
        );

        let mut pixels = Vec::<(i32, i32, [u8; 4])>::new();
        let mut min_x = i32::MAX;
        let mut min_y = i32::MAX;
        let mut max_x = i32::MIN;
        let mut max_y = i32::MIN;

        {
            let runtime = &mut *self
                .font_runtime
                .lock()
                .expect("UIRenderer font runtime poisoned");
            let mut text_buffer = runtime.text_buffer.borrow_with(&mut runtime.font_system);
            text_buffer.set_metrics(metrics);
            text_buffer.set_size(canvas_width as f32, canvas_height as f32);
            text_buffer.set_wrap(wrap_mode);
            text_buffer.set_text(&text, attrs, Shaping::Advanced);
            text_buffer.shape_until_scroll();
            text_buffer.draw(
                &mut runtime.swash_cache,
                text_color,
                |x, y, _w, _h, color| {
                    let rgba = color.as_rgba();
                    if rgba[3] == 0 {
                        return;
                    }
                    min_x = min_x.min(x);
                    min_y = min_y.min(y);
                    max_x = max_x.max(x);
                    max_y = max_y.max(y);
                    pixels.push((x, y, rgba));
                },
            );
        }

        if pixels.is_empty() {
            return Ok(());
        }

        let text_width = (max_x - min_x + 1).max(1);
        let text_height = (max_y - min_y + 1).max(1);

        // Alignment is computed against the authored bounds (C++
        // `text_x += (width / 2) - (text_width / 2)`), so overflowing text
        // spills out of the rect instead of being squeezed or clipped.
        let bounds_width = layout.bounds.width.ceil() as i32;
        let bounds_height = layout.bounds.height.ceil() as i32;
        let x_offset = match layout.alignment {
            TextAlignment::Left => 0,
            TextAlignment::Center => (bounds_width - text_width) / 2,
            TextAlignment::Right => bounds_width - text_width,
            TextAlignment::Justify => 0,
        };
        let y_offset = match layout.vertical_alignment {
            VerticalAlignment::Top => 0,
            VerticalAlignment::Middle => (bounds_height - text_height) / 2,
            VerticalAlignment::Bottom => bounds_height - text_height,
        };

        // The atlas canvas covers bounds ∪ placed-glyph-extent, like C++
        // Store_GDI_Char which measures each glyph's real bitmap
        // (GetTextExtentPoint32W) instead of guessing an advance. Wrapped
        // layouts stay bounds-fitted — wrapping is their contract.
        let (canvas_origin_x, canvas_origin_y, quad_width, quad_height) = if wrap_mode == Wrap::Word
        {
            (0, 0, canvas_width, canvas_height)
        } else {
            let left = x_offset.min(0);
            let top = y_offset.min(0);
            let right = (x_offset + text_width).max(canvas_width as i32);
            let bottom = (y_offset + text_height).max(canvas_height as i32);
            let width = (right - left).clamp(1, 2048);
            let height = (bottom - top).clamp(1, 512);
            (
                left,
                top,
                width as u32,
                height as u32,
            )
        };
        let canvas_width = quad_width;
        let canvas_height = quad_height;
        let mut canvas = vec![0u8; canvas_width as usize * canvas_height as usize * 4];

        for (x, y, src) in pixels {
            let dst_x = x - min_x + x_offset - canvas_origin_x;
            let dst_y = y - min_y + y_offset - canvas_origin_y;
            if dst_x < 0
                || dst_y < 0
                || dst_x >= canvas_width as i32
                || dst_y >= canvas_height as i32
            {
                continue;
            }

            let pixel_index = (dst_y as usize * canvas_width as usize + dst_x as usize) * 4;
            let dst = &mut canvas[pixel_index..pixel_index + 4];
            let src_a = src[3] as f32 / 255.0;
            let dst_a = dst[3] as f32 / 255.0;
            let out_a = src_a + dst_a * (1.0 - src_a);
            if out_a <= f32::EPSILON {
                continue;
            }
            for channel in 0..3 {
                let src_c = src[channel] as f32 / 255.0;
                let dst_c = dst[channel] as f32 / 255.0;
                let out_c = (src_c * src_a + dst_c * dst_a * (1.0 - src_a)) / out_a;
                dst[channel] = (out_c * 255.0).clamp(0.0, 255.0) as u8;
            }
            dst[3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
        }

        let texture = self.create_texture_from_rgba(canvas_width, canvas_height, &canvas);
        let quad = UIRect::new(
            layout.bounds.x + canvas_origin_x as f32,
            layout.bounds.y + canvas_origin_y as f32,
            canvas_width as f32,
            canvas_height as f32,
        );
        if self.text_texture_cache.len() >= 256 {
            self.text_texture_cache.clear();
        }
        self.text_texture_cache
            .insert(cache_key, (texture.clone(), quad));
        self.draw_textured_rect(quad, texture, [1.0, 1.0, 1.0, 1.0], None, z_order);
        Ok(())
    }

    /// Render all UI elements to the given render pass
    pub fn render(&mut self, render_pass: &mut RenderPass) -> Result<()> {
        // Update uniform buffer
        let uniforms = UIUniforms {
            view_projection: self.view_projection.to_cols_array_2d(),
            screen_size: [self.screen_size.0 as f32, self.screen_size.1 as f32],
            time: self.current_time,
            _padding: 0.0,
        };

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[uniforms]));

        // Sort draw commands by z-order
        self.frame_buffers.draw_commands.sort_by(|a, b| {
            a.z_order
                .partial_cmp(&b.z_order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Batch and render commands
        let mut stats = RenderStats::default();
        let start_time = std::time::Instant::now();

        // Combine all vertex and index data and track per-command index spans.
        let mut vertex_offset = 0u32;
        let mut command_ranges: Vec<(u32, u32)> =
            Vec::with_capacity(self.frame_buffers.draw_commands.len());
        for command in &self.frame_buffers.draw_commands {
            if self.frame_buffers.vertex_data.len() + command.vertices.len() > Self::MAX_UI_VERTICES
            {
                log::error!(
                    "UI vertex flood ({} + {} > {}); dropping remaining commands",
                    self.frame_buffers.vertex_data.len(),
                    command.vertices.len(),
                    Self::MAX_UI_VERTICES
                );
                break;
            }
            let base_vertex = vertex_offset;
            let start = self.frame_buffers.index_data.len() as u32;

            self.frame_buffers
                .vertex_data
                .extend_from_slice(&command.vertices);
            vertex_offset += command.vertices.len() as u32;
            for &index in &command.indices {
                self.frame_buffers.index_data.push(base_vertex + index);
            }

            let count = command.indices.len() as u32;
            command_ranges.push((start, count));
            stats.vertices_rendered += command.vertices.len() as u32;
            stats.triangles_rendered += count / 3;
        }

        if !self.frame_buffers.vertex_data.is_empty() {
            self.ensure_geometry_buffer_capacity();

            // Upload vertex data
            self.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&self.frame_buffers.vertex_data),
            );

            // Upload index data
            self.queue.write_buffer(
                &self.index_buffer,
                0,
                bytemuck::cast_slice(&self.frame_buffers.index_data),
            );

            // Render batched geometry
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.default_texture_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            // Draw each command with exact index ranges (correct for non-quad primitives).
            // Keep a semantic pipeline tag rather than comparing raw pointers.
            #[derive(Clone, Copy, PartialEq, Eq)]
            enum PipelineKind {
                Solid,
                Textured,
                TexturedAdditive,
                TexturedGrayscale,
            }
            let mut current_pipeline: Option<PipelineKind> = None;
            let mut current_texture: Option<Arc<TextureView>> = None;

            for (command, (start, count)) in
                self.frame_buffers.draw_commands.iter().zip(command_ranges)
            {
                let (pipeline, pipeline_kind) =
                    match (command.texture.is_some(), command.blend_mode) {
                        (true, UIBlendMode::Additive) => (
                            &self.textured_additive_pipeline,
                            PipelineKind::TexturedAdditive,
                        ),
                        (true, UIBlendMode::Grayscale) => (
                            &self.textured_grayscale_pipeline,
                            PipelineKind::TexturedGrayscale,
                        ),
                        (true, _) => (&self.textured_pipeline, PipelineKind::Textured),
                        (false, _) => (&self.solid_pipeline, PipelineKind::Solid),
                    };
                if current_pipeline != Some(pipeline_kind) {
                    render_pass.set_pipeline(pipeline);
                    current_pipeline = Some(pipeline_kind);
                }

                match &command.texture {
                    Some(texture) => {
                        let texture_changed = current_texture
                            .as_ref()
                            .is_none_or(|current| !Arc::ptr_eq(current, texture));
                        if texture_changed {
                            let texture_bind_group =
                                self.device.create_bind_group(&BindGroupDescriptor {
                                    label: Some("UI Texture Bind Group"),
                                    layout: &self.texture_bind_group_layout,
                                    entries: &[
                                        BindGroupEntry {
                                            binding: 0,
                                            resource: wgpu::BindingResource::TextureView(texture),
                                        },
                                        BindGroupEntry {
                                            binding: 1,
                                            resource: wgpu::BindingResource::Sampler(
                                                &self.linear_sampler,
                                            ),
                                        },
                                    ],
                                });
                            render_pass.set_bind_group(1, &texture_bind_group, &[]);
                            current_texture = Some(texture.clone());
                            stats.texture_switches += 1;
                        }
                    }
                    None => {
                        if current_texture.is_some() {
                            render_pass.set_bind_group(1, &self.default_texture_bind_group, &[]);
                            current_texture = None;
                        }
                    }
                }

                if let Some(scissor) = command.scissor_rect {
                    let x = scissor.x.max(0.0).floor() as u32;
                    let y = scissor.y.max(0.0).floor() as u32;
                    // A scissor outside the render target is a skipped draw,
                    // never a fatal: wgpu rejects rects exceeding the target
                    // (stale 800x600 layout coords against a smaller window).
                    if x >= self.screen_size.0 || y >= self.screen_size.1 {
                        continue;
                    }
                    let max_w = self.screen_size.0.saturating_sub(x);
                    let max_h = self.screen_size.1.saturating_sub(y);
                    let w = scissor.width.max(0.0).ceil() as u32;
                    let h = scissor.height.max(0.0).ceil() as u32;
                    let w = w.min(max_w).max(1);
                    let h = h.min(max_h).max(1);
                    render_pass.set_scissor_rect(x, y, w, h);
                } else {
                    render_pass.set_scissor_rect(
                        0,
                        0,
                        self.screen_size.0.max(1),
                        self.screen_size.1.max(1),
                    );
                }

                if count > 0 {
                    render_pass.draw_indexed(start..start + count, 0, 0..1);
                    stats.draw_calls += 1;
                }
            }
        }

        stats.render_time_ms = start_time.elapsed().as_secs_f32() * 1000.0;
        self.last_frame_stats = stats;

        Ok(())
    }

    /// End the current frame
    pub fn end_frame(&mut self) {
        self.frame_buffers.end();
    }

    /// Current screen size in pixels.
    pub fn screen_size(&self) -> (u32, u32) {
        self.screen_size
    }

    /// Access the renderer device.
    pub fn device(&self) -> &Device {
        self.device.as_ref()
    }

    /// Access the renderer queue.
    pub fn queue(&self) -> &Queue {
        self.queue.as_ref()
    }

    /// Load a font from file
    pub fn load_font(&mut self, name: &str, font_data: &[u8]) -> Result<()> {
        let font = Font::from_bytes(font_data, FontSettings::default())
            .map_err(|e| UIRendererError::FontError(format!("Failed to load font: {}", e)))?;

        self.font_cache.insert(name.to_string(), font);
        Ok(())
    }

    /// Get rendering statistics from the last frame
    pub fn get_stats(&self) -> &RenderStats {
        &self.last_frame_stats
    }

    /// Number of queued draw commands for the current frame before render().
    pub fn queued_draw_command_count(&self) -> usize {
        self.frame_buffers.draw_commands.len()
    }

    // Convenience methods for backward compatibility

    /// Draw a filled rectangle with scissor support (convenience wrapper)
    pub fn draw_rect_with_scissor(
        &mut self,
        rect: UIRect,
        color: [f32; 4],
        scissor: Option<UIRect>,
    ) -> Result<()> {
        // Modify the last draw command if we just added one
        self.draw_rect(rect, color, 0.0);
        if let Some(ref mut cmd) = self.frame_buffers.draw_commands.last_mut() {
            cmd.scissor_rect = scissor;
        }
        Ok(())
    }

    /// Draw text at a position (convenience wrapper). Default game font
    /// (Arial regular — the C++ fallback family, render2dsentence.cpp:1481).
    pub fn draw_text_simple(
        &mut self,
        text: &str,
        position: Vec2,
        font_size: f32,
        color: [f32; 4],
    ) -> Result<()> {
        self.draw_text_simple_named(text, position, font_size, color, "Arial", false)
    }

    /// Draw text with an explicit game font. `font_size` is the POINT size;
    /// it is converted to the GDI pixel em here (MulDiv(point, 96, 72),
    /// render2dsentence.cpp:1492) so measure and raster agree.
    pub fn draw_text_simple_named(
        &mut self,
        text: &str,
        position: Vec2,
        point_size: f32,
        color: [f32; 4],
        font_name: &str,
        bold: bool,
    ) -> Result<()> {
        let px = super::font::font_pixel_size(point_size.max(1.0) as i32) as f32;
        let char_width = px * 0.6;
        let text_width = text.len() as f32 * char_width;

        let layout = TextLayout {
            text: text.to_string(),
            font_size: px,
            color,
            bounds: UIRect::new(position.x, position.y, text_width, (px * 1.2).ceil()),
            alignment: TextAlignment::Left,
            vertical_alignment: VerticalAlignment::Top,
            word_wrap: false,
            single_line: true,
        };

        self.draw_text_with_font(&layout, font_name, bold, 0.0)
    }

    /// Draw text at a position with scissor support.
    pub fn draw_text_simple_with_scissor(
        &mut self,
        text: &str,
        position: Vec2,
        font_size: f32,
        color: [f32; 4],
        scissor: UIRect,
    ) -> Result<()> {
        self.draw_text_simple(text, position, font_size, color)?;
        if let Some(cmd) = self.frame_buffers.draw_commands.last_mut() {
            cmd.scissor_rect = Some(scissor);
        }
        Ok(())
    }

    /// Draw text with an explicit game font and scissor support.
    pub fn draw_text_simple_named_with_scissor(
        &mut self,
        text: &str,
        position: Vec2,
        point_size: f32,
        color: [f32; 4],
        font_name: &str,
        bold: bool,
        scissor: UIRect,
    ) -> Result<()> {
        self.draw_text_simple_named(text, position, point_size, color, font_name, bold)?;
        if let Some(cmd) = self.frame_buffers.draw_commands.last_mut() {
            cmd.scissor_rect = Some(scissor);
        }
        Ok(())
    }

    /// Draw a rectangle outline with scissor support (convenience wrapper)
    pub fn draw_rect_outline_with_scissor(
        &mut self,
        rect: UIRect,
        thickness: f32,
        color: [f32; 4],
        scissor: Option<UIRect>,
    ) -> Result<()> {
        self.draw_rect_outline(rect, thickness, color, 0.0);
        // Apply scissor to the last 4 commands (the 4 edges)
        let len = self.frame_buffers.draw_commands.len();
        if len >= 4 {
            for cmd in &mut self.frame_buffers.draw_commands[len - 4..] {
                cmd.scissor_rect = scissor;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn queued_command() -> UIDrawCommand {
        UIDrawCommand {
            vertices: vec![UIVertex {
                position: [0.0, 0.0, 0.0],
                tex_coord: [0.0, 0.0],
                color: [1.0, 1.0, 1.0, 1.0],
            }],
            indices: vec![0],
            texture: None,
            blend_mode: UIBlendMode::Alpha,
            scissor_rect: None,
            z_order: 0.0,
        }
    }

    #[test]
    fn overlay_frame_preserves_presentation_commands_and_resets_render_scratch() {
        let mut buffers = UIFrameBuffers::default();
        buffers.draw_commands.push(queued_command());
        buffers.vertex_data.push(UIVertex {
            position: [1.0, 1.0, 0.0],
            tex_coord: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        });
        buffers.index_data.push(0);
        buffers.instance_data.push(UIInstance {
            transform: [[0.0; 4]; 4],
            color_modifier: [1.0, 1.0, 1.0, 1.0],
            texture_rect: [0.0, 0.0, 1.0, 1.0],
        });

        buffers.begin_overlay();

        assert!(buffers.open);
        assert_eq!(buffers.draw_commands.len(), 1);
        assert!(buffers.vertex_data.is_empty());
        assert!(buffers.index_data.is_empty());
        assert!(buffers.instance_data.is_empty());
    }

    #[test]
    fn overlay_frame_discards_an_abandoned_open_frame() {
        let mut buffers = UIFrameBuffers::default();
        buffers.begin_fresh();
        buffers.draw_commands.push(queued_command());

        buffers.begin_overlay();

        assert!(buffers.open);
        assert!(
            buffers.draw_commands.is_empty(),
            "an already-open frame is stale rather than new presentation work"
        );
    }

    #[test]
    fn frame_end_closes_and_clears_every_buffer() {
        let mut buffers = UIFrameBuffers::default();
        buffers.begin_fresh();
        buffers.draw_commands.push(queued_command());
        buffers.vertex_data.push(UIVertex {
            position: [1.0, 1.0, 0.0],
            tex_coord: [1.0, 1.0],
            color: [1.0, 1.0, 1.0, 1.0],
        });
        buffers.index_data.push(0);

        buffers.end();

        assert!(!buffers.open);
        assert!(buffers.draw_commands.is_empty());
        assert!(buffers.vertex_data.is_empty());
        assert!(buffers.index_data.is_empty());
        assert!(buffers.instance_data.is_empty());
    }
}
