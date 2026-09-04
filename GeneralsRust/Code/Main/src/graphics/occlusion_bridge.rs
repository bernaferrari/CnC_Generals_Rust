//! Live-path occluded-player color overlay.
//!
//! C++ `RTS3DScene::flagOccludedObjects` (`W3DScene.cpp:226-286`) ray-tests
//! score units against structure spheres, then
//! `flushOccludedObjects` / `renderStenciledPlayerColor` (`:1206-1291`,
//! `:1509-1573`) paint those pixels in scaled player color. Main previously
//! never consumed `w3d::scene` classification. This module classifies from
//! the frozen presentation roster and issues a depth-Greater player-color
//! silhouette after the 3D scene — a simplified flat overlay closer to ZH
//! than an unused crate pass.

use crate::game_logic::ObjectId;
use crate::graphics::render_pipeline::RenderPipeline;
use crate::presentation_frame::{PresentationFrame, RenderableObject};
use glam::{Mat4, Vec3};
use std::collections::BTreeMap;
use std::sync::Arc;
use wgpu::util::DeviceExt;

/// C++ `MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS` (`W3DScene.cpp:1301`).
pub const MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS: usize = 512;
/// C++ `NUMBER_PLAYER_COLOR_BITS` (`W3DScene.cpp:1180`).
const NUMBER_PLAYER_COLOR_BITS: u32 = 4;
/// C++ `GlobalData::m_occludedLuminanceScale` default (`GlobalData.cpp:746`).
pub const OCCLUDED_LUMINANCE_SCALE: f32 = 0.5;
/// C++ `Convert_Color(rgb, 0.5f)` alpha (`W3DScene.cpp:1383`).
pub const OCCLUDED_COLOR_ALPHA: f32 = 0.5;

/// One unit that should receive the behind-building player-color marker.
#[derive(Debug, Clone, PartialEq)]
pub struct OccludedPlayerOverlay {
    pub object_id: ObjectId,
    pub position: Vec3,
    pub radius: f32,
    pub color: [f32; 4],
    pub player_index: usize,
    pub color_index: usize,
    pub stencil_ref: u32,
}

/// Presentation facts needed to classify occluders / occludees.
#[derive(Debug, Clone, Copy)]
pub struct OcclusionCandidate {
    pub id: ObjectId,
    pub position: Vec3,
    pub radius: f32,
    pub is_structure: bool,
    pub is_occludee: bool,
    pub destroyed: bool,
    pub player_index: usize,
    pub team_color: [f32; 4],
}

impl OcclusionCandidate {
    pub fn from_renderable(object: &RenderableObject, current_frame: u32) -> Self {
        let is_structure = object.is_structure;
        Self {
            id: object.id,
            position: object.position,
            radius: object
                .selection_radius
                .max(if is_structure { 12.0 } else { 5.0 }),
            is_structure,
            // C++ requires `getObject()` plus SCORE / SCORE_CREATE /
            // SCORE_DESTROY / MP_COUNT_FOR_VICTORY. Live host KindOf has no
            // Score bit; combat units are the score-kind roster.
            // C++ W3DScene.cpp:474-475: getSafeOcclusionFrame() <= currentFrame.
            is_occludee: !is_structure
                && (object.is_unit || object.is_mobile)
                && object.safe_occlusion_frame <= current_frame,
            destroyed: object.destroyed,
            player_index: object
                .owner_player_id
                .map(|id| id as usize)
                .unwrap_or_else(|| match object.team {
                    crate::game_logic::Team::China => 0,
                    crate::game_logic::Team::USA => 1,
                    crate::game_logic::Team::GLA => 4,
                    crate::game_logic::Team::Neutral => 7,
                }),
            team_color: object.team_color,
        }
    }
}

/// C++ `playerIndexToColorIndex` (`W3DScene.cpp:1181-1202`).
pub fn player_index_to_color_index(player_index: usize) -> usize {
    let mut result = 0usize;
    for bit in 0..NUMBER_PLAYER_COLOR_BITS {
        let flipped = NUMBER_PLAYER_COLOR_BITS - 1 - bit;
        if (player_index & (1usize << bit)) != 0 {
            result |= 1usize << flipped;
        }
    }
    result
}

/// RGB luminance scale used by the crate `scale_argb_luminance` stand-in for
/// C++ HSV-V `m_occludedLuminanceScale` (`W3DScene.cpp:1380-1382`).
pub fn scale_player_color_luminance(rgb: [f32; 3], scale: f32) -> [f32; 3] {
    [
        (rgb[0] * scale).clamp(0.0, 1.0),
        (rgb[1] * scale).clamp(0.0, 1.0),
        (rgb[2] * scale).clamp(0.0, 1.0),
    ]
}

