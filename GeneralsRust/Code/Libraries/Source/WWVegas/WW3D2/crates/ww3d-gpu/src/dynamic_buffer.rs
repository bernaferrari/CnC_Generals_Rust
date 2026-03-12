//! Dynamic Buffer Ring System
//!
//! This module provides a ring buffer system for streaming vertex and index data
//! to the GPU each frame. It replaces the DX8 dynamic buffer with lock/unlock API.
//!
//! The ring buffer uses triple-buffering to allow CPU-side updates while the GPU
//! is processing previous frames, avoiding stalls and maximizing throughput.

use crate::{GpuBuffer, GpuError};
use parking_lot::Mutex;
use std::sync::Arc;

/// Default vertex buffer size (vertices)
pub const DEFAULT_VERTEX_COUNT: usize = 32768;

/// Default index buffer size (indices)
pub const DEFAULT_INDEX_COUNT: usize = 65536;

/// Number of frames to buffer (triple buffering)
pub const FRAME_BUFFER_COUNT: usize = 3;

/// Dynamic vertex buffer ring for per-frame streaming data
pub struct DynamicVertexBufferRing {
    /// GPU device
    device: Arc<crate::device::GpuDevice>,
    /// Ring of vertex buffers (one per frame)
    buffers: Vec<Arc<GpuBuffer>>,
    /// Current frame index
    current_frame: usize,
    /// Current allocation offset within frame buffer
    current_offset: u64,
    /// Vertex stride in bytes
    vertex_stride: u64,
    /// Total vertex capacity per buffer
    vertex_capacity: usize,
    /// FVF format
    fvf_format: crate::fvf::FvfFormat,
}

impl DynamicVertexBufferRing {
    /// Create a new dynamic vertex buffer ring
    pub fn new(
        device: Arc<crate::device::GpuDevice>,
        vertex_capacity: usize,
        fvf_format: crate::fvf::FvfFormat,
    ) -> Result<Self, GpuError> {
        let vertex_stride = fvf_format.stride();
        let buffer_size = vertex_stride * vertex_capacity as u64;

        // Create buffers for each frame
        let mut buffers = Vec::with_capacity(FRAME_BUFFER_COUNT);
        for i in 0..FRAME_BUFFER_COUNT {
            let buffer = GpuBuffer::vertex_buffer(
                &device,
                buffer_size,
                Some(&format!("Dynamic Vertex Buffer {}", i)),
            )?;
            buffers.push(Arc::new(buffer));
        }

        Ok(Self {
            device,
            buffers,
            current_frame: 0,
            current_offset: 0,
            vertex_stride,
            vertex_capacity,
            fvf_format,
        })
    }

    /// Allocate space for vertices in the current frame buffer
    /// Returns (buffer, offset in bytes, offset in vertices)
    pub fn allocate(
        &mut self,
        vertex_count: usize,
    ) -> Result<(Arc<GpuBuffer>, u64, u32), GpuError> {
        let required_size = vertex_count as u64 * self.vertex_stride;
        let buffer_size = self.vertex_capacity as u64 * self.vertex_stride;

        // Check if allocation fits in current buffer
        if self.current_offset + required_size > buffer_size {
            return Err(GpuError::OutOfMemory);
        }

        let buffer = self.buffers[self.current_frame].clone();
        let byte_offset = self.current_offset;
        let vertex_offset = (self.current_offset / self.vertex_stride) as u32;

        self.current_offset += required_size;

        Ok((buffer, byte_offset, vertex_offset))
    }

    /// Write vertex data to allocated space
    pub fn write_vertices<T: bytemuck::Pod>(
        &mut self,
        vertices: &[T],
    ) -> Result<(Arc<GpuBuffer>, u32), GpuError> {
        let (buffer, byte_offset, vertex_offset) = self.allocate(vertices.len())?;

        // Write data to buffer
        let data = bytemuck::cast_slice(vertices);
        self.device
            .queue()
            .write_buffer(buffer.wgpu_buffer(), byte_offset, data);

        Ok((buffer, vertex_offset))
    }

    /// Advance to next frame buffer
    pub fn next_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % FRAME_BUFFER_COUNT;
        self.current_offset = 0;
    }

    /// Reset current frame (clear allocations but stay on same frame)
    pub fn reset(&mut self) {
        self.current_offset = 0;
    }

    /// Get current buffer
    pub fn current_buffer(&self) -> &Arc<GpuBuffer> {
        &self.buffers[self.current_frame]
    }

    /// Get vertex capacity
    pub fn capacity(&self) -> usize {
        self.vertex_capacity
    }

    /// Get remaining capacity in current frame
    pub fn remaining_capacity(&self) -> usize {
        let used_vertices = (self.current_offset / self.vertex_stride) as usize;
        self.vertex_capacity.saturating_sub(used_vertices)
    }

    /// Get FVF format
    pub fn fvf_format(&self) -> crate::fvf::FvfFormat {
        self.fvf_format
    }
}

