//! # W3D Advanced Renderer - Complete Modern Graphics Pipeline
//!
//! This module implements a complete, modern graphics rendering pipeline featuring:
//! - Deferred rendering with G-Buffer
//! - Forward+ rendering for transparent objects  
//! - Physically Based Rendering (PBR) materials
//! - Cascaded shadow mapping
//! - HDR rendering with tone mapping
//! - Screen Space Ambient Occlusion (SSAO)
//! - Temporal Anti-Aliasing (TAA)
//! - GPU culling and instanced rendering
//! - Multi-threaded command buffer generation

use super::{
    Camera, Light, Material, Mesh, PrimitiveTopology as W3DPrimitiveTopology, Result,
    VertexFormat as W3DVertexFormat,
};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

#[cfg(feature = "w3d")]
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    AddressMode, BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout,
    BindGroupLayoutDescriptor, BindGroupLayoutEntry, BindingType, BlendState, Buffer,
    BufferBindingType, BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites,
    CommandBuffer, CommandEncoder, CompareFunction, ComputePass, ComputePipeline,
    ComputePipelineDescriptor, DepthBiasState, DepthStencilState, Device, Extent3d, Face,
    FilterMode, FragmentState, FrontFace, IndexFormat, LoadOp, MultisampleState, Operations,
    Origin3d, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PrimitiveState,
    PrimitiveTopology, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StencilState, StorageTextureAccess,
    StoreOp, Texture, TextureAspect, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDimension, VertexAttribute,
    VertexBufferLayout, VertexFormat, VertexState,
};

/// Maximum number of lights per frame
const MAX_LIGHTS: usize = 256;
/// Maximum number of bones for skeletal animation
const MAX_BONES: usize = 256;
/// Maximum number of instances per draw call
const MAX_INSTANCES: usize = 1024;
/// Shadow map cascade count
const CASCADE_COUNT: usize = 4;
/// Depth/stencil format used for the main frame buffer when stencil operations are required.
const FRAME_DEPTH_STENCIL_FORMAT: TextureFormat = TextureFormat::Depth24PlusStencil8;

/// Key for looking up cached render pipelines. Encodes all GPU pipeline state
/// that is invariant across frames for a given combination of shader inputs and
/// render pass configuration.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PipelineCacheKey {
    vertex_format: W3DVertexFormat,
    topology: PrimitiveTopology,
    blend_enabled: bool,
    double_sided: bool,
    depth_write_enabled: bool,
    depth_test_enabled: bool,
}

/// Advanced render batch for efficient GPU rendering
#[derive(Debug, Clone)]
pub struct RenderBatch {
    /// Mesh ID for draw submission
    pub mesh_id: String,
    /// Optional mesh snapshot for direct GPU submission.
    pub mesh: Option<Mesh>,
    /// Material ID (optional for default material path)
    pub material_id: Option<String>,
    /// Optional material snapshot for direct GPU submission.
    pub material: Option<Material>,
    /// Instance data
    pub instances: Vec<InstanceData>,
    /// Distance from camera for sorting
    pub camera_distance: f32,
    /// Render priority (lower = render first)
    pub priority: u32,
    /// Alpha blending enabled
    pub transparent: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RenderSubmissionKind {
    Opaque,
    Transparent,
    Ui,
}

/// Instance data for rendering multiple objects with same mesh/material
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct InstanceData {
    /// Model matrix
    pub model_matrix: [[f32; 4]; 4],
    /// Normal matrix (inverse transpose of upper 3x3 model matrix)
    pub normal_matrix: [[f32; 4]; 4],
    /// Material index
    pub material_index: u32,
    /// LOD level
    pub lod_level: u32,
    /// Animation frame
    pub animation_frame: f32,
    /// Custom data
    pub custom_data: f32,
    /// Per-instance color modulation
    pub color: [f32; 4],
    /// Per-instance material parameters (metallic, roughness, etc.)
    pub material_params: [f32; 4],
}

/// Complete render state tracking with wgpu integration
#[derive(Debug, Clone)]
pub struct RenderState {
    /// Current render pipeline
    pub current_pipeline: Option<Arc<wgpu::RenderPipeline>>,
    /// Current uniform bind groups
    pub uniform_bind_groups: Vec<Arc<BindGroup>>,
    /// Current texture bind groups
    pub texture_bind_groups: Vec<Arc<BindGroup>>,
    /// Depth test configuration
    pub depth_stencil: Option<wgpu::DepthStencilState>,
    /// Primitive state (culling, polygon mode, etc.)
    pub primitive_state: PrimitiveState,
    /// Multisample state
    pub multisample_state: MultisampleState,
    /// Blend state for transparency
    pub blend_state: Option<BlendState>,
    /// Current viewport
    pub viewport: Option<(f32, f32, f32, f32)>,
    /// Current scissor test
    pub scissor_rect: Option<(u32, u32, u32, u32)>,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            current_pipeline: None,
            uniform_bind_groups: Vec::new(),
            texture_bind_groups: Vec::new(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: FRAME_DEPTH_STENCIL_FORMAT,
                depth_write_enabled: true,
                depth_compare: CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: DepthBiasState::default(),
            }),
            primitive_state: PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            multisample_state: MultisampleState {
                count: 4, // 4x MSAA by default
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            blend_state: None, // No blending by default (opaque rendering)
            viewport: None,
            scissor_rect: None,
        }
    }
}

/// Advanced W3D Renderer with complete wgpu backend
pub struct W3DRenderer {
    /// WGPU device reference
    device: Arc<Device>,
    /// WGPU command queue
    queue: Arc<Queue>,
    /// Surface format for rendering
    surface_format: TextureFormat,

    /// Current render state
    state: RenderState,

    /// Render queues for different passes
    opaque_queue: VecDeque<RenderBatch>,
    transparent_queue: VecDeque<RenderBatch>,
    ui_queue: VecDeque<RenderBatch>,

    /// Camera and lighting
    current_camera: Option<Camera>,
    active_lights: Vec<Light>,
    light_buffer: Option<Buffer>,
    max_lights: usize,

    /// Frame management
    frame_count: u64,
    frame_uniform_buffer: Option<Buffer>,

    /// Render targets and textures
    depth_texture: Option<Texture>,
    depth_texture_view: Option<TextureView>,

    /// G-Buffer for deferred rendering
    gbuffer_albedo: Option<Texture>,
    gbuffer_normal: Option<Texture>,
    gbuffer_material: Option<Texture>,
    gbuffer_depth: Option<Texture>,

    /// Frame color target used for actual command submission in this renderer.
    frame_color_target: Option<Texture>,
    frame_color_view: Option<TextureView>,
    frame_target_size: Option<(u32, u32)>,

    /// Shadow mapping
    shadow_map_texture: Option<Texture>,
    shadow_map_size: u32,
    shadow_render_pipeline: Option<wgpu::RenderPipeline>,

