//! Tactical overlays for health bars, drag selection, and ground effects.
//!
//! Selection appears on the drawable through its flash envelope and above it
//! through `Drawable::drawHealthBar`. The drag marquee is the separate
//! screen-space primitive in `W3DInGameUI::drawSelectionRegion`.

use glam::{Mat4, Vec2, Vec3};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::time::{Duration, Instant};
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// Constants (C++ parity)
// ---------------------------------------------------------------------------

/// C++ `W3DInGameUI::drawSelectionRegion()` uses `drawOpenRect(..., 2.0f,
/// 0x9933FF33)`: a two-pixel, 60%-alpha green screen-space border.
const DRAG_RECT_COLOR: [f32; 4] = [0.2, 1.0, 0.2, 0.6];
const DRAG_RECT_LINE_WIDTH_PX: f32 = 2.0;

fn drag_select_tolerance_px() -> f32 {
    game_engine::common::ini::get_mouse_settings()
        .map(|s| s.drag_tolerance)
        .filter(|&v| v > 0)
        .unwrap_or(5) as f32
}

/// C++ `InGameUI::postDraw()` draws the active RMB camera anchor as two black
/// drop rectangles followed by two opaque green center rectangles.
const RMB_SCROLL_ANCHOR_DROP_COLOR: [f32; 4] = [0.0, 0.0, 0.0, 1.0];
const RMB_SCROLL_ANCHOR_MAIN_COLOR: [f32; 4] = [0.0, 1.0, 0.0, 1.0];
const RMB_SCROLL_ANCHOR_HALF_SIZE_PX: f32 = 2.0;
const RMB_SCROLL_ANCHOR_RATIO: f32 = 4.0;

const TERRAIN_Y_OFFSET: f32 = 0.5;
const CIRCLE_SEGMENTS: u32 = 24;

// ---------------------------------------------------------------------------
// Selection render data
// ---------------------------------------------------------------------------
#[derive(Debug, Clone, Copy, Default)]
pub struct DragSelectRect {
    pub start: Vec2,
    pub end: Vec2,
    pub window_width: f32,
    pub window_height: f32,
}

impl DragSelectRect {
    pub fn is_valid(&self) -> bool {
        // C++ SelectionXlat.cpp:399-407 — per-axis delta > Mouse DragTolerance (default 5).
        let tol = drag_select_tolerance_px();
        let dx = (self.end.x - self.start.x).abs();
        let dy = (self.end.y - self.start.y).abs();
        dx > tol || dy > tol
    }
}

/// A C++ `InGameUI::postDraw()` RMB-scroll anchor expressed in screen pixels.
/// It remains an ephemeral Main presentation residual: it is derived from the
/// active input gesture, not simulation state or a savegame payload.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RmbScrollAnchorOverlay {
    pub position: Vec2,
    pub window_size: Vec2,
}

impl RmbScrollAnchorOverlay {
    /// Return an overlay only for the same condition as C++
    /// `LookAtTranslator::getRMBScrollAnchor()`: a live RMB scroll gesture
    /// with an available anchor, plus the player's DrawScrollAnchor setting.
    pub fn from_active_rmb_scroll(
        draw_anchor: bool,
        is_rmb_scrolling: bool,
        rmb_scroll_anchor: Option<(f32, f32)>,
        display_size: (f32, f32),
    ) -> Option<Self> {
        if !draw_anchor || !is_rmb_scrolling {
            return None;
        }
        let (x, y) = rmb_scroll_anchor?;
        let (width, height) = display_size;
        if !x.is_finite()
            || !y.is_finite()
            || !width.is_finite()
            || !height.is_finite()
            || width <= 0.0
            || height <= 0.0
        {
            return None;
        }
        Some(Self {
            position: Vec2::new(x, y),
            window_size: Vec2::new(width, height),
        })
    }
}

/// Pack the C++ `drawOpenRect` marquee as four screen-space quads.  Positions
/// are already WGPU clip coordinates; each vertex is `[x, y, r, g, b, a]`.
/// Keeping this CPU-only makes the geometry and its two-pixel thickness easy
/// to test without a graphics device.
fn drag_rect_screen_vertices(rect: &DragSelectRect) -> Vec<f32> {
    if !rect.is_valid() {
        return Vec::new();
    }

    let viewport_width = rect.window_width.max(1.0);
    let viewport_height = rect.window_height.max(1.0);
    let left = (rect.start.x.min(rect.end.x) / viewport_width) * 2.0 - 1.0;
    let right = (rect.start.x.max(rect.end.x) / viewport_width) * 2.0 - 1.0;
    let top = 1.0 - (rect.start.y.min(rect.end.y) / viewport_height) * 2.0;
    let bottom = 1.0 - (rect.start.y.max(rect.end.y) / viewport_height) * 2.0;
    // Pixel → clip scale is two clip units per viewport dimension.  A two
    // pixel line therefore has a half-thickness of `2 / dimension`.
    let half_width = DRAG_RECT_LINE_WIDTH_PX / viewport_width;
    let half_height = DRAG_RECT_LINE_WIDTH_PX / viewport_height;

    let mut vertices = Vec::with_capacity(24 * 6);
    let mut push_vertex = |x: f32, y: f32| {
        vertices.extend_from_slice(&[
            x,
            y,
            DRAG_RECT_COLOR[0],
            DRAG_RECT_COLOR[1],
            DRAG_RECT_COLOR[2],
            DRAG_RECT_COLOR[3],
        ]);
    };
    let mut push_quad = |min_x: f32, min_y: f32, max_x: f32, max_y: f32| {
        push_vertex(min_x, min_y);
        push_vertex(max_x, min_y);
        push_vertex(max_x, max_y);
        push_vertex(min_x, min_y);
        push_vertex(max_x, max_y);
        push_vertex(min_x, max_y);
    };

    push_quad(
        left - half_width,
        bottom - half_height,
        right + half_width,
        bottom + half_height,
    );
    push_quad(
        left - half_width,
        top - half_height,
        right + half_width,
        top + half_height,
    );
    push_quad(
        left - half_width,
        bottom - half_height,
        left + half_width,
        top + half_height,
    );
    push_quad(
        right - half_width,
        bottom - half_height,
        right + half_width,
        top + half_height,
    );
    vertices
}

/// Append one C++ `Display::drawFillRect` equivalent in screen pixels to a
/// clip-space colored-quad vertex stream.
fn append_screen_fill_rect_vertices(
    vertices: &mut Vec<f32>,
    origin: Vec2,
    size: Vec2,
    viewport: Vec2,
    color: [f32; 4],
) {
    let left = (origin.x / viewport.x) * 2.0 - 1.0;
    let right = ((origin.x + size.x) / viewport.x) * 2.0 - 1.0;
    let top = 1.0 - (origin.y / viewport.y) * 2.0;
    let bottom = 1.0 - ((origin.y + size.y) / viewport.y) * 2.0;
    let mut push_vertex = |x: f32, y: f32| {
        vertices.extend_from_slice(&[x, y, color[0], color[1], color[2], color[3]]);
    };
    push_vertex(left, bottom);
    push_vertex(right, bottom);
    push_vertex(right, top);
    push_vertex(left, bottom);
    push_vertex(right, top);
    push_vertex(left, top);
}

/// Pack C++ `InGameUI.cpp:3786-3801` exactly: four filled rectangles in draw
/// order, black horizontal/vertical drop then green horizontal/vertical main.
/// The same existing screen-space quad shader that draws the selection marquee
/// consumes the resulting vertices; this adds no shader or render subsystem.
fn rmb_scroll_anchor_screen_vertices(anchor: &RmbScrollAnchorOverlay) -> Vec<f32> {
    let w = RMB_SCROLL_ANCHOR_HALF_SIZE_PX;
    let h = RMB_SCROLL_ANCHOR_HALF_SIZE_PX;
    let ratio = RMB_SCROLL_ANCHOR_RATIO;
    let position = anchor.position;
    let viewport = anchor.window_size;
    let mut vertices = Vec::with_capacity(4 * 6 * 6);

    append_screen_fill_rect_vertices(
        &mut vertices,
        Vec2::new(position.x - w * ratio - 1.0, position.y - h - 1.0),
        Vec2::new(w * 2.0 * ratio + 3.0, h * 2.0 + 3.0),
        viewport,
        RMB_SCROLL_ANCHOR_DROP_COLOR,
    );
    append_screen_fill_rect_vertices(
        &mut vertices,
        Vec2::new(position.x - w - 1.0, position.y - h * ratio - 1.0),
        Vec2::new(w * 2.0 + 3.0, h * 2.0 * ratio + 3.0),
        viewport,
        RMB_SCROLL_ANCHOR_DROP_COLOR,
    );
    append_screen_fill_rect_vertices(
        &mut vertices,
        Vec2::new(position.x - w * ratio, position.y - h),
        Vec2::new(w * 2.0 * ratio + 1.0, h * 2.0 + 1.0),
        viewport,
        RMB_SCROLL_ANCHOR_MAIN_COLOR,
    );
    append_screen_fill_rect_vertices(
        &mut vertices,
        Vec2::new(position.x - w, position.y - h * ratio),
        Vec2::new(w * 2.0 + 1.0, h * 2.0 * ratio + 1.0),
        viewport,
        RMB_SCROLL_ANCHOR_MAIN_COLOR,
    );
    vertices
}

