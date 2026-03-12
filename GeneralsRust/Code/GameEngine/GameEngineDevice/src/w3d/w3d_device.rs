//! # W3D Device Implementation
//!
//! Complete W3D device with modern wgpu-based graphics backend while maintaining
//! full W3D C++ API compatibility. Features hardware-accelerated rendering,
//! advanced materials, lighting, and performance optimizations.

use super::{
    BoundingBox, Camera, GraphicsContext, Light, Material, Mesh, Result, Shader, Texture, W3DError,
    W3DRenderer,
};
use crate::{
    video::{MsaaSettings, Resolution},
    DeviceStatus, DeviceType, PerformanceMetrics,
};
use bytemuck::{Pod, Zeroable};
use glam::Mat4;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::mem;
use std::sync::Arc;
use tokio::sync::RwLock;
use wgpu::{
    Adapter, Backends, BufferUsages, CompositeAlphaMode, Device, Features, Instance, Limits,
    PowerPreference, PresentMode, Queue, RequestAdapterOptions, Surface, SurfaceConfiguration,
    SurfaceTargetUnsafe, TextureUsages,
};
use winit::window::Window;

/// Complete W3D device configuration with modern graphics features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct W3DConfig {
    /// Enable debug mode with validation layers
    pub debug_mode: bool,
    /// Maximum number of dynamic lights
    pub max_lights: u32,
    /// Maximum texture size (power of 2)
    pub max_texture_size: u32,
    /// Enable hardware vertex buffer objects
    pub enable_vbo: bool,
    /// Enable hardware vertex processing
    pub hardware_vertex_processing: bool,
    /// Enable hardware transform & lighting
    pub hardware_tnl: bool,
    /// Texture memory budget in bytes
    pub texture_memory_budget: u64,
    /// Enable texture compression (BC/DXT formats)
    pub texture_compression: bool,
    /// Default shader quality level
    pub shader_quality: ShaderQuality,
    /// Enable occlusion culling
    pub occlusion_culling: bool,
    /// Enable frustum culling
    pub frustum_culling: bool,
    /// Target resolution
    pub resolution: Resolution,
    /// MSAA settings
    pub msaa: MsaaSettings,
    /// Enable V-Sync
    pub vsync: bool,
    /// Power preference (high performance vs battery)
    #[serde(skip)]
    pub power_preference: PowerPreference,
    /// Backend preference (Vulkan, DirectX, Metal, OpenGL)
    #[serde(skip)]
    pub backend: Backends,
    /// Enable compute shader support
    pub enable_compute_shaders: bool,
    /// Enable tessellation support
    pub enable_tessellation: bool,
    /// Maximum uniform buffer size
    pub max_uniform_buffer_size: u64,
    /// Enable instanced rendering
    pub enable_instancing: bool,
    /// Enable multi-draw indirect
    pub enable_multi_draw_indirect: bool,
    /// Thread pool size for asset loading
    pub loader_thread_count: usize,
}

/// Shader quality levels
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderQuality {
    /// Low quality - simplified shaders
    Low,
    /// Medium quality - standard shaders
    Medium,
    /// High quality - advanced shaders with all features
    High,
    /// Ultra quality - maximum quality with all effects
    Ultra,
}

impl Default for W3DConfig {
    fn default() -> Self {
        Self {
            debug_mode: cfg!(debug_assertions),
            max_lights: 256,         // Modern GPUs can handle many more lights
            max_texture_size: 16384, // 16K textures for modern hardware
            enable_vbo: true,
            hardware_vertex_processing: true,
            hardware_tnl: true,
            texture_memory_budget: 2 * 1024 * 1024 * 1024, // 2GB
            texture_compression: true,
            shader_quality: ShaderQuality::High,
            occlusion_culling: true,
            frustum_culling: true,
            resolution: Resolution::hd_1080p(),
            msaa: MsaaSettings::msaa_4x(),
            vsync: true,
            power_preference: PowerPreference::HighPerformance,
            backend: Backends::all(),
            enable_compute_shaders: true,
            enable_tessellation: false, // Requires explicit GPU support check
            max_uniform_buffer_size: 64 * 1024, // 64KB uniform buffers
            enable_instancing: true,
            enable_multi_draw_indirect: true,
            loader_thread_count: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
        }
    }
}

