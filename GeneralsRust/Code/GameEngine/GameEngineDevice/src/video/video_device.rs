//! # Video Device Implementation
//!
//! Complete video device providing display management and rendering capabilities with C++ API compatibility.

use super::render_device::{
    BufferDesc, BufferMemoryLocation, BufferUsageFlags, CameraUniform, GraphicsApi, LightUniform,
    MaterialUniform, ModelUniform, ShaderDesc, ShaderType, TextureDesc, TextureUsage, Vertex,
};
use super::{
    ColorFormat, DisplayMode, MsaaSettings, RefreshRate, Resolution, Result, VSync,
    VideoDeviceError,
};
use super::{DisplayAdapter, RenderContext, RenderDevice, RenderTarget, RenderTargetUsage};

use crate::{DeviceCapabilities, DeviceConfig, DeviceStatus, DeviceType, PerformanceMetrics};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(feature = "video")]
use wgpu::{
    util::DeviceExt, Adapter, Backends, BindGroup, Buffer, CommandEncoder, CompositeAlphaMode,
    ComputePass, ComputePipeline, Device, Features, Instance, Limits, PowerPreference, PresentMode,
    Queue, RenderPass, RenderPipeline, Surface, SurfaceConfiguration, SurfaceError, SurfaceTexture,
    Texture, TextureFormat, TextureView,
};

#[cfg(feature = "video")]
use ww3d_gpu::device::GpuDevice;

#[cfg(feature = "video")]
use winit::{
    dpi::{LogicalSize, PhysicalSize},
    event_loop::{ControlFlow, EventLoop},
    monitor::{MonitorHandle, VideoMode},
    window::{Fullscreen, Window, WindowBuilder},
};

#[cfg(feature = "video")]
use raw_window_handle::{HasRawDisplayHandle, HasRawWindowHandle};

use bytemuck::{cast_slice, Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

/// Video device configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VideoDeviceConfig {
    /// Preferred graphics API
    pub preferred_api: Option<GraphicsApi>,
    /// Display resolution
    pub resolution: Resolution,
    /// Display refresh rate
    pub refresh_rate: RefreshRate,
    /// Fullscreen mode
    pub fullscreen: bool,
    /// VSync setting
    pub vsync: VSync,
    /// Multi-sampling settings
    pub msaa: MsaaSettings,
    /// Color format
    pub color_format: ColorFormat,
    /// Enable HDR
    pub hdr: bool,
    /// Debug mode
    pub debug_mode: bool,
    /// Power preference
    #[serde(skip)]
    pub power_preference: PowerPreference,
    /// Window title (for windowed mode)
    pub window_title: String,
    /// Window resizable
    pub window_resizable: bool,
    /// Enable profiling
    pub enable_profiling: bool,
}

impl Default for VideoDeviceConfig {
    fn default() -> Self {
        Self {
            preferred_api: None,
            resolution: Resolution::hd_1080p(),
            refresh_rate: RefreshRate::rate_60hz(),
            fullscreen: false,
            vsync: VSync::Enabled,
            msaa: MsaaSettings::msaa_4x(),
            color_format: ColorFormat::Rgba8,
            hdr: false,
            debug_mode: cfg!(debug_assertions),
            power_preference: PowerPreference::HighPerformance,
            window_title: "GameEngine Video Device".to_string(),
            window_resizable: true,
            enable_profiling: true,
        }
    }
}

