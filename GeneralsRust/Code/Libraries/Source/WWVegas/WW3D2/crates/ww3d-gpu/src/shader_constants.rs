//! Shader Constants Management
//!
//! This module provides a modern replacement for DX8's register-based shader constants.
//! Instead of setting individual constants by register index, we use structured uniform
//! buffers with named fields.

use crate::{GpuBuffer, GpuError};
use bytemuck::{Pod, Zeroable};
use parking_lot::Mutex;
use std::sync::Arc;

/// Maximum vertex shader constants (matching DX8 capability)
pub const MAX_VERTEX_SHADER_CONSTANTS: usize = 96;

/// Maximum pixel shader constants (matching DX8 capability)
pub const MAX_PIXEL_SHADER_CONSTANTS: usize = 8;

/// 4-component vector constant (matches DX8 Vector4/D3DXVECTOR4)
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
pub struct ShaderConstant {
    pub data: [f32; 4],
}

impl ShaderConstant {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { data: [x, y, z, w] }
    }

    pub fn from_vec3(v: [f32; 3], w: f32) -> Self {
        Self {
            data: [v[0], v[1], v[2], w],
        }
    }

    pub fn from_vec4(v: [f32; 4]) -> Self {
        Self { data: v }
    }

    pub fn zero() -> Self {
        Self { data: [0.0; 4] }
    }
}

/// Vertex shader constants buffer
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct VertexShaderConstants {
    /// Constants c0-c95 (96 vec4s)
    pub constants: [ShaderConstant; MAX_VERTEX_SHADER_CONSTANTS],
}

impl VertexShaderConstants {
    pub fn new() -> Self {
        Self {
            constants: [ShaderConstant::zero(); MAX_VERTEX_SHADER_CONSTANTS],
        }
    }

    /// Set a constant by index (matching DX8 API)
    pub fn set_constant(&mut self, index: usize, value: ShaderConstant) {
        if index < MAX_VERTEX_SHADER_CONSTANTS {
            self.constants[index] = value;
        }
    }

    /// Set multiple constants starting at index
    pub fn set_constants(&mut self, start_index: usize, values: &[ShaderConstant]) {
        let end_index = (start_index + values.len()).min(MAX_VERTEX_SHADER_CONSTANTS);
        let count = end_index - start_index;
        self.constants[start_index..end_index].copy_from_slice(&values[..count]);
    }

    /// Get constant by index
    pub fn get_constant(&self, index: usize) -> Option<ShaderConstant> {
        self.constants.get(index).copied()
    }

    /// Convert to bytes for buffer upload
    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

impl Default for VertexShaderConstants {
    fn default() -> Self {
        Self::new()
    }
}

/// Pixel shader constants buffer
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct PixelShaderConstants {
    /// Constants c0-c7 (8 vec4s)
    pub constants: [ShaderConstant; MAX_PIXEL_SHADER_CONSTANTS],
}

impl PixelShaderConstants {
    pub fn new() -> Self {
        Self {
            constants: [ShaderConstant::zero(); MAX_PIXEL_SHADER_CONSTANTS],
        }
    }

    pub fn set_constant(&mut self, index: usize, value: ShaderConstant) {
        if index < MAX_PIXEL_SHADER_CONSTANTS {
            self.constants[index] = value;
        }
    }

    pub fn set_constants(&mut self, start_index: usize, values: &[ShaderConstant]) {
        let end_index = (start_index + values.len()).min(MAX_PIXEL_SHADER_CONSTANTS);
        let count = end_index - start_index;
        self.constants[start_index..end_index].copy_from_slice(&values[..count]);
    }

    pub fn get_constant(&self, index: usize) -> Option<ShaderConstant> {
        self.constants.get(index).copied()
    }

