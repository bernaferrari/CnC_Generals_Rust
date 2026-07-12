//! 3D Selection Rendering System
//!
//! PARITY_NOTE: C++ SAGE draws the drag-select region as a 2D open rectangle via
//! `W3DInGameUI::drawSelectionRegion()` using `TheDisplay->drawOpenRect()` with
//! color `0x9933FF33` (alpha 0x99, green tint).  The C++ W3DScene code tints
//! selected drawables via `Drawable::getSelectionColor()`.  This Rust implementation
//! projects the drag rectangle into 3D world space on the terrain plane (standard RTS
//! approach) and draws per-unit selection circles using team colors from
//! `color_for_player()`.

use glam::{Mat4, Vec2, Vec3, Vec4};
use log::trace;
use std::sync::Arc;
use wgpu::util::DeviceExt;

// ---------------------------------------------------------------------------
// Constants (C++ parity)
// ---------------------------------------------------------------------------

/// PARITY_NOTE: C++ `W3DInGameUI::drawSelectionRegion()` uses color `0x9933FF33`.
/// Alpha = 0x99 ≈ 0.6 but we use a lower alpha for the 3D projected quad to avoid
/// obscuring terrain detail.
const DRAG_RECT_COLOR: [f32; 4] = [0.2, 1.0, 0.2, 0.3];

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
        let dx = (self.end.x - self.start.x).abs();
        let dy = (self.end.y - self.start.y).abs();
        dx > 2.0 || dy > 2.0
    }
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

const SELECTION_VERTEX_SHADER: &str = r"
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
";

const SELECTION_FRAGMENT_SHADER: &str = r"
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
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl SelectionRenderer {
    pub fn new() -> Option<Self> {
        let device = ww3d_engine::device().ok()?;
        let queue = ww3d_engine::queue().ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("selection_overlay_shader"),
            source: wgpu::ShaderSource::Wgsl(SELECTION_VERTEX_SHADER.into()),
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

        Some(Self {
            device,
            queue,
            pipeline,
            uniform_buffer,
            uniform_bind_group,
        })
    }

    pub fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        view_proj: &Mat4,
        inv_view_proj: &Mat4,
        drag_rect: Option<&DragSelectRect>,
        selected_units: &[SelectedUnit],
    ) {
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(view_proj.to_cols_array().as_ref()),
        );

        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        if let Some(rect) = drag_rect {
            if rect.is_valid() {
                self.draw_drag_rect(render_pass, inv_view_proj, rect);
            }
        }

        for unit in selected_units {
            self.draw_selection_circle(render_pass, unit);
        }
    }

    // -----------------------------------------------------------------------
    // Drag rectangle projected onto the XZ plane
    // -----------------------------------------------------------------------

    fn draw_drag_rect(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        inv_view_proj: &Mat4,
        rect: &DragSelectRect,
    ) {
        // Screen corners in pixel coords (top-left origin).
        let corners_screen: [(f32, f32); 4] = [
            (rect.start.x, rect.start.y),
            (rect.end.x, rect.start.y),
            (rect.end.x, rect.end.y),
            (rect.start.x, rect.end.y),
        ];

        let w = rect.window_width.max(1.0);
        let h = rect.window_height.max(1.0);

        // Project each screen corner to world space via ray-plane intersection.
        // NDC: X in [-1,1], Y in [-1,1] (Y flipped from screen space).
        // Then unproject near/far clip points, build ray, intersect Y=0 plane.
        let mut world_corners: [Vec3; 4] = [Vec3::ZERO; 4];
        for (i, (sx, sy)) in corners_screen.iter().enumerate() {
            // NDC: X in [-1,1], Y in [-1,1] (Y flipped from screen space).
            let ndc_x = (*sx / w) * 2.0 - 1.0;
            let ndc_y = 1.0 - (*sy / h) * 2.0;

            let near_clip = Vec4::new(ndc_x, ndc_y, 0.0, 1.0);
            let far_clip = Vec4::new(ndc_x, ndc_y, 1.0, 1.0);

            let near_world = *inv_view_proj * near_clip;
            let far_world = *inv_view_proj * far_clip;

            // Perspective divide: w may be negative for behind-camera points.
            let near_w = near_world.w.abs().max(1e-6);
            let far_w = far_world.w.abs().max(1e-6);
            let near_pt = Vec3::new(
                near_world.x / near_w,
                near_world.y / near_w,
                near_world.z / near_w,
            );
            let far_pt = Vec3::new(
                far_world.x / far_w,
                far_world.y / far_w,
                far_world.z / far_w,
            );

            let ray_dir = far_pt - near_pt;

            // Intersect ray with Y=0 plane: t = -near_pt.y / ray_dir.y
            if ray_dir.y.abs() < 1e-6 {
                world_corners[i] = near_pt;
                continue;
            }
            let t = -near_pt.y / ray_dir.y;
            let hit = near_pt + ray_dir * t;
            world_corners[i] = Vec3::new(hit.x, TERRAIN_Y_OFFSET, hit.z);
        }

        let indices: [usize; 6] = [0, 1, 2, 0, 2, 3];
        let vertices: Vec<f32> = indices
            .iter()
            .flat_map(|&idx| {
                let p = world_corners[idx];
                [
                    p.x,
                    p.y,
                    p.z,
                    DRAG_RECT_COLOR[0],
                    DRAG_RECT_COLOR[1],
                    DRAG_RECT_COLOR[2],
                    DRAG_RECT_COLOR[3],
                ]
            })
            .collect();

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("selection_drag_rect_verts"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..6, 0..1);

        trace!("Drew 3D drag-select rect: corners={:?}", world_corners);
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
}

