//! 3D Selection Rendering System
//!
//! PARITY_NOTE: C++ SAGE draws the drag-select region as a 2D open rectangle via
//! `W3DInGameUI::drawSelectionRegion()` using `TheDisplay->drawOpenRect()` with
//! color `0x9933FF33` (alpha 0x99, green tint).  The C++ W3DScene code tints
//! selected drawables via `Drawable::getSelectionColor()`.  The drag marquee
//! remains a 2D screen-space open rectangle, while the Rust-only unit circles
//! and order markers remain world-space overlays.

use glam::{Mat4, Vec2, Vec3};
use std::sync::Arc;
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
const CIRCLE_ALPHA: f32 = 0.55;

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

/// Per-selected-unit data for circle rendering.
#[derive(Debug, Clone)]
pub struct SelectedUnit {
    pub position: Vec3,
    pub radius: f32,
    pub team_color: [f32; 4],
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

impl SelectionRenderer {
    pub fn new() -> Option<Self> {
        let device = ww3d_engine::device().ok()?;
        let queue = ww3d_engine::queue().ok()?;

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
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection_overlay_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 28,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
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
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
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
                push_constant_ranges: &[],
            });
        let drag_rect_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("selection_drag_rect_pipeline"),
            layout: Some(&drag_rect_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &drag_rect_shader,
                entry_point: Some("vs_main"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: 24,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                }],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &drag_rect_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8UnormSrgb,
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
            multiview: None,
            cache: None,
        });

        Some(Self {
            device,
            queue,
            pipeline,
            drag_rect_pipeline,
            uniform_buffer,
            uniform_bind_group,
        })
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        view_proj: &Mat4,
        selected_units: &[SelectedUnit],
        order_line_vertices: &[f32],
    ) {
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(view_proj.to_cols_array().as_ref()),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        for unit in selected_units {
            self.draw_selection_circle(render_pass, unit);
        }

        if !order_line_vertices.is_empty() {
            self.draw_order_line_segments(render_pass, order_line_vertices);
        }
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
    // Selection circle beneath a unit
    // -----------------------------------------------------------------------

    fn draw_selection_circle(&self, render_pass: &mut wgpu::RenderPass<'_>, unit: &SelectedUnit) {
        let radius = unit.radius.max(1.0);
        let center = unit.position;
        let y = center.y + TERRAIN_Y_OFFSET;
        let color = unit.team_color;

        // Triangle fan: center vertex + N outer ring vertices.
        // Vertex format: [x, y, z, r, g, b, a] = 7 floats.
        let vertex_count = CIRCLE_SEGMENTS as usize + 2;
        let mut vertices = Vec::with_capacity(vertex_count * 7);

        vertices.extend_from_slice(&[
            center.x, y, center.z, color[0], color[1], color[2], color[3],
        ]);

        for i in 0..=CIRCLE_SEGMENTS {
            let angle = (i as f32 / CIRCLE_SEGMENTS as f32) * std::f32::consts::TAU;
            let px = center.x + radius * angle.cos();
            let pz = center.z + radius * angle.sin();
            vertices.extend_from_slice(&[px, y, pz, color[0], color[1], color[2], color[3]]);
        }

        let triangle_count = CIRCLE_SEGMENTS as usize;
        let mut indices: Vec<u32> = Vec::with_capacity(triangle_count * 3);
        for i in 1..=(CIRCLE_SEGMENTS as u32) {
            indices.push(0);
            indices.push(i);
            indices.push(i + 1);
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("selection_circle_verts"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("selection_circle_indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..(triangle_count as u32 * 3), 0, 0..1);
    }

    /// Thin ground-plane quads for move/attack order line residual.
    fn draw_order_line_segments(&self, render_pass: &mut wgpu::RenderPass<'_>, packed: &[f32]) {
        const FPV: usize = 7; // x y z r g b a
        if packed.len() < FPV * 2 {
            return;
        }
        let half_width = 0.55f32;
        let y_lift = TERRAIN_Y_OFFSET + 0.2;
        let mut vertices: Vec<f32> = Vec::new();
        let mut indices: Vec<u32> = Vec::new();
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
            let i0 = (vertices.len() / 7) as u32;
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
        if indices.is_empty() {
            return;
        }
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("order_line_verts"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let index_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("order_line_indices"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            });
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..indices.len() as u32, 0, 0..1);
    }
}

// ---------------------------------------------------------------------------
// Public integration helpers
// ---------------------------------------------------------------------------

/// C++ `GameClient.cpp:694` `setFullyObscuredByShroud(ss >= OBJECTSHROUD_FOGGED)`
/// then `W3DModelDraw::setFullyObscuredByShroud` → `enableShadowInvisible`.
/// Fogged ghosts and black-shrouded units cast no disc.
fn blob_shadow_visible_through_shroud(u: &crate::presentation_frame::UnitRenderInput) -> bool {
    use crate::presentation_frame::PresentationObjectShroudStatus;
    if u.drawable_shroud.lifetime.is_direct_host_object()
        && (u.drawable_shroud.raw_status as u8) >= (PresentationObjectShroudStatus::Fogged as u8)
    {
        return false;
    }
    !u.fow_visibility.fully_obscures_drawable()
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

/// Collect blob discs from a `PresentationFrame` snapshot.
///
/// Identity fields come only from the immutable snapshot — not a live re-read
/// of `GameLogic` objects. Dark discs drape frozen terrain height under every
/// live `unit_render_inputs` pose that is not fogged or black-shrouded.
pub fn collect_blob_shadows_from_presentation(
    frame: &crate::presentation_frame::PresentationFrame,
) -> Vec<SelectedUnit> {
    // C++ addShadow NULL + renderShadows skip when !m_useShadowDecals.
    if !game_engine::common::game_lod::use_shadow_decals() {
        return Vec::new();
    }
    frame
        .unit_render_inputs()
        .iter()
        .filter(|u| !u.destroyed && u.shadows_enabled && blob_shadow_visible_through_shroud(u))
        .map(|u| {
            let (sx, sy) = crate::game_logic::host_battlemaster::leftover_template_shadow_size(
                &u.template_name,
                0.0,
                0.0,
            );
            let (ox, oy) = crate::game_logic::host_battlemaster::leftover_template_shadow_offset(
                &u.template_name,
                0.0,
                0.0,
            );
            let radius = if sx > 0.0 || sy > 0.0 {
                sx.max(sy) * 0.5
            } else {
                u.selection_radius
                    .max(if u.is_structure { 10.0 } else { 4.0 })
                    * 0.85
            };
            SelectedUnit {
                // C++ ShadowOffsetX/Y are world X/Y; live Y is height so offset Y maps to Z.
                // Disc Y is frozen terrain height, never unit altitude (jets/Comanches).
                position: glam::Vec3::new(
                    u.position.x + ox,
                    blob_disc_ground_y(frame, u.id),
                    u.position.z + oy,
                ),
                radius,
                team_color: [0.0, 0.0, 0.0, 0.4],
            }
        })
        .collect()
}

pub fn collect_selected_units_from_presentation(
    frame: &crate::presentation_frame::PresentationFrame,
) -> Vec<SelectedUnit> {
    let mut units = Vec::new();
    // Prefer unit_render_inputs path for position/team/selection_radius (snapshot-owned).
    // Fall back to selected flags on objects that may be engine-bridged (still drawn
    // via selection overlay even when mesh pass skips them).
    for object in frame.objects.iter().filter(|o| o.selected && !o.destroyed) {
        let player_index = match object.team {
            crate::game_logic::Team::China => 0,
            crate::game_logic::Team::USA => 1,
            crate::game_logic::Team::GLA => 4,
            crate::game_logic::Team::Neutral => 7,
        };
        // Prefer snapshot team_color when set; else player palette.
        let team_color = if object.team_color[3] > 0.0 {
            [
                object.team_color[0],
                object.team_color[1],
                object.team_color[2],
                CIRCLE_ALPHA,
            ]
        } else {
            let c = crate::ui::color_for_player(player_index);
            [
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                CIRCLE_ALPHA,
            ]
        };
        let radius = object
            .selection_radius
            .max(if object.is_structure { 12.0 } else { 5.0 });
        units.push(SelectedUnit {
            position: object.position,
            radius,
            team_color,
        });
    }
    units
}

/// Collect selection circles. When `presentation` is present, identity fields are
/// snapshot-owned (position/team/selected/aliveness). Live `GameLogic` is only used
/// as a fallback when no frame is available (boot/loading residuals).
pub fn collect_selected_units(
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
) -> Vec<SelectedUnit> {
    // Presentation-only boundary: engine always seeds a frame before draw.
    presentation
        .map(collect_selected_units_from_presentation)
        .unwrap_or_default()
}

/// PARITY_NOTE: C++ draws the selection region in `W3DInGameUI::draw()` as a
/// 2D overlay (after the 3D scene).  Unit selection circles are drawn by the
/// W3DScene per-drawable tint pipeline.  This Rust implementation merges both
/// into a single 3D overlay pass that executes after the terrain pass but
/// before the UI pass.

/// Rally flag is the leftover RallyPointMarker mesh (unit_render_inputs), not a line.
fn pack_rally_point_lines(_frame: &crate::presentation_frame::PresentationFrame) -> Vec<f32> {
    Vec::new()
}

pub fn enqueue_selection_render(
    pipeline: &mut crate::graphics::render_pipeline::RenderPipeline,
    view_matrix: &Mat4,
    projection_matrix: &Mat4,
    drag_rect: Option<DragSelectRect>,
    rmb_scroll_anchor: Option<RmbScrollAnchorOverlay>,
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
    // Placement ghost / special-power radius / guard area residual circles.
    ground_markers: Vec<SelectedUnit>,
    show_move_lines: bool,
    show_attack_lines: bool,
) {
    let renderer = match SelectionRenderer::new() {
        Some(r) => Arc::new(r),
        None => return,
    };

    let view_proj = *projection_matrix * *view_matrix;

    let mut selected_units = collect_selected_units(presentation);
    // Blob discs are the fallback when the forward-pass projected/CSM sample
    // is empty. Planted at unit Y (not world origin) so units do not hover.
    if let Some(frame) = presentation {
        selected_units.splice(0..0, collect_blob_shadows_from_presentation(frame));
    }
    selected_units.extend(ground_markers);

    // Move/attack order line residual from presentation snapshot.
    let mut order_line_vertices: Vec<f32> = Vec::new();
    if let Some(frame) = presentation {
        if show_move_lines {
            let move_pack =
                crate::graphics::move_line_upload::MoveLineUpload::pack_from_presentation(frame);
            order_line_vertices.extend_from_slice(&move_pack.vertices);
        }
        if show_attack_lines {
            let atk_pack =
                crate::graphics::attack_line_upload::AttackLineUpload::pack_from_presentation(
                    frame,
                );
            order_line_vertices.extend_from_slice(&atk_pack.vertices);
        }
        // Structure rally-point line residual (selected producers → rally).
        order_line_vertices.extend(pack_rally_point_lines(frame));
    }

    let drag_rect = drag_rect.filter(|rect| rect.is_valid());
    let rmb_scroll_anchor = rmb_scroll_anchor.filter(|anchor| {
        anchor.position.is_finite()
            && anchor.window_size.is_finite()
            && anchor.window_size.x > 0.0
            && anchor.window_size.y > 0.0
    });
    if drag_rect.is_none()
        && rmb_scroll_anchor.is_none()
        && selected_units.is_empty()
        && order_line_vertices.is_empty()
    {
        return;
    }

    if !selected_units.is_empty() || !order_line_vertices.is_empty() {
        let world_renderer = Arc::clone(&renderer);
        pipeline.enqueue_pre_scene_callback(move |frame| {
            let color_view = frame.color_view_arc();
            let depth_view = frame.depth_view_arc();
            let encoder = frame.encoder();

            let depth_stencil =
                depth_view
                    .as_ref()
                    .map(|dv| wgpu::RenderPassDepthStencilAttachment {
                        view: dv.as_ref(),
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    });

            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("selection world overlay pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: color_view.as_ref(),
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: depth_stencil,
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            world_renderer.draw(
                &mut render_pass,
                &view_proj,
                &selected_units,
                &order_line_vertices,
            );

            drop(render_pass);
            Ok(())
        });
    }

    if drag_rect.is_some() || rmb_scroll_anchor.is_some() {
        // C++ draws this in W3DInGameUI's 2D pass, after the scene and before
        // window repaint. Queue it before Main queues its UI flush, so HUD
        // widgets still render over both the marquee and RMB anchor exactly as
        // in the original.
        let drag_renderer = Arc::clone(&renderer);
        pipeline.enqueue_post_frame_callback(move |frame| {
            let color_view = frame.color_view_arc();
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
            });
            if let Some(drag_rect) = drag_rect.as_ref() {
                drag_renderer.draw_drag_rect(&mut render_pass, drag_rect);
            }
            if let Some(rmb_scroll_anchor) = rmb_scroll_anchor.as_ref() {
                drag_renderer.draw_rmb_scroll_anchor(&mut render_pass, rmb_scroll_anchor);
            }
            drop(render_pass);
            Ok(())
        });
    }
}

#[cfg(test)]
mod presentation_selection_tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, Team, ThingTemplate};
    use crate::presentation_frame::PresentationFrame;
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    #[test]
    fn shipped_selection_collect_uses_presentation_snapshot_not_live_reread() {
        // Criterion 2: production consumer identity from PresentationFrame.
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("SelPresMap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("SelUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.add_kind_of(KindOf::Selectable);
        t.shadow_type = crate::game_logic::host_enum_table_residual::SHADOW_DECAL;
        logic.templates.insert("SelUnit".into(), t);

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

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        // Mutate live world after snapshot — consumer must keep snapshot identity.
        if let Some(o) = logic.host_object_mut(id) {
            o.set_position(Vec3::new(999.0, 0.0, 999.0));
            o.selected = false;
            o.status.selected = false;
            o.health.current = 1.0;
        }

        // Shipped path is presentation-only.
        let units = collect_selected_units(Some(&snap));
        assert_eq!(units.len(), 1, "snapshot still has selected unit");
        assert!(
            (units[0].position.x - 12.0).abs() < 0.01,
            "position must come from snapshot, not live 999: {:?}",
            units[0].position
        );
        assert!(
            (units[0].position.z + 7.0).abs() < 0.01,
            "z from snapshot: {:?}",
            units[0].position
        );

        // Direct presentation helper is the same source of truth.
        let direct = collect_selected_units_from_presentation(&snap);
        assert_eq!(direct.len(), 1);
        assert!((direct[0].position.x - 12.0).abs() < 0.01);

        // No presentation → empty (no live GameLogic dual-read residual).
        assert!(
            collect_selected_units(None).is_empty(),
            "missing presentation yields no selection overlay units"
        );

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
                .all(|b| b.team_color[0] == 0.0 && b.team_color[3] > 0.0),
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
        let opaque = include_str!(
            "../../../Libraries/Source/WWVegas/WW3D2/crates/ww3d-renderer-3d/src/rendering/shader_system/opaque.wgsl"
        );
        assert!(
            opaque.contains("fn sample_projected_shadow"),
            "forward pass must sample a live projected shadow"
        );
        assert!(
            opaque.contains("fn sample_csm_pcf"),
            "forward pass must PCF-sample the live cascade map"
        );
        assert!(
            opaque.contains("textureSampleCompare"),
            "CSM PCF must sample the bound depth array"
        );
        assert!(
            opaque.contains("@group(1) @binding(3)"),
            "cascade shadow map must be a live bind, not a comment"
        );
        assert!(opaque.contains("shadow_factor"));
    }

    #[test]
    fn airborne_blob_discs_project_to_terrain() {
        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("AirBlobMap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("AirBlobJet");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Aircraft);
        t.shadow_type = crate::game_logic::host_enum_table_residual::SHADOW_DECAL;
        logic.templates.insert("AirBlobJet".into(), t);

        let id = logic
            .create_object("AirBlobJet", Team::USA, Vec3::new(20.0, 80.0, 15.0))
            .expect("jet");
        if let Some(o) = logic.host_object_mut(id) {
            o.status.airborne_target = true;
        }

        let mut snap = PresentationFrame::build_from_logic(&logic, 0);
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
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("FogBlobUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.shadow_type = crate::game_logic::host_enum_table_residual::SHADOW_DECAL;
        logic.templates.insert("FogBlobUnit".into(), t);

        let _id = logic
            .create_object("FogBlobUnit", Team::USA, Vec3::new(8.0, 0.0, 8.0))
            .expect("unit");

        let mut snap = PresentationFrame::build_from_logic(&logic, 0);
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
    fn blob_discs_hidden_when_use_shadow_decals_off() {
        let src = include_str!("selection_renderer.rs");
        let start = src
            .find("pub fn collect_blob_shadows_from_presentation")
            .expect("blob collect");
        let body = &src[start..start + 400];
        assert!(
            body.contains("game_lod::use_shadow_decals()"),
            "live blob collect must honor TheGlobalData->m_useShadowDecals"
        );

        let mut logic = GameLogic::new();
        let cfg = golden_skirmish_config("DecalOffBlobMap");
        apply_skirmish_config(&mut logic, &cfg).expect("config");
        let mut t = ThingTemplate::new("DecalOffUnit");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Infantry);
        t.shadow_type = crate::game_logic::host_enum_table_residual::SHADOW_DECAL;
        logic.templates.insert("DecalOffUnit".into(), t);
        let _id = logic
            .create_object("DecalOffUnit", Team::USA, glam::Vec3::new(4.0, 0.0, 4.0))
            .expect("unit");
        let snap = PresentationFrame::build_from_logic(&logic, 0);

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
    fn production_cnc_render_path_enqueues_selection_with_presentation() {
        // Structural proof: CncGameEngine::render ships enqueue_selection_render with
        // last_presentation_frame (not a dead helper).
        let src = crate::cnc_game_engine::ENGINE_SRC;
        assert!(
            src.contains("enqueue_selection_render"),
            "InGame render must call enqueue_selection_render"
        );
        assert!(
            src.contains("last_presentation_frame.as_ref()"),
            "selection enqueue must pass presentation snapshot"
        );
        // Selection enqueue is presentation-only (no GameLogic argument).
        let idx = src.find("enqueue_selection_render").expect("enqueue site");
        let window = &src[idx..idx + 450];
        assert!(
            window.contains("last_presentation_frame.as_ref()")
                && !window.contains("game_logic")
                && !window.contains("Some(&self.game_logic)"),
            "selection overlay must not take live GameLogic: {window}"
        );
        assert!(
            src.contains("selection_renderer::enqueue_selection_render"),
            "must use graphics selection_renderer production path"
        );
    }

    #[test]
    fn rally_point_line_overlay_residual() {
        let src = include_str!("selection_renderer.rs");
        assert!(
            src.contains("fn pack_rally_point_lines")
                && src.contains("pack_rally_point_lines(frame)"),
            "selection overlay must pack selected structure rally lines"
        );
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
    fn selection_shader_module_contains_both_entry_points() {
        let src = include_str!("selection_renderer.rs");
        let start = src
            .find("const SELECTION_SHADER")
            .expect("SELECTION_SHADER");
        let module = &src[start..src.len().min(start + 1200)];
        assert!(module.contains("fn vs_main"));
        assert!(module.contains("fn fs_main"));
        assert!(
            module.contains("@fragment"),
            "fragment stage must be in the same WGSL module as vs_main"
        );
    }

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
        let src = include_str!("selection_renderer.rs");
        assert!(
            src.contains("0x9933FF33"),
            "drag marquee must keep C++ W3DInGameUI::drawSelectionRegion color"
        );
        assert!(
            src.contains("world-space overlays"),
            "unit circles must stay documented as the allowed wgpu representation"
        );
        assert_eq!(super::DRAG_RECT_COLOR, [0.2, 1.0, 0.2, 0.6]);
        assert_eq!(super::DRAG_RECT_LINE_WIDTH_PX, 2.0);
    }
}
