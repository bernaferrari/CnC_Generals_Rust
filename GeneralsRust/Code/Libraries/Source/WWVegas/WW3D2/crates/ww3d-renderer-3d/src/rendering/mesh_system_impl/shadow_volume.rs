//! Stencil shadow volumes (C++ `W3DVolumetricShadow` + `W3DShadowManager`).
//!
//! For every `Shadow = SHADOW_VOLUME` caster the silhouette of its rigid
//! meshes, as seen from the sun, is extruded away from the light. The side
//! quads are drawn into the stencil only (z-pass: front faces increment,
//! back faces decrement, C++ `D3DSTENCILOP_INCR` / `DECRSAT` passes), then a
//! screen quad multiplies the colour by the shadow colour wherever the count
//! is non-zero (C++ `SRCBLEND = DESTCOLOR`, `DESTBLEND = ZERO`,
//! `m_shadowColor = 0x7fa0a0a0`). This runs after the opaque meshes and
//! before translucent ones, the `DoShadows(stencil)` slot of
//! `RTS3DScene::Flush`.
//!
//! Silhouettes are recomputed on the CPU each frame from a per-model edge
//! table built once (welded positions, so UV seams do not count as edges);
//! skinned meshes are skipped (their pose lives on the GPU).

use std::collections::HashMap;
use std::sync::Arc;

use glam::{Mat4, Vec3};

use super::{MeshClass, MeshModelClass};

/// C++ `W3DShadowManager::m_shadowColor` default (0x7fa0a0a0): rgb 160/255.
pub const SHADOW_COLOR: [f32; 4] = [160.0 / 255.0, 160.0 / 255.0, 160.0 / 255.0, 1.0];

/// Extra extrusion below the caster's lowest point so the volume always
/// pierces the ground it stands on.
const GROUND_MARGIN: f32 = 20.0;
/// Shallowest light slope used for the extrusion length (keeps grazing sun
/// angles from producing near-infinite volumes).
const MIN_LIGHT_DROP: f32 = 0.2;

/// Welded triangle soup plus edge adjacency of one mesh model.
struct EdgeMesh {
    positions: Vec<Vec3>,
    triangles: Vec<[u32; 3]>,
    /// (a, b, face using a->b, face using b->a if any)
    edges: Vec<(u32, u32, u32, Option<u32>)>,
}

impl EdgeMesh {
    fn build(model: &MeshModelClass) -> Self {
        let mut weld: HashMap<[i32; 3], u32> = HashMap::new();
        let mut positions = Vec::new();
        let remap: Vec<u32> = model
            .vertices
            .iter()
            .map(|v| {
                let key = [
                    (v.x * 1000.0).round() as i32,
                    (v.y * 1000.0).round() as i32,
                    (v.z * 1000.0).round() as i32,
                ];
                *weld.entry(key).or_insert_with(|| {
                    positions.push(Vec3::new(v.x, v.y, v.z));
                    (positions.len() - 1) as u32
                })
            })
            .collect();
        let triangles: Vec<[u32; 3]> = model
            .triangles
            .iter()
            .filter_map(|t| {
                let i = t.vindex.map(|v| remap.get(v as usize).copied());
                let [Some(a), Some(b), Some(c)] = i else {
                    return None;
                };
                (a != b && b != c && a != c).then_some([a, b, c])
            })
            .collect();
        let mut open: HashMap<(u32, u32), usize> = HashMap::new();
        let mut edges: Vec<(u32, u32, u32, Option<u32>)> = Vec::new();
        for (face, tri) in triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                if let Some(index) = open.remove(&(b, a)) {
                    if edges[index].3.is_none() {
                        edges[index].3 = Some(face as u32);
                        continue;
                    }
                }
                open.insert((a, b), edges.len());
                edges.push((a, b, face as u32, None));
            }
        }
        Self {
            positions,
            triangles,
            edges,
        }
    }

    /// Append the extruded silhouette side quads (world space) for `transform`.
    fn extrude(&self, transform: &Mat4, light_dir: Vec3, out: &mut Vec<[f32; 4]>) {
        let world: Vec<Vec3> = self
            .positions
            .iter()
            .map(|p| transform.transform_point3(*p))
            .collect();
        if world.is_empty() {
            return;
        }
        let (min_y, max_y) = world
            .iter()
            .fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p.y), hi.max(p.y)));
        let drop = (-light_dir.y).max(MIN_LIGHT_DROP);
        let extrude = light_dir * ((max_y - min_y + GROUND_MARGIN) / drop);
        let lit: Vec<bool> = self
            .triangles
            .iter()
            .map(|[a, b, c]| {
                let (a, b, c) = (world[*a as usize], world[*b as usize], world[*c as usize]);
                (b - a).cross(c - a).dot(-light_dir) > 0.0
            })
            .collect();
        let mut quad = |a: Vec3, b: Vec3| {
            let (a2, b2) = (a + extrude, b + extrude);
            for p in [b, a, a2, b, a2, b2] {
                out.push([p.x, p.y, p.z, 1.0]);
            }
        };
        for &(a, b, face, other) in &self.edges {
            let face_lit = lit[face as usize];
            let (pa, pb) = (world[a as usize], world[b as usize]);
            match other.map(|f| lit[f as usize]) {
                // Silhouette: oriented by the lit face's winding.
                Some(other_lit) if other_lit != face_lit => {
                    if face_lit {
                        quad(pa, pb)
                    } else {
                        quad(pb, pa)
                    }
                }
                // Open boundary of a lit face also bounds the volume.
                None if face_lit => quad(pa, pb),
                _ => {}
            }
        }
    }
}

