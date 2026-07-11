//! GPU Synchronization Primitives
//!
//! This module provides GPU synchronization primitives including fences,
//! semaphores, events, and barriers for coordinating GPU operations.

use crate::*;
use std::sync::Arc;

/// GPU fence for CPU-GPU synchronization
#[derive(Debug)]
pub struct Fence {
    /// Fence value for tracking completion
    value: u64,
    /// Fence label
    _label: Option<String>,
}

impl Fence {
    /// Create a new fence
    pub fn new(_device: &crate::device::GpuDevice, label: Option<&str>) -> Result<Self, GpuError> {
        // WGPU doesn't have explicit fences in the same way as Vulkan/DX12
        // This is a placeholder for potential future extensions
        Ok(Self {
            value: 0,
            _label: label.map(|s| s.to_string()),
        })
    }

    /// Signal the fence
    pub fn signal(&mut self, value: u64) {
        self.value = value;
    }

    /// Wait for the fence to reach a specific value
    pub fn wait(&self, value: u64, _timeout: u64) -> Result<(), GpuError> {
        // In WGPU, synchronization is handled differently
        // This is a simplified implementation
        if self.value >= value {
            Ok(())
        } else {
            Err(GpuError::InvalidOperation("Fence not signaled".to_string()))
        }
    }

    /// Get the current fence value
    pub fn value(&self) -> u64 {
        self.value
    }

    /// Check if fence is signaled
    pub fn is_signaled(&self, value: u64) -> bool {
        self.value >= value
    }

    /// Reset the fence
    pub fn reset(&mut self, value: u64) {
        self.value = value;
    }
}

/// GPU semaphore for GPU-GPU synchronization
#[derive(Debug)]
pub struct Semaphore {
    /// Semaphore value
    value: u64,
    /// Semaphore label
    _label: Option<String>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(initial_value: u64, label: Option<&str>) -> Self {
        Self {
            value: initial_value,
            _label: label.map(|s| s.to_string()),
        }
    }

    /// Signal the semaphore
    pub fn signal(&mut self, value: u64) {
        self.value = value;
    }

    /// Wait for the semaphore to reach a specific value
    pub fn wait(&self, value: u64) -> Result<(), GpuError> {
        if self.value >= value {
            Ok(())
        } else {
            Err(GpuError::InvalidOperation(
                "Semaphore not signaled".to_string(),
            ))
        }
    }

    /// Get the current semaphore value
    pub fn value(&self) -> u64 {
        self.value
    }
}

/// GPU event for fine-grained synchronization
#[derive(Debug)]
pub struct Event {
    /// Event state
    signaled: bool,
    /// Event label
    _label: Option<String>,
}

impl Event {
    /// Create a new event
    pub fn new(label: Option<&str>) -> Self {
        Self {
            signaled: false,
            _label: label.map(|s| s.to_string()),
        }
    }

    /// Signal the event
    pub fn signal(&mut self) {
        self.signaled = true;
    }

    /// Reset the event
    pub fn reset(&mut self) {
        self.signaled = false;
    }

    /// Check if event is signaled
    pub fn is_signaled(&self) -> bool {
        self.signaled
    }

    /// Wait for the event to be signaled
    pub fn wait(&self) -> Result<(), GpuError> {
        if self.signaled {
            Ok(())
        } else {
            Err(GpuError::InvalidOperation("Event not signaled".to_string()))
        }
    }
}

/// Memory barrier for synchronization
#[derive(Debug, Clone)]
pub struct MemoryBarrier {
    /// Source access flags
    pub src_access: AccessFlags,
    /// Destination access flags
    pub dst_access: AccessFlags,
    /// Source stage
    pub src_stage: PipelineStage,
    /// Destination stage
    pub dst_stage: PipelineStage,
}

impl MemoryBarrier {
    /// Create a new memory barrier
    pub fn new(
        src_stage: PipelineStage,
        dst_stage: PipelineStage,
        src_access: AccessFlags,
        dst_access: AccessFlags,
    ) -> Self {
        Self {
            src_access,
            dst_access,
            src_stage,
            dst_stage,
        }
    }

