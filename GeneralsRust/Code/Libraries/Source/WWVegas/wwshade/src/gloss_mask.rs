//! Gloss Mask Shader Implementation
//!
//! This module implements a gloss mask shader equivalent to the C++ ShdGlossMaskDefClass.
//! It provides texture rendering with specular masking using the texture's alpha channel
//! for controlling gloss intensity.

use crate::{
    class_ids::SHDDEF_CLASSID_GLOSSMASK,
    def::ShdDefClass,
    error::{ShdError, ShdResult},
    interface::{RenderInfo, ShdInterface, VertexStreams},
};
use glam::{Mat4, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::any::Any;

/// Gloss Mask Shader Definition
///
/// This shader definition provides gloss-masked rendering with ambient, diffuse, and specular colors.
/// It's equivalent to the C++ ShdGlossMaskDefClass and represents a shader that uses
/// the alpha channel of a texture to mask specular highlights.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GlossMaskShaderDef {
    /// Base shader properties
    name: String,
    surface_type: i32,

    /// Texture file name/path
    texture_name: String,

    /// Ambient color component
    ambient: Vec3,

    /// Diffuse color component  
    diffuse: Vec3,

    /// Specular color component
    specular: Vec3,
}

impl Default for GlossMaskShaderDef {
    fn default() -> Self {
        Self {
            name: "Gloss Mask".to_string(),
            surface_type: 0,
            texture_name: String::new(),
            ambient: Vec3::ONE,  // Default white ambient
            diffuse: Vec3::ONE,  // Default white diffuse
            specular: Vec3::ONE, // Default white specular (as in C++ code)
        }
    }
}

impl GlossMaskShaderDef {
    /// Create a new gloss mask shader definition
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a gloss mask shader with a specific texture
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

    /// Set the specular color
    pub fn set_specular(&mut self, specular: Vec3) {
        self.specular = specular;
    }

    /// Get the specular color
    pub fn get_specular(&self) -> Vec3 {
        self.specular
    }
}

impl ShdDefClass for GlossMaskShaderDef {
    fn get_class_id(&self) -> u32 {
        SHDDEF_CLASSID_GLOSSMASK
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
        Ok(Box::new(GlossMaskShader::new(self)?))
    }

    fn is_valid_config(&self) -> ShdResult<()> {
        if self.texture_name.is_empty() {
            return Err(ShdError::InvalidConfig(
                "Gloss mask shader requires a texture name".to_string(),
            ));
        }
        Ok(())
    }

    fn requires_normals(&self) -> bool {
        true // Gloss mask shader needs normals for lighting calculations
    }

    fn requires_tangent_space_vectors(&self) -> bool {
        false // Gloss mask shader doesn't need tangent vectors
    }

    fn requires_sorting(&self) -> bool {
        false // Gloss mask shader is typically opaque
    }

    fn static_sort_index(&self) -> i32 {
        0
    }

    fn save(&self) -> ShdResult<Vec<u8>> {
        bincode::serialize(self).map_err(|e| ShdError::Serialization(e.to_string()))
    }

