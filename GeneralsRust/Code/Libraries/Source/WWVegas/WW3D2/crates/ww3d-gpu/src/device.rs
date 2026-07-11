//! GPU Device Abstraction Layer
//!
//! This module provides the core GPU device interface, managing device creation,
//! resource allocation, and providing a unified API across different GPU backends.

use crate::*;
use std::sync::Arc;

/// GPU device abstraction
#[derive(Debug)]
pub struct GpuDevice {
    /// WGPU device handle
    device: Arc<wgpu::Device>,
    /// Command queue for submitting work
    queue: Arc<wgpu::Queue>,
    /// Downlevel capabilities reported by the adapter
    downlevel: wgpu::DownlevelCapabilities,
    /// Device capabilities and limits
    capabilities: GpuCapabilities,
    /// Memory allocator for GPU resources
    _memory_allocator: MemoryAllocator,
    /// Resource tracker for memory management
    resource_tracker: ResourceTracker,
}

impl GpuDevice {
    /// Create a new GPU device from WGPU device
    pub fn new(device: wgpu::Device, queue: wgpu::Queue) -> Self {
        Self::new_with_downlevel(device, queue, wgpu::DownlevelCapabilities::default())
    }

    /// Create a new GPU device providing explicit downlevel capabilities.
    pub fn new_with_downlevel(
        device: wgpu::Device,
        queue: wgpu::Queue,
        downlevel: wgpu::DownlevelCapabilities,
    ) -> Self {
        Self::from_shared_with_downlevel(Arc::new(device), Arc::new(queue), downlevel)
    }

    /// Create a GPU device from existing shared handles.
    pub fn from_shared(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        Self::from_shared_with_downlevel(device, queue, wgpu::DownlevelCapabilities::default())
    }

    /// Create a GPU device from shared handles with explicit downlevel capabilities.
    pub fn from_shared_with_downlevel(
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        downlevel: wgpu::DownlevelCapabilities,
    ) -> Self {
        let capabilities = GpuCapabilities::from_device(&device, &downlevel);
        let memory_allocator = MemoryAllocator::new(&capabilities);
        let resource_tracker = ResourceTracker::new();

        Self {
            device,
            queue,
            downlevel,
            capabilities,
            _memory_allocator: memory_allocator,
            resource_tracker,
        }
    }

