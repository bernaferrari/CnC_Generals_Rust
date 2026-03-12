//	This program is free software: you can redistribute it and/or modify
//	it under the terms of the GNU General Public License as published by
//	the Free Software Foundation, either version 3 of the License, or
//	(at your option) any later version.
//
//	This program is distributed in the hope that it will be useful,
//	but WITHOUT ANY WARRANTY; without even the implied warranty of
//	MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
//	GNU General Public License for more details.
//
//	You should have received a copy of the GNU General Public License
//	along with this program.  If not, see <http://www.gnu.org/licenses/>.
//

//! Embedded shader source files for runtime compilation
//!
//! This module provides compile-time embedded access to all original DirectX shader source files.
//! The shaders are organized by DirectX version and can be accessed by name for runtime compilation.

use include_dir::{include_dir, Dir};
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// DirectX version enum for shader organization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DirectXVersion {
    DX6,
    DX7,
    DX8,
}

/// Shader type enum
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ShaderType {
    Vertex, // .vsh files
    Pixel,  // .psh files
}

/// Shader identifier for lookup
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ShaderKey {
    pub dx_version: DirectXVersion,
    pub shader_type: ShaderType,
    pub name: String,
}

impl ShaderKey {
    pub fn new(
        dx_version: DirectXVersion,
        shader_type: ShaderType,
        name: impl Into<String>,
    ) -> Self {
        Self {
            dx_version,
            shader_type,
            name: name.into(),
        }
    }
}

/// Embedded shader directories
static SHADERS_DIR: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/shaders");

/// Lazy-initialized shader source cache
static SHADER_CACHE: Lazy<HashMap<ShaderKey, &'static str>> = Lazy::new(|| {
    let mut cache = HashMap::new();

    // Load DX6 shaders
    if let Some(dx6_dir) = SHADERS_DIR.get_dir("dx6") {
        for file in dx6_dir.files() {
            if let Some(name) = file.path().file_name().and_then(|n| n.to_str()) {
                if let Some(source) = file.contents_utf8() {
                    let (shader_type, base_name) = if name.ends_with(".vsh") {
                        (ShaderType::Vertex, &name[..name.len() - 4])
                    } else if name.ends_with(".psh") {
                        (ShaderType::Pixel, &name[..name.len() - 4])
                    } else {
                        continue;
                    };

                    let key = ShaderKey::new(DirectXVersion::DX6, shader_type, base_name);
                    cache.insert(key, source);
                }
            }
        }
    }

    // Load DX7 shaders
    if let Some(dx7_dir) = SHADERS_DIR.get_dir("dx7") {
        for file in dx7_dir.files() {
            if let Some(name) = file.path().file_name().and_then(|n| n.to_str()) {
                if let Some(source) = file.contents_utf8() {
                    let (shader_type, base_name) = if name.ends_with(".vsh") {
                        (ShaderType::Vertex, &name[..name.len() - 4])
                    } else if name.ends_with(".psh") {
                        (ShaderType::Pixel, &name[..name.len() - 4])
                    } else {
                        continue;
                    };

                    let key = ShaderKey::new(DirectXVersion::DX7, shader_type, base_name);
                    cache.insert(key, source);
                }
            }
        }
    }

    // Load DX8 shaders
    if let Some(dx8_dir) = SHADERS_DIR.get_dir("dx8") {
        for file in dx8_dir.files() {
            if let Some(name) = file.path().file_name().and_then(|n| n.to_str()) {
                if let Some(source) = file.contents_utf8() {
                    let (shader_type, base_name) = if name.ends_with(".vsh") {
                        (ShaderType::Vertex, &name[..name.len() - 4])
                    } else if name.ends_with(".psh") {
                        (ShaderType::Pixel, &name[..name.len() - 4])
                    } else {
                        continue;
                    };

                    let key = ShaderKey::new(DirectXVersion::DX8, shader_type, base_name);
                    cache.insert(key, source);
                }
            }
        }
    }

    cache
});

/// Error type for shader access operations
#[derive(Debug, thiserror::Error)]
pub enum ShaderError {
    #[error("Shader not found: {name} (DirectX {dx_version:?}, {shader_type:?})")]
    NotFound {
        name: String,
        dx_version: DirectXVersion,
        shader_type: ShaderType,
    },
    #[error("Invalid shader name: {name}")]
    InvalidName { name: String },
}

/// Shader source accessor with metadata
#[derive(Debug, Clone)]
pub struct ShaderSource {
    pub key: ShaderKey,
    pub source: &'static str,
    pub size: usize,
}

impl ShaderSource {
    /// Get the shader source code
    pub fn source(&self) -> &str {
        self.source
    }

    /// Get the shader size in bytes
    pub fn size(&self) -> usize {
        self.size
    }

    /// Get the DirectX version
    pub fn dx_version(&self) -> DirectXVersion {
        self.key.dx_version
    }

    /// Get the shader type
    pub fn shader_type(&self) -> ShaderType {
        self.key.shader_type
    }

    /// Get the shader name
    pub fn name(&self) -> &str {
        &self.key.name
    }
}

/// Main shader source accessor
pub struct EmbeddedShaders;

impl EmbeddedShaders {
    /// Get shader source by DirectX version, type, and name
    pub fn get_shader(
        dx_version: DirectXVersion,
        shader_type: ShaderType,
        name: &str,
    ) -> Result<ShaderSource, ShaderError> {
        let key = ShaderKey::new(dx_version, shader_type, name);

        SHADER_CACHE
            .get(&key)
            .map(|&source| ShaderSource {
                key: key.clone(),
                source,
                size: source.len(),
            })
            .ok_or(ShaderError::NotFound {
                name: name.to_string(),
                dx_version,
                shader_type,
            })
    }