/// A filled ground effect, such as a fallback shadow or targeting radius.
#[derive(Debug, Clone)]
pub struct GroundCircle {
    pub position: Vec3,
    pub radius: f32,
    pub color: [f32; 4],
}

// ---------------------------------------------------------------------------
// WGSL shaders
// ---------------------------------------------------------------------------

// Single WGSL module: both entry points must live in the same ShaderModule
// (create_render_pipeline references vs_main + fs_main on `module`).
const SELECTION_SHADER: &str = r"
struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> view_proj: mat4x4<f32>;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = view_proj * vec4<f32>(input.position, 1.0);
    output.vertex_color = input.color;
    return output;
}

@fragment
fn fs_main(@location(0) vertex_color: vec4<f32>) -> @location(0) vec4<f32> {
    return vertex_color;
}
";

/// The drag marquee is a Display/UI primitive in C++, not terrain geometry.
/// Its vertices already arrive in clip coordinates, so this shader deliberately
/// has no camera uniform and no depth attachment.
const DRAG_RECT_SHADER: &str = r"
struct VertexInput {
    @location(0) clip_position: vec2<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) vertex_color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(input.clip_position, 0.0, 1.0);
    output.vertex_color = input.color;
    return output;
}

@fragment
fn fs_main(@location(0) vertex_color: vec4<f32>) -> @location(0) vec4<f32> {
    return vertex_color;
}
";

// ---------------------------------------------------------------------------
// SelectionRenderer
// ---------------------------------------------------------------------------

pub struct SelectionRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    drag_rect_pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

/// One GPU resource set per active render-pipeline device. The callbacks keep
/// their own Arc alive until submission even if a later device replaces it.
#[derive(Default)]
pub(crate) struct SelectionRendererCache {
    renderer: Option<Arc<SelectionRenderer>>,
    creations: u64,
    build_time: Duration,
}

impl SelectionRendererCache {
    pub(crate) fn get_or_create(
        &mut self,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
    ) -> Arc<SelectionRenderer> {
        if let Some(renderer) = self.renderer.as_ref()
            && Arc::ptr_eq(&renderer.device, &device)
            && Arc::ptr_eq(&renderer.queue, &queue)
        {
            return Arc::clone(renderer);
        }

        let started = Instant::now();
        let renderer = Arc::new(SelectionRenderer::new(device, queue));
        let elapsed = started.elapsed();
        self.creations += 1;
        self.build_time += elapsed;
        log::info!(
            "Selection renderer GPU resources: creations={} latest_build={:?} total_build={:?}",
            self.creations,
            elapsed,
            self.build_time,
        );
        self.renderer = Some(Arc::clone(&renderer));
        renderer
    }

    pub(crate) fn creation_count(&self) -> u64 {
        self.creations
    }

    pub(crate) fn build_time(&self) -> Duration {
        self.build_time
    }
}

impl SelectionRenderer {
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection_overlay_shader"),
            source: wgpu::ShaderSource::Wgsl(SELECTION_SHADER.into()),
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("selection_view_proj_ubo"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("selection_uniform_bgl"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("selection_uniform_bg"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("selection_pipeline_layout"),
            bind_group_layouts: &[Some(&uniform_bind_group_layout)],
            immediate_size: 0,
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection_overlay_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 28,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            depth_stencil: Some(wgpu::DepthStencilState {
                format: ww3d_gpu::SCENE_DEPTH_FORMAT,
                depth_write_enabled: Some(true),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        let drag_rect_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection_drag_rect_shader"),
            source: wgpu::ShaderSource::Wgsl(DRAG_RECT_SHADER.into()),
        });
        let drag_rect_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("selection_drag_rect_pipeline_layout"),
                bind_group_layouts: &[],
                immediate_size: 0,
            });
        let drag_rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection_drag_rect_pipeline"),
            layout: Some(&drag_rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &drag_rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &drag_rect_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
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
            multiview_mask: None,
            cache: None,
        });

        Self {
            device,
            queue,
            pipeline,
            drag_rect_pipeline,
            uniform_buffer,
            uniform_bind_group,
        }
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        view_proj: &Mat4,
        ground_circles: &[GroundCircle],
        order_line_vertices: &[f32],
    ) {
        // Shadows, ground markers and orders share the same depth and blend state.
        let mut vertices =
            Vec::with_capacity(ground_circles.len() * (CIRCLE_SEGMENTS as usize + 2) * 7);
        let mut indices = Vec::with_capacity(ground_circles.len() * CIRCLE_SEGMENTS as usize * 3);
        for circle in ground_circles {
            Self::append_ground_circle(&mut vertices, &mut indices, circle);
        }
        Self::append_order_line_segments(&mut vertices, &mut indices, order_line_vertices);
        if indices.is_empty() {
            return;
        }
        let vertex_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_world_vertices"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("selection_world_indices"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(view_proj.to_cols_array().as_ref()),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }

    // -----------------------------------------------------------------------
    // Screen-space drag rectangle
    // -----------------------------------------------------------------------

    fn draw_screen_overlay_vertices(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        vertices: &[f32],
        label: &'static str,
    ) {
        if vertices.is_empty() {
            return;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some(label),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        render_pass.set_pipeline(&self.drag_rect_pipeline);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..(vertices.len() / 6) as u32, 0..1);
    }

    fn draw_drag_rect(&self, render_pass: &mut wgpu::RenderPass<'_>, rect: &DragSelectRect) {
        let vertices = drag_rect_screen_vertices(rect);
        self.draw_screen_overlay_vertices(render_pass, &vertices, "selection_drag_rect_verts");
    }

    fn draw_rmb_scroll_anchor(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        anchor: &RmbScrollAnchorOverlay,
    ) {
        let vertices = rmb_scroll_anchor_screen_vertices(anchor);
        self.draw_screen_overlay_vertices(render_pass, &vertices, "rmb_scroll_anchor_verts");
    }

    // -----------------------------------------------------------------------
    // Filled ground effects
    // -----------------------------------------------------------------------

    fn append_ground_circle(vertices: &mut Vec<f32>, indices: &mut Vec<u32>, circle: &GroundCircle) {
        let radius = circle.radius.max(1.0);
        let center = circle.position;
        let [r, g, b, a] = circle.color;
        let y = center.y + TERRAIN_Y_OFFSET;
        let base = (vertices.len() / 7) as u32;

        vertices.extend_from_slice(&[center.x, y, center.z, r, g, b, a]);
        for i in 0..=CIRCLE_SEGMENTS {
            let angle = (i as f32 / CIRCLE_SEGMENTS as f32) * std::f32::consts::TAU;
            vertices.extend_from_slice(&[
                center.x + radius * angle.cos(),
                y,
                center.z + radius * angle.sin(),
                r,
                g,
                b,
                a,
            ]);
        }
        for i in 1..=CIRCLE_SEGMENTS {
            indices.extend_from_slice(&[base, base + i, base + i + 1]);
        }
    }

    /// Thin ground-plane quads for move/attack order line residual.
    fn append_order_line_segments(vertices: &mut Vec<f32>, indices: &mut Vec<u32>, packed: &[f32]) {
        const FPV: usize = 7; // x y z r g b a
        let half_width = 0.55f32;
        let y_lift = TERRAIN_Y_OFFSET + 0.2;
        let segments = packed.len() / (FPV * 2);
        for s in 0..segments {
            let base = s * FPV * 2;
            let ax = packed[base];
            let az = packed[base + 2];
            let ar = packed[base + 3];
            let ag = packed[base + 4];
            let ab = packed[base + 5];
            let aa = packed[base + 6];
            let bx = packed[base + FPV];
            let bz = packed[base + FPV + 2];
            let br = packed[base + FPV + 3];
            let bg = packed[base + FPV + 4];
            let bb = packed[base + FPV + 5];
            let ba = packed[base + FPV + 6];
            let dx = bx - ax;
            let dz = bz - az;
            let len = (dx * dx + dz * dz).sqrt();
            if len < 0.05 {
                continue;
            }
            let px = -dz / len * half_width;
            let pz = dx / len * half_width;
            let i0 = (vertices.len() / FPV) as u32;
            for (x, z, r, g, b, a) in [
                (ax + px, az + pz, ar, ag, ab, aa),
                (ax - px, az - pz, ar, ag, ab, aa),
                (bx - px, bz - pz, br, bg, bb, ba),
                (bx + px, bz + pz, br, bg, bb, ba),
            ] {
                vertices.extend_from_slice(&[x, y_lift, z, r, g, b, a]);
            }
            indices.extend_from_slice(&[i0, i0 + 1, i0 + 2, i0, i0 + 2, i0 + 3]);
        }
    }
}

