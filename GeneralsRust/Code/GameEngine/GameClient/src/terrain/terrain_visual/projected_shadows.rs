use crate::terrain::scorch_mesh::ScorchHeightSource;

/// One visible W3DModelDraw shadow using an authored SHADOW_DECAL texture.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectedUnitShadow {
    pub object_id: u32,
    pub transform: Mat4,
    pub size: Vec2,
    pub offset: Vec2,
    pub texture_name: String,
    /// Receiving bridge-layer height before the C++ BRIDGE_OFFSET_FACTOR.
    pub layer_height: Option<f32>,
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct ProjectedShadowVertex {
    position: [f32; 3],
    uv: [f32; 2],
}

struct ProjectedShadowGpu {
    pipeline: wgpu::RenderPipeline,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    textures: HashMap<String, Option<BindGroup>>,
    inputs: Vec<ProjectedUnitShadow>,
    geometry_draw_window: Option<([i32; 2], [i32; 2])>,
    vertices: Vec<ProjectedShadowVertex>,
    indices: Vec<u32>,
    batches: Vec<(String, std::ops::Range<u32>)>,
    vertex_buffer: Option<Buffer>,
    index_buffer: Option<Buffer>,
}

impl ProjectedShadowGpu {
    fn new(device: &wgpu::Device, camera_layout: &wgpu::BindGroupLayout) -> Self {
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Projected shadow texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Projected shadow clamp sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Projected shadow pipeline layout"),
            bind_group_layouts: &[Some(camera_layout), Some(&texture_layout)],
            immediate_size: 0,
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Projected shadow shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../shaders/projected_shadow.wgsl").into(),
            ),
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Projected terrain shadow"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<ProjectedShadowVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &wgpu::vertex_attr_array![0 => Float32x3, 1 => Float32x2],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Bgra8Unorm,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Zero,
                            dst_factor: wgpu::BlendFactor::Src,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent::REPLACE,
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
            }),
            primitive: Default::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: TERRAIN_PIPELINES_DEPTH_FORMAT,
                depth_write_enabled: Some(false),
                depth_compare: Some(wgpu::CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview_mask: None,
            cache: None,
        });
        Self {
            pipeline,
            texture_layout,
            sampler,
            textures: HashMap::new(),
            inputs: Vec::new(),
            geometry_draw_window: None,
            vertices: Vec::new(),
            indices: Vec::new(),
            batches: Vec::new(),
            vertex_buffer: None,
            index_buffer: None,
        }
    }

    fn upload_buffer(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        buffer: &mut Option<Buffer>,
        usage: wgpu::BufferUsages,
        bytes: &[u8],
        limit: u64,
    ) {
        let size = bytes.len() as u64;
        if buffer.as_ref().is_none_or(|buffer| buffer.size() < size) {
            *buffer = Some(device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Projected shadow geometry"),
                size: size.next_power_of_two().min(limit),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(buffer.as_ref().unwrap(), 0, bytes);
    }
}

impl TerrainVisualImpl {
    /// Admit this frame’s source descriptors with a borrowed mounted-content
    /// reader. Return the casters that have a resident authored texture.
    pub fn admit_projected_unit_shadows(
        &mut self,
        mut inputs: Vec<ProjectedUnitShadow>,
        mut read: impl FnMut(&str) -> Option<Vec<u8>>,
    ) -> HashSet<u32> {
        let (Some(device), Some(queue), Some(camera_layout)) = (
            self.device.as_ref(),
            self.queue.as_ref(),
            self.terrain_camera_bind_group_layout.as_ref(),
        ) else {
            return HashSet::new();
        };
        if inputs.is_empty() {
            if let Some(gpu) = self.projected_shadow_gpu.as_mut() {
                gpu.inputs.clear();
                gpu.geometry_draw_window = None;
                gpu.batches.clear();
            }
            return HashSet::new();
        }
        let gpu = self
            .projected_shadow_gpu
            .get_or_insert_with(|| ProjectedShadowGpu::new(device, camera_layout));
        let texture_layout = &gpu.texture_layout;
        let sampler = &gpu.sampler;
        inputs.retain_mut(|shadow| {
            shadow.texture_name.make_ascii_lowercase();
            let bound = gpu
                .textures
                .entry(shadow.texture_name.clone())
                .or_insert_with(|| {
                    // WW3D Get_Texture asks for a TGA name and admits the DDS
                    // counterpart. Both cases use the original base level only.
                    let name = shadow
                        .texture_name
                        .trim_end_matches(".tga")
                        .trim_end_matches(".dds");
                    let texture = ["dds", "tga"].into_iter().find_map(|extension| {
                        let path = format!("Art/Textures/{name}.{extension}");
                        let bytes = read(&path)?;
                        Self::upload_runtime_texture(device, queue, Path::new(&path), &bytes, 1)
                            .ok()
                    })?;
                    let view = texture.create_view(&Default::default());
                    Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("Authored projected shadow texture"),
                        layout: texture_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::Sampler(sampler),
                            },
                        ],
                    }))
                });
            bound.is_some()
        });
        let casters = inputs.iter().map(|shadow| shadow.object_id).collect();
        if gpu.inputs != inputs {
            gpu.inputs = inputs;
            gpu.geometry_draw_window = None;
        }
        casters
    }

    pub fn has_projected_unit_shadow_inputs(&self) -> bool {
        self.projected_shadow_gpu
            .as_ref()
            .is_some_and(|gpu| !gpu.inputs.is_empty())
    }

    pub fn retain_projected_unit_shadow_casters(
        &mut self,
        mut transform: impl FnMut(u32) -> Option<Mat4>,
    ) {
        if let Some(gpu) = self.projected_shadow_gpu.as_mut() {
            let mut changed = false;
            gpu.inputs.retain_mut(|shadow| {
                let Some(current) = transform(shadow.object_id) else {
                    changed = true;
                    return false;
                };
                if shadow.transform != current {
                    shadow.transform = current;
                    changed = true;
                }
                true
            });
            if changed {
                gpu.geometry_draw_window = None;
            }
        }
    }

    fn invalidate_projected_unit_shadow_meshes(&mut self) {
        if let Some(gpu) = self.projected_shadow_gpu.as_mut() {
            gpu.geometry_draw_window = None;
        }
    }

    /// Rebuild receiver geometry after changed casters, terrain, or clipping.
    /// The result is the number of bytes queued for the two vertex/index writes.
    fn update_projected_unit_shadow_meshes(&mut self) -> usize {
        let (Some(gpu), Some(map), Some(device), Some(queue)) = (
            self.projected_shadow_gpu.as_mut(),
            self.height_map.as_ref(),
            self.device.as_ref(),
            self.queue.as_ref(),
        ) else {
            return 0;
        };
        let start = [self.draw_origin_x, self.draw_origin_y];
        let end = [
            (start[0] + self.draw_width - 1).min(map.width as i32 - 1),
            (start[1] + self.draw_height - 1).min(map.height as i32 - 1),
        ];
        let draw_window = (start, end);
        if gpu.geometry_draw_window == Some(draw_window) {
            return 0;
        }
        gpu.geometry_draw_window = Some(draw_window);
        gpu.vertices.clear();
        gpu.indices.clear();
        gpu.batches.clear();
        for shadow in &gpu.inputs {
            let before = gpu.indices.len() as u32;
            append_projected_shadow(map, start, end, shadow, &mut gpu.vertices, &mut gpu.indices);
            let after = gpu.indices.len() as u32;
            if before == after {
                continue;
            }
            if let Some((texture, range)) = gpu.batches.last_mut()
                && texture == &shadow.texture_name
            {
                range.end = after;
            } else {
                gpu.batches
                    .push((shadow.texture_name.clone(), before..after));
            }
        }
        if gpu.indices.is_empty() {
            return 0;
        }
        let limit = device.limits().max_buffer_size;
        let vertices = cast_slice(&gpu.vertices);
        let indices = cast_slice(&gpu.indices);
        if vertices.len() as u64 > limit || indices.len() as u64 > limit {
            gpu.batches.clear();
            return 0;
        }
        ProjectedShadowGpu::upload_buffer(
            device,
            queue,
            &mut gpu.vertex_buffer,
            wgpu::BufferUsages::VERTEX,
            vertices,
            limit,
        );
        ProjectedShadowGpu::upload_buffer(
            device,
            queue,
            &mut gpu.index_buffer,
            wgpu::BufferUsages::INDEX,
            indices,
            limit,
        );
        vertices.len() + indices.len()
    }

    /// C++ `DoShadows(false)`, after terrain and before opaque scene models.
    pub fn record_projected_unit_shadows(&self, pass: &mut RenderPass<'_>) {
        let (Some(gpu), Some(camera)) = (
            self.projected_shadow_gpu.as_ref(),
            self.terrain_camera_bind_group.as_ref(),
        ) else {
            return;
        };
        let (Some(vertices), Some(indices)) =
            (gpu.vertex_buffer.as_ref(), gpu.index_buffer.as_ref())
        else {
            return;
        };
        if !self.enabled || gpu.batches.is_empty() {
            return;
        }
        pass.set_pipeline(&gpu.pipeline);
        pass.set_bind_group(0, camera, &[]);
        pass.set_vertex_buffer(0, vertices.slice(..));
        pass.set_index_buffer(indices.slice(..), wgpu::IndexFormat::Uint32);
        for (name, range) in &gpu.batches {
            if let Some(Some(texture)) = gpu.textures.get(name) {
                pass.set_bind_group(1, texture, &[]);
                pass.draw_indexed(range.clone(), 0, 0..1);
            }
        }
    }
}