/// Graphics Gems I p388 sphere reject used by C++ `flagOccludedObjects`
/// (`W3DScene.cpp:250-265`), then a segment hit in lieu of `Cast_Ray`.
pub fn ray_sphere_occludes(
    camera: Vec3,
    target: Vec3,
    sphere_center: Vec3,
    sphere_radius: f32,
) -> bool {
    let to_target = target - camera;
    let max_distance = to_target.length();
    if max_distance <= f32::EPSILON {
        return false;
    }
    let direction = to_target / max_distance;
    let sphere_vector = sphere_center - camera;
    let alpha = sphere_vector.dot(direction);
    let beta = sphere_radius * sphere_radius - (sphere_vector.dot(sphere_vector) - alpha * alpha);
    if beta < 0.0 {
        return false;
    }
    let distance = alpha - beta.sqrt();
    distance >= 0.0 && distance < max_distance
}

/// Classify occluded score-units behind structures.
///
/// Mirrors `RTS3DScene::flagOccludedObjects` plus the per-player bucket cap
/// and stencil-ref layout from `flushOccludedObjectsIntoStencil`.
pub fn classify_occluded_player_overlays(
    camera_position: Vec3,
    candidates: &[OcclusionCandidate],
) -> Vec<OccludedPlayerOverlay> {
    let occluders: Vec<&OcclusionCandidate> = candidates
        .iter()
        .filter(|c| c.is_structure && !c.destroyed)
        .collect();
    if occluders.is_empty() {
        return Vec::new();
    }

    let mut buckets: BTreeMap<usize, Vec<&OcclusionCandidate>> = BTreeMap::new();
    for candidate in candidates {
        if candidate.destroyed || !candidate.is_occludee {
            continue;
        }
        let occluded = occluders.iter().any(|occluder| {
            ray_sphere_occludes(
                camera_position,
                candidate.position,
                occluder.position,
                occluder.radius,
            )
        });
        if !occluded {
            continue;
        }
        let bucket = buckets.entry(candidate.player_index).or_default();
        if bucket.len() < MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS {
            bucket.push(candidate);
        }
    }

    let mut overlays = Vec::new();
    let mut visible_player_colors = 0usize;
    for (player_index, units) in buckets {
        visible_player_colors += 1;
        let color_index = player_index_to_color_index(visible_player_colors);
        let stencil_ref = ((color_index as u32) << 3) | 0x80;
        for unit in units {
            let rgb = scale_player_color_luminance(
                [unit.team_color[0], unit.team_color[1], unit.team_color[2]],
                OCCLUDED_LUMINANCE_SCALE,
            );
            overlays.push(OccludedPlayerOverlay {
                object_id: unit.id,
                position: unit.position,
                radius: unit.radius,
                color: [rgb[0], rgb[1], rgb[2], OCCLUDED_COLOR_ALPHA],
                player_index,
                color_index,
                stencil_ref,
            });
        }
    }
    overlays
}

pub fn classify_from_presentation(
    camera_position: Vec3,
    frame: &PresentationFrame,
) -> Vec<OccludedPlayerOverlay> {
    let current_frame = frame.frame.0;
    let candidates: Vec<OcclusionCandidate> = frame
        .objects
        .iter()
        .map(|object| OcclusionCandidate::from_renderable(object, current_frame))
        .collect();
    classify_occluded_player_overlays(camera_position, &candidates)
}

const OVERLAY_SHADER: &str = r"
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

struct OcclusionOverlayRenderer {
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    pipeline: wgpu::RenderPipeline,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
}