// ---------------------------------------------------------------------------
// Public integration helpers
// ---------------------------------------------------------------------------

/// Collect selection circles from a `PresentationFrame` snapshot (preferred production path).
///
/// Identity fields (position/team/selected/aliveness) come only from the immutable
/// snapshot — not a live re-read of `GameLogic` objects.
pub fn collect_selected_units_from_presentation(
    frame: &crate::presentation_frame::PresentationFrame,
) -> Vec<SelectedUnit> {
    let mut units = Vec::new();
    for object in frame.objects.iter().filter(|o| o.selected && !o.destroyed) {
        let player_index = match object.team {
            crate::game_logic::Team::China => 0,
            crate::game_logic::Team::USA => 1,
            crate::game_logic::Team::GLA => 4,
            crate::game_logic::Team::Neutral => 7,
        };
        let c = crate::ui::color_for_player(player_index);
        let radius = if object.is_structure { 12.0 } else { 8.0 };
        units.push(SelectedUnit {
            position: object.position,
            radius,
            team_color: [
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                CIRCLE_ALPHA,
            ],
        });
    }
    units
}

/// Collect selection circles. When `presentation` is present, identity fields are
/// snapshot-owned (position/team/selected/aliveness). Live `GameLogic` is only used
/// as a fallback when no frame is available (boot/loading residuals).
pub fn collect_selected_units(
    game_logic: &crate::game_logic::GameLogic,
    _local_player_id: u32,
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
) -> Vec<SelectedUnit> {
    if let Some(frame) = presentation {
        return collect_selected_units_from_presentation(frame);
    }

    let mut units = Vec::new();

    for object in game_logic.get_objects().values() {
        if !object.is_alive() || !object.selected {
            continue;
        }

        let color = if object.team_color[3] > 0.0 {
            object.team_color
        } else {
            let player_index = game_logic
                .get_players()
                .values()
                .find(|player| player.team == object.team)
                .map(|player| player.id as u8)
                .unwrap_or_else(|| match object.team {
                    crate::game_logic::Team::China => 0,
                    crate::game_logic::Team::USA => 1,
                    crate::game_logic::Team::GLA => 4,
                    crate::game_logic::Team::Neutral => 7,
                });
            let c = crate::ui::color_for_player(player_index);
            [
                c.r as f32 / 255.0,
                c.g as f32 / 255.0,
                c.b as f32 / 255.0,
                CIRCLE_ALPHA,
            ]
        };

        units.push(SelectedUnit {
            position: object.get_position(),
            radius: object.selection_radius.max(5.0),
            team_color: color,
        });
    }

    units
}