/// `W3DProjectedShadowManager::queueDecal`: project UV onto the active
/// heightfield grid and use its same cell diagonal for receiver depth.
fn append_projected_shadow(
    map: &HeightMap,
    draw_start: [i32; 2],
    draw_end: [i32; 2],
    shadow: &ProjectedUnitShadow,
    vertices: &mut Vec<ProjectedShadowVertex>,
    indices: &mut Vec<u32>,
) {
    if !shadow.transform.is_finite()
        || !shadow.offset.is_finite()
        || !shadow.size.is_finite()
        || shadow.size.min_element() <= 0.0
        || map.scale <= 0.0
        || draw_start[0] >= draw_end[0]
        || draw_start[1] >= draw_end[1]
    {
        return;
    }
    let x = Vec2::new(shadow.transform.x_axis.x, shadow.transform.x_axis.z).normalize_or_zero();
    let (u, v) = if x != Vec2::ZERO {
        (x, -x.perp())
    } else {
        let y = Vec2::new(shadow.transform.z_axis.x, shadow.transform.z_axis.z).normalize_or_zero();
        let v = if y == Vec2::ZERO { Vec2::NEG_Y } else { y };
        (v.perp(), v)
    };
    // Authored offsets are added to the UV, moving its midpoint in the
    // opposite direction along each projected axis.
    let center = Vec2::new(shadow.transform.w_axis.x, shadow.transform.w_axis.z)
        - u * shadow.offset.x
        - v * shadow.offset.y;
    let half = (u.abs() * shadow.size.x + v.abs() * shadow.size.y) * 0.5;
    let min = ((center - half) / map.scale).floor().as_ivec2();
    let max = ((center + half) / map.scale).ceil().as_ivec2();
    let clamp = |a: i32, b: i32, draw_start: i32, draw_end: i32| {
        let mut lo = a
            .saturating_add(map.border_size)
            .clamp(draw_start, draw_end);
        let mut hi = b
            .saturating_add(map.border_size)
            .clamp(draw_start, draw_end);
        let excess = (hi - lo + 1 - 104).max(0);
        lo += excess / 2;
        hi -= excess - excess / 2;
        (lo, hi)
    };
    let (x0, x1) = clamp(min.x, max.x, draw_start[0], draw_end[0]);
    let (z0, z1) = clamp(min.y, max.y, draw_start[1], draw_end[1]);
    if x0 == x1 || z0 == z1 {
        return;
    }
    let base = vertices.len() as u32;
    for z in z0..=z1 {
        for x in x0..=x1 {
            let planar =
                Vec2::new((x - map.border_size) as f32, (z - map.border_size) as f32) * map.scale;
            let height = map.world_height_at_index(x as u32, z as u32);
            let y = shadow.layer_height.map_or_else(
                || height + 0.01 * map.scale,
                |layer| height.max(layer + 1.5),
            );
            let rel = planar - center;
            vertices.push(ProjectedShadowVertex {
                position: [planar.x, y, planar.y],
                uv: [
                    rel.dot(u) / shadow.size.x + 0.5,
                    rel.dot(v) / shadow.size.y + 0.5,
                ],
            });
        }
    }
    let stride = (x1 - x0 + 1) as u32;
    for z in z0..z1 {
        for x in x0..x1 {
            let i = base + (z - z0) as u32 * stride + (x - x0) as u32;
            let right = i + 1;
            let next = i + stride;
            if map.flip_state(x, z) {
                indices.extend_from_slice(&[right, next, i, right, next + 1, next]);
            } else {
                indices.extend_from_slice(&[i, next + 1, next, i, right, next + 1]);
            }
        }
    }
}