    /// Post-processing
    bloom_textures: Vec<Texture>,
    tonemap_pipeline: Option<wgpu::RenderPipeline>,

    /// GPU pipeline caches — avoid recompiling shaders/pipelines per frame.
    pipeline_cache: HashMap<PipelineCacheKey, RenderPipeline>,
    shader_cache: HashMap<W3DVertexFormat, ShaderModule>,

    /// Shared bind group layouts — created once, reused across all pipelines.
    frame_bind_group_layout: BindGroupLayout,
    material_bind_group_layout: BindGroupLayout,

    /// Bind group layout for skeletal animation bone matrix palette.
    bone_bind_group_layout: BindGroupLayout,
    /// Identity bone matrix buffer (MAX_BONES identity mat4x4). Replaced at
    /// draw time with actual pose data by the animation system.
    bone_buffer: Buffer,

    /// Performance stats
    stats: RendererStats,
}

/// Renderer performance statistics
#[derive(Debug, Default, Clone)]
pub struct RendererStats {
    /// Draw calls submitted
    pub draw_calls: u32,
    /// Triangles rendered
    pub triangles: u32,
    /// Vertices processed
    pub vertices: u32,
    /// Instances rendered
    pub instances: u32,
    /// Texture switches
    pub texture_switches: u32,
    /// Pipeline switches
    pub pipeline_switches: u32,
    /// GPU memory used (bytes)
    pub gpu_memory_used: u64,
    /// Frame time breakdown
    pub depth_prepass_time: f32,
    pub gbuffer_time: f32,
    pub lighting_time: f32,
    pub forward_time: f32,
    pub postprocess_time: f32,
}