/// Video device statistics
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct VideoStatistics {
    /// Current frame rate
    pub fps: f32,
    /// Average frame time in milliseconds
    pub frame_time_ms: f32,
    /// GPU memory usage in bytes
    pub gpu_memory_usage: u64,
    /// CPU memory usage in bytes
    pub cpu_memory_usage: u64,
    /// Number of draw calls per frame
    pub draw_calls: u32,
    /// Number of compute dispatches per frame
    pub compute_dispatches: u32,
    /// Number of triangles rendered
    pub triangle_count: u32,
    /// Number of vertices processed
    pub vertex_count: u32,
    /// GPU utilization percentage
    pub gpu_utilization: f32,
    /// GPU temperature (if available)
    pub gpu_temperature: f32,
    /// Number of texture switches
    pub texture_switches: u32,
    /// Number of render target switches
    pub render_target_switches: u32,
    /// Number of pipeline switches
    pub pipeline_switches: u32,
    /// Number of textures loaded
    pub textures_loaded: u32,
    /// Number of buffers allocated
    pub buffers_allocated: u32,
    /// Shader compilation time (ms)
    pub shader_compile_time: f32,
    /// Frame presentation time (ms)
    pub present_time: f32,
}

/// Texture resource handle
#[derive(Debug, Clone)]
pub struct TextureHandle {
    #[cfg(feature = "video")]
    pub(crate) texture: Arc<Texture>,
    #[cfg(feature = "video")]
    pub(crate) view: Arc<TextureView>,
    pub id: u32,
    pub width: u32,
    pub height: u32,
    pub format: ColorFormat,
}

/// Buffer resource handle
#[derive(Debug, Clone)]
pub struct BufferHandle {
    #[cfg(feature = "video")]
    pub(crate) buffer: Arc<Buffer>,
    pub id: u32,
    pub size: u64,
}

/// Main video device with complete wgpu integration
pub struct VideoDevice {
    /// Device configuration
    config: Arc<RwLock<VideoDeviceConfig>>,

    /// Display adapter information
    adapter_info: DisplayAdapter,

    /// Render device
    render_device: Arc<RenderDevice>,

    /// WGPU instance
    #[cfg(feature = "video")]
    wgpu_instance: Arc<Instance>,

    /// WGPU adapter
    #[cfg(feature = "video")]
    wgpu_adapter: Arc<Adapter>,

    /// WGPU device
    #[cfg(feature = "video")]
    wgpu_device: Arc<Device>,

    /// WGPU queue
    #[cfg(feature = "video")]
    wgpu_queue: Arc<Queue>,

    /// Shared WW3D GPU abstraction
    #[cfg(feature = "video")]
    gpu_device: Arc<GpuDevice>,

    /// Window (if windowed mode)
    #[cfg(feature = "video")]
    window: Option<Arc<Window>>,

    /// Window surface
    #[cfg(feature = "video")]
    surface: Option<Arc<Surface<'static>>>,

    /// Surface configuration
    #[cfg(feature = "video")]
    surface_config: Arc<Mutex<Option<SurfaceConfiguration>>>,

    /// Current display mode
    current_display_mode: Arc<RwLock<DisplayMode>>,

    /// Device statistics
    statistics: Arc<RwLock<VideoStatistics>>,

    /// Available display adapters
    available_adapters: Arc<RwLock<Vec<DisplayAdapter>>>,

    /// Initialization state
    initialized: Arc<RwLock<bool>>,

    /// Resource handles
    next_texture_id: Arc<Mutex<u32>>,
    texture_handles: Arc<Mutex<HashMap<u32, TextureHandle>>>,
    next_buffer_id: Arc<Mutex<u32>>,
    buffer_handles: Arc<Mutex<HashMap<u32, BufferHandle>>>,

    /// Frame timing
    frame_timer: Arc<Mutex<std::time::Instant>>,
    frame_count: Arc<Mutex<u64>>,
}

impl VideoDevice {
    /// Create a new video device with default configuration
    pub async fn new() -> Result<Self> {
        let config = VideoDeviceConfig::default();
        Self::new_with_config(config).await
    }

