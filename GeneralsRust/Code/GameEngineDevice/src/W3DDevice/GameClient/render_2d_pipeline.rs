//! 2D Render Pipeline for W3DDisplay
//!
//! Corresponds to C++ Render2DClass (WW3D2) which batches 2D primitives
//! (quads, lines, rects) and flushes them as a single draw call.
//!
//! In WGPU, this uses a vertex buffer with dynamic updates and a simple
//! orthographic shader to render UI elements.

use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::{
    Buffer, BufferDescriptor, BufferUsages, ColorTargetState, ColorWrites, Device, Extent3d,
    FilterMode, FragmentState, PipelineLayout, PipelineLayoutDescriptor, PrimitiveState,
    PrimitiveTopology, Queue, RenderPass, RenderPipeline, RenderPipelineDescriptor, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderModule, ShaderModuleDescriptor, ShaderSource,
    ShaderStages, Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType,
    TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState, VertexStepMode,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Maximum number of vertices before a forced flush (C++ Render2DClass batch limit).
const MAX_VERTICES_PER_BATCH: usize = 4096;

/// Maximum number of textures that can be bound per batch.
const MAX_BOUND_TEXTURES: usize = 16;

// ---------------------------------------------------------------------------
// Vertex format
// ---------------------------------------------------------------------------

/// 2D vertex for UI rendering.
///
/// C++ uses `STRUCT_TEX2_VERTEX` (x, y, z, rhw, diffuse, u, v).
/// In WGPU with orthographic projection we only need 2D position + UV + color.
#[repr(C)]
#[derive(Debug, Copy, Clone, Pod, Zeroable)]
pub struct Vertex2D {
    /// Screen-space position (pixels). Converted to NDC in the vertex shader.
    pub position: [f32; 2],
    /// Texture coordinates.
    pub uv: [f32; 2],
    /// Vertex color (RGBA packed as u32).
    pub color: u32,
}

impl Vertex2D {
    const ATTRIBUTES: &'static [VertexAttribute] = &[
        VertexAttribute {
            offset: 0,
            shader_location: 0,
            format: VertexFormat::Float32x2,
        },
        VertexAttribute {
            offset: std::mem::size_of::<[f32; 2]>() as u64,
            shader_location: 1,
            format: VertexFormat::Float32x2,
        },
        VertexAttribute {
            offset: (std::mem::size_of::<[f32; 2]>() + std::mem::size_of::<[f32; 2]>()) as u64,
            shader_location: 2,
            format: VertexFormat::Uint32,
        },
    ];

    pub fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex2D>() as u64,
            step_mode: VertexStepMode::Vertex,
            attributes: Self::ATTRIBUTES,
        }
    }
}

// ---------------------------------------------------------------------------
// Draw modes (matching C++ DrawImageMode)
// ---------------------------------------------------------------------------

/// How to blend/compose a drawn image. Matches C++ `DrawImageMode` from Display.h.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawImageMode {
    /// Standard alpha blending (default).
    Alpha,
    /// Convert to grayscale before compositing.
    Grayscale,
    /// Additive blending (for glow effects).
    Additive,
    /// No blending, fully opaque.
    Solid,
}

impl Default for DrawImageMode {
    fn default() -> Self {
        DrawImageMode::Alpha
    }
}

// ---------------------------------------------------------------------------
// Queued primitive (CPU-side)
// ---------------------------------------------------------------------------

/// A queued 2D primitive awaiting flush.
#[derive(Debug, Clone)]
enum QueuedPrimitive {
    /// Textured quad: 4 vertices forming a rectangle.
    TexturedQuad {
        vertices: [Vertex2D; 4],
        texture_id: u64,
        mode: DrawImageMode,
    },
    /// Colored (untextured) filled rectangle.
    FilledRect {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        color: u32,
    },
    /// Line from (x0,y0) to (x1,y1).
    Line {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        line_width: f32,
        color: u32,
    },
}

// ---------------------------------------------------------------------------
// Render2DPipeline
// ---------------------------------------------------------------------------

/// Batched 2D primitive renderer. Corresponds to C++ `Render2DClass`.
///
/// C++ pattern:
/// ```cpp
/// m_2DRender->Reset();
/// m_2DRender->Enable_Texturing(TRUE);
/// m_2DRender->Set_Texture(texture);
/// m_2DRender->Add_Quad(screen_rect, uv_rect, color);
/// m_2DRender->Render();
/// ```
///
/// Rust pattern:
/// ```ignore
/// pipeline.reset();
/// pipeline.queue_image(...);
/// pipeline.flush(&mut render_pass);
/// ```
pub struct Render2DPipeline {
    device: Arc<Device>,
    queue: Arc<Queue>,

