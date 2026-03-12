#![allow(unused)]
//! Modern 2025 shader system using WGPU and WGSL
//!
//! This module provides a modern shader abstraction that can run on:
//! - Windows (Vulkan, DX12, DX11)  
//! - macOS (Metal)
//! - Linux (Vulkan)
//! - Web (WebGPU)

use crate::{RenderInfo, ShdError, ShdInterface, ShdResult};
use glam::{Mat4, Vec2, Vec3, Vec4};
use std::collections::HashMap;
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, ComputePipeline, Device, PipelineLayout, Queue, RenderPass,
    RenderPipeline, Sampler, ShaderModule, Texture, TextureView,
};

/// Modern shader system that wraps wgpu
pub struct ModernShaderSystem {
    device: wgpu::Device,
    queue: wgpu::Queue,

    // Shader pipeline cache
    pipelines: HashMap<String, RenderPipeline>,

    // Resource binding cache
    bind_groups: HashMap<String, BindGroup>,
    bind_group_layouts: HashMap<String, BindGroupLayout>,

    // Uniform buffers for common data
    camera_buffer: Buffer,
    light_buffer: Buffer,
    material_buffer: Buffer,
}

/// Modern vertex input structure
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct ModernVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
    tangent: [f32; 3],
    color: [f32; 4],
}

/// Camera uniform data
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_projection: [[f32; 4]; 4],
    view_position: [f32; 3],
    _padding: f32,
}

/// Light uniform data
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LightUniform {
    position: [f32; 3],
    _padding1: f32,
    color: [f32; 3],
    intensity: f32,
    direction: [f32; 3],
    _padding2: f32,
}

/// Material uniform data
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    ambient: [f32; 3],
    _padding1: f32,
    diffuse: [f32; 3],
    _padding2: f32,
    specular: [f32; 3],
    shininess: f32,
}

impl ModernShaderSystem {
    /// Create a new modern shader system
    pub async fn new() -> ShdResult<Self> {
        // Create wgpu instance with all backends
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(), // Vulkan, Metal, DX12, DX11, GL, WebGPU
            ..Default::default()
        });

        // Request adapter (will choose best backend for platform)
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| {
                ShdError::HardwareUnsupported("No suitable GPU adapter found".to_string())
            })?;

        // Get device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("WWShade Modern Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                ..Default::default()
            })
            .await
            .map_err(|e| ShdError::HardwareUnsupported(format!("Failed to get device: {}", e)))?;

        // Create uniform buffers
        let camera_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Camera Buffer"),
            size: std::mem::size_of::<CameraUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Light Buffer"),
            size: std::mem::size_of::<LightUniform>() as u64 * 8, // Support 8 lights
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let material_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Material Buffer"),
            size: std::mem::size_of::<MaterialUniform>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Ok(Self {
            device,
            queue,
            pipelines: HashMap::new(),
            bind_groups: HashMap::new(),
            bind_group_layouts: HashMap::new(),
            camera_buffer,
            light_buffer,
            material_buffer,
        })
    }

    /// Create a modern bump mapping pipeline
    pub fn create_bump_mapping_pipeline(
        &mut self,
        surface_format: wgpu::TextureFormat,
    ) -> ShdResult<String> {
        let pipeline_id = "modern_bump_mapping".to_string();

        // Modern WGSL bump mapping shader
        let shader_source = include_str!("../shaders/modern/bump_mapping.wgsl");
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Bump Mapping Shader"),
                source: wgpu::ShaderSource::Wgsl(shader_source.into()),
            });

        // Create bind group layout for textures and uniforms
        let bind_group_layout =
            self.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    entries: &[
                        // Camera uniforms
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Light uniforms
                        wgpu::BindGroupLayoutEntry {
                            binding: 1,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Material uniforms
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Buffer {
                                ty: wgpu::BufferBindingType::Uniform,
                                has_dynamic_offset: false,
                                min_binding_size: None,
                            },
                            count: None,
                        },
                        // Diffuse texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        // Normal/bump texture
                        wgpu::BindGroupLayoutEntry {
                            binding: 4,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                multisampled: false,
                                view_dimension: wgpu::TextureViewDimension::D2,
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            },
                            count: None,
                        },
                        // Sampler
                        wgpu::BindGroupLayoutEntry {
                            binding: 5,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                    label: Some("Bump Mapping Bind Group Layout"),
                });

        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Bump Mapping Pipeline Layout"),
                bind_group_layouts: &[&bind_group_layout],
                push_constant_ranges: &[],
            });

        // Create render pipeline
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Bump Mapping Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[ModernVertex::desc()],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: surface_format,
                        blend: Some(wgpu::BlendState::REPLACE),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: Some(wgpu::Face::Back),
                    polygon_mode: wgpu::PolygonMode::Fill,
                    unclipped_depth: false,
                    conservative: false,
                },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: wgpu::TextureFormat::Depth32Float,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::Less,
                    stencil: wgpu::StencilState::default(),
                    bias: wgpu::DepthBiasState::default(),
                }),
                multisample: wgpu::MultisampleState::default(),
                cache: None,
                multiview: None,
            });

        self.pipelines.insert(pipeline_id.clone(), pipeline);
        self.bind_group_layouts
            .insert("bump_mapping".to_string(), bind_group_layout);

        Ok(pipeline_id)
    }

    /// Render using modern pipeline
    pub fn render_modern<'a>(
        &'a mut self,
        render_pass: &mut RenderPass<'a>,
        pipeline_id: &str,
    ) -> ShdResult<()> {
        let pipeline = self.pipelines.get(pipeline_id).ok_or_else(|| {
            ShdError::ResourceNotFound(format!("Pipeline not found: {}", pipeline_id))
        })?;

        render_pass.set_pipeline(pipeline);

        // Set bind groups with uniforms and textures
        if let Some(bind_group) = self.bind_groups.get("current_material") {
            render_pass.set_bind_group(0, bind_group, &[]);
        }

        Ok(())
    }

    /// Update camera uniform buffer
    pub fn update_camera(&self, view_projection: Mat4, view_position: Vec3) {
        let camera_uniform = CameraUniform {
            view_projection: view_projection.to_cols_array_2d(),
            view_position: view_position.into(),
            _padding: 0.0,
        };

        self.queue.write_buffer(
            &self.camera_buffer,
            0,
            bytemuck::cast_slice(&[camera_uniform]),
        );
    }

    /// Update light uniform buffer
    pub fn update_lights(&self, lights: &[LightUniform]) {
        self.queue
            .write_buffer(&self.light_buffer, 0, bytemuck::cast_slice(lights));
    }

    /// Get backend info for debugging
    pub fn get_backend_info(&self) -> String {
        format!("Modern WGPU Backend: {:?}", self.device.features())
    }
}