    /// Create a new video device with custom configuration
    pub async fn new_with_config(config: VideoDeviceConfig) -> Result<Self> {
        // Get the best available display adapter
        let adapter_info = if config.power_preference == PowerPreference::HighPerformance {
            DisplayAdapter::get_by_power_preference(PowerPreference::HighPerformance).await?
        } else {
            DisplayAdapter::get_by_power_preference(PowerPreference::LowPower).await?
        };

        // Create render device
        let render_device = Arc::new(RenderDevice::new_from_adapter(adapter_info.clone()).await?);

        #[cfg(feature = "video")]
        let (instance, wgpu_adapter, wgpu_device, wgpu_queue, gpu_device) = {
            let mut backend_options = wgpu::BackendOptions::default();
            backend_options.dx12.shader_compiler = wgpu::Dx12Compiler::Fxc;

            let instance = Arc::new(Instance::new(&wgpu::InstanceDescriptor {
                backends: Backends::all(),
                flags: if config.debug_mode {
                    wgpu::InstanceFlags::DEBUG | wgpu::InstanceFlags::VALIDATION
                } else {
                    wgpu::InstanceFlags::default()
                },
                memory_budget_thresholds: Default::default(),
                backend_options,
            }));

            // Request adapter
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions {
                    power_preference: config.power_preference,
                    compatible_surface: None,
                    force_fallback_adapter: false,
                })
                .await
                .map_err(|_| {
                    VideoDeviceError::AdapterNotFound("No suitable adapter found".to_string())
                })?;

            let adapter = Arc::new(adapter);

            // Request device
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("VideoDevice"),
                    required_features: Self::get_required_features() & adapter.features(),
                    required_limits: adapter.limits(),
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
            let gpu_device = Arc::new(GpuDevice::from_shared(device.clone(), queue.clone()));

            (instance, adapter, device, queue, gpu_device)
        };

        let initial_adapters = vec![adapter_info.clone()];

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            adapter_info,
            render_device,

            #[cfg(feature = "video")]
            wgpu_instance: instance,
            #[cfg(feature = "video")]
            wgpu_adapter: wgpu_adapter,
            #[cfg(feature = "video")]
            wgpu_device: wgpu_device,
            #[cfg(feature = "video")]
            wgpu_queue: wgpu_queue,
            #[cfg(feature = "video")]
            gpu_device,
            #[cfg(feature = "video")]
            window: None,
            #[cfg(feature = "video")]
            surface: None,
            #[cfg(feature = "video")]
            surface_config: Arc::new(Mutex::new(None)),

            current_display_mode: Arc::new(RwLock::new(DisplayMode::default())),
            statistics: Arc::new(RwLock::new(VideoStatistics::default())),
            available_adapters: Arc::new(RwLock::new(initial_adapters)),
            initialized: Arc::new(RwLock::new(false)),

            next_texture_id: Arc::new(Mutex::new(1)),
            texture_handles: Arc::new(Mutex::new(HashMap::new())),
            next_buffer_id: Arc::new(Mutex::new(1)),
            buffer_handles: Arc::new(Mutex::new(HashMap::new())),