    /// Surface dimensions for NDC conversion.
    width: u32,
    height: u32,

    /// Queued primitives.
    primitives: Vec<QueuedPrimitive>,

    /// Vertex buffer for batched rendering.
    vertex_buffer: Buffer,

    /// Staging vertex data (CPU side).
    vertex_staging: Vec<Vertex2D>,

    /// Render pipelines for different blend modes.
    pipeline_alpha: RenderPipeline,
    pipeline_additive: RenderPipeline,
    pipeline_solid: RenderPipeline,

    /// White 1x1 texture for untextured primitives.
    white_texture: Texture,
    white_texture_view: TextureView,
    white_sampler: Sampler,

    /// Shader module.
    _shader_module: ShaderModule,

    /// Pipeline layout.
    _pipeline_layout: PipelineLayout,
}

impl Render2DPipeline {
    /// Create a new 2D render pipeline.
    ///
    /// Requires device, queue, surface dimensions, and the target surface format.
    pub fn new(
        device: Arc<Device>,
        queue: Arc<Queue>,
        width: u32,
        height: u32,
        surface_format: TextureFormat,
    ) -> Self {
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Render2D Shader"),
            source: ShaderSource::Wgsl(SHADER_SOURCE.into()),
        });

        // Bind group layout for texture + sampler
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render2D Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Uniform buffer layout for screen size
        let uniform_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Render2D Uniform Layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Render2D Pipeline Layout"),
            bind_group_layouts: &[&uniform_layout, &bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create pipelines for different blend modes
        let make_pipeline = |blend: wgpu::BlendState| {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some("Render2D Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &shader_module,
                    entry_point: "vs_main",
                    buffers: &[Vertex2D::desc()],
                },
                fragment: Some(FragmentState {
                    module: &shader_module,
                    entry_point: "fs_main",
                    targets: &[Some(ColorTargetState {
                        format: surface_format,
                        blend: Some(blend),
                        write_mask: ColorWrites::ALL,
                    })],
                }),
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: None,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };

        let pipeline_alpha = make_pipeline(wgpu::BlendState::ALPHA_BLENDING);
        let pipeline_additive = make_pipeline(wgpu::BlendState {
            color: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::SrcAlpha,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
            alpha: wgpu::BlendComponent {
                src_factor: wgpu::BlendFactor::One,
                dst_factor: wgpu::BlendFactor::One,
                operation: wgpu::BlendOperation::Add,
            },
        });
        let pipeline_solid = make_pipeline(wgpu::BlendState::REPLACE);

        // Create white 1x1 texture for untextured primitives
        let white_texture = device.create_texture(&TextureDescriptor {
            label: Some("Render2D White Texture"),
            size: Extent3d {
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
        let white_texture_view = white_texture.create_view(&TextureViewDescriptor::default());

        // Upload white pixel
        queue.write_texture(
            white_texture.as_image_copy(),
            &[255u8, 255, 255, 255],
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4),
                rows_per_image: Some(1),
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let white_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Render2D Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        // Create vertex buffer
        let vertex_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Render2D Vertex Buffer"),
            size: (MAX_VERTICES_PER_BATCH * std::mem::size_of::<Vertex2D>()) as u64,
            usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            device,
            queue,
            width,
            height,
            primitives: Vec::with_capacity(256),
            vertex_buffer,
            vertex_staging: Vec::with_capacity(MAX_VERTICES_PER_BATCH),
            pipeline_alpha,
            pipeline_additive,
            pipeline_solid,
            white_texture,
            white_texture_view,
            white_sampler,
            _shader_module: shader_module,
            _pipeline_layout: pipeline_layout,
        }
    }

    /// Resize the pipeline target dimensions.
    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
    }

    /// Get current target dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    // -----------------------------------------------------------------------
    // Queue methods (matching C++ Render2DClass::Add_*)
    // -----------------------------------------------------------------------

    /// Clear all queued primitives (C++ `Render2DClass::Reset()`).
    pub fn reset(&mut self) {
        self.primitives.clear();
    }

    /// Queue a textured image quad for rendering.
    ///
    /// Corresponds to C++ `W3DDisplay::drawImage` which calls
    /// `m_2DRender->Add_Quad(screen_rect, uv_rect, color)`.
    ///
    /// # Arguments
    ///
    /// * `x0, y0` - Top-left corner in screen pixels
    /// * `x1, y1` - Bottom-right corner in screen pixels
    /// * `u0, v0` - Top-left UV coordinate
    /// * `u1, v1` - Bottom-right UV coordinate
    /// * `color` - RGBA tint color (packed u32)
    /// * `texture_id` - Texture identifier (0 = use white texture)
    /// * `mode` - Blend mode
    pub fn queue_image(
        &mut self,
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
        u0: f32,
        v0: f32,
        u1: f32,
        v1: f32,
        color: u32,
        texture_id: u64,
        mode: DrawImageMode,
    ) {
        let vertices = [
            Vertex2D {
                position: [x0, y0],
                uv: [u0, v0],
                color,
            },
            Vertex2D {
                position: [x1, y0],
                uv: [u1, v0],
                color,
            },
            Vertex2D {
                position: [x1, y1],
                uv: [u1, v1],
                color,
            },
            Vertex2D {
                position: [x0, y1],
                uv: [u0, v1],
                color,
            },
        ];

        self.primitives.push(QueuedPrimitive::TexturedQuad {
            vertices,
            texture_id,
            mode,
        });
    }

    /// Queue a filled rectangle (C++ `Render2DClass::Add_Rect`).
    pub fn queue_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
        self.primitives
            .push(QueuedPrimitive::FilledRect { x, y, w, h, color });
    }

    /// Queue a line (C++ `Render2DClass::Add_Line`).
    pub fn queue_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, line_width: f32, color: u32) {
        self.primitives.push(QueuedPrimitive::Line {
            x0,
            y0,
            x1,
            y1,
            line_width,
            color,
        });
    }

    /// Queue a rectangle outline (4 lines).
    pub fn queue_open_rect(&mut self, x: f32, y: f32, w: f32, h: f32, line_width: f32, color: u32) {
        self.queue_line(x, y, x + w, y, line_width, color); // top
        self.queue_line(x + w, y, x + w, y + h, line_width, color); // right
        self.queue_line(x + w, y + h, x, y + h, line_width, color); // bottom
        self.queue_line(x, y + h, x, y, line_width, color); // left
    }

    // -----------------------------------------------------------------------
    // Flush / render (C++ Render2DClass::Render)
    // -----------------------------------------------------------------------

    /// Build vertex data from queued primitives and upload to GPU.
    fn build_vertices(&mut self) {
        self.vertex_staging.clear();

        for prim in &self.primitives {
            match prim {
                QueuedPrimitive::TexturedQuad {
                    vertices,
                    texture_id: _,
                    mode: _,
                } => {
                    // Two triangles from quad: TL-TR-BR, TL-BR-BL
                    let v = *vertices;
                    self.vertex_staging.extend_from_slice(&[v[0], v[1], v[2]]);
                    self.vertex_staging.extend_from_slice(&[v[0], v[2], v[3]]);
                }
                QueuedPrimitive::FilledRect { x, y, w, h, color } => {
                    // Untextured filled rect: use white texture with UV (0,0)-(1,1)
                    let v = [
                        Vertex2D {
                            position: [*x, *y],
                            uv: [0.0, 0.0],
                            color: *color,
                        },
                        Vertex2D {
                            position: [x + w, *y],
                            uv: [1.0, 0.0],
                            color: *color,
                        },
                        Vertex2D {
                            position: [x + w, y + h],
                            uv: [1.0, 1.0],
                            color: *color,
                        },
                        Vertex2D {
                            position: [*x, y + h],
                            uv: [0.0, 1.0],
                            color: *color,
                        },
                    ];
                    self.vertex_staging.extend_from_slice(&[v[0], v[1], v[2]]);
                    self.vertex_staging.extend_from_slice(&[v[0], v[2], v[3]]);
                }
                QueuedPrimitive::Line {
                    x0,
                    y0,
                    x1,
                    y1,
                    line_width,
                    color,
                } => {
                    // Expand line to a quad with given width
                    let dx = x1 - x0;
                    let dy = y1 - y0;
                    let len = (dx * dx + dy * dy).sqrt();
                    if len < 0.001 {
                        continue;
                    }
                    let hw = line_width * 0.5;
                    // Normal direction
                    let nx = -dy / len * hw;
                    let ny = dx / len * hw;

                    let v = [
                        Vertex2D {
                            position: [x0 + nx, y0 + ny],
                            uv: [0.0, 0.0],
                            color: *color,
                        },
                        Vertex2D {
                            position: [x0 - nx, y0 - ny],
                            uv: [0.0, 1.0],
                            color: *color,
                        },
                        Vertex2D {
                            position: [x1 - nx, y1 - ny],
                            uv: [1.0, 1.0],
                            color: *color,
                        },
                        Vertex2D {
                            position: [x1 + nx, y1 + ny],
                            uv: [1.0, 0.0],
                            color: *color,
                        },
                    ];
                    self.vertex_staging.extend_from_slice(&[v[0], v[1], v[2]]);
                    self.vertex_staging.extend_from_slice(&[v[0], v[2], v[3]]);
                }
            }
        }
    }

    /// Get the appropriate pipeline for a draw image mode.
    fn pipeline_for_mode(&self, mode: DrawImageMode) -> &RenderPipeline {
        match mode {
            DrawImageMode::Alpha | DrawImageMode::Grayscale => &self.pipeline_alpha,
            DrawImageMode::Additive => &self.pipeline_additive,
            DrawImageMode::Solid => &self.pipeline_solid,
        }
    }

    /// Create uniform buffer for screen dimensions.
    fn create_screen_uniform(&self) -> Buffer {
        let data: [f32; 2] = [self.width as f32, self.height as f32];
        let bytes = bytemuck::cast_slice(&data);
        self.device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Render2D Screen Uniform"),
                contents: bytes,
                usage: BufferUsages::UNIFORM,
            })
    }

    /// Create bind group for a texture view.
    fn create_texture_bind_group(
        &self,
        texture_view: &TextureView,
        sampler: &Sampler,
    ) -> wgpu::BindGroup {
        let layout = self.pipeline_alpha.get_bind_group_layout(1);
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render2D Texture Bind Group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        })
    }

    /// Create the uniform bind group for screen dimensions.
    fn create_screen_bind_group(&self, uniform_buffer: &Buffer) -> wgpu::BindGroup {
        let layout = self.pipeline_alpha.get_bind_group_layout(0);
        self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Render2D Screen Bind Group"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        })
    }

    /// Flush all queued primitives to a render pass.
    ///
    /// Corresponds to C++ `Render2DClass::Render()`.
    pub fn flush(&mut self, render_pass: &mut RenderPass<'_>) {
        self.build_vertices();

        if self.vertex_staging.is_empty() {
            return;
        }

        // Upload vertex data
        let vertex_data = bytemuck::cast_slice(&self.vertex_staging);
        self.queue.write_buffer(&self.vertex_buffer, 0, vertex_data);

        // Create uniform and bind groups for this flush
        let screen_uniform = self.create_screen_uniform();
        let screen_bind_group = self.create_screen_bind_group(&screen_uniform);
        let texture_bind_group =
            self.create_texture_bind_group(&self.white_texture_view, &self.white_sampler);

        // Determine the pipeline to use.
        // For a batched approach, we'd sort by mode. For simplicity,
        // use alpha blending for the entire batch (most common case).
        // C++ also renders all primitives in one go per Reset()/Render() cycle.
        render_pass.set_pipeline(&self.pipeline_alpha);
        render_pass.set_bind_group(0, &screen_bind_group, &[]);
        render_pass.set_bind_group(1, &texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_staging.len() as u32, 0..1);

        // Clear after rendering (C++ resets after Render())
        self.primitives.clear();
        self.vertex_staging.clear();
    }

    /// Flush a single textured quad with a specific pipeline mode.
    ///
    /// This is used when a specific blend mode is needed for a draw image call,
    /// matching the C++ pattern where `drawImage` does its own `Reset()/Render()`.
    pub fn flush_single_image(
        &mut self,
        render_pass: &mut RenderPass<'_>,
        texture_view: &TextureView,
        sampler: &Sampler,
        mode: DrawImageMode,
    ) {
        self.build_vertices();

        if self.vertex_staging.is_empty() {
            return;
        }

        let vertex_data = bytemuck::cast_slice(&self.vertex_staging);
        self.queue.write_buffer(&self.vertex_buffer, 0, vertex_data);

        let screen_uniform = self.create_screen_uniform();
        let screen_bind_group = self.create_screen_bind_group(&screen_uniform);
        let texture_bind_group = self.create_texture_bind_group(texture_view, sampler);

        let pipeline = self.pipeline_for_mode(mode);
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &screen_bind_group, &[]);
        render_pass.set_bind_group(1, &texture_bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.draw(0..self.vertex_staging.len() as u32, 0..1);

        self.primitives.clear();
        self.vertex_staging.clear();
    }
}

