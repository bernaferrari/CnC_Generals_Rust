//! # GPU Particle Renderer
//!
//! High-performance GPU-accelerated particle rendering with WGPU.
//! Supports all C++ particle shader modes: additive, alpha, alpha test, multiply.
//! Uses instanced rendering and GPU compute shaders for maximum performance.

use bytemuck::{Pod, Zeroable};
use image::GenericImageView;
use nalgebra::{Matrix4, Point3, Vector3, Vector4};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, OnceLock};
use wgpu::util::DeviceExt;

use super::decals::DecalRenderItem;
use super::particle_manager::*;
use super::particle_system::{Particle, ParticleSystem};
use super::weather_complete::WeatherParticle;

/// Maximum particles per batch for GPU rendering
pub const MAX_PARTICLES_PER_BATCH: usize = 10000;

/// Particle vertex data for GPU (matches C++ billboard rendering)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ParticleVertex {
    /// World position
    pub position: [f32; 3],
    /// Size (width, height, 0, 0)
    pub size: [f32; 2],
    /// Color (RGBA)
    pub color: [f32; 4],
    /// UV coordinates (u_min, v_min, u_max, v_max)
    pub uv_rect: [f32; 4],
    /// Rotation angle in radians
    pub rotation: f32,
    /// Alpha value (for separate alpha control)
    pub alpha: f32,
    /// Padding to keep the instance stride on a 16-byte boundary.
    pub _padding: f32,
}

impl Default for ParticleVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            size: [1.0, 1.0],
            color: [1.0; 4],
            uv_rect: [0.0, 0.0, 1.0, 1.0],
            rotation: 0.0,
            alpha: 1.0,
            _padding: 0.0,
        }
    }
}

/// GPU uniform data for particle rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ParticleUniforms {
    /// View matrix
    pub view_matrix: [[f32; 4]; 4],
    /// Projection matrix  
    pub projection_matrix: [[f32; 4]; 4],
    /// Camera position
    pub camera_position: [f32; 3],
    /// Time for animation
    pub time: f32,
    /// Screen dimensions
    pub screen_size: [f32; 2],
    /// Particle count this frame
    pub particle_count: u32,
    /// Padding
    pub _padding: u32,
}

impl Default for ParticleUniforms {
    fn default() -> Self {
        Self {
            view_matrix: Matrix4::identity().into(),
            projection_matrix: Matrix4::identity().into(),
            camera_position: [0.0; 3],
            time: 0.0,
            screen_size: [1024.0, 768.0],
            particle_count: 0,
            _padding: 0,
        }
    }
}

/// Particle batch for rendering (groups particles by shader type and texture)
pub struct ParticleBatch {
    /// Shader type for this batch
    pub shader_type: ParticleShaderType,
    /// Texture name/path
    pub texture_name: String,
    /// Particle vertices
    pub vertices: Vec<ParticleVertex>,
    /// GPU vertex buffer
    pub vertex_buffer: Option<wgpu::Buffer>,
    /// Needs buffer update
    pub dirty: bool,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct DecalVertex {
    pub position: [f32; 3],
    pub color: [f32; 4],
}

impl ParticleBatch {
    pub fn new(shader_type: ParticleShaderType, texture_name: String) -> Self {
        Self {
            shader_type,
            texture_name,
            vertices: Vec::with_capacity(MAX_PARTICLES_PER_BATCH),
            vertex_buffer: None,
            dirty: true,
        }
    }

    /// Add particle to batch
    pub fn add_particle(&mut self, particle: &Particle, system: &ParticleSystem) {
        if self.vertices.len() >= MAX_PARTICLES_PER_BATCH {
            return; // Batch full
        }

        let vertex = ParticleVertex {
            position: [
                particle.position.x,
                particle.position.y,
                particle.position.z,
            ],
            size: [particle.size, particle.size],
            color: [
                particle.color[0] * particle.color_scale,
                particle.color[1] * particle.color_scale,
                particle.color[2] * particle.color_scale,
                1.0,
            ],
            uv_rect: [0.0, 0.0, 1.0, 1.0], // Will be set based on texture atlas
            rotation: particle.angle_z,
            alpha: particle.alpha,
            _padding: 0.0,
        };

        self.vertices.push(vertex);
        self.dirty = true;
    }

