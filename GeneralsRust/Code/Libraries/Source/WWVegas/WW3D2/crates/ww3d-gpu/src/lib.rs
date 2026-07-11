//! WW3D GPU Abstraction Layer
//!
//! This crate provides a cross-platform graphics API abstraction layer,
//! matching the original C++ WW3D DirectX8 wrapper functionality while
//! supporting modern graphics APIs through WGPU.
//!
//! ## Features
//!
//! - Cross-platform GPU abstraction (DirectX, Vulkan, Metal)
//! - DirectX8 API compatibility layer (FVF, blend modes, shader constants)
//! - Modern shader pipeline support
//! - Resource management and caching
//! - Performance monitoring and profiling
//! - Multi-threading support
//! - Dynamic buffer ring system for streaming data
//! - Sorting renderer for transparent objects
//! - Render-to-texture with depth buffer support
//! - Shadow mapping
//! - Pipeline caching for optimal performance

pub mod adapter;
pub mod blend_modes;
pub mod buffer;
pub mod caps;
pub mod command;
pub mod device;
pub mod dynamic_buffer;
pub mod fvf;
pub mod pipeline;
pub mod pipeline_cache;
pub mod render_target;
pub mod shader;
pub mod shader_constants;
pub mod shader_presets;
pub mod sorting_renderer;
pub mod surface;
pub mod sync;
pub mod tessellation;
pub mod texture;

pub use adapter::*;
pub use blend_modes::*;
pub use buffer::{BufferManager, BufferStats, GpuBuffer};
pub use caps::*;
pub use command::*;
pub use device::MemoryType;
pub use device::{DeviceBuffer, GpuDevice, MemoryStats};
pub use dynamic_buffer::*;
pub use fvf::*;
pub use pipeline::*;
pub use pipeline_cache::*;
pub use render_target::*;
// Note: shader::Shader and ww3d_core::Shader are different types
// (fixed-function state vs WGPU shader module)
#[allow(ambiguous_glob_reexports)]
pub use shader::*;
pub use shader_constants::*;
pub use shader_presets::ShaderPresets;
pub use sorting_renderer::*;
pub use surface::*;
pub use sync::*;
pub use tessellation::*;
// Note: texture::TextureManager and ww3d_core::TextureManager are different types
#[allow(ambiguous_glob_reexports)]
pub use texture::*;
pub use DeviceBuffer as Buffer;

// Re-export wgpu types for convenience
pub use wgpu;

/// Present a surface texture to the active swapchain.
#[inline]
pub fn present_surface_texture(frame: wgpu::SurfaceTexture) {
    frame.present();
}

// Re-export common types
#[allow(ambiguous_glob_reexports)]
pub use ww3d_core::*;
// DX8 compatibility modules removed; WGPU is the sole path

/// GPU feature flags
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuFeature {
    /// Instancing support
    Instancing,
    /// Geometry shaders
    GeometryShaders,
    /// Tessellation shaders
    TessellationShaders,
    /// Compute shaders
    ComputeShaders,
    /// Multi-sample anti-aliasing
    Msaa,
    /// Anisotropic filtering
    AnisotropicFiltering,
    /// Multi-threading
    MultiThreading,
    /// Texture operation: ADD (add texture RGB to diffuse RGB)
    /// C++ Reference: D3DTEXOPCAPS_ADD
    TexOpAdd,
    /// Texture operation: MODULATE2X (modulate and multiply by 2)
    /// C++ Reference: D3DTEXOPCAPS_MODULATE2X
    TexOpModulate2X,
    /// Texture operation: BUMPENVMAP (environment-mapped bump mapping)
    /// C++ Reference: D3DTEXOPCAPS_BUMPENVMAP
    TexOpBumpEnvMap,
    /// Texture operation: BUMPENVMAPLUMINANCE (bump mapping with luminance)
    /// C++ Reference: D3DTEXOPCAPS_BUMPENVMAPLUMINANCE
    TexOpBumpEnvMapLuminance,
}