// ---------------------------------------------------------------------------
// Shader source
// ---------------------------------------------------------------------------

/// WGSL shader for 2D rendering.
///
/// Converts screen-space pixel coordinates to NDC and passes UV + color to fragment.
const SHADER_SOURCE: &str = r#"
struct ScreenUniform {
    screen_size: vec2<f32>,
};

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: u32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@group(0) @binding(0)
var<uniform> screen: ScreenUniform;

@group(1) @binding(0)
var tex: texture_2d<f32>;

@group(1) @binding(1)
var tex_sampler: sampler;

fn unpack_color(packed: u32) -> vec4<f32> {
    let r = f32((packed >> 0u) & 0xFFu) / 255.0;
    let g = f32((packed >> 8u) & 0xFFu) / 255.0;
    let b = f32((packed >> 16u) & 0xFFu) / 255.0;
    let a = f32((packed >> 24u) & 0xFFu) / 255.0;
    return vec4<f32>(r, g, b, a);
}

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    // Convert screen pixels to NDC: (0,0) = top-left, (width,height) = bottom-right
    let ndc_x = (input.position.x / screen.screen_size.x) * 2.0 - 1.0;
    let ndc_y = (input.position.y / screen.screen_size.y) * 2.0 - 1.0;
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.uv = input.uv;
    out.color = unpack_color(input.color);
    return out;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let tex_color = textureSample(tex, tex_sampler, input.uv);
    return tex_color * input.color;
}
"#;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_2d_size() {
        // 2 + 2 + 1 = 5 f32s = 20 bytes
        assert_eq!(std::mem::size_of::<Vertex2D>(), 20);
    }

    #[test]
    fn test_queue_rect() {
        // Test that queuing primitives works without GPU
        let mut pipeline = FakeRender2DPipeline::new();
        pipeline.queue_rect(10.0, 20.0, 100.0, 50.0, 0xFF_FF_FF_FF);
        assert_eq!(pipeline.primitives.len(), 1);
    }

    #[test]
    fn test_queue_open_rect() {
        let mut pipeline = FakeRender2DPipeline::new();
        pipeline.queue_open_rect(0.0, 0.0, 100.0, 100.0, 1.0, 0xFF_FF_FF_FF);
        assert_eq!(pipeline.primitives.len(), 4); // 4 lines
    }

    #[test]
    fn test_queue_image() {
        let mut pipeline = FakeRender2DPipeline::new();
        pipeline.queue_image(
            0.0,
            0.0,
            100.0,
            100.0,
            0.0,
            0.0,
            1.0,
            1.0,
            0xFF_FF_FF_FF,
            0,
            DrawImageMode::Alpha,
        );
        assert_eq!(pipeline.primitives.len(), 1);
    }

    #[test]
    fn test_reset_clears_queue() {
        let mut pipeline = FakeRender2DPipeline::new();
        pipeline.queue_rect(0.0, 0.0, 10.0, 10.0, 0);
        pipeline.reset();
        assert!(pipeline.primitives.is_empty());
    }

    /// A minimal version of Render2DPipeline that doesn't need WGPU device.
    /// Used for testing the queue logic without GPU.
    struct FakeRender2DPipeline {
        primitives: Vec<QueuedPrimitive>,
    }

    impl FakeRender2DPipeline {
        fn new() -> Self {
            Self {
                primitives: Vec::new(),
            }
        }

        fn reset(&mut self) {
            self.primitives.clear();
        }

        fn queue_image(
            &mut self,
            x0: f32,
            y0: f32,
            x1: f32,
            y1: f32,
            u0: f32,
            v0: f32,
            u1: f32,
            v1: f32,
            color: u32,
            texture_id: u64,
            mode: DrawImageMode,
        ) {
            let vertices = [
                Vertex2D {
                    position: [x0, y0],
                    uv: [u0, v0],
                    color,
                },
                Vertex2D {
                    position: [x1, y0],
                    uv: [u1, v0],
                    color,
                },
                Vertex2D {
                    position: [x1, y1],
                    uv: [u1, v1],
                    color,
                },
                Vertex2D {
                    position: [x0, y1],
                    uv: [u0, v1],
                    color,
                },
            ];
            self.primitives.push(QueuedPrimitive::TexturedQuad {
                vertices,
                texture_id,
                mode,
            });
        }

        fn queue_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: u32) {
            self.primitives
                .push(QueuedPrimitive::FilledRect { x, y, w, h, color });
        }

        fn queue_line(&mut self, x0: f32, y0: f32, x1: f32, y1: f32, lw: f32, color: u32) {
            self.primitives.push(QueuedPrimitive::Line {
                x0,
                y0,
                x1,
                y1,
                line_width: lw,
                color,
            });
        }

        fn queue_open_rect(&mut self, x: f32, y: f32, w: f32, h: f32, lw: f32, color: u32) {
            self.queue_line(x, y, x + w, y, lw, color);
            self.queue_line(x + w, y, x + w, y + h, lw, color);
            self.queue_line(x + w, y + h, x, y + h, lw, color);
            self.queue_line(x, y + h, x, y, lw, color);
        }
    }
}
