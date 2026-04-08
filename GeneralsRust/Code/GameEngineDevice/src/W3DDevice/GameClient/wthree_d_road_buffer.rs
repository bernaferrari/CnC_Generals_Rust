use anyhow::Result;
use bytemuck::{Pod, Zeroable};
use cgmath::{InnerSpace, Matrix4, Point3, Vector2, Vector3};
use std::sync::Arc;
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingType, BlendState, Buffer, BufferAddress, BufferBindingType,
    BufferUsages, ColorTargetState, ColorWrites, CompareFunction, DepthStencilState, Device, Face,
    FragmentState, FrontFace, IndexFormat, MultisampleState, PipelineLayoutDescriptor,
    PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPipeline, Sampler,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexState, VertexStepMode,
};

const ROAD_LOD_SWITCH_DISTANCE: f32 = 300.0;
const DEFAULT_ROAD_TEXTURE: [u8; 16] = [
    96, 96, 96, 255, 120, 120, 120, 255, 120, 120, 120, 255, 96, 96, 96, 255,
];

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct RoadVertex {
    position: [f32; 3],
    color: u32,
    uv: [f32; 2],
    edge: [f32; 2],
}

impl RoadVertex {
    fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<RoadVertex>() as BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Uint32,
                },
                VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>() + std::mem::size_of::<u32>())
                        as BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: (std::mem::size_of::<[f32; 3]>()
                        + std::mem::size_of::<u32>()
                        + std::mem::size_of::<[f32; 2]>())
                        as BufferAddress,
                    shader_location: 3,
                    format: VertexFormat::Float32x2,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct RoadUniforms {
    view_proj: [[f32; 4]; 4],
    tint: [f32; 4],
}

#[derive(Debug, Clone)]
pub enum RoadSegmentKind {
    Straight,
    Curve,
    Intersection3,
    Intersection4,
}

#[derive(Debug, Clone)]
pub struct RoadSegmentDefinition {
    pub points: Vec<[f32; 2]>,
    pub width: f32,
    pub texture_scale: f32,
    pub road_type: u32,
    pub kind: RoadSegmentKind,
}

impl RoadSegmentDefinition {
    pub fn straight(start: [f32; 2], end: [f32; 2], width: f32) -> Self {
        Self {
            points: vec![start, end],
            width,
            texture_scale: 1.0,
            road_type: 0,
            kind: RoadSegmentKind::Straight,
        }
    }
}

struct RoadGpuMesh {
    near_vertex: Buffer,
    near_index: Buffer,
    near_index_count: u32,
    far_vertex: Buffer,
    far_index: Buffer,
    far_index_count: u32,
    center: Vector3<f32>,
    radius: f32,
}

pub struct WthreeDRoadBuffer {
    device: Option<Arc<Device>>,
    queue: Option<Arc<Queue>>,
    pipeline: Option<RenderPipeline>,
    bind_group_layout: Option<BindGroupLayout>,
    bind_group: Option<BindGroup>,
    uniform_buffer: Option<Buffer>,
    road_texture: Option<Texture>,
    road_texture_view: Option<TextureView>,
    road_sampler: Option<Sampler>,
    road_segments: Vec<RoadSegmentDefinition>,
    road_meshes: Vec<RoadGpuMesh>,
    height_map_width: usize,
    height_map_height: usize,
    height_data: Vec<u8>,
    camera_position: Point3<f32>,
    tint: [f32; 4],
}

impl Default for WthreeDRoadBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl WthreeDRoadBuffer {
    pub fn new() -> Self {
        Self {
            device: None,
            queue: None,
            pipeline: None,
            bind_group_layout: None,
            bind_group: None,
            uniform_buffer: None,
            road_texture: None,
            road_texture_view: None,
            road_sampler: None,
            road_segments: Vec::new(),
            road_meshes: Vec::new(),
            height_map_width: 0,
            height_map_height: 0,
            height_data: Vec::new(),
            camera_position: Point3::new(0.0, 0.0, 0.0),
            tint: [1.0, 1.0, 1.0, 1.0],
        }
    }