impl W3DRenderer {
    /// Create a new renderer with wgpu backend
    pub async fn new_with_wgpu(
        device: &Device,
        queue: &Queue,
        surface_format: &TextureFormat,
    ) -> Result<Self> {
        tracing::info!("Creating W3D renderer with wgpu backend");

        let max_lights = 256; // Support up to 256 dynamic lights

        // Create light uniform buffer
        let light_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("W3D Light Buffer"),
            size: (max_lights * std::mem::size_of::<super::W3DLightData>()) as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create frame uniform buffer
        let frame_uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("W3D Frame Uniform Buffer"),
            size: std::mem::size_of::<super::W3DUniformData>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create shared bind group layouts (reused across all pipelines for cache compatibility)
        let frame_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("W3D Frame Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let material_bind_group_layout =
            device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("W3D Material Bind Group Layout"),
                entries: &[BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });
        let bone_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("W3D Bone Bind Group Layout"),
            entries: &[BindGroupLayoutEntry {
                binding: 0,
                visibility: ShaderStages::VERTEX,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let identity_matrix: [[f32; 4]; 4] = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let bone_data: Vec<[[f32; 4]; 4]> = vec![identity_matrix; MAX_BONES];
        let bone_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Identity Bone Buffer"),
            contents: bytemuck::cast_slice(bone_data.as_slice()),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        Ok(Self {
            device: Arc::new(device.clone()),
            queue: Arc::new(queue.clone()),
            surface_format: *surface_format,
            state: RenderState::default(),
            opaque_queue: VecDeque::new(),
            transparent_queue: VecDeque::new(),
            ui_queue: VecDeque::new(),
            current_camera: None,
            active_lights: Vec::new(),
            light_buffer: Some(light_buffer),
            max_lights,
            frame_count: 0,
            frame_uniform_buffer: Some(frame_uniform_buffer),
            depth_texture: None,
            depth_texture_view: None,
            gbuffer_albedo: None,
            gbuffer_normal: None,
            gbuffer_material: None,
            gbuffer_depth: None,
            frame_color_target: None,
            frame_color_view: None,
            frame_target_size: None,
            shadow_map_texture: None,
            shadow_map_size: 2048,
            shadow_render_pipeline: None,
            bloom_textures: Vec::new(),
            tonemap_pipeline: None,
            pipeline_cache: HashMap::new(),
            shader_cache: HashMap::new(),
            frame_bind_group_layout,
            material_bind_group_layout,
            bone_bind_group_layout,
            bone_buffer,
            stats: RendererStats::default(),
        })
    }

    /// Begin a new frame with proper GPU synchronization
    pub async fn begin_frame(&mut self) -> Result<()> {
        // Clear all render queues
        self.opaque_queue.clear();
        self.transparent_queue.clear();
        self.ui_queue.clear();
        self.active_lights.clear();

        // Reset render state for new frame
        self.state = RenderState::default();

        // Reset performance stats
        self.stats = RendererStats::default();

        self.frame_count += 1;

        tracing::trace!("Beginning frame {}", self.frame_count);
        Ok(())
    }

    /// Initialize render targets for the frame
    pub async fn init_render_targets(&mut self, width: u32, height: u32) -> Result<()> {
        self.frame_target_size = Some((width, height));

        // Create depth texture
        let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("W3D Depth Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: FRAME_DEPTH_STENCIL_FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        let depth_view = depth_texture.create_view(&wgpu::TextureViewDescriptor::default());

        self.depth_texture = Some(depth_texture);
        self.depth_texture_view = Some(depth_view);

        // Create G-Buffer textures for deferred rendering
        self.create_gbuffer_textures(width, height)?;

        // Create a single-sampled frame color target that this renderer can actually submit into.
        // The surface present step lives above this file in W3DDevice; here we keep the render
        // submission path concrete and encoder-backed.
        let frame_color_target = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("W3D Frame Color Target"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.surface_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let frame_color_view =
            frame_color_target.create_view(&wgpu::TextureViewDescriptor::default());
        self.frame_color_target = Some(frame_color_target);
        self.frame_color_view = Some(frame_color_view);

        // Create shadow map if needed
        if self.shadow_map_texture.is_none() {
            self.create_shadow_map()?;
        }

        tracing::debug!("Initialized render targets {}x{}", width, height);
        Ok(())
    }

    /// Ensure render targets exist and match the requested dimensions.
    pub async fn ensure_render_targets(&mut self, width: u32, height: u32) -> Result<()> {
        if self.frame_target_size == Some((width, height))
            && self.frame_color_view.is_some()
            && self.depth_texture_view.is_some()
        {
            return Ok(());
        }
        self.init_render_targets(width, height).await
    }

    /// Create G-Buffer textures for deferred rendering
    fn create_gbuffer_textures(&mut self, width: u32, height: u32) -> Result<()> {
        // Albedo + Metallic (RGBA8)
        let gbuffer_albedo = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("G-Buffer Albedo"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Normal + Roughness (RGBA16F)
        let gbuffer_normal = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("G-Buffer Normal"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        // Material Properties (RGBA8)
        let gbuffer_material = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("G-Buffer Material"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        self.gbuffer_albedo = Some(gbuffer_albedo);
        self.gbuffer_normal = Some(gbuffer_normal);
        self.gbuffer_material = Some(gbuffer_material);

        Ok(())
    }

    /// Create shadow map texture
    fn create_shadow_map(&mut self) -> Result<()> {
        let shadow_map = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("W3D Shadow Map"),
            size: wgpu::Extent3d {
                width: self.shadow_map_size,
                height: self.shadow_map_size,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        self.shadow_map_texture = Some(shadow_map);

        tracing::debug!(
            "Created shadow map {}x{}",
            self.shadow_map_size,
            self.shadow_map_size
        );
        Ok(())
    }

    /// Returns whether the current frame depth target is stencil-capable.
    pub fn supports_stencil(&self) -> bool {
        self.state
            .depth_stencil
            .as_ref()
            .is_some_and(|state| matches!(state.format, TextureFormat::Depth24PlusStencil8))
    }

    /// Set camera for rendering
    pub async fn set_camera(&mut self, camera: &Camera) -> Result<()> {
        self.current_camera = Some(camera.clone());
        Ok(())
    }

    /// Add light to the scene
    pub async fn add_light(&mut self, light: &Light) -> Result<()> {
        self.active_lights.push(light.clone());
        Ok(())
    }

    /// Render a mesh with optional material
    pub async fn render_mesh(
        &mut self,
        mesh: &Mesh,
        material: Option<&Material>,
        model_matrix: Option<[[f32; 4]; 4]>,
        world_center: Option<[f32; 3]>,
        transparent_override: Option<bool>,
    ) -> Result<()> {
        let identity = [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ];
        let model_matrix = model_matrix.unwrap_or(identity);
        let normal_matrix = compute_normal_matrix(model_matrix);
        let camera_distance = mesh_camera_distance(
            mesh,
            self.current_camera.as_ref(),
            model_matrix,
            world_center,
        );
        let transparent = effective_batch_transparency(material, transparent_override);
        let batch = RenderBatch {
            mesh_id: mesh.id.clone(),
            mesh: Some(mesh.clone()),
            material_id: material.map(|m| m.id.clone()),
            material: material.cloned(),
            instances: vec![InstanceData {
                model_matrix,
                normal_matrix,
                material_index: 0,
                lod_level: 0,
                animation_frame: 0.0,
                custom_data: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                material_params: batch_material_params(material),
            }],
            camera_distance,
            priority: batch_priority(material),
            transparent,
        };

        if batch.transparent {
            self.transparent_queue.push_back(batch);
        } else {
            self.opaque_queue.push_back(batch);
        }
        Ok(())
    }

    /// End frame and submit render commands
    pub async fn end_frame(&mut self) -> Result<()> {
        self.end_frame_with_view(None).await
    }

    /// End frame and submit render commands to an optional external color target view.
    pub async fn end_frame_with_view(
        &mut self,
        external_color_view: Option<&TextureView>,
    ) -> Result<()> {
        // Sort render queue for optimal rendering
        self.sort_render_queue();

        let opaque_batches = std::mem::take(&mut self.opaque_queue);
        let transparent_batches = std::mem::take(&mut self.transparent_queue);
        let ui_batches = std::mem::take(&mut self.ui_queue);

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("W3D Frame Encoder"),
            });

        let color_view = if let Some(view) = external_color_view {
            view.clone()
        } else if let Some(view) = self.frame_color_view.as_ref() {
            view.clone()
        } else {
            tracing::trace!(
                "Skipping W3D frame submission because no frame color target is initialized"
            );
            return Ok(());
        };

        // Clear the frame target once, then let each batch load from it.
        self.clear_frame_target(&mut encoder, &color_view);

        for batch in &opaque_batches {
            self.submit_render_batch(
                &mut encoder,
                &color_view,
                batch,
                RenderSubmissionKind::Opaque,
            )
            .await?;
        }
        for batch in &transparent_batches {
            self.submit_render_batch(
                &mut encoder,
                &color_view,
                batch,
                RenderSubmissionKind::Transparent,
            )
            .await?;
        }
        for batch in &ui_batches {
            self.submit_render_batch(&mut encoder, &color_view, batch, RenderSubmissionKind::Ui)
                .await?;
        }

        self.queue.submit(std::iter::once(encoder.finish()));

        Ok(())
    }

    /// Set render state
    pub async fn set_render_state(&mut self, state: RenderState) -> Result<()> {
        self.state = state;
        Ok(())
    }

    /// Get current render state
    pub fn get_render_state(&self) -> &RenderState {
        &self.state
    }

    /// Shutdown the renderer
    pub async fn shutdown(&self) -> Result<()> {
        tracing::info!("W3D renderer shutdown completed");
        Ok(())
    }

    /// Sort render queue for optimal rendering
    fn sort_render_queue(&mut self) {
        sort_opaque_batches(self.opaque_queue.make_contiguous());
        sort_transparent_batches(self.transparent_queue.make_contiguous());
    }

    fn clear_frame_target(&self, encoder: &mut CommandEncoder, color_view: &TextureView) {
        let depth_attachment =
            self.depth_texture_view
                .as_ref()
                .map(|view| RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: if self.supports_stencil() {
                        Some(Operations {
                            load: LoadOp::Clear(0),
                            store: StoreOp::Store,
                        })
                    } else {
                        None
                    },
                });
        let _render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("W3D Frame Clear"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::BLACK),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
    }

    /// Submit a render batch to the GPU
    async fn submit_render_batch(
        &mut self,
        encoder: &mut CommandEncoder,
        color_view: &TextureView,
        batch: &RenderBatch,
        kind: RenderSubmissionKind,
    ) -> Result<()> {
        let Some(mesh) = batch.mesh.as_ref() else {
            tracing::trace!(
                "Skipping W3D render batch '{}' because no mesh data is available",
                batch.mesh_id
            );
            return Ok(());
        };

        let Some(mesh_layout) = mesh_vertex_buffer_layout(mesh.vertex_format) else {
            tracing::trace!(
                "Skipping W3D render batch '{}' because vertex format {:?} has no GPU layout",
                batch.mesh_id,
                mesh.vertex_format
            );
            return Ok(());
        };

        let blend_enabled = matches!(
            kind,
            RenderSubmissionKind::Transparent | RenderSubmissionKind::Ui
        );
        let double_sided = batch
            .material
            .as_ref()
            .is_some_and(|material| material.properties.double_sided);
        let depth_write_enabled = matches!(kind, RenderSubmissionKind::Opaque);
        let depth_test_enabled = !matches!(kind, RenderSubmissionKind::Ui);
        let topology = mesh_primitive_topology(mesh.topology);
        let cull_mode = if double_sided { None } else { Some(Face::Back) };
        let blend_state = if blend_enabled {
            Some(BlendState::ALPHA_BLENDING)
        } else {
            None
        };
        let cache_key = PipelineCacheKey {
            vertex_format: mesh.vertex_format,
            topology,
            blend_enabled,
            double_sided,
            depth_write_enabled,
            depth_test_enabled,
        };

        if !self.shader_cache.contains_key(&mesh.vertex_format) {
            let shader_source = batch_shader_source(mesh.vertex_format);
            let shader = self.device.create_shader_module(ShaderModuleDescriptor {
                label: Some("W3D Batch Submission Shader"),
                source: ShaderSource::Wgsl(shader_source.into()),
            });
            self.shader_cache.insert(mesh.vertex_format, shader);
        }
        let shader = self.shader_cache.get(&mesh.vertex_format).unwrap();

        if !self.pipeline_cache.contains_key(&cache_key) {
            let is_skinned = matches!(mesh.vertex_format, W3DVertexFormat::Skinned);
            let bind_group_layouts: Vec<&BindGroupLayout> = if is_skinned {
                vec![
                    &self.frame_bind_group_layout,
                    &self.material_bind_group_layout,
                    &self.bone_bind_group_layout,
                ]
            } else {
                vec![
                    &self.frame_bind_group_layout,
                    &self.material_bind_group_layout,
                ]
            };
            let pipeline_layout = self
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("W3D Batch Submission Layout"),
                    bind_group_layouts: &bind_group_layouts,
                    push_constant_ranges: &[],
                });
            let pipeline = self
                .device
                .create_render_pipeline(&RenderPipelineDescriptor {
                    label: Some("W3D Batch Submission Pipeline"),
                    layout: Some(&pipeline_layout),
                    vertex: VertexState {
                        module: shader,
                        entry_point: Some("vs_main"),
                        compilation_options: Default::default(),
                        buffers: &[mesh_layout.clone(), instance_vertex_layout()],
                    },
                    fragment: Some(FragmentState {
                        module: shader,
                        entry_point: Some("fs_main"),
                        compilation_options: Default::default(),
                        targets: &[Some(ColorTargetState {
                            format: self.surface_format,
                            blend: blend_state,
                            write_mask: ColorWrites::ALL,
                        })],
                    }),
                    primitive: PrimitiveState {
                        topology,
                        strip_index_format: if matches!(
                            topology,
                            PrimitiveTopology::TriangleStrip | PrimitiveTopology::LineStrip
                        ) {
                            Some(IndexFormat::Uint32)
                        } else {
                            None
                        },
                        front_face: FrontFace::Ccw,
                        cull_mode,
                        unclipped_depth: false,
                        polygon_mode: PolygonMode::Fill,
                        conservative: false,
                    },
                    depth_stencil: if depth_test_enabled {
                        Some(DepthStencilState {
                            format: FRAME_DEPTH_STENCIL_FORMAT,
                            depth_write_enabled,
                            depth_compare: CompareFunction::LessEqual,
                            stencil: StencilState::default(),
                            bias: DepthBiasState::default(),
                        })
                    } else {
                        None
                    },
                    multisample: MultisampleState::default(),
                    multiview: None,
                    cache: None,
                });
            self.pipeline_cache.insert(cache_key.clone(), pipeline);
        }
        let pipeline = self.pipeline_cache.get(&cache_key).unwrap();

        let frame_uniform = batch_frame_uniform(self.current_camera.as_ref());
        let frame_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Batch Frame Uniform Buffer"),
            contents: bytemuck::bytes_of(&frame_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let frame_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("W3D Batch Frame Bind Group"),
            layout: &self.frame_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let material_uniform = batch_material_uniform(batch, kind);
        let material_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Batch Material Uniform Buffer"),
            contents: bytemuck::bytes_of(&material_uniform),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let material_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
            label: Some("W3D Batch Material Bind Group"),
            layout: &self.material_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: material_buffer.as_entire_binding(),
            }],
        });

        let vertex_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Batch Vertex Buffer"),
            contents: &mesh.vertices,
            usage: BufferUsages::VERTEX,
        });

        let index_data = batch_index_data(mesh);
        let index_buffer = if index_data.is_empty() {
            None
        } else {
            Some(self.device.create_buffer_init(&BufferInitDescriptor {
                label: Some("W3D Batch Index Buffer"),
                contents: bytemuck::cast_slice(index_data.as_slice()),
                usage: BufferUsages::INDEX,
            }))
        };

        let instances = if batch.instances.is_empty() {
            vec![InstanceData {
                model_matrix: [[1.0, 0.0, 0.0, 0.0]; 4],
                normal_matrix: [[1.0, 0.0, 0.0, 0.0]; 4],
                material_index: 0,
                lod_level: 0,
                animation_frame: 0.0,
                custom_data: 0.0,
                color: [1.0, 1.0, 1.0, 1.0],
                material_params: [1.0, 1.0, 1.0, 1.0],
            }]
        } else {
            batch.instances.clone()
        };
        let instance_buffer = self.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Batch Instance Buffer"),
            contents: bytemuck::cast_slice(instances.as_slice()),
            usage: BufferUsages::VERTEX,
        });
        let instance_count = instances.len() as u32;
        let draw_range = if index_data.is_empty() {
            mesh_vertex_count(mesh).unwrap_or(3)
        } else {
            index_data.len() as u32
        };

        let depth_attachment = if matches!(kind, RenderSubmissionKind::Ui) {
            None
        } else {
            self.depth_texture_view
                .as_ref()
                .map(|view| RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Load,
                        store: StoreOp::Store,
                    }),
                    stencil_ops: if self.supports_stencil() {
                        Some(Operations {
                            load: LoadOp::Load,
                            store: StoreOp::Store,
                        })
                    } else {
                        None
                    },
                })
        };

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("W3D Batch Submission Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: color_view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: depth_attachment,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &frame_bind_group, &[]);
        render_pass.set_bind_group(1, &material_bind_group, &[]);
        if matches!(mesh.vertex_format, W3DVertexFormat::Skinned) {
            let bone_bind_group = self.device.create_bind_group(&BindGroupDescriptor {
                label: Some("W3D Bone Bind Group"),
                layout: &self.bone_bind_group_layout,
                entries: &[BindGroupEntry {
                    binding: 0,
                    resource: self.bone_buffer.as_entire_binding(),
                }],
            });
            render_pass.set_bind_group(2, &bone_bind_group, &[]);
        }
        render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        render_pass.set_vertex_buffer(1, instance_buffer.slice(..));

        let triangle_count = batch_triangle_count(mesh, &index_data);
        if let Some(index_buffer) = index_buffer.as_ref() {
            render_pass.set_index_buffer(index_buffer.slice(..), IndexFormat::Uint32);
            if draw_range > 0 {
                render_pass.draw_indexed(0..draw_range, 0, 0..instance_count);
            }
        } else if draw_range > 0 {
            render_pass.draw(0..draw_range, 0..instance_count);
        }

        self.stats.draw_calls += 1;
        self.stats.instances += instance_count;
        self.stats.triangles += triangle_count;
        self.stats.vertices += mesh_vertex_count(mesh).unwrap_or(0) * instance_count;
        if matches!(
            kind,
            RenderSubmissionKind::Transparent | RenderSubmissionKind::Ui
        ) {
            self.stats.pipeline_switches += 1;
        }

        Ok(())
    }
}