/// Comprehensive W3D device statistics with GPU performance metrics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct W3DStatistics {
    /// Frames rendered per second (current)
    pub fps: f32,
    /// Average frame time in milliseconds
    pub frame_time_ms: f32,
    /// Minimum frame time (best performance)
    pub min_frame_time_ms: f32,
    /// Maximum frame time (worst performance)
    pub max_frame_time_ms: f32,
    /// Draw calls submitted per frame
    pub draw_calls: u32,
    /// Compute dispatches per frame
    pub compute_dispatches: u32,
    /// Triangles rendered per frame
    pub triangles: u32,
    /// Vertices processed per frame
    pub vertices: u32,
    /// Instanced draw calls per frame
    pub instanced_draws: u32,
    /// Total instances rendered per frame
    pub total_instances: u32,
    /// Texture bindings per frame
    pub texture_bindings: u32,
    /// Shader program switches per frame
    pub shader_switches: u32,
    /// Render target switches per frame
    pub render_target_switches: u32,
    /// GPU memory used for textures (bytes)
    pub texture_memory_used: u64,
    /// GPU memory used for vertex/index buffers (bytes)
    pub buffer_memory_used: u64,
    /// GPU memory used for uniform buffers (bytes)
    pub uniform_memory_used: u64,
    /// Number of active lights in scene
    pub active_lights: u32,
    /// Number of shadow-casting lights
    pub shadow_casters: u32,
    /// Objects culled by frustum culling
    pub frustum_culled_objects: u32,
    /// Objects culled by occlusion culling
    pub occlusion_culled_objects: u32,
    /// Objects rendered (passed all culling)
    pub visible_objects: u32,
    /// LOD transitions per frame
    pub lod_transitions: u32,
    /// Texture uploads per frame
    pub texture_uploads: u32,
    /// Buffer updates per frame
    pub buffer_updates: u32,
    /// GPU stall time in milliseconds
    pub gpu_stall_time_ms: f32,
    /// CPU wait time for GPU in milliseconds
    pub cpu_gpu_sync_time_ms: f32,
}

/// Advanced W3D scene description with modern rendering features
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    /// Unique scene identifier
    pub id: String,
    /// Human-readable scene name
    pub name: String,
    /// Active camera for rendering
    pub camera: Camera,
    /// Dynamic lights in the scene
    pub lights: Vec<Light>,
    /// Render objects with transform matrices
    pub render_objects: Vec<RenderObject>,
    /// Ambient light color and intensity
    pub ambient_light: [f32; 3],
    /// Sky/background color
    pub background_color: [f32; 4],
    /// Environment cubemap for reflections
    pub environment_map: Option<String>,
    /// Image-based lighting intensity
    pub ibl_intensity: f32,
    /// Enable volumetric fog
    pub fog_enabled: bool,
    /// Fog parameters (start, end, density, height_falloff)
    pub fog_params: [f32; 4],
    /// Fog color and intensity
    pub fog_color: [f32; 4],
    /// Shadow settings
    pub shadow_settings: ShadowSettings,
    /// Post-processing effects
    pub post_processing: PostProcessingSettings,
    /// Scene bounding box for culling
    pub bounds: BoundingBox,
    /// Level of detail settings
    pub lod_settings: LodSettings,
}

/// Render object combining mesh, material, and transform
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderObject {
    /// Mesh ID to render
    pub mesh_id: String,
    /// Material ID for shading
    pub material_id: Option<String>,
    /// World transform matrix
    pub transform: [[f32; 4]; 4],
    /// Bounding box in world space
    pub world_bounds: BoundingBox,
    /// LOD bias for this object
    pub lod_bias: f32,
    /// Cast shadows
    pub cast_shadows: bool,
    /// Receive shadows
    pub receive_shadows: bool,
    /// Visibility flags for culling
    pub visible: bool,
    /// Whether this object should be rendered in transparent passes
    #[serde(default)]
    pub transparent: bool,
    /// Cached per-instance material parameters for batching/instancing paths
    #[serde(default = "default_render_object_material_params")]
    pub material_params: [f32; 4],
    /// Cached render priority derived from material state
    #[serde(default = "default_render_object_priority")]
    pub priority: u32,
}

fn default_render_object_material_params() -> [f32; 4] {
    [0.0, 0.5, 1.0, 0.0]
}

fn default_render_object_priority() -> u32 {
    10
}

/// Shadow rendering settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShadowSettings {
    /// Enable shadow mapping
    pub enabled: bool,
    /// Shadow map resolution
    pub resolution: u32,
    /// Number of cascade splits for CSM
    pub cascade_count: u32,
    /// Shadow map filtering quality
    pub filter_quality: ShadowFilterQuality,
    /// Shadow bias to prevent shadow acne
    pub depth_bias: f32,
    /// Normal offset bias
    pub normal_bias: f32,
    /// Maximum shadow distance
    pub max_distance: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShadowFilterQuality {
    /// Hard shadows (no filtering)
    Hard,
    /// Percentage-closer filtering
    PCF,
    /// Percentage-closer soft shadows
    PCSS,
    /// Variance shadow maps
    VSM,
}

/// Post-processing settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostProcessingSettings {
    /// Enable bloom effect
    pub bloom_enabled: bool,
    /// Bloom intensity
    pub bloom_intensity: f32,
    /// Enable tone mapping
    pub tone_mapping_enabled: bool,
    /// Exposure value
    pub exposure: f32,
    /// Enable SSAO
    pub ssao_enabled: bool,
    /// SSAO intensity
    pub ssao_intensity: f32,
    /// Enable temporal anti-aliasing
    pub taa_enabled: bool,
    /// Enable FXAA
    pub fxaa_enabled: bool,
}

/// Level of detail settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LodSettings {
    /// Enable automatic LOD selection
    pub enabled: bool,
    /// LOD bias (negative = higher quality, positive = lower quality)
    pub bias: f32,
    /// Distance thresholds for LOD levels
    pub distance_thresholds: Vec<f32>,
    /// Enable LOD morphing for smooth transitions
    pub morphing_enabled: bool,
}