/// GPU limits and capabilities
#[derive(Debug, Clone)]
pub struct GpuLimits {
    pub max_texture_size: u32,
    pub max_texture_array_layers: u32,
    pub max_bind_groups: u32,
    pub max_bindings_per_bind_group: u32,
    pub max_dynamic_uniform_buffers_per_pipeline_layout: u32,
    pub max_dynamic_storage_buffers_per_pipeline_layout: u32,
    pub max_sampled_textures_per_shader_stage: u32,
    pub max_samplers_per_shader_stage: u32,
    pub max_storage_buffers_per_shader_stage: u32,
    pub max_storage_textures_per_shader_stage: u32,
    pub max_uniform_buffers_per_shader_stage: u32,
    pub max_uniform_buffer_binding_size: u64,
    pub max_storage_buffer_binding_size: u64,
    pub max_vertex_buffers: u32,
    pub max_buffer_size: u64,
    pub max_vertex_attributes: u32,
    pub max_vertex_buffer_array_stride: u32,
    pub max_push_constant_size: u32,
    pub max_inter_stage_shader_components: u32,
    pub max_compute_workgroup_storage_size: u32,
    pub max_compute_invocations_per_workgroup: u32,
    pub max_compute_workgroup_size_x: u32,
    pub max_compute_workgroup_size_y: u32,
    pub max_compute_workgroup_size_z: u32,
    pub max_compute_workgroups_per_dimension: u32,
}

impl From<wgpu::Limits> for GpuLimits {
    fn from(limits: wgpu::Limits) -> Self {
        Self {
            max_texture_size: limits.max_texture_dimension_2d,
            max_texture_array_layers: limits.max_texture_array_layers,
            max_bind_groups: limits.max_bind_groups,
            max_bindings_per_bind_group: limits.max_bindings_per_bind_group,
            max_dynamic_uniform_buffers_per_pipeline_layout: limits
                .max_dynamic_uniform_buffers_per_pipeline_layout,
            max_dynamic_storage_buffers_per_pipeline_layout: limits
                .max_dynamic_storage_buffers_per_pipeline_layout,
            max_sampled_textures_per_shader_stage: limits.max_sampled_textures_per_shader_stage,
            max_samplers_per_shader_stage: limits.max_samplers_per_shader_stage,
            max_storage_buffers_per_shader_stage: limits.max_storage_buffers_per_shader_stage,
            max_storage_textures_per_shader_stage: limits.max_storage_textures_per_shader_stage,
            max_uniform_buffers_per_shader_stage: limits.max_uniform_buffers_per_shader_stage,
            max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size as u64,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size as u64,
            max_vertex_buffers: limits.max_vertex_buffers,
            max_buffer_size: limits.max_buffer_size,
            max_vertex_attributes: limits.max_vertex_attributes,
            max_vertex_buffer_array_stride: limits.max_vertex_buffer_array_stride,
            max_push_constant_size: limits.max_push_constant_size,
            max_inter_stage_shader_components: limits.max_inter_stage_shader_components,
            max_compute_workgroup_storage_size: limits.max_compute_workgroup_storage_size,
            max_compute_invocations_per_workgroup: limits.max_compute_invocations_per_workgroup,
            max_compute_workgroup_size_x: limits.max_compute_workgroup_size_x,
            max_compute_workgroup_size_y: limits.max_compute_workgroup_size_y,
            max_compute_workgroup_size_z: limits.max_compute_workgroup_size_z,
            max_compute_workgroups_per_dimension: limits.max_compute_workgroups_per_dimension,
        }
    }
}

/// GPU memory statistics
#[derive(Debug, Clone, Default)]
pub struct GpuMemoryStats {
    pub total_memory: u64,
    pub used_memory: u64,
    pub buffer_count: usize,
    pub texture_count: usize,
    pub shader_count: usize,
    pub pipeline_count: usize,
}

/// GPU performance statistics
#[derive(Debug, Clone, Default)]
pub struct GpuPerformanceStats {
    pub frame_time_ms: f32,
    pub fps: f32,
    pub draw_calls: u32,
    pub triangles_rendered: u32,
    pub texture_switches: u32,
    pub shader_switches: u32,
    pub buffer_uploads: u32,
}

/// Main GPU context structure
#[derive(Debug)]
pub struct GpuContext {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub adapter: wgpu::Adapter,
    pub instance: wgpu::Instance,
    pub surface: Option<wgpu::Surface<'static>>,
    pub surface_config: Option<wgpu::SurfaceConfiguration>,
    pub limits: GpuLimits,
    pub features: Vec<GpuFeature>,
    pub memory_stats: GpuMemoryStats,
    pub performance_stats: GpuPerformanceStats,
}