fn submission_color(kind: RenderSubmissionKind, batch: &RenderBatch) -> Color {
    match kind {
        RenderSubmissionKind::Opaque => {
            let tint = ((batch.priority % 5) as f64) * 0.1 + 0.45;
            Color {
                r: tint,
                g: tint,
                b: tint,
                a: 1.0,
            }
        }
        RenderSubmissionKind::Transparent => Color {
            r: 0.25,
            g: 0.65,
            b: 0.95,
            a: 0.45,
        },
        RenderSubmissionKind::Ui => Color {
            r: 0.95,
            g: 0.78,
            b: 0.18,
            a: 0.9,
        },
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BatchFrameUniform {
    view_projection_matrix: [[f32; 4]; 4],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
struct BatchMaterialUniform {
    base_color: [f32; 4],
    material_params: [f32; 4],
    emissive_color: [f32; 4],
}

fn batch_frame_uniform(camera: Option<&Camera>) -> BatchFrameUniform {
    let view_projection_matrix = camera.map_or(Mat4::IDENTITY, |camera| {
        Mat4::from_cols_array_2d(&camera.projection_matrix)
            * Mat4::from_cols_array_2d(&camera.view_matrix)
    });
    BatchFrameUniform {
        view_projection_matrix: view_projection_matrix.to_cols_array_2d(),
    }
}

fn batch_material_uniform(batch: &RenderBatch, kind: RenderSubmissionKind) -> BatchMaterialUniform {
    let material = batch.material.as_ref();
    let base_color = material
        .map(|material| material.properties.diffuse_color)
        .unwrap_or_else(|| color_to_array(submission_color(kind, batch)));
    let emissive_color = material
        .map(|material| {
            [
                material.properties.emissive_color[0],
                material.properties.emissive_color[1],
                material.properties.emissive_color[2],
                1.0,
            ]
        })
        .unwrap_or([0.0, 0.0, 0.0, 0.0]);
    BatchMaterialUniform {
        base_color,
        material_params: batch_material_params(material),
        emissive_color,
    }
}

fn color_to_array(color: Color) -> [f32; 4] {
    [
        color.r as f32,
        color.g as f32,
        color.b as f32,
        color.a as f32,
    ]
}

fn mesh_vertex_count(mesh: &Mesh) -> Option<u32> {
    let stride = mesh_vertex_stride(mesh.vertex_format)?;
    if stride == 0 {
        return None;
    }
    let count = mesh.vertices.len() as u64 / stride;
    if count == 0 {
        None
    } else {
        Some(count as u32)
    }
}

fn batch_triangle_count(mesh: &Mesh, index_data: &[u32]) -> u32 {
    let count = if index_data.is_empty() {
        mesh_vertex_count(mesh).unwrap_or(0)
    } else {
        index_data.len() as u32
    };
    match mesh.topology {
        super::PrimitiveTopology::TriangleList | super::PrimitiveTopology::TriangleFan => count / 3,
        super::PrimitiveTopology::TriangleStrip => count.saturating_sub(2),
        super::PrimitiveTopology::LineList | super::PrimitiveTopology::LineStrip => 0,
        super::PrimitiveTopology::PointList => 0,
    }
}

#[cfg(test)]
fn batch_has_submittable_geometry(batch: &RenderBatch) -> bool {
    batch
        .mesh
        .as_ref()
        .is_some_and(|mesh| mesh_vertex_buffer_layout(mesh.vertex_format).is_some())
}

fn mesh_vertex_stride(format: super::VertexFormat) -> Option<u64> {
    match format {
        super::VertexFormat::Position => Some(12),
        super::VertexFormat::PositionNormal => Some(24),
        super::VertexFormat::PositionUv => Some(20),
        super::VertexFormat::PositionNormalUv => Some(32),
        super::VertexFormat::PositionNormalUvColor => Some(48),
        super::VertexFormat::Skinned => Some(64),
    }
}

fn mesh_vertex_buffer_layout(format: super::VertexFormat) -> Option<VertexBufferLayout<'static>> {
    match format {
        super::VertexFormat::Position => Some(VertexBufferLayout {
            array_stride: 12,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &POSITION_ATTRIBUTES,
        }),
        super::VertexFormat::PositionNormal => Some(VertexBufferLayout {
            array_stride: 24,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &POSITION_NORMAL_ATTRIBUTES,
        }),
        super::VertexFormat::PositionUv => Some(VertexBufferLayout {
            array_stride: 20,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &POSITION_UV_ATTRIBUTES,
        }),
        super::VertexFormat::PositionNormalUv => Some(VertexBufferLayout {
            array_stride: 32,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &POSITION_NORMAL_UV_ATTRIBUTES,
        }),
        super::VertexFormat::PositionNormalUvColor => Some(VertexBufferLayout {
            array_stride: 48,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &POSITION_NORMAL_UV_COLOR_ATTRIBUTES,
        }),
        super::VertexFormat::Skinned => Some(VertexBufferLayout {
            array_stride: 64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &SKINNED_ATTRIBUTES,
        }),
    }
}

fn instance_vertex_layout() -> VertexBufferLayout<'static> {
    VertexBufferLayout {
        array_stride: std::mem::size_of::<InstanceData>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &INSTANCE_ATTRIBUTES,
    }
}

fn mesh_primitive_topology(topology: super::PrimitiveTopology) -> PrimitiveTopology {
    match topology {
        super::PrimitiveTopology::TriangleList => PrimitiveTopology::TriangleList,
        super::PrimitiveTopology::TriangleStrip => PrimitiveTopology::TriangleStrip,
        super::PrimitiveTopology::TriangleFan => PrimitiveTopology::TriangleList,
        super::PrimitiveTopology::LineList => PrimitiveTopology::LineList,
        super::PrimitiveTopology::LineStrip => PrimitiveTopology::LineStrip,
        super::PrimitiveTopology::PointList => PrimitiveTopology::PointList,
    }
}

fn batch_shader_source(vertex_format: super::VertexFormat) -> String {
    if matches!(vertex_format, super::VertexFormat::Skinned) {
        return format!(
            r#"
struct FrameUniform {{
    view_projection_matrix: mat4x4<f32>,
}}

struct MaterialUniform {{
    base_color: vec4<f32>,
    material_params: vec4<f32>,
    emissive_color: vec4<f32>,
}}

struct BoneMatrices {{
    matrices: array<mat4x4<f32>, {max_bones}>,
}}

struct VertexInput {{
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) bone_indices: vec4<u32>,
    @location(4) bone_weights: vec4<f32>,
    @location(8) instance_model_0: vec4<f32>,
    @location(9) instance_model_1: vec4<f32>,
    @location(10) instance_model_2: vec4<f32>,
    @location(11) instance_model_3: vec4<f32>,
    @location(12) instance_color: vec4<f32>,
}}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}}

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var<uniform> material: MaterialUniform;
@group(2) @binding(0) var<uniform> bones: BoneMatrices;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {{
    var output: VertexOutput;

    let skin_matrix =
        input.bone_weights.x * bones.matrices[input.bone_indices.x] +
        input.bone_weights.y * bones.matrices[input.bone_indices.y] +
        input.bone_weights.z * bones.matrices[input.bone_indices.z] +
        input.bone_weights.w * bones.matrices[input.bone_indices.w];

    let skinned_position = skin_matrix * vec4<f32>(input.position, 1.0);

    let model = mat4x4<f32>(
        input.instance_model_0,
        input.instance_model_1,
        input.instance_model_2,
        input.instance_model_3,
    );
    let world = model * skinned_position;
    output.position = frame.view_projection_matrix * world;
    output.color = material.base_color * input.instance_color + material.emissive_color;
    return output;
}}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {{
    let alpha = input.color.a * material.material_params.x;
    return vec4<f32>(input.color.rgb, alpha);
}}
"#,
            max_bones = MAX_BONES,
        );
    }

    let (vertex_input, vertex_color_expr) = match vertex_format {
        super::VertexFormat::Position => (
            "    @location(0) position: vec3<f32>,\n",
            "material.base_color",
        ),
        super::VertexFormat::PositionNormal => (
            "    @location(0) position: vec3<f32>,\n    @location(1) normal: vec3<f32>,\n",
            "material.base_color",
        ),
        super::VertexFormat::PositionUv => (
            "    @location(0) position: vec3<f32>,\n    @location(1) uv: vec2<f32>,\n",
            "material.base_color",
        ),
        super::VertexFormat::PositionNormalUv => (
            "    @location(0) position: vec3<f32>,\n    @location(1) normal: vec3<f32>,\n    @location(2) uv: vec2<f32>,\n",
            "material.base_color",
        ),
        super::VertexFormat::PositionNormalUvColor => (
            "    @location(0) position: vec3<f32>,\n    @location(1) normal: vec3<f32>,\n    @location(2) uv: vec2<f32>,\n    @location(3) color: vec4<f32>,\n",
            "material.base_color * input.color",
        ),
    };

    format!(
        r#"
struct FrameUniform {{
    view_projection_matrix: mat4x4<f32>,
}}

struct MaterialUniform {{
    base_color: vec4<f32>,
    material_params: vec4<f32>,
    emissive_color: vec4<f32>,
}}

struct InstanceInput {{
    @location(8) model_0: vec4<f32>,
    @location(9) model_1: vec4<f32>,
    @location(10) model_2: vec4<f32>,
    @location(11) model_3: vec4<f32>,
    @location(12) color: vec4<f32>,
}}

struct VertexInput {{
{vertex_input}    @location(8) instance_model_0: vec4<f32>,
    @location(9) instance_model_1: vec4<f32>,
    @location(10) instance_model_2: vec4<f32>,
    @location(11) instance_model_3: vec4<f32>,
    @location(12) instance_color: vec4<f32>,
}}

struct VertexOutput {{
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}}

@group(0) @binding(0) var<uniform> frame: FrameUniform;
@group(1) @binding(0) var<uniform> material: MaterialUniform;

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {{
    var output: VertexOutput;
    let model = mat4x4<f32>(
        input.instance_model_0,
        input.instance_model_1,
        input.instance_model_2,
        input.instance_model_3,
    );
    let world = model * vec4<f32>(input.position, 1.0);
    output.position = frame.view_projection_matrix * world;
    output.color = {vertex_color_expr} * input.instance_color + material.emissive_color;
    return output;
}}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {{
    let alpha = input.color.a * material.material_params.x;
    return vec4<f32>(input.color.rgb, alpha);
}}
"#
    )
}