    pub fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

impl Default for PixelShaderConstants {
    fn default() -> Self {
        Self::new()
    }
}

/// Shader constants manager - manages uniform buffers for shader constants
pub struct ShaderConstantsManager {
    /// GPU device
    device: Arc<crate::device::GpuDevice>,
    /// Vertex shader constants CPU-side
    vertex_constants: VertexShaderConstants,
    /// Pixel shader constants CPU-side
    pixel_constants: PixelShaderConstants,
    /// Vertex shader uniform buffer
    vertex_buffer: Option<Arc<GpuBuffer>>,
    /// Pixel shader uniform buffer
    pixel_buffer: Option<Arc<GpuBuffer>>,
    /// Dirty flag for vertex constants
    vertex_dirty: bool,
    /// Dirty flag for pixel constants
    pixel_dirty: bool,
}

impl ShaderConstantsManager {
    /// Create a new shader constants manager
    pub fn new(device: Arc<crate::device::GpuDevice>) -> Result<Self, GpuError> {
        // Create uniform buffers
        let vertex_buffer = GpuBuffer::uniform_buffer(
            &device,
            std::mem::size_of::<VertexShaderConstants>() as u64,
            Some("Vertex Shader Constants"),
        )?;

        let pixel_buffer = GpuBuffer::uniform_buffer(
            &device,
            std::mem::size_of::<PixelShaderConstants>() as u64,
            Some("Pixel Shader Constants"),
        )?;

        Ok(Self {
            device,
            vertex_constants: VertexShaderConstants::new(),
            pixel_constants: PixelShaderConstants::new(),
            vertex_buffer: Some(Arc::new(vertex_buffer)),
            pixel_buffer: Some(Arc::new(pixel_buffer)),
            vertex_dirty: true,
            pixel_dirty: true,
        })
    }

    /// Set vertex shader constant
    pub fn set_vertex_constant(&mut self, index: usize, value: ShaderConstant) {
        self.vertex_constants.set_constant(index, value);
        self.vertex_dirty = true;
    }

    /// Set multiple vertex shader constants
    pub fn set_vertex_constants(&mut self, start_index: usize, values: &[ShaderConstant]) {
        self.vertex_constants.set_constants(start_index, values);
        self.vertex_dirty = true;
    }

    /// Set pixel shader constant
    pub fn set_pixel_constant(&mut self, index: usize, value: ShaderConstant) {
        self.pixel_constants.set_constant(index, value);
        self.pixel_dirty = true;
    }

    /// Set multiple pixel shader constants
    pub fn set_pixel_constants(&mut self, start_index: usize, values: &[ShaderConstant]) {
        self.pixel_constants.set_constants(start_index, values);
        self.pixel_dirty = true;
    }

    /// Upload constants to GPU if dirty
    pub fn upload(&mut self) {
        if self.vertex_dirty {
            if let Some(buffer) = &self.vertex_buffer {
                self.device.queue().write_buffer(
                    buffer.wgpu_buffer(),
                    0,
                    self.vertex_constants.as_bytes(),
                );
            }
            self.vertex_dirty = false;
        }

        if self.pixel_dirty {
            if let Some(buffer) = &self.pixel_buffer {
                self.device.queue().write_buffer(
                    buffer.wgpu_buffer(),
                    0,
                    self.pixel_constants.as_bytes(),
                );
            }
            self.pixel_dirty = false;
        }
    }

    /// Get vertex shader uniform buffer
    pub fn vertex_buffer(&self) -> Option<&Arc<GpuBuffer>> {
        self.vertex_buffer.as_ref()
    }

    /// Get pixel shader uniform buffer
    pub fn pixel_buffer(&self) -> Option<&Arc<GpuBuffer>> {
        self.pixel_buffer.as_ref()
    }