// ---------------------------------------------------------------------------
// Public integration helpers
// ---------------------------------------------------------------------------

/// C++ `GameClient.cpp:694` `setFullyObscuredByShroud(ss >= OBJECTSHROUD_FOGGED)`
/// then `W3DModelDraw::setFullyObscuredByShroud` → `enableShadowInvisible`.
/// Fogged ghosts and black-shrouded units cast no disc.
fn blob_shadow_visible_through_shroud(u: &crate::presentation_frame::UnitRenderInput) -> bool {
    shadow_visible_through_shroud(u.drawable_shroud, u.fow_visibility)
}

pub(super) fn shadow_visible_through_shroud(
    shroud: crate::presentation_frame::PresentationDrawableShroudFacts,
    visibility: crate::fow_rendering::ObjectVisibility,
) -> bool {
    use crate::presentation_frame::PresentationObjectShroudStatus;
    if shroud.lifetime.is_direct_host_object()
        && (shroud.raw_status as u8) >= (PresentationObjectShroudStatus::Fogged as u8)
    {
        return false;
    }
    !visibility.fully_obscures_drawable()
}

/// C++ projected/volumetric shadows drape the heightmap at object XY; object Z unused.
/// Frozen `RenderableObject::ground_height` is the presentation-owned terrain sample.
fn blob_disc_ground_y(
    frame: &crate::presentation_frame::PresentationFrame,
    id: crate::game_logic::ObjectId,
) -> f32 {
    frame
        .objects
        .iter()
        .find(|o| o.id == id)
        .map(|o| o.ground_height)
        .filter(|h| h.is_finite())
        .unwrap_or(crate::presentation_frame::PRESENTATION_DEFAULT_GROUND_HEIGHT)
}

fn casts_volume(u: &crate::presentation_frame::UnitRenderInput) -> bool {
    u.shadow_type_bits & crate::game_logic::host_enum_table_residual::SHADOW_VOLUME != 0
}

/// Objects whose meshes cast stencil shadow volumes this frame (live,
/// shadows enabled, not fogged or shrouded, template `SHADOW_VOLUME`).
pub(crate) fn volume_shadow_caster_ids(
    inputs: &[crate::presentation_frame::UnitRenderInput],
) -> std::collections::HashSet<crate::game_logic::ObjectId> {
    inputs
        .iter()
        .filter(|u| casts_volume(u) && !u.destroyed && u.shadows_enabled && blob_shadow_visible_through_shroud(u))
        .map(|u| u.id)
        .collect()
}

/// Collect blob discs from a `PresentationFrame` snapshot.
///
/// Identity fields come only from the immutable snapshot — not a live re-read
/// of `GameLogic` objects. Projected shadows drape frozen terrain height for
/// visible Drawables with an authored non-volume shadow type.
pub fn collect_blob_shadows_from_presentation(
    frame: &crate::presentation_frame::PresentationFrame,
) -> Vec<GroundCircle> {
    collect_remaining_blob_shadows(frame, &std::collections::HashSet::new(), &|_, p| p)
}

fn collect_remaining_blob_shadows(
    frame: &crate::presentation_frame::PresentationFrame,
    projected_shadow_ids: &std::collections::HashSet<u32>,
    interp: &crate::graphics::move_line_upload::InterpPoint<'_>,
) -> Vec<GroundCircle> {
    // C++ addShadow NULL + renderShadows skip when !m_useShadowDecals.
    if !game_engine::common::game_lod::use_shadow_decals() {
        return Vec::new();
    }
    frame
        .unit_render_inputs_shared()
        .iter()
        .filter(|u| !u.destroyed && u.shadows_enabled && blob_shadow_visible_through_shroud(u))
        .filter(|u| !u.template_decal_disabled)
        // W3DModelDraw::allocateShadows skips SHADOW_NONE. W3DShadowManager
        // routes volumes exclusively to the stencil manager.
        .filter(|u| {
            u.shadow_type_bits != crate::game_logic::host_enum_table_residual::SHADOW_NONE
                && !casts_volume(u)
        })
        .filter(|u| !projected_shadow_ids.contains(&u.id.0))
        .map(|u| {
            // The temporary projected shadow approximation uses authored decal
            // size or a bounding extent. Volume casters use the stencil pass.
            let (sx, sy) = crate::game_logic::host_battlemaster::leftover_template_blob_shadow_size(
                &u.template_name,
            );
            let (ox, oy) = crate::game_logic::host_battlemaster::leftover_template_shadow_offset(
                &u.template_name,
                0.0,
                0.0,
            );
            let radius = sx.max(sy) * 0.5;
            // `-interpolate`: the disc anchor rides the blended root pose.
            let anchor = interp(u.id, u.position);
            GroundCircle {
                // C++ ShadowOffsetX/Y are world X/Y; live Y is height so offset Y maps to Z.
                // Disc Y is frozen terrain height, never unit altitude (jets/Comanches).
                position: glam::Vec3::new(
                    anchor.x + ox,
                    blob_disc_ground_y(frame, u.id),
                    anchor.z + oy,
                ),
                radius,
                color: [0.0, 0.0, 0.0, 0.4],
            }
        })
        .collect()
}

/// Rally flag is the leftover RallyPointMarker mesh (unit_render_inputs), not a line.
fn pack_rally_point_lines(_frame: &crate::presentation_frame::PresentationFrame) -> Vec<f32> {
    Vec::new()
}

/// C++ `Drawable::drawIconUI` health bar (`Drawable.cpp:2738-2754` via
/// `computeHealthRegion` `:2661-2704` + `drawHealthBar` `:3825-3937`): a
/// screen-space bar above each selected or moused-over drawable. The frozen
/// presentation carries Object::getHealthBoxPosition's height and
/// getHealthBoxDimensions's width; the driving camera supplies C++ getZoom.
/// Width scales by 1 / zoom, height remains 3 pixels, and the anchor must be
/// inside the tactical frustum (View::worldToScreen).
// ---------------------------------------------------------------------------
// ShowObjectHealth gate (C++ Drawable.cpp:3834)
// ---------------------------------------------------------------------------

/// `TheGlobalData->m_showObjectHealth` decision.
///
/// The parsed INI value leads (`Drawable.cpp:3834`; C++ constructor default
/// FALSE, `GlobalData.cpp:795`). When this install never parsed a GameData
/// block at all, `show_object_health` sits at the Rust constructor default
/// (`ini_game_data.rs:862` `false`) — not an authored value — so the live
/// default follows retail, which ships `ShowObjectHealth = Yes`
/// (`GameData.ini:25`, INIZH `Data/INI/GameData.ini:31835`). A present
/// GameData source is trusted as parsed: an authored `No` stays closed.
fn resolve_show_object_health(parsed: Option<bool>, retail_game_data_source_present: bool) -> bool {
    match parsed {
        Some(true) => true,
        Some(false) => !retail_game_data_source_present,
        None => true,
    }
}

/// Cached probe: 0 unresolved, 1 a GameData INI source resolves on this
/// install, 2 none resolves.

/// Whether any boot GameData INI source (`Data/INI/GameData.ini` or
/// `Data/INI/Default/GameData.ini`, the `bootstrap_global_data_from_ini`
/// pair) resolves from this install's extracted disk roots.
fn retail_game_data_source_present() -> bool {
    match crate::main_services::main_services().retail_game_data_probe.load(Ordering::Relaxed) {
        1 => true,
        2 => false,
        _ => {
            let present = ["Data/INI/GameData.ini", "Data/INI/Default/GameData.ini"]
                .iter()
                .any(|path| {
                    crate::cnc_game_engine::CnCGameEngine::read_startup_ini_from_disk(path)
                        .is_some()
                });
            crate::main_services::main_services().retail_game_data_probe.store(if present { 1 } else { 2 }, Ordering::Relaxed);
            present
        }
    }
}

/// Test-only override of the retail GameData probe cache (1 present,
/// 2 absent, 0 re-probe on next use).
#[cfg(test)]
fn force_retail_game_data_probe_for_tests(state: u8) {
    crate::main_services::main_services().retail_game_data_probe.store(state, Ordering::Relaxed);
}

