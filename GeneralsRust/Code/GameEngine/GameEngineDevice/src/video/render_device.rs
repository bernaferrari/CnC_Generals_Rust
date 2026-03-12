//! # Render Device Implementation
//!
//! Provides complete high-level rendering interface abstraction using wgpu.

use super::{ColorFormat, DisplayAdapter, Resolution, Result, VideoDeviceError};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[cfg(feature = "video")]
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Adapter, AddressMode, Backends, BindGroup, BindGroupDescriptor, BindGroupLayout,
    BindGroupLayoutDescriptor, BindingType, BlendState, Buffer, BufferBindingType,
    BufferDescriptor, BufferSlice, BufferUsages, ColorTargetState, ColorWrites, CommandBuffer,
    CommandEncoder, CompareFunction, ComputePass, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, DepthStencilState, Device, Extent3d, Face, Features, FilterMode,
    FragmentState, FrontFace, IndexFormat, Instance, Limits, LoadOp, MultisampleState, Operations,
    Origin3d, PipelineLayout, PipelineLayoutDescriptor, PolygonMode, PowerPreference,
    PrimitiveState, PrimitiveTopology, Queue, RenderPass, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBorderColor, SamplerDescriptor, ShaderModule,
    ShaderModuleDescriptor, ShaderSource, StencilState, StorageTextureAccess, StoreOp, Surface,
    SurfaceConfiguration, TexelCopyBufferLayout, TexelCopyTextureInfo, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, VertexAttribute, VertexBufferLayout,
    VertexFormat, VertexState, VertexStepMode,
};

// GPU allocator currently disabled for compilation - can be re-enabled when needed
// #[cfg(feature = "video")]
// use gpu_alloc::{GpuAllocator, Request};

#[cfg(feature = "video")]
use wgpu_profiler::{GpuProfiler, GpuProfilerSettings, GpuTimerQueryResult};

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec2, Vec3, Vec4};

/// Supported graphics APIs
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum GraphicsApi {
    /// Vulkan API
    Vulkan,
    /// DirectX 12 (Windows)
    DirectX12,
    /// DirectX 11 (Windows)
    DirectX11,
    /// Metal (macOS)
    Metal,
    /// OpenGL
    OpenGL,
    /// WebGPU (cross-platform)
    WebGPU,
}

impl Default for GraphicsApi {
    fn default() -> Self {
        #[cfg(target_os = "windows")]
        return Self::DirectX12;

        #[cfg(target_os = "macos")]
        return Self::Metal;

        #[cfg(target_os = "linux")]
        return Self::Vulkan;

        #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
        Self::WebGPU
    }
}

/// Vertex data for basic rendering
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct Vertex {
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Normal vector (x, y, z)
    pub normal: [f32; 3],
    /// Texture coordinates (u, v)
    pub tex_coords: [f32; 2],
    /// Vertex color (r, g, b, a)
    pub color: [f32; 4],
}

impl Vertex {
    /// Create vertex buffer layout description
    pub fn desc<'a>() -> VertexBufferLayout<'a> {
        VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: VertexStepMode::Vertex,
            attributes: &[
                VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: VertexFormat::Float32x3,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 6]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: VertexFormat::Float32x2,
                },
                VertexAttribute {
                    offset: std::mem::size_of::<[f32; 8]>() as wgpu::BufferAddress,
                    shader_location: 3,
                    format: VertexFormat::Float32x4,
                },
            ],
        }
    }
}

/// Uniform buffer data for camera/view matrices
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct CameraUniform {
    /// View matrix
    pub view_matrix: [[f32; 4]; 4],
    /// Projection matrix
    pub projection_matrix: [[f32; 4]; 4],
    /// View-projection matrix (combined)
    pub view_proj_matrix: [[f32; 4]; 4],
    /// Camera position in world space
    pub camera_position: [f32; 4],
    /// View direction
    pub view_direction: [f32; 4],
}

/// Uniform buffer data for object/model transforms
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct ModelUniform {
    /// Model matrix
    pub model_matrix: [[f32; 4]; 4],
    /// Normal matrix (inverse transpose of model)
    pub normal_matrix: [[f32; 4]; 4],
    /// Model-view-projection matrix
    pub mvp_matrix: [[f32; 4]; 4],
    /// Object color tint
    pub color: [f32; 4],
}