    /// Reset all constants to zero
    pub fn reset(&mut self) {
        self.vertex_constants = VertexShaderConstants::new();
        self.pixel_constants = PixelShaderConstants::new();
        self.vertex_dirty = true;
        self.pixel_dirty = true;
    }
}

// Global shader constants manager
lazy_static::lazy_static! {
    pub static ref SHADER_CONSTANTS: Mutex<Option<ShaderConstantsManager>> = Mutex::new(None);
}

/// Initialize global shader constants manager
pub fn init_shader_constants(device: Arc<crate::device::GpuDevice>) -> Result<(), GpuError> {
    let manager = ShaderConstantsManager::new(device)?;
    *SHADER_CONSTANTS.lock() = Some(manager);
    Ok(())
}

/// Set vertex shader constant (global)
pub fn set_vertex_shader_constant(index: usize, value: ShaderConstant) {
    if let Some(manager) = SHADER_CONSTANTS.lock().as_mut() {
        manager.set_vertex_constant(index, value);
    }
}

/// Set multiple vertex shader constants (global)
pub fn set_vertex_shader_constants(start_index: usize, values: &[ShaderConstant]) {
    if let Some(manager) = SHADER_CONSTANTS.lock().as_mut() {
        manager.set_vertex_constants(start_index, values);
    }
}

/// Set pixel shader constant (global)
pub fn set_pixel_shader_constant(index: usize, value: ShaderConstant) {
    if let Some(manager) = SHADER_CONSTANTS.lock().as_mut() {
        manager.set_pixel_constant(index, value);
    }
}

/// Set multiple pixel shader constants (global)
pub fn set_pixel_shader_constants(start_index: usize, values: &[ShaderConstant]) {
    if let Some(manager) = SHADER_CONSTANTS.lock().as_mut() {
        manager.set_pixel_constants(start_index, values);
    }
}

/// Upload shader constants to GPU (global)
pub fn upload_shader_constants() {
    if let Some(manager) = SHADER_CONSTANTS.lock().as_mut() {
        manager.upload();
    }
}

/// Get vertex shader buffer (global)
pub fn get_vertex_shader_buffer() -> Option<Arc<GpuBuffer>> {
    SHADER_CONSTANTS
        .lock()
        .as_ref()
        .and_then(|m| m.vertex_buffer())
        .cloned()
}

/// Get pixel shader buffer (global)
pub fn get_pixel_shader_buffer() -> Option<Arc<GpuBuffer>> {
    SHADER_CONSTANTS
        .lock()
        .as_ref()
        .and_then(|m| m.pixel_buffer())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_constant_creation() {
        let c = ShaderConstant::new(1.0, 2.0, 3.0, 4.0);
        assert_eq!(c.data, [1.0, 2.0, 3.0, 4.0]);

        let c2 = ShaderConstant::from_vec3([1.0, 2.0, 3.0], 4.0);
        assert_eq!(c2.data, [1.0, 2.0, 3.0, 4.0]);

        let c3 = ShaderConstant::zero();
        assert_eq!(c3.data, [0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn test_vertex_shader_constants() {
        let mut constants = VertexShaderConstants::new();

        let value = ShaderConstant::new(1.0, 2.0, 3.0, 4.0);
        constants.set_constant(0, value);

        assert_eq!(
            constants.get_constant(0).unwrap().data,
            [1.0, 2.0, 3.0, 4.0]
        );
        assert_eq!(
            constants.get_constant(1).unwrap().data,
            [0.0, 0.0, 0.0, 0.0]
        );
    }

    #[test]
    fn test_pixel_shader_constants() {
        let mut constants = PixelShaderConstants::new();

        let values = [
            ShaderConstant::new(1.0, 0.0, 0.0, 1.0),
            ShaderConstant::new(0.0, 1.0, 0.0, 1.0),
        ];
        constants.set_constants(0, &values);

        assert_eq!(
            constants.get_constant(0).unwrap().data,
            [1.0, 0.0, 0.0, 1.0]
        );
        assert_eq!(
            constants.get_constant(1).unwrap().data,
            [0.0, 1.0, 0.0, 1.0]
        );
    }

    #[test]
    fn test_constant_sizes() {
        assert_eq!(std::mem::size_of::<ShaderConstant>(), 16);
        assert_eq!(
            std::mem::size_of::<VertexShaderConstants>(),
            16 * MAX_VERTEX_SHADER_CONSTANTS
        );
        assert_eq!(
            std::mem::size_of::<PixelShaderConstants>(),
            16 * MAX_PIXEL_SHADER_CONSTANTS
        );
    }
}