    fn load(&mut self, data: &[u8]) -> ShdResult<()> {
        let loaded: GlossMaskShaderDef =
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

/// Gloss Mask Shader Implementation
///
/// This is the actual shader implementation that performs the gloss-masked rendering.
/// It's equivalent to the C++ Shd6GlossMaskClass.
#[derive(Debug)]
pub struct GlossMaskShader {
    class_id: u32,
    texture_name: String,
    ambient: Vec4,
    diffuse: Vec4,
    specular: Vec4,
    specular_power: f32,

    // Hardware capability flags
    supports_mod_alpha_add_color: bool,

    // Render state
    _view_projection_matrix: Mat4,
}

impl GlossMaskShader {
    /// Create a new gloss mask shader from a definition
    pub fn new(def: &GlossMaskShaderDef) -> ShdResult<Self> {
        let ambient = def.ambient.extend(1.0);
        let diffuse = def.diffuse.extend(1.0);
        let specular = def.specular.extend(1.0);

        Ok(Self {
            class_id: SHDDEF_CLASSID_GLOSSMASK,
            texture_name: def.texture_name.clone(),
            ambient,
            diffuse,
            specular,
            specular_power: 20.0,               // Default power as in C++ code
            supports_mod_alpha_add_color: true, // Assume modern hardware support
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

    /// Check if the hardware supports ModAlphaAddColor texture operations
    pub fn supports_mod_alpha_add_color(&self) -> bool {
        self.supports_mod_alpha_add_color
    }

    /// Set the hardware capability flag for ModAlphaAddColor
    pub fn set_mod_alpha_add_color_support(&mut self, supported: bool) {
        self.supports_mod_alpha_add_color = supported;
    }
}

impl ShdInterface for GlossMaskShader {
    fn get_class_id(&self) -> u32 {
        self.class_id
    }

    fn get_pass_count(&self) -> u32 {
        1 // Gloss mask shader only needs one rendering pass
    }

    fn is_opaque(&self) -> bool {
        true // Gloss mask shader is typically opaque
    }

    fn apply_shared(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // In the original C++ code, this would set up DirectX texture stage states
        // based on hardware capabilities
        if _pass >= self.get_pass_count() {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                _pass
            )));
        }

        // The C++ code has two paths:
        // 1. If ModAlphaAddColor is supported:
        //    - Stage 0: Modulate texture with diffuse
        //    - Stage 1: ModulateAlpha_AddColor with specular
        // 2. If not supported:
        //    - Stage 0: Simple modulate texture with diffuse
        //    - No specular highlighting

        // Modern equivalent would be setting up multi-pass rendering or
        // advanced texture blending operations in the shader
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn apply_instance(&mut self, _pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        // In the original C++ code, this would bind the texture(s) and set materials
        if _pass >= self.get_pass_count() {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                _pass
            )));
        }

        // The C++ code binds:
        // - The texture to stage 0
        // - If ModAlphaAddColor is supported, also binds the same texture to stage 1
        // - Sets the material properties

        // Modern equivalent would be binding textures and updating material uniforms
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn get_vertex_stream_count(&self) -> u32 {
        1 // Gloss mask shader uses a single vertex stream
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
        if self.supports_mod_alpha_add_color {
            2 // Uses the same texture in two stages when advanced blending is available
        } else {
            1 // Uses single texture when advanced blending is not available
        }
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

/// Vertex format used by the gloss mask shader
/// Equivalent to VertexFormatXYZNDUV1 in the C++ code
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GlossMaskVertex {
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Normal vector (nx, ny, nz)  
    pub normal: [f32; 3],
    /// Diffuse color (32-bit RGBA)
    pub diffuse: u32,
    /// Texture coordinates (u, v)
    pub tex_coords: [f32; 2],
}

impl Default for GlossMaskVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            normal: [0.0; 3],
            diffuse: 0xFFFFFFFF, // Default to white
            tex_coords: [0.0; 2],
        }
    }
}

