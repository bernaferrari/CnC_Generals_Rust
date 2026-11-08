//! W3DDisplay occlusion overlay pass and visible-box helpers.
//!
//! Stencil shadow volumes (C++ `W3DVolumetricShadow`, `DoShadows(true)`) are
//! drawn by the mesh renderer (`ww3d_renderer_3d` shadow volumes) and occluded
//! player-colour silhouettes (`flushOccludedObjectsIntoStencil`) by Main's
//! occlusion bridge. This module keeps the per-Display overlay pass for
//! detected-stealth markers and the particle-cull visible box. It reads no
//! process or live-client state: callers pass their drawables and own the
//! GPU state.

mod frustum;
pub use frustum::clipped_visible_box_z_up;

use crate::drawable::StealthLook;
use crate::terrain::TerrainVisual;
use crate::terrain::terrain_visual::get_terrain_visual;


/// Detected-stealth marker colour.
const HEAT_VISION_COLOR: [f32; 4] = [1.0, 0.35, 0.05, 0.85];
/// Floats per overlay vertex: position xyz + colour rgba.
const OVERLAY_VERTEX_FLOATS: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayKind {
    PlayerColor,
    HeatVision,
}

#[derive(Debug, Clone)]
pub struct OcclusionOverlay {
    pub position: [f32; 3],
    pub color: [f32; 4],
    pub kind: OverlayKind,
}

/// Heat-vision markers for the visible detected-stealth drawables of `manager`.
pub fn heat_vision_overlays(manager: &crate::drawable::DrawableManager) -> Vec<OcclusionOverlay> {
    let mut out = Vec::new();
    for id in manager.get_all_drawable_ids() {
        let Some(drawable) = manager.get_drawable(id) else {
            continue;
        };
        if !drawable.is_visible() {
            continue;
        }
        if !matches!(
            drawable.get_stealth_look(),
            StealthLook::VisibleDetected | StealthLook::VisibleFriendlyDetected
        ) {
            continue;
        }
        let pos = drawable.get_position();
        out.push(OcclusionOverlay {
            position: [pos.x, pos.y, pos.z],
            color: HEAT_VISION_COLOR,
            kind: OverlayKind::HeatVision,
        });
    }
    out
}

const OVERLAY_SHADER: &str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0) var<uniform> uniforms: Uniforms;

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_overlay(@location(0) position: vec3<f32>, @location(1) color: vec4<f32>) -> VsOut {
    var out: VsOut;
    out.position = uniforms.view_proj * vec4<f32>(position, 1.0);
    out.color = color;
    return out;
}

@fragment
fn fs_overlay(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
"#;

const OVERLAY_VERTEX_ATTRS: [wgpu::VertexAttribute; 2] =
    wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x4];

struct OverlayGpu {
    color_format: wgpu::TextureFormat,
    depth_format: wgpu::TextureFormat,
    pipeline: wgpu::RenderPipeline,
    uniforms: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: Option<wgpu::Buffer>,
}

impl OverlayGpu {
    fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("occlusion_overlay"),
            source: wgpu::ShaderSource::Wgsl(OVERLAY_SHADER.into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("occlusion_overlay_uniforms"),
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
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("occlusion_overlay_layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("occlusion_overlay"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_overlay"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: (OVERLAY_VERTEX_FLOATS * std::mem::size_of::<f32>())
                        as wgpu::BufferAddress,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &OVERLAY_VERTEX_ATTRS,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_overlay"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Greater),
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("occlusion_overlay_uniforms"),
            size: std::mem::size_of::<[[f32; 4]; 4]>() as wgpu::BufferAddress,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("occlusion_overlay_bind_group"),
            layout: &bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniforms.as_entire_binding(),
            }],
        });
        Self {
            color_format,
            depth_format,
            pipeline,
            uniforms,
            bind_group,
            vertices: None,
        }
    }

    /// Grow-only vertex buffer, rewritten once per recorded frame.
    fn upload_vertices(&mut self, device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) {
        let len = bytes.len() as wgpu::BufferAddress;
        let fits = self.vertices.as_ref().is_some_and(|buffer| buffer.size() >= len);
        if !fits {
            self.vertices = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("occlusion_overlay_vertices"),
                size: len.next_power_of_two().max(4096),
                usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        let buffer = self.vertices.as_ref().expect("vertex buffer allocated above");
        queue.write_buffer(buffer, 0, bytes);
    }
}

/// One Display's overlay pass: pipeline, uniforms and vertices persist across
/// frames and are rebuilt only when the target formats change.
#[derive(Default)]
pub struct OcclusionOverlayPass {
    gpu: Option<OverlayGpu>,
}