/// Material properties
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct MaterialUniform {
    /// Ambient color
    pub ambient: [f32; 4],
    /// Diffuse color
    pub diffuse: [f32; 4],
    /// Specular color
    pub specular: [f32; 4],
    /// Shininess exponent
    pub shininess: f32,
    /// Metallic factor
    pub metallic: f32,
    /// Roughness factor
    pub roughness: f32,
    /// Padding for alignment
    pub _padding: f32,
}

/// Lighting data
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct LightUniform {
    /// Light position (w = 1 for point, 0 for directional)
    pub position: [f32; 4],
    /// Light direction (for directional/spot lights)
    pub direction: [f32; 4],
    /// Light color and intensity
    pub color: [f32; 4],
    /// Attenuation factors (constant, linear, quadratic, range)
    pub attenuation: [f32; 4],
    /// Spot light parameters (inner_cone, outer_cone, falloff, type)
    pub spot_params: [f32; 4],
}

/// Render target description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderTarget {
    /// Width in pixels
    pub width: u32,
    /// Height in pixels
    pub height: u32,
    /// Color format
    pub format: ColorFormat,
    /// Multi-sampling settings
    pub sample_count: u32,
    /// Mip levels
    pub mip_levels: u32,
    /// Usage flags
    pub usage: RenderTargetUsage,
}

/// Render target usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderTargetUsage {
    /// Can be used as render attachment
    pub render_attachment: bool,
    /// Can be sampled in shaders
    pub shader_resource: bool,
    /// Can be used for compute shader output
    pub storage: bool,
    /// Can be copied to/from
    pub copy_src: bool,
    pub copy_dst: bool,
}

impl Default for RenderTargetUsage {
    fn default() -> Self {
        Self {
            render_attachment: true,
            shader_resource: true,
            storage: false,
            copy_src: true,
            copy_dst: true,
        }
    }
}

/// Shader description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShaderDesc {
    /// Shader ID
    pub id: String,
    /// Shader type
    pub shader_type: ShaderType,
    /// Shader source (WGSL)
    pub source: String,
    /// Entry point function name
    pub entry_point: String,
    /// Preprocessor defines
    pub defines: HashMap<String, String>,
}

/// Shader types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderType {
    /// Vertex shader
    Vertex,
    /// Fragment/Pixel shader
    Fragment,
    /// Compute shader
    Compute,
}

/// Texture description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextureDesc {
    /// Texture width
    pub width: u32,
    /// Texture height
    pub height: u32,
    /// Texture depth (for 3D textures)
    pub depth: u32,
    /// Texture format
    pub format: ColorFormat,
    /// Mip levels (0 = generate all)
    pub mip_levels: u32,
    /// Array layers (for texture arrays)
    pub array_layers: u32,
    /// Sample count for multisampled textures
    pub sample_count: u32,
    /// Usage flags
    pub usage: TextureUsage,
}

/// Texture usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextureUsage {
    /// Can be sampled in shaders
    pub shader_resource: bool,
    /// Can be used as render target
    pub render_target: bool,
    /// Can be used for compute shader output
    pub storage: bool,
    /// Can be copied to/from
    pub copy_src: bool,
    pub copy_dst: bool,
}

impl Default for TextureUsage {
    fn default() -> Self {
        Self {
            shader_resource: true,
            render_target: false,
            storage: false,
            copy_src: true,
            copy_dst: true,
        }
    }
}

/// Buffer description
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BufferDesc {
    /// Buffer size in bytes
    pub size: u64,
    /// Buffer usage
    pub usage: BufferUsageFlags,
    /// Memory location preference
    pub memory_location: BufferMemoryLocation,
}

/// Buffer usage flags
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BufferUsageFlags {
    /// Vertex buffer
    pub vertex: bool,
    /// Index buffer
    pub index: bool,
    /// Uniform buffer
    pub uniform: bool,
    /// Storage buffer
    pub storage: bool,
    /// Can be copied to/from
    pub copy_src: bool,
    pub copy_dst: bool,
}

/// Buffer memory location preference
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum BufferMemoryLocation {
    /// GPU memory (fast access from GPU, slow from CPU)
    GpuOnly,
    /// CPU memory (fast access from CPU, slower from GPU)
    CpuToGpu,
    /// Readback memory (for GPU->CPU transfers)
    GpuToCpu,
}