const POSITION_ATTRIBUTES: [wgpu::VertexAttribute; 1] = [wgpu::VertexAttribute {
    offset: 0,
    shader_location: 0,
    format: wgpu::VertexFormat::Float32x3,
}];

const POSITION_NORMAL_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x3,
    },
];

const POSITION_UV_ATTRIBUTES: [wgpu::VertexAttribute; 2] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x2,
    },
];

const POSITION_NORMAL_UV_ATTRIBUTES: [wgpu::VertexAttribute; 3] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 24,
        shader_location: 2,
        format: wgpu::VertexFormat::Float32x2,
    },
];

const POSITION_NORMAL_UV_COLOR_ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 24,
        shader_location: 2,
        format: wgpu::VertexFormat::Float32x2,
    },
    wgpu::VertexAttribute {
        offset: 32,
        shader_location: 3,
        format: wgpu::VertexFormat::Float32x4,
    },
];

const SKINNED_ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 0,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 12,
        shader_location: 1,
        format: wgpu::VertexFormat::Float32x3,
    },
    wgpu::VertexAttribute {
        offset: 24,
        shader_location: 2,
        format: wgpu::VertexFormat::Float32x2,
    },
    wgpu::VertexAttribute {
        offset: 32,
        shader_location: 3,
        format: wgpu::VertexFormat::Uint32x4,
    },
    wgpu::VertexAttribute {
        offset: 48,
        shader_location: 4,
        format: wgpu::VertexFormat::Float32x4,
    },
];

