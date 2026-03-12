//! Bump Diffuse Shader Implementation
//!
//! This module implements a bump mapping shader that applies diffuse lighting
//! with normal mapping. It's a port of the original ShdBumpDiffDefClass from
//! the C++ WW3D engine.

use super::{constants::*, BumpMappingParams, ShaderVersion};
use crate::def::ShdDefClass;
use crate::error::{ShdError, ShdResult};
use crate::interface::{RenderInfo, ShdInterface};
use glam::{Vec2, Vec3};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Class ID for bump diffuse shader definitions
pub const BUMP_DIFF_CLASS_ID: u32 = 4; // SHDDEF_CLASSID_BUMPDIFF from original

/// Bump Diffuse Shader Definition
///
/// This shader applies diffuse lighting with bump mapping using normal maps.
/// It supports multiple hardware versions for backward compatibility.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BumpDiffShaderDef {
    /// Base shader definition properties
    pub name: String,
    pub surface_type: i32,

    /// Shader version to use
    pub version: ShaderVersion,

    /// Bump mapping parameters
    pub params: BumpMappingParams,
}

impl BumpDiffShaderDef {
    /// Create a new bump diffuse shader definition with default parameters
    pub fn new() -> Self {
        Self {
            name: String::new(),
            surface_type: 0,
            version: ShaderVersion::detect_best_version(),
            params: BumpMappingParams::new(),
        }
    }

    /// Create a new bump diffuse shader definition with specified textures
    pub fn with_textures(texture_name: String, bump_map_name: String) -> Self {
        let mut shader = Self::new();
        shader.params.texture_name = texture_name;
        shader.params.bump_map_name = bump_map_name;
        shader
    }

    /// Set the base texture name
    pub fn set_texture_name(&mut self, name: String) {
        self.params.texture_name = name;
    }

    /// Get the base texture name
    pub fn get_texture_name(&self) -> &str {
        &self.params.texture_name
    }

    /// Set the bump map texture name
    pub fn set_bump_map_name(&mut self, name: String) {
        self.params.bump_map_name = name;
    }

    /// Get the bump map texture name
    pub fn get_bump_map_name(&self) -> &str {
        &self.params.bump_map_name
    }

    /// Set the ambient color
    pub fn set_ambient(&mut self, ambient: Vec3) {
        self.params.ambient = ambient;
    }

    /// Get the ambient color
    pub fn get_ambient(&self) -> Vec3 {
        self.params.ambient
    }

    /// Set the diffuse color
    pub fn set_diffuse(&mut self, diffuse: Vec3) {
        self.params.diffuse = diffuse;
    }

    /// Get the diffuse color
    pub fn get_diffuse(&self) -> Vec3 {
        self.params.diffuse
    }

    /// Set the diffuse bumpiness parameters (scale, bias)
    pub fn set_diffuse_bumpiness(&mut self, bumpiness: Vec2) {
        self.params.diffuse_bumpiness = bumpiness;
    }

    /// Get the diffuse bumpiness parameters
    pub fn get_diffuse_bumpiness(&self) -> Vec2 {
        self.params.diffuse_bumpiness
    }

    /// Initialize the shader system for this shader type
    pub fn init() -> ShdResult<()> {
        // In the original C++, this would select the appropriate shader version
        // and initialize the corresponding shader classes (Shd6BumpDiffClass, etc.)

        let version = ShaderVersion::detect_best_version();
        log::info!(
            "Initializing Bump Diffuse shader with version {:?}",
            version
        );

        match version {
            ShaderVersion::Modern => {
                // Initialize modern shader pipeline
                // This would set up compute shaders, descriptor sets, etc.
                Ok(())
            }
            ShaderVersion::V8 => {
                // Initialize DirectX 8 compatible shaders
                Ok(())
            }
            ShaderVersion::V7 => {
                // Initialize DirectX 7 compatible shaders with DOT3
                Ok(())
            }
            ShaderVersion::V6 => {
                // Initialize DirectX 6 fallback shaders
                Ok(())
            }
        }
    }

    /// Shutdown the shader system for this shader type
    pub fn shutdown() -> ShdResult<()> {
        log::info!("Shutting down Bump Diffuse shader");
        // Clean up resources allocated during init
        Ok(())
    }
}

impl Default for BumpDiffShaderDef {
    fn default() -> Self {
        Self::new()
    }
}

impl ShdDefClass for BumpDiffShaderDef {
    fn get_class_id(&self) -> u32 {
        BUMP_DIFF_CLASS_ID
    }

    fn get_name(&self) -> &str {
        &self.name
    }

    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn get_surface_type(&self) -> i32 {
        self.surface_type
    }

