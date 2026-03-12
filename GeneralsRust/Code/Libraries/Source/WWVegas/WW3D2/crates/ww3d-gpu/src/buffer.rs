//! GPU Buffer Resource Management
//!
//! This module provides GPU buffer creation, management, and data transfer
//! functionality for vertex buffers, index buffers, uniform buffers, and storage buffers.

use crate::*;
use std::sync::Arc;

/// Memory type enumeration (local copy for buffer.rs)
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryType {
    /// Device local memory (fast GPU access)
    DeviceLocal,
    /// Host visible memory (CPU can write, GPU can read)
    HostVisible,
    /// Host coherent memory (no cache flushing required)
    HostCoherent,
}

/// GPU buffer abstraction
#[derive(Debug)]
pub struct GpuBuffer {
    /// Underlying WGPU buffer
    buffer: wgpu::Buffer,
    /// Buffer size in bytes
    size: u64,
    /// Buffer usage flags
    usage: wgpu::BufferUsages,
    /// Memory type
    memory_type: MemoryType,
    /// Buffer label for debugging
    label: Option<String>,
    /// Last update timestamp
    last_update: std::time::Instant,
}

impl GpuBuffer {
    /// Create a new GPU buffer
    pub fn new(
        device: &crate::device::GpuDevice,
        desc: &wgpu::BufferDescriptor,
    ) -> Result<Self, GpuError> {
        let wgpu_buffer = device.create_buffer(desc)?;
        let memory_type = Self::infer_memory_type(desc.usage);

        Ok(Self {
            buffer: wgpu_buffer.buffer,
            size: desc.size,
            usage: desc.usage,
            memory_type,
            label: desc.label.map(|s| s.to_string()),
            last_update: std::time::Instant::now(),
        })
    }

    /// Create a vertex buffer
    pub fn vertex_buffer(
        device: &crate::device::GpuDevice,
        size: u64,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        Self::new(device, &desc)
    }

    /// Create an index buffer
    pub fn index_buffer(
        device: &crate::device::GpuDevice,
        size: u64,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        Self::new(device, &desc)
    }

    /// Create a uniform buffer
    pub fn uniform_buffer(
        device: &crate::device::GpuDevice,
        size: u64,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        };