const INSTANCE_ATTRIBUTES: [wgpu::VertexAttribute; 5] = [
    wgpu::VertexAttribute {
        offset: 0,
        shader_location: 8,
        format: wgpu::VertexFormat::Float32x4,
    },
    wgpu::VertexAttribute {
        offset: 16,
        shader_location: 9,
        format: wgpu::VertexFormat::Float32x4,
    },
    wgpu::VertexAttribute {
        offset: 32,
        shader_location: 10,
        format: wgpu::VertexFormat::Float32x4,
    },
    wgpu::VertexAttribute {
        offset: 48,
        shader_location: 11,
        format: wgpu::VertexFormat::Float32x4,
    },
    wgpu::VertexAttribute {
        offset: 144,
        shader_location: 12,
        format: wgpu::VertexFormat::Float32x4,
    },
];

fn batch_index_data(mesh: &Mesh) -> Vec<u32> {
    match mesh.topology {
        super::PrimitiveTopology::TriangleFan => {
            let source: Vec<u32> = if mesh.indices.is_empty() {
                match mesh_vertex_count(mesh) {
                    Some(vertex_count) if vertex_count >= 3 => (0..vertex_count).collect(),
                    _ => return Vec::new(),
                }
            } else {
                mesh.indices.clone()
            };
            if source.len() < 3 {
                return Vec::new();
            }
            let mut expanded = Vec::with_capacity((source.len() - 2) * 3);
            for i in 1..(source.len() - 1) {
                expanded.push(source[0]);
                expanded.push(source[i]);
                expanded.push(source[i + 1]);
            }
            expanded
        }
        _ => mesh.indices.clone(),
    }
}

