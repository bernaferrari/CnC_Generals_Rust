/*
**	Command & Conquer Generals Zero Hour(tm) Rust Port
**	Copyright 2025
**
**	Dazzle GPU Rendering System
**	Implements actual GPU rendering for dazzle effects (lens flares, halos, screen flashes)
*/

use glam::{Vec2, Vec3, Vec4};

/// Vertex format for dazzle quads
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DazzleVertex {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
    pub color: [f32; 4],
    pub size: f32,
    pub _padding: [f32; 3],
}

impl DazzleVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DazzleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // TexCoord
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 5]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
                // Size
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 9]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32,
                },
            ],
        }
    }
}

/// Uniform buffer for dazzle rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DazzleUniforms {
    pub view_proj: [[f32; 4]; 4],
    pub camera_pos: [f32; 3],
    pub time: f32,
    pub screen_size: [f32; 2],
    pub _padding: [f32; 2],
}

/// Dazzle render instance (one quad/sprite)
#[derive(Debug, Clone)]
pub struct DazzleRenderInstance {
    pub position: Vec3,
    pub size: f32,
    pub color: Vec4,
    pub intensity: f32,
    pub texture_index: usize,
    pub blend_mode: DazzleBlendMode,
}

/// Blend modes for dazzle rendering
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum DazzleBlendMode {
    Additive,
    AlphaBlend,
    Screen,
}

/// Render batch for grouping dazzle instances by blend mode and texture
#[derive(Debug, Clone)]
struct RenderBatch {
    blend_mode: DazzleBlendMode,
    texture_index: usize,
    start_index: u32,
    index_count: u32,
}

impl DazzleBlendMode {
    pub fn to_wgpu_blend(&self) -> wgpu::BlendState {
        match self {
            Self::Additive => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            Self::AlphaBlend => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::SrcAlpha,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            },
            Self::Screen => wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrc,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                    operation: wgpu::BlendOperation::Add,
                },
            },
        }
    }
}

/// Lens flare element
#[derive(Debug, Clone)]
pub struct LensFlareElement {
    pub offset: f32,     // Position along light-to-center axis (-1 to 1)
    pub size_scale: f32, // Size multiplier
    pub color: Vec4,     // Color tint
    pub texture_index: usize,
    pub rotation: f32, // Rotation in radians
}

/// Complete lens flare configuration
#[derive(Debug, Clone)]
pub struct LensFlareConfig {
    pub elements: Vec<LensFlareElement>,
    pub base_size: f32,
    pub intensity_scale: f32,
}

impl Default for LensFlareConfig {
    fn default() -> Self {
        Self {
            elements: vec![
                // Main glow
                LensFlareElement {
                    offset: 0.0,
                    size_scale: 2.0,
                    color: Vec4::new(1.0, 0.9, 0.8, 0.8),
                    texture_index: 0,
                    rotation: 0.0,
                },
                // Ring
                LensFlareElement {
                    offset: 0.0,
                    size_scale: 1.5,
                    color: Vec4::new(0.8, 0.9, 1.0, 0.4),
                    texture_index: 1,
                    rotation: 0.0,
                },
                // Hexagonal artifacts
                LensFlareElement {
                    offset: -0.3,
                    size_scale: 0.8,
                    color: Vec4::new(0.7, 0.8, 1.0, 0.3),
                    texture_index: 2,
                    rotation: 0.0,
                },
                LensFlareElement {
                    offset: -0.5,
                    size_scale: 0.5,
                    color: Vec4::new(1.0, 0.7, 0.5, 0.25),
                    texture_index: 2,
                    rotation: std::f32::consts::PI / 6.0,
                },
                LensFlareElement {
                    offset: 0.4,
                    size_scale: 0.6,
                    color: Vec4::new(0.8, 1.0, 0.7, 0.2),
                    texture_index: 2,
                    rotation: std::f32::consts::PI / 3.0,
                },
            ],
            base_size: 50.0,
            intensity_scale: 1.0,
        }
    }
}

/// Lens flare instance
pub struct LensFlare {
    pub world_position: Vec3,
    pub config: LensFlareConfig,
    pub intensity: f32,
    pub visible: bool,
}

impl LensFlare {
    pub fn new(world_position: Vec3, config: LensFlareConfig) -> Self {
        Self {
            world_position,
            config,
            intensity: 1.0,
            visible: true,
        }
    }

    /// Generate render instances for this lens flare
    pub fn generate_instances(
        &self,
        screen_pos: Vec2,
        screen_center: Vec2,
        camera_distance: f32,
    ) -> Vec<DazzleRenderInstance> {
        if !self.visible || self.intensity < 0.01 {
            return Vec::new();
        }

        let mut instances = Vec::new();

        // Vector from light to screen center
        let to_center = screen_center - screen_pos;

        for element in &self.config.elements {
            // Position along line from light to center
            let element_pos_2d = screen_pos + to_center * element.offset;

            // Scale size based on distance
            let distance_scale = 1.0 / (1.0 + camera_distance * 0.1);
            let final_size = self.config.base_size * element.size_scale * distance_scale;

            // Combine colors and intensity
            let final_color = element.color * self.intensity * self.config.intensity_scale;

            instances.push(DazzleRenderInstance {
                position: Vec3::new(element_pos_2d.x, element_pos_2d.y, 0.0),
                size: final_size,
                color: final_color,
                intensity: self.intensity,
                texture_index: element.texture_index,
                blend_mode: DazzleBlendMode::Additive,
            });
        }

        instances
    }
}