/// GPU state and per-model edge tables for the stencil shadow pass.
pub(super) struct ShadowVolumeRenderer {
    volume_pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: Option<(wgpu::Buffer, u64)>,
    /// Keyed by model address; the Arc keeps the address from being reused.
    edge_meshes: HashMap<usize, (Arc<MeshModelClass>, Arc<EdgeMesh>)>,
}

const SHADER: &str = r#"
struct Uniforms { view_proj: mat4x4<f32>, color: vec4<f32> };
@group(0) @binding(0) var<uniform> u: Uniforms;

@vertex
fn vs_volume(@location(0) position: vec4<f32>) -> @builtin(position) vec4<f32> {
    return u.view_proj * vec4<f32>(position.xyz, 1.0);
}
@fragment
fn fs_volume() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

@vertex
fn vs_fill(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let uv = vec2<f32>(f32((index << 1u) & 2u), f32(index & 2u));
    return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}
@fragment
fn fs_fill() -> @location(0) vec4<f32> {
    return u.color;
}
"#;

impl ShadowVolumeRenderer {
    pub(super) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("shadow_volume"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("shadow_volume_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("shadow_volume_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let face = |pass_op| wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::Always,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op,
        };
        let volume_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_volume_stencil"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_volume"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: 16,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x4],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_volume"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::empty(),
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Less),
                stencil: wgpu::StencilState {
                    front: face(wgpu::StencilOperation::IncrementWrap),
                    back: face(wgpu::StencilOperation::DecrementWrap),
                    read_mask: 0xff,
                    write_mask: 0xff,
                },
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let not_zero = wgpu::StencilFaceState {
            compare: wgpu::CompareFunction::NotEqual,
            fail_op: wgpu::StencilOperation::Keep,
            depth_fail_op: wgpu::StencilOperation::Keep,
            pass_op: wgpu::StencilOperation::Keep,
        };
        let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("shadow_volume_fill"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &module,
                entry_point: Some("vs_fill"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &module,
                entry_point: Some("fs_fill"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Dst,
                            dst_factor: wgpu::BlendFactor::Zero,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_format,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::Always),
                stencil: wgpu::StencilState {
                    front: not_zero,
                    back: not_zero,
                    read_mask: 0xff,
                    write_mask: 0,
                },
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow_volume_uniform"),
            size: 80,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow_volume_bg"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        Self {
            volume_pipeline,
            fill_pipeline,
            uniform,
            bind_group,
            vertices: None,
            edge_meshes: HashMap::new(),
        }
    }

    /// Build this frame's volumes for `casters` and record the stencil and
    /// fill draws. The pass must carry a stencil aspect cleared to zero.
    pub(super) fn record(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pass: &mut wgpu::RenderPass<'_>,
        casters: &[Arc<MeshClass>],
        view_proj: Mat4,
        light_dir: Vec3,
    ) -> u32 {
        let light_dir = light_dir.normalize_or_zero();
        if casters.is_empty() || light_dir == Vec3::ZERO {
            return 0;
        }
        let mut vertices: Vec<[f32; 4]> = Vec::new();
        for mesh in casters {
            if mesh.is_hidden || mesh.is_animation_hidden {
                continue;
            }
            let Some(model) = mesh.model.as_ref() else {
                continue;
            };
            if !model.vertex_influences.is_empty() {
                continue;
            }
            let key = Arc::as_ptr(model) as usize;
            let edges = Arc::clone(
                &self
                    .edge_meshes
                    .entry(key)
                    .or_insert_with(|| (Arc::clone(model), Arc::new(EdgeMesh::build(model))))
                    .1,
            );
            edges.extrude(&mesh.transform, light_dir, &mut vertices);
        }
        if vertices.is_empty() {
            return 0;
        }
        let bytes: &[u8] = bytemuck::cast_slice(&vertices);
        let needed = bytes.len() as u64;
        if self.vertices.as_ref().is_none_or(|(_, cap)| *cap < needed) {
            let capacity = needed.next_power_of_two();
            self.vertices = Some((
                device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("shadow_volume_vertices"),
                    size: capacity,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                }),
                capacity,
            ));
        }
        let (buffer, _) = self.vertices.as_ref().expect("volume buffer");
        queue.write_buffer(buffer, 0, bytes);
        let mut uniform = [0.0f32; 20];
        uniform[..16].copy_from_slice(&view_proj.to_cols_array());
        uniform[16..].copy_from_slice(&SHADOW_COLOR);
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));

        pass.set_stencil_reference(0);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.volume_pipeline);
        pass.set_vertex_buffer(0, buffer.slice(..needed));
        pass.draw(0..vertices.len() as u32, 0..1);
        pass.set_pipeline(&self.fill_pipeline);
        pass.draw(0..3, 0..1);
        (vertices.len() / 6) as u32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> EdgeMesh {
        let p = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
            [0.0, 1.0, 1.0],
        ];
        // Outward CCW faces of a unit cube.
        let quads = [
            [0, 3, 2, 1],
            [4, 5, 6, 7],
            [0, 1, 5, 4],
            [2, 3, 7, 6],
            [1, 2, 6, 5],
            [0, 4, 7, 3],
        ];
        let mut triangles = Vec::new();
        for q in quads {
            triangles.push([q[0], q[1], q[2]]);
            triangles.push([q[0], q[2], q[3]]);
        }
        let mut open: HashMap<(u32, u32), usize> = HashMap::new();
        let mut edges: Vec<(u32, u32, u32, Option<u32>)> = Vec::new();
        for (face, tri) in triangles.iter().enumerate() {
            for k in 0..3 {
                let (a, b) = (tri[k], tri[(k + 1) % 3]);
                if let Some(i) = open.remove(&(b, a)) {
                    edges[i].3 = Some(face as u32);
                    continue;
                }
                open.insert((a, b), edges.len());
                edges.push((a, b, face as u32, None));
            }
        }
        EdgeMesh {
            positions: p.iter().map(|v| Vec3::from_array(*v)).collect(),
            triangles,
            edges,
        }
    }

    #[test]
    fn closed_cube_under_vertical_sun_extrudes_its_four_top_edges() {
        let mesh = cube();
        assert!(mesh.edges.iter().all(|e| e.3.is_some()), "cube is closed");
        let mut out = Vec::new();
        mesh.extrude(&Mat4::IDENTITY, Vec3::new(0.0, -1.0, 0.0), &mut out);
        // Silhouette = the 4 edges around the lit top (y = 1) face.
        assert_eq!(out.len(), 4 * 6);
        let lowest = out.iter().map(|v| v[1]).fold(f32::MAX, f32::min);
        assert!(lowest < -GROUND_MARGIN + 1.5, "volume reaches below the caster");
    }
}
