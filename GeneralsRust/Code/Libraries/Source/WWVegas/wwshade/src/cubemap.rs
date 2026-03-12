//! Cube Map Shader Implementation
//!
//! This module implements a cube map reflection shader equivalent to the C++ ShdCubeMapDefClass.
//! It provides environment reflection mapping using cube map textures with specular lighting.

use crate::{
    class_ids::SHDDEF_CLASSID_CUBEMAP,
    def::ShdDefClass,
    error::{ShdError, ShdResult},
    interface::{RenderInfo, ShdInterface, VertexStreams},
};
use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Cube Map Shader Definition
///
/// This shader definition provides cube map reflection rendering with ambient, diffuse, and specular colors.
/// It's equivalent to the C++ ShdCubeMapDefClass and represents a reflective shader that uses
/// a cube map texture for environment reflections.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CubeMapShaderDef {
    /// Base shader properties
    name: String,
    surface_type: i32,

    /// Cube map texture file name/path (typically a .dds file)
    texture_name: String,

    /// Ambient color component
    ambient: Vec3,

    /// Diffuse color component  
    diffuse: Vec3,

    /// Specular color component
    specular: Vec3,
}

impl Default for CubeMapShaderDef {
    fn default() -> Self {
        Self {
            name: "Cube Map".to_string(),
            surface_type: 0,
            texture_name: String::new(),
            ambient: Vec3::ONE,   // Default white ambient
            diffuse: Vec3::ONE,   // Default white diffuse
            specular: Vec3::ZERO, // Default no specular (as in C++ code)
        }
    }
}

impl CubeMapShaderDef {
    /// Create a new cube map shader definition
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a cube map shader with a specific texture
    pub fn with_texture<S: Into<String>>(mut self, texture_name: S) -> Self {
        self.texture_name = texture_name.into();
        self
    }

    /// Set the cube map texture name
    pub fn set_texture_name<S: Into<String>>(&mut self, name: S) {
        self.texture_name = name.into();
    }

    /// Get the cube map texture name
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

    /// Set the specular color
    pub fn set_specular(&mut self, specular: Vec3) {
        self.specular = specular;
    }

    /// Get the specular color
    pub fn get_specular(&self) -> Vec3 {
        self.specular
    }
}

impl ShdDefClass for CubeMapShaderDef {
    fn get_class_id(&self) -> u32 {
        SHDDEF_CLASSID_CUBEMAP
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
        Ok(Box::new(CubeMapShader::new(self)?))
    }

    fn is_valid_config(&self) -> ShdResult<()> {
        if self.texture_name.is_empty() {
            return Err(ShdError::InvalidConfig(
                "Cube map shader requires a texture name".to_string(),
            ));
        }
        Ok(())
    }

    fn requires_normals(&self) -> bool {
        true // Cube map shader needs normals for reflection vector calculation
    }

    fn requires_tangent_space_vectors(&self) -> bool {
        false // Cube map shader doesn't need tangent vectors
    }

    fn requires_sorting(&self) -> bool {
        false // Cube map shader is typically opaque
    }

    fn static_sort_index(&self) -> i32 {
        0
    }

    fn save(&self) -> ShdResult<Vec<u8>> {
        bincode::serialize(self).map_err(|e| ShdError::Serialization(e.to_string()))
    }

    fn load(&mut self, data: &[u8]) -> ShdResult<()> {
        let loaded: CubeMapShaderDef =
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

/// Cube Map Shader Implementation
///
/// This is the actual shader implementation that performs the cube map reflection rendering.
/// It's equivalent to the C++ Shd6CubeMapClass.
#[derive(Debug)]
pub struct CubeMapShader {
    class_id: u32,
    texture_name: String,
    ambient: Vec4,
    diffuse: Vec4,
    specular: Vec4,
    specular_power: f32,

    // Render state
    _view_projection_matrix: Mat4,
}

impl CubeMapShader {
    /// Create a new cube map shader from a definition
    pub fn new(def: &CubeMapShaderDef) -> ShdResult<Self> {
        let ambient = def.ambient.extend(1.0);
        let diffuse = def.diffuse.extend(1.0);
        let specular = def.specular.extend(1.0);

        Ok(Self {
            class_id: SHDDEF_CLASSID_CUBEMAP,
            texture_name: def.texture_name.clone(),
            ambient,
            diffuse,
            specular,
            specular_power: 20.0, // Default power as in C++ code
            _view_projection_matrix: Mat4::IDENTITY,
        })
    }

    /// Get the cube map texture name used by this shader
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

    /// Get the specular color
    pub fn get_specular(&self) -> Vec4 {
        self.specular
    }

    /// Get the specular power
    pub fn get_specular_power(&self) -> f32 {
        self.specular_power
    }

    /// Set the specular power
    pub fn set_specular_power(&mut self, power: f32) {
        self.specular_power = power;
    }
}

impl ShdInterface for CubeMapShader {
    fn get_class_id(&self) -> u32 {
        self.class_id
    }

    fn get_pass_count(&self) -> u32 {
        1 // Cube map shader only needs one rendering pass
    }

    fn is_opaque(&self) -> bool {
        true // Cube map shader is typically opaque
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // In the original C++ code, this would set up DirectX texture stage states
        // for cube map reflection coordinate generation
        if _pass >= self.get_pass_count() {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                _pass
            )));
        }