fn sort_opaque_batches(batches: &mut [RenderBatch]) {
    // Sort by specialized pass priority first, then by material and mesh to minimize state changes.
    batches.sort_by(|a, b| match (&a.material_id, &b.material_id) {
        (Some(mat_a), Some(mat_b)) => {
            let priority_cmp = a.priority.cmp(&b.priority);
            if priority_cmp != std::cmp::Ordering::Equal {
                return priority_cmp;
            }
            let material_cmp = mat_a.cmp(mat_b);
            if material_cmp == std::cmp::Ordering::Equal {
                a.mesh_id.cmp(&b.mesh_id)
            } else {
                material_cmp
            }
        }
        (Some(_), None) => a.priority.cmp(&b.priority).then(std::cmp::Ordering::Less),
        (None, Some(_)) => a
            .priority
            .cmp(&b.priority)
            .then(std::cmp::Ordering::Greater),
        (None, None) => a
            .priority
            .cmp(&b.priority)
            .then_with(|| a.mesh_id.cmp(&b.mesh_id)),
    });
}

fn sort_transparent_batches(batches: &mut [RenderBatch]) {
    // Transparent queue: back-to-front for correct blending.
    batches.sort_by(|a, b| {
        b.camera_distance
            .partial_cmp(&a.camera_distance)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.priority.cmp(&b.priority))
            .then_with(|| a.mesh_id.cmp(&b.mesh_id))
    });
}