    /// Get vertex shader source
    pub fn get_vertex_shader(
        dx_version: DirectXVersion,
        name: &str,
    ) -> Result<ShaderSource, ShaderError> {
        Self::get_shader(dx_version, ShaderType::Vertex, name)
    }

    /// Get pixel shader source
    pub fn get_pixel_shader(
        dx_version: DirectXVersion,
        name: &str,
    ) -> Result<ShaderSource, ShaderError> {
        Self::get_shader(dx_version, ShaderType::Pixel, name)
    }

    /// List all available shaders for a specific DirectX version
    pub fn list_shaders(dx_version: DirectXVersion) -> Vec<ShaderKey> {
        SHADER_CACHE
            .keys()
            .filter(|key| key.dx_version == dx_version)
            .cloned()
            .collect()
    }

    /// List all available shaders of a specific type
    pub fn list_shaders_by_type(shader_type: ShaderType) -> Vec<ShaderKey> {
        SHADER_CACHE
            .keys()
            .filter(|key| key.shader_type == shader_type)
            .cloned()
            .collect()
    }

    /// Get all shader keys
    pub fn list_all_shaders() -> Vec<ShaderKey> {
        SHADER_CACHE.keys().cloned().collect()
    }

    /// Check if a shader exists
    pub fn has_shader(dx_version: DirectXVersion, shader_type: ShaderType, name: &str) -> bool {
        let key = ShaderKey::new(dx_version, shader_type, name);
        SHADER_CACHE.contains_key(&key)
    }

    /// Get the total number of embedded shaders
    pub fn shader_count() -> usize {
        SHADER_CACHE.len()
    }
}

/// Convenience macros for common shader access patterns
#[macro_export]
macro_rules! get_dx6_vertex_shader {
    ($name:expr) => {
        $crate::embedded_shaders::EmbeddedShaders::get_vertex_shader(
            $crate::embedded_shaders::DirectXVersion::DX6,
            $name,
        )
    };
}

#[macro_export]
macro_rules! get_dx7_vertex_shader {
    ($name:expr) => {
        $crate::embedded_shaders::EmbeddedShaders::get_vertex_shader(
            $crate::embedded_shaders::DirectXVersion::DX7,
            $name,
        )
    };
}

#[macro_export]
macro_rules! get_dx8_vertex_shader {
    ($name:expr) => {
        $crate::embedded_shaders::EmbeddedShaders::get_vertex_shader(
            $crate::embedded_shaders::DirectXVersion::DX8,
            $name,
        )
    };
}

#[macro_export]
macro_rules! get_dx8_pixel_shader {
    ($name:expr) => {
        $crate::embedded_shaders::EmbeddedShaders::get_pixel_shader(
            $crate::embedded_shaders::DirectXVersion::DX8,
            $name,
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_loading() {
        // Test that shaders are loaded
        assert!(EmbeddedShaders::shader_count() > 0);

        // Test specific shader access
        assert!(EmbeddedShaders::has_shader(
            DirectXVersion::DX6,
            ShaderType::Vertex,
            "shd6bumpdiff"
        ));
        assert!(EmbeddedShaders::has_shader(
            DirectXVersion::DX8,
            ShaderType::Pixel,
            "shd8bumpdiff"
        ));
    }

    #[test]
    fn test_shader_retrieval() {
        // Test getting a specific shader
        if let Ok(shader) = EmbeddedShaders::get_vertex_shader(DirectXVersion::DX6, "shd6bumpdiff")
        {
            assert!(shader.source().contains("vs.1.1"));
            assert!(shader.size() > 0);
            assert_eq!(shader.dx_version(), DirectXVersion::DX6);
            assert_eq!(shader.shader_type(), ShaderType::Vertex);
            assert_eq!(shader.name(), "shd6bumpdiff");
        } else {
            panic!("Failed to load expected shader");
        }
    }

    #[test]
    fn test_shader_listing() {
        let dx6_shaders = EmbeddedShaders::list_shaders(DirectXVersion::DX6);
        assert!(!dx6_shaders.is_empty());

        let vertex_shaders = EmbeddedShaders::list_shaders_by_type(ShaderType::Vertex);
        assert!(!vertex_shaders.is_empty());

        let all_shaders = EmbeddedShaders::list_all_shaders();
        assert!(!all_shaders.is_empty());
    }

    #[test]
    fn test_macros() {
        // Test convenience macros
        if let Ok(shader) = get_dx6_vertex_shader!("shd6bumpdiff") {
            assert_eq!(shader.dx_version(), DirectXVersion::DX6);
            assert_eq!(shader.shader_type(), ShaderType::Vertex);
        }

        if let Ok(shader) = get_dx8_pixel_shader!("shd8bumpdiff") {
            assert_eq!(shader.dx_version(), DirectXVersion::DX8);
            assert_eq!(shader.shader_type(), ShaderType::Pixel);
        }
    }

    #[test]
    fn test_error_handling() {
        // Test non-existent shader
        let result = EmbeddedShaders::get_vertex_shader(DirectXVersion::DX6, "nonexistent");
        assert!(result.is_err());

        if let Err(ShaderError::NotFound {
            name,
            dx_version,
            shader_type,
        }) = result
        {
            assert_eq!(name, "nonexistent");
            assert_eq!(dx_version, DirectXVersion::DX6);
            assert_eq!(shader_type, ShaderType::Vertex);
        } else {
            panic!("Expected NotFound error");
        }
    }
}