/// Targets and camera for one [`OcclusionOverlayPass::record`].
pub struct OverlayTargets<'a> {
    pub color_view: &'a wgpu::TextureView,
    pub depth_view: &'a wgpu::TextureView,
    pub color_format: wgpu::TextureFormat,
    pub depth_format: wgpu::TextureFormat,
    pub view_proj: glam::Mat4,
    pub camera: [f32; 3],
    /// Tactical viewport (w, h) in framebuffer pixels; (0, 0) keeps the
    /// full-attachment default.
    pub viewport_px: (f32, f32),
}

impl OcclusionOverlayPass {
    /// Draw `overlays` as camera-facing billboards. Records nothing (and
    /// creates nothing) when the list is empty.
    pub fn record(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        targets: &OverlayTargets<'_>,
        overlays: &[OcclusionOverlay],
    ) {
        if overlays.is_empty() {
            return;
        }
        let mut vertices = Vec::with_capacity(overlays.len() * 6 * OVERLAY_VERTEX_FLOATS);
        for overlay in overlays {
            push_overlay_billboard(&mut vertices, overlay.position, targets.camera, overlay.color);
        }
        let rebuild = self.gpu.as_ref().is_none_or(|gpu| {
            gpu.color_format != targets.color_format || gpu.depth_format != targets.depth_format
        });
        if rebuild {
            self.gpu = Some(OverlayGpu::new(device, targets.color_format, targets.depth_format));
        }
        let gpu = self.gpu.as_mut().expect("overlay gpu built above");
        queue.write_buffer(
            &gpu.uniforms,
            0,
            bytemuck::bytes_of(&targets.view_proj.to_cols_array_2d()),
        );
        gpu.upload_vertices(device, queue, bytemuck::cast_slice(&vertices));
        let vertex_buffer = gpu.vertices.as_ref().expect("vertices uploaded above");
        let vertex_bytes = (vertices.len() * std::mem::size_of::<f32>()) as wgpu::BufferAddress;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Display Occlusion Overlay Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: targets.color_view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: targets.depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            occlusion_query_set: None,
            timestamp_writes: None,
            multiview_mask: None,
        });
        if targets.viewport_px.0 > 0.0 && targets.viewport_px.1 > 0.0 {
            let (vw, vh) = targets.viewport_px;
            pass.set_viewport(0.0, 0.0, vw, vh, 0.0, 1.0);
            pass.set_scissor_rect(0, 0, vw as u32, vh as u32);
        }
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, &gpu.bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..vertex_bytes));
        pass.draw(0..(vertices.len() / OVERLAY_VERTEX_FLOATS) as u32, 0..1);
    }
}

fn push_overlay_billboard(
    vertices: &mut Vec<f32>,
    position: [f32; 3],
    camera: [f32; 3],
    color: [f32; 4],
) {
    let pos = glam::Vec3::from_array(position);
    let camera = glam::Vec3::from_array(camera);
    let to_cam = camera - pos;
    let to_cam_xy = glam::Vec3::new(to_cam.x, to_cam.y, 0.0);
    let forward = if to_cam_xy.length_squared() < 1e-6 {
        glam::Vec3::Y
    } else {
        to_cam_xy.normalize()
    };
    let mut right = glam::Vec3::Z.cross(forward);
    if right.length_squared() < 1e-6 {
        right = glam::Vec3::X;
    } else {
        right = right.normalize();
    }
    let half = 6.0f32;
    let center = pos + glam::Vec3::Z * half;
    let corners = [
        center - right * half - glam::Vec3::Z * half,
        center + right * half - glam::Vec3::Z * half,
        center + right * half + glam::Vec3::Z * half,
        center - right * half + glam::Vec3::Z * half,
    ];
    for index in [0usize, 1, 2, 0, 2, 3] {
        let p = corners[index];
        vertices.extend_from_slice(&[p.x, p.y, p.z, color[0], color[1], color[2], color[3]]);
    }
}

/// Terrain-and-sky AABB used by particle cull (C++ getMaximumVisibleBox(..., TRUE)).
#[derive(Debug, Clone, Copy)]
pub struct VisibleBox {
    pub center: [f32; 3],
    pub extent: [f32; 3],
}

impl VisibleBox {
    pub fn contains_expanded(&self, pos: [f32; 3], size: f32) -> bool {
        (pos[0] - self.center[0]).abs() <= self.extent[0] + size
            && (pos[1] - self.center[1]).abs() <= self.extent[1] + size
            && (pos[2] - self.center[2]).abs() <= self.extent[2] + size
    }
}