impl Default for Scene {
    fn default() -> Self {
        Self {
            id: "default_scene".to_string(),
            name: "Default Scene".to_string(),
            camera: Camera::default(),
            lights: vec![Light {
                id: "default_sun".to_string(),
                name: "Sun Light".to_string(),
                light_type: super::LightType::Directional,
                position: [0.0, 100.0, 100.0],
                direction: [-0.3, -0.7, -0.6],
                color: [1.0, 0.95, 0.8],
                intensity: 3.0,
                attenuation: [1.0, 0.0, 0.0],
                spot_params: None,
            }],
            render_objects: Vec::new(),
            ambient_light: [0.03, 0.04, 0.06],
            background_color: [0.1, 0.15, 0.25, 1.0],
            environment_map: None,
            ibl_intensity: 1.0,
            fog_enabled: false,
            fog_params: [500.0, 2000.0, 0.0001, 1.0],
            fog_color: [0.6, 0.7, 0.8, 1.0],
            shadow_settings: ShadowSettings {
                enabled: true,
                resolution: 2048,
                cascade_count: 4,
                filter_quality: ShadowFilterQuality::PCF,
                depth_bias: 0.001,
                normal_bias: 0.01,
                max_distance: 1000.0,
            },
            post_processing: PostProcessingSettings {
                bloom_enabled: true,
                bloom_intensity: 0.8,
                tone_mapping_enabled: true,
                exposure: 1.0,
                ssao_enabled: true,
                ssao_intensity: 1.0,
                taa_enabled: true,
                fxaa_enabled: false,
            },
            bounds: BoundingBox::new([-1000.0, -1000.0, -1000.0], [1000.0, 1000.0, 1000.0]),
            lod_settings: LodSettings {
                enabled: true,
                bias: 0.0,
                distance_thresholds: vec![50.0, 150.0, 400.0, 1000.0],
                morphing_enabled: true,
            },
        }
    }
}

/// GPU mesh resource with wgpu buffers
#[derive(Debug)]
pub struct W3DMeshGpu {
    /// Original mesh data
    pub mesh: Mesh,
    /// GPU vertex buffer
    pub vertex_buffer: wgpu::Buffer,
    /// GPU index buffer
    pub index_buffer: Option<wgpu::Buffer>,
    /// Vertex buffer layout for shaders
    pub vertex_layout: wgpu::VertexBufferLayout<'static>,
    /// Number of vertices
    pub vertex_count: u32,
    /// Number of indices
    pub index_count: u32,
    /// GPU memory usage in bytes
    pub gpu_memory_size: u64,
}

/// GPU material resource with shader bindings
#[derive(Debug)]
pub struct W3DMaterialGpu {
    /// Original material data
    pub material: Material,
    /// GPU uniform buffer for material properties
    pub uniform_buffer: wgpu::Buffer,
    /// Bind group for textures and samplers
    pub bind_group: wgpu::BindGroup,
    /// Bind group layout
    pub bind_group_layout: wgpu::BindGroupLayout,
    /// Shader pipeline for this material
    pub render_pipeline: wgpu::RenderPipeline,
    /// GPU memory usage in bytes
    pub gpu_memory_size: u64,
}

/// GPU texture resource with wgpu textures and views
#[derive(Debug)]
pub struct W3DTextureGpu {
    /// Original texture data
    pub texture: Texture,
    /// GPU texture
    pub gpu_texture: wgpu::Texture,
    /// Texture view for shaders
    pub view: wgpu::TextureView,
    /// Sampler for texture filtering
    pub sampler: wgpu::Sampler,
    /// GPU memory usage in bytes
    pub gpu_memory_size: u64,
}

/// GPU shader resource with compiled WGSL
#[derive(Debug)]
pub struct W3DShaderGpu {
    /// Original shader data
    pub shader: Shader,
    /// Compiled vertex shader module
    pub vertex_module: wgpu::ShaderModule,
    /// Compiled fragment shader module
    pub fragment_module: wgpu::ShaderModule,
    /// Compiled compute shader module (optional)
    pub compute_module: Option<wgpu::ShaderModule>,
    /// Bind group layouts for uniform binding
    pub bind_group_layouts: Vec<wgpu::BindGroupLayout>,
    /// Pipeline layout
    pub pipeline_layout: wgpu::PipelineLayout,
}

/// GPU buffer pool for efficient memory management
#[derive(Debug)]
pub struct BufferPool {
    /// Pool of free buffers by size
    free_buffers: HashMap<u64, Vec<wgpu::Buffer>>,
    /// Currently allocated buffers
    allocated_buffers: HashSet<wgpu::Buffer>,
    /// Total memory allocated
    total_memory: u64,
    /// Buffer usage type
    usage: wgpu::BufferUsages,
    /// Buffer alignment requirement
    alignment: u64,
}

impl BufferPool {
    /// Create a new buffer pool
    pub fn new(usage: wgpu::BufferUsages, alignment: u64) -> Self {
        Self {
            free_buffers: HashMap::new(),
            allocated_buffers: HashSet::new(),
            total_memory: 0,
            usage,
            alignment,
        }
    }