    pub fn init(
        &mut self,
        device: Arc<Device>,
        queue: Arc<Queue>,
        surface_format: TextureFormat,
        depth_format: TextureFormat,
    ) -> Result<()> {
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Road Buffer Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::VERTEX_FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        multisampled: false,
                        view_dimension: TextureViewDimension::D2,
                        sample_type: TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Road Uniform Buffer"),
            size: std::mem::size_of::<RoadUniforms>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Road Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let (road_texture, road_texture_view) = Self::create_texture(
            &device,
            &queue,
            2,
            2,
            &DEFAULT_ROAD_TEXTURE,
            "Default Road Texture",
        );

        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Road Buffer Bind Group"),
            layout: &bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&road_texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });

        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Road Buffer Shader"),
            source: ShaderSource::Wgsl(ROAD_SHADER.into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Road Buffer Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Road Buffer Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[RoadVertex::desc()],
            },
            fragment: Some(FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(ColorTargetState {
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
            }),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: Some(DepthStencilState {
                format: depth_format,
                depth_write_enabled: false,
                depth_compare: CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        self.device = Some(device);
        self.queue = Some(queue);
        self.pipeline = Some(pipeline);
        self.bind_group_layout = Some(bind_group_layout);
        self.bind_group = Some(bind_group);
        self.uniform_buffer = Some(uniform_buffer);
        self.road_texture = Some(road_texture);
        self.road_texture_view = Some(road_texture_view);
        self.road_sampler = Some(sampler);
        Ok(())
    }

    pub fn set_map(&mut self, width: usize, height: usize, height_data: Vec<u8>) {
        self.height_map_width = width;
        self.height_map_height = height;
        self.height_data = height_data;
    }

    pub fn set_texture_rgba(&mut self, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
        let device = self.device.as_ref().expect("road buffer not initialized");
        let queue = self.queue.as_ref().expect("road buffer not initialized");
        let bind_group_layout = self
            .bind_group_layout
            .as_ref()
            .expect("road buffer not initialized");
        let uniform_buffer = self
            .uniform_buffer
            .as_ref()
            .expect("road buffer not initialized");
        let sampler = self
            .road_sampler
            .as_ref()
            .expect("road buffer not initialized");

        let (texture, texture_view) =
            Self::create_texture(device, queue, width, height, rgba, "Road Texture");
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Road Buffer Bind Group"),
            layout: bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: uniform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(sampler),
                },
            ],
        });

        self.road_texture = Some(texture);
        self.road_texture_view = Some(texture_view);
        self.bind_group = Some(bind_group);
        Ok(())
    }

    pub fn load_roads(&mut self, roads: Vec<RoadSegmentDefinition>) -> Result<()> {
        self.road_segments = roads;
        self.rebuild_meshes()
    }

    pub fn update_center(&mut self, camera_position: Point3<f32>) {
        self.camera_position = camera_position;
    }

    pub fn update_lighting(&mut self, tint: [f32; 4]) {
        self.tint = tint;
    }

    pub fn draw_roads<'a>(&'a self, render_pass: &mut RenderPass<'a>, view_proj: Matrix4<f32>) {
        let (pipeline, bind_group, uniform_buffer, queue) = match (
            self.pipeline.as_ref(),
            self.bind_group.as_ref(),
            self.uniform_buffer.as_ref(),
            self.queue.as_ref(),
        ) {
            (Some(pipeline), Some(bind_group), Some(uniform_buffer), Some(queue)) => {
                (pipeline, bind_group, uniform_buffer, queue)
            }
            _ => return,
        };

        let uniforms = RoadUniforms {
            view_proj: view_proj.into(),
            tint: self.tint,
        };
        queue.write_buffer(uniform_buffer, 0, bytemuck::bytes_of(&uniforms));

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, bind_group, &[]);
        for mesh in &self.road_meshes {
            let distance = (mesh.center - self.camera_position.to_vec()).magnitude() - mesh.radius;
            let use_far_lod = distance > ROAD_LOD_SWITCH_DISTANCE;
            if use_far_lod {
                render_pass.set_vertex_buffer(0, mesh.far_vertex.slice(..));
                render_pass.set_index_buffer(mesh.far_index.slice(..), IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.far_index_count, 0, 0..1);
            } else {
                render_pass.set_vertex_buffer(0, mesh.near_vertex.slice(..));
                render_pass.set_index_buffer(mesh.near_index.slice(..), IndexFormat::Uint32);
                render_pass.draw_indexed(0..mesh.near_index_count, 0, 0..1);
            }
        }
    }

    fn rebuild_meshes(&mut self) -> Result<()> {
        let device = self.device.as_ref().expect("road buffer not initialized");
        self.road_meshes.clear();

        for segment in &self.road_segments {
            let near_geometry =
                self.build_geometry(segment, Self::segment_subdivisions(&segment.kind, false));
            let far_geometry =
                self.build_geometry(segment, Self::segment_subdivisions(&segment.kind, true));
            if near_geometry.0.is_empty() || near_geometry.1.is_empty() {
                continue;
            }

            let center = Self::compute_center(&near_geometry.0);
            let radius = near_geometry
                .0
                .iter()
                .map(|vertex| {
                    let delta = Vector3::new(
                        vertex.position[0] - center.x,
                        vertex.position[1] - center.y,
                        vertex.position[2] - center.z,
                    );
                    delta.magnitude()
                })
                .fold(0.0, f32::max);

            self.road_meshes.push(RoadGpuMesh {
                near_vertex: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Road Near Vertex Buffer"),
                    contents: bytemuck::cast_slice(&near_geometry.0),
                    usage: BufferUsages::VERTEX,
                }),
                near_index: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Road Near Index Buffer"),
                    contents: bytemuck::cast_slice(&near_geometry.1),
                    usage: BufferUsages::INDEX,
                }),
                near_index_count: near_geometry.1.len() as u32,
                far_vertex: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Road Far Vertex Buffer"),
                    contents: bytemuck::cast_slice(&far_geometry.0),
                    usage: BufferUsages::VERTEX,
                }),
                far_index: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("Road Far Index Buffer"),
                    contents: bytemuck::cast_slice(&far_geometry.1),
                    usage: BufferUsages::INDEX,
                }),
                far_index_count: far_geometry.1.len() as u32,
                center,
                radius,
            });
        }

        Ok(())
    }

    fn build_geometry(
        &self,
        segment: &RoadSegmentDefinition,
        subdivisions: usize,
    ) -> (Vec<RoadVertex>, Vec<u32>) {
        let sampled_points = Self::resample_points(&segment.points, subdivisions.max(1));
        if sampled_points.len() < 2 {
            return (Vec::new(), Vec::new());
        }

        let width = segment.width.max(1.0);
        let texture_scale = segment.texture_scale.max(0.01);
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let color = 0xFFFF_FFFF;
        let mut distance_along = 0.0f32;

        for i in 0..sampled_points.len() {
            let point = Vector2::new(sampled_points[i][0], sampled_points[i][1]);
            let prev = if i > 0 {
                Vector2::new(sampled_points[i - 1][0], sampled_points[i - 1][1])
            } else {
                point
            };
            let next = if i + 1 < sampled_points.len() {
                Vector2::new(sampled_points[i + 1][0], sampled_points[i + 1][1])
            } else {
                point
            };
            let tangent = if (next - prev).magnitude2() > 0.0 {
                (next - prev).normalize()
            } else {
                Vector2::new(1.0, 0.0)
            };
            let normal = Vector2::new(-tangent.y, tangent.x);
            let left = point + normal * (width * 0.5);
            let right = point - normal * (width * 0.5);
            let left_height = self.sample_height(left.x, left.y);
            let right_height = self.sample_height(right.x, right.y);
            let v = distance_along / texture_scale;
            vertices.push(RoadVertex {
                position: [left.x, left.y, left_height],
                color,
                uv: [0.0, v],
                edge: [-1.0, 0.0],
            });
            vertices.push(RoadVertex {
                position: [right.x, right.y, right_height],
                color,
                uv: [1.0, v],
                edge: [1.0, 0.0],
            });

            if i + 1 < sampled_points.len() {
                let current = Vector2::new(sampled_points[i][0], sampled_points[i][1]);
                let upcoming = Vector2::new(sampled_points[i + 1][0], sampled_points[i + 1][1]);
                distance_along += (upcoming - current).magnitude();
                let base = (i as u32) * 2;
                indices.extend_from_slice(&[
                    base,
                    base + 1,
                    base + 2,
                    base + 1,
                    base + 3,
                    base + 2,
                ]);
            }
        }

        match segment.kind {
            RoadSegmentKind::Intersection3 | RoadSegmentKind::Intersection4 => {
                self.append_intersection_fan(segment, &mut vertices, &mut indices)
            }
            _ => {}
        }

        (vertices, indices)
    }

    fn append_intersection_fan(
        &self,
        segment: &RoadSegmentDefinition,
        vertices: &mut Vec<RoadVertex>,
        indices: &mut Vec<u32>,
    ) {
        if segment.points.is_empty() {
            return;
        }
        let center = segment
            .points
            .iter()
            .fold(Vector2::new(0.0, 0.0), |acc, p| {
                acc + Vector2::new(p[0], p[1])
            })
            / segment.points.len() as f32;
        let center_index = vertices.len() as u32;
        vertices.push(RoadVertex {
            position: [center.x, center.y, self.sample_height(center.x, center.y)],
            color: 0xFFFF_FFFF,
            uv: [0.5, 0.5],
            edge: [0.0, 1.0],
        });

        let mut ring = Vec::new();
        for point in &segment.points {
            let dir = Vector2::new(point[0], point[1]) - center;
            if dir.magnitude2() == 0.0 {
                continue;
            }
            let dir = dir.normalize();
            let edge = center + dir * (segment.width * 0.5);
            ring.push(vertices.len() as u32);
            vertices.push(RoadVertex {
                position: [edge.x, edge.y, self.sample_height(edge.x, edge.y)],
                color: 0xFFFF_FFFF,
                uv: [dir.x * 0.5 + 0.5, dir.y * 0.5 + 0.5],
                edge: [0.0, 0.0],
            });
        }

        if ring.len() >= 3 {
            for i in 0..ring.len() {
                let a = ring[i];
                let b = ring[(i + 1) % ring.len()];
                indices.extend_from_slice(&[center_index, a, b]);
            }
        }
    }

    fn segment_subdivisions(kind: &RoadSegmentKind, far_lod: bool) -> usize {
        match (kind, far_lod) {
            (RoadSegmentKind::Curve, false) => 8,
            (RoadSegmentKind::Curve, true) => 3,
            (RoadSegmentKind::Intersection3 | RoadSegmentKind::Intersection4, false) => 2,
            _ => 1,
        }
    }

    fn resample_points(points: &[[f32; 2]], subdivisions: usize) -> Vec<[f32; 2]> {
        if points.len() <= 2 || subdivisions <= 1 {
            return points.to_vec();
        }

        let mut result = Vec::new();
        for window in points.windows(2) {
            let start = Vector2::new(window[0][0], window[0][1]);
            let end = Vector2::new(window[1][0], window[1][1]);
            if result.is_empty() {
                result.push([start.x, start.y]);
            }
            for step in 1..=subdivisions {
                let t = step as f32 / subdivisions as f32;
                let point = start + (end - start) * t;
                result.push([point.x, point.y]);
            }
        }
        result
    }

    fn sample_height(&self, x: f32, y: f32) -> f32 {
        if self.height_map_width == 0 || self.height_map_height == 0 || self.height_data.is_empty()
        {
            return 0.0;
        }
        let grid_x = (x / 10.0)
            .round()
            .clamp(0.0, (self.height_map_width.saturating_sub(1)) as f32)
            as usize;
        let grid_y = (y / 10.0)
            .round()
            .clamp(0.0, (self.height_map_height.saturating_sub(1)) as f32)
            as usize;
        let idx = grid_y * self.height_map_width + grid_x;
        self.height_data.get(idx).copied().unwrap_or_default() as f32 * 0.625
    }

    fn compute_center(vertices: &[RoadVertex]) -> Vector3<f32> {
        let mut sum = Vector3::new(0.0, 0.0, 0.0);
        for vertex in vertices {
            sum += Vector3::new(vertex.position[0], vertex.position[1], vertex.position[2]);
        }
        if vertices.is_empty() {
            sum
        } else {
            sum / vertices.len() as f32
        }
    }

    fn create_texture(
        device: &Device,
        queue: &Queue,
        width: u32,
        height: u32,
        rgba: &[u8],
        label: &str,
    ) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some(label),
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
        let view = texture.create_view(&TextureViewDescriptor::default());
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        (texture, view)
    }
}