    fn set_surface_type(&mut self, surface_type: i32) {
        self.surface_type = surface_type;
    }

    fn clone_def(&self) -> Box<dyn ShdDefClass> {
        Box::new(self.clone())
    }

    fn create_shader(&self) -> ShdResult<Box<dyn ShdInterface>> {
        // Create the appropriate shader implementation based on the detected version
        match self.version {
            ShaderVersion::Modern => Ok(Box::new(ModernBumpDiffShader::new(self.clone())?)),
            ShaderVersion::V8 => Ok(Box::new(V8BumpDiffShader::new(self.clone())?)),
            ShaderVersion::V7 => Ok(Box::new(V7BumpDiffShader::new(self.clone())?)),
            ShaderVersion::V6 => Ok(Box::new(V6BumpDiffShader::new(self.clone())?)),
        }
    }

    fn is_valid_config(&self) -> ShdResult<()> {
        self.params.validate()?;

        // Additional validation specific to diffuse bump mapping
        if self.params.diffuse_bumpiness.x <= 0.0 {
            return Err(ShdError::InvalidConfig(
                "Diffuse bump scale must be greater than 0".to_string(),
            ));
        }

        Ok(())
    }

    fn requires_normals(&self) -> bool {
        true
    }

    fn requires_tangent_space_vectors(&self) -> bool {
        true
    }

    fn requires_sorting(&self) -> bool {
        false
    }

    fn static_sort_index(&self) -> i32 {
        0
    }

    fn save(&self) -> ShdResult<Vec<u8>> {
        // Serialize the shader definition to binary format
        // This mimics the original ChunkSaveClass functionality
        let mut data = Vec::new();

        // Save the parameters in a chunk-based format
        let chunk_data = self.serialize_variables()?;
        data.extend_from_slice(&CHUNKID_VARIABLES.to_le_bytes());
        data.extend_from_slice(&(chunk_data.len() as u32).to_le_bytes());
        data.extend_from_slice(&chunk_data);

        Ok(data)
    }

    fn load(&mut self, data: &[u8]) -> ShdResult<()> {
        // Deserialize the shader definition from binary format
        // This mimics the original ChunkLoadClass functionality
        let mut offset = 0;

        while offset + 8 <= data.len() {
            let chunk_id = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]) as usize;
            offset += 8;

            if offset + chunk_size > data.len() {
                return Err(ShdError::Serialization("Invalid chunk size".to_string()));
            }

            match chunk_id {
                CHUNKID_VARIABLES => {
                    self.deserialize_variables(&data[offset..offset + chunk_size])?;
                }
                _ => {
                    // Skip unknown chunks
                }
            }