    /// Allocate a buffer from the pool
    pub fn allocate(&mut self, device: &wgpu::Device, size: u64) -> wgpu::Buffer {
        let aligned_size = (size + self.alignment - 1) & !(self.alignment - 1);

        // Try to reuse an existing buffer
        if let Some(buffers) = self.free_buffers.get_mut(&aligned_size) {
            if let Some(buffer) = buffers.pop() {
                self.allocated_buffers.insert(buffer.clone());
                return buffer;
            }
        }

        // Create a new buffer
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("W3D Buffer Pool Buffer"),
            size: aligned_size,
            usage: self.usage,
            mapped_at_creation: false,
        });

        self.allocated_buffers.insert(buffer.clone());
        self.total_memory += aligned_size;
        buffer
    }

    /// Return a buffer to the pool
    pub fn deallocate(&mut self, buffer: wgpu::Buffer) {
        let size = buffer.size();
        self.allocated_buffers.remove(&buffer);

        self.free_buffers
            .entry(size)
            .or_insert_with(Vec::new)
            .push(buffer);
    }

    /// Get total memory usage
    pub fn memory_usage(&self) -> u64 {
        self.total_memory
    }
}

/// Vertex data for W3D meshes with GPU-optimal layout
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DVertex {
    /// Position (x, y, z, w)
    pub position: [f32; 4],
    /// Normal vector (x, y, z, w)
    pub normal: [f32; 4],
    /// Texture coordinates (u, v, u2, v2)
    pub tex_coords: [f32; 4],
    /// Vertex color (r, g, b, a)
    pub color: [f32; 4],
    /// Bone indices for skinning (4 bones max)
    pub bone_indices: [u32; 4],
    /// Bone weights for skinning (4 weights)
    pub bone_weights: [f32; 4],
}

impl W3DVertex {
    /// Get vertex buffer layout for wgpu
    pub fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Self>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                // Position
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 0,
                    shader_location: 0,
                },
                // Normal
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 16,
                    shader_location: 1,
                },
                // Texture coordinates
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 32,
                    shader_location: 2,
                },
                // Color
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 48,
                    shader_location: 3,
                },
                // Bone indices
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Uint32x4,
                    offset: 64,
                    shader_location: 4,
                },
                // Bone weights
                wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x4,
                    offset: 80,
                    shader_location: 5,
                },
            ],
        }
    }
}

/// Uniform buffer data for W3D shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DUniformData {
    /// Model-View-Projection matrix
    pub mvp_matrix: [[f32; 4]; 4],
    /// Model matrix
    pub model_matrix: [[f32; 4]; 4],
    /// View matrix
    pub view_matrix: [[f32; 4]; 4],
    /// Projection matrix
    pub projection_matrix: [[f32; 4]; 4],
    /// Normal matrix (inverse transpose of model)
    pub normal_matrix: [[f32; 4]; 4],
    /// Camera position
    pub camera_position: [f32; 4],
    /// Time value for animations
    pub time: f32,
    /// Frame delta time
    pub delta_time: f32,
    /// Reserved padding
    pub _padding: [f32; 2],
}

/// Light uniform data for shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DLightData {
    /// Light position (w=1) or direction (w=0)
    pub position_or_direction: [f32; 4],
    /// Light color and intensity
    pub color_intensity: [f32; 4],
    /// Attenuation parameters (constant, linear, quadratic, range)
    pub attenuation: [f32; 4],
    /// Spot light parameters (inner_cos, outer_cos, unused, unused)
    pub spot_params: [f32; 4],
    /// Light type (0=directional, 1=point, 2=spot, 3=area)
    pub light_type: u32,
    /// Cast shadows flag
    pub cast_shadows: u32,
    /// Reserved padding
    pub _padding: [u32; 2],
}

/// Material uniform data for shaders
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DMaterialData {
    /// Base color (albedo)
    pub base_color: [f32; 4],
    /// Metallic, roughness, ambient occlusion, fixed-function unlit flag
    pub material_params: [f32; 4],
    /// Emissive color and intensity
    pub emissive: [f32; 4],
    /// Normal map scale, height scale, unused, unused
    pub texture_params: [f32; 4],
}

/// Complete W3D device with modern wgpu backend
pub struct W3DDevice {
    /// Device configuration
    config: Arc<RwLock<W3DConfig>>,

    /// WGPU instance for creating adapters
    instance: Arc<Instance>,

    /// WGPU adapter (GPU/integrated graphics)
    adapter: Arc<RwLock<Option<Adapter>>>,

    /// WGPU logical device
    device: Arc<RwLock<Option<Device>>>,

    /// WGPU command queue
    queue: Arc<RwLock<Option<Queue>>>,

    /// Surface for rendering (window)
    surface: Arc<RwLock<Option<Surface<'static>>>>,

    /// Surface configuration
    surface_config: Arc<RwLock<Option<SurfaceConfiguration>>>,

    /// W3D renderer with wgpu backend
    renderer: Arc<RwLock<Option<W3DRenderer>>>,

