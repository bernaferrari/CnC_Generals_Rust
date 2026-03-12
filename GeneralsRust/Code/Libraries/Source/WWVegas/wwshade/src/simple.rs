//! Simple Shader Implementation
//!
//! This module implements a basic texture shader equivalent to the C++ ShdSimpleDefClass.
//! It provides simple unlit texture rendering with ambient and diffuse colors.

use crate::{
    class_ids::SHDDEF_CLASSID_SIMPLE,
    def::ShdDefClass,
    error::{ShdError, ShdResult},
    interface::{RenderInfo, ShdInterface, VertexStreams},
};
use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Simple Shader Definition
///
/// This shader definition provides basic texture rendering with ambient and diffuse colors.
/// It's equivalent to the C++ ShdSimpleDefClass and represents an unlit shader that simply
/// modulates a texture with vertex colors and material properties.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimpleShaderDef {
    /// Base shader properties
    name: String,
    surface_type: i32,

    /// Texture file name/path
    texture_name: String,

    /// Ambient color component
    ambient: Vec3,

    /// Diffuse color component  
    diffuse: Vec3,
}

impl Default for SimpleShaderDef {
    fn default() -> Self {
        Self {
            name: "Simple".to_string(),
            surface_type: 0,
            texture_name: String::new(),
            ambient: Vec3::ONE, // Default white ambient
            diffuse: Vec3::ONE, // Default white diffuse
        }
    }
}

impl SimpleShaderDef {
    /// Create a new simple shader definition
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a simple shader with a specific texture
    pub fn with_texture<S: Into<String>>(mut self, texture_name: S) -> Self {
        self.texture_name = texture_name.into();
        self
    }

    /// Set the texture name
    pub fn set_texture_name<S: Into<String>>(&mut self, name: S) {
        self.texture_name = name.into();
    }

    /// Get the texture name
    pub fn get_texture_name(&self) -> &str {
        &self.texture_name
    }

    /// Set the ambient color
    pub fn set_ambient(&mut self, ambient: Vec3) {
        self.ambient = ambient;
    }

    /// Get the ambient color
    pub fn get_ambient(&self) -> Vec3 {
        self.ambient
    }

    /// Set the diffuse color
    pub fn set_diffuse(&mut self, diffuse: Vec3) {
        self.diffuse = diffuse;
    }

    /// Get the diffuse color  
    pub fn get_diffuse(&self) -> Vec3 {
        self.diffuse
    }
}

impl ShdDefClass for SimpleShaderDef {
    fn get_class_id(&self) -> u32 {
        SHDDEF_CLASSID_SIMPLE
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
        Ok(Box::new(SimpleShader::new(self)?))
    }

    fn is_valid_config(&self) -> ShdResult<()> {
        if self.texture_name.is_empty() {
            return Err(ShdError::InvalidConfig(
                "Simple shader requires a texture name".to_string(),
            ));
        }
        Ok(())
    }

    fn requires_normals(&self) -> bool {
        true // Simple shader needs normals for lighting
    }

    fn requires_tangent_space_vectors(&self) -> bool {
        false // Simple shader doesn't need tangent vectors
    }

    fn requires_sorting(&self) -> bool {
        false // Simple shader is typically opaque
    }

    fn static_sort_index(&self) -> i32 {
        0
    }

    fn save(&self) -> ShdResult<Vec<u8>> {
        bincode::serialize(self).map_err(|e| ShdError::Serialization(e.to_string()))
    }

    fn load(&mut self, data: &[u8]) -> ShdResult<()> {
        let loaded: SimpleShaderDef =
            bincode::deserialize(data).map_err(|e| ShdError::Serialization(e.to_string()))?;
        *self = loaded;
        Ok(())
    }

    fn as_any(&self) -> &dyn Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

/// Simple Shader Implementation
///
/// This is the actual shader implementation that performs the rendering.
/// It's equivalent to the C++ Shd6SimpleClass.
#[derive(Debug)]
pub struct SimpleShader {
    class_id: u32,
    texture_name: String,
    ambient: Vec4,
    diffuse: Vec4,

    // Render state
    _view_projection_matrix: Mat4,
}

impl SimpleShader {
    /// Create a new simple shader from a definition
    pub fn new(def: &SimpleShaderDef) -> ShdResult<Self> {
        let ambient = def.ambient.extend(1.0);
        let diffuse = def.diffuse.extend(1.0);

        Ok(Self {
            class_id: SHDDEF_CLASSID_SIMPLE,
            texture_name: def.texture_name.clone(),
            ambient,
            diffuse,
            _view_projection_matrix: Mat4::IDENTITY,
        })
    }