    /// Create a full memory barrier
    pub fn full() -> Self {
        Self {
            src_access: AccessFlags::all(),
            dst_access: AccessFlags::all(),
            src_stage: PipelineStage::AllCommands,
            dst_stage: PipelineStage::AllCommands,
        }
    }
}

/// Buffer memory barrier
#[derive(Debug, Clone)]
pub struct BufferMemoryBarrier {
    /// Base memory barrier
    pub barrier: MemoryBarrier,
    /// Buffer reference
    pub buffer: Option<Arc<crate::buffer::GpuBuffer>>,
    /// Offset in buffer
    pub offset: u64,
    /// Size of the barrier
    pub size: u64,
}

impl BufferMemoryBarrier {
    /// Create a new buffer memory barrier
    pub fn new(buffer: Arc<crate::buffer::GpuBuffer>, barrier: MemoryBarrier) -> Self {
        Self {
            barrier,
            buffer: Some(buffer),
            offset: 0,
            size: u64::MAX,
        }
    }
}

/// Image memory barrier
#[derive(Debug, Clone)]
pub struct ImageMemoryBarrier {
    /// Base memory barrier
    pub barrier: MemoryBarrier,
    /// Texture reference
    pub texture: Option<Arc<crate::texture::GpuTexture>>,
    /// Old layout
    pub old_layout: ImageLayout,
    /// New layout
    pub new_layout: ImageLayout,
    /// Subresource range
    pub subresource_range: ImageSubresourceRange,
}

impl ImageMemoryBarrier {
    /// Create a new image memory barrier
    pub fn new(
        texture: Arc<crate::texture::GpuTexture>,
        old_layout: ImageLayout,
        new_layout: ImageLayout,
        barrier: MemoryBarrier,
    ) -> Self {
        Self {
            barrier,
            texture: Some(texture),
            old_layout,
            new_layout,
            subresource_range: ImageSubresourceRange::default(),
        }
    }
}

/// Image subresource range
#[derive(Debug, Clone, Default)]
pub struct ImageSubresourceRange {
    pub aspect_mask: ImageAspect,
    pub base_mip_level: u32,
    pub level_count: u32,
    pub base_array_layer: u32,
    pub layer_count: u32,
}

impl ImageSubresourceRange {
    /// Create a full range
    pub fn full() -> Self {
        Self {
            aspect_mask: ImageAspect::Color,
            base_mip_level: 0,
            level_count: u32::MAX,
            base_array_layer: 0,
            layer_count: u32::MAX,
        }
    }
}

/// Access flags for memory operations
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AccessFlags(u32);

impl AccessFlags {
    /// No access
    pub const NONE: Self = Self(0);
    /// Indirect command read
    pub const INDIRECT_COMMAND_READ: Self = Self(1 << 0);
    /// Index read
    pub const INDEX_READ: Self = Self(1 << 1);
    /// Vertex attribute read
    pub const VERTEX_ATTRIBUTE_READ: Self = Self(1 << 2);
    /// Uniform read
    pub const UNIFORM_READ: Self = Self(1 << 3);
    /// Input attachment read
    pub const INPUT_ATTACHMENT_READ: Self = Self(1 << 4);
    /// Shader read
    pub const SHADER_READ: Self = Self(1 << 5);
    /// Shader write
    pub const SHADER_WRITE: Self = Self(1 << 6);
    /// Color attachment read
    pub const COLOR_ATTACHMENT_READ: Self = Self(1 << 7);
    /// Color attachment write
    pub const COLOR_ATTACHMENT_WRITE: Self = Self(1 << 8);
    /// Depth stencil attachment read
    pub const DEPTH_STENCIL_ATTACHMENT_READ: Self = Self(1 << 9);
    /// Depth stencil attachment write
    pub const DEPTH_STENCIL_ATTACHMENT_WRITE: Self = Self(1 << 10);
    /// Transfer read
    pub const TRANSFER_READ: Self = Self(1 << 11);
    /// Transfer write
    pub const TRANSFER_WRITE: Self = Self(1 << 12);
    /// Host read
    pub const HOST_READ: Self = Self(1 << 13);
    /// Host write
    pub const HOST_WRITE: Self = Self(1 << 14);
    /// Memory read
    pub const MEMORY_READ: Self = Self(1 << 15);
    /// Memory write
    pub const MEMORY_WRITE: Self = Self(1 << 16);