    /// Graphics context with wgpu state management
    graphics_context: Arc<RwLock<Option<GraphicsContext>>>,

    /// Device statistics and performance metrics
    statistics: Arc<RwLock<W3DStatistics>>,

    /// Resource management with GPU buffers
    meshes: Arc<RwLock<HashMap<String, Mesh>>>,
    materials: Arc<RwLock<HashMap<String, Material>>>,
    textures: Arc<RwLock<HashMap<String, Texture>>>,
    shaders: Arc<RwLock<HashMap<String, Shader>>>,

    /// GPU buffer pools for efficient memory management
    vertex_buffer_pool: Arc<RwLock<BufferPool>>,
    index_buffer_pool: Arc<RwLock<BufferPool>>,
    uniform_buffer_pool: Arc<RwLock<BufferPool>>,

    /// Current scene with render objects
    current_scene: Arc<RwLock<Scene>>,

    /// Frame-in-flight management
    frame_counter: Arc<RwLock<u64>>,

    /// Command buffer recording
    command_encoder: Arc<RwLock<Option<wgpu::CommandEncoder>>>,

    /// Render pass management
    current_render_pass: Arc<RwLock<Option<wgpu::RenderPass<'static>>>>,

    /// Initialization and lifecycle state
    initialized: Arc<RwLock<bool>>,
    shutting_down: Arc<RwLock<bool>>,
}

impl W3DDevice {
    /// Create a new W3D device with default configuration
    pub async fn new() -> Result<Self> {
        Self::new_with_config(W3DConfig::default()).await
    }