/// Dynamic index buffer ring for per-frame streaming data
pub struct DynamicIndexBufferRing {
    /// GPU device
    device: Arc<crate::device::GpuDevice>,
    /// Ring of index buffers (one per frame)
    buffers: Vec<Arc<GpuBuffer>>,
    /// Current frame index
    current_frame: usize,
    /// Current allocation offset
    current_offset: u64,
    /// Index size (2 or 4 bytes)
    index_size: u64,
    /// Total index capacity per buffer
    index_capacity: usize,
}

impl DynamicIndexBufferRing {
    /// Create a new dynamic index buffer ring
    pub fn new(
        device: Arc<crate::device::GpuDevice>,
        index_capacity: usize,
        use_u32: bool,
    ) -> Result<Self, GpuError> {
        let index_size = if use_u32 { 4 } else { 2 };
        let buffer_size = index_size * index_capacity as u64;

        // Create buffers for each frame
        let mut buffers = Vec::with_capacity(FRAME_BUFFER_COUNT);
        for i in 0..FRAME_BUFFER_COUNT {
            let buffer = GpuBuffer::index_buffer(
                &device,
                buffer_size,
                Some(&format!("Dynamic Index Buffer {}", i)),
            )?;
            buffers.push(Arc::new(buffer));
        }

        Ok(Self {
            device,
            buffers,
            current_frame: 0,
            current_offset: 0,
            index_size,
            index_capacity,
        })
    }

    /// Allocate space for indices in the current frame buffer
    /// Returns (buffer, offset in bytes, offset in indices)
    pub fn allocate(&mut self, index_count: usize) -> Result<(Arc<GpuBuffer>, u64, u32), GpuError> {
        let required_size = index_count as u64 * self.index_size;
        let buffer_size = self.index_capacity as u64 * self.index_size;

        if self.current_offset + required_size > buffer_size {
            return Err(GpuError::OutOfMemory);
        }

        let buffer = self.buffers[self.current_frame].clone();
        let byte_offset = self.current_offset;
        let index_offset = (self.current_offset / self.index_size) as u32;

        self.current_offset += required_size;

        Ok((buffer, byte_offset, index_offset))
    }

    /// Write index data to allocated space (u16)
    pub fn write_indices_u16(
        &mut self,
        indices: &[u16],
    ) -> Result<(Arc<GpuBuffer>, u32), GpuError> {
        if self.index_size != 2 {
            return Err(GpuError::InvalidOperation(
                "Buffer configured for u32 indices".to_string(),
            ));
        }

        let (buffer, byte_offset, index_offset) = self.allocate(indices.len())?;

        let data = bytemuck::cast_slice(indices);
        self.device
            .queue()
            .write_buffer(buffer.wgpu_buffer(), byte_offset, data);

        Ok((buffer, index_offset))
    }

    /// Write index data to allocated space (u32)
    pub fn write_indices_u32(
        &mut self,
        indices: &[u32],
    ) -> Result<(Arc<GpuBuffer>, u32), GpuError> {
        if self.index_size != 4 {
            return Err(GpuError::InvalidOperation(
                "Buffer configured for u16 indices".to_string(),
            ));
        }

        let (buffer, byte_offset, index_offset) = self.allocate(indices.len())?;

        let data = bytemuck::cast_slice(indices);
        self.device
            .queue()
            .write_buffer(buffer.wgpu_buffer(), byte_offset, data);

        Ok((buffer, index_offset))
    }

    /// Advance to next frame buffer
    pub fn next_frame(&mut self) {
        self.current_frame = (self.current_frame + 1) % FRAME_BUFFER_COUNT;
        self.current_offset = 0;
    }

    /// Reset current frame
    pub fn reset(&mut self) {
        self.current_offset = 0;
    }

    /// Get current buffer
    pub fn current_buffer(&self) -> &Arc<GpuBuffer> {
        &self.buffers[self.current_frame]
    }

    /// Get index capacity
    pub fn capacity(&self) -> usize {
        self.index_capacity
    }

    /// Get remaining capacity
    pub fn remaining_capacity(&self) -> usize {
        let used_indices = (self.current_offset / self.index_size) as usize;
        self.index_capacity.saturating_sub(used_indices)
    }

    /// Check if using u32 indices
    pub fn is_u32(&self) -> bool {
        self.index_size == 4
    }
}

/// Global dynamic buffer manager
pub struct DynamicBufferManager {
    /// Vertex buffer ring
    vertex_ring: Mutex<Option<DynamicVertexBufferRing>>,
    /// Index buffer ring
    index_ring: Mutex<Option<DynamicIndexBufferRing>>,
}

