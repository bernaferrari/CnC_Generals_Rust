//! WGPU Buffer Management
//!
//! This module handles vertex and index buffer management for WGPU,
//! equivalent to the DirectX8 vertex/index buffer functionality.

use crate::core::error::{Error, Result};
use bytemuck::{Pod, Zeroable};
use std::sync::Arc;
use wgpu::{Buffer, BufferDescriptor, BufferUsages, Device};

/// Reference counting for engine resources
pub trait EngineRef {
    fn add_engine_ref(&self);
    fn release_engine_ref(&self);
    fn engine_ref_count(&self) -> u32;
}

/// WGPU Vertex Buffer wrapper
#[derive(Debug)]
pub struct WgpuVertexBuffer {
    /// WGPU buffer handle
    buffer: Arc<Buffer>,
    /// Buffer size in bytes
    size: u64,
    /// Number of vertices
    vertex_count: u32,
    /// Vertex stride/size in bytes
    vertex_stride: u32,
    /// Reference count
    ref_count: std::sync::atomic::AtomicU32,
}

impl WgpuVertexBuffer {
    /// Create a new vertex buffer
    pub fn new(
        device: &Device,
        data: &[u8],
        vertex_stride: u32,
        usage: BufferUsages,
    ) -> Result<Self> {
        let size = data.len() as u64;
        let vertex_count = size / vertex_stride as u64;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("WW3D Vertex Buffer"),
            size,
            usage: usage | BufferUsages::VERTEX,
            mapped_at_creation: true,
        });

        // Copy data to buffer
        {
            let mut view = buffer.slice(..).get_mapped_range_mut();
            view.copy_from_slice(data);
        }
        buffer.unmap();

        Ok(Self {
            buffer: Arc::new(buffer),
            size,
            vertex_count: vertex_count as u32,
            vertex_stride,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Create a vertex buffer with initial data
    pub fn with_data<T: Pod>(device: &Device, vertices: &[T], usage: BufferUsages) -> Result<Self> {
        let data = bytemuck::cast_slice(vertices);
        let vertex_stride = std::mem::size_of::<T>() as u32;
        Self::new(device, data, vertex_stride, usage)
    }

    /// Create an empty vertex buffer
    pub fn empty(
        device: &Device,
        vertex_count: u32,
        vertex_stride: u32,
        usage: BufferUsages,
    ) -> Result<Self> {
        let size = vertex_count as u64 * vertex_stride as u64;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("WW3D Empty Vertex Buffer"),
            size,
            usage: usage | BufferUsages::VERTEX,
            mapped_at_creation: false,
        });

        Ok(Self {
            buffer: Arc::new(buffer),
            size,
            vertex_count,
            vertex_stride,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Get the WGPU buffer handle
    pub fn buffer(&self) -> &Arc<Buffer> {
        &self.buffer
    }

    /// Get buffer size in bytes
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get vertex count
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    /// Get vertex stride
    pub fn vertex_stride(&self) -> u32 {
        self.vertex_stride
    }

    /// Update buffer data
    pub fn update_data(&self, queue: &wgpu::Queue, offset: u64, data: &[u8]) -> Result<()> {
        if offset + data.len() as u64 > self.size {
            return Err(Error::BufferOverflow("Buffer size exceeded".to_string()));
        }

        queue.write_buffer(&self.buffer, offset, data);
        Ok(())
    }

    /// Update vertex data
    pub fn update_vertices<T: Pod>(&self, queue: &wgpu::Queue, vertices: &[T]) -> Result<()> {
        let data = bytemuck::cast_slice(vertices);
        self.update_data(queue, 0, data)
    }

    /// Get buffer usage
    pub fn usage(&self) -> BufferUsages {
        // This information isn't directly available from the buffer
        // In practice, we'd need to store this separately
        BufferUsages::VERTEX
    }
}

impl EngineRef for WgpuVertexBuffer {
    fn add_engine_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn release_engine_ref(&self) {
        let old_count = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        if old_count == 1 {
            // Buffer will be dropped when this Arc goes out of scope
        }
    }

    fn engine_ref_count(&self) -> u32 {
        self.ref_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Clone for WgpuVertexBuffer {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            size: self.size,
            vertex_count: self.vertex_count,
            vertex_stride: self.vertex_stride,
            ref_count: std::sync::atomic::AtomicU32::new(
                self.ref_count.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl PartialEq for WgpuVertexBuffer {
    fn eq(&self, other: &Self) -> bool {
        // Compare Arc pointers and other fields
        Arc::ptr_eq(&self.buffer, &other.buffer)
            && self.size == other.size
            && self.vertex_count == other.vertex_count
            && self.vertex_stride == other.vertex_stride
    }
}

/// WGPU Index Buffer wrapper
#[derive(Debug)]
pub struct WgpuIndexBuffer {
    /// WGPU buffer handle
    buffer: Arc<Buffer>,
    /// Buffer size in bytes
    size: u64,
    /// Number of indices
    index_count: u32,
    /// Index format
    index_format: wgpu::IndexFormat,
    /// Reference count
    ref_count: std::sync::atomic::AtomicU32,
}

impl WgpuIndexBuffer {
    /// Create a new index buffer
    pub fn new(
        device: &Device,
        data: &[u8],
        index_format: wgpu::IndexFormat,
        usage: BufferUsages,
    ) -> Result<Self> {
        let size = data.len() as u64;
        let index_count = match index_format {
            wgpu::IndexFormat::Uint16 => size / 2,
            wgpu::IndexFormat::Uint32 => size / 4,
        } as u32;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("WW3D Index Buffer"),
            size,
            usage: usage | BufferUsages::INDEX,
            mapped_at_creation: true,
        });

        // Copy data to buffer
        {
            let mut view = buffer.slice(..).get_mapped_range_mut();
            view.copy_from_slice(data);
        }
        buffer.unmap();

        Ok(Self {
            buffer: Arc::new(buffer),
            size,
            index_count,
            index_format,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Create an index buffer with u16 indices
    pub fn with_u16_indices(device: &Device, indices: &[u16], usage: BufferUsages) -> Result<Self> {
        let data = bytemuck::cast_slice(indices);
        Self::new(device, data, wgpu::IndexFormat::Uint16, usage)
    }

    /// Create an index buffer with u32 indices
    pub fn with_u32_indices(device: &Device, indices: &[u32], usage: BufferUsages) -> Result<Self> {
        let data = bytemuck::cast_slice(indices);
        Self::new(device, data, wgpu::IndexFormat::Uint32, usage)
    }

    /// Create an empty index buffer
    pub fn empty(
        device: &Device,
        index_count: u32,
        index_format: wgpu::IndexFormat,
        usage: BufferUsages,
    ) -> Result<Self> {
        let size = match index_format {
            wgpu::IndexFormat::Uint16 => index_count as u64 * 2,
            wgpu::IndexFormat::Uint32 => index_count as u64 * 4,
        };

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("WW3D Empty Index Buffer"),
            size,
            usage: usage | BufferUsages::INDEX,
            mapped_at_creation: false,
        });

        Ok(Self {
            buffer: Arc::new(buffer),
            size,
            index_count,
            index_format,
            ref_count: std::sync::atomic::AtomicU32::new(1),
        })
    }

    /// Get the WGPU buffer handle
    pub fn buffer(&self) -> &Arc<Buffer> {
        &self.buffer
    }

    /// Get buffer size in bytes
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get index count
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Get index format
    pub fn index_format(&self) -> wgpu::IndexFormat {
        self.index_format
    }

    /// Update buffer data
    pub fn update_data(&self, queue: &wgpu::Queue, offset: u64, data: &[u8]) -> Result<()> {
        if offset + data.len() as u64 > self.size {
            return Err(Error::BufferOverflow("Buffer size exceeded".to_string()));
        }

        queue.write_buffer(&self.buffer, offset, data);
        Ok(())
    }

    /// Update u16 indices
    pub fn update_u16_indices(&self, queue: &wgpu::Queue, indices: &[u16]) -> Result<()> {
        let data = bytemuck::cast_slice(indices);
        self.update_data(queue, 0, data)
    }

    /// Update u32 indices
    pub fn update_u32_indices(&self, queue: &wgpu::Queue, indices: &[u32]) -> Result<()> {
        let data = bytemuck::cast_slice(indices);
        self.update_data(queue, 0, data)
    }
}

impl EngineRef for WgpuIndexBuffer {
    fn add_engine_ref(&self) {
        self.ref_count
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn release_engine_ref(&self) {
        let old_count = self
            .ref_count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
        if old_count == 1 {
            // Buffer will be dropped when this Arc goes out of scope
        }
    }

    fn engine_ref_count(&self) -> u32 {
        self.ref_count.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl Clone for WgpuIndexBuffer {
    fn clone(&self) -> Self {
        Self {
            buffer: Arc::clone(&self.buffer),
            size: self.size,
            index_count: self.index_count,
            index_format: self.index_format,
            ref_count: std::sync::atomic::AtomicU32::new(
                self.ref_count.load(std::sync::atomic::Ordering::Relaxed),
            ),
        }
    }
}

impl PartialEq for WgpuIndexBuffer {
    fn eq(&self, other: &Self) -> bool {
        // Compare Arc pointers and other fields
        Arc::ptr_eq(&self.buffer, &other.buffer)
            && self.size == other.size
            && self.index_count == other.index_count
            && self.index_format == other.index_format
    }
}

/// Implement EngineRef for Arc<WgpuVertexBuffer> for convenience
impl EngineRef for Arc<WgpuVertexBuffer> {
    fn add_engine_ref(&self) {
        (**self).add_engine_ref()
    }

    fn release_engine_ref(&self) {
        (**self).release_engine_ref()
    }

    fn engine_ref_count(&self) -> u32 {
        (**self).engine_ref_count()
    }
}

/// Implement EngineRef for Arc<WgpuIndexBuffer> for convenience
impl EngineRef for Arc<WgpuIndexBuffer> {
    fn add_engine_ref(&self) {
        (**self).add_engine_ref()
    }

    fn release_engine_ref(&self) {
        (**self).release_engine_ref()
    }

    fn engine_ref_count(&self) -> u32 {
        (**self).engine_ref_count()
    }
}

/// Dynamic vertex buffer access class (equivalent to DynamicVBAccessClass)
pub struct DynamicVertexBufferAccess {
    /// Current vertex buffer
    buffer: Option<WgpuVertexBuffer>,
    /// Current offset in vertices
    offset: u32,
    /// Current count of vertices
    count: u32,
}

impl DynamicVertexBufferAccess {
    /// Create new dynamic vertex buffer access
    pub fn new() -> Self {
        Self {
            buffer: None,
            offset: 0,
            count: 0,
        }
    }

    /// Set vertex buffer
    pub fn set_buffer(&mut self, buffer: WgpuVertexBuffer, offset: u32, count: u32) {
        self.buffer = Some(buffer);
        self.offset = offset;
        self.count = count;
    }

    /// Get vertex buffer
    pub fn buffer(&self) -> Option<&WgpuVertexBuffer> {
        self.buffer.as_ref()
    }

    /// Get offset
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Get count
    pub fn count(&self) -> u32 {
        self.count
    }

    /// Reset access
    pub fn reset(&mut self) {
        self.buffer = None;
        self.offset = 0;
        self.count = 0;
    }
}

impl Default for DynamicVertexBufferAccess {
    fn default() -> Self {
        Self::new()
    }
}

/// Dynamic index buffer access class (equivalent to DynamicIBAccessClass)
pub struct DynamicIndexBufferAccess {
    /// Current index buffer
    buffer: Option<WgpuIndexBuffer>,
    /// Current offset in indices
    offset: u32,
}

impl DynamicIndexBufferAccess {
    /// Create new dynamic index buffer access
    pub fn new() -> Self {
        Self {
            buffer: None,
            offset: 0,
        }
    }

    /// Set index buffer
    pub fn set_buffer(&mut self, buffer: WgpuIndexBuffer, offset: u32) {
        self.buffer = Some(buffer);
        self.offset = offset;
    }

    /// Get index buffer
    pub fn buffer(&self) -> Option<&WgpuIndexBuffer> {
        self.buffer.as_ref()
    }

    /// Get offset
    pub fn offset(&self) -> u32 {
        self.offset
    }

    /// Reset access
    pub fn reset(&mut self) {
        self.buffer = None;
        self.offset = 0;
    }
}

impl Default for DynamicIndexBufferAccess {
    fn default() -> Self {
        Self::new()
    }
}

/// Buffer usage flags (equivalent to original buffer type system)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BufferUsage {
    Static,
    Dynamic,
    Stream,
}

/// Vertex buffer creation utilities
pub struct VertexBufferUtils;

impl VertexBufferUtils {
    /// Calculate vertex stride for common vertex formats
    pub fn calculate_stride(vertex_format: &VertexFormat) -> u32 {
        match vertex_format {
            VertexFormat::Position => 12,                    // 3 * f32
            VertexFormat::PositionNormal => 24,              // 3 * f32 + 3 * f32
            VertexFormat::PositionColor => 16,               // 3 * f32 + 4 * u8
            VertexFormat::PositionTexCoord => 20,            // 3 * f32 + 2 * f32
            VertexFormat::PositionNormalTexCoord => 32,      // 3 * f32 + 3 * f32 + 2 * f32
            VertexFormat::PositionNormalColorTexCoord => 36, // 3 * f32 + 3 * f32 + 4 * u8 + 2 * f32
        }
    }

    /// Create vertex buffer with common format
    pub fn create_with_format<T: Pod>(
        device: &Device,
        vertices: &[T],
        format: VertexFormat,
        usage: BufferUsage,
    ) -> Result<WgpuVertexBuffer> {
        let data = bytemuck::cast_slice(vertices);
        let stride = Self::calculate_stride(&format);

        let buffer_usage = match usage {
            BufferUsage::Static => BufferUsages::VERTEX,
            BufferUsage::Dynamic => BufferUsages::VERTEX | BufferUsages::COPY_DST,
            BufferUsage::Stream => BufferUsages::VERTEX | BufferUsages::COPY_DST,
        };

        WgpuVertexBuffer::new(device, data, stride, buffer_usage)
    }
}

/// Index buffer creation utilities
pub struct IndexBufferUtils;

impl IndexBufferUtils {
    /// Create index buffer with optimal format
    pub fn create_optimized(
        device: &Device,
        indices: &[u32],
        usage: BufferUsage,
    ) -> Result<WgpuIndexBuffer> {
        let buffer_usage = match usage {
            BufferUsage::Static => BufferUsages::INDEX,
            BufferUsage::Dynamic => BufferUsages::INDEX | BufferUsages::COPY_DST,
            BufferUsage::Stream => BufferUsages::INDEX | BufferUsages::COPY_DST,
        };

        // Use u16 if possible for better performance
        if indices.iter().all(|&i| i <= u16::MAX as u32) {
            let indices_u16: Vec<u16> = indices.iter().map(|&i| i as u16).collect();
            WgpuIndexBuffer::with_u16_indices(device, &indices_u16, buffer_usage)
        } else {
            WgpuIndexBuffer::with_u32_indices(device, indices, buffer_usage)
        }
    }
}

/// Common vertex format enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum VertexFormat {
    Position,
    PositionNormal,
    PositionColor,
    PositionTexCoord,
    PositionNormalTexCoord,
    PositionNormalColorTexCoord,
}

/// Standard vertex structures
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexPosition {
    pub position: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexPositionNormal {
    pub position: [f32; 3],
    pub normal: [f32; 3],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexPositionColor {
    pub position: [f32; 3],
    pub color: u32,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexPositionTexCoord {
    pub position: [f32; 3],
    pub tex_coord: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexPositionNormalTexCoord {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub tex_coord: [f32; 2],
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct VertexPositionNormalColorTexCoord {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub color: u32,
    pub tex_coord: [f32; 2],
}