    /// Create a new W3D device with custom configuration
    pub async fn new_with_config(config: W3DConfig) -> Result<Self> {
        tracing::info!("Creating W3D device with wgpu backend");

        // Create wgpu instance
        let mut backend_options = wgpu::BackendOptions::default();
        backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::default();

        let instance = Instance::new(&wgpu::InstanceDescriptor {
            backends: config.backend,
            memory_budget_thresholds: Default::default(),
            backend_options,
            ..Default::default()
        });

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            instance: Arc::new(instance),
            adapter: Arc::new(RwLock::new(None)),
            device: Arc::new(RwLock::new(None)),
            queue: Arc::new(RwLock::new(None)),
            surface: Arc::new(RwLock::new(None)),
            surface_config: Arc::new(RwLock::new(None)),
            renderer: Arc::new(RwLock::new(None)),
            graphics_context: Arc::new(RwLock::new(None)),
            statistics: Arc::new(RwLock::new(W3DStatistics::default())),
            meshes: Arc::new(RwLock::new(HashMap::new())),
            materials: Arc::new(RwLock::new(HashMap::new())),
            textures: Arc::new(RwLock::new(HashMap::new())),
            shaders: Arc::new(RwLock::new(HashMap::new())),
            vertex_buffer_pool: Arc::new(RwLock::new(BufferPool::new(
                BufferUsages::VERTEX,
                256, // 256-byte alignment for vertex buffers
            ))),
            index_buffer_pool: Arc::new(RwLock::new(BufferPool::new(
                BufferUsages::INDEX,
                64, // 64-byte alignment for index buffers
            ))),
            uniform_buffer_pool: Arc::new(RwLock::new(BufferPool::new(
                BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                256, // 256-byte alignment for uniform buffers (required by some GPUs)
            ))),
            current_scene: Arc::new(RwLock::new(Scene::default())),
            frame_counter: Arc::new(RwLock::new(0)),
            command_encoder: Arc::new(RwLock::new(None)),
            current_render_pass: Arc::new(RwLock::new(None)),
            initialized: Arc::new(RwLock::new(false)),
            shutting_down: Arc::new(RwLock::new(false)),
        })
    }

    /// Initialize the W3D device with default/headless surface configuration.
    pub async fn init(&self) -> Result<()> {
        self.init_internal(None, None).await
    }

    async fn init_internal(
        &self,
        mut surface: Option<Surface<'static>>,
        requested_size: Option<(u32, u32)>,
    ) -> Result<()> {
        tracing::info!("Initializing W3D device with wgpu backend");

        let config = self.config.read().await;

        // Request adapter
        let adapter = self
            .instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: config.power_preference,
                compatible_surface: surface.as_ref(),
                force_fallback_adapter: false,
            })
            .await
            .map_err(|_| W3DError::InitializationFailed("No suitable adapter found".to_string()))?;

        tracing::info!("Selected adapter: {}", adapter.get_info().name);

        // Get required features and limits
        let mut required_features = Features::empty();
        if config.enable_multi_draw_indirect {
            required_features |= Features::MULTI_DRAW_INDIRECT_COUNT;
        }

        let required_limits = Limits {
            max_uniform_buffer_binding_size: config.max_uniform_buffer_size as u32,
            max_storage_buffer_binding_size: 1024 * 1024 * 128, // 128MB storage buffers
            ..Limits::default()
        };

        // Create device and queue
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("W3D Device"),
                required_features: required_features & adapter.features(),
                required_limits: required_limits.using_resolution(adapter.limits()),
                ..Default::default()
            })
            .await
            .map_err(|e| {
                W3DError::InitializationFailed(format!("Failed to create device: {:?}", e))
            })?;

        tracing::info!("Created wgpu device with features: {:?}", device.features());

        let (surface_format, surface_config) = if let Some(surface_ref) = surface.as_ref() {
            let capabilities = surface_ref.get_capabilities(&adapter);
            let surface_format = select_surface_format(&capabilities.formats);
            let present_mode = select_present_mode(&capabilities.present_modes, config.vsync);
            let alpha_mode = select_alpha_mode(&capabilities.alpha_modes);
            let (width, height) =
                requested_size.unwrap_or((config.resolution.width, config.resolution.height));

            let surface_config = SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: width.max(1),
                height: height.max(1),
                present_mode,
                alpha_mode,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };

            surface_ref.configure(&device, &surface_config);
            (surface_format, surface_config)
        } else {
            let surface_format = wgpu::TextureFormat::Bgra8UnormSrgb;
            let surface_config = SurfaceConfiguration {
                usage: TextureUsages::RENDER_ATTACHMENT,
                format: surface_format,
                width: config.resolution.width.max(1),
                height: config.resolution.height.max(1),
                present_mode: if config.vsync {
                    PresentMode::Fifo
                } else {
                    PresentMode::Immediate
                },
                alpha_mode: CompositeAlphaMode::Opaque,
                view_formats: vec![],
                desired_maximum_frame_latency: 2,
            };
            (surface_format, surface_config)
        };

        // Store wgpu objects
        *self.adapter.write().await = Some(adapter);
        *self.device.write().await = Some(device);
        *self.queue.write().await = Some(queue);
        *self.surface.write().await = surface.take();
        *self.surface_config.write().await = Some(surface_config);

        // Initialize renderer with wgpu backend
        let device_ref = self.device.read().await;
        let device = device_ref.as_ref().unwrap();
        let queue_ref = self.queue.read().await;
        let queue = queue_ref.as_ref().unwrap();

        let renderer = W3DRenderer::new_with_wgpu(device, queue, &surface_format)
            .await
            .map_err(|e| {
                W3DError::RendererCreationFailed(format!("Failed to create renderer: {:?}", e))
            })?;
        *self.renderer.write().await = Some(renderer);

        // Initialize graphics context with wgpu state
        let context = GraphicsContext::new_with_wgpu(device, queue)
            .await
            .map_err(|e| {
                W3DError::ContextCreationFailed(format!("Failed to create context: {:?}", e))
            })?;
        *self.graphics_context.write().await = Some(context);

        drop(device_ref);
        drop(queue_ref);
        drop(config);

        // Load default shaders
        self.load_default_wgsl_shaders().await?;

        *self.initialized.write().await = true;

        tracing::info!("W3D device initialized successfully with wgpu backend");
        Ok(())
    }

    /// Initialize the W3D device with a window surface.
    ///
    /// Uses explicit unsafe surface creation to store a `'static` surface handle that
    /// mirrors legacy device lifetime assumptions.
    pub async fn init_with_window(&self, window: &Window) -> Result<()> {
        let surface_target = unsafe {
            SurfaceTargetUnsafe::from_window(window).map_err(|e| {
                W3DError::InitializationFailed(format!("Failed to create surface target: {e}"))
            })?
        };
        let surface = unsafe {
            self.instance
                .create_surface_unsafe(surface_target)
                .map_err(|e| {
                    W3DError::InitializationFailed(format!("Failed to create window surface: {e}"))
                })?
        };
        let size = window.inner_size();
        self.init_internal(Some(surface), Some((size.width, size.height)))
            .await
    }

    /// Load default WGSL shaders for modern rendering
    async fn load_default_wgsl_shaders(&self) -> Result<()> {
        // Load default PBR shader
        let default_shader = Shader {
            id: "w3d_default_pbr".to_string(),
            name: "W3D Default PBR Shader".to_string(),
            vertex_source: include_str!("../../shaders/w3d_default.wgsl").to_string(),
            fragment_source: include_str!("../../shaders/w3d_default.wgsl").to_string(),
            geometry_source: None,
            uniforms: vec![
                super::ShaderUniform {
                    name: "uniforms".to_string(),
                    uniform_type: super::ShaderUniformType::Mat4,
                    array_size: 1,
                },
                super::ShaderUniform {
                    name: "material".to_string(),
                    uniform_type: super::ShaderUniformType::Vec4,
                    array_size: 1,
                },
                super::ShaderUniform {
                    name: "lights".to_string(),
                    uniform_type: super::ShaderUniformType::Vec4,
                    array_size: 256,
                },
            ],
        };

        self.shaders
            .write()
            .await
            .insert(default_shader.id.clone(), default_shader);

        Ok(())
    }

    /// Compile shader to GPU resource
    async fn compile_shader_to_gpu(
        &self,
        shader: &Shader,
        device: &wgpu::Device,
    ) -> Result<W3DShaderGpu> {
        // Create shader modules
        let vertex_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{} Vertex", shader.name)),
            source: wgpu::ShaderSource::Wgsl(shader.vertex_source.as_str().into()),
        });

        let fragment_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(&format!("{} Fragment", shader.name)),
            source: wgpu::ShaderSource::Wgsl(shader.fragment_source.as_str().into()),
        });

        // Create bind group layouts
        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("W3D Uniform Bind Group Layout"),
                entries: &[
                    // Uniform buffer (matrices, camera, time, etc.)
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
                    // Material uniform buffer
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
                    // Light uniform buffer
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
                ],
            });

        let texture_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("W3D Texture Bind Group Layout"),
                entries: &[
                    // Diffuse texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Diffuse sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // Normal texture
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        },
                        count: None,
                    },
                    // Normal sampler
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let bind_group_layouts = vec![uniform_bind_group_layout, texture_bind_group_layout];

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some(&format!("{} Pipeline Layout", shader.name)),
            bind_group_layouts: &bind_group_layouts.iter().collect::<Vec<_>>(),
            push_constant_ranges: &[],
        });

        Ok(W3DShaderGpu {
            shader: shader.clone(),
            vertex_module,
            fragment_module,
            compute_module: None,
            bind_group_layouts,
            pipeline_layout,
        })
    }

    /// Set the current scene
    pub async fn set_scene(&self, scene: Scene) -> Result<()> {
        *self.current_scene.write().await = scene;
        Ok(())
    }

    /// Get the current scene
    pub async fn get_scene(&self) -> Scene {
        self.current_scene.read().await.clone()
    }

    /// Add a mesh to the device
    pub async fn add_mesh(&self, mesh: Mesh) -> Result<()> {
        let id = mesh.id.clone();
        self.meshes.write().await.insert(id, mesh);
        Ok(())
    }

    /// Add a material to the device
    pub async fn add_material(&self, material: Material) -> Result<()> {
        let id = material.id.clone();
        self.materials.write().await.insert(id, material);
        Ok(())
    }

    /// Add a texture to the device
    pub async fn add_texture(&self, texture: Texture) -> Result<()> {
        let id = texture.id.clone();

        // Update texture memory usage
        let texture_size = texture.data.len() as u64;
        let mut stats = self.statistics.write().await;
        stats.texture_memory_used += texture_size;

        self.textures.write().await.insert(id, texture);
        Ok(())
    }

    /// Add a shader to the device
    pub async fn add_shader(&self, shader: Shader) -> Result<()> {
        let id = shader.id.clone();
        self.shaders.write().await.insert(id, shader);
        Ok(())
    }

    /// Render the current scene
    pub async fn render_scene(&self) -> Result<()> {
        let mut renderer_guard = self.renderer.write().await;
        let renderer = renderer_guard
            .as_mut()
            .ok_or_else(|| W3DError::RenderingError("Renderer not initialized".to_string()))?;

        let scene = self.current_scene.read().await;

        // Begin frame
        renderer.begin_frame().await?;

        // Set camera
        renderer.set_camera(&scene.camera).await?;

        // Set lights
        for light in &scene.lights {
            renderer.add_light(light).await?;
        }

        // Render meshes
        let meshes_guard = self.meshes.read().await;
        let materials_guard = self.materials.read().await;

        let mut draw_calls = 0;
        let mut triangles = 0;
        let mut vertices = 0;

        for render_object in &scene.render_objects {
            if !render_object.visible {
                continue;
            }
            if let Some(mesh) = meshes_guard.get(&render_object.mesh_id) {
                // Get material
                let material = render_object
                    .material_id
                    .as_ref()
                    .and_then(|id| materials_guard.get(id));

                // Render mesh
                renderer
                    .render_mesh(
                        mesh,
                        material,
                        Some(render_object.transform),
                        Some(render_object.world_bounds.center()),
                        Some(render_object.transparent),
                    )
                    .await?;

                // Update statistics
                draw_calls += 1;
                triangles += mesh.indices.len() as u32 / 3;
                vertices += mesh.vertices.len() as u32;
            }
        }

        // End frame
        renderer.end_frame().await?;

        // Update statistics
        let mut stats = self.statistics.write().await;
        stats.draw_calls = draw_calls;
        stats.triangles = triangles;
        stats.vertices = vertices;
        stats.active_lights = scene.lights.len() as u32;

        Ok(())
    }

    /// Get device statistics
    pub async fn get_statistics(&self) -> W3DStatistics {
        self.statistics.read().await.clone()
    }

    /// Update statistics (called by render loop)
    pub async fn update_statistics(&self, frame_time: f32) {
        let mut stats = self.statistics.write().await;
        stats.fps = 1000.0 / frame_time.max(0.001);
        stats.frame_time_ms = frame_time;
    }

    /// Get device status
    pub async fn get_status(&self) -> Result<DeviceStatus> {
        let initialized = *self.initialized.read().await;
        let stats = self.get_statistics().await;

        Ok(DeviceStatus {
            device_type: DeviceType::W3D,
            initialized,
            active: initialized && stats.fps > 0.0,
            capabilities: crate::DeviceCapabilities {
                hardware_acceleration: true,
                multi_threading: true,
                simd_support: true,
                platform_features: vec![
                    "3D Rendering".to_string(),
                    "Shader Support".to_string(),
                    "Hardware T&L".to_string(),
                    "Texture Compression".to_string(),
                ],
            },
            performance: PerformanceMetrics {
                cpu_usage: 0.0,
                memory_usage: stats.texture_memory_used + stats.buffer_memory_used,
                latency_ms: stats.frame_time_ms,
                throughput: stats.fps,
            },
        })
    }

    /// Get performance metrics
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        let stats = self.get_statistics().await;

        Ok(PerformanceMetrics {
            cpu_usage: 0.0,
            memory_usage: stats.texture_memory_used + stats.buffer_memory_used,
            latency_ms: stats.frame_time_ms,
            throughput: stats.fps,
        })
    }

    /// Shutdown the W3D device
    pub async fn shutdown(&self) -> Result<()> {
        *self.initialized.write().await = false;

        // Clear resources
        self.meshes.write().await.clear();
        self.materials.write().await.clear();
        self.textures.write().await.clear();
        self.shaders.write().await.clear();

        // Shutdown renderer and context
        if let Some(renderer) = self.renderer.write().await.take() {
            renderer.shutdown().await?;
        }

        if let Some(context) = self.graphics_context.write().await.take() {
            context.shutdown().await?;
        }

        tracing::info!("W3D device shutdown completed");
        Ok(())
    }

    /// Get mesh by ID
    pub async fn get_mesh(&self, id: &str) -> Option<Mesh> {
        self.meshes.read().await.get(id).cloned()
    }

    /// Get material by ID
    pub async fn get_material(&self, id: &str) -> Option<Material> {
        self.materials.read().await.get(id).cloned()
    }

    /// Get texture by ID
    pub async fn get_texture(&self, id: &str) -> Option<Texture> {
        self.textures.read().await.get(id).cloned()
    }

    /// Get shader by ID
    pub async fn get_shader(&self, id: &str) -> Option<Shader> {
        self.shaders.read().await.get(id).cloned()
    }

    /// Remove mesh
    pub async fn remove_mesh(&self, id: &str) -> Result<()> {
        self.meshes.write().await.remove(id);
        Ok(())
    }

    /// Remove material
    pub async fn remove_material(&self, id: &str) -> Result<()> {
        self.materials.write().await.remove(id);
        Ok(())
    }

    /// Remove texture
    pub async fn remove_texture(&self, id: &str) -> Result<()> {
        if let Some(texture) = self.textures.write().await.remove(id) {
            // Update texture memory usage
            let texture_size = texture.data.len() as u64;
            let mut stats = self.statistics.write().await;
            stats.texture_memory_used = stats.texture_memory_used.saturating_sub(texture_size);
        }
        Ok(())
    }

    /// Remove shader
    pub async fn remove_shader(&self, id: &str) -> Result<()> {
        self.shaders.write().await.remove(id);
        Ok(())
    }
}