        Self::new(device, &desc)
    }

    /// Create a storage buffer
    pub fn storage_buffer(
        device: &crate::device::GpuDevice,
        size: u64,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::BufferDescriptor {
            label,
            size,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        };

        Self::new(device, &desc)
    }

    /// Create a buffer with initial data
    pub fn with_data(
        device: &crate::device::GpuDevice,
        data: &[u8],
        usage: wgpu::BufferUsages,
        label: Option<&str>,
    ) -> Result<Self, GpuError> {
        let desc = wgpu::BufferDescriptor {
            label,
            size: data.len() as u64,
            usage,
            mapped_at_creation: false,
        };

        let mut buffer = Self::new(device, &desc)?;
        buffer.write_data(device, data);
        Ok(buffer)
    }

    /// Write data to the buffer
    pub fn write_data(&mut self, device: &crate::device::GpuDevice, data: &[u8]) {
        if data.len() as u64 > self.size {
            panic!("Data size {} exceeds buffer size {}", data.len(), self.size);
        }

        device.queue().write_buffer(&self.buffer, 0, data);
        self.last_update = std::time::Instant::now();
    }

    /// Write data to a specific offset in the buffer
    pub fn write_data_offset(
        &mut self,
        device: &crate::device::GpuDevice,
        data: &[u8],
        offset: u64,
    ) {
        if offset + data.len() as u64 > self.size {
            panic!("Data write exceeds buffer bounds");
        }

        device.queue().write_buffer(&self.buffer, offset, data);
        self.last_update = std::time::Instant::now();
    }

    /// Read data from the buffer (requires COPY_SRC usage)
    pub async fn read_data(
        &self,
        device: &crate::device::GpuDevice,
        size: u64,
    ) -> Result<Vec<u8>, GpuError> {
        if !self.usage.contains(wgpu::BufferUsages::COPY_SRC) {
            return Err(GpuError::InvalidOperation(
                "Buffer does not support reading".to_string(),
            ));
        }

        // Create a staging buffer for reading
        let staging_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        })?;

        // Copy from GPU buffer to staging buffer
        let mut encoder = device.create_command_encoder(Some("Read Buffer"));
        encoder.copy_buffer_to_buffer(&self.buffer, 0, &staging_buffer.buffer, 0, size);
        device.submit(vec![encoder.finish()]);

        // Map the staging buffer and read the data
        let buffer_slice = staging_buffer.buffer.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();

        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).unwrap();
        });

        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        rx.recv()
            .unwrap()
            .map_err(|_| GpuError::InvalidOperation("Failed to map buffer".to_string()))?;

        let data = buffer_slice.get_mapped_range().to_vec();
        staging_buffer.buffer.unmap();

        Ok(data)
    }

    /// Get the underlying WGPU buffer
    pub fn wgpu_buffer(&self) -> &wgpu::Buffer {
        &self.buffer
    }

    /// Get buffer size
    pub fn size(&self) -> u64 {
        self.size
    }

    /// Get buffer usage
    pub fn usage(&self) -> wgpu::BufferUsages {
        self.usage
    }

    /// Get memory type
    pub fn memory_type(&self) -> MemoryType {
        self.memory_type
    }

    /// Get buffer label
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Check if buffer is vertex buffer
    pub fn is_vertex_buffer(&self) -> bool {
        self.usage.contains(wgpu::BufferUsages::VERTEX)
    }

    /// Check if buffer is index buffer
    pub fn is_index_buffer(&self) -> bool {
        self.usage.contains(wgpu::BufferUsages::INDEX)
    }

    /// Check if buffer is uniform buffer
    pub fn is_uniform_buffer(&self) -> bool {
        self.usage.contains(wgpu::BufferUsages::UNIFORM)
    }

    /// Check if buffer is storage buffer
    pub fn is_storage_buffer(&self) -> bool {
        self.usage.contains(wgpu::BufferUsages::STORAGE)
    }

    /// Get time since last update
    pub fn time_since_update(&self) -> std::time::Duration {
        self.last_update.elapsed()
    }

    /// Infer memory type from usage
    fn infer_memory_type(usage: wgpu::BufferUsages) -> MemoryType {
        if usage.contains(wgpu::BufferUsages::UNIFORM)
            || usage.contains(wgpu::BufferUsages::STORAGE)
        {
            MemoryType::DeviceLocal
        } else if usage.contains(wgpu::BufferUsages::COPY_DST) {
            MemoryType::HostVisible
        } else {
            MemoryType::DeviceLocal
        }
    }
}

/// Buffer manager for handling multiple buffers
#[derive(Debug)]
pub struct BufferManager {
    /// GPU device reference
    device: Arc<crate::device::GpuDevice>,
    /// Managed buffers
    buffers: Vec<Arc<GpuBuffer>>,
    /// Staging buffers for data transfer
    staging_buffers: Vec<GpuBuffer>,
    /// Buffer allocation statistics
    stats: BufferStats,
}

impl BufferManager {
    /// Create a new buffer manager
    pub fn new(device: Arc<crate::device::GpuDevice>) -> Self {
        Self {
            device,
            buffers: Vec::new(),
            staging_buffers: Vec::new(),
            stats: BufferStats::default(),
        }
    }

    /// Create a vertex buffer
    pub fn create_vertex_buffer(
        &mut self,
        size: u64,
        label: Option<&str>,
    ) -> Result<Arc<GpuBuffer>, GpuError> {
        let buffer = GpuBuffer::vertex_buffer(&self.device, size, label)?;
        let buffer_arc = Arc::new(buffer);
        self.buffers.push(buffer_arc.clone());
        self.update_stats();
        Ok(buffer_arc)
    }

    /// Create an index buffer
    pub fn create_index_buffer(
        &mut self,
        size: u64,
        label: Option<&str>,
    ) -> Result<Arc<GpuBuffer>, GpuError> {
        let buffer = GpuBuffer::index_buffer(&self.device, size, label)?;
        let buffer_arc = Arc::new(buffer);
        self.buffers.push(buffer_arc.clone());
        self.update_stats();
        Ok(buffer_arc)
    }