            offset += chunk_size;
        }

        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl BumpDiffShaderDef {
    /// Serialize shader variables to binary format
    fn serialize_variables(&self) -> ShdResult<Vec<u8>> {
        let mut data = Vec::new();

        // Serialize texture names (extract filename only, like original C++)
        let texture_filename = BumpMappingParams::extract_filename(&self.params.texture_name);
        let bump_filename = BumpMappingParams::extract_filename(&self.params.bump_map_name);

        // Write micro chunks
        self.write_micro_chunk_string(&mut data, VARID_TEXTURE_NAME, &texture_filename)?;
        self.write_micro_chunk_string(&mut data, VARID_BUMP_MAP_NAME, &bump_filename)?;
        self.write_micro_chunk_vec3(&mut data, VARID_AMBIENT_COLOR, &self.params.ambient)?;
        self.write_micro_chunk_vec3(&mut data, VARID_DIFFUSE_COLOR, &self.params.diffuse)?;
        self.write_micro_chunk_vec2(
            &mut data,
            VARID_DIFFUSE_BUMPINESS,
            &self.params.diffuse_bumpiness,
        )?;

        Ok(data)
    }

    /// Deserialize shader variables from binary format
    fn deserialize_variables(&mut self, data: &[u8]) -> ShdResult<()> {
        let mut offset = 0;

        while offset + 1 < data.len() {
            let var_id = data[offset];
            offset += 1;

            match var_id {
                VARID_TEXTURE_NAME => {
                    self.params.texture_name = self.read_micro_chunk_string(data, &mut offset)?;
                }
                VARID_BUMP_MAP_NAME => {
                    self.params.bump_map_name = self.read_micro_chunk_string(data, &mut offset)?;
                }
                VARID_AMBIENT_COLOR => {
                    self.params.ambient = self.read_micro_chunk_vec3(data, &mut offset)?;
                }
                VARID_DIFFUSE_COLOR => {
                    self.params.diffuse = self.read_micro_chunk_vec3(data, &mut offset)?;
                }
                VARID_DIFFUSE_BUMPINESS => {
                    self.params.diffuse_bumpiness =
                        self.read_micro_chunk_vec2(data, &mut offset)?;
                }
                _ => {
                    return Err(ShdError::Serialization(format!(
                        "Unknown variable ID: {}",
                        var_id
                    )));
                }
            }
        }

        Ok(())
    }

    /// Write a micro chunk to the data buffer
    fn write_micro_chunk_string(&self, data: &mut Vec<u8>, id: u8, value: &str) -> ShdResult<()> {
        data.push(id);
        data.extend_from_slice(&(value.len() as u32).to_le_bytes());
        data.extend_from_slice(value.as_bytes());
        Ok(())
    }

    /// Write a micro chunk to the data buffer
    fn write_micro_chunk_vec3(&self, data: &mut Vec<u8>, id: u8, value: &Vec3) -> ShdResult<()> {
        data.push(id);
        data.extend_from_slice(&value.x.to_le_bytes());
        data.extend_from_slice(&value.y.to_le_bytes());
        data.extend_from_slice(&value.z.to_le_bytes());
        Ok(())
    }

    /// Write a micro chunk to the data buffer
    fn write_micro_chunk_vec2(&self, data: &mut Vec<u8>, id: u8, value: &Vec2) -> ShdResult<()> {
        data.push(id);
        data.extend_from_slice(&value.x.to_le_bytes());
        data.extend_from_slice(&value.y.to_le_bytes());
        Ok(())
    }

    /// Read a string micro chunk from the data buffer
    fn read_micro_chunk_string(&self, data: &[u8], offset: &mut usize) -> ShdResult<String> {
        if *offset + 4 > data.len() {
            return Err(ShdError::Serialization(
                "Not enough data for string length".to_string(),
            ));
        }

        let len = u32::from_le_bytes([
            data[*offset],
            data[*offset + 1],
            data[*offset + 2],
            data[*offset + 3],
        ]) as usize;
        *offset += 4;

        if *offset + len > data.len() {
            return Err(ShdError::Serialization(
                "Not enough data for string content".to_string(),
            ));
        }

        let string = String::from_utf8(data[*offset..*offset + len].to_vec())
            .map_err(|e| ShdError::Serialization(e.to_string()))?;
        *offset += len;

        Ok(string)
    }

    /// Read a Vec3 micro chunk from the data buffer
    fn read_micro_chunk_vec3(&self, data: &[u8], offset: &mut usize) -> ShdResult<Vec3> {
        if *offset + 12 > data.len() {
            return Err(ShdError::Serialization(
                "Not enough data for Vec3".to_string(),
            ));
        }

        let x = f32::from_le_bytes([
            data[*offset],
            data[*offset + 1],
            data[*offset + 2],
            data[*offset + 3],
        ]);
        let y = f32::from_le_bytes([
            data[*offset + 4],
            data[*offset + 5],
            data[*offset + 6],
            data[*offset + 7],
        ]);
        let z = f32::from_le_bytes([
            data[*offset + 8],
            data[*offset + 9],
            data[*offset + 10],
            data[*offset + 11],
        ]);
        *offset += 12;

        Ok(Vec3::new(x, y, z))
    }

    /// Read a Vec2 micro chunk from the data buffer
    fn read_micro_chunk_vec2(&self, data: &[u8], offset: &mut usize) -> ShdResult<Vec2> {
        if *offset + 8 > data.len() {
            return Err(ShdError::Serialization(
                "Not enough data for Vec2".to_string(),
            ));
        }

        let x = f32::from_le_bytes([
            data[*offset],
            data[*offset + 1],
            data[*offset + 2],
            data[*offset + 3],
        ]);
        let y = f32::from_le_bytes([
            data[*offset + 4],
            data[*offset + 5],
            data[*offset + 6],
            data[*offset + 7],
        ]);
        *offset += 8;

        Ok(Vec2::new(x, y))
    }
}

// Shader implementation structs for different hardware versions

/// Modern bump diffuse shader implementation using advanced graphics APIs
#[derive(Debug)]
struct ModernBumpDiffShader {
    _definition: BumpDiffShaderDef,
}

impl ModernBumpDiffShader {
    fn new(def: BumpDiffShaderDef) -> ShdResult<Self> {
        Ok(Self { _definition: def })
    }
}

impl ShdInterface for ModernBumpDiffShader {
    fn get_class_id(&self) -> u32 {
        BUMP_DIFF_CLASS_ID
    }

    fn get_pass_count(&self) -> u32 {
        1 // Modern implementation uses single pass
    }