/// Render context for command recording
pub struct RenderContext {
    /// Graphics API being used
    pub api: GraphicsApi,

    /// WGPU device
    #[cfg(feature = "video")]
    device: Arc<Device>,

    /// WGPU queue
    #[cfg(feature = "video")]
    queue: Arc<Queue>,

    /// Command encoder
    #[cfg(feature = "video")]
    encoder: Option<CommandEncoder>,

    /// Current render pass
    #[cfg(feature = "video")]
    render_pass: Option<RenderPass<'static>>,

    /// Current compute pass
    #[cfg(feature = "video")]
    compute_pass: Option<ComputePass<'static>>,

    /// Active render pipeline
    #[cfg(feature = "video")]
    active_render_pipeline: Option<Arc<RenderPipeline>>,

    /// Active compute pipeline
    #[cfg(feature = "video")]
    active_compute_pipeline: Option<Arc<ComputePipeline>>,

    /// Bound vertex buffers
    #[cfg(feature = "video")]
    bound_vertex_buffers: Vec<Arc<Buffer>>,

    /// Bound index buffer
    #[cfg(feature = "video")]
    bound_index_buffer: Option<(Arc<Buffer>, IndexFormat)>,

    /// Bind groups
    #[cfg(feature = "video")]
    bind_groups: Vec<Arc<BindGroup>>,

    /// GPU profiler
    #[cfg(feature = "video")]
    profiler: Option<Arc<RwLock<GpuProfiler>>>,

    /// Statistics
    statistics: RenderStatistics,
}

/// Render statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RenderStatistics {
    /// Draw calls this frame
    pub draw_calls: u32,
    /// Dispatch calls this frame
    pub dispatch_calls: u32,
    /// Triangles rendered this frame
    pub triangles: u32,
    /// Vertices processed this frame
    pub vertices: u32,
    /// Render targets bound this frame
    pub render_target_switches: u32,
    /// Texture bindings this frame
    pub texture_bindings: u32,
    /// Buffer bindings this frame
    pub buffer_bindings: u32,
    /// Pipeline switches this frame
    pub pipeline_switches: u32,
}

/// Render device capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderCapabilities {
    /// Maximum texture size
    pub max_texture_size_1d: u32,
    pub max_texture_size_2d: u32,
    pub max_texture_size_3d: u32,
    /// Maximum texture array layers
    pub max_texture_array_layers: u32,
    /// Maximum number of render targets
    pub max_render_targets: u32,
    /// Maximum uniform buffer size
    pub max_uniform_buffer_size: u64,
    /// Maximum storage buffer size
    pub max_storage_buffer_size: u64,
    /// Compute shader support
    pub compute_shaders: bool,
    /// Geometry shader support (emulated via compute)
    pub geometry_shaders: bool,
    /// Tessellation support (emulated)
    pub tessellation: bool,
    /// Multi-draw indirect support
    pub multi_draw_indirect: bool,
    /// Conservative rasterization
    pub conservative_rasterization: bool,
    /// Variable rate shading
    pub variable_rate_shading: bool,
    /// Ray tracing support
    pub ray_tracing: bool,
    /// Mesh shaders support
    pub mesh_shaders: bool,
    /// Timestamp queries
    pub timestamp_queries: bool,
    /// Pipeline statistics queries
    pub pipeline_statistics_queries: bool,
    /// Texture compression formats
    pub texture_compression_bc: bool,
    pub texture_compression_etc2: bool,
    pub texture_compression_astc: bool,
    /// HDR support
    pub hdr10_support: bool,
}

/// High-level render device interface
pub struct RenderDevice {
    /// Graphics API
    api: GraphicsApi,

    /// Display adapter info
    adapter_info: DisplayAdapter,

    /// WGPU instance
    #[cfg(feature = "video")]
    instance: Arc<Instance>,

    /// WGPU adapter
    #[cfg(feature = "video")]
    adapter: Arc<Adapter>,

    /// WGPU device
    #[cfg(feature = "video")]
    device: Arc<Device>,

    /// WGPU queue
    #[cfg(feature = "video")]
    queue: Arc<Queue>,