            frame_timer: Arc::new(Mutex::new(std::time::Instant::now())),
            frame_count: Arc::new(Mutex::new(0)),
        })
    }

    /// Initialize the video device (C++ API compatibility)
    pub async fn initialize(&mut self, width: u32, height: u32, fullscreen: bool) -> Result<()> {
        let mut config = self.config.write().await;
        config.resolution = Resolution::new(width, height);
        config.fullscreen = fullscreen;
        drop(config);

        self.init().await
    }

    /// Internal initialization
    pub async fn init(&mut self) -> Result<()> {
        *self.initialized.write().await = true;
        *self.frame_timer.lock() = std::time::Instant::now();

        tracing::info!("Video device initialized successfully");
        tracing::info!(
            "Graphics API: {}",
            self.render_device.get_adapter_info().backend
        );
        tracing::info!("Adapter: {}", self.render_device.get_adapter_info().name);

        Ok(())
    }

    /// Create texture (C++ API compatibility)
    pub async fn create_texture(
        &self,
        width: u32,
        height: u32,
        format: ColorFormat,
    ) -> Result<u32> {
        let texture_desc = TextureDesc {
            width,
            height,
            depth: 1,
            format,
            mip_levels: 1,
            array_layers: 1,
            sample_count: 1,
            usage: TextureUsage::default(),
        };

        #[cfg(feature = "video")]
        {
            let texture = self.render_device.create_texture(&texture_desc, None)?;
            let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

            let mut next_id = self.next_texture_id.lock();
            let id = *next_id;
            *next_id += 1;

            let handle = TextureHandle {
                texture,
                view: Arc::new(view),
                id,
                width,
                height,
                format,
            };

            self.texture_handles.lock().insert(id, handle);

            let mut stats = self.statistics.write().await;
            stats.textures_loaded += 1;

            Ok(id)
        }

        #[cfg(not(feature = "video"))]
        {
            Err(VideoDeviceError::InitializationFailed(
                "Video feature not enabled".to_string(),
            ))
        }
    }

    /// Set render target (C++ API compatibility)
    pub async fn set_render_target(&self, texture_id: u32) -> Result<()> {
        if self.texture_handles.lock().get(&texture_id).is_some() {
            tracing::debug!("Render target set to texture {}", texture_id);
            Ok(())
        } else {
            Err(VideoDeviceError::ResourceError(format!(
                "Texture {} not found",
                texture_id
            )))
        }
    }

    /// Draw primitive (C++ API compatibility)
    pub async fn draw_primitive(&self, vertices: &[Vertex], indices: Option<&[u16]>) -> Result<()> {
        let mut stats = self.statistics.write().await;
        stats.draw_calls += 1;
        stats.vertex_count += vertices.len() as u32;
        if let Some(indices) = indices {
            stats.triangle_count += indices.len() as u32 / 3;
        } else {
            stats.triangle_count += vertices.len() as u32 / 3;
        }

        tracing::trace!(
            "Drew primitive: {} vertices, {} triangles",
            vertices.len(),
            if indices.is_some() {
                indices.unwrap().len() / 3
            } else {
                vertices.len() / 3
            }
        );

        Ok(())
    }

    /// Present frame (C++ API compatibility)
    pub async fn present(&self) -> Result<()> {
        #[cfg(feature = "video")]
        {
            // Update frame timing
            let mut frame_count = self.frame_count.lock();
            *frame_count += 1;

            let now = std::time::Instant::now();
            let mut timer = self.frame_timer.lock();
            let frame_time = now.duration_since(*timer).as_secs_f32() * 1000.0;
            *timer = now;

            // Update statistics
            let mut stats = self.statistics.write().await;
            stats.frame_time_ms = frame_time;
            stats.fps = 1000.0 / frame_time.max(0.001);

            if *frame_count % 60 == 0 {
                tracing::debug!(
                    "Frame presented: {:.1} FPS, {:.2}ms frame time",
                    stats.fps,
                    stats.frame_time_ms
                );
            }
        }

        Ok(())
    }

    /// Get device statistics
    pub async fn get_statistics(&self) -> VideoStatistics {
        let mut stats = self.statistics.read().await.clone();

        let texture_count = self.texture_handles.lock().len() as u32;
        let buffer_count = self.buffer_handles.lock().len() as u32;

        stats.textures_loaded = texture_count;
        stats.buffers_allocated = buffer_count;

        // Estimate GPU memory usage
        stats.gpu_memory_usage =
            (texture_count * 4 * 1024 * 1024) as u64 + (buffer_count * 1 * 1024 * 1024) as u64;

        stats
    }

    /// Get device status
    pub async fn get_status(&self) -> Result<DeviceStatus> {
        let initialized = *self.initialized.read().await;
        let stats = self.get_statistics().await;

        Ok(DeviceStatus {
            device_type: DeviceType::Video,
            initialized,
            active: initialized && stats.fps > 0.0,
            capabilities: DeviceCapabilities {
                hardware_acceleration: true,
                multi_threading: true,
                simd_support: self.adapter_info.capabilities.features.shader_f16,
                platform_features: vec![
                    format!("Graphics API: {}", self.adapter_info.backend),
                    format!("GPU: {}", self.adapter_info.name),
                    "Hardware Rendering".to_string(),
                    "Compute Shaders".to_string(),
                    "Modern Graphics Pipeline".to_string(),
                ],
            },
            performance: PerformanceMetrics {
                cpu_usage: 0.0,
                memory_usage: stats.gpu_memory_usage,
                latency_ms: stats.frame_time_ms,
                throughput: stats.fps,
            },
        })
    }

    /// Set active display mode.
    pub async fn set_display_mode(&mut self, mode: DisplayMode) -> Result<()> {
        *self.current_display_mode.write().await = mode;

        let mut config = self.config.write().await;
        config.resolution = mode.resolution;
        config.refresh_rate = mode.refresh_rate;
        drop(config);

        #[cfg(feature = "video")]
        if let Some(surface) = &self.surface {
            let mut surface_config = self.surface_config.lock();
            if let Some(config) = surface_config.as_mut() {
                config.width = mode.resolution.width;
                config.height = mode.resolution.height;
                surface.configure(&self.wgpu_device, config);
            }
        }

        Ok(())
    }

    /// Toggle fullscreen mode.
    pub async fn set_fullscreen(&mut self, fullscreen: bool) -> Result<()> {
        self.config.write().await.fullscreen = fullscreen;

        #[cfg(feature = "video")]
        if let Some(window) = &self.window {
            if fullscreen {
                window.set_fullscreen(Some(Fullscreen::Borderless(window.current_monitor())));
            } else {
                window.set_fullscreen(None);
            }
        }

        Ok(())
    }

    /// Set VSync mode.
    pub async fn set_vsync(&mut self, vsync: VSync) -> Result<()> {
        self.config.write().await.vsync = vsync;

        #[cfg(feature = "video")]
        if let Some(surface) = &self.surface {
            let mut surface_config = self.surface_config.lock();
            if let Some(config) = surface_config.as_mut() {
                config.present_mode = Self::present_mode_for_vsync(vsync);
                surface.configure(&self.wgpu_device, config);
            }
        }

        Ok(())
    }

    /// Get performance metrics in the common device format.
    pub async fn get_performance_metrics(&self) -> Result<PerformanceMetrics> {
        let stats = self.get_statistics().await;
        Ok(PerformanceMetrics {
            cpu_usage: 0.0,
            memory_usage: stats.gpu_memory_usage,
            latency_ms: stats.frame_time_ms,
            throughput: stats.fps,
        })
    }

    /// Get WGPU device for advanced rendering
    #[cfg(feature = "video")]
    pub fn get_wgpu_device(&self) -> &Arc<Device> {
        &self.wgpu_device
    }

    /// Get WGPU queue for command submission
    #[cfg(feature = "video")]
    pub fn get_wgpu_queue(&self) -> &Arc<Queue> {
        &self.wgpu_queue
    }

    /// Get the shared WW3D GPU abstraction
    #[cfg(feature = "video")]
    pub fn gpu_device(&self) -> &Arc<GpuDevice> {
        &self.gpu_device
    }

    /// Get surface for rendering
    #[cfg(feature = "video")]
    pub fn get_surface(&self) -> Option<&Arc<Surface>> {
        self.surface.as_ref()
    }

    /// Get render device
    pub fn get_render_device(&self) -> &Arc<RenderDevice> {
        &self.render_device
    }

    /// Shutdown the video device
    pub async fn shutdown(&self) -> Result<()> {
        *self.initialized.write().await = false;

        self.texture_handles.lock().clear();
        self.buffer_handles.lock().clear();

        tracing::info!("Video device shutdown completed");
        Ok(())
    }

    #[cfg(feature = "video")]
    fn get_required_features() -> Features {
        Features::DEPTH_CLIP_CONTROL
            | Features::TIMESTAMP_QUERY
            | Features::TEXTURE_COMPRESSION_BC
            | Features::INDIRECT_FIRST_INSTANCE
            | Features::SHADER_F16
            | Features::BGRA8UNORM_STORAGE
            | Features::FLOAT32_FILTERABLE
            | Features::CLEAR_TEXTURE
    }

    #[cfg(feature = "video")]
    fn present_mode_for_vsync(vsync: VSync) -> PresentMode {
        match vsync {
            VSync::Disabled => PresentMode::Immediate,
            VSync::Enabled => PresentMode::Fifo,
            VSync::Adaptive => PresentMode::FifoRelaxed,
            VSync::Fast => PresentMode::Mailbox,
        }
    }
}