    /// Create a uniform buffer
    pub fn create_uniform_buffer(
        &mut self,
        size: u64,
        label: Option<&str>,
    ) -> Result<Arc<GpuBuffer>, GpuError> {
        let buffer = GpuBuffer::uniform_buffer(&self.device, size, label)?;
        let buffer_arc = Arc::new(buffer);
        self.buffers.push(buffer_arc.clone());
        self.update_stats();
        Ok(buffer_arc)
    }

    /// Create a storage buffer
    pub fn create_storage_buffer(
        &mut self,
        size: u64,
        label: Option<&str>,
    ) -> Result<Arc<GpuBuffer>, GpuError> {
        let buffer = GpuBuffer::storage_buffer(&self.device, size, label)?;
        let buffer_arc = Arc::new(buffer);
        self.buffers.push(buffer_arc.clone());
        self.update_stats();
        Ok(buffer_arc)
    }

    /// Create a buffer with data
    pub fn create_buffer_with_data(
        &mut self,
        data: &[u8],
        usage: wgpu::BufferUsages,
        label: Option<&str>,
    ) -> Result<Arc<GpuBuffer>, GpuError> {
        let buffer = GpuBuffer::with_data(&self.device, data, usage, label)?;
        let buffer_arc = Arc::new(buffer);
        self.buffers.push(buffer_arc.clone());
        self.update_stats();
        Ok(buffer_arc)
    }

    /// Get a staging buffer for data transfer
    pub fn get_staging_buffer(&mut self, size: u64) -> Result<&mut GpuBuffer, GpuError> {
        // Find an existing staging buffer that fits
        let existing_index = self
            .staging_buffers
            .iter()
            .position(|staging| staging.size >= size);

        if let Some(index) = existing_index {
            return Ok(&mut self.staging_buffers[index]);
        }

        // Create a new staging buffer
        let buffer = GpuBuffer::with_data(
            &self.device,
            &vec![0u8; size as usize],
            wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::MAP_READ
                | wgpu::BufferUsages::MAP_WRITE,
            Some("Staging Buffer"),
        )?;

        self.staging_buffers.push(buffer);
        Ok(self.staging_buffers.last_mut().unwrap())
    }

    /// Update buffer data using staging buffer
    pub async fn update_buffer_data(
        &mut self,
        buffer: &GpuBuffer,
        data: &[u8],
        offset: u64,
    ) -> Result<(), GpuError> {
        if offset + data.len() as u64 > buffer.size {
            return Err(GpuError::InvalidOperation(
                "Data exceeds buffer bounds".to_string(),
            ));
        }

        // Get staging buffer index first
        let staging_size = data.len() as u64;
        let staging_index = {
            // Find existing staging buffer
            let mut found = None;
            for (i, staging) in self.staging_buffers.iter().enumerate() {
                if staging.size >= staging_size {
                    found = Some(i);
                    break;
                }
            }
            found
        };

        let staging = if let Some(index) = staging_index {
            &mut self.staging_buffers[index]
        } else {
            // Create new staging buffer
            let staging_desc = wgpu::BufferDescriptor {
                label: Some("Staging Buffer"),
                size: staging_size.max(65536),
                usage: wgpu::BufferUsages::MAP_WRITE | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            };
            let wgpu_staging_buffer = self.device.create_buffer(&staging_desc)?;
            let new_staging = GpuBuffer {
                buffer: wgpu_staging_buffer.buffer,
                size: staging_desc.size,
                usage: staging_desc.usage,
                memory_type: MemoryType::HostVisible,
                label: staging_desc.label.map(|s| s.to_string()),
                last_update: std::time::Instant::now(),
            };
            self.staging_buffers.push(new_staging);
            self.staging_buffers.last_mut().unwrap()
        };

        staging.write_data_offset(&self.device, data, 0);

        // Create encoder and submit
        let mut encoder = self.device.create_command_encoder(Some("Update Buffer"));
        encoder.copy_buffer_to_buffer(
            &staging.buffer,
            0,
            &buffer.buffer,
            offset,
            data.len() as u64,
        );
        self.device.submit(vec![encoder.finish()]);

        Ok(())
    }