        // The C++ code sets:
        // - D3DTSS_TEXCOORDINDEX to D3DTSS_TCI_CAMERASPACEREFLECTIONVECTOR for automatic reflection coords
        // - Various texture stage states for modulating texture with diffuse color
        // - Lighting and specular enable states
        // - Material source states

        // Modern equivalent would be setting up cube map sampler, enabling specular lighting, etc.
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // In the original C++ code, this would bind the cube map texture
        if _pass >= self.get_pass_count() {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                _pass
            )));
        }

        // Modern equivalent would be binding the cube map texture and updating material uniforms
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn get_vertex_stream_count(&self) -> u32 {
        1 // Cube map shader uses a single vertex stream
    }

    fn get_vertex_size(&self, stream: u32) -> u32 {
        if stream >= self.get_vertex_stream_count() {
            return 0;
        }

        // Size for VertexFormatXYZNDCUBEMAP: position (12) + normal (12) + diffuse (4) = 28 bytes
        // Note: cube map coordinates are generated automatically, so no explicit UV storage needed
        28
    }

    fn use_hardware_vertex_processing(&self) -> bool {
        true // Modern shaders use hardware vertex processing
    }

    fn get_texture_count(&self) -> u32 {
        1 // Cube map shader uses one cube map texture
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

/// Vertex format used by the cube map shader
/// Equivalent to VertexFormatXYZNDCUBEMAP in the C++ code
/// Note: cube map coordinates are generated automatically from the normal and view position
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CubeMapVertex {
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Normal vector (nx, ny, nz) - used for reflection vector calculation
    pub normal: [f32; 3],
    /// Diffuse color (32-bit RGBA)
    pub diffuse: u32,
}

impl Default for CubeMapVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            normal: [0.0; 3],
            diffuse: 0xFFFFFFFF, // Default to white
        }
    }
}

impl CubeMapVertex {
    /// Create a new cube map vertex with the specified attributes
    pub fn new(position: [f32; 3], normal: [f32; 3], diffuse: u32) -> Self {
        Self {
            position,
            normal,
            diffuse,
        }
    }
}

/// Utility function to copy vertex stream data into the cube map vertex format
/// This is equivalent to the Copy_Vertex_Stream method in the C++ code
pub fn copy_vertex_stream(
    dest_buffer: &mut [CubeMapVertex],
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
    }

    Ok(())
}

/// Material properties for cube map rendering
/// This corresponds to the D3DMATERIAL8 structure used in the C++ code
#[derive(Debug, Clone)]
pub struct CubeMapMaterial {
    pub ambient: Vec4,
    pub diffuse: Vec4,
    pub specular: Vec4,
    pub emissive: Vec4,
    pub power: f32,
}

impl CubeMapMaterial {
    /// Create a new cube map material from color components
    pub fn new(ambient: Vec3, diffuse: Vec3, specular: Vec3, power: f32) -> Self {
        Self {
            ambient: ambient.extend(1.0),
            diffuse: diffuse.extend(1.0),
            specular: specular.extend(1.0),
            emissive: Vec4::ZERO,
            power,
        }
    }
}

impl Default for CubeMapMaterial {
    fn default() -> Self {
        Self {
            ambient: Vec4::ONE,
            diffuse: Vec4::ONE,
            specular: Vec4::ZERO,
            emissive: Vec4::ZERO,
            power: 20.0,
        }
    }
}

/// Cube map texture types supported
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CubeMapType {
    /// Standard 6-face cube map
    SixFace,
    /// Spherical environment map projected to cube
    Spherical,
    /// Array texture with 6 layers
    TextureArray,
}