impl OcclusionOverlayRenderer {
    fn new() -> Option<Self> {
        let device = ww3d_engine::device().ok()?;
        let queue = ww3d_engine::queue().ok()?;
        let depth_format = ww3d_engine::depth_format()
            .ok()
            .flatten()
            .unwrap_or(wgpu::TextureFormat::Depth32Float);

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("occluded_player_color_shader"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("occluded_player_view_proj_ubo"),
            size: 64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("occluded_player_uniform_bgl"),
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
            label: Some("occluded_player_uniform_bg"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("occluded_player_pipeline_layout"),
            bind_group_layouts: &[&uniform_bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("occluded_player_color_pipeline"),
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
            // Depth Greater: fragment passes only where it is *behind* already
            // written scene depth — the ZH behind-building silhouette.
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Greater,
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

    fn draw(
        &self,
        render_pass: &mut wgpu::RenderPass<'_>,
        view_proj: &Mat4,
        camera_position: Vec3,
        overlays: &[OccludedPlayerOverlay],
    ) {
        if overlays.is_empty() {
            return;
        }
        self.queue.write_buffer(
            &self.uniform_buffer,
            0,
            bytemuck::cast_slice(&view_proj.to_cols_array()),
        );
        let mut vertices: Vec<f32> = Vec::with_capacity(overlays.len() * 6 * 7);
        for overlay in overlays {
            push_unit_proxy_box(
                &mut vertices,
                overlay.position,
                camera_position,
                overlay.radius,
                overlay.color,
            );
        }
        if vertices.is_empty() {
            return;
        }
        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("occluded_player_color_verts"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
        render_pass.set_pipeline(&self.pipeline);
        render_pass.set_bind_group(0, Some(&self.uniform_bind_group), &[]);
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.draw(0..(vertices.len() / 7) as u32, 0..1);
    }
}

/// Vertical camera-facing quad standing on the unit pose. Depth-Greater then
/// reveals the player color only where a closer building already won the
/// depth test.
fn push_unit_proxy_box(
    vertices: &mut Vec<f32>,
    position: Vec3,
    camera: Vec3,
    radius: f32,
    color: [f32; 4],
) {
    let to_cam = camera - position;
    let to_cam_xz = Vec3::new(to_cam.x, 0.0, to_cam.z);
    let forward = if to_cam_xz.length_squared() < 1e-6 {
        Vec3::Z
    } else {
        to_cam_xz.normalize()
    };
    let right = Vec3::Y.cross(forward).normalize_or_zero();
    let right = if right.length_squared() < 1e-6 {
        Vec3::X
    } else {
        right
    };
    let half_w = radius.max(1.0);
    let half_h = radius.max(1.0);
    let center = position + Vec3::Y * half_h;
    let corners = [
        center - right * half_w - Vec3::Y * half_h,
        center + right * half_w - Vec3::Y * half_h,
        center + right * half_w + Vec3::Y * half_h,
        center - right * half_w + Vec3::Y * half_h,
    ];
    let indices = [0usize, 1, 2, 0, 2, 3];
    for index in indices {
        let p = corners[index];
        vertices.extend_from_slice(&[p.x, p.y, p.z, color[0], color[1], color[2], color[3]]);
    }
}

/// Queue the overlay after the 3D scene so the depth buffer already holds
/// buildings and units. C++ `renderStenciledPlayerColor` is a post-flush
/// fullscreen stencil fill; this is the live wgpu stand-in.
pub fn enqueue_occluded_player_color_pass(
    pipeline: &mut RenderPipeline,
    view_matrix: &Mat4,
    projection_matrix: &Mat4,
    camera_position: Vec3,
    presentation: Option<&PresentationFrame>,
) {
    let Some(frame) = presentation else {
        return;
    };
    let mut overlays = classify_from_presentation(camera_position, frame);
    if overlays.is_empty() {
        return;
    }
    let Some(renderer) = OcclusionOverlayRenderer::new() else {
        return;
    };
    let renderer = Arc::new(renderer);
    let view_proj = *projection_matrix * *view_matrix;
    pipeline.enqueue_post_frame_callback(move |gpu_frame| {
        let color_view = gpu_frame.color_view_arc();
        let depth_view = gpu_frame.depth_view_arc();
        let encoder = gpu_frame.encoder();
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
            label: Some("occluded_player_color_pass"),
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
            occlusion_query_set: None,
            timestamp_writes: None,
        });
        renderer.draw(&mut render_pass, &view_proj, camera_position, &overlays);
        drop(render_pass);
        Ok(())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(
        id: u32,
        position: Vec3,
        radius: f32,
        is_structure: bool,
        player_index: usize,
        color: [f32; 4],
    ) -> OcclusionCandidate {
        OcclusionCandidate {
            id: ObjectId(id),
            position,
            radius,
            is_structure,
            is_occludee: !is_structure,
            destroyed: false,
            player_index,
            team_color: color,
        }
    }

    #[test]
    fn player_color_bit_flip_matches_cpp_w3d_scene() {
        // C++ playerIndexToColorIndex (W3DScene.cpp:1181-1202).
        assert_eq!(player_index_to_color_index(0), 0);
        assert_eq!(player_index_to_color_index(1), 8);
        assert_eq!(player_index_to_color_index(2), 4);
        assert_eq!(player_index_to_color_index(3), 12);
        assert_eq!(player_index_to_color_index(7), 14);
        assert_eq!(player_index_to_color_index(8), 1);
    }

    #[test]
    fn flag_occluded_objects_marks_units_hidden_by_buildings() {
        // C++ RTS3DScene::flagOccludedObjects (W3DScene.cpp:226-286).
        let camera = Vec3::new(0.0, 0.0, 100.0);
        let building = candidate(
            1,
            Vec3::new(0.0, 0.0, 50.0),
            10.0,
            true,
            0,
            [0.2, 0.2, 0.2, 1.0],
        );
        let hidden = candidate(2, Vec3::ZERO, 2.0, false, 2, [0.25, 0.5, 0.25, 1.0]);
        let clear = candidate(
            3,
            Vec3::new(80.0, 0.0, 0.0),
            2.0,
            false,
            2,
            [0.25, 0.5, 0.25, 1.0],
        );
        let overlays = classify_occluded_player_overlays(camera, &[building, hidden, clear]);
        assert_eq!(overlays.len(), 1);
        assert_eq!(overlays[0].object_id, ObjectId(2));
        assert_eq!(overlays[0].player_index, 2);
        assert_eq!(overlays[0].color_index, 8);
        assert_eq!(overlays[0].stencil_ref, (8 << 3) | 0x80);
        assert!((overlays[0].color[0] - 0.125).abs() < 1e-5);
        assert!((overlays[0].color[1] - 0.25).abs() < 1e-5);
        assert!((overlays[0].color[3] - OCCLUDED_COLOR_ALPHA).abs() < 1e-5);
    }

    #[test]
    fn clear_line_of_sight_does_not_emit_player_color() {
        let camera = Vec3::new(0.0, 0.0, 100.0);
        let building = candidate(1, Vec3::new(50.0, 0.0, 0.0), 5.0, true, 0, [1.0; 4]);
        let unit = candidate(2, Vec3::new(-50.0, 0.0, 0.0), 2.0, false, 1, [1.0; 4]);
        assert!(classify_occluded_player_overlays(camera, &[building, unit]).is_empty());
    }

    #[test]
    fn per_player_bucket_caps_at_cpp_max() {
        // C++ MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS (W3DScene.cpp:1301).
        let camera = Vec3::new(0.0, 0.0, 100.0);
        let mut candidates = vec![candidate(
            1,
            Vec3::new(0.0, 0.0, 50.0),
            20.0,
            true,
            0,
            [1.0; 4],
        )];
        for i in 0..(MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS + 8) {
            candidates.push(candidate(
                10 + i as u32,
                Vec3::new(0.1, 0.0, 0.0),
                1.0,
                false,
                3,
                [1.0, 0.0, 0.0, 1.0],
            ));
        }
        let overlays = classify_occluded_player_overlays(camera, &candidates);
        assert_eq!(overlays.len(), MAX_VISIBLE_OCCLUDED_PLAYER_OBJECTS);
    }

    #[test]
    fn safe_occlusion_frame_skips_walk_out_unit() {
        let mut logic = crate::game_logic::GameLogic::new();
        let mut bunker_t = crate::game_logic::ThingTemplate::new("OccBunker");
        bunker_t
            .add_kind_of(crate::game_logic::KindOf::Structure)
            .set_health(1000.0);
        logic.templates.insert("OccBunker".into(), bunker_t);
        let mut pax_t = crate::game_logic::ThingTemplate::new("OccRanger");
        pax_t
            .add_kind_of(crate::game_logic::KindOf::Infantry)
            .add_kind_of(crate::game_logic::KindOf::Selectable)
            .set_health(100.0);
        logic.templates.insert("OccRanger".into(), pax_t);
        let _bunker = logic
            .create_object(
                "OccBunker",
                crate::game_logic::Team::USA,
                Vec3::new(0.0, 0.0, 50.0),
            )
            .unwrap();
        let pax = logic
            .create_object("OccRanger", crate::game_logic::Team::USA, Vec3::ZERO)
            .unwrap();
        let safe_frame = logic.frame;
        if let Some(p) = logic.host_object_mut(pax) {
            p.stamp_safe_occlusion_frame(safe_frame);
        }
        let frame = crate::presentation_frame::PresentationFrame::build_from_logic(&logic, 0);
        let camera = Vec3::new(0.0, 0.0, 100.0);
        let during = classify_from_presentation(camera, &frame);
        assert!(
            during.iter().all(|o| o.object_id != pax),
            "walk-out unit must not silhouette until OcclusionDelay"
        );
        let mut later = frame.clone();
        later.frame = crate::presentation_frame::LogicFrame(logic.frame + 90);
        let after = classify_from_presentation(camera, &later);
        assert!(
            after.iter().any(|o| o.object_id == pax),
            "after OcclusionDelay the unit is a potential occludee"
        );
    }
}