/// Clip frustum corners to the terrain min-height plane.
pub fn maximum_visible_box(
    camera: [f32; 3],
    target: [f32; 3],
    near_z: f32,
    far_z: f32,
    fov: f32,
    aspect: f32,
    min_height: f32,
) -> VisibleBox {
    let mut forward = [
        target[0] - camera[0],
        target[1] - camera[1],
        target[2] - camera[2],
    ];
    let fl = (forward[0] * forward[0] + forward[1] * forward[1] + forward[2] * forward[2]).sqrt();
    if fl > 1e-5 {
        forward[0] /= fl;
        forward[1] /= fl;
        forward[2] /= fl;
    } else {
        forward = [0.0, 1.0, 0.0];
    }
    let mut up = [0.0, 0.0, 1.0];
    if forward[2].abs() > 0.99 {
        up = [0.0, 1.0, 0.0];
    }
    let mut right = [
        forward[1] * up[2] - forward[2] * up[1],
        forward[2] * up[0] - forward[0] * up[2],
        forward[0] * up[1] - forward[1] * up[0],
    ];
    let rl = (right[0] * right[0] + right[1] * right[1] + right[2] * right[2])
        .sqrt()
        .max(1e-5);
    right[0] /= rl;
    right[1] /= rl;
    right[2] /= rl;
    up = [
        right[1] * forward[2] - right[2] * forward[1],
        right[2] * forward[0] - right[0] * forward[2],
        right[0] * forward[1] - right[1] * forward[0],
    ];

    let tan_half = (fov * 0.5).tan();
    let mut corners = [[0.0f32; 3]; 8];
    for (i, &dist) in [near_z, far_z].iter().enumerate() {
        let hh = dist * tan_half;
        let hw = hh * aspect;
        let c = [
            camera[0] + forward[0] * dist,
            camera[1] + forward[1] * dist,
            camera[2] + forward[2] * dist,
        ];
        let signs = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];
        for (j, (sx, sy)) in signs.iter().enumerate() {
            corners[i * 4 + j] = [
                c[0] + right[0] * hw * sx + up[0] * hh * sy,
                c[1] + right[1] * hw * sx + up[1] * hh * sy,
                c[2] + right[2] * hw * sx + up[2] * hh * sy,
            ];
        }
    }
    // Clip far corners down to the ground plane (C++ ignoreMaxHeight = TRUE).
    for i in 0..4 {
        let a = corners[i];
        let b = corners[i + 4];
        let dz = b[2] - a[2];
        if dz.abs() > 1e-5 {
            let t = (min_height - a[2]) / dz;
            if (0.0..=1.0).contains(&t) {
                corners[i + 4] = [
                    a[0] + (b[0] - a[0]) * t,
                    a[1] + (b[1] - a[1]) * t,
                    min_height,
                ];
            }
        }
    }
    let mut min = corners[0];
    let mut max = corners[0];
    for c in &corners {
        for k in 0..3 {
            min[k] = min[k].min(c[k]);
            max[k] = max[k].max(c[k]);
        }
    }
    VisibleBox {
        center: [
            (min[0] + max[0]) * 0.5,
            (min[1] + max[1]) * 0.5,
            (min[2] + max[2]) * 0.5,
        ],
        extent: [
            (max[0] - min[0]) * 0.5,
            (max[1] - min[1]) * 0.5,
            (max[2] - min[2]) * 0.5,
        ],
    }
}

pub fn terrain_min_height() -> f32 {
    if let Ok(guard) = get_terrain_visual() {
        if let Some(terrain) = guard.as_ref() {
            if let Ok(h) = terrain.get_height_at(0.0, 0.0) {
                return h.min(0.0);
            }
        }
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_box_contains_camera_target() {
        crate::test_support::ensure_ambient_host();
        let box_ = maximum_visible_box(
            [0.0, 0.0, 100.0],
            [0.0, 100.0, 0.0],
            1.0,
            500.0,
            1.0,
            1.0,
            0.0,
        );
        assert!(box_.contains_expanded([0.0, 50.0, 20.0], 5.0));
    }

    #[test]
    fn heat_vision_overlays_without_detected_drawables_are_empty() {
        crate::test_support::ensure_ambient_host();
        let manager = crate::drawable::DrawableManager::new();
        assert!(heat_vision_overlays(&manager).is_empty());
    }

    #[test]
    fn overlay_billboard_is_two_camera_facing_triangles() {
        let mut vertices = Vec::new();
        push_overlay_billboard(&mut vertices, [0.0, 0.0, 0.0], [0.0, -100.0, 50.0], HEAT_VISION_COLOR);
        assert_eq!(vertices.len(), 6 * OVERLAY_VERTEX_FLOATS);
        // Every corner lies in the plane facing the camera (y == 0).
        for vertex in vertices.chunks_exact(OVERLAY_VERTEX_FLOATS) {
            assert!(vertex[1].abs() < 1e-5);
            assert_eq!(&vertex[3..], &HEAT_VISION_COLOR);
        }
    }
}