impl GlossMaskVertex {
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

/// Utility function to copy vertex stream data into the gloss mask vertex format
/// This is equivalent to the Copy_Vertex_Stream method in the C++ code
pub fn copy_vertex_stream(
    dest_buffer: &mut [GlossMaskVertex],
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

/// Material properties for gloss mask rendering
/// This corresponds to the D3DMATERIAL8 structure used in the C++ code
#[derive(Debug, Clone)]
pub struct GlossMaskMaterial {
    pub ambient: Vec4,
    pub diffuse: Vec4,
    pub specular: Vec4,
    pub emissive: Vec4,
    pub power: f32,
}

impl GlossMaskMaterial {
    /// Create a new gloss mask material from color components
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

impl Default for GlossMaskMaterial {
    fn default() -> Self {
        Self {
            ambient: Vec4::ONE,
            diffuse: Vec4::ONE,
            specular: Vec4::ONE, // Gloss mask shader defaults to white specular
            emissive: Vec4::ZERO,
            power: 20.0,
        }
    }
}

/// Texture stage blending modes for gloss mask rendering
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlossBlendMode {
    /// Simple diffuse texture modulation (fallback mode)
    Simple,
    /// Advanced blending with specular highlights (preferred mode)
    ModulateAlphaAddColor,
}

impl Default for GlossBlendMode {
    fn default() -> Self {
        GlossBlendMode::ModulateAlphaAddColor
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_gloss_mask_shader_def_creation() {
        let def = GlossMaskShaderDef::new().with_texture("test.tga");

        assert_eq!(def.get_texture_name(), "test.tga");
        assert_eq!(def.get_ambient(), Vec3::ONE);
        assert_eq!(def.get_diffuse(), Vec3::ONE);
        assert_eq!(def.get_specular(), Vec3::ONE);
        assert_eq!(def.get_class_id(), SHDDEF_CLASSID_GLOSSMASK);
    }

    #[test]
    fn test_gloss_mask_shader_def_validation() {
        let mut def = GlossMaskShaderDef::new();

        // Should fail without texture
        assert!(def.is_valid_config().is_err());

        // Should pass with texture
        def.set_texture_name("valid.tga");
        assert!(def.is_valid_config().is_ok());
    }

    #[test]
    fn test_gloss_mask_shader_def_properties() {
        let mut def = GlossMaskShaderDef::new();

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
    fn test_gloss_mask_shader_creation() {
        let def = GlossMaskShaderDef::new().with_texture("test.tga");
        let shader = GlossMaskShader::new(&def).unwrap();

        assert_eq!(shader.get_class_id(), SHDDEF_CLASSID_GLOSSMASK);
        assert_eq!(shader.get_pass_count(), 1);
        assert!(shader.is_opaque());
        assert_eq!(shader.get_texture_name(), "test.tga");
        assert_eq!(shader.get_vertex_stream_count(), 1);
        assert_eq!(shader.get_vertex_size(0), 36);
        assert!(shader.use_hardware_vertex_processing());
        assert_eq!(shader.get_specular_power(), 20.0);
        assert!(shader.supports_mod_alpha_add_color());
    }

    #[test]
    fn test_gloss_mask_texture_count_based_on_capability() {
        let def = GlossMaskShaderDef::new().with_texture("test.tga");
        let mut shader = GlossMaskShader::new(&def).unwrap();

        // With advanced blending support
        assert_eq!(shader.get_texture_count(), 2);

        // Without advanced blending support
        shader.set_mod_alpha_add_color_support(false);
        assert_eq!(shader.get_texture_count(), 1);
    }

    #[test]
    fn test_gloss_mask_vertex_creation() {
        let vertex = GlossMaskVertex::new([1.0, 2.0, 3.0], [0.0, 1.0, 0.0], 0xFF0000FF, [0.5, 0.5]);

        assert_eq!(vertex.position, [1.0, 2.0, 3.0]);
        assert_eq!(vertex.normal, [0.0, 1.0, 0.0]);
        assert_eq!(vertex.diffuse, 0xFF0000FF);
        assert_eq!(vertex.tex_coords, [0.5, 0.5]);
    }

    #[test]
    fn test_gloss_mask_material_creation() {
        let material = GlossMaskMaterial::new(
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
    fn test_gloss_mask_shader_serialization() {
        let def = GlossMaskShaderDef::new().with_texture("serialize_test.tga");

        let data = def.save().unwrap();

        let mut loaded_def = GlossMaskShaderDef::default();
        loaded_def.load(&data).unwrap();

        assert_eq!(loaded_def.get_texture_name(), "serialize_test.tga");
        assert_eq!(loaded_def.get_ambient(), def.get_ambient());
        assert_eq!(loaded_def.get_diffuse(), def.get_diffuse());
        assert_eq!(loaded_def.get_specular(), def.get_specular());
    }

    #[test]
    fn test_copy_gloss_mask_vertex_stream() {
        let mut dest_buffer = vec![GlossMaskVertex::default(); 2];

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

    #[test]
    fn test_gloss_blend_mode_enum() {
        let blend_mode = GlossBlendMode::default();
        assert_eq!(blend_mode, GlossBlendMode::ModulateAlphaAddColor);

        assert_ne!(
            GlossBlendMode::Simple,
            GlossBlendMode::ModulateAlphaAddColor
        );
    }
}