/// Log the active ShowObjectHealth value once at InGame entry (first
/// health-bar pack of the session).
fn log_show_object_health_gate_once(open: bool) {
    let services = crate::main_services::main_services();
    if !services
        .show_object_health_gate_logged
        .swap(true, Ordering::Relaxed)
    {
        if open {
            log::info!("ShowObjectHealth gate open at InGame entry: health bars visible");
        } else {
            log::info!("ShowObjectHealth gate closed at InGame entry: GameData block sets No");
        }
    }
}

fn pack_health_bar_quads(
    frame: &crate::presentation_frame::PresentationFrame,
    view_matrix: &Mat4,
    projection_matrix: &Mat4,
    tactical_viewport: (f32, f32),
    display_size: (f32, f32),
    camera_zoom: f32,
    hovered_id: Option<u32>,
    interp: &crate::graphics::move_line_upload::InterpPoint<'_>,
) -> (Vec<f32>, usize) {
    use crate::game_logic::host_enum_table_residual::{
        MC_BIT_DAMAGED, MC_BIT_REALLYDAMAGED, host_model_condition_has,
    };

    // C++ gate: TheGlobalData->m_showObjectHealth (`Drawable.cpp:3834`).
    // The parsed INI value leads; when this install never parsed a GameData
    // block the retail default (`ShowObjectHealth = Yes`, GameData.ini:25 /
    // INIZH GameData.ini:31835) applies.
    let parsed_show_object_health =
        game_engine::common::ini::get_global_data().map(|data| data.read().show_object_health);
    let show_object_health =
        resolve_show_object_health(parsed_show_object_health, retail_game_data_source_present());
    log_show_object_health_gate_once(show_object_health);
    if !show_object_health
        || display_size.0 <= 0.0
        || display_size.1 <= 0.0
        || !camera_zoom.is_finite()
        || camera_zoom <= 0.0
    {
        return (Vec::new(), 0);
    }

    let viewport = Vec2::new(display_size.0.max(1.0), display_size.1.max(1.0));
    let view_proj = *projection_matrix * *view_matrix;
    let (vp_w, vp_h) = (tactical_viewport.0.max(1.0), tactical_viewport.1.max(1.0));
    let mut vertices = Vec::new();
    let mut selected_bars = 0;

    // C++ gate (`Drawable.cpp:3834-3835`): selected OR moused-over
    // drawables. The host feeds the mouseover id at the HintSpy boundary
    // (camera.rs `sync_ingame_mouseover_hint` → `create_mouseover_hint`,
    // C++ InGameUI.cpp:2254-2284 / impl_input.rs moused_over_drawable_id),
    // and the id shares the presentation ObjectId u32 space.

    for object in frame
        .drawable_objects()
        .filter(|o| !o.destroyed && (o.selected || hovered_id == Some(o.id.0)))
    {
        // C++ bails when maxHealth == 0 or health == 0 (`Drawable.cpp:3860`).
        if object.health_max <= 0.0
            || object.health_current <= 0.0
            || object.health_box_width <= 0.0
        {
            continue;
        }
        // C++ skips force-attackable props that would otherwise flash a bar
        // on force-attack (`Drawable.cpp:3846-3849`).
        if object
            .kind_of
            .iter()
            .any(|k| matches!(k, crate::game_logic::KindOf::ForceAttackable))
        {
            continue;
        }
        let ratio = (object.health_current / object.health_max).clamp(0.0, 1.0);
        let really_damaged =
            host_model_condition_has(object.model_condition_bits, MC_BIT_REALLYDAMAGED);
        let damaged = host_model_condition_has(object.model_condition_bits, MC_BIT_DAMAGED);
        // C++ `isDisabled() && !isDisabledByType(DISABLED_HELD)` (`:3872`);
        let disabled_not_held = object.disabled && !object.disabled_held;
        let (fill, outline) = game_client::drawable::drawable::health_bar_colors(
            ratio,
            object.under_construction || disabled_not_held,
            really_damaged,
            damaged,
        );

        // Project the bar anchor (unit top + lift) into tactical-viewport
        // pixels; the viewport starts at target pixel (0,0) so the coordinates
        // double as full-target pixels for the clip conversion below.
        // `-interpolate`: the anchor rides the blended root pose.
        let object_position = interp(object.id, object.position);
        let anchor = Vec3::new(
            object_position.x,
            object_position.y + object.health_box_z_offset,
            object_position.z,
        );
        let clip = view_proj * anchor.extend(1.0);
        if clip.w <= 0.0 {
            continue;
        }
        let ndc = clip.truncate() / clip.w;
        // C++ computeHealthRegion returns false when worldToScreen does not
        // return WTS_INSIDE_FRUSTUM, including points beyond the clip planes.
        if !ndc.is_finite()
            || ndc.x.abs() > 1.0
            || ndc.y.abs() > 1.0
            || !(0.0..=1.0).contains(&ndc.z)
        {
            continue;
        }
        let cx = (ndc.x * 0.5 + 0.5) * vp_w;
        let cy = (1.0 - (ndc.y * 0.5 + 0.5)) * vp_h;
        if !cx.is_finite() || !cy.is_finite() {
            continue;
        }

        // getHealthBoxDimensions #else branch: width = MAX(20, MIN(150,
        // major+minor) * 2) frozen into `health_box_width`; height 3 px.
        let bar_w = object.health_box_width / camera_zoom;
        let bar_h = 3.0;
        // computeHealthRegion: lo = center − (width*0.45, height*0.5).
        let bar_x = cx - bar_w * 0.45;
        let bar_y = cy - bar_h * 0.5;
        let fill_color = [fill[0], fill[1], fill[2], fill[3]];
        let outline_color = [outline[0], outline[1], outline[2], outline[3]];
        // drawOpenRect outline (1 px) as four thin quads …
        push_open_rect(
            &mut vertices,
            Vec2::new(bar_x, bar_y),
            Vec2::new(bar_w, bar_h),
            1.0,
            viewport,
            outline_color,
        );
        // … then the drawFillRect inset fill, width × healthRatio.
        append_screen_fill_rect_vertices(
            &mut vertices,
            Vec2::new(bar_x + 1.0, bar_y + 1.0),
            Vec2::new(((bar_w - 2.0) * ratio).max(0.0), bar_h - 2.0),
            viewport,
            fill_color,
        );
        selected_bars += usize::from(object.selected);
    }
    (vertices, selected_bars)
}

/// Four thin quads forming a C++ `drawOpenRect` 1-px outline.
fn push_open_rect(
    vertices: &mut Vec<f32>,
    origin: Vec2,
    size: Vec2,
    thickness: f32,
    viewport: Vec2,
    color: [f32; 4],
) {
    let (x, y) = (origin.x, origin.y);
    let (w, h) = (size.x, size.y);
    let t = thickness.max(1.0);
    append_screen_fill_rect_vertices(vertices, Vec2::new(x, y), Vec2::new(w, t), viewport, color);
    append_screen_fill_rect_vertices(
        vertices,
        Vec2::new(x, y + h - t),
        Vec2::new(w, t),
        viewport,
        color,
    );
    append_screen_fill_rect_vertices(
        vertices,
        Vec2::new(x, y + t),
        Vec2::new(t, (h - 2.0 * t).max(0.0)),
        viewport,
        color,
    );
    append_screen_fill_rect_vertices(
        vertices,
        Vec2::new(x + w - t, y + t),
        Vec2::new(t, (h - 2.0 * t).max(0.0)),
        viewport,
        color,
    );
}