/// GPU renderer for dazzle effects
#[allow(dead_code)] // C++ parity
pub struct DazzleGpuRenderer {
    pipeline_additive: wgpu::RenderPipeline,
    pipeline_alpha: wgpu::RenderPipeline,
    pipeline_screen: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_groups: Vec<wgpu::BindGroup>,
    max_instances: usize,
}

impl DazzleGpuRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        textures: &[&wgpu::Texture],
    ) -> Self {
        let max_instances = 1024;

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Dazzle Shader"),
            source: wgpu::ShaderSource::Wgsl(Self::SHADER_SOURCE.into()),
        });

        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dazzle Uniform Buffer"),
            size: std::mem::size_of::<DazzleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout for uniforms
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Dazzle Uniform Bind Group Layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        // Create uniform bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Dazzle Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create texture bind group layout
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Dazzle Texture Bind Group Layout"),
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

        // Create sampler
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Dazzle Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        // Create texture bind groups
        let texture_bind_groups: Vec<_> = textures
            .iter()
            .map(|texture| {
                let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
                device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("Dazzle Texture Bind Group"),
                    layout: &texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&sampler),
                        },
                    ],
                })
            })
            .collect();

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Dazzle Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Helper to create pipeline with different blend mode
        let create_pipeline = |blend: wgpu::BlendState, label: &str| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[DazzleVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(blend),
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
            })
        };

        let pipeline_additive = create_pipeline(
            DazzleBlendMode::Additive.to_wgpu_blend(),
            "Dazzle Pipeline Additive",
        );
        let pipeline_alpha = create_pipeline(
            DazzleBlendMode::AlphaBlend.to_wgpu_blend(),
            "Dazzle Pipeline Alpha",
        );
        let pipeline_screen = create_pipeline(
            DazzleBlendMode::Screen.to_wgpu_blend(),
            "Dazzle Pipeline Screen",
        );

        // Create vertex and index buffers
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dazzle Vertex Buffer"),
            size: (max_instances * 4 * std::mem::size_of::<DazzleVertex>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Dazzle Index Buffer"),
            size: (max_instances * 6 * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Self {
            pipeline_additive,
            pipeline_alpha,
            pipeline_screen,
            vertex_buffer,
            index_buffer,
            uniform_buffer,
            uniform_bind_group,
            texture_bind_groups,
            max_instances,
        }
    }

    /// Render dazzle instances
    pub fn render(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        instances: &[DazzleRenderInstance],
        uniforms: &DazzleUniforms,
        queue: &wgpu::Queue,
    ) {
        if instances.is_empty() {
            return;
        }

        // Update uniforms
        queue.write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        // Generate geometry and update buffers (C++ dazzle.cpp lines 367-428)
        let (vertices, indices) = self.generate_dazzle_geometry(instances, uniforms);

        if vertices.is_empty() || indices.is_empty() {
            return;
        }

        // Write geometry to buffers
        queue.write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(&vertices));
        queue.write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(&indices));

        // Sort instances by blend mode and texture (C++ dazzle.cpp rendering order)
        let sorted_batches = self.sort_instances_into_batches(instances);

        let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("Dazzle Render Pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        // Bind uniform buffer
        render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

        // Batch render calls (C++ dazzle.cpp rendering loop)
        self.render_batches(&mut render_pass, &sorted_batches);
    }

    /// Generate quad geometry for dazzle sprites (C++ dazzle.cpp lines 367-428)
    fn generate_dazzle_geometry(
        &self,
        instances: &[DazzleRenderInstance],
        uniforms: &DazzleUniforms,
    ) -> (Vec<DazzleVertex>, Vec<u32>) {
        let mut vertices = Vec::with_capacity(instances.len() * 4);
        let mut indices = Vec::with_capacity(instances.len() * 6);

        let _camera_pos = Vec3::from_slice(&uniforms.camera_pos);

        for (_i, instance) in instances.iter().enumerate() {
            // Calculate billboard axes in screen space
            // Dazzles are always screen-aligned quads
            let half_size = instance.size * 0.5;

            // Create quad vertices (billboarded to camera)
            // The quad is created in NDC space, already screen-aligned
            let center = instance.position;

            let base_idx = vertices.len() as u32;

            // Top-left
            vertices.push(DazzleVertex {
                position: [center.x - half_size, center.y + half_size, center.z],
                tex_coord: [0.0, 0.0],
                color: instance.color.to_array(),
                size: instance.size,
                _padding: [0.0; 3],
            });

            // Top-right
            vertices.push(DazzleVertex {
                position: [center.x + half_size, center.y + half_size, center.z],
                tex_coord: [1.0, 0.0],
                color: instance.color.to_array(),
                size: instance.size,
                _padding: [0.0; 3],
            });

            // Bottom-right
            vertices.push(DazzleVertex {
                position: [center.x + half_size, center.y - half_size, center.z],
                tex_coord: [1.0, 1.0],
                color: instance.color.to_array(),
                size: instance.size,
                _padding: [0.0; 3],
            });

            // Bottom-left
            vertices.push(DazzleVertex {
                position: [center.x - half_size, center.y - half_size, center.z],
                tex_coord: [0.0, 1.0],
                color: instance.color.to_array(),
                size: instance.size,
                _padding: [0.0; 3],
            });

            // Two triangles per quad
            indices.extend_from_slice(&[
                base_idx,
                base_idx + 1,
                base_idx + 2,
                base_idx,
                base_idx + 2,
                base_idx + 3,
            ]);
        }

        (vertices, indices)
    }

    /// Sort instances by blend mode and texture for efficient batching (C++ dazzle.cpp rendering order)
    fn sort_instances_into_batches(&self, instances: &[DazzleRenderInstance]) -> Vec<RenderBatch> {
        let mut batches = Vec::new();

        if instances.is_empty() {
            return batches;
        }

        // Create sorted instance indices
        let mut sorted_indices: Vec<usize> = (0..instances.len()).collect();

        // Sort by blend mode first, then by texture
        sorted_indices.sort_by(|&a, &b| {
            let inst_a = &instances[a];
            let inst_b = &instances[b];

            // First by blend mode (Additive < AlphaBlend < Screen)
            match inst_a.blend_mode.partial_cmp(&inst_b.blend_mode) {
                Some(std::cmp::Ordering::Equal) => {
                    // Then by texture index
                    inst_a.texture_index.cmp(&inst_b.texture_index)
                }
                Some(order) => order,
                None => std::cmp::Ordering::Equal,
            }
        });

        // Build batches from sorted instances
        let mut current_blend = instances[sorted_indices[0]].blend_mode;
        let mut current_texture = instances[sorted_indices[0]].texture_index;
        let mut batch_start = 0u32;
        let mut current_count = 0u32;

        for (i, &idx) in sorted_indices.iter().enumerate() {
            let instance = &instances[idx];

            // Check if we need to start a new batch
            if instance.blend_mode != current_blend || instance.texture_index != current_texture {
                // Save current batch
                batches.push(RenderBatch {
                    blend_mode: current_blend,
                    texture_index: current_texture,
                    start_index: batch_start * 6, // 6 indices per quad
                    index_count: current_count * 6,
                });

                // Start new batch
                current_blend = instance.blend_mode;
                current_texture = instance.texture_index;
                batch_start = i as u32;
                current_count = 1;
            } else {
                current_count += 1;
            }
        }

        // Add final batch
        batches.push(RenderBatch {
            blend_mode: current_blend,
            texture_index: current_texture,
            start_index: batch_start * 6,
            index_count: current_count * 6,
        });

        batches
    }

    /// Execute batched rendering (C++ dazzle.cpp rendering loop)
    fn render_batches<'a>(
        &'a self,
        render_pass: &mut wgpu::RenderPass<'a>,
        batches: &[RenderBatch],
    ) {
        // Set vertex and index buffers
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        for batch in batches {
            // Select pipeline based on blend mode
            let pipeline = match batch.blend_mode {
                DazzleBlendMode::Additive => &self.pipeline_additive,
                DazzleBlendMode::AlphaBlend => &self.pipeline_alpha,
                DazzleBlendMode::Screen => &self.pipeline_screen,
            };

            render_pass.set_pipeline(pipeline);

            // Bind texture if available
            if batch.texture_index < self.texture_bind_groups.len() {
                render_pass.set_bind_group(1, &self.texture_bind_groups[batch.texture_index], &[]);
            }

            // Draw this batch
            render_pass.draw_indexed(
                batch.start_index..(batch.start_index + batch.index_count),
                0,
                0..1,
            );
        }
    }

    const SHADER_SOURCE: &'static str = r#"
struct Uniforms {
    view_proj: mat4x4<f32>,
    camera_pos: vec3<f32>,
    time: f32,
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(1) @binding(0)
var tex: texture_2d<f32>;

@group(1) @binding(1)
var tex_sampler: sampler;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coord: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) size: f32,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coord: vec2<f32>,
    @location(1) color: vec4<f32>,
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = uniforms.view_proj * vec4<f32>(in.position, 1.0);
    out.tex_coord = in.tex_coord;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    var tex_color = textureSample(tex, tex_sampler, in.tex_coord);
    return tex_color * in.color;
}
"#;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lens_flare_generation() {
        let flare = LensFlare::new(Vec3::ZERO, LensFlareConfig::default());
        let screen_pos = Vec2::new(100.0, 100.0);
        let screen_center = Vec2::new(400.0, 300.0);
        let instances = flare.generate_instances(screen_pos, screen_center, 10.0);

        assert_eq!(instances.len(), 5); // Default config has 5 elements
    }

    #[test]
    fn test_blend_mode_conversion() {
        let additive = DazzleBlendMode::Additive.to_wgpu_blend();
        assert_eq!(additive.color.operation, wgpu::BlendOperation::Add);
    }
}