    /// Memory allocator (disabled for compilation compatibility)
    #[cfg(feature = "video")]
    _allocator_placeholder: Option<()>,

    /// GPU profiler
    #[cfg(feature = "video")]
    profiler: Arc<RwLock<GpuProfiler>>,

    /// Device capabilities
    capabilities: RenderCapabilities,

    /// Shader cache
    shader_cache: Arc<RwLock<HashMap<String, Arc<ShaderModule>>>>,

    /// Pipeline cache
    render_pipeline_cache: Arc<RwLock<HashMap<String, Arc<RenderPipeline>>>>,
    compute_pipeline_cache: Arc<RwLock<HashMap<String, Arc<ComputePipeline>>>>,

    /// Bind group layout cache
    bind_group_layout_cache: Arc<RwLock<HashMap<String, Arc<BindGroupLayout>>>>,

    /// Texture cache
    texture_cache: Arc<RwLock<HashMap<String, Arc<Texture>>>>,

    /// Buffer pool
    buffer_pool: Arc<RwLock<HashMap<String, Arc<Buffer>>>>,

    /// Active contexts
    contexts: Arc<RwLock<Vec<RenderContext>>>,
}

impl RenderDevice {
    /// Create a new render device from adapter
    pub async fn new_from_adapter(adapter_info: DisplayAdapter) -> Result<Self> {
        #[cfg(feature = "video")]
        {
            let mut backend_options = wgpu::BackendOptions::default();
            backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;

            let instance = Arc::new(Instance::new(&wgpu::InstanceDescriptor {
                backends: Backends::all(),
                flags: wgpu::InstanceFlags::default(),
                memory_budget_thresholds: Default::default(),
                backend_options,
            }));

            // Request adapter with high performance preference
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: PowerPreference::HighPerformance,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|_| {
                    VideoDeviceError::AdapterNotFound("No suitable adapter found".to_string())
                })?;

            let adapter = Arc::new(adapter);

            // Get device features and limits
            let features = adapter.features();
            let limits = adapter.limits();

            // Request device with maximum features
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("RenderDevice"),
                    required_features: features & Self::get_required_features(),
                    required_limits: limits.clone(),
                    ..Default::default()
                })
                .await
                .map_err(|e| {
                    VideoDeviceError::InitializationFailed(format!(
                        "Failed to create device: {}",
                        e
                    ))
                })?;

            let device = Arc::new(device);
            let queue = Arc::new(queue);

            // Memory allocator placeholder (disabled for now)
            let _allocator_placeholder = None;

            // Initialize GPU profiler
            let profiler = Arc::new(RwLock::new(
                GpuProfiler::new(&device, GpuProfilerSettings::default()).unwrap(),
            ));

            let api = match adapter.get_info().backend {
                wgpu::Backend::Vulkan => GraphicsApi::Vulkan,
                wgpu::Backend::Dx12 => GraphicsApi::DirectX12,
                wgpu::Backend::Metal => GraphicsApi::Metal,
                wgpu::Backend::Gl => GraphicsApi::OpenGL,
                wgpu::Backend::BrowserWebGpu => GraphicsApi::WebGPU,
                _ => GraphicsApi::WebGPU,
            };

            let capabilities = Self::build_capabilities(&adapter, &features, &limits);

            Ok(Self {
                api,
                adapter_info,
                instance,
                adapter,
                device,
                queue,
                _allocator_placeholder,
                profiler,
                capabilities,
                shader_cache: Arc::new(RwLock::new(HashMap::new())),
                render_pipeline_cache: Arc::new(RwLock::new(HashMap::new())),
                compute_pipeline_cache: Arc::new(RwLock::new(HashMap::new())),
                bind_group_layout_cache: Arc::new(RwLock::new(HashMap::new())),
                texture_cache: Arc::new(RwLock::new(HashMap::new())),
                buffer_pool: Arc::new(RwLock::new(HashMap::new())),
                contexts: Arc::new(RwLock::new(Vec::new())),
            })
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Create a new render device with auto-selected adapter
    pub async fn new() -> Result<Self> {
        let adapter = DisplayAdapter::get_primary().await?;
        Self::new_from_adapter(adapter).await
    }

    /// Create a render context
    pub fn create_context(&self) -> Result<RenderContext> {
        #[cfg(feature = "video")]
        {
            let context = RenderContext {
                api: self.api,
                device: self.device.clone(),
                queue: self.queue.clone(),
                encoder: None,
                render_pass: None,
                compute_pass: None,
                active_render_pipeline: None,
                active_compute_pipeline: None,
                bound_vertex_buffers: Vec::new(),
                bound_index_buffer: None,
                bind_groups: Vec::new(),
                profiler: Some(self.profiler.clone()),
                statistics: RenderStatistics::default(),
            };

            Ok(context)
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Get device capabilities
    pub fn get_capabilities(&self) -> &RenderCapabilities {
        &self.capabilities
    }

    /// Get adapter information
    pub fn get_adapter_info(&self) -> &DisplayAdapter {
        &self.adapter_info
    }

    /// Get underlying WGPU device.
    #[cfg(feature = "video")]
    pub fn get_wgpu_device(&self) -> &Device {
        self.device.as_ref()
    }

    /// Get underlying WGPU queue.
    #[cfg(feature = "video")]
    pub fn get_wgpu_queue(&self) -> &Queue {
        self.queue.as_ref()
    }

    /// Create a shader module
    pub async fn create_shader(&self, desc: &ShaderDesc) -> Result<Arc<ShaderModule>> {
        #[cfg(feature = "video")]
        {
            // Check cache first
            let cache_key = format!("{}_{:?}", desc.id, desc.shader_type);
            if let Some(shader) = self.shader_cache.read().get(&cache_key) {
                return Ok(shader.clone());
            }

            // Preprocess shader source with defines
            let mut source = desc.source.clone();
            for (key, value) in &desc.defines {
                source = source.replace(
                    &format!("#define {}", key),
                    &format!("#define {} {}", key, value),
                );
            }

            let shader_module = self.device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&desc.id),
                source: ShaderSource::Wgsl(source.into()),
            });

            let shader_module = Arc::new(shader_module);

            // Cache the shader
            self.shader_cache
                .write()
                .insert(cache_key, shader_module.clone());

            tracing::debug!("Created shader: {} (type: {:?})", desc.id, desc.shader_type);
            Ok(shader_module)
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Create a render pipeline
    pub async fn create_render_pipeline(
        &self,
        label: &str,
        vertex_shader: &ShaderDesc,
        fragment_shader: Option<&ShaderDesc>,
        vertex_buffers: &[VertexBufferLayout<'_>],
        bind_group_layouts: &[&BindGroupLayout],
        render_targets: &[Option<ColorTargetState>],
        depth_stencil: Option<DepthStencilState>,
        primitive: PrimitiveState,
        multisample: MultisampleState,
    ) -> Result<Arc<RenderPipeline>> {
        #[cfg(feature = "video")]
        {
            // Check cache
            let cache_key = label.to_string();
            if let Some(pipeline) = self.render_pipeline_cache.read().get(&cache_key) {
                return Ok(pipeline.clone());
            }

            let vs_module = self.create_shader(vertex_shader).await?;
            let fs_module = if let Some(fs_desc) = fragment_shader {
                Some(self.create_shader(fs_desc).await?)
            } else {
                None
            };

            let pipeline_layout = self
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some(&format!("{}_layout", label)),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });

            let pipeline = self
                .device
                .create_render_pipeline(&RenderPipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    vertex: VertexState {
                        module: &vs_module,
                        entry_point: Some(&vertex_shader.entry_point),
                        buffers: vertex_buffers,
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    },
                    primitive,
                    depth_stencil,
                    multisample,
                    fragment: fs_module.as_ref().map(|fs| FragmentState {
                        module: fs,
                        entry_point: Some(&fragment_shader.as_ref().unwrap().entry_point),
                        targets: render_targets,
                        compilation_options: wgpu::PipelineCompilationOptions::default(),
                    }),
                    cache: None,
                    multiview: None,
                });

            let pipeline = Arc::new(pipeline);
            self.render_pipeline_cache
                .write()
                .insert(cache_key, pipeline.clone());

            tracing::debug!("Created render pipeline: {}", label);
            Ok(pipeline)
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Create a compute pipeline
    pub async fn create_compute_pipeline(
        &self,
        label: &str,
        compute_shader: &ShaderDesc,
        bind_group_layouts: &[&BindGroupLayout],
    ) -> Result<Arc<ComputePipeline>> {
        #[cfg(feature = "video")]
        {
            let cache_key = label.to_string();
            if let Some(pipeline) = self.compute_pipeline_cache.read().get(&cache_key) {
                return Ok(pipeline.clone());
            }

            let cs_module = self.create_shader(compute_shader).await?;

            let pipeline_layout = self
                .device
                .create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some(&format!("{}_layout", label)),
                    bind_group_layouts,
                    push_constant_ranges: &[],
                });

            let pipeline = self
                .device
                .create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some(label),
                    layout: Some(&pipeline_layout),
                    module: &cs_module,
                    entry_point: Some(&compute_shader.entry_point),
                    compilation_options: wgpu::PipelineCompilationOptions::default(),
                    cache: None,
                });

            let pipeline = Arc::new(pipeline);
            self.compute_pipeline_cache
                .write()
                .insert(cache_key, pipeline.clone());

            tracing::debug!("Created compute pipeline: {}", label);
            Ok(pipeline)
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Create a texture
    pub fn create_texture(&self, desc: &TextureDesc, data: Option<&[u8]>) -> Result<Arc<Texture>> {
        #[cfg(feature = "video")]
        {
            let format = Self::map_color_format_to_wgpu(desc.format);
            let usage = Self::map_texture_usage_to_wgpu(desc.usage);

            let texture = self.device.create_texture(&TextureDescriptor {
                label: None,
                size: Extent3d {
                    width: desc.width,
                    height: desc.height,
                    depth_or_array_layers: desc.depth.max(1),
                },
                mip_level_count: if desc.mip_levels == 0 {
                    ((desc.width.max(desc.height) as f32).log2().floor() as u32) + 1
                } else {
                    desc.mip_levels
                },
                sample_count: desc.sample_count.max(1),
                dimension: if desc.depth > 1 {
                    TextureDimension::D3
                } else {
                    TextureDimension::D2
                },
                format,
                usage,
                view_formats: &[],
            });

            // Upload initial data if provided
            if let Some(data) = data {
                let bytes_per_pixel = Self::get_bytes_per_pixel(desc.format);
                self.queue.write_texture(
                    TexelCopyTextureInfo {
                        texture: &texture,
                        mip_level: 0,
                        origin: Origin3d::ZERO,
                        aspect: TextureAspect::All,
                    },
                    data,
                    TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(desc.width * bytes_per_pixel),
                        rows_per_image: Some(desc.height),
                    },
                    Extent3d {
                        width: desc.width,
                        height: desc.height,
                        depth_or_array_layers: 1,
                    },
                );
            }

            Ok(Arc::new(texture))
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Create a buffer
    pub fn create_buffer(&self, desc: &BufferDesc, data: Option<&[u8]>) -> Result<Arc<Buffer>> {
        #[cfg(feature = "video")]
        {
            let usage = Self::map_buffer_usage_to_wgpu(desc.usage);

            let buffer = if let Some(data) = data {
                self.device.create_buffer_init(&BufferInitDescriptor {
                    label: None,
                    contents: data,
                    usage,
                })
            } else {
                self.device.create_buffer(&BufferDescriptor {
                    label: None,
                    size: desc.size,
                    usage,
                    mapped_at_creation: false,
                })
            };

            Ok(Arc::new(buffer))
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Create a render target
    pub fn create_render_target(
        &self,
        desc: &RenderTarget,
    ) -> Result<(Arc<Texture>, Arc<TextureView>)> {
        let texture_desc = TextureDesc {
            width: desc.width,
            height: desc.height,
            depth: 1,
            format: desc.format,
            mip_levels: desc.mip_levels,
            array_layers: 1,
            sample_count: desc.sample_count,
            usage: TextureUsage {
                shader_resource: desc.usage.shader_resource,
                render_target: desc.usage.render_attachment,
                storage: desc.usage.storage,
                copy_src: desc.usage.copy_src,
                copy_dst: desc.usage.copy_dst,
            },
        };

        let texture = self.create_texture(&texture_desc, None)?;

        #[cfg(feature = "video")]
        {
            let view = texture.create_view(&TextureViewDescriptor::default());
            Ok((texture, Arc::new(view)))
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    // Helper methods

    #[cfg(feature = "video")]
    fn get_required_features() -> Features {
        Features::DEPTH_CLIP_CONTROL
            | Features::TIMESTAMP_QUERY
            | Features::TEXTURE_COMPRESSION_BC
            | Features::TEXTURE_COMPRESSION_ETC2
            | Features::TEXTURE_COMPRESSION_ASTC
            | Features::INDIRECT_FIRST_INSTANCE
            | Features::SHADER_F16
            | Features::RG11B10UFLOAT_RENDERABLE
            | Features::BGRA8UNORM_STORAGE
            | Features::FLOAT32_FILTERABLE
            | Features::MULTI_DRAW_INDIRECT_COUNT
            | Features::ADDRESS_MODE_CLAMP_TO_BORDER
            | Features::POLYGON_MODE_LINE
            | Features::POLYGON_MODE_POINT
            | Features::CLEAR_TEXTURE
    }

    #[cfg(feature = "video")]
    fn build_capabilities(
        adapter: &Adapter,
        features: &Features,
        limits: &Limits,
    ) -> RenderCapabilities {
        RenderCapabilities {
            max_texture_size_1d: limits.max_texture_dimension_1d,
            max_texture_size_2d: limits.max_texture_dimension_2d,
            max_texture_size_3d: limits.max_texture_dimension_3d,
            max_texture_array_layers: limits.max_texture_array_layers,
            max_render_targets: 8, // Common maximum
            max_uniform_buffer_size: limits.max_uniform_buffer_binding_size as u64,
            max_storage_buffer_size: limits.max_storage_buffer_binding_size as u64,
            compute_shaders: true,   // wgpu always supports compute
            geometry_shaders: false, // Not directly supported by wgpu
            tessellation: false,     // Not directly supported by wgpu
            multi_draw_indirect: features.contains(Features::MULTI_DRAW_INDIRECT_COUNT),
            conservative_rasterization: features.contains(Features::CONSERVATIVE_RASTERIZATION),
            variable_rate_shading: false, // Not yet supported by wgpu
            ray_tracing: features.contains(Features::EXPERIMENTAL_RAY_QUERY),
            mesh_shaders: false, // Not yet supported by wgpu
            timestamp_queries: features.contains(Features::TIMESTAMP_QUERY),
            pipeline_statistics_queries: features.contains(Features::PIPELINE_STATISTICS_QUERY),
            texture_compression_bc: features.contains(Features::TEXTURE_COMPRESSION_BC),
            texture_compression_etc2: features.contains(Features::TEXTURE_COMPRESSION_ETC2),
            texture_compression_astc: features.contains(Features::TEXTURE_COMPRESSION_ASTC),
            hdr10_support: true, // Depends on surface format support
        }
    }

    #[cfg(feature = "video")]
    fn map_color_format_to_wgpu(format: ColorFormat) -> TextureFormat {
        match format {
            ColorFormat::Rgba8 => TextureFormat::Rgba8UnormSrgb,
            ColorFormat::Bgra8 => TextureFormat::Bgra8UnormSrgb,
            ColorFormat::Rgba16 => TextureFormat::Rgba16Float,
            ColorFormat::Rgba32Float => TextureFormat::Rgba32Float,
            ColorFormat::Rgb10A2 => TextureFormat::Rgb10a2Unorm,
            ColorFormat::Hdr10 => TextureFormat::Rgb10a2Unorm,
            ColorFormat::Depth24Stencil8 => TextureFormat::Depth24PlusStencil8,
            ColorFormat::Depth32Float => TextureFormat::Depth32Float,
        }
    }

    #[cfg(feature = "video")]
    fn map_texture_usage_to_wgpu(usage: TextureUsage) -> TextureUsages {
        let mut wgpu_usage = TextureUsages::empty();

        if usage.shader_resource {
            wgpu_usage |= TextureUsages::TEXTURE_BINDING;
        }
        if usage.render_target {
            wgpu_usage |= TextureUsages::RENDER_ATTACHMENT;
        }
        if usage.storage {
            wgpu_usage |= TextureUsages::STORAGE_BINDING;
        }
        if usage.copy_src {
            wgpu_usage |= TextureUsages::COPY_SRC;
        }
        if usage.copy_dst {
            wgpu_usage |= TextureUsages::COPY_DST;
        }

        wgpu_usage
    }

    #[cfg(feature = "video")]
    fn map_buffer_usage_to_wgpu(usage: BufferUsageFlags) -> BufferUsages {
        let mut wgpu_usage = BufferUsages::empty();

        if usage.vertex {
            wgpu_usage |= BufferUsages::VERTEX;
        }
        if usage.index {
            wgpu_usage |= BufferUsages::INDEX;
        }
        if usage.uniform {
            wgpu_usage |= BufferUsages::UNIFORM;
        }
        if usage.storage {
            wgpu_usage |= BufferUsages::STORAGE;
        }
        if usage.copy_src {
            wgpu_usage |= BufferUsages::COPY_SRC;
        }
        if usage.copy_dst {
            wgpu_usage |= BufferUsages::COPY_DST;
        }

        wgpu_usage
    }

    fn get_bytes_per_pixel(format: ColorFormat) -> u32 {
        match format {
            ColorFormat::Rgba8 | ColorFormat::Bgra8 => 4,
            ColorFormat::Rgba16 => 8,
            ColorFormat::Rgba32Float => 16,
            ColorFormat::Rgb10A2 | ColorFormat::Hdr10 => 4,
            ColorFormat::Depth24Stencil8 => 4,
            ColorFormat::Depth32Float => 4,
        }
    }
}

impl RenderContext {
    /// Begin command encoding
    pub fn begin_encoding(&mut self, label: Option<&str>) -> Result<()> {
        #[cfg(feature = "video")]
        {
            if self.encoder.is_some() {
                return Err(VideoDeviceError::RenderContextError(
                    "Already encoding commands".to_string(),
                ));
            }

            let encoder = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label });

            self.encoder = Some(encoder);
            self.statistics = RenderStatistics::default();

            Ok(())
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Finish command encoding and submit
    pub fn finish_encoding(&mut self) -> Result<()> {
        #[cfg(feature = "video")]
        {
            if let Some(encoder) = self.encoder.take() {
                let command_buffer = encoder.finish();
                self.queue.submit(std::iter::once(command_buffer));

                tracing::trace!(
                    "Submitted commands - Draw calls: {}, Dispatches: {}, Triangles: {}",
                    self.statistics.draw_calls,
                    self.statistics.dispatch_calls,
                    self.statistics.triangles
                );

                Ok(())
            } else {
                Err(VideoDeviceError::RenderContextError(
                    "No active encoder".to_string(),
                ))
            }
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Get render statistics
    pub fn get_statistics(&self) -> &RenderStatistics {
        &self.statistics
    }
}

impl Default for RenderCapabilities {
    fn default() -> Self {
        Self {
            max_texture_size_1d: 8192,
            max_texture_size_2d: 8192,
            max_texture_size_3d: 2048,
            max_texture_array_layers: 256,
            max_render_targets: 4,
            max_uniform_buffer_size: 65536,
            max_storage_buffer_size: 134217728,
            compute_shaders: true,
            geometry_shaders: false,
            tessellation: false,
            multi_draw_indirect: false,
            conservative_rasterization: false,
            variable_rate_shading: false,
            ray_tracing: false,
            mesh_shaders: false,
            timestamp_queries: false,
            pipeline_statistics_queries: false,
            texture_compression_bc: false,
            texture_compression_etc2: false,
            texture_compression_astc: false,
            hdr10_support: false,
        }
    }
}

impl std::fmt::Display for GraphicsApi {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphicsApi::Vulkan => write!(f, "Vulkan"),
            GraphicsApi::DirectX12 => write!(f, "DirectX 12"),
            GraphicsApi::DirectX11 => write!(f, "DirectX 11"),
            GraphicsApi::Metal => write!(f, "Metal"),
            GraphicsApi::OpenGL => write!(f, "OpenGL"),
            GraphicsApi::WebGPU => write!(f, "WebGPU"),
        }
    }
}

impl std::fmt::Display for ShaderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ShaderType::Vertex => write!(f, "Vertex"),
            ShaderType::Fragment => write!(f, "Fragment"),
            ShaderType::Compute => write!(f, "Compute"),
        }
    }
}
