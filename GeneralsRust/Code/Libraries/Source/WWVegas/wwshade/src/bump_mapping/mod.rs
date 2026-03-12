//! Bump Mapping Shader Module
//!
//! This module contains bump mapping shader implementations, including both
//! diffuse-only and specular bump mapping shaders. These are Rust ports of
//! the original C++ WW3D bump mapping shaders from Command & Conquer Generals.

pub mod bump_diff;
pub mod bump_spec;

pub use bump_diff::BumpDiffShaderDef;
pub use bump_spec::BumpSpecShaderDef;

use crate::error::{ShdError, ShdResult};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};

/// Shared constants for bump mapping shaders
pub mod constants {
    /// Chunk ID for shader variables during serialization
    pub const CHUNKID_VARIABLES: u32 = 0x16490450;

    /// Variable IDs for serialization
    pub const VARID_TEXTURE_NAME: u8 = 0x00;
    pub const VARID_BUMP_MAP_NAME: u8 = 0x01;
    pub const VARID_AMBIENT_COLOR: u8 = 0x02;
    pub const VARID_DIFFUSE_COLOR: u8 = 0x03;
    pub const VARID_SPECULAR_COLOR: u8 = 0x04;
    pub const VARID_DIFFUSE_BUMPINESS: u8 = 0x05;
    pub const VARID_SPECULAR_BUMPINESS: u8 = 0x06;
}

/// Supported shader hardware versions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ShaderVersion {
    /// DirectX 6 compatible version (legacy fallback)
    V6,
    /// DirectX 7 compatible version with DOT3 support
    V7,
    /// DirectX 8 compatible version with pixel shaders
    V8,
    /// Modern version using compute shaders and advanced features
    Modern,
}

impl Default for ShaderVersion {
    fn default() -> Self {
        ShaderVersion::Modern
    }
}

impl ShaderVersion {
    /// Determine the best shader version based on hardware capabilities
    pub fn detect_best_version() -> Self {
        // In a real implementation, this would query hardware capabilities
        // For now, we default to modern shaders
        ShaderVersion::Modern
    }

    /// Check if this shader version supports advanced features
    pub fn supports_pixel_shaders(&self) -> bool {
        matches!(self, ShaderVersion::V8 | ShaderVersion::Modern)
    }

    /// Check if this shader version supports DOT3 operations
    pub fn supports_dot3(&self) -> bool {
        matches!(
            self,
            ShaderVersion::V7 | ShaderVersion::V8 | ShaderVersion::Modern
        )
    }
}

/// Common bump mapping parameters used by both diffuse and specular shaders
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BumpMappingParams {
    /// Base texture file name
    pub texture_name: String,

    /// Normal/bump map texture file name
    pub bump_map_name: String,

    /// Ambient lighting color
    pub ambient: Vec3,

    /// Diffuse lighting color
    pub diffuse: Vec3,

    /// Diffuse bump mapping parameters (scale, bias)
    pub diffuse_bumpiness: Vec2,
}

impl Default for BumpMappingParams {
    fn default() -> Self {
        Self {
            texture_name: String::new(),
            bump_map_name: String::new(),
            ambient: Vec3::ONE,
            diffuse: Vec3::ONE,
            diffuse_bumpiness: Vec2::new(1.0, 0.0),
        }
    }
}

impl BumpMappingParams {
    /// Create new bump mapping parameters with default values
    pub fn new() -> Self {
        Self::default()
    }

    /// Validate that the parameters are in acceptable ranges
    pub fn validate(&self) -> ShdResult<()> {
        if self.texture_name.is_empty() {
            return Err(ShdError::InvalidConfig(
                "Base texture name cannot be empty".to_string(),
            ));
        }

        if self.bump_map_name.is_empty() {
            return Err(ShdError::InvalidConfig(
                "Bump map texture name cannot be empty".to_string(),
            ));
        }

        // Validate color components are in reasonable range
        if self.ambient.min_element() < 0.0 || self.ambient.max_element() > 10.0 {
            return Err(ShdError::InvalidConfig(
                "Ambient color components should be between 0.0 and 10.0".to_string(),
            ));
        }

        if self.diffuse.min_element() < 0.0 || self.diffuse.max_element() > 10.0 {
            return Err(ShdError::InvalidConfig(
                "Diffuse color components should be between 0.0 and 10.0".to_string(),
            ));
        }

        Ok(())
    }

    /// Extract filename from full path (maintaining compatibility with original C++ behavior)
    pub fn extract_filename(path: &str) -> String {
        // Handle both Unix and Windows path separators
        let filename = path
            .split('/')
            .last()
            .unwrap_or(path)
            .split('\\')
            .last()
            .unwrap_or(path);

        // Remove extension if present
        let stem = if let Some(dot_pos) = filename.rfind('.') {
            &filename[..dot_pos]
        } else {
            filename
        };

        format!("{}.tga", stem)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_version_detection() {
        let version = ShaderVersion::detect_best_version();
        assert_eq!(version, ShaderVersion::Modern);
    }

    #[test]
    fn test_shader_version_capabilities() {
        assert!(!ShaderVersion::V6.supports_pixel_shaders());
        assert!(!ShaderVersion::V6.supports_dot3());

        assert!(!ShaderVersion::V7.supports_pixel_shaders());
        assert!(ShaderVersion::V7.supports_dot3());

        assert!(ShaderVersion::V8.supports_pixel_shaders());
        assert!(ShaderVersion::V8.supports_dot3());

        assert!(ShaderVersion::Modern.supports_pixel_shaders());
        assert!(ShaderVersion::Modern.supports_dot3());
    }

    #[test]
    fn test_bump_mapping_params_validation() {
        let mut params = BumpMappingParams::new();
        params.texture_name = "test.tga".to_string();
        params.bump_map_name = "normal.tga".to_string();

        assert!(params.validate().is_ok());

        // Test empty texture name
        params.texture_name.clear();
        assert!(params.validate().is_err());

        // Test invalid color range
        params.texture_name = "test.tga".to_string();
        params.ambient = Vec3::new(-1.0, 0.5, 0.5);
        assert!(params.validate().is_err());
    }

    #[test]
    fn test_extract_filename() {
        assert_eq!(BumpMappingParams::extract_filename("test"), "test.tga");
        assert_eq!(
            BumpMappingParams::extract_filename("path/to/texture.jpg"),
            "texture.tga"
        );
        assert_eq!(
            BumpMappingParams::extract_filename("C:\\textures\\normal_map.png"),
            "normal_map.tga"
        );
    }
}