impl Default for CubeMapType {
    fn default() -> Self {
        CubeMapType::SixFace
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cubemap_shader_def_creation() {
        let def = CubeMapShaderDef::new().with_texture("test.dds");

        assert_eq!(def.get_texture_name(), "test.dds");
        assert_eq!(def.get_ambient(), Vec3::ONE);
        assert_eq!(def.get_diffuse(), Vec3::ONE);
        assert_eq!(def.get_specular(), Vec3::ZERO);
        assert_eq!(def.get_class_id(), SHDDEF_CLASSID_CUBEMAP);
    }

    #[test]
    fn test_cubemap_shader_def_validation() {
        let mut def = CubeMapShaderDef::new();

        // Should fail without texture
        assert!(def.is_valid_config().is_err());

        // Should pass with texture
        def.set_texture_name("valid.dds");
        assert!(def.is_valid_config().is_ok());
    }

    #[test]
    fn test_cubemap_shader_def_properties() {
        let mut def = CubeMapShaderDef::new();

        assert!(def.requires_normals());
        assert!(!def.requires_tangent_space_vectors());
        assert!(!def.requires_sorting());
        assert_eq!(def.static_sort_index(), 0);

        def.set_ambient(Vec3::new(0.2, 0.3, 0.4));
        def.set_diffuse(Vec3::new(0.8, 0.9, 1.0));
        def.set_specular(Vec3::new(0.5, 0.6, 0.7));

        assert_eq!(def.get_ambient(), Vec3::new(0.2, 0.3, 0.4));
        assert_eq!(def.get_diffuse(), Vec3::new(0.8, 0.9, 1.0));
        assert_eq!(def.get_specular(), Vec3::new(0.5, 0.6, 0.7));
    }

    #[test]
    fn test_cubemap_shader_creation() {
        let def = CubeMapShaderDef::new().with_texture("test.dds");
        let shader = CubeMapShader::new(&def).unwrap();

        assert_eq!(shader.get_class_id(), SHDDEF_CLASSID_CUBEMAP);
        assert_eq!(shader.get_pass_count(), 1);
        assert!(shader.is_opaque());
        assert_eq!(shader.get_texture_name(), "test.dds");
        assert_eq!(shader.get_vertex_stream_count(), 1);
        assert_eq!(shader.get_vertex_size(0), 28);
        assert!(shader.use_hardware_vertex_processing());
        assert_eq!(shader.get_texture_count(), 1);
        assert_eq!(shader.get_specular_power(), 20.0);
    }

    #[test]
    fn test_cubemap_vertex_creation() {
        let vertex = CubeMapVertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], 0xFF0000FF);

        assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.normal, [0.0, 1.0, 0.0]);
        assert_eq!(vertex.diffuse, 0xFF0000FF);
    }

    #[test]
    fn test_cubemap_material_creation() {
        let material = CubeMapMaterial::new(
            Vec3::new(0.1, 0.1, 0.1),
            Vec3::new(0.8, 0.8, 0.8),
            Vec3::new(1.0, 1.0, 1.0),
            32.0,
        );

        assert_eq!(material.ambient, Vec4::new(0.1, 0.1, 0.1, 1.0));
        assert_eq!(material.diffuse, Vec4::new(0.8, 0.8, 0.8, 1.0));
        assert_eq!(material.specular, Vec4::new(1.0, 1.0, 1.0, 1.0));
        assert_eq!(material.power, 32.0);
    }

    #[test]
    fn test_cubemap_shader_serialization() {
        let def = CubeMapShaderDef::new().with_texture("serialize_test.dds");

        let data = def.save().unwrap();

        let mut loaded_def = CubeMapShaderDef::default();
        loaded_def.load(&data).unwrap();

        assert_eq!(loaded_def.get_texture_name(), "serialize_test.dds");
        assert_eq!(loaded_def.get_ambient(), def.get_ambient());
        assert_eq!(loaded_def.get_diffuse(), def.get_diffuse());
        assert_eq!(loaded_def.get_specular(), def.get_specular());
    }

    #[test]
    fn test_copy_cubemap_vertex_stream() {
        let mut dest_buffer = vec![CubeMapVertex::default(); 2];

        let mut vertex_streams = VertexStreams::new();
        vertex_streams.positions = Some(vec![
            glam::Vec3::new(1.0, 2.0, 3.0),
            glam::Vec3::new(4.0, 5.0, 6.0),
        ]);
        vertex_streams.normals = Some(vec![glam::Vec3::Y, glam::Vec3::Z]);
        vertex_streams.colors_int = Some(vec![0xFF0000FF, 0x00FF00FF]);

        copy_vertex_stream(&mut dest_buffer, &vertex_streams).unwrap();

        assert_eq!(dest_buffer[0].position, [1.0, 2.0, 3.0]);
        assert_eq!(dest_buffer[0].normal, [0.0, 1.0, 0.0]);
        assert_eq!(dest_buffer[0].diffuse, 0xFF0000FF);

        assert_eq!(dest_buffer[1].position, [4.0, 5.0, 6.0]);
        assert_eq!(dest_buffer[1].normal, [0.0, 0.0, 1.0]);
        assert_eq!(dest_buffer[1].diffuse, 0x00FF00FF);
    }

    #[test]
    fn test_cubemap_type_enum() {
        let cube_type = CubeMapType::default();
        assert_eq!(cube_type, CubeMapType::SixFace);

        assert_ne!(CubeMapType::SixFace, CubeMapType::Spherical);
        assert_ne!(CubeMapType::Spherical, CubeMapType::TextureArray);
    }
}