#[cfg(test)]
mod projected_unit_shadow_tests {
    use super::*;

    #[test]
    fn projected_unit_shadow_uses_rotated_offsets_and_receiver_triangles() {
        let mut map = HeightMap::new(8, 8, 255.0, 10.0);
        map.border_size = 1;
        map.set_raw_height(3, 2, 80);
        map.cliff_info.push(crate::terrain::height_map::TCliffInfo {
            flip: true,
            ..Default::default()
        });
        map.cliff_info_ndxes[2 * 8 + 3] = 1;
        let mut decal = ProjectedUnitShadow {
            object_id: 1,
            transform: Mat4::from_cols_array_2d(&[
                [0.0, 0.0, -1.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [1.0, 0.0, 0.0, 0.0],
                [19.0, 123.0, 21.0, 1.0],
            ]),
            size: Vec2::new(14.0, 10.0),
            offset: Vec2::new(4.0, 6.0),
            texture_name: "ShadowI".into(),
            layer_height: None,
        };
        let (mut vertices, mut indices) = (Vec::new(), Vec::new());
        append_projected_shadow(&map, [0, 0], [7, 7], &decal, &mut vertices, &mut indices);
        assert_eq!(vertices.len(), 8);
        assert_eq!(&indices[..6], &[1, 2, 0, 1, 3, 2]);
        assert_eq!(&indices[6..12], &[2, 5, 4, 2, 3, 5]);
        assert!((vertices[0].position[1] - 80.1).abs() < 0.0001);
        assert_eq!(
            (vertices[0].position[0], vertices[0].position[2]),
            (20.0, 10.0)
        );
        assert_eq!(vertices[0].uv, [15.0 / 14.0 + 0.5, 1.0]);
        assert_eq!(vertices[1].uv, [15.0 / 14.0 + 0.5, 0.0]);

        decal.layer_height = Some(33.0);
        vertices.clear();
        indices.clear();
        append_projected_shadow(&map, [0, 0], [7, 7], &decal, &mut vertices, &mut indices);
        assert!((vertices[0].position[1] - 80.0).abs() < 0.0001);
        assert_eq!(vertices[1].position[1], 34.5);
    }
}