    /// Create a GPU device with custom configuration
    pub async fn create_device(
        adapter: &wgpu::Adapter,
        features: wgpu::Features,
        limits: wgpu::Limits,
    ) -> Result<Self, GpuError> {
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("WW3D GPU Device"),
                required_features: features,
                required_limits: limits,
                ..Default::default()
            })
            .await
            .map_err(|_e| GpuError::DeviceLost)?;

        let downlevel = adapter.get_downlevel_capabilities();

        Ok(Self::new_with_downlevel(device, queue, downlevel))
    }

    /// Get the underlying WGPU device
    pub fn wgpu_device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get the command queue
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    /// Downlevel capabilities reported by the adapter.
    pub fn downlevel(&self) -> &wgpu::DownlevelCapabilities {
        &self.downlevel
    }

    /// Cloneable handle to the underlying device
    pub fn device_arc(&self) -> Arc<wgpu::Device> {
        Arc::clone(&self.device)
    }

    /// Cloneable handle to the underlying queue
    pub fn queue_arc(&self) -> Arc<wgpu::Queue> {
        Arc::clone(&self.queue)
    }

    /// Get device capabilities
    pub fn capabilities(&self) -> &GpuCapabilities {
        &self.capabilities
    }

    /// Create a command encoder
    pub fn create_command_encoder(&self, label: Option<&str>) -> wgpu::CommandEncoder {
        self.device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label })
    }

    /// Submit command buffers to the queue
    pub fn submit(&self, command_buffers: Vec<wgpu::CommandBuffer>) {
        self.queue.submit(command_buffers);
    }

    /// Present a surface texture to the display.
    pub fn present_surface_texture(&self, frame: wgpu::SurfaceTexture) {
        crate::present_surface_texture(frame);
    }

    /// Create a buffer
    pub fn create_buffer(&self, desc: &wgpu::BufferDescriptor) -> Result<DeviceBuffer, GpuError> {
        let buffer = self.device.create_buffer(desc);

        // Create our wrapper buffer
        let wrapped_buffer = DeviceBuffer {
            buffer,
            size: desc.size,
            usage: desc.usage,
        };

        Ok(wrapped_buffer)
    }

    /// Create a texture
    pub fn create_texture(&self, desc: &wgpu::TextureDescriptor) -> Result<Texture, GpuError> {
        let texture = self.device.create_texture(desc);

        // Create our wrapper texture
        let wrapped_texture = Texture {
            texture,
            size: desc.size,
            format: desc.format,
            usage: desc.usage,
        };

        Ok(wrapped_texture)
    }

    /// Create a shader module
    pub fn create_shader_module(&self, desc: wgpu::ShaderModuleDescriptor) -> wgpu::ShaderModule {
        self.device.create_shader_module(desc)
    }

    /// Create a bind group layout
    pub fn create_bind_group_layout(
        &self,
        desc: &wgpu::BindGroupLayoutDescriptor,
    ) -> wgpu::BindGroupLayout {
        self.device.create_bind_group_layout(desc)
    }

    /// Create a bind group
    pub fn create_bind_group(&self, desc: &wgpu::BindGroupDescriptor) -> wgpu::BindGroup {
        self.device.create_bind_group(desc)
    }

    /// Create a pipeline layout
    pub fn create_pipeline_layout(
        &self,
        desc: &wgpu::PipelineLayoutDescriptor,
    ) -> wgpu::PipelineLayout {
        self.device.create_pipeline_layout(desc)
    }

    /// Create a render pipeline
    pub fn create_render_pipeline(
        &self,
        desc: &wgpu::RenderPipelineDescriptor,
    ) -> wgpu::RenderPipeline {
        self.device.create_render_pipeline(desc)
    }

    /// Create a compute pipeline
    pub fn create_compute_pipeline(
        &self,
        desc: &wgpu::ComputePipelineDescriptor,
    ) -> wgpu::ComputePipeline {
        self.device.create_compute_pipeline(desc)
    }

    /// Create a sampler
    pub fn create_sampler(&self, desc: &wgpu::SamplerDescriptor) -> wgpu::Sampler {
        self.device.create_sampler(desc)
    }

    /// Poll the device for completion of asynchronous operations
    pub fn poll(&self, poll_type: wgpu::PollType) -> Result<wgpu::PollStatus, wgpu::PollError> {
        self.device.poll(poll_type)
    }

    /// Get memory statistics
    pub fn memory_stats(&self) -> &MemoryStats {
        &self.resource_tracker.memory_stats
    }

    /// Flush pending operations
    pub fn flush(&self) {
        // WGPU automatically manages command submission
        // This is a placeholder for any device-specific flushing
    }

    /// Check if a feature is supported
    pub fn supports_feature(&self, feature: GpuFeature) -> bool {
        self.capabilities.supports_feature(feature)
    }

    /// Get the maximum texture size supported
    pub fn max_texture_size(&self) -> u32 {
        self.capabilities.max_texture_size
    }

    /// Get the maximum number of bind groups
    pub fn max_bind_groups(&self) -> u32 {
        self.capabilities.max_bind_groups
    }
}

/// GPU capabilities and limits
#[derive(Debug, Clone)]
pub struct GpuCapabilities {
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
    pub max_uniform_buffer_binding_size: u32,
    pub max_storage_buffer_binding_size: u32,
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
    pub supported_features: Vec<GpuFeature>,
}

impl GpuCapabilities {
    /// Create capabilities from WGPU device
    pub fn from_device(device: &wgpu::Device, downlevel: &wgpu::DownlevelCapabilities) -> Self {
        let limits = device.limits();

        let supported_features = detect_supported_features(downlevel, &limits);

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
            max_uniform_buffer_binding_size: limits.max_uniform_buffer_binding_size,
            max_storage_buffer_binding_size: limits.max_storage_buffer_binding_size,
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
            supported_features,
        }
    }

    /// Check if a feature is supported
    pub fn supports_feature(&self, feature: GpuFeature) -> bool {
        self.supported_features.contains(&feature)
    }
}