    fn is_opaque(&self) -> bool {
        true // Bump diffuse shaders are typically opaque
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Apply shared render states for this shader type
        // This would set up textures, samplers, etc.
        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Apply per-instance render states
        // This would set up per-object constants like colors, matrices, etc.
        Ok(())
    }
}

/// DirectX 8 compatible bump diffuse shader implementation
#[derive(Debug)]
struct V8BumpDiffShader {
    _definition: BumpDiffShaderDef,
}

impl V8BumpDiffShader {
    fn new(def: BumpDiffShaderDef) -> ShdResult<Self> {
        Ok(Self { _definition: def })
    }
}

impl ShdInterface for V8BumpDiffShader {
    fn get_class_id(&self) -> u32 {
        BUMP_DIFF_CLASS_ID
    }

    fn get_pass_count(&self) -> u32 {
        1 // DX8 pixel shaders support single pass bump mapping
    }

    fn is_opaque(&self) -> bool {
        true
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Set up DX8 pixel shader states
        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Set up DX8 per-instance states
        Ok(())
    }
}

/// DirectX 7 compatible bump diffuse shader implementation with DOT3 support
#[derive(Debug)]
struct V7BumpDiffShader {
    _definition: BumpDiffShaderDef,
}

impl V7BumpDiffShader {
    fn new(def: BumpDiffShaderDef) -> ShdResult<Self> {
        Ok(Self { _definition: def })
    }
}

impl ShdInterface for V7BumpDiffShader {
    fn get_class_id(&self) -> u32 {
        BUMP_DIFF_CLASS_ID
    }

    fn get_pass_count(&self) -> u32 {
        2 // DX7 typically requires multiple passes for bump mapping
    }

    fn is_opaque(&self) -> bool {
        true
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Set up DX7 DOT3 texture blend states
        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Set up DX7 per-instance states
        Ok(())
    }
}

/// DirectX 6 fallback bump diffuse shader implementation
#[derive(Debug)]
struct V6BumpDiffShader {
    _definition: BumpDiffShaderDef,
}

impl V6BumpDiffShader {
    fn new(def: BumpDiffShaderDef) -> ShdResult<Self> {
        Ok(Self { _definition: def })
    }
}

impl ShdInterface for V6BumpDiffShader {
    fn get_class_id(&self) -> u32 {
        BUMP_DIFF_CLASS_ID
    }

    fn get_pass_count(&self) -> u32 {
        3 // DX6 requires multiple passes to simulate bump mapping
    }

    fn is_opaque(&self) -> bool {
        true
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Set up DX6 multi-pass texture blend states
        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // Set up DX6 per-instance states
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bump_diff_shader_creation() {
        let shader = BumpDiffShaderDef::new();
        assert_eq!(shader.get_class_id(), BUMP_DIFF_CLASS_ID);
        assert!(shader.requires_normals());
        assert!(shader.requires_tangent_space_vectors());
        assert!(!shader.requires_sorting());
    }

    #[test]
    fn test_bump_diff_shader_with_textures() {
        let shader = BumpDiffShaderDef::with_textures(
            "base_texture.tga".to_string(),
            "normal_map.tga".to_string(),
        );
        assert_eq!(shader.get_texture_name(), "base_texture.tga");
        assert_eq!(shader.get_bump_map_name(), "normal_map.tga");
    }

    #[test]
    fn test_bump_diff_shader_validation() {
        let mut shader = BumpDiffShaderDef::new();
        shader.set_texture_name("test.tga".to_string());
        shader.set_bump_map_name("normal.tga".to_string());

        assert!(shader.is_valid_config().is_ok());

        // Test invalid bumpiness scale
        shader.set_diffuse_bumpiness(Vec2::new(-1.0, 0.0));
        assert!(shader.is_valid_config().is_err());
    }

    #[test]
    fn test_bump_diff_shader_serialization() {
        let mut shader = BumpDiffShaderDef::new();
        shader.set_texture_name("test.tga".to_string());
        shader.set_bump_map_name("normal.tga".to_string());
        shader.set_ambient(Vec3::new(0.2, 0.2, 0.2));
        shader.set_diffuse(Vec3::new(0.8, 0.8, 0.8));

        let data = shader.save().unwrap();

        let mut loaded_shader = BumpDiffShaderDef::new();
        loaded_shader.load(&data).unwrap();

        assert_eq!(loaded_shader.get_ambient(), Vec3::new(0.2, 0.2, 0.2));
        assert_eq!(loaded_shader.get_diffuse(), Vec3::new(0.8, 0.8, 0.8));
    }
}