/// PARITY_NOTE: C++ draws the selection region in `W3DInGameUI::draw()` as a
/// 2D overlay (after the 3D scene).  Unit selection circles are drawn by the
/// W3DScene per-drawable tint pipeline.  This Rust implementation merges both
/// into a single 3D overlay pass that executes after the terrain pass but
/// before the UI pass.
pub fn enqueue_selection_render(
    pipeline: &mut crate::graphics::render_pipeline::RenderPipeline,
    view_matrix: &Mat4,
    projection_matrix: &Mat4,
    game_logic: &crate::game_logic::GameLogic,
    drag_rect: Option<DragSelectRect>,
    local_player_id: u32,
    presentation: Option<&crate::presentation_frame::PresentationFrame>,
) {
    let renderer = match SelectionRenderer::new() {
        Some(r) => Arc::new(r),
        None => return,
    };

    let view_proj = *projection_matrix * *view_matrix;
    let inv_view_proj = view_proj.inverse();

    let selected_units = collect_selected_units(game_logic, local_player_id, presentation);

    if drag_rect.is_none() && selected_units.is_empty() {
        return;
    }

    let drag_rect_owned = drag_rect;

    pipeline.enqueue_pre_scene_callback(move |frame| {
        let color_view = frame.color_view_arc();
        let depth_view = frame.depth_view_arc();
        let encoder = frame.encoder();

        let depth_stencil = depth_view
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
            label: Some("selection overlay pass"),
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

        renderer.draw(
            &mut render_pass,
            &view_proj,
            &inv_view_proj,
            drag_rect_owned.as_ref(),
            &selected_units,
        );

        drop(render_pass);
        Ok(())
    });
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
        logic.templates.insert("SelUnit".into(), t);
        let id = logic
            .create_object("SelUnit", Team::USA, Vec3::new(12.0, 0.0, -7.0))
            .expect("unit");
        if let Some(o) = logic.get_object_mut(id) {
            o.selected = true;
            o.status.selected = true;
            o.selection_radius = 9.0;
        }
        if let Some(p) = logic.get_player_mut(0) {
            p.selected_objects = vec![id];
        }

        let snap = PresentationFrame::build_from_logic(&logic, 0);
        // Mutate live world after snapshot — consumer must keep snapshot identity.
        if let Some(o) = logic.get_object_mut(id) {
            o.set_position(Vec3::new(999.0, 0.0, 999.0));
            o.selected = false;
            o.status.selected = false;
            o.health.current = 1.0;
        }

        // Shipped path with presentation prefers snapshot.
        let units = collect_selected_units(&logic, 0, Some(&snap));
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

        // Without presentation, live re-read would see deselected unit (empty).
        let live_fallback = collect_selected_units(&logic, 0, None);
        assert!(
            live_fallback.is_empty(),
            "live path reflects post-snapshot mutation (deselected)"
        );
    }

    #[test]
    fn production_cnc_render_path_enqueues_selection_with_presentation() {
        // Structural proof: CncGameEngine::render ships enqueue_selection_render with
        // last_presentation_frame (not a dead helper).
        let src = include_str!("../cnc_game_engine.rs");
        assert!(
            src.contains("enqueue_selection_render"),
            "InGame render must call enqueue_selection_render"
        );
        assert!(
            src.contains("last_presentation_frame.as_ref()"),
            "selection enqueue must pass presentation snapshot"
        );
        assert!(
            src.contains("selection_renderer::enqueue_selection_render"),
            "must use graphics selection_renderer production path"
        );
    }
}