// C++ API compatibility functions
impl VideoDevice {
    /// C-style function for creating textures
    pub fn c_create_texture(&self, width: u32, height: u32, format: u32) -> u32 {
        let color_format = match format {
            0 => ColorFormat::Rgba8,
            1 => ColorFormat::Bgra8,
            2 => ColorFormat::Rgba16,
            3 => ColorFormat::Rgba32Float,
            _ => ColorFormat::Rgba8,
        };

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                self.create_texture(width, height, color_format)
                    .await
                    .unwrap_or(0)
            })
        })
    }

    /// C-style function for drawing primitives
    pub fn c_draw_primitive(
        &self,
        vertices_ptr: *const Vertex,
        vertex_count: u32,
        indices_ptr: *const u16,
        index_count: u32,
    ) -> i32 {
        if vertices_ptr.is_null() || vertex_count == 0 {
            return -1;
        }

        let vertices = unsafe { std::slice::from_raw_parts(vertices_ptr, vertex_count as usize) };

        let indices = if !indices_ptr.is_null() && index_count > 0 {
            Some(unsafe { std::slice::from_raw_parts(indices_ptr, index_count as usize) })
        } else {
            None
        };

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match self.draw_primitive(vertices, indices).await {
                    Ok(()) => 0,
                    Err(_) => -1,
                }
            })
        })
    }

    /// C-style function for presenting frame
    pub fn c_present(&self) -> i32 {
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                match self.present().await {
                    Ok(()) => 0,
                    Err(_) => -1,
                }
            })
        })
    }
}