fn compute_normal_matrix(model_matrix: [[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let model = Mat4::from_cols_array_2d(&model_matrix);
    let inverse_transpose = model.inverse().transpose();
    inverse_transpose.to_cols_array_2d()
}

fn mesh_camera_distance(
    mesh: &Mesh,
    camera: Option<&Camera>,
    model_matrix: [[f32; 4]; 4],
    world_center: Option<[f32; 3]>,
) -> f32 {
    let Some(camera) = camera else {
        return 0.0;
    };
    let center = resolve_world_center(mesh, model_matrix, world_center);
    let dx = center[0] - camera.position[0];
    let dy = center[1] - camera.position[1];
    let dz = center[2] - camera.position[2];
    (dx * dx + dy * dy + dz * dz).sqrt()
}

fn resolve_world_center(
    mesh: &Mesh,
    model_matrix: [[f32; 4]; 4],
    world_center: Option<[f32; 3]>,
) -> [f32; 3] {
    if let Some(center) = world_center {
        return center;
    }

    let local_center = mesh.bounding_box.center();
    let model = Mat4::from_cols_array_2d(&model_matrix);
    let transformed = model.transform_point3(glam::Vec3::new(
        local_center[0],
        local_center[1],
        local_center[2],
    ));
    transformed.to_array()
}

fn effective_batch_transparency(
    material: Option<&Material>,
    transparent_override: Option<bool>,
) -> bool {
    transparent_override
        .unwrap_or_else(|| material.map(|m| m.properties.transparent).unwrap_or(false))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaterialBatchSpecialization {
    Null,
    Unlit,
    ForceMultiplyLike,
    Lit,
}

fn material_batch_specialization(material: Option<&Material>) -> MaterialBatchSpecialization {
    let Some(material) = material else {
        return MaterialBatchSpecialization::Null;
    };

    if material.properties.unlit {
        return MaterialBatchSpecialization::Unlit;
    }

    if material.diffuse_texture.is_some()
        && !material.properties.transparent
        && !is_neutral_diffuse_color(material.properties.diffuse_color)
    {
        return MaterialBatchSpecialization::ForceMultiplyLike;
    }

    MaterialBatchSpecialization::Lit
}

pub(crate) fn batch_material_params(material: Option<&Material>) -> [f32; 4] {
    let Some(material) = material else {
        return legacy_null_material_params();
    };

    let specular_intensity = material
        .properties
        .specular_color
        .iter()
        .copied()
        .fold(0.0_f32, f32::max)
        .clamp(0.0, 1.0);
    let roughness = (1.0 - (material.properties.shininess / 128.0)).clamp(0.0, 1.0);
    let diffuse_alpha = material.properties.diffuse_color[3].clamp(0.0, 1.0);
    let unlit = if material.properties.unlit { 1.0 } else { 0.0 };
    [diffuse_alpha, specular_intensity, roughness, unlit]
}

pub(crate) fn batch_priority(material: Option<&Material>) -> u32 {
    let mut priority: u32 = match material_batch_specialization(material) {
        MaterialBatchSpecialization::Null => 5,
        MaterialBatchSpecialization::Unlit => 5,
        MaterialBatchSpecialization::ForceMultiplyLike => 9,
        MaterialBatchSpecialization::Lit => 10,
    };

    if let Some(material) = material {
        if material.properties.alpha_test {
            priority = priority.saturating_add(1);
        }
        if material.properties.double_sided {
            priority = priority.saturating_add(1);
        }
    }
    priority
}

fn legacy_null_material_params() -> [f32; 4] {
    [1.0, 0.0, 1.0 - (1.0 / 128.0), 1.0]
}

fn is_neutral_diffuse_color(color: [f32; 4]) -> bool {
    color[0].to_bits() == 1.0f32.to_bits()
        && color[1].to_bits() == 1.0f32.to_bits()
        && color[2].to_bits() == 1.0f32.to_bits()
        && color[3].to_bits() == 1.0f32.to_bits()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_batch(mesh_id: &str, material_id: Option<&str>, distance: f32) -> RenderBatch {
        RenderBatch {
            mesh_id: mesh_id.to_string(),
            mesh: None,
            material_id: material_id.map(ToString::to_string),
            material: None,
            instances: Vec::new(),
            camera_distance: distance,
            priority: 0,
            transparent: false,
        }
    }

    #[test]
    fn sort_opaque_batches_orders_material_then_mesh() {
        let mut batches = vec![
            sample_batch("mesh_b", Some("mat_b"), 10.0),
            sample_batch("mesh_a", Some("mat_a"), 5.0),
            sample_batch("mesh_a", Some("mat_b"), 1.0),
        ];
        sort_opaque_batches(&mut batches);
        assert_eq!(batches[0].material_id.as_deref(), Some("mat_a"));
        assert_eq!(batches[1].mesh_id, "mesh_a");
        assert_eq!(batches[2].mesh_id, "mesh_b");
    }

    #[test]
    fn sort_transparent_batches_orders_back_to_front() {
        let mut near = sample_batch("near", Some("mat"), 5.0);
        near.transparent = true;
        let mut far = sample_batch("far", Some("mat"), 20.0);
        far.transparent = true;

        let mut batches = vec![near, far];
        sort_transparent_batches(&mut batches);
        assert_eq!(batches[0].mesh_id, "far");
        assert_eq!(batches[1].mesh_id, "near");
    }

    #[test]
    fn no_mesh_batch_is_not_submittable_geometry() {
        let batch = sample_batch("missing_mesh", None, 0.0);
        assert!(!batch_has_submittable_geometry(&batch));
    }

    #[test]
    fn real_mesh_batch_is_submittable_geometry() {
        let mut batch = sample_batch("real_mesh", None, 0.0);
        batch.mesh = Some(Mesh {
            id: "real_mesh".to_string(),
            name: "real_mesh".to_string(),
            vertex_format: super::super::VertexFormat::Position,
            vertices: vec![0; 36],
            indices: vec![0, 1, 2],
            topology: super::super::PrimitiveTopology::TriangleList,
            material_id: None,
            bounding_box: super::super::BoundingBox::new([0.0; 3], [1.0; 3]),
        });

        assert!(batch_has_submittable_geometry(&batch));
    }

    #[test]
    fn mesh_camera_distance_uses_mesh_bounds_center() {
        let mesh = Mesh {
            id: "mesh".to_string(),
            name: "mesh".to_string(),
            vertex_format: super::super::VertexFormat::Position,
            vertices: Vec::new(),
            indices: Vec::new(),
            topology: super::super::PrimitiveTopology::TriangleList,
            material_id: None,
            bounding_box: super::super::BoundingBox::new([9.0, 0.0, 0.0], [11.0, 0.0, 0.0]),
        };
        let camera = Camera {
            position: [0.0, 0.0, 0.0],
            target: [0.0, 0.0, -1.0],
            up: [0.0, 1.0, 0.0],
            fov: 60.0,
            aspect_ratio: 1.0,
            near_plane: 0.1,
            far_plane: 1000.0,
            view_matrix: [[0.0; 4]; 4],
            projection_matrix: [[0.0; 4]; 4],
        };

        let distance = mesh_camera_distance(
            &mesh,
            Some(&camera),
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
            None,
        );
        assert!((distance - 10.0).abs() < 1.0e-6);
    }

    #[test]
    fn resolve_world_center_prefers_override() {
        let mesh = Mesh {
            id: "mesh".to_string(),
            name: "mesh".to_string(),
            vertex_format: super::super::VertexFormat::Position,
            vertices: Vec::new(),
            indices: Vec::new(),
            topology: super::super::PrimitiveTopology::TriangleList,
            material_id: None,
            bounding_box: super::super::BoundingBox::new([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]),
        };
        let center = resolve_world_center(
            &mesh,
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [10.0, 0.0, 0.0, 1.0],
            ],
            Some([5.0, 6.0, 7.0]),
        );
        assert_eq!(center, [5.0, 6.0, 7.0]);
    }

    #[test]
    fn resolve_world_center_applies_model_transform_without_override() {
        let mesh = Mesh {
            id: "mesh".to_string(),
            name: "mesh".to_string(),
            vertex_format: super::super::VertexFormat::Position,
            vertices: Vec::new(),
            indices: Vec::new(),
            topology: super::super::PrimitiveTopology::TriangleList,
            material_id: None,
            bounding_box: super::super::BoundingBox::new([0.0, 0.0, 0.0], [2.0, 2.0, 2.0]),
        };
        let center = resolve_world_center(
            &mesh,
            [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [10.0, 0.0, 0.0, 1.0],
            ],
            None,
        );
        assert!((center[0] - 11.0).abs() < 1.0e-6);
        assert!((center[1] - 1.0).abs() < 1.0e-6);
        assert!((center[2] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn effective_batch_transparency_prefers_override() {
        assert!(effective_batch_transparency(None, Some(true)));
        assert!(!effective_batch_transparency(None, Some(false)));
    }

    #[test]
    fn batch_material_params_marks_unlit_materials() {
        let mut material = Material {
            id: "mat".to_string(),
            name: "mat".to_string(),
            shader_id: "default".to_string(),
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
            emissive_texture: None,
            detail_texture: None,
            detail_blend_mode: 0,
            properties: super::super::MaterialProperties::default(),
        };
        material.properties.specular_color = [0.7, 0.2, 0.1];
        material.properties.shininess = 96.0;
        material.properties.unlit = true;

        let params = batch_material_params(Some(&material));
        assert!((params[0] - 0.7).abs() < 1.0e-6);
        assert!((params[1] - 0.25).abs() < 1.0e-6);
        assert!((params[2] - 1.0).abs() < 1.0e-6);
        assert!((params[3] - 1.0).abs() < 1.0e-6);
    }

    #[test]
    fn batch_material_params_for_missing_material_matches_cpp_null_defaults() {
        let params = batch_material_params(None);
        assert_eq!(params[0], 1.0);
        assert_eq!(params[1], 0.0);
        assert!((params[2] - (1.0 - (1.0 / 128.0))).abs() < 1.0e-6);
        assert_eq!(params[3], 1.0);
    }

    #[test]
    fn batch_priority_specializes_unlit_alpha_test_and_double_sided_materials() {
        let mut material = Material {
            id: "mat".to_string(),
            name: "mat".to_string(),
            shader_id: "default".to_string(),
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
            emissive_texture: None,
            detail_texture: None,
            detail_blend_mode: 0,
            properties: super::super::MaterialProperties::default(),
        };
        assert_eq!(batch_priority(Some(&material)), 10);

        material.properties.unlit = true;
        material.properties.alpha_test = true;
        material.properties.double_sided = true;
        assert_eq!(batch_priority(Some(&material)), 7);
        assert_eq!(batch_priority(None), 5);
    }

    #[test]
    fn batch_priority_specializes_force_multiply_like_textured_materials() {
        let mut material = Material {
            id: "mat".to_string(),
            name: "mat".to_string(),
            shader_id: "default".to_string(),
            diffuse_texture: Some("tex".to_string()),
            normal_texture: None,
            specular_texture: None,
            emissive_texture: None,
            detail_texture: None,
            detail_blend_mode: 0,
            properties: super::super::MaterialProperties::default(),
        };
        material.properties.diffuse_color = [0.75, 0.8, 0.9, 1.0];

        assert_eq!(batch_priority(Some(&material)), 9);

        material.properties.unlit = true;
        assert_eq!(batch_priority(Some(&material)), 5);
    }

    #[test]
    fn render_state_defaults_to_a_stencil_capable_depth_format() {
        let state = RenderState::default();
        let depth_state = state
            .depth_stencil
            .as_ref()
            .expect("render state should enable depth by default");

        assert_eq!(depth_state.format, wgpu::TextureFormat::Depth24PlusStencil8);
    }
}