pub fn enqueue_selection_render(
    pipeline: &mut crate::graphics::render_pipeline::RenderPipeline,
    view_matrix: &Mat4,
    projection_matrix: &Mat4,
    drag_rect: Option<DragSelectRect>,
    rmb_scroll_anchor: Option<RmbScrollAnchorOverlay>,
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
    projected_shadow_ids: &std::collections::HashSet<u32>,
    // Placement ghost / special-power radius / guard area residual circles.
    ground_markers: Vec<GroundCircle>,
    show_move_lines: bool,
    show_attack_lines: bool,
    // Full window pixel size, for screen-space health-bar clip conversion.
    display_size: (f32, f32),
    camera_zoom: f32,
    hovered_drawable: Option<u32>,
) {
    let view_proj = *projection_matrix * *view_matrix;

    // `-interpolate`: overlay anchors ride the same blended pose as the
    // corrected meshes; an absent flag leaves every point unchanged.
    let interp = |id: crate::game_logic::ObjectId, p: glam::Vec3| -> glam::Vec3 {
        pipeline.interpolate_object_point(id, p)
    };

    let mut ground_circles = presentation
        .map(|frame| collect_remaining_blob_shadows(frame, projected_shadow_ids, &interp))
        .unwrap_or_default();
    ground_circles.extend(ground_markers);

    // C++ Drawable::drawIconUI health bars for the selected set (Drawable.cpp
    // 2738-2754 / 3825-3937), packed for the 2D overlay pass.
    let (health_bar_vertices, selected_bars) = presentation
        .map(|frame| {
            let tactical_viewport = pipeline.tactical_viewport_pixel_size();
            pack_health_bar_quads(
                frame,
                view_matrix,
                projection_matrix,
                (tactical_viewport.0.max(1.0), tactical_viewport.1.max(1.0)),
                display_size,
                camera_zoom,
                hovered_drawable,
                &interp,
            )
        })
        .unwrap_or_default();

    // Move/attack order line residual from presentation snapshot.
    let mut order_line_vertices: Vec<f32> = Vec::new();
    if let Some(frame) = presentation {
        if show_move_lines {
            let move_pack =
                crate::graphics::move_line_upload::MoveLineUpload::pack_from_presentation(
                    frame, &interp,
                );
            order_line_vertices.extend_from_slice(&move_pack.vertices);
        }
        if show_attack_lines {
            let atk_pack =
                crate::graphics::attack_line_upload::AttackLineUpload::pack_from_presentation(
                    frame, &interp,
                );
            order_line_vertices.extend_from_slice(&atk_pack.vertices);
        }
        // Structure rally-point line residual (selected producers → rally).
        order_line_vertices.extend(pack_rally_point_lines(frame));
    }
    pipeline.debug_last_selection_indicators = selected_bars;

    let drag_rect = drag_rect.filter(|rect| rect.is_valid());
    let rmb_scroll_anchor = rmb_scroll_anchor.filter(|anchor| {
        anchor.position.is_finite()
            && anchor.window_size.is_finite()
            && anchor.window_size.x > 0.0
            && anchor.window_size.y > 0.0
    });
    if drag_rect.is_none()
        && rmb_scroll_anchor.is_none()
        && ground_circles.is_empty()
        && order_line_vertices.is_empty()
        && health_bar_vertices.is_empty()
    {
        return;
    }

    let renderer = pipeline.selection_renderer();

    if !ground_circles.is_empty() || !order_line_vertices.is_empty() {
        let world_renderer = Arc::clone(&renderer);
        // C++ dx8wrapper Set_Viewport parity: this world-space overlay renders
        // through the SAME tactical viewport as the terrain and mesh lanes, so
        // its depth reads/writes line up row-for-row with theirs.
        let (vp_w, vp_h) = pipeline.tactical_viewport_pixel_size();
        pipeline.enqueue_frame_work(ww3d_renderer_3d::rendering::wgpu_main_renderer::FramePhase::DecalShadows, move |frame| {
            // Ground decals share the frame's scene pass (plan R12).
            let render_pass = frame.scene_pass();
            let vp_w = vp_w.max(1.0);
            let vp_h = vp_h.max(1.0);
            render_pass.set_viewport(0.0, 0.0, vp_w, vp_h, 0.0, 1.0);
            render_pass.set_scissor_rect(0, 0, vp_w as u32, vp_h as u32);

            world_renderer.draw(
                render_pass,
                &view_proj,
                &ground_circles,
                &order_line_vertices,
            );
            Ok(())
        });
    }

    if drag_rect.is_some() || rmb_scroll_anchor.is_some() || !health_bar_vertices.is_empty() {
        // C++ draws this in W3DInGameUI's 2D pass, after the scene and before
        // window repaint. Queue it before Main queues its UI flush, so HUD
        // widgets still render over both the marquee and RMB anchor exactly as
        // in the original.
        let drag_renderer = Arc::clone(&renderer);
        pipeline.enqueue_frame_work(ww3d_renderer_3d::rendering::wgpu_main_renderer::FramePhase::Overlay2D, move |frame| {
            let color_view = frame.color_view_arc();
            frame.note_render_pass();
            let encoder = frame.encoder();
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("selection drag rectangle pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            if let Some(drag_rect) = drag_rect.as_ref() {
                drag_renderer.draw_drag_rect(&mut render_pass, drag_rect);
            }
            if let Some(rmb_scroll_anchor) = rmb_scroll_anchor.as_ref() {
                drag_renderer.draw_rmb_scroll_anchor(&mut render_pass, rmb_scroll_anchor);
            }
            if !health_bar_vertices.is_empty() {
                drag_renderer.draw_screen_overlay_vertices(
                    &mut render_pass,
                    &health_bar_vertices,
                    "selection_health_bar_verts",
                );
            }
            drop(render_pass);
            Ok(())
        });
    }
}

#[cfg(test)]
mod selection_renderer_cache_tests {
    use super::*;

    #[test]
    fn gpu_resources_reuse_one_device_and_rebuild_for_another() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("headless GPU adapter");
        let make_device = || {
            let (device, queue) =
                pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                    .expect("headless GPU device");
            (Arc::new(device), Arc::new(queue))
        };

        let (device_a, queue_a) = make_device();
        let mut cache = SelectionRendererCache::default();
        assert_eq!(cache.creation_count(), 0);
        let first = cache.get_or_create(Arc::clone(&device_a), Arc::clone(&queue_a));
        for _ in 0..100 {
            let reused = cache.get_or_create(Arc::clone(&device_a), Arc::clone(&queue_a));
            assert!(Arc::ptr_eq(&first, &reused));
        }
        assert_eq!(cache.creation_count(), 1);
        assert!(cache.build_time() > Duration::ZERO);

        let (device_b, queue_b) = make_device();
        let second = cache.get_or_create(Arc::clone(&device_b), Arc::clone(&queue_b));
        assert!(!Arc::ptr_eq(&first, &second));
        assert!(Arc::ptr_eq(&second.device, &device_b));
        assert_eq!(cache.creation_count(), 2);
        // The queued callback's old Arc still retains its original resources.
        assert!(Arc::ptr_eq(&first.device, &device_a));
        eprintln!(
            "selection renderer: 2 GPU resource builds in {:?}",
            cache.build_time()
        );
    }
}