    /// Get the texture name used by this shader
    pub fn get_texture_name(&self) -> &str {
        &self.texture_name
    }

    /// Get the ambient color
    pub fn get_ambient(&self) -> Vec4 {
        self.ambient
    }

    /// Get the diffuse color
    pub fn get_diffuse(&self) -> Vec4 {
        self.diffuse
    }
}

impl ShdInterface for SimpleShader {
    fn get_class_id(&self) -> u32 {
        self.class_id
    }

    fn get_pass_count(&self) -> u32 {
        1 // Simple shader only needs one rendering pass
    }

    fn is_opaque(&self) -> bool {
        true // Simple shader is typically opaque
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // In the original C++ code, this would set up DirectX texture stage states
        // For now, we just validate the pass index
        if _pass >= self.get_pass_count() {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                _pass
            )));
        }

        // Modern equivalent would be setting up shader programs, samplers, etc.
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // In the original C++ code, this would set textures and materials per instance
        if _pass >= self.get_pass_count() {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                _pass
            )));
        }

        // Modern equivalent would be binding textures and updating uniform buffers
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn get_vertex_stream_count(&self) -> u32 {
        1 // Simple shader uses a single vertex stream
    }

    fn get_vertex_size(&self, stream: u32) -> u32 {
        if stream >= self.get_vertex_stream_count() {
            return 0;
        }

        // Size for VertexFormatXYZNDUV1: position (12) + normal (12) + diffuse (4) + UV (8) = 36 bytes
        36
    }

    fn use_hardware_vertex_processing(&self) -> bool {
        true // Modern shaders use hardware vertex processing
    }

    fn get_texture_count(&self) -> u32 {
        1 // Simple shader uses one texture
    }

    fn setup_frame(&mut self) -> ShdResult<()> {
        // Any per-frame setup can go here
        Ok(())
    }

    fn cleanup(&mut self) -> ShdResult<()> {
        // Any cleanup can go here
        Ok(())
    }
}

/// Vertex format used by the simple shader
/// Equivalent to VertexFormatXYZNDUV1 in the C++ code
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SimpleVertex {
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Normal vector (nx, ny, nz)  
    pub normal: [f32; 3],
    /// Diffuse color (32-bit RGBA)
    pub diffuse: u32,
    /// Texture coordinates (u, v)
    pub tex_coords: [f32; 2],
}

impl Default for SimpleVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            normal: [0.0; 3],
            diffuse: 0xFFFFFFFF, // Default to white
            tex_coords: [0.0; 2],
        }
    }
}

impl SimpleVertex {
    /// Create a new vertex with the specified attributes
    pub fn new(position: [f32; 3], normal: [f32; 3], diffuse: u32, tex_coords: [f32; 2]) -> Self {
        Self {
            position,
            normal,
            diffuse,
            tex_coords,
        }
    }
}