impl DynamicBufferManager {
    /// Create a new dynamic buffer manager
    pub fn new() -> Self {
        Self {
            vertex_ring: Mutex::new(None),
            index_ring: Mutex::new(None),
        }
    }

    /// Initialize vertex buffer ring
    pub fn init_vertex_ring(
        &self,
        device: Arc<crate::device::GpuDevice>,
        vertex_capacity: usize,
        fvf_format: crate::fvf::FvfFormat,
    ) -> Result<(), GpuError> {
        let ring = DynamicVertexBufferRing::new(device, vertex_capacity, fvf_format)?;
        *self.vertex_ring.lock() = Some(ring);
        Ok(())
    }

    /// Initialize index buffer ring
    pub fn init_index_ring(
        &self,
        device: Arc<crate::device::GpuDevice>,
        index_capacity: usize,
        use_u32: bool,
    ) -> Result<(), GpuError> {
        let ring = DynamicIndexBufferRing::new(device, index_capacity, use_u32)?;
        *self.index_ring.lock() = Some(ring);
        Ok(())
    }

    /// Access vertex ring
    pub fn with_vertex_ring<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut DynamicVertexBufferRing) -> R,
    {
        self.vertex_ring.lock().as_mut().map(f)
    }

    /// Access index ring
    pub fn with_index_ring<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut DynamicIndexBufferRing) -> R,
    {
        self.index_ring.lock().as_mut().map(f)
    }

    /// Advance both rings to next frame
    pub fn next_frame(&self) {
        if let Some(ref mut ring) = *self.vertex_ring.lock() {
            ring.next_frame();
        }
        if let Some(ref mut ring) = *self.index_ring.lock() {
            ring.next_frame();
        }
    }

    /// Reset both rings for current frame
    pub fn reset(&self) {
        if let Some(ref mut ring) = *self.vertex_ring.lock() {
            ring.reset();
        }
        if let Some(ref mut ring) = *self.index_ring.lock() {
            ring.reset();
        }
    }

    /// Shutdown and release all resources
    pub fn shutdown(&self) {
        *self.vertex_ring.lock() = None;
        *self.index_ring.lock() = None;
    }
}

impl Default for DynamicBufferManager {
    fn default() -> Self {
        Self::new()
    }
}

// Global dynamic buffer manager instance
lazy_static::lazy_static! {
    pub static ref DYNAMIC_BUFFERS: DynamicBufferManager = DynamicBufferManager::new();
}

/// Initialize dynamic buffers with default settings
pub fn init_dynamic_buffers(device: Arc<crate::device::GpuDevice>) -> Result<(), GpuError> {
    DYNAMIC_BUFFERS.init_vertex_ring(
        device.clone(),
        DEFAULT_VERTEX_COUNT,
        crate::fvf::FvfFormat::XYZNDUV2, // Default dynamic format
    )?;
    DYNAMIC_BUFFERS.init_index_ring(device, DEFAULT_INDEX_COUNT, false)?;
    Ok(())
}

/// Write vertices to dynamic buffer
pub fn write_dynamic_vertices<T: bytemuck::Pod>(
    vertices: &[T],
) -> Result<(Arc<GpuBuffer>, u32), GpuError> {
    DYNAMIC_BUFFERS
        .with_vertex_ring(|ring| ring.write_vertices(vertices))
        .ok_or_else(|| GpuError::NotInitialized)?
}

/// Write indices to dynamic buffer (u16)
pub fn write_dynamic_indices_u16(indices: &[u16]) -> Result<(Arc<GpuBuffer>, u32), GpuError> {
    DYNAMIC_BUFFERS
        .with_index_ring(|ring| ring.write_indices_u16(indices))
        .ok_or_else(|| GpuError::NotInitialized)?
}

/// Write indices to dynamic buffer (u32)
pub fn write_dynamic_indices_u32(indices: &[u32]) -> Result<(Arc<GpuBuffer>, u32), GpuError> {
    DYNAMIC_BUFFERS
        .with_index_ring(|ring| ring.write_indices_u32(indices))
        .ok_or_else(|| GpuError::NotInitialized)?
}

/// Advance dynamic buffers to next frame
pub fn advance_dynamic_buffers() {
    DYNAMIC_BUFFERS.next_frame();
}

/// Reset dynamic buffers for current frame
pub fn reset_dynamic_buffers() {
    DYNAMIC_BUFFERS.reset();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dynamic_buffer_allocation() {
        // Note: This test would require a real GPU device
        // In practice, this would be an integration test
        assert_eq!(DEFAULT_VERTEX_COUNT, 32768);
        assert_eq!(DEFAULT_INDEX_COUNT, 65536);
        assert_eq!(FRAME_BUFFER_COUNT, 3);
    }

    #[test]
    fn test_frame_advance() {
        let manager = DynamicBufferManager::new();
        manager.next_frame(); // Should not panic even without initialized rings
        manager.reset();
    }
}
