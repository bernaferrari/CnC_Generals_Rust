//! Hardware Capability Detection
//!
//! This module provides hardware capability detection functionality,
//! similar to the original C++ DX8Wrapper capabilities system.

use std::sync::OnceLock;

/// Hardware capabilities structure
#[derive(Debug, Clone)]
pub struct HardwareCaps {
    pub pixel_shader_major_version: u32,
    pub pixel_shader_minor_version: u32,
    pub vertex_shader_major_version: u32,
    pub vertex_shader_minor_version: u32,
    pub supports_dot3: bool,
    pub supports_bump_mapping: bool,
    pub supports_cube_mapping: bool,
    pub max_texture_stages: u32,
    pub max_simultaneous_textures: u32,
}

impl Default for HardwareCaps {
    fn default() -> Self {
        // Modern default capabilities
        Self {
            pixel_shader_major_version: 5,
            pixel_shader_minor_version: 0,
            vertex_shader_major_version: 5,
            vertex_shader_minor_version: 0,
            supports_dot3: true,
            supports_bump_mapping: true,
            supports_cube_mapping: true,
            max_texture_stages: 16,
            max_simultaneous_textures: 16,
        }
    }
}

impl HardwareCaps {
    /// Detect current hardware capabilities
    pub fn detect() -> Self {
        // In a real implementation, this would query the graphics API
        // For now, return modern capabilities
        Self::default()
    }

    /// Check if hardware supports pixel shaders of a specific version
    pub fn supports_pixel_shader(&self, major: u32, minor: u32) -> bool {
        self.pixel_shader_major_version > major
            || (self.pixel_shader_major_version == major
                && self.pixel_shader_minor_version >= minor)
    }

    /// Check if hardware supports vertex shaders of a specific version
    pub fn supports_vertex_shader(&self, major: u32, minor: u32) -> bool {
        self.vertex_shader_major_version > major
            || (self.vertex_shader_major_version == major
                && self.vertex_shader_minor_version >= minor)
    }
}

/// Global hardware capabilities singleton
static HARDWARE_CAPS: OnceLock<HardwareCaps> = OnceLock::new();

/// Initialize hardware capabilities
pub fn init_hardware_caps() {
    let _ = HARDWARE_CAPS.get_or_init(HardwareCaps::detect);
}

/// Get current hardware capabilities
pub fn get_hardware_caps() -> HardwareCaps {
    HARDWARE_CAPS.get_or_init(HardwareCaps::detect).clone()
}