#[cfg(test)]
mod presentation_selection_tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::golden_skirmish_config;
    use glam::Vec3;

    #[test]
    fn shipped_blob_shadows_use_the_frozen_presentation_pose() {
        // Criterion 2: production consumer identity from PresentationFrame.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelPresMap");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("SelUnit").field("Shadow = SHADOW_DECAL").kind_of("INFANTRY SELECTABLE").health(100.0).walker(),
            ],
        );
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg).expect("config");

        let id = logic
            .create_object("SelUnit", Team::USA, Vec3::new(12.0, 4.0, -7.0))
            .expect("unit");
        if let Some(o) = logic./* Wave 950 */ host_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 9.0;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }

        let snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        // Mutate live world after snapshot — consumer must keep snapshot identity.
        logic.set_fixture_object_position(id, Vec3::new(999.0, 0.0, 999.0));
        if let Some(o) = logic.host_object_mut(id) {
            o.selected = false;
            o.status.selected = false;
            o.health.current = 1.0;
        }

        let blobs = collect_blob_shadows_from_presentation(&snap);
        let inputs = snap.unit_render_inputs();
        let visible_inputs: Vec<_> = inputs
            .iter()
            .filter(|u| !u.destroyed && u.shadows_enabled && blob_shadow_visible_through_shroud(u))
            .collect();
        assert!(
            !blobs.is_empty(),
            "blob shadows must be drawn under unit_render_inputs poses"
        );
        assert_eq!(blobs.len(), visible_inputs.len());
        assert!(
            blobs
                .iter()
                .any(|b| (b.position.x - 12.0).abs() < 0.01 && (b.position.z + 7.0).abs() < 0.01),
            "created unit still casts a blob at snapshot XY"
        );
        assert!(
            blobs
                .iter()
                .all(|b| b.color[0] == 0.0 && b.color[3] > 0.0),
            "blob shadows are dark discs"
        );
        for (blob, unit) in blobs.iter().zip(visible_inputs.iter()) {
            let ground = blob_disc_ground_y(&snap, unit.id);
            assert!(
                (blob.position.y - ground).abs() < 0.01,
                "blob Y must drape frozen ground_height, not unit altitude: blob={} ground={} unit={}",
                blob.position.y,
                ground,
                unit.position.y
            );
        }
        // W3D mesh lighting has no shadow term; C++ shadows are stencil
        // volumes and projected decals (W3DShadow.cpp), so the forward pass
        // samples no cascade map (asserted by the renderer crate's
        // live_cascade_shadow tests). The blob discs above are that decal pass.
    }

    #[test]
    fn blob_shadow_anchor_follows_interp_remap() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelInterpMap");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("SelUnit").field("Shadow = SHADOW_DECAL").kind_of("INFANTRY SELECTABLE").health(100.0).walker(),
            ],
        );
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg).expect("config");
        let id = logic
            .create_object("SelUnit", Team::USA, Vec3::new(12.0, 4.0, -7.0))
            .expect("unit");
        if let Some(o) = logic.host_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 9.0;
        }
        let snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let base = collect_blob_shadows_from_presentation(&snap);
        let shift = Vec3::new(5.0, 0.0, 2.0);
        let blobs =
            collect_remaining_blob_shadows(&snap, &std::collections::HashSet::new(), &|_, p| {
                p + shift
            });
        assert_eq!(blobs.len(), base.len());
        for (blended, current) in blobs.iter().zip(base.iter()) {
            assert!((blended.position.x - (current.position.x + 5.0)).abs() < 0.01);
            assert!((blended.position.z - (current.position.z + 2.0)).abs() < 0.01);
            // Disc Y still drapes frozen terrain height, not the blended pose.
            assert!((blended.position.y - current.position.y).abs() < 0.01);
        }
    }

    #[test]
    fn airborne_blob_discs_project_to_terrain() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AirBlobMap");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("AirBlobJet").field("Shadow = SHADOW_DECAL").kind_of("AIRCRAFT").health(100.0),
            ],
        );
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg).expect("config");

        let id = logic
            .create_object("AirBlobJet", Team::USA, Vec3::new(20.0, 80.0, 15.0))
            .expect("jet");
        if let Some(o) = logic.host_object_mut(id) {
            o.status.airborne_target = true;
        }

        let mut snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        for o in &mut snap.objects {
            if o.id == id {
                o.position.y = 80.0;
                o.ground_height = 3.5;
                o.ground_height_from_terrain = true;
            }
        }
        let blobs = collect_blob_shadows_from_presentation(&snap);
        assert_eq!(blobs.len(), 1, "airborne unit still casts a ground disc");
        assert!(
            (blobs[0].position.y - 3.5).abs() < 0.01,
            "airborne blob must sit on terrain, not flight altitude: {:?}",
            blobs[0].position
        );
        assert!(
            (blobs[0].position.x - 20.0).abs() < 0.01 && (blobs[0].position.z - 15.0).abs() < 0.01,
            "blob XZ follows unit XY: {:?}",
            blobs[0].position
        );
    }

    #[test]
    fn blob_discs_hidden_under_fog_and_black_shroud() {
        use crate::fow_rendering::ObjectVisibility;
        use crate::presentation_frame::{
            PresentationDrawableShroudFacts, PresentationObjectShroudStatus,
        };

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("FogBlobMap");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("FogBlobUnit").field("Shadow = SHADOW_DECAL").kind_of("INFANTRY").health(100.0).walker(),
            ],
        );
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg).expect("config");

        let _id = logic
            .create_object("FogBlobUnit", Team::USA, Vec3::new(8.0, 0.0, 8.0))
            .expect("unit");

        let mut snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        assert!(
            !collect_blob_shadows_from_presentation(&snap).is_empty(),
            "clear unit casts a blob"
        );

        for o in &mut snap.objects {
            o.fow_visibility = ObjectVisibility::VISIBLE;
            o.drawable_shroud = PresentationDrawableShroudFacts::direct_host_object(
                PresentationObjectShroudStatus::PartialClear,
                false,
            );
        }
        assert!(
            !collect_blob_shadows_from_presentation(&snap).is_empty(),
            "partial-clear still casts a blob"
        );

        for o in &mut snap.objects {
            o.fow_visibility = ObjectVisibility::FOGGED;
            o.drawable_shroud = PresentationDrawableShroudFacts::direct_host_object(
                PresentationObjectShroudStatus::Fogged,
                false,
            );
        }
        assert!(
            collect_blob_shadows_from_presentation(&snap).is_empty(),
            "fogged units must not cast blob discs"
        );

        for o in &mut snap.objects {
            o.fow_visibility = ObjectVisibility::HIDDEN;
            o.drawable_shroud = PresentationDrawableShroudFacts::direct_host_object(
                PresentationObjectShroudStatus::Shrouded,
                false,
            );
        }
        assert!(
            collect_blob_shadows_from_presentation(&snap).is_empty(),
            "black-shrouded units must not cast blob discs"
        );

        for o in &mut snap.objects {
            o.fow_visibility = ObjectVisibility::FOGGED;
            o.drawable_shroud = PresentationDrawableShroudFacts::default();
        }
        assert!(
            collect_blob_shadows_from_presentation(&snap).is_empty(),
            "FOW-alpha fog without direct shroud facts must still hide the disc"
        );
    }

    #[test]
    fn tree_draw_with_shadows_disabled_emits_no_shadow_disc() {
        use crate::fixture_content::FixtureObject;

        fn decal_object(name: &str) -> FixtureObject {
            FixtureObject::new(name)
                .field("Shadow = SHADOW_DECAL")
                .health(100.0)
        }

        let mut logic = GameLogic::new();
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                decal_object("DefaultShadowlessTree")
                    .module("Draw = W3DTreeDraw Tree\n ModelName = TestTree\nEnd"),
                decal_object("EnabledShadowTree")
                    .module("Draw = W3DTreeDraw Tree\n ModelName = TestTree\n DoShadow = Yes\nEnd"),
                decal_object("ModelDecalTree").model("TestTree"),
            ],
        );
        let cfg = golden_skirmish_config("TreeShadowMap");
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg)
            .expect("config");

        let default = logic
            .create_object("DefaultShadowlessTree", Team::USA, Vec3::new(10.0, 0.0, 10.0))
            .expect("default shadowless tree");
        let enabled = logic
            .create_object("EnabledShadowTree", Team::USA, Vec3::new(30.0, 0.0, 10.0))
            .expect("enabled shadow tree");
        let model = logic
            .create_object("ModelDecalTree", Team::USA, Vec3::new(50.0, 0.0, 10.0))
            .expect("model draw");
        let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let inputs = frame.unit_render_inputs();
        for id in [default, enabled, model] {
            let input = inputs.iter().find(|input| input.id == id).expect("render input");
            assert!(input.shadows_enabled && blob_shadow_visible_through_shroud(input));
            assert_eq!(
                input.shadow_type_bits,
                crate::game_logic::host_enum_table_residual::SHADOW_DECAL
            );
        }

        let circles = collect_blob_shadows_from_presentation(&frame);
        let disc_at = |x: f32| {
            circles
                .iter()
                .any(|disc| (disc.position.x - x).abs() < 0.01 && (disc.position.z - 10.0).abs() < 0.01)
        };
        assert!(!disc_at(10.0), "W3DTreeDraw defaults DoShadow to No");
        assert!(disc_at(30.0), "trees with DoShadow enabled still cast a shadow");
        assert!(disc_at(50.0), "W3DModelDraw has separate decal admission");
    }

    #[test]
    fn blob_discs_hidden_when_use_shadow_decals_off() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DecalOffBlobMap");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("DecalOffUnit").field("Shadow = SHADOW_DECAL").kind_of("INFANTRY").health(100.0).walker(),
            ],
        );
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg).expect("config");
        let _id = logic
            .create_object("DecalOffUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("unit");
        let snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();

        let prev = game_engine::common::global_data::read_safe()
            .map(|g| g.writable.use_shadow_decals)
            .unwrap_or(true);
        let restore = || {
            if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
                runtime.writable.use_shadow_decals = prev;
            }
        };

        if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
            runtime.writable.use_shadow_decals = true;
        }
        assert!(
            !collect_blob_shadows_from_presentation(&snap).is_empty(),
            "2D Shadows on still casts a blob"
        );

        if let Ok(mut runtime) = game_engine::common::global_data::write_safe() {
            runtime.writable.use_shadow_decals = false;
        }
        assert!(
            collect_blob_shadows_from_presentation(&snap).is_empty(),
            "C++ addShadow/renderShadows skip unit blobs when !UseShadowDecals"
        );
        restore();
    }

    #[test]
    fn disabling_volume_shadows_does_not_replace_them_with_discs() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("VolBlobMap");
        crate::fixture_content::admit_fixture_objects(
            &mut logic,
            &[
                crate::fixture_content::FixtureObject::new("VolBlobTank")
                    .field("Shadow = SHADOW_VOLUME")
                    .kind_of("VEHICLE")
                    .health(100.0)
                    .walker(),
                crate::fixture_content::FixtureObject::new("DecalShadowInfantry")
                    .field("Shadow = SHADOW_DECAL")
                    .kind_of("INFANTRY")
                    .health(100.0),
                crate::fixture_content::FixtureObject::new("NoShadowInfantry")
                    .field("Shadow = NONE")
                    .kind_of("INFANTRY")
                    .health(100.0),
            ],
        );
        crate::skirmish_config::lobby_definition_tests::start_slot_skirmish(&mut logic, &cfg)
            .expect("config");
        let tank = logic
            .create_object("VolBlobTank", Team::USA, Vec3::new(4.0, 0.0, 6.0))
            .unwrap();
        logic
            .create_object("DecalShadowInfantry", Team::USA, Vec3::new(14.0, 0.0, 16.0))
            .unwrap();
        let no_shadow = logic
            .create_object("NoShadowInfantry", Team::USA, Vec3::new(24.0, 0.0, 26.0))
            .unwrap();

        let snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
        let none = snap
            .unit_render_inputs()
            .into_iter()
            .find(|unit| unit.id == no_shadow)
            .unwrap();
        assert_eq!(
            none.shadow_type_bits,
            crate::game_logic::host_enum_table_residual::SHADOW_NONE,
        );
        let set_volumes = |enabled| {
            let mut runtime = game_engine::common::global_data::write_safe().unwrap();
            runtime.writable.use_shadow_volumes = enabled;
        };
        let previous = game_engine::common::game_lod::use_shadow_volumes();
        set_volumes(true);
        let with_volumes = collect_blob_shadows_from_presentation(&snap);
        set_volumes(false);
        let without_volumes = collect_blob_shadows_from_presentation(&snap);
        set_volumes(previous);

        assert!(volume_shadow_caster_ids(&snap.unit_render_inputs()).contains(&tank));
        // SHADOW_DECAL still draws with either stencil setting.
        for decals in [with_volumes, without_volumes] {
            assert_eq!(decals.len(), 1);
            assert_eq!(decals[0].position.x, 14.0);
            assert_eq!(decals[0].position.z, 16.0);
        }
    }

    #[test]
    fn health_bar_disabled_colors_follow_cpp_held_mask() {
        // Drawable.cpp:3872 exempts DISABLED_HELD, not DISABLED_FREEFALL.
        use crate::game_logic::{KindOf, ThingTemplate};
        game_engine::common::ini::ini_game_data::init_global_data();
        super::force_retail_game_data_probe_for_tests(1);
        game_engine::common::ini::get_global_data()
            .expect("GameData")
            .write()
            .show_object_health = true;

        let mut logic = GameLogic::new();
        let mut template = ThingTemplate::new("HeldHealthColorUnit");
        template.set_health(100.0);
        template.add_kind_of(KindOf::Infantry);
        template.add_kind_of(KindOf::Selectable);
        logic.templates.insert(template.name.clone(), template);
        let id = logic
            .create_object("HeldHealthColorUnit", Team::USA, Vec3::ZERO)
            .expect("unit");
        let view = Mat4::look_at_rh(Vec3::new(0.0, 30.0, 80.0), Vec3::ZERO, Vec3::Y);
        let proj = Mat4::perspective_rh(1.0, 640.0 / 384.0, 1.0, 800.0);

        for (held, freefall, emp, construction, expected) in [
            (true, false, false, false, [0.25, 1.0, 0.0]),
            (false, true, false, false, [0.0, 0.75, 1.0]),
            (true, true, false, false, [0.25, 1.0, 0.0]),
            (true, false, true, false, [0.25, 1.0, 0.0]),
            (false, false, true, false, [0.0, 0.75, 1.0]),
            (true, false, false, true, [0.0, 0.75, 1.0]),
        ] {
            let object = logic.host_object_mut(id).expect("unit");
            object.selected = true;
            object.status.selected = true;
            object.health.current = 75.0;
            object.health.maximum = 100.0;
            object.status.disabled_held = held;
            object.status.disabled_freefall = freefall;
            object.status.disabled_emp = emp;
            object.status.under_construction = construction;
            let frame = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();
            assert_eq!(frame.objects[0].disabled_held, held);
            let mut encoded = serde_json::to_value(&frame.objects[0]).expect("encode frame object");
            let restored: crate::presentation_frame::RenderableObject =
                serde_json::from_value(encoded.clone()).expect("decode held fact");
            assert_eq!(restored.disabled_held, held);
            encoded.as_object_mut().unwrap().remove("disabled_held");
            let older: crate::presentation_frame::RenderableObject =
                serde_json::from_value(encoded).expect("older JSON frame object");
            assert!(!older.disabled_held, "missing held fact defaults inactive");
            let (packed, selected_bars) = pack_health_bar_quads(
                &frame,
                &view,
                &proj,
                (640.0, 384.0),
                (640.0, 480.0),
                1.0,
                None,
                &|_, p| p,
            );
            assert_eq!(selected_bars, 1);
            assert_eq!(packed.len(), 180, "one outlined health bar");
            let fill = &packed[packed.len() - 36..];
            for channel in 0..3 {
                assert!(
                    (fill[channel + 2] - expected[channel]).abs() < 0.01,
                    "held={held} freefall={freefall} emp={emp} construction={construction}: \
                     fill RGB {:?}, expected {expected:?}",
                    &fill[2..5]
                );
            }
        }
        super::force_retail_game_data_probe_for_tests(0);
    }

    #[test]
    fn health_bar_packs_cpp_quads_when_gate_open() {
        // C++ Drawable::drawHealthBar (`Drawable.cpp:3825-3937`): one open-rect
        // outline + inset fill per selected OR moused-over drawable
        // (`:3834-3835`), gated on TheGlobalData->m_showObjectHealth.
        use crate::game_logic::{KindOf, ThingTemplate};
        game_engine::common::ini::ini_game_data::init_global_data();
        let mut logic = GameLogic::new();
        let mut t = ThingTemplate::new("SelHpUnit");
        t.set_health(80.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("SelHpUnit".into(), t);
        let mut other = ThingTemplate::new("HoverHpUnit");
        other.set_health(90.0);
        other.add_kind_of(KindOf::Infantry);
        other.add_kind_of(KindOf::Selectable);
        logic.templates.insert("HoverHpUnit".into(), other);
        let id = logic
            .create_object("SelHpUnit", Team::USA, Vec3::new(12.0, 4.0, -7.0))
            .expect("unit");
        let hover_id = logic
            .create_object("HoverHpUnit", Team::USA, Vec3::new(24.0, 4.0, -14.0))
            .expect("hover unit");
        if let Some(o) = logic.host_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 6.0;
            o.health.current = 60.0;
            o.health.maximum = 80.0;
        }
        if let Some(o) = logic.host_object_mut(hover_id) {
            o.health.current = 45.0;
            o.health.maximum = 90.0;
        }
        let snap = PresentationFrame::try_build_from_logic(&mut logic, 0).unwrap();

        let view = Mat4::look_at_rh(
            Vec3::new(12.0, 30.0, 40.0),
            Vec3::new(12.0, 4.0, -7.0),
            Vec3::Y,
        );
        let proj = Mat4::perspective_rh(1.0, 640.0 / 384.0, 1.0, 800.0);
        let pack = |snap: &PresentationFrame, hover| {
            pack_health_bar_quads(
                snap,
                &view,
                &proj,
                (640.0, 384.0),
                (640.0, 480.0),
                1.0,
                hover,
                &|_, p| p,
            )
        };

        // Gate closed: authored No with a GameData source present (C++
        // GlobalData.cpp:795 default stays FALSE only when authored).
        super::force_retail_game_data_probe_for_tests(1);
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().show_object_health = false;
        }
        assert!(pack(&snap, None).0.is_empty());

        // Gate open: outline (4 quads) + fill (1 quad) = 5 quads of 6 verts
        // x 6 floats = 180 floats for the selected unit alone.
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().show_object_health = true;
        }
        let (verts, selected_bars) = pack(&snap, None);
        assert_eq!(selected_bars, 1);
        assert_eq!(
            verts.len(),
            180,
            "outline 4 + fill 1 quads, 6 verts x 6 floats each"
        );
        // C++ Object::getHealthBoxPosition uses authored height, never bar width.
        let object = snap.objects.iter().find(|object| object.id == id).unwrap();
        let anchor = object.position + Vec3::Y * object.health_box_z_offset;
        let clip = proj * view * anchor.extend(1.0);
        let expected_top = (1.0 - (clip.y / clip.w * 0.5 + 0.5)) * 384.0 - 1.5;
        let actual_top = (1.0 - verts[13]) * 0.5 * 480.0;
        assert!(
            (actual_top - expected_top).abs() < 0.01,
            "bar must use frozen health-box height: got {actual_top}, expected {expected_top}"
        );
        // ratio 0.75 → red = 1−((0.75−0.5)/0.5) = 0.5, green = 1; the
        // not-DAMAGED modulation (`Drawable.cpp:3908-3912`) halves red and
        // averages green → fill (0.25, 1.0, 0).
        let fill = &verts[verts.len() - 36..];
        assert!((fill[2] - 0.25).abs() < 0.01, "fill red {}", fill[2]);
        assert!((fill[3] - 1.0).abs() < 0.01, "fill green {}", fill[3]);
        assert!(fill[4].abs() < 0.01, "health bars carry no blue");

        // Width follows C++ 1/zoom without changing the authored height.
        for zoom in [0.5, 1.0, 1.3] {
            let (packed, selected_bars) = pack_health_bar_quads(
                &snap,
                &view,
                &proj,
                (640.0, 384.0),
                (640.0, 480.0),
                zoom,
                None,
                &|_, p| p,
            );
            assert_eq!(selected_bars, 1);
            let width = (packed[6] - packed[0]) * 0.5 * 640.0;
            assert!((width - object.health_box_width / zoom).abs() < 0.01);
            let top = (1.0 - packed[13]) * 0.5 * 480.0;
            assert!((top - expected_top).abs() < 0.01);
        }
        // C++ IGNORED_IN_GUI dimensions are zero, and offscreen anchors fail
        // computeHealthRegion rather than painting partial bars at the edge.
        let mut no_bar = snap.clone();
        no_bar
            .objects
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .health_box_width = 0.0;
        assert!(pack(&no_bar, None).0.is_empty());
        let mut offscreen = snap.clone();
        offscreen
            .objects
            .iter_mut()
            .find(|o| o.id == id)
            .unwrap()
            .position
            .x += 10_000.0;
        assert!(pack(&offscreen, None).0.is_empty());
        // Contract (d): a hovered NON-selected unit gets its own bar quad set
        // (Drawable.cpp:3834-3835 moused-over branch).
        let (verts, selected_bars) = pack(&snap, Some(hover_id.0));
        assert_eq!(selected_bars, 1);
        assert_eq!(
            verts.len(),
            360,
            "selected + hovered units each pack 5 quads"
        );

        // Hover alone still shows one bar when nothing is selected.
        let mut hover_only = snap.clone();
        {
            let sel = hover_only
                .objects
                .iter_mut()
                .find(|o| o.id == id)
                .expect("selected row");
            sel.selected = false;
        }
        let (hover_vertices, selected_bars) = pack(&hover_only, Some(hover_id.0));
        assert_eq!(selected_bars, 0);
        assert_eq!(
            hover_vertices.len(),
            180,
            "hover-only unit keeps its bar"
        );

        // Clearing the mouseover (INVALID_DRAWABLE_ID) removes the hover bar.
        assert!(pack(&hover_only, None).0.is_empty());

        // Retail default: an install whose boot lacks the GameData block
        // falls open to the shipped `ShowObjectHealth = Yes`
        // (GameData.ini:25 / INIZH GameData.ini:31835).
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().show_object_health = false;
        }
        super::force_retail_game_data_probe_for_tests(2);
        assert_eq!(
            pack(&snap, None).0.len(),
            180,
            "absent GameData block must fail open to the retail default"
        );

        // Restore shared state for other tests.
        super::force_retail_game_data_probe_for_tests(0);
        if let Some(data) = game_engine::common::ini::get_global_data() {
            data.write().show_object_health = false;
        }
    }

    #[test]
    fn show_object_health_default_follows_retail_when_game_data_block_absent() {
        // Parsed Yes always wins; parsed No only survives a present GameData
        // source; an absent block (or absent global) fails open to the
        // retail `ShowObjectHealth = Yes` default.
        assert!(super::resolve_show_object_health(Some(true), false));
        assert!(super::resolve_show_object_health(Some(true), true));
        assert!(
            super::resolve_show_object_health(Some(false), false),
            "no GameData source → retail default Yes (GameData.ini:25)"
        );
        assert!(
            !super::resolve_show_object_health(Some(false), true),
            "authored No stays closed (Drawable.cpp:3834 gate)"
        );
        assert!(super::resolve_show_object_health(None, false));
        assert!(super::resolve_show_object_health(None, true));
    }
}