impl GpuContext {
    /// Create a new GPU context
    pub async fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                label: Some("WW3D GPU Device"),
                ..Default::default()
            })
            .await?;

        let limits = GpuLimits::from(adapter.limits());
        let features = Self::detect_features(&adapter);

        Ok(Self {
            device,
            queue,
            adapter,
            instance,
            surface: None,
            surface_config: None,
            limits,
            features,
            memory_stats: GpuMemoryStats::default(),
            performance_stats: GpuPerformanceStats::default(),
        })
    }

    /// Create GPU context with surface
    pub async fn with_surface<W>(
        window: W,
        width: u32,
        height: u32,
    ) -> Result<Self, Box<dyn std::error::Error>>
    where
        W: Into<wgpu::SurfaceTarget<'static>> + Send + Sync + 'static,
    {
        let mut context = Self::new().await?;

        let gpu_surface =
            GpuSurface::from_window(window, &context.instance, &context.adapter, width, height)
                .await?;
        let surface = gpu_surface.surface;
        let surface_config = gpu_surface.config;

        surface.configure(&context.device, &surface_config);

        context.surface = Some(surface);
        context.surface_config = Some(surface_config);

        Ok(context)
    }

    /// Detect supported GPU features
    fn detect_features(adapter: &wgpu::Adapter) -> Vec<GpuFeature> {
        let mut features = Vec::new();
        let limits = adapter.limits();
        let downlevel = adapter.get_downlevel_capabilities();

        if limits.max_vertex_buffers > 0 {
            features.push(GpuFeature::Instancing);
        }

        if downlevel
            .flags
            .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
        {
            features.push(GpuFeature::ComputeShaders);
        }

        if downlevel
            .flags
            .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING)
        {
            features.push(GpuFeature::AnisotropicFiltering);
        }

        if limits.max_sampled_textures_per_shader_stage > 0 {
            features.push(GpuFeature::Msaa);
        }

        // WGPU currently does not expose geometry/tessellation shader stages.
        features.push(GpuFeature::MultiThreading);

        features
    }

    /// Check if a feature is supported
    pub fn supports_feature(&self, feature: GpuFeature) -> bool {
        self.features.contains(&feature)
    }

    /// Get current frame texture for rendering
    pub fn get_current_frame(&self) -> Result<wgpu::SurfaceTexture, wgpu::SurfaceError> {
        self.surface.as_ref().unwrap().get_current_texture()
    }

    /// Submit command buffer to queue
    pub fn submit(&self, command_buffer: wgpu::CommandBuffer) {
        self.queue.submit(Some(command_buffer));
    }

    /// Submit multiple command buffers
    pub fn submit_multiple(&self, command_buffers: Vec<wgpu::CommandBuffer>) {
        self.queue.submit(command_buffers);
    }

    /// Create command encoder
    pub fn create_command_encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }

    /// Present the current frame
    pub fn present(&self, frame: wgpu::SurfaceTexture) {
        present_surface_texture(frame);
    }

    /// Resize surface
    pub fn resize(&mut self, width: u32, height: u32) {
        if let Some(config) = &mut self.surface_config {
            config.width = width;
            config.height = height;
            if let Some(surface) = &self.surface {
                surface.configure(&self.device, config);
            }
        }
    }

    /// Get device reference
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get queue reference
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Get adapter reference
    pub fn adapter(&self) -> &wgpu::Adapter {
        &self.adapter
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> &GpuMemoryStats {
        &self.memory_stats
    }

    /// Get performance statistics
    pub fn performance_stats(&self) -> &GpuPerformanceStats {
        &self.performance_stats
    }

    /// Update performance statistics
    pub fn update_performance_stats(&mut self, frame_time_ms: f32) {
        self.performance_stats.frame_time_ms = frame_time_ms;
        if frame_time_ms > 0.0 {
            self.performance_stats.fps = 1000.0 / frame_time_ms;
        }
    }

    /// Create a render pass descriptor
    pub fn create_render_pass_descriptor<'a>(
        &'a self,
        color_attachments: &'a [Option<wgpu::RenderPassColorAttachment<'a>>],
        depth_stencil_attachment: Option<wgpu::RenderPassDepthStencilAttachment<'a>>,
    ) -> wgpu::RenderPassDescriptor<'a> {
        wgpu::RenderPassDescriptor {
            label: Some("WW3D Render Pass"),
            color_attachments,
            depth_stencil_attachment,
            occlusion_query_set: None,
            timestamp_writes: None,
        }
    }

    /// Create a compute pass descriptor
    pub fn create_compute_pass_descriptor(&self) -> wgpu::ComputePassDescriptor<'static> {
        wgpu::ComputePassDescriptor {
            label: Some("WW3D Compute Pass"),
            timestamp_writes: None,
        }
    }
}