fn detect_supported_features(
    downlevel: &wgpu::DownlevelCapabilities,
    limits: &wgpu::Limits,
) -> Vec<GpuFeature> {
    let mut supported = Vec::new();

    // Instancing is core in modern APIs if we have at least one vertex buffer slot.
    if limits.max_vertex_buffers > 0 {
        supported.push(GpuFeature::Instancing);
    }

    if downlevel
        .flags
        .contains(wgpu::DownlevelFlags::COMPUTE_SHADERS)
    {
        supported.push(GpuFeature::ComputeShaders);
    }

    if downlevel
        .flags
        .contains(wgpu::DownlevelFlags::ANISOTROPIC_FILTERING)
    {
        supported.push(GpuFeature::AnisotropicFiltering);
    }

    // WGPU currently has no geometry/tessellation shaders; leave them absent.

    // WGPU guarantees at least 1x MSAA; consider it supported if sample count > 1.
    if limits.max_sampled_textures_per_shader_stage > 0 {
        supported.push(GpuFeature::Msaa);
    }

    supported.push(GpuFeature::MultiThreading);

    supported
}

/// Memory allocator for GPU resources
#[derive(Debug)]
pub struct MemoryAllocator {
    /// Available memory heaps
    _heaps: Vec<MemoryHeap>,
    /// Current allocations
    _allocations: Vec<MemoryAllocation>,
}

impl MemoryAllocator {
    /// Create a new memory allocator
    pub fn new(_capabilities: &GpuCapabilities) -> Self {
        // Simplified memory allocator - in a real implementation,
        // this would manage GPU memory heaps and sub-allocations
        Self {
            _heaps: Vec::new(),
            _allocations: Vec::new(),
        }
    }

    /// Allocate memory for a resource
    pub fn allocate(
        &mut self,
        size: u64,
        _alignment: u64,
        memory_type: MemoryType,
    ) -> Option<MemoryAllocation> {
        // Simplified allocation - just return a placeholder
        Some(MemoryAllocation {
            offset: 0,
            size,
            memory_type,
        })
    }

    /// Free a memory allocation
    pub fn free(&mut self, _allocation: MemoryAllocation) {
        // Simplified deallocation
    }
}

/// Memory heap information
#[derive(Debug, Clone)]
pub struct MemoryHeap {
    pub size: u64,
    pub flags: wgpu::BufferUsages,
}

/// Memory allocation
#[derive(Debug, Clone)]
pub struct MemoryAllocation {
    pub offset: u64,
    pub size: u64,
    pub memory_type: MemoryType,
}

/// Memory types for different usage patterns
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryType {
    /// Device local memory (fast GPU access)
    DeviceLocal,
    /// Host visible memory (CPU can write, GPU can read)
    HostVisible,
    /// Host coherent memory (no cache flushing required)
    HostCoherent,
}

/// Resource tracker for memory management
#[derive(Debug)]
pub struct ResourceTracker {
    pub memory_stats: MemoryStats,
    pub buffers: Vec<Arc<wgpu::Buffer>>,
    pub textures: Vec<Arc<wgpu::Texture>>,
}

impl Default for ResourceTracker {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceTracker {
    /// Create a new resource tracker
    pub fn new() -> Self {
        Self {
            memory_stats: MemoryStats::default(),
            buffers: Vec::new(),
            textures: Vec::new(),
        }
    }

    /// Track a buffer for memory management
    pub fn track_buffer(&mut self, buffer: wgpu::Buffer, size: u64) {
        self.memory_stats.buffer_count += 1;
        self.memory_stats.used_memory += size;
        self.buffers.push(Arc::new(buffer));
    }

    /// Track a texture for memory management
    pub fn track_texture(&mut self, texture: wgpu::Texture, size: wgpu::Extent3d) {
        // Estimate texture memory usage
        let texel_size = 4; // Assume 4 bytes per texel (RGBA8)
        let memory_size =
            (size.width * size.height * size.depth_or_array_layers) as u64 * texel_size as u64;

        self.memory_stats.texture_count += 1;
        self.memory_stats.used_memory += memory_size;
        self.textures.push(Arc::new(texture));
    }
}

/// Memory statistics
#[derive(Debug, Clone, Default)]
pub struct MemoryStats {
    pub total_memory: u64,
    pub used_memory: u64,
    pub buffer_count: usize,
    pub texture_count: usize,
}

/// GPU buffer wrapper
#[derive(Debug)]
pub struct DeviceBuffer {
    pub buffer: wgpu::Buffer,
    pub size: u64,
    pub usage: wgpu::BufferUsages,
}

impl DeviceBuffer {
    /// Write data to the buffer
    pub fn write_data(&self, device: &GpuDevice, data: &[u8]) {
        device.queue.write_buffer(&self.buffer, 0, data);
    }