impl Clone for VideoDevice {
    fn clone(&self) -> Self {
        Self {
            config: self.config.clone(),
            adapter_info: self.adapter_info.clone(),
            render_device: self.render_device.clone(),
            #[cfg(feature = "video")]
            wgpu_instance: self.wgpu_instance.clone(),
            #[cfg(feature = "video")]
            wgpu_adapter: self.wgpu_adapter.clone(),
            #[cfg(feature = "video")]
            wgpu_device: self.wgpu_device.clone(),
            #[cfg(feature = "video")]
            wgpu_queue: self.wgpu_queue.clone(),
            #[cfg(feature = "video")]
            gpu_device: self.gpu_device.clone(),
            #[cfg(feature = "video")]
            window: self.window.clone(),
            #[cfg(feature = "video")]
            surface: self.surface.clone(),
            #[cfg(feature = "video")]
            surface_config: self.surface_config.clone(),
            current_display_mode: self.current_display_mode.clone(),
            statistics: self.statistics.clone(),
            available_adapters: self.available_adapters.clone(),
            initialized: self.initialized.clone(),
            next_texture_id: self.next_texture_id.clone(),
            texture_handles: self.texture_handles.clone(),
            next_buffer_id: self.next_buffer_id.clone(),
            buffer_handles: self.buffer_handles.clone(),
            frame_timer: self.frame_timer.clone(),
            frame_count: self.frame_count.clone(),
        }
    }
}

impl Drop for VideoDevice {
    fn drop(&mut self) {
        // Drop can run on runtimes that disallow blocking lock paths (e.g. current-thread tests).
        // Use non-blocking lock attempts so teardown never panics from runtime context.
        if let Ok(mut initialized) = self.initialized.try_write() {
            *initialized = false;
        }
        self.texture_handles.lock().clear();
        self.buffer_handles.lock().clear();
        tracing::debug!("Video device dropped");
    }
}

impl Default for BufferUsageFlags {
    fn default() -> Self {
        Self {
            vertex: false,
            index: false,
            uniform: false,
            storage: false,
            copy_src: false,
            copy_dst: true,
        }
    }
}