    /// Clear batch
    pub fn clear(&mut self) {
        self.vertices.clear();
        self.dirty = true;
    }

    /// Update GPU buffer
    pub fn update_buffer(&mut self, device: &wgpu::Device) {
        if !self.dirty || self.vertices.is_empty() {
            return;
        }

        let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Particle Vertex Buffer"),
            contents: bytemuck::cast_slice(&self.vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        self.vertex_buffer = Some(buffer);
        self.dirty = false;
    }
}

/// GPU particle renderer
pub struct ParticleRenderer {
    /// Graphics device
    device: Arc<wgpu::Device>,
    /// Command queue
    queue: Arc<wgpu::Queue>,

    /// Render pipelines for different shader modes
    additive_pipeline: wgpu::RenderPipeline,
    alpha_pipeline: wgpu::RenderPipeline,
    alpha_test_pipeline: wgpu::RenderPipeline,
    multiply_pipeline: wgpu::RenderPipeline,
    decal_pipeline: wgpu::RenderPipeline,

    /// Uniform buffer
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    texture_bind_group_layout: wgpu::BindGroupLayout,

    /// Texture atlas for particle textures
    texture_atlas: HashMap<String, wgpu::Texture>,
    texture_bind_groups: HashMap<String, wgpu::BindGroup>,

    /// Batches grouped by shader and texture
    batches: HashMap<String, ParticleBatch>,

    /// Default texture for particles without specific texture
    default_texture: wgpu::Texture,
    default_bind_group: wgpu::BindGroup,

    /// Billboard vertices (quad)
    billboard_buffer: wgpu::Buffer,

    /// Performance stats
    pub stats: ParticleRenderStats,
}

static PARTICLE_RENDERER: OnceLock<Arc<Mutex<ParticleRenderer>>> = OnceLock::new();

pub fn register_particle_renderer(renderer: Arc<Mutex<ParticleRenderer>>) {
    let _ = PARTICLE_RENDERER.set(renderer);
}

pub fn with_particle_renderer<R>(f: impl FnOnce(&Arc<Mutex<ParticleRenderer>>) -> R) -> Option<R> {
    PARTICLE_RENDERER.get().map(f)
}

/// Particle rendering statistics
#[derive(Debug, Default)]
pub struct ParticleRenderStats {
    pub particles_rendered: usize,
    pub batches_rendered: usize,
    pub draw_calls: usize,
    pub gpu_memory_used: usize,
    pub render_time_ms: f64,
}

impl ParticleRenderStats {
    fn reset_frame_counters(&mut self) {
        self.particles_rendered = 0;
        self.batches_rendered = 0;
        self.draw_calls = 0;
        self.render_time_ms = 0.0;
    }
}

impl ParticleRenderer {
    /// Create new particle renderer
    pub fn new(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        surface_format: wgpu::TextureFormat,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        // Create uniform buffer
        let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Particle Uniforms"),
            size: std::mem::size_of::<ParticleUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create bind group layout for uniforms
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Uniform Bind Group Layout"),
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

        // Create bind group layout for textures
        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Particle Texture Bind Group Layout"),
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

        // Create uniform bind group
        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Uniform Bind Group"),
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
        });

        // Create render pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Particle Pipeline Layout"),
            bind_group_layouts: &[&uniform_bind_group_layout, &texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        // Load shaders
        let vertex_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Vertex Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_vertex.wgsl").into()),
        });