    /// Get buffer size
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Check if buffer has a specific usage
    pub fn has_usage(&self, usage: wgpu::BufferUsages) -> bool {
        self.usage.contains(usage)
    }
}

/// GPU texture wrapper
#[derive(Debug)]
pub struct Texture {
    pub texture: wgpu::Texture,
    pub size: wgpu::Extent3d,
    pub format: wgpu::TextureFormat,
    pub usage: wgpu::TextureUsages,
}

impl Texture {
    /// Create a texture view
    pub fn create_view(&self, desc: &wgpu::TextureViewDescriptor) -> wgpu::TextureView {
        self.texture.create_view(desc)
    }

    /// Get texture size
    pub fn size(&self) -> wgpu::Extent3d {
        self.size
    }

    /// Get texture format
    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    /// Get texture usage
    pub fn usage(&self) -> wgpu::TextureUsages {
        self.usage
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gpu_capabilities_creation() {
        // This test would require a mock WGPU device
        // For now, just test the structure
        let capabilities = GpuCapabilities {
            max_texture_size: 8192,
            max_texture_array_layers: 256,
            max_bind_groups: 4,
            max_bindings_per_bind_group: 16,
            max_dynamic_uniform_buffers_per_pipeline_layout: 8,
            max_dynamic_storage_buffers_per_pipeline_layout: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 8,
            max_storage_textures_per_shader_stage: 8,
            max_uniform_buffers_per_shader_stage: 12,
            max_uniform_buffer_binding_size: 65536,
            max_storage_buffer_binding_size: 134217728,
            max_vertex_buffers: 8,
            max_buffer_size: 268435456,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2048,
            max_push_constant_size: 128,
            max_inter_stage_shader_components: 60,
            max_compute_workgroup_storage_size: 16384,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
            supported_features: vec![GpuFeature::Instancing, GpuFeature::ComputeShaders],
        };

        assert_eq!(capabilities.max_texture_size, 8192);
        assert!(capabilities.supports_feature(GpuFeature::Instancing));
        assert!(!capabilities.supports_feature(GpuFeature::GeometryShaders));
    }

    #[test]
    fn test_memory_stats() {
        let stats = MemoryStats::default();
        assert_eq!(stats.total_memory, 0);
        assert_eq!(stats.used_memory, 0);
        assert_eq!(stats.buffer_count, 0);
        assert_eq!(stats.texture_count, 0);
    }

    #[test]
    fn test_memory_allocator() {
        let capabilities = GpuCapabilities {
            max_texture_size: 8192,
            max_texture_array_layers: 256,
            max_bind_groups: 4,
            max_bindings_per_bind_group: 16,
            max_dynamic_uniform_buffers_per_pipeline_layout: 8,
            max_dynamic_storage_buffers_per_pipeline_layout: 4,
            max_sampled_textures_per_shader_stage: 16,
            max_samplers_per_shader_stage: 16,
            max_storage_buffers_per_shader_stage: 8,
            max_storage_textures_per_shader_stage: 8,
            max_uniform_buffers_per_shader_stage: 12,
            max_uniform_buffer_binding_size: 65536,
            max_storage_buffer_binding_size: 134217728,
            max_vertex_buffers: 8,
            max_buffer_size: 268435456,
            max_vertex_attributes: 16,
            max_vertex_buffer_array_stride: 2048,
            max_push_constant_size: 128,
            max_inter_stage_shader_components: 60,
            max_compute_workgroup_storage_size: 16384,
            max_compute_invocations_per_workgroup: 256,
            max_compute_workgroup_size_x: 256,
            max_compute_workgroup_size_y: 256,
            max_compute_workgroup_size_z: 64,
            max_compute_workgroups_per_dimension: 65535,
            supported_features: Vec::new(),
        };

        let mut allocator = MemoryAllocator::new(&capabilities);
        let allocation = allocator.allocate(1024, 256, MemoryType::DeviceLocal);

        assert!(allocation.is_some());
        let alloc = allocation.unwrap();
        assert_eq!(alloc.size, 1024);
        assert_eq!(alloc.memory_type, MemoryType::DeviceLocal);
    }
}