    /// Get buffer statistics
    pub fn stats(&self) -> &BufferStats {
        &self.stats
    }

    /// Update statistics
    fn update_stats(&mut self) {
        self.stats.buffer_count = self.buffers.len();
        self.stats.total_memory = self.buffers.iter().map(|b| b.size).sum();
        self.stats.vertex_buffer_count =
            self.buffers.iter().filter(|b| b.is_vertex_buffer()).count();
        self.stats.index_buffer_count = self.buffers.iter().filter(|b| b.is_index_buffer()).count();
        self.stats.uniform_buffer_count = self
            .buffers
            .iter()
            .filter(|b| b.is_uniform_buffer())
            .count();
        self.stats.storage_buffer_count = self
            .buffers
            .iter()
            .filter(|b| b.is_storage_buffer())
            .count();
    }

    /// Cleanup unused buffers
    pub fn cleanup(&mut self) {
        // Remove buffers that haven't been used recently
        let cutoff = std::time::Duration::from_secs(60); // 1 minute
        self.buffers
            .retain(|buffer| buffer.time_since_update() < cutoff);
        self.update_stats();
    }
}

/// Buffer statistics
#[derive(Debug, Clone, Default)]
pub struct BufferStats {
    pub buffer_count: usize,
    pub total_memory: u64,
    pub vertex_buffer_count: usize,
    pub index_buffer_count: usize,
    pub uniform_buffer_count: usize,
    pub storage_buffer_count: usize,
}

/// Buffer usage utilities
pub struct BufferUsage;

impl BufferUsage {
    /// Vertex buffer usage
    pub fn vertex() -> wgpu::BufferUsages {
        wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST
    }

    /// Index buffer usage
    pub fn index() -> wgpu::BufferUsages {
        wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST
    }

    /// Uniform buffer usage
    pub fn uniform() -> wgpu::BufferUsages {
        wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST
    }

    /// Storage buffer usage
    pub fn storage() -> wgpu::BufferUsages {
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC
    }

    /// Staging buffer usage
    pub fn staging() -> wgpu::BufferUsages {
        wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST
            | wgpu::BufferUsages::MAP_READ
            | wgpu::BufferUsages::MAP_WRITE
    }
}

lazy_static::lazy_static! {
    /// Vertex buffer usage constant
    pub static ref VERTEX: wgpu::BufferUsages = BufferUsage::vertex();
    /// Index buffer usage constant
    pub static ref INDEX: wgpu::BufferUsages = BufferUsage::index();
    /// Uniform buffer usage constant
    pub static ref UNIFORM: wgpu::BufferUsages = BufferUsage::uniform();
    /// Storage buffer usage constant
    pub static ref STORAGE: wgpu::BufferUsages = BufferUsage::storage();
    /// Staging buffer usage constant
    pub static ref STAGING: wgpu::BufferUsages = BufferUsage::staging();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_usage_constants() {
        assert!(VERTEX.contains(wgpu::BufferUsages::VERTEX));
        assert!(INDEX.contains(wgpu::BufferUsages::INDEX));
        assert!(UNIFORM.contains(wgpu::BufferUsages::UNIFORM));
        assert!(STORAGE.contains(wgpu::BufferUsages::STORAGE));
        assert!(STAGING.contains(wgpu::BufferUsages::MAP_READ));
    }

    #[test]
    fn test_buffer_stats() {
        let stats = BufferStats::default();
        assert_eq!(stats.buffer_count, 0);
        assert_eq!(stats.total_memory, 0);
        assert_eq!(stats.vertex_buffer_count, 0);
    }

    #[test]
    fn test_memory_types() {
        assert_eq!(format!("{:?}", MemoryType::DeviceLocal), "DeviceLocal");
        assert_eq!(format!("{:?}", MemoryType::HostVisible), "HostVisible");
        assert_eq!(format!("{:?}", MemoryType::HostCoherent), "HostCoherent");
    }
}
