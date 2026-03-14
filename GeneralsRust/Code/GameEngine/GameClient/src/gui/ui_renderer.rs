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
use std::sync::Arc;
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
    pub font_size: f32,
    pub color: [f32; 4],
    pub bounds: UIRect,
    pub alignment: TextAlignment,
    pub vertical_alignment: VerticalAlignment,
    pub word_wrap: bool,
    pub single_line: bool,
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

    // Render pipelines
    solid_pipeline: RenderPipeline,
    textured_pipeline: RenderPipeline,
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

    // Text rendering system
    font_system: FontSystem,
    swash_cache: SwashCache,
    text_buffer: TextBuffer,
    font_cache: HashMap<String, Font>,

    // Textures and samplers
    default_texture: Arc<TextureView>,
    linear_sampler: Sampler,
    nearest_sampler: Sampler,

    // Rendering state
    screen_size: (u32, u32),
    view_projection: Mat4,
    current_time: f32,

    // Command batching
    draw_commands: Vec<UIDrawCommand>,
    vertex_data: Vec<UIVertex>,
    index_data: Vec<u32>,
    instance_data: Vec<UIInstance>,

    // Performance statistics
    last_frame_stats: RenderStats,
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

        Ok(Self {
            device,
            queue,
            solid_pipeline,
            textured_pipeline,
            text_pipeline,
            ui_shader,
            text_shader,
            uniform_bind_group_layout,
            texture_bind_group_layout,
            vertex_buffer,
            index_buffer,
            instance_buffer,
            uniform_buffer,
            vertex_capacity: 65536,
            index_capacity: 98304,
            uniform_bind_group,
            font_system,
            swash_cache: SwashCache::new(),
            text_buffer,
            font_cache: HashMap::new(),
            default_texture: default_texture_view,
            linear_sampler,
            nearest_sampler,
            screen_size: (800, 600),
            view_projection: Mat4::IDENTITY,
            current_time: 0.0,
            draw_commands: Vec::new(),
            vertex_data: Vec::new(),
            index_data: Vec::new(),
            instance_data: Vec::new(),
            last_frame_stats: RenderStats::default(),
        })
    }

    fn ensure_geometry_buffer_capacity(&mut self) {
        let required_vertices = self.vertex_data.len();
        if required_vertices > self.vertex_capacity {
            let new_capacity = required_vertices
                .next_power_of_two()
                .max(self.vertex_capacity * 2);
            self.vertex_buffer = self.device.create_buffer(&BufferDescriptor {
                label: Some("UI Vertex Buffer"),
                size: (std::mem::size_of::<UIVertex>() * new_capacity) as u64,
                usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            });
            self.vertex_capacity = new_capacity;
        }

        let required_indices = self.index_data.len();
        if required_indices > self.index_capacity {
            let new_capacity = required_indices
                .next_power_of_two()
                .max(self.index_capacity * 2);
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
        self.draw_commands.clear();
        self.vertex_data.clear();
        self.index_data.clear();
        self.instance_data.clear();
    }

    /// Add a rectangle draw command
    pub fn draw_rect(&mut self, rect: UIRect, color: [f32; 4], z_order: f32) {
        let vertices = vec![
            UIVertex {
                position: [rect.x, rect.y, z_order],
                tex_coord: [0.0, 0.0],
                color,
            },
            UIVertex {
                position: [rect.x + rect.width, rect.y, z_order],
                tex_coord: [1.0, 0.0],
                color,
            },
            UIVertex {
                position: [rect.x + rect.width, rect.y + rect.height, z_order],
                tex_coord: [1.0, 1.0],
                color,
            },
            UIVertex {
                position: [rect.x, rect.y + rect.height, z_order],
                tex_coord: [0.0, 1.0],
                color,
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        self.draw_commands.push(UIDrawCommand {
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

        self.draw_commands.push(UIDrawCommand {
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
                color,
            },
            UIVertex {
                position: [p1.x, p1.y, z_order],
                tex_coord: [0.0, 1.0],
                color,
            },
            UIVertex {
                position: [p2.x, p2.y, z_order],
                tex_coord: [1.0, 1.0],
                color,
            },
            UIVertex {
                position: [p3.x, p3.y, z_order],
                tex_coord: [1.0, 0.0],
                color,
            },
        ];

        let indices = vec![0, 1, 2, 0, 2, 3];

        self.draw_commands.push(UIDrawCommand {
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
        let indices = vec![0, 1, 2];
        self.draw_commands.push(UIDrawCommand {
            vertices,
            indices,
            texture: Some(self.default_texture.clone()),
            blend_mode: UIBlendMode::Alpha,
            scissor_rect: None,
            z_order,
        });
    }

    /// Add a text draw command
    pub fn draw_text(&mut self, layout: &TextLayout, z_order: f32) -> Result<()> {
        if layout.text.is_empty() || layout.bounds.width <= 0.0 || layout.bounds.height <= 0.0 {
            return Ok(());
        }

        let canvas_width = layout.bounds.width.ceil().max(1.0) as u32;
        let canvas_height = layout.bounds.height.ceil().max(1.0) as u32;
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

        let attrs = Attrs::new()
            .family(Family::SansSerif)
            .weight(Weight::NORMAL)
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
            let mut text_buffer = self.text_buffer.borrow_with(&mut self.font_system);
            text_buffer.set_metrics(metrics);
            text_buffer.set_size(canvas_width as f32, canvas_height as f32);
            text_buffer.set_wrap(wrap_mode);
            text_buffer.set_text(&text, attrs, Shaping::Advanced);
            text_buffer.shape_until_scroll();
            text_buffer.draw(&mut self.swash_cache, text_color, |x, y, _w, _h, color| {
                let rgba = color.as_rgba();
                if rgba[3] == 0 {
                    return;
                }
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
                pixels.push((x, y, rgba));
            });
        }

        if pixels.is_empty() {
            return Ok(());
        }

        let text_width = (max_x - min_x + 1).max(1);
        let text_height = (max_y - min_y + 1).max(1);
        let max_x_offset = (canvas_width as i32 - text_width).max(0);
        let max_y_offset = (canvas_height as i32 - text_height).max(0);
        let x_offset = match layout.alignment {
            TextAlignment::Left => 0,
            TextAlignment::Center => max_x_offset / 2,
            TextAlignment::Right => max_x_offset,
            TextAlignment::Justify => 0,
        };
        let y_offset = match layout.vertical_alignment {
            VerticalAlignment::Top => 0,
            VerticalAlignment::Middle => max_y_offset / 2,
            VerticalAlignment::Bottom => max_y_offset,
        };

        for (x, y, src) in pixels {
            let dst_x = x - min_x + x_offset;
            let dst_y = y - min_y + y_offset;
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
        self.draw_textured_rect(layout.bounds, texture, [1.0, 1.0, 1.0, 1.0], None, z_order);

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
        self.draw_commands.sort_by(|a, b| {
            a.z_order
                .partial_cmp(&b.z_order)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        // Batch and render commands
        let mut stats = RenderStats::default();
        let start_time = std::time::Instant::now();

        // Combine all vertex and index data and track per-command index spans.
        let mut vertex_offset = 0u32;
        let mut command_ranges: Vec<(u32, u32)> = Vec::with_capacity(self.draw_commands.len());
        for command in &self.draw_commands {
            let base_vertex = vertex_offset;
            let start = self.index_data.len() as u32;

            self.vertex_data.extend_from_slice(&command.vertices);
            vertex_offset += command.vertices.len() as u32;
            for &index in &command.indices {
                self.index_data.push(base_vertex + index);
            }

            let count = command.indices.len() as u32;
            command_ranges.push((start, count));
            stats.vertices_rendered += command.vertices.len() as u32;
            stats.triangles_rendered += count / 3;
        }

        if !self.vertex_data.is_empty() {
            self.ensure_geometry_buffer_capacity();

            // Upload vertex data
            self.queue.write_buffer(
                &self.vertex_buffer,
                0,
                bytemuck::cast_slice(&self.vertex_data),
            );

            // Upload index data
            self.queue.write_buffer(
                &self.index_buffer,
                0,
                bytemuck::cast_slice(&self.index_data),
            );

            // Render batched geometry
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

            // Draw each command with exact index ranges (correct for non-quad primitives).
            let mut current_pipeline: Option<*const RenderPipeline> = None;
            let mut current_texture: Option<Arc<TextureView>> = None;

            for (command, (start, count)) in
                self.draw_commands.iter().zip(command_ranges.into_iter())
            {
                let pipeline = if command.texture.is_some() {
                    &self.textured_pipeline
                } else {
                    &self.solid_pipeline
                };
                let pipeline_ptr = pipeline as *const _;
                if current_pipeline != Some(pipeline_ptr) {
                    render_pass.set_pipeline(pipeline);
                    current_pipeline = Some(pipeline_ptr);
                }

                match &command.texture {
                    Some(texture) => {
                        let texture_changed = current_texture
                            .as_ref()
                            .map_or(true, |current| !Arc::ptr_eq(current, texture));
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
                        current_texture = None;
                    }
                }

                if let Some(scissor) = command.scissor_rect {
                    let x = scissor.x.max(0.0).floor() as u32;
                    let y = scissor.y.max(0.0).floor() as u32;
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
        // Clear frame data
        self.draw_commands.clear();
        self.vertex_data.clear();
        self.index_data.clear();
        self.instance_data.clear();
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
        self.draw_commands.len()
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
        if let Some(ref mut cmd) = self.draw_commands.last_mut() {
            cmd.scissor_rect = scissor;
        }
        Ok(())
    }

    /// Draw text at a position (convenience wrapper)
    pub fn draw_text_simple(
        &mut self,
        text: &str,
        position: Vec2,
        font_size: f32,
        color: [f32; 4],
    ) -> Result<()> {
        // Create a simple text layout
        let char_width = font_size * 0.6;
        let text_width = text.len() as f32 * char_width;

        let layout = TextLayout {
            text: text.to_string(),
            font_size,
            color,
            bounds: UIRect::new(position.x, position.y, text_width, font_size * 1.2),
            alignment: TextAlignment::Left,
            vertical_alignment: VerticalAlignment::Top,
            word_wrap: false,
            single_line: true,
        };

        self.draw_text(&layout, 0.0)
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
        if let Some(cmd) = self.draw_commands.last_mut() {
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
        let len = self.draw_commands.len();
        if len >= 4 {
            for cmd in &mut self.draw_commands[len - 4..] {
                cmd.scissor_rect = scissor;
            }
        }
        Ok(())
    }
}

// Implement Send and Sync for UIRenderer (needed for multi-threading)
unsafe impl Send for UIRenderer {}
unsafe impl Sync for UIRenderer {}
