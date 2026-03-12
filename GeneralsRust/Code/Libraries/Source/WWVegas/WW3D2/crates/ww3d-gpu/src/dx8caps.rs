//! DirectX 8 Capabilities and Feature Detection
//!
//! This module provides capabilities detection and feature querying
//! equivalent to the original DX8Caps functionality.

use wgpu::Adapter;

/// DX8-style capabilities structure
#[derive(Debug, Clone)]
pub struct DX8Caps {
    pub device_name: String,
    pub driver_version: String,
    pub vendor_id: u32,
    pub device_id: u32,
    pub subsys_id: u32,
    pub revision: u32,

    // Hardware capabilities
    pub max_texture_width: u32,
    pub max_texture_height: u32,
    pub max_texture_aspect_ratio: u32,
    pub max_texture_repeat: u32,
    pub max_texture_stages: u32,
    pub max_simultaneous_textures: u32,
    pub max_active_lights: u32,
    pub max_user_clip_planes: u32,
    pub max_vertex_index: u32,
    pub max_primitive_count: u32,

    // Texture format support
    pub supports_dxt1: bool,
    pub supports_dxt3: bool,
    pub supports_dxt5: bool,
    pub supports_rgba8: bool,
    pub supports_rgb8: bool,

    // Shader capabilities
    pub vertex_shader_version: u32,
    pub pixel_shader_version: u32,

    // Other features
    pub supports_anisotropic_filtering: bool,
    pub max_anisotropy: u32,
    pub supports_vertex_buffer: bool,
    pub supports_index_buffer: bool,
}

impl DX8Caps {
    /// Create DX8Caps from WGPU adapter information
    pub fn from_adapter(adapter: &Adapter) -> Self {
        let info = adapter.get_info();
        let limits = adapter.limits();

        Self {
            device_name: info.name.clone(),
            driver_version: "1.0".to_string(), // Simplified for WGPU compatibility
            vendor_id: info.vendor,
            device_id: 0, // WGPU doesn't provide device ID
            subsys_id: 0,
            revision: 0,

            max_texture_width: limits.max_texture_dimension_2d,
            max_texture_height: limits.max_texture_dimension_2d,
            max_texture_aspect_ratio: limits.max_texture_dimension_2d,
            max_texture_repeat: limits.max_texture_dimension_2d,
            max_texture_stages: 8, // Typical DX8 limit
            max_simultaneous_textures: 8,
            max_active_lights: 8,
            max_user_clip_planes: 6, // WGPU doesn't expose this directly
            max_vertex_index: (limits.max_buffer_size / 4).min(u32::MAX as u64) as u32, // Rough estimate
            max_primitive_count: (limits.max_buffer_size / 12).min(u32::MAX as u64) as u32, // Rough estimate (3 vertices per triangle, 4 bytes per vertex)

            supports_dxt1: true, // WGPU supports compressed textures
            supports_dxt3: true,
            supports_dxt5: true,
            supports_rgba8: true,
            supports_rgb8: true,

            vertex_shader_version: 0x0101, // Equivalent to vs_1_1
            pixel_shader_version: 0x0101,  // Equivalent to ps_1_1

            supports_anisotropic_filtering: true,
            max_anisotropy: 16,
            supports_vertex_buffer: true,
            supports_index_buffer: true,
        }
    }

    /// Check if a specific texture format is supported
    pub fn supports_texture_format(&self, format: TextureFormat) -> bool {
        match format {
            TextureFormat::DXT1 => self.supports_dxt1,
            TextureFormat::DXT3 => self.supports_dxt3,
            TextureFormat::DXT5 => self.supports_dxt5,
            TextureFormat::RGBA8 => self.supports_rgba8,
            TextureFormat::RGB8 => self.supports_rgb8,
        }
    }

    /// Get maximum texture size supported
    pub fn get_max_texture_size(&self) -> u32 {
        self.max_texture_width.min(self.max_texture_height)
    }
}

/// Texture format enumeration
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TextureFormat {
    DXT1,
    DXT3,
    DXT5,
    RGBA8,
    RGB8,
}

/// Convert DX8 texture format to WGPU format
pub fn dx8_format_to_wgpu(format: TextureFormat) -> Option<wgpu::TextureFormat> {
    match format {
        TextureFormat::DXT1 => Some(wgpu::TextureFormat::Bc1RgbaUnorm),
        TextureFormat::DXT3 => Some(wgpu::TextureFormat::Bc2RgbaUnorm),
        TextureFormat::DXT5 => Some(wgpu::TextureFormat::Bc3RgbaUnorm),
        TextureFormat::RGBA8 => Some(wgpu::TextureFormat::Rgba8Unorm),
        TextureFormat::RGB8 => Some(wgpu::TextureFormat::Rgba8Unorm), // WGPU doesn't have RGB8, use RGBA8
    }
}