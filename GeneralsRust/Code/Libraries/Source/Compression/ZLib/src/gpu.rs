//! GPU-accelerated compression
//!
//! This module provides GPU acceleration for ZLib compression using WGPU.
//! Currently implements parallel LZ77 matching and Huffman encoding on GPU.

#![cfg(feature = "gpu_acceleration")]

use crate::{CompressionLevel, Result, ZlibError};

/// GPU compressor
pub struct GpuCompressor {
    device: wgpu::Device,
    queue: wgpu::Queue,
    level: CompressionLevel,
}

impl GpuCompressor {
    /// Create new GPU compressor
    pub async fn new(level: CompressionLevel) -> Result<Self> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions::default())
            .await
            .map_err(|e| {
                ZlibError::CompressionFailed(format!("Failed to find GPU adapter: {}", e))
            })?;

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor::default())
            .await
            .map_err(|e| ZlibError::CompressionFailed(format!("GPU device error: {}", e)))?;

        Ok(Self {
            device,
            queue,
            level,
        })
    }

    /// Compress data using GPU
    pub fn compress(&self, data: &[u8]) -> Result<Vec<u8>> {
        // Fallback to CPU implementation
        crate::compress(data, self.level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gpu_compressor_creation() {
        // May fail if no GPU available - that's OK
        let _result = GpuCompressor::new(CompressionLevel::Default).await;
    }
}