impl ModernVertex {
    /// Vertex buffer layout descriptor
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<ModernVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Normal
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // UV
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x2,
                },
                // Tangent
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: wgpu::VertexFormat::Float32x3,
                },
                // Color
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 11]>() as wgpu::BufferAddress,
                    shader_location: 4,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Hybrid shader system that supports both legacy and modern approaches
pub struct HybridShaderSystem {
    // Legacy DirectX support (your existing code)
    pub legacy_dx6: bool,
    pub legacy_dx7: bool,
    pub legacy_dx8: bool,

    // Modern wgpu support
    pub modern_system: Option<ModernShaderSystem>,
}

impl HybridShaderSystem {
    /// Create hybrid system with runtime backend detection
    pub async fn new() -> ShdResult<Self> {
        // Try to initialize modern system first
        let modern_system = match ModernShaderSystem::new().await {
            Ok(system) => {
                log::info!("Modern WGPU backend initialized successfully");
                Some(system)
            }
            Err(e) => {
                log::warn!("Modern WGPU backend failed, falling back to legacy: {}", e);
                None
            }
        };

        Ok(Self {
            legacy_dx6: true, // Keep legacy support
            legacy_dx7: true,
            legacy_dx8: true,
            modern_system,
        })
    }

    /// Check if modern backend is available
    pub fn has_modern_support(&self) -> bool {
        self.modern_system.is_some()
    }

    /// Get available backends as string
    pub fn get_backend_info(&self) -> String {
        let mut info = Vec::new();

        if let Some(modern) = &self.modern_system {
            info.push(modern.get_backend_info());
        }

        if self.legacy_dx8 {
            info.push("Legacy DirectX 8".to_string());
        }
        if self.legacy_dx7 {
            info.push("Legacy DirectX 7".to_string());
        }
        if self.legacy_dx6 {
            info.push("Legacy DirectX 6".to_string());
        }

        info.join(", ")
    }
}