/// GPU resource trait for memory tracking
pub trait GpuResource {
    fn memory_usage(&self) -> u64;
    fn resource_type(&self) -> &'static str;
}

/// Frame timing utilities
pub struct FrameTimer {
    start_time: std::time::Instant,
    frame_count: u64,
    total_time: std::time::Duration,
}

impl Default for FrameTimer {
    fn default() -> Self {
        Self::new()
    }
}

impl FrameTimer {
    pub fn new() -> Self {
        Self {
            start_time: std::time::Instant::now(),
            frame_count: 0,
            total_time: std::time::Duration::ZERO,
        }
    }

    pub fn start_frame(&mut self) {
        self.start_time = std::time::Instant::now();
    }

    pub fn end_frame(&mut self) -> f32 {
        let frame_time = self.start_time.elapsed();
        self.total_time += frame_time;
        self.frame_count += 1;

        frame_time.as_secs_f32() * 1000.0 // Convert to milliseconds
    }

    pub fn average_frame_time(&self) -> f32 {
        if self.frame_count > 0 {
            (self.total_time.as_secs_f32() * 1000.0) / self.frame_count as f32
        } else {
            0.0
        }
    }

    pub fn fps(&self) -> f32 {
        let avg_frame_time = self.average_frame_time();
        if avg_frame_time > 0.0 {
            1000.0 / avg_frame_time
        } else {
            0.0
        }
    }

    pub fn reset(&mut self) {
        self.frame_count = 0;
        self.total_time = std::time::Duration::ZERO;
    }
}

/// Global GPU context instance
static GPU_CONTEXT: std::sync::OnceLock<GpuContext> = std::sync::OnceLock::new();
/// Initialize global GPU context
pub async fn init_gpu_context() -> Result<(), Box<dyn std::error::Error>> {
    let context = GpuContext::new().await?;
    let _ = GPU_CONTEXT.set(context);
    Ok(())
}

/// Get global GPU context
pub fn get_gpu_context() -> Option<&'static GpuContext> {
    GPU_CONTEXT.get()
}

/// GPU error types
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("GPU device lost")]
    DeviceLost,
    #[error("Out of GPU memory")]
    OutOfMemory,
    #[error("Surface error: {0}")]
    SurfaceError(#[from] wgpu::SurfaceError),
    #[error("GPU not initialized")]
    NotInitialized,
    #[error("Unsupported feature: {0}")]
    UnsupportedFeature(String),
    #[error("Invalid operation: {0}")]
    InvalidOperation(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
}

pub type GpuResult<T> = Result<T, GpuError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_limits_conversion() {
        let wgpu_limits = wgpu::Limits::default();
        let gpu_limits = GpuLimits::from(wgpu_limits.clone());

        assert_eq!(
            gpu_limits.max_texture_size,
            wgpu_limits.max_texture_dimension_2d
        );
        assert_eq!(gpu_limits.max_bind_groups, wgpu_limits.max_bind_groups);
    }

    #[test]
    fn test_frame_timer() {
        let mut timer = FrameTimer::new();

        timer.start_frame();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let frame_time = timer.end_frame();

        assert!(frame_time >= 10.0);
        assert!(timer.frame_count == 1);
    }

    #[test]
    fn test_gpu_memory_stats() {
        let stats = GpuMemoryStats::default();
        assert_eq!(stats.total_memory, 0);
        assert_eq!(stats.used_memory, 0);
        assert_eq!(stats.buffer_count, 0);
    }

    #[test]
    fn test_gpu_performance_stats() {
        let stats = GpuPerformanceStats::default();
        assert_eq!(stats.frame_time_ms, 0.0);
        assert_eq!(stats.fps, 0.0);
        assert_eq!(stats.draw_calls, 0);
    }
}