        let additive_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Additive Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_additive.wgsl").into()),
        });

        let alpha_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Alpha Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_alpha.wgsl").into()),
        });

        let alpha_test_fragment_shader =
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Particle Alpha Test Fragment Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("shaders/particle_alpha_test.wgsl").into(),
                ),
            });

        let multiply_fragment_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Particle Multiply Fragment Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/particle_multiply.wgsl").into()),
        });

        let decal_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Decal Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/decal.wgsl").into()),
        });

        // Create vertex buffer layout
        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ParticleVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                // Size
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 12,
                    shader_location: 1,
                },
                // Color
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 20,
                    shader_location: 2,
                },
                // UV Rect
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 36,
                    shader_location: 3,
                },
                // Rotation
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 52,
                    shader_location: 4,
                },
                // Alpha
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32,
                    offset: 56,
                    shader_location: 5,
                },
            ],
        };

        // Create billboard quad vertices
        #[repr(C)]
        #[derive(Clone, Copy, Pod, Zeroable)]
        struct BillboardVertex {
            position: [f32; 2],
            tex_coord: [f32; 2],
        }

        let billboard_vertices = [
            BillboardVertex {
                position: [-0.5, -0.5],
                tex_coord: [0.0, 1.0],
            },
            BillboardVertex {
                position: [0.5, -0.5],
                tex_coord: [1.0, 1.0],
            },
            BillboardVertex {
                position: [-0.5, 0.5],
                tex_coord: [0.0, 0.0],
            },
            BillboardVertex {
                position: [0.5, 0.5],
                tex_coord: [1.0, 0.0],
            },
        ];

        let billboard_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Billboard Vertex Buffer"),
            contents: bytemuck::cast_slice(&billboard_vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let billboard_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<BillboardVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 6,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 8,
                    shader_location: 7,
                },
            ],
        };

        let decal_vertex_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DecalVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x3,
                    offset: 0,
                    shader_location: 0,
                },
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 12,
                    shader_location: 1,
                },
            ],
        };

        // Create additive blend pipeline
        let additive_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Additive Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &additive_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::SrcAlpha,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false, // Particles don't write depth
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create alpha blend pipeline
        let alpha_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Alpha Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &alpha_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create alpha test pipeline
        let alpha_test_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Alpha Test Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &alpha_test_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: None, // No blending for alpha test
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true, // Alpha test writes depth
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create multiply pipeline
        let multiply_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Particle Multiply Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &vertex_shader,
                entry_point: Some("vs_main"),
                buffers: &[billboard_layout.clone(), vertex_buffer_layout.clone()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &multiply_fragment_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Dst,
                            dst_factor: wgpu::BlendFactor::Src,
                            operation: wgpu::BlendOperation::Subtract,
                        },
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let decal_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Decal Pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &decal_shader,
                entry_point: Some("vs_main"),
                buffers: &[decal_vertex_layout],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &decal_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: surface_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // Create default white texture
        let default_texture = Self::create_default_texture(&device);
        let default_bind_group =
            Self::create_texture_bind_group(&device, &texture_bind_group_layout, &default_texture);

        Ok(Self {
            device,
            queue,

            additive_pipeline,
            alpha_pipeline,
            alpha_test_pipeline,
            multiply_pipeline,
            decal_pipeline,

            uniform_buffer,
            uniform_bind_group,
            texture_bind_group_layout,

            texture_atlas: HashMap::new(),
            texture_bind_groups: HashMap::new(),

            batches: HashMap::new(),

            default_texture,
            default_bind_group,

            billboard_buffer,

            stats: ParticleRenderStats::default(),
        })
    }

    /// Render all particle systems
    pub fn render_particles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        systems: &[&ParticleSystem],
        uniforms: &ParticleUniforms,
    ) {
        let start_time = std::time::Instant::now();
        self.stats.reset_frame_counters();

        // Update uniforms
        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        // Clear batches
        for batch in self.batches.values_mut() {
            batch.clear();
        }

        // Collect particles into batches
        for system in systems {
            self.collect_system_particles(system);
        }

        // Update GPU buffers for batches
        for batch in self.batches.values_mut() {
            batch.update_buffer(&self.device);
        }

        // Render batches
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Particle Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            // Set uniform bind group
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);

            // Set billboard vertices
            render_pass.set_vertex_buffer(0, self.billboard_buffer.slice(..));

            let mut rendered_batches = 0usize;

            // Render each batch
            for batch in self.batches.values() {
                if batch.vertices.is_empty() || batch.vertex_buffer.is_none() {
                    continue;
                }

                // Select pipeline based on shader type
                match batch.shader_type {
                    ParticleShaderType::Additive => {
                        render_pass.set_pipeline(&self.additive_pipeline);
                    }
                    ParticleShaderType::Alpha => {
                        render_pass.set_pipeline(&self.alpha_pipeline);
                    }
                    ParticleShaderType::AlphaTest => {
                        render_pass.set_pipeline(&self.alpha_test_pipeline);
                    }
                    ParticleShaderType::Multiply => {
                        render_pass.set_pipeline(&self.multiply_pipeline);
                    }
                }

                // Set texture bind group
                let texture_bind_group = self
                    .texture_bind_groups
                    .get(&batch.texture_name)
                    .unwrap_or(&self.default_bind_group);
                render_pass.set_bind_group(1, texture_bind_group, &[]);

                // Set particle data
                render_pass.set_vertex_buffer(1, batch.vertex_buffer.as_ref().unwrap().slice(..));

                // Draw instanced
                render_pass.draw(0..4, 0..batch.vertices.len() as u32);

                self.stats.draw_calls += 1;
                rendered_batches += 1;
            }

            self.stats.batches_rendered = rendered_batches;
        }

        self.stats.render_time_ms = start_time.elapsed().as_secs_f64() * 1000.0;
    }

    /// Render weather particles (rain/snow/dust) using the alpha pipeline.
    pub fn render_weather_particles(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        particles: &[WeatherParticle],
        uniforms: &ParticleUniforms,
    ) {
        if particles.is_empty() {
            return;
        }

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        let mut vertices = Vec::with_capacity(particles.len());
        for particle in particles {
            if particle.age >= particle.lifetime || particle.alpha <= 0.0 {
                continue;
            }
            let alpha = (particle.alpha * particle.color[3]).clamp(0.0, 1.0);
            let vertex = ParticleVertex {
                position: [
                    particle.position.x,
                    particle.position.y,
                    particle.position.z,
                ],
                size: [particle.size, particle.size],
                color: [particle.color[0], particle.color[1], particle.color[2], 1.0],
                uv_rect: [0.0, 0.0, 1.0, 1.0],
                rotation: particle.rotation,
                alpha,
                _padding: 0.0,
            };
            vertices.push(vertex);
        }

        if vertices.is_empty() {
            return;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Weather Particle Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let start_time = std::time::Instant::now();
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Weather Particle Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.alpha_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.default_bind_group, &[]);
            render_pass.set_vertex_buffer(0, self.billboard_buffer.slice(..));
            render_pass.set_vertex_buffer(1, vertex_buffer.slice(..));
            render_pass.draw(0..4, 0..vertices.len() as u32);
        }

        self.stats.draw_calls += 1;
        self.stats.particles_rendered += vertices.len();
        self.stats.render_time_ms += start_time.elapsed().as_secs_f64() * 1000.0;
    }

    pub fn render_decals(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        depth_view: &wgpu::TextureView,
        decals: &[DecalRenderItem],
        uniforms: &ParticleUniforms,
    ) {
        if decals.is_empty() {
            return;
        }

        self.queue
            .write_buffer(&self.uniform_buffer, 0, bytemuck::cast_slice(&[*uniforms]));

        let mut vertices: Vec<DecalVertex> = Vec::new();
        for decal in decals {
            let half = decal.size * 0.5;
            let sin_r = decal.rotation.sin();
            let cos_r = decal.rotation.cos();

            let offset = |x: f32, y: f32| {
                let rot_x = x * cos_r - y * sin_r;
                let rot_y = x * sin_r + y * cos_r;
                (rot_x, rot_y)
            };

            let (x0, y0) = offset(-half, -half);
            let (x1, y1) = offset(half, -half);
            let (x2, y2) = offset(-half, half);
            let (x3, y3) = offset(half, half);

            let base = decal.position;
            let z = base.z + 0.01;
            let color = decal.color;

            let v0 = DecalVertex {
                position: [base.x + x0, base.y + y0, z],
                color,
            };
            let v1 = DecalVertex {
                position: [base.x + x1, base.y + y1, z],
                color,
            };
            let v2 = DecalVertex {
                position: [base.x + x2, base.y + y2, z],
                color,
            };
            let v3 = DecalVertex {
                position: [base.x + x3, base.y + y3, z],
                color,
            };

            vertices.extend_from_slice(&[v0, v1, v2, v2, v1, v3]);
        }

        if vertices.is_empty() {
            return;
        }

        let vertex_buffer = self
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Decal Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let start_time = std::time::Instant::now();
        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Decal Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.decal_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            render_pass.set_bind_group(1, &self.default_bind_group, &[]);
            render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            render_pass.draw(0..vertices.len() as u32, 0..1);
        }

        self.stats.draw_calls += 1;
        self.stats.particles_rendered += vertices.len();
        self.stats.render_time_ms += start_time.elapsed().as_secs_f64() * 1000.0;
    }

    /// Collect particles from a system into appropriate batches
    fn collect_system_particles(&mut self, system: &ParticleSystem) {
        let template = system.template();
        let info = template.info();
        let texture_name = if info.particle_type_name.is_empty() {
            "default".to_string()
        } else {
            info.particle_type_name.clone()
        };

        let batch_key = format!("{}_{:?}", texture_name, info.shader_type);

        let batch = self
            .batches
            .entry(batch_key.clone())
            .or_insert_with(|| ParticleBatch::new(info.shader_type, texture_name));

        // Add particles from system to batch
        for particle in system.particles() {
            if particle.lifetime_left > 0 && !particle.is_culled {
                batch.add_particle(particle, system);
                self.stats.particles_rendered += 1;
            }
        }
    }

    /// Load texture for particles
    pub fn load_texture(
        &mut self,
        name: &str,
        texture_data: &[u8],
    ) -> Result<(), Box<dyn std::error::Error>> {
        if texture_data.is_empty() {
            return Err("Texture data is empty".into());
        }

        let image = image::load_from_memory(texture_data)?;
        let rgba = image.to_rgba8();
        let (width, height) = image.dimensions();

        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Particle Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        self.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &rgba,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(4 * width),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let bind_group = Self::create_texture_bind_group(
            &self.device,
            &self.texture_bind_group_layout,
            &texture,
        );

        self.texture_atlas.insert(name.to_string(), texture);
        self.texture_bind_groups
            .insert(name.to_string(), bind_group);
        self.stats.gpu_memory_used = self.stats.gpu_memory_used.saturating_add(
            (width as usize)
                .saturating_mul(height as usize)
                .saturating_mul(4),
        );

        Ok(())
    }

    /// Create default white texture
    fn create_default_texture(device: &wgpu::Device) -> wgpu::Texture {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Default Particle Texture"),
            size: wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload white pixel
        let white_pixel = [255u8; 4];
        device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None })
            .copy_buffer_to_texture(
                wgpu::TexelCopyBufferInfo {
                    buffer: &device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                        label: Some("White Pixel Buffer"),
                        contents: &white_pixel,
                        usage: wgpu::BufferUsages::COPY_SRC,
                    }),
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4),
                        rows_per_image: Some(1),
                    },
                },
                wgpu::TexelCopyTextureInfo {
                    texture: &texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
            );

        texture
    }

    /// Create bind group for texture
    fn create_texture_bind_group(
        device: &wgpu::Device,
        layout: &wgpu::BindGroupLayout,
        texture: &wgpu::Texture,
    ) -> wgpu::BindGroup {
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Particle Texture Sampler"),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Particle Texture Bind Group"),
            layout,
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_particle_vertex_layout() {
        // Test that ParticleVertex is correctly sized and aligned
        assert_eq!(std::mem::size_of::<ParticleVertex>(), 64);
        assert_eq!(std::mem::align_of::<ParticleVertex>(), 4);
        assert_eq!(std::mem::offset_of!(ParticleVertex, position), 0);
        assert_eq!(std::mem::offset_of!(ParticleVertex, size), 12);
        assert_eq!(std::mem::offset_of!(ParticleVertex, color), 20);
        assert_eq!(std::mem::offset_of!(ParticleVertex, uv_rect), 36);
        assert_eq!(std::mem::offset_of!(ParticleVertex, rotation), 52);
        assert_eq!(std::mem::offset_of!(ParticleVertex, alpha), 56);
    }

    #[test]
    fn test_particle_batch() {
        let mut batch = ParticleBatch::new(ParticleShaderType::Alpha, "test.tga".to_string());
        assert_eq!(batch.vertices.len(), 0);
        assert!(batch.dirty);
    }

    #[test]
    fn particle_stats_frame_reset_preserves_gpu_memory() {
        let mut stats = ParticleRenderStats {
            particles_rendered: 17,
            batches_rendered: 3,
            draw_calls: 3,
            gpu_memory_used: 4096,
            render_time_ms: 2.5,
        };

        stats.reset_frame_counters();

        assert_eq!(stats.particles_rendered, 0);
        assert_eq!(stats.batches_rendered, 0);
        assert_eq!(stats.draw_calls, 0);
        assert_eq!(stats.gpu_memory_used, 4096);
        assert_eq!(stats.render_time_ms, 0.0);
    }
}