    /// All access flags
    pub const fn all() -> Self {
        Self((1 << 17) - 1)
    }

    /// Check if flag is set
    pub fn contains(&self, other: Self) -> bool {
        (self.0 & other.0) != 0
    }

    /// Combine access flags
    pub fn union(&self, other: Self) -> Self {
        Self(self.0 | other.0)
    }
}

/// Pipeline stages
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStage {
    /// Top of pipe
    TopOfPipe,
    /// Draw indirect
    DrawIndirect,
    /// Vertex input
    VertexInput,
    /// Vertex shader
    VertexShader,
    /// Tessellation control shader
    TessellationControlShader,
    /// Tessellation evaluation shader
    TessellationEvaluationShader,
    /// Geometry shader
    GeometryShader,
    /// Fragment shader
    FragmentShader,
    /// Early fragment tests
    EarlyFragmentTests,
    /// Late fragment tests
    LateFragmentTests,
    /// Color attachment output
    ColorAttachmentOutput,
    /// Compute shader
    ComputeShader,
    /// Transfer
    Transfer,
    /// Bottom of pipe
    BottomOfPipe,
    /// Host
    Host,
    /// All commands
    AllCommands,
    /// All graphics
    AllGraphics,
}

/// Image layouts
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageLayout {
    /// Undefined layout
    Undefined,
    /// General layout
    General,
    /// Color attachment optimal
    ColorAttachmentOptimal,
    /// Depth stencil attachment optimal
    DepthStencilAttachmentOptimal,
    /// Depth stencil read only optimal
    DepthStencilReadOnlyOptimal,
    /// Shader read only optimal
    ShaderReadOnlyOptimal,
    /// Transfer src optimal
    TransferSrcOptimal,
    /// Transfer dst optimal
    TransferDstOptimal,
    /// Preinitialized
    Preinitialized,
    /// Present src
    PresentSrc,
}

/// Image aspects
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ImageAspect {
    /// Color aspect
    #[default]
    Color,
    /// Depth aspect
    Depth,
    /// Stencil aspect
    Stencil,
    /// Depth and stencil aspects
    DepthStencil,
}

/// Synchronization manager
#[derive(Debug)]
pub struct SynchronizationManager {
    /// Fences
    fences: Vec<Arc<Fence>>,
    /// Semaphores
    semaphores: Vec<Arc<Semaphore>>,
    /// Events
    events: Vec<Arc<Event>>,
    /// Synchronization statistics
    stats: SyncStats,
}

impl Default for SynchronizationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl SynchronizationManager {
    /// Create a new synchronization manager
    pub fn new() -> Self {
        Self {
            fences: Vec::new(),
            semaphores: Vec::new(),
            events: Vec::new(),
            stats: SyncStats::default(),
        }
    }

    /// Create a fence
    pub fn create_fence(
        &mut self,
        device: &crate::device::GpuDevice,
        label: Option<&str>,
    ) -> Result<Arc<Fence>, GpuError> {
        let fence = Fence::new(device, label)?;
        let fence_arc = Arc::new(fence);
        self.fences.push(fence_arc.clone());
        self.update_stats();
        Ok(fence_arc)
    }

    /// Create a semaphore
    pub fn create_semaphore(&mut self, initial_value: u64, label: Option<&str>) -> Arc<Semaphore> {
        let semaphore = Semaphore::new(initial_value, label);
        let semaphore_arc = Arc::new(semaphore);
        self.semaphores.push(semaphore_arc.clone());
        self.update_stats();
        semaphore_arc
    }

