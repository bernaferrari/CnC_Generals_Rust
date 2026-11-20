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
//! Silhouettes come from welded per-model edge tables. The uploaded world-space
//! volume is reused for the same ordered caster transforms and sun direction.
//! Skinned meshes are skipped (their pose lives on the GPU).

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
    uploaded_casters: Vec<(usize, [u32; 16])>,
    uploaded_light: Option<[u32; 3]>,
    uploaded_vertex_count: usize,
    scratch_vertices: Vec<[f32; 4]>,
    /// Keyed by model address; the Arc keeps the address from being reused.
    edge_meshes: HashMap<usize, (Arc<MeshModelClass>, EdgeMesh)>,
}

fn shadow_volume_model(mesh: &MeshClass) -> Option<&Arc<MeshModelClass>> {
    if mesh.is_hidden || mesh.is_animation_hidden {
        return None;
    }
    mesh.model.as_ref().filter(|model| model.vertex_influences.is_empty())
}

fn caster_geometry_key(mesh: &MeshClass, model: &Arc<MeshModelClass>) -> (usize, [u32; 16]) {
    (
        Arc::as_ptr(model) as usize,
        mesh.transform.to_cols_array().map(f32::to_bits),
    )
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
                // W3DVolumetricShadowManager::renderShadows uses D3DCMP_LESSEQUAL.
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
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
            uploaded_casters: Vec::new(),
            uploaded_light: None,
            uploaded_vertex_count: 0,
            scratch_vertices: Vec::new(),
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
        let light = light_dir.to_array().map(f32::to_bits);
        let eligible = || {
            casters.iter().filter_map(|mesh| shadow_volume_model(mesh).map(|model| (mesh, model)))
        };
        if self.uploaded_light != Some(light)
            || !eligible()
                .map(|(mesh, model)| caster_geometry_key(mesh, model))
                .eq(self.uploaded_casters.iter().copied())
        {
            self.uploaded_casters.clear();
            self.scratch_vertices.clear();
            for (mesh, model) in eligible() {
                let key = caster_geometry_key(mesh, model);
                let (_, edges) = self.edge_meshes.entry(key.0)
                    .or_insert_with(|| (Arc::clone(model), EdgeMesh::build(model)));
                edges.extrude(&mesh.transform, light_dir, &mut self.scratch_vertices);
                self.uploaded_casters.push(key);
            }
            self.uploaded_light = Some(light);
            self.uploaded_vertex_count = self.scratch_vertices.len();
            let bytes: &[u8] = bytemuck::cast_slice(&self.scratch_vertices);
            let needed = bytes.len() as u64;
            if needed != 0 {
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
                queue.write_buffer(&self.vertices.as_ref().expect("volume buffer").0, 0, bytes);
            }
        }
        let vertex_count = self.uploaded_vertex_count;
        if vertex_count == 0 {
            return 0;
        }
        let needed = (vertex_count * std::mem::size_of::<[f32; 4]>()) as u64;
        let (buffer, _) = self.vertices.as_ref().expect("volume buffer");
        let mut uniform = [0.0f32; 20];
        uniform[..16].copy_from_slice(&view_proj.to_cols_array());
        uniform[16..].copy_from_slice(&SHADOW_COLOR);
        queue.write_buffer(&self.uniform, 0, bytemuck::cast_slice(&uniform));

        pass.set_stencil_reference(0);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_pipeline(&self.volume_pipeline);
        pass.set_vertex_buffer(0, buffer.slice(..needed));
        pass.draw(0..vertex_count as u32, 0..1);
        pass.set_pipeline(&self.fill_pipeline);
        pass.draw(0..3, 0..1);
        (vertex_count / 6) as u32
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

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    #[ignore = "requires a native GPU adapter"]
    fn stencil_shadow_accepts_an_equal_depth_receiver() {
        use super::super::{W3dTriangleStruct, W3dVectorStruct};

        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
                .expect("shadow volume test requires a GPU adapter");
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
                .expect("shadow volume test requires a GPU device");
        let color_format = wgpu::TextureFormat::Rgba8Unorm;
        let depth_format = wgpu::TextureFormat::Depth24PlusStencil8;
        let extent = wgpu::Extent3d {
            width: 16,
            height: 8,
            depth_or_array_layers: 1,
        };
        let texture = |label, format, usage| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: Some(label),
                size: extent,
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage,
                view_formats: &[],
            })
        };
        let color = texture(
            "shadow volume receiver color",
            color_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        );
        let depth = texture(
            "shadow volume receiver depth and stencil",
            depth_format,
            wgpu::TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shadow receiver pixels"),
            size: 256 * u64::from(extent.height),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut renderer = ShadowVolumeRenderer::new(&device, color_format, depth_format);
        let mut model = MeshModelClass::new("stencil-receiver-caster");
        let geometry = cube();
        model.vertices = geometry
            .positions
            .into_iter()
            .map(|p| W3dVectorStruct {
                x: p.x,
                y: p.y,
                z: p.z,
            })
            .collect();
        model.triangles = geometry
            .triangles
            .into_iter()
            .map(|vindex| W3dTriangleStruct {
                vindex,
                attributes: 0,
                normal: W3dVectorStruct {
                    x: 0.0,
                    y: 0.0,
                    z: 0.0,
                },
                distance: 0.0,
            })
            .collect();
        let mut mesh = MeshClass::new();
        mesh.model = Some(Arc::new(model));
        mesh.transform = Mat4::from_scale_rotation_translation(
            Vec3::new(1.0, 1.0, 0.25),
            glam::Quat::IDENTITY,
            Vec3::new(-0.5, 0.25, -0.125),
        );
        // Identity projects the near volume wall outside clip space and the
        // other to depth 1/8. The center pixel intersects only that wall.
        let base_transform = mesh.transform;
        let mut casters = [Arc::new(mesh)];
        let cases = [
            (0.25, 160, 0.0, false),
            (0.0625, 255, 0.0, true),
            (0.125, 160, 0.0, true),
            (0.25, 255, 2.0, false),
            (0.25, 160, 0.0, false),
        ];
        for (receiver, shade, offset, reuse) in cases {
            Arc::get_mut(&mut casters[0]).unwrap().transform =
                Mat4::from_translation(Vec3::X * offset) * base_transform;
            renderer.scratch_vertices.clear();
            let mut encoder = device.create_command_encoder(&Default::default());
            {
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("stencil shadow and receiver"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &color_view,
                        resolve_target: None,
                        depth_slice: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(receiver),
                            store: wgpu::StoreOp::Discard,
                        }),
                        stencil_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0),
                            store: wgpu::StoreOp::Discard,
                        }),
                    }),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                assert_eq!(
                    renderer.record(
                        &device,
                        &queue,
                        &mut pass,
                        &casters,
                        Mat4::IDENTITY,
                        -Vec3::Y
                    ),
                    4
                );
                assert_eq!(renderer.scratch_vertices.is_empty(), reuse);
            }
            encoder.copy_texture_to_buffer(
                color.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(extent.height),
                    },
                },
                extent,
            );
            queue.submit([encoder.finish()]);
            let (complete, received) = std::sync::mpsc::channel();
            let slice = readback.slice(..);
            slice.map_async(wgpu::MapMode::Read, move |result| {
                complete.send(result).unwrap()
            });
            device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            received.recv().unwrap().unwrap();
            {
                let pixels = slice.get_mapped_range().unwrap();
                let row = 4 * 256;
                let inside = row + 8 * 4;
                assert_eq!(
                    &pixels[row..row + 4],
                    &[255, 255, 255, 255],
                    "outside the volume"
                );
                assert_eq!(
                    &pixels[inside..inside + 4],
                    &[shade, shade, shade, 255],
                    "receiver depth {receiver}"
                );
            }
            readback.unmap();
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