fn select_surface_format(formats: &[wgpu::TextureFormat]) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(wgpu::TextureFormat::is_srgb)
        .unwrap_or(wgpu::TextureFormat::Bgra8UnormSrgb)
}

fn select_present_mode(modes: &[PresentMode], vsync: bool) -> PresentMode {
    if vsync {
        modes
            .iter()
            .copied()
            .find(|mode| *mode == PresentMode::Fifo)
            .unwrap_or(PresentMode::Fifo)
    } else {
        modes
            .iter()
            .copied()
            .find(|mode| *mode == PresentMode::Immediate)
            .or_else(|| {
                modes
                    .iter()
                    .copied()
                    .find(|mode| *mode == PresentMode::Mailbox)
            })
            .unwrap_or(PresentMode::Fifo)
    }
}

fn select_alpha_mode(modes: &[CompositeAlphaMode]) -> CompositeAlphaMode {
    modes
        .iter()
        .copied()
        .find(|mode| *mode == CompositeAlphaMode::Opaque)
        .unwrap_or(CompositeAlphaMode::Opaque)
}

impl Clone for W3DDevice {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            instance: self.instance.clone(),
            adapter: self.adapter.clone(),
            device: self.device.clone(),
            queue: self.queue.clone(),
            surface: self.surface.clone(),
            surface_config: self.surface_config.clone(),
            renderer: self.renderer.clone(),
            graphics_context: self.graphics_context.clone(),
            statistics: self.statistics.clone(),
            meshes: self.meshes.clone(),
            materials: self.materials.clone(),
            textures: self.textures.clone(),
            shaders: self.shaders.clone(),
            vertex_buffer_pool: self.vertex_buffer_pool.clone(),
            index_buffer_pool: self.index_buffer_pool.clone(),
            uniform_buffer_pool: self.uniform_buffer_pool.clone(),
            current_scene: self.current_scene.clone(),
            frame_counter: self.frame_counter.clone(),
            command_encoder: self.command_encoder.clone(),
            current_render_pass: self.current_render_pass.clone(),
            initialized: self.initialized.clone(),
            shutting_down: self.shutting_down.clone(),
        }
    }
}

impl Drop for W3DDevice {
    fn drop(&mut self) {
        tracing::debug!("W3D device dropped");
    }
}