const ROAD_SHADER: &str = r#"
struct RoadUniforms {
    view_proj: mat4x4<f32>,
    tint: vec4<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: RoadUniforms;
@group(0) @binding(1) var road_texture: texture_2d<f32>;
@group(0) @binding(2) var road_sampler: sampler;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) color: u32,
    @location(2) uv: vec2<f32>,
    @location(3) edge: vec2<f32>,
};

struct VsOut {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) edge: vec2<f32>,
};

@vertex
fn vs_main(input: VsIn) -> VsOut {
    var out: VsOut;
    out.position = uniforms.view_proj * vec4<f32>(input.position, 1.0);
    out.uv = input.uv;
    out.edge = input.edge;
    return out;
}

@fragment
fn fs_main(input: VsOut) -> @location(0) vec4<f32> {
    let sampled = textureSample(road_texture, road_sampler, input.uv);
    let edge_falloff = 1.0 - smoothstep(0.75, 1.0, abs(input.edge.x));
    return vec4<f32>(sampled.rgb * uniforms.tint.rgb, sampled.a * edge_falloff * uniforms.tint.a);
}
"#;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wthree_d_road_buffer_basic() {
        let road = RoadSegmentDefinition::straight([0.0, 0.0], [100.0, 0.0], 12.0);
        let buffer = WthreeDRoadBuffer::new();
        assert_eq!(road.points.len(), 2);
        assert!(buffer.road_segments.is_empty());
    }
}