    /// Create an event
    pub fn create_event(&mut self, label: Option<&str>) -> Arc<Event> {
        let event = Event::new(label);
        let event_arc = Arc::new(event);
        self.events.push(event_arc.clone());
        self.update_stats();
        event_arc
    }

    /// Get synchronization statistics
    pub fn stats(&self) -> &SyncStats {
        &self.stats
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.fence_count = self.fences.len();
        self.stats.semaphore_count = self.semaphores.len();
        self.stats.event_count = self.events.len();
    }

    /// Cleanup unused synchronization objects
    pub fn cleanup(&mut self) {
        // In a real implementation, you'd track usage and remove unused objects
        self.update_stats();
    }
}

/// Synchronization statistics
#[derive(Debug, Clone, Default)]
pub struct SyncStats {
    pub fence_count: usize,
    pub semaphore_count: usize,
    pub event_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_access_flags() {
        let flags = AccessFlags::VERTEX_ATTRIBUTE_READ.union(AccessFlags::UNIFORM_READ);
        assert!(flags.contains(AccessFlags::VERTEX_ATTRIBUTE_READ));
        assert!(flags.contains(AccessFlags::UNIFORM_READ));
        assert!(!flags.contains(AccessFlags::SHADER_WRITE));
    }

    #[test]
    fn test_access_flags_all() {
        let all = AccessFlags::all();
        assert!(all.contains(AccessFlags::VERTEX_ATTRIBUTE_READ));
        assert!(all.contains(AccessFlags::MEMORY_WRITE));
    }

    #[test]
    fn test_memory_barrier() {
        let barrier = MemoryBarrier::new(
            PipelineStage::VertexShader,
            PipelineStage::FragmentShader,
            AccessFlags::SHADER_READ,
            AccessFlags::SHADER_WRITE,
        );

        assert_eq!(barrier.src_stage, PipelineStage::VertexShader);
        assert_eq!(barrier.dst_stage, PipelineStage::FragmentShader);
        assert!(barrier.src_access.contains(AccessFlags::SHADER_READ));
        assert!(barrier.dst_access.contains(AccessFlags::SHADER_WRITE));
    }

    #[test]
    fn test_memory_barrier_full() {
        let barrier = MemoryBarrier::full();
        assert_eq!(barrier.src_stage, PipelineStage::AllCommands);
        assert_eq!(barrier.dst_stage, PipelineStage::AllCommands);
        assert_eq!(barrier.src_access, AccessFlags::all());
        assert_eq!(barrier.dst_access, AccessFlags::all());
    }

    #[test]
    fn test_image_subresource_range() {
        let range = ImageSubresourceRange::full();
        assert_eq!(range.aspect_mask, ImageAspect::Color);
        assert_eq!(range.base_mip_level, 0);
        assert_eq!(range.level_count, u32::MAX);
    }

    #[test]
    fn test_sync_stats() {
        let stats = SyncStats::default();
        assert_eq!(stats.fence_count, 0);
        assert_eq!(stats.semaphore_count, 0);
        assert_eq!(stats.event_count, 0);
    }

    #[test]
    fn test_event() {
        let mut event = Event::new(Some("test_event"));
        assert!(!event.is_signaled());

        event.signal();
        assert!(event.is_signaled());
        assert!(event.wait().is_ok());

        event.reset();
        assert!(!event.is_signaled());
        assert!(event.wait().is_err());
    }

    #[test]
    fn test_semaphore() {
        let mut semaphore = Semaphore::new(0, Some("test_semaphore"));
        assert_eq!(semaphore.value(), 0);

        semaphore.signal(5);
        assert_eq!(semaphore.value(), 5);
        assert!(semaphore.wait(3).is_ok());
        assert!(semaphore.wait(7).is_err());
    }
}