#[cfg(test)]
mod selection_shader_residual_tests {
    use super::{
        DRAG_RECT_COLOR, DragSelectRect, RMB_SCROLL_ANCHOR_DROP_COLOR,
        RMB_SCROLL_ANCHOR_MAIN_COLOR, RmbScrollAnchorOverlay, drag_rect_screen_vertices,
        rmb_scroll_anchor_screen_vertices,
    };
    use glam::Vec2;

    #[test]
    fn drag_marquee_is_a_two_pixel_screen_space_open_rectangle() {
        let vertices = drag_rect_screen_vertices(&DragSelectRect {
            start: Vec2::new(100.0, 50.0),
            end: Vec2::new(300.0, 150.0),
            window_width: 400.0,
            window_height: 200.0,
        });
        // Four border quads × six vertices × [clip_x, clip_y, rgba].
        assert_eq!(vertices.len(), 4 * 6 * 6);
        assert_eq!(&vertices[2..6], &DRAG_RECT_COLOR);
        assert!(vertices.iter().all(|value| value.is_finite()));

        // The first quad is the bottom 2px border.  In a 400×200 viewport,
        // its half-thickness is 0.005 NDC horizontally and 0.01 vertically.
        assert!((vertices[0] - -0.505).abs() < f32::EPSILON);
        assert!((vertices[1] - -0.51).abs() < f32::EPSILON);
        assert!((vertices[6] - 0.505).abs() < f32::EPSILON);
        assert!((vertices[13] - -0.49).abs() < f32::EPSILON);
    }