/// Utility function to copy vertex stream data into the simple vertex format
/// This is equivalent to the Copy_Vertex_Stream method in the C++ code
pub fn copy_vertex_stream(
    dest_buffer: &mut [SimpleVertex],
    vertex_streams: &VertexStreams,
) -> ShdResult<()> {
    let vertex_count = dest_buffer.len();

    if vertex_streams.get_vertex_count() != vertex_count {
        return Err(ShdError::VertexProcessingError(
            "Vertex count mismatch between destination buffer and streams".to_string(),
        ));
    }

    for (i, vertex) in dest_buffer.iter_mut().enumerate() {
        // Copy position
        if let Some(positions) = &vertex_streams.positions {
            if i < positions.len() {
                let pos = positions[i];
                vertex.position = [pos.x, pos.y, pos.z];
            }
        }

        // Copy normals
        if let Some(normals) = &vertex_streams.normals {
            if i < normals.len() {
                let normal = normals[i];
                vertex.normal = [normal.x, normal.y, normal.z];
            }
        }

        // Copy diffuse color
        if let Some(colors) = &vertex_streams.colors_int {
            if i < colors.len() {
                vertex.diffuse = colors[i];
            }
        } else {
            vertex.diffuse = 0xFFFFFFFF; // Default to white
        }

        // Copy UV coordinates
        if let Some(uvs) = &vertex_streams.uv_coords[0] {
            if i < uvs.len() {
                let uv = uvs[i];
                vertex.tex_coords = [uv.x, uv.y];
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_shader_def_creation() {
        let def = SimpleShaderDef::new().with_texture("test.tga");

        assert_eq!(def.get_texture_name(), "test.tga");
        assert_eq!(def.get_ambient(), Vec3::ONE);
        assert_eq!(def.get_diffuse(), Vec3::ONE);
        assert_eq!(def.get_class_id(), SHDDEF_CLASSID_SIMPLE);
    }

    #[test]
    fn test_simple_shader_def_validation() {
        let mut def = SimpleShaderDef::new();

        // Should fail without texture
        assert!(def.is_valid_config().is_err());

        // Should pass with texture
        def.set_texture_name("valid.tga");
        assert!(def.is_valid_config().is_ok());
    }

    #[test]
    fn test_simple_shader_def_properties() {
        let mut def = SimpleShaderDef::new();

        assert!(def.requires_normals());
        assert!(!def.requires_tangent_space_vectors());
        assert!(!def.requires_sorting());
        assert_eq!(def.static_sort_index(), 0);

        def.set_ambient(Vec3::new(0.2, 0.3, 0.4));
        def.set_diffuse(Vec3::new(0.8, 0.9, 1.0));

        assert_eq!(def.get_ambient(), Vec3::new(0.2, 0.3, 0.4));
        assert_eq!(def.get_diffuse(), Vec3::new(0.8, 0.9, 1.0));
    }

    #[test]
    fn test_simple_shader_creation() {
        let def = SimpleShaderDef::new().with_texture("test.tga");
        let shader = SimpleShader::new(&def).unwrap();

        assert_eq!(shader.get_class_id(), SHDDEF_CLASSID_SIMPLE);
        assert_eq!(shader.get_pass_count(), 1);
        assert!(shader.is_opaque());
        assert_eq!(shader.get_texture_name(), "test.tga");
        assert_eq!(shader.get_vertex_stream_count(), 1);
        assert_eq!(shader.get_vertex_size(0), 36);
        assert!(shader.use_hardware_vertex_processing());
        assert_eq!(shader.get_texture_count(), 1);
    }

    #[test]
    fn test_simple_vertex_creation() {
        let vertex = SimpleVertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], 0xFF0000FF, [0.5, 0.5]);

        assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.normal, [0.0, 1.0, 0.0]);
        assert_eq!(vertex.diffuse, 0xFF0000FF);
        assert_eq!(vertex.tex_coords, [0.5, 0.5]);
    }

    #[test]
    fn test_simple_shader_serialization() {
        let def = SimpleShaderDef::new().with_texture("serialize_test.tga");

        let data = def.save().unwrap();

        let mut loaded_def = SimpleShaderDef::default();
        loaded_def.load(&data).unwrap();

        assert_eq!(loaded_def.get_texture_name(), "serialize_test.tga");
        assert_eq!(loaded_def.get_ambient(), def.get_ambient());
        assert_eq!(loaded_def.get_diffuse(), def.get_diffuse());
    }

    #[test]
    fn test_copy_vertex_stream() {
        let mut dest_buffer = vec![SimpleVertex::default(); 2];

        let mut vertex_streams = VertexStreams::new();
        vertex_streams.positions = Some(vec![
            glam::Vec3::new(1.0, 2.0, 3.0),
            glam::Vec3::new(4.0, 5.0, 6.0),
        ]);
        vertex_streams.normals = Some(vec![glam::Vec3::Y, glam::Vec3::Z]);
        vertex_streams.colors_int = Some(vec![0xFF0000FF, 0x00FF00FF]);
        vertex_streams.uv_coords[0] =
            Some(vec![glam::Vec2::new(0.0, 0.0), glam::Vec2::new(1.0, 1.0)]);

        copy_vertex_stream(&mut dest_buffer, &vertex_streams).unwrap();

        assert_eq!(dest_buffer[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(dest_buffer[0].normal, [0.0, 1.0, 0.0]);
        assert_eq!(dest_buffer[0].diffuse, 0xFF0000FF);
        assert_eq!(dest_buffer[0].tex_coords, [0.0, 0.0]);

        assert_eq!(dest_buffer[1].position, [4.0, 5.0, 6.0]);
        assert_eq!(dest_buffer[1].normal, [0.0, 0.0, 1.0]);
        assert_eq!(dest_buffer[1].diffuse, 0x00FF00FF);
        assert_eq!(dest_buffer[1].tex_coords, [1.0, 1.0]);
    }
}