    #[test]
    fn drag_marquee_waits_for_mouse_drag_tolerance() {
        // C++ SelectionXlat.cpp:399-407 — per-axis > DragTolerance (default 5).
        let tiny = DragSelectRect {
            start: Vec2::new(100.0, 50.0),
            end: Vec2::new(104.0, 50.0),
            window_width: 400.0,
            window_height: 200.0,
        };
        assert!(!tiny.is_valid());
        assert!(drag_rect_screen_vertices(&tiny).is_empty());
        let ok = DragSelectRect {
            start: Vec2::new(100.0, 50.0),
            end: Vec2::new(106.0, 50.0),
            window_width: 400.0,
            window_height: 200.0,
        };
        assert!(ok.is_valid());
        assert!(!drag_rect_screen_vertices(&ok).is_empty());
    }

    #[test]
    fn rmb_scroll_anchor_matches_cpp_four_rect_green_black_cross() {
        let anchor = RmbScrollAnchorOverlay::from_active_rmb_scroll(
            true,
            true,
            Some((100.0, 125.0)),
            (400.0, 300.0),
        )
        .expect("active RMB scroll has an anchor");
        let vertices = rmb_scroll_anchor_screen_vertices(&anchor);

        // Four C++ fill rectangles × six vertices × [clip_x, clip_y, rgba].
        assert_eq!(vertices.len(), 4 * 6 * 6);
        assert_eq!(&vertices[2..6], &RMB_SCROLL_ANCHOR_DROP_COLOR);
        assert_eq!(&vertices[38..42], &RMB_SCROLL_ANCHOR_DROP_COLOR);
        assert_eq!(&vertices[74..78], &RMB_SCROLL_ANCHOR_MAIN_COLOR);
        assert_eq!(&vertices[110..114], &RMB_SCROLL_ANCHOR_MAIN_COLOR);

        // C++ first black horizontal rect: x=91, y=122, width=19, height=7.
        assert!((vertices[0] - -0.545).abs() < f32::EPSILON);
        assert!((vertices[1] - 0.14).abs() < f32::EPSILON);
        assert!((vertices[12] - -0.45).abs() < f32::EPSILON);
        assert!((vertices[13] - 0.186_666_67).abs() < 0.000_001);
    }

    #[test]
    fn rmb_scroll_anchor_requires_the_enabled_active_gesture() {
        let display_size = (400.0, 300.0);
        assert!(
            RmbScrollAnchorOverlay::from_active_rmb_scroll(
                false,
                true,
                Some((100.0, 125.0)),
                display_size,
            )
            .is_none()
        );
        assert!(
            RmbScrollAnchorOverlay::from_active_rmb_scroll(
                true,
                false,
                Some((100.0, 125.0)),
                display_size,
            )
            .is_none()
        );
        assert!(
            RmbScrollAnchorOverlay::from_active_rmb_scroll(true, true, None, display_size,)
                .is_none()
        );
        assert!(
            RmbScrollAnchorOverlay::from_active_rmb_scroll(
                true,
                true,
                Some((f32::NAN, 125.0)),
                display_size,
            )
            .is_none()
        );
    }

    /// hq-6gq7: world-space unit circles are the allowed wgpu representation
    /// change. C++ `W3DInGameUI::drawSelectionRegion` color `0x9933FF33` and
    /// 2px width stay on the 2D drag marquee; do not rewrite the overlay.
    #[test]
    fn selection_overlay_preserves_cpp_drag_color_and_documents_wgpu_circles() {
        assert_eq!(super::DRAG_RECT_COLOR, [0.2, 1.0, 0.2, 0.6]);
        assert_eq!(super::DRAG_RECT_LINE_WIDTH_PX, 2.0);
    }
}
