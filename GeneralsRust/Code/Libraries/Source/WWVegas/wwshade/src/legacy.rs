//! Legacy W3D Shader Implementation
//!
//! This module implements a legacy W3D compatibility shader equivalent to the C++ ShdLegacyW3DDefClass.
//! It provides backward compatibility with the original W3D material system, wrapping legacy
//! materials and shader descriptions in the modern shader interface.

use crate::{
    class_ids::SHDDEF_CLASSID_LEGACYW3D,
    def::ShdDefClass,
    error::{ShdError, ShdResult},
    interface::{RenderInfo, ShdInterface},
};
use glam::{Mat4, Vec3};
use serde::{Deserialize, Serialize};
use std::any::Any;
use std::cmp::Ordering;

/// Maximum number of rendering passes for legacy shaders
pub const MAX_LEGACY_PASSES: usize = 4;

/// Maximum number of texture stages per pass
pub const MAX_TEXTURE_STAGES: usize = 2;

/// Legacy W3D Shader Definition
///
/// This shader definition provides compatibility with the original W3D material system.
/// It's equivalent to the C++ ShdLegacyW3DDefClass and is used to wrap legacy
/// material descriptions in the modern shader framework.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LegacyW3DShaderDef {
    /// Base shader properties
    name: String,
    surface_type: i32,

    /// Number of rendering passes
    pass_count: usize,

    /// Texture names for each pass and stage
    texture_names: [[String; MAX_TEXTURE_STAGES]; MAX_LEGACY_PASSES],

    /// Texture attributes (filtering, addressing, etc.)
    texture_attributes: [[u32; MAX_TEXTURE_STAGES]; MAX_LEGACY_PASSES],

    /// Legacy shader descriptions
    shaders: [LegacyShaderDesc; MAX_LEGACY_PASSES],

    /// Legacy material descriptions  
    materials: [LegacyMaterialDesc; MAX_LEGACY_PASSES],

    /// Texture mapper arguments
    mapper_args: [[String; MAX_TEXTURE_STAGES]; MAX_LEGACY_PASSES],

    /// UV mapping channels
    map_channels: [[i32; MAX_TEXTURE_STAGES]; MAX_LEGACY_PASSES],
}

impl Default for LegacyW3DShaderDef {
    fn default() -> Self {
        Self {
            name: "Legacy W3D".to_string(),
            surface_type: 0,
            pass_count: 0,
            texture_names: Default::default(),
            texture_attributes: Default::default(),
            shaders: Default::default(),
            materials: Default::default(),
            mapper_args: Default::default(),
            map_channels: Default::default(),
        }
    }
}

impl LegacyW3DShaderDef {
    /// Create a new legacy W3D shader definition
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the number of passes
    pub fn get_pass_count(&self) -> usize {
        self.pass_count
    }

    /// Set the number of passes
    pub fn set_pass_count(&mut self, count: usize) {
        self.pass_count = count;
    }

    /// Get the texture name for a specific pass and stage
    pub fn get_texture_name(&self, pass: usize, stage: usize) -> &str {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            &self.texture_names[pass][stage]
        } else {
            ""
        }
    }

    /// Set the texture name for a specific pass and stage
    pub fn set_texture_name(&mut self, pass: usize, stage: usize, name: String) {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            self.texture_names[pass][stage] = name;
        }
    }

    /// Get the texture attributes for a specific pass and stage
    pub fn get_texture_attributes(&self, pass: usize, stage: usize) -> u32 {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            self.texture_attributes[pass][stage]
        } else {
            0
        }
    }

    /// Set the texture attributes for a specific pass and stage
    pub fn set_texture_attributes(&mut self, pass: usize, stage: usize, attributes: u32) {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            self.texture_attributes[pass][stage] = attributes;
        }
    }

    /// Get the shader description for a specific pass
    pub fn get_shader(&self, pass: usize) -> LegacyShaderDesc {
        if pass < MAX_LEGACY_PASSES {
            self.shaders[pass]
        } else {
            LegacyShaderDesc::default()
        }
    }

    /// Set the shader description for a specific pass
    pub fn set_shader(&mut self, pass: usize, shader: LegacyShaderDesc) {
        if pass < MAX_LEGACY_PASSES {
            self.shaders[pass] = shader;
        }
    }

    /// Get the material description for a specific pass
    pub fn get_material(&self, pass: usize) -> LegacyMaterialDesc {
        if pass < MAX_LEGACY_PASSES {
            self.materials[pass]
        } else {
            LegacyMaterialDesc::default()
        }
    }

    /// Set the material description for a specific pass
    pub fn set_material(&mut self, pass: usize, material: LegacyMaterialDesc) {
        if pass < MAX_LEGACY_PASSES {
            self.materials[pass] = material;
        }
    }

    /// Get the mapper arguments for a specific pass and stage
    pub fn get_mapper_args(&self, pass: usize, stage: usize) -> &str {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            &self.mapper_args[pass][stage]
        } else {
            ""
        }
    }

    /// Set the mapper arguments for a specific pass and stage
    pub fn set_mapper_args(&mut self, pass: usize, stage: usize, args: String) {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            self.mapper_args[pass][stage] = args;
        }
    }

    /// Get the UV mapping channel for a specific pass and stage
    pub fn get_map_channel(&self, pass: usize, stage: usize) -> i32 {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            self.map_channels[pass][stage]
        } else {
            0
        }
    }

    /// Set the UV mapping channel for a specific pass and stage
    pub fn set_map_channel(&mut self, pass: usize, stage: usize, channel: i32) {
        if pass < MAX_LEGACY_PASSES && stage < MAX_TEXTURE_STAGES {
            self.map_channels[pass][stage] = channel;
        }
    }
}

impl ShdDefClass for LegacyW3DShaderDef {
    fn get_class_id(&self) -> u32 {
        SHDDEF_CLASSID_LEGACYW3D
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
        Ok(Box::new(LegacyW3DShader::new(self)?))
    }

    fn is_valid_config(&self) -> ShdResult<()> {
        if self.pass_count == 0 {
            return Err(ShdError::InvalidConfig(
                "Legacy shader requires at least one pass".to_string(),
            ));
        }
        if self.pass_count > MAX_LEGACY_PASSES {
            return Err(ShdError::InvalidConfig(format!(
                "Legacy shader supports maximum {} passes, got {}",
                MAX_LEGACY_PASSES, self.pass_count
            )));
        }
        Ok(())
    }

    fn uses_uv_channel(&self, channel: u32) -> bool {
        for pass in 0..self.pass_count.min(MAX_LEGACY_PASSES) {
            for stage in 0..MAX_TEXTURE_STAGES {
                if self.map_channels[pass][stage] as u32 == channel {
                    return true;
                }
            }
        }
        false
    }

    fn requires_normals(&self) -> bool {
        true // Legacy shaders typically need normals for lighting
    }

    fn requires_tangent_space_vectors(&self) -> bool {
        false // Legacy shaders don't use tangent space vectors
    }

    fn requires_sorting(&self) -> bool {
        // Check if any pass uses blending (non-zero source blend)
        for pass in 0..self.pass_count.min(MAX_LEGACY_PASSES) {
            if self.shaders[pass].dest_blend != LegacyBlendMode::Zero {
                return true;
            }
        }
        false
    }

    fn static_sort_index(&self) -> i32 {
        0
    }

    fn save(&self) -> ShdResult<Vec<u8>> {
        bincode::serialize(self).map_err(|e| ShdError::Serialization(e.to_string()))
    }

    fn load(&mut self, data: &[u8]) -> ShdResult<()> {
        let loaded: LegacyW3DShaderDef =
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

/// Legacy Shader Description
///
/// This corresponds to the W3dShaderStruct in the original C++ code
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LegacyShaderDesc {
    pub depth_compare: LegacyDepthCompare,
    pub depth_mask: bool,
    pub color_mask: LegacyColorMask,
    pub dest_blend: LegacyBlendMode,
    pub fog_func: LegacyFogMode,
    pub primary_gradient: LegacyGradientMode,
    pub secondary_gradient: LegacyGradientMode,
    pub src_blend: LegacyBlendMode,
    pub texturing: LegacyTexturingMode,
    pub detail_color_func: LegacyDetailColorMode,
    pub detail_alpha_func: LegacyDetailAlphaMode,
    pub shader_preset: LegacyShaderPreset,
    pub alpha_test: bool,
    pub post_detail_color_func: LegacyDetailColorMode,
    pub post_detail_alpha_func: LegacyDetailAlphaMode,
}

impl Default for LegacyShaderDesc {
    fn default() -> Self {
        Self {
            depth_compare: LegacyDepthCompare::LessEqual,
            depth_mask: true,
            color_mask: LegacyColorMask::All,
            dest_blend: LegacyBlendMode::Zero,
            fog_func: LegacyFogMode::None,
            primary_gradient: LegacyGradientMode::None,
            secondary_gradient: LegacyGradientMode::None,
            src_blend: LegacyBlendMode::One,
            texturing: LegacyTexturingMode::Enable,
            detail_color_func: LegacyDetailColorMode::Disable,
            detail_alpha_func: LegacyDetailAlphaMode::Disable,
            shader_preset: LegacyShaderPreset::Opaque,
            alpha_test: false,
            post_detail_color_func: LegacyDetailColorMode::Disable,
            post_detail_alpha_func: LegacyDetailAlphaMode::Disable,
        }
    }
}

/// Legacy Material Description
///
/// This corresponds to the W3dVertexMaterialStruct in the original C++ code
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct LegacyMaterialDesc {
    pub ambient: Vec3,
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub emissive: Vec3,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

impl Default for LegacyMaterialDesc {
    fn default() -> Self {
        Self {
            ambient: Vec3::splat(0.3),
            diffuse: Vec3::ONE,
            specular: Vec3::ZERO,
            emissive: Vec3::ZERO,
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }
}

/// Legacy W3D Shader Implementation
///
/// This is the actual shader implementation that wraps legacy W3D materials.
/// It's equivalent to the C++ Shd6LegacyW3DClass.
#[derive(Debug)]
pub struct LegacyW3DShader {
    class_id: u32,
    pass_count: u32,

    // Legacy data converted to modern formats
    texture_names: Vec<Vec<String>>,
    shaders: Vec<LegacyShaderDesc>,
    materials: Vec<LegacyMaterialDesc>,

    // Flexible Vertex Format for dynamic vertex layout
    vertex_format: LegacyVertexFormat,

    // Render state
    _view_projection_matrix: Mat4,
}

impl LegacyW3DShader {
    /// Create a new legacy W3D shader from a definition
    pub fn new(def: &LegacyW3DShaderDef) -> ShdResult<Self> {
        let mut texture_names = Vec::with_capacity(def.pass_count);
        let mut shaders = Vec::with_capacity(def.pass_count);
        let mut materials = Vec::with_capacity(def.pass_count);

        for pass in 0..def.pass_count {
            let mut pass_textures = Vec::new();
            for stage in 0..MAX_TEXTURE_STAGES {
                let tex_name = def.get_texture_name(pass, stage);
                if !tex_name.is_empty() {
                    pass_textures.push(tex_name.to_string());
                }
            }
            texture_names.push(pass_textures);
            shaders.push(def.get_shader(pass));
            materials.push(def.get_material(pass));
        }

        // Determine vertex format based on used UV channels
        let mut vertex_format = LegacyVertexFormat::new();
        for pass in 0..def.pass_count {
            for stage in 0..MAX_TEXTURE_STAGES {
                let channel = def.get_map_channel(pass, stage);
                if channel > 0 {
                    vertex_format.add_uv_channel(channel as u32);
                }
            }
        }

        Ok(Self {
            class_id: SHDDEF_CLASSID_LEGACYW3D,
            pass_count: def.pass_count as u32,
            texture_names,
            shaders,
            materials,
            vertex_format,
            _view_projection_matrix: Mat4::IDENTITY,
        })
    }

    /// Get the legacy shader description for a specific pass
    pub fn get_shader(&self, pass: usize) -> Option<&LegacyShaderDesc> {
        self.shaders.get(pass)
    }

    /// Get the legacy material description for a specific pass
    pub fn get_material(&self, pass: usize) -> Option<&LegacyMaterialDesc> {
        self.materials.get(pass)
    }

    /// Get the texture names for a specific pass
    pub fn get_textures(&self, pass: usize) -> Option<&Vec<String>> {
        self.texture_names.get(pass)
    }

    /// Check if this shader is opaque (no blending)
    pub fn is_opaque_pass(&self, pass: usize) -> bool {
        if let Some(shader) = self.get_shader(pass) {
            shader.dest_blend == LegacyBlendMode::Zero
        } else {
            true
        }
    }

    /// Get the vertex format used by this shader
    pub fn get_vertex_format(&self) -> &LegacyVertexFormat {
        &self.vertex_format
    }
}

impl ShdInterface for LegacyW3DShader {
    fn get_class_id(&self) -> u32 {
        self.class_id
    }

    fn get_pass_count(&self) -> u32 {
        self.pass_count
    }

    fn is_opaque(&self) -> bool {
        // Consider opaque if the first pass is opaque
        self.is_opaque_pass(0)
    }

    fn apply_shared(&mut self, pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        if pass >= self.pass_count {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                pass
            )));
        }

        // In the original C++ code, this would set texture coordinate generation to pass-through
        // and configure other shared render states for legacy rendering

        // Modern equivalent would be setting up the appropriate shader program
        // and configuring texture sampling states
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn apply_instance(&mut self, pass: u32, _render_info: &RenderInfo) -> ShdResult<()> {
        if pass >= self.pass_count {
            return Err(ShdError::InvalidParameter(format!(
                "Invalid pass index: {}",
                pass
            )));
        }

        // In the original C++ code, this would:
        // - Set the legacy shader render states
        // - Set the legacy material properties
        // - Bind textures for all stages
        // - Configure the vertex format

        // Modern equivalent would be binding textures, updating material uniforms,
        // and configuring the vertex input layout
        // This would be implemented when integrating with wgpu or another graphics API

        Ok(())
    }

    fn compare_for_sorting(&self, other: &dyn ShdInterface, pass: u32) -> Ordering {
        let class_cmp = self.get_class_id().cmp(&other.get_class_id());
        if class_cmp != Ordering::Equal {
            return class_cmp;
        }

        let Some(other_legacy) = (other as &dyn Any).downcast_ref::<LegacyW3DShader>() else {
            return Ordering::Equal;
        };

        let pass = pass as usize;
        let self_textures: &[String] = self
            .texture_names
            .get(pass)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        let other_textures: &[String] = other_legacy
            .texture_names
            .get(pass)
            .map(Vec::as_slice)
            .unwrap_or(&[]);
        self_textures.cmp(other_textures)
    }

    fn get_vertex_stream_count(&self) -> u32 {
        1 // Legacy shader uses a single vertex stream
    }

    fn get_vertex_size(&self, stream: u32) -> u32 {
        if stream >= self.get_vertex_stream_count() {
            return 0;
        }

        self.vertex_format.get_vertex_size()
    }

    fn use_hardware_vertex_processing(&self) -> bool {
        true // Modern hardware should use HW vertex processing
    }

    fn get_texture_count(&self) -> u32 {
        0 // Legacy shader manages its own textures
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

/// Flexible vertex format for legacy shaders
#[derive(Debug, Clone)]
pub struct LegacyVertexFormat {
    has_position: bool,
    has_normal: bool,
    has_diffuse: bool,
    uv_channels: Vec<u32>,
}

impl LegacyVertexFormat {
    /// Create a new legacy vertex format
    pub fn new() -> Self {
        Self {
            has_position: true,   // Always have position
            has_normal: true,     // Always have normal for lighting
            has_diffuse: true,    // Always have diffuse color
            uv_channels: vec![0], // Start with UV channel 0
        }
    }

    /// Add a UV channel to the vertex format
    pub fn add_uv_channel(&mut self, channel: u32) {
        if !self.uv_channels.contains(&channel) {
            self.uv_channels.push(channel);
            self.uv_channels.sort_unstable();
        }
    }

    /// Get the size of a vertex in bytes
    pub fn get_vertex_size(&self) -> u32 {
        let mut size = 0u32;

        if self.has_position {
            size += 12; // 3 floats for position
        }
        if self.has_normal {
            size += 12; // 3 floats for normal
        }
        if self.has_diffuse {
            size += 4; // 1 uint32 for diffuse color
        }

        size += (self.uv_channels.len() as u32) * 8; // 2 floats per UV channel

        size
    }

    /// Get the number of UV channels
    pub fn get_uv_count(&self) -> usize {
        self.uv_channels.len()
    }
}

impl Default for LegacyVertexFormat {
    fn default() -> Self {
        Self::new()
    }
}

// Legacy enums matching the original C++ values

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyDepthCompare {
    Never = 0,
    Less = 1,
    Equal = 2,
    LessEqual = 3,
    Greater = 4,
    NotEqual = 5,
    GreaterEqual = 6,
    Always = 7,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyColorMask {
    None = 0,
    Red = 1,
    Green = 2,
    Blue = 4,
    Alpha = 8,
    All = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyBlendMode {
    Zero = 0,
    One = 1,
    SrcColor = 2,
    OneMinusSrcColor = 3,
    SrcAlpha = 4,
    OneMinusSrcAlpha = 5,
    DstAlpha = 6,
    OneMinusDstAlpha = 7,
    DstColor = 8,
    OneMinusDstColor = 9,
    SrcAlphaSat = 10,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyFogMode {
    None = 0,
    Linear = 1,
    Exp = 2,
    Exp2 = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyGradientMode {
    None = 0,
    Modulate = 1,
    Add = 2,
    BumpEnvMap = 3,
    BumpEnvMapLuminance = 4,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyTexturingMode {
    Disable = 0,
    Enable = 1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyDetailColorMode {
    Disable = 0,
    Detail = 1,
    Scale = 2,
    InvScale = 3,
    Add = 4,
    SubR = 5,
    SubG = 6,
    SubB = 7,
    AddR = 8,
    AddG = 9,
    AddB = 10,
    DetailGBR = 11,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyDetailAlphaMode {
    Disable = 0,
    Detail = 1,
    Scale = 2,
    InvScale = 3,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LegacyShaderPreset {
    Opaque = 0,
    AlphaTest = 1,
    AlphaBlend = 2,
    Screen = 3,
    Additive = 4,
    Multiply = 5,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_legacy_shader_def_creation() {
        let mut def = LegacyW3DShaderDef::new();
        def.set_pass_count(2);
        def.set_texture_name(0, 0, "texture0.tga".to_string());
        def.set_texture_name(1, 0, "texture1.tga".to_string());

        assert_eq!(def.get_pass_count(), 2);
        assert_eq!(def.get_texture_name(0, 0), "texture0.tga");
        assert_eq!(def.get_texture_name(1, 0), "texture1.tga");
        assert_eq!(def.get_class_id(), SHDDEF_CLASSID_LEGACYW3D);
    }

    #[test]
    fn test_legacy_shader_def_validation() {
        let mut def = LegacyW3DShaderDef::new();

        // Should fail with no passes
        assert!(def.is_valid_config().is_err());

        // Should pass with valid pass count
        def.set_pass_count(1);
        assert!(def.is_valid_config().is_ok());

        // Should fail with too many passes
        def.set_pass_count(MAX_LEGACY_PASSES + 1);
        assert!(def.is_valid_config().is_err());
    }

    #[test]
    fn test_legacy_shader_def_uv_channels() {
        let mut def = LegacyW3DShaderDef::new();
        def.set_pass_count(2);
        def.set_map_channel(0, 0, 0);
        def.set_map_channel(0, 1, 1);
        def.set_map_channel(1, 0, 2);

        assert!(def.uses_uv_channel(0));
        assert!(def.uses_uv_channel(1));
        assert!(def.uses_uv_channel(2));
        assert!(!def.uses_uv_channel(3));
    }

    #[test]
    fn test_legacy_shader_creation() {
        let mut def = LegacyW3DShaderDef::new();
        def.set_pass_count(1);
        def.set_texture_name(0, 0, "test.tga".to_string());

        let shader = LegacyW3DShader::new(&def).unwrap();

        assert_eq!(shader.get_class_id(), SHDDEF_CLASSID_LEGACYW3D);
        assert_eq!(shader.get_pass_count(), 1);
        assert!(shader.is_opaque());
        assert_eq!(shader.get_vertex_stream_count(), 1);
        assert!(shader.use_hardware_vertex_processing());
        assert_eq!(shader.get_texture_count(), 0);
    }

    #[test]
    fn test_legacy_shader_desc_default() {
        let shader_desc = LegacyShaderDesc::default();

        assert_eq!(shader_desc.depth_compare, LegacyDepthCompare::LessEqual);
        assert!(shader_desc.depth_mask);
        assert_eq!(shader_desc.color_mask, LegacyColorMask::All);
        assert_eq!(shader_desc.dest_blend, LegacyBlendMode::Zero);
        assert_eq!(shader_desc.src_blend, LegacyBlendMode::One);
    }

    #[test]
    fn test_legacy_material_desc_default() {
        let material_desc = LegacyMaterialDesc::default();

        assert_eq!(material_desc.ambient, Vec3::splat(0.3));
        assert_eq!(material_desc.diffuse, Vec3::ONE);
        assert_eq!(material_desc.specular, Vec3::ZERO);
        assert_eq!(material_desc.emissive, Vec3::ZERO);
        assert_eq!(material_desc.shininess, 1.0);
        assert_eq!(material_desc.opacity, 1.0);
        assert_eq!(material_desc.translucency, 0.0);
    }

    #[test]
    fn test_legacy_vertex_format() {
        let mut format = LegacyVertexFormat::new();

        // Default format: position + normal + diffuse + UV0
        assert_eq!(format.get_vertex_size(), 12 + 12 + 4 + 8); // 36 bytes
        assert_eq!(format.get_uv_count(), 1);

        // Add another UV channel
        format.add_uv_channel(1);
        assert_eq!(format.get_vertex_size(), 12 + 12 + 4 + 16); // 44 bytes
        assert_eq!(format.get_uv_count(), 2);

        // Adding the same channel again should not change anything
        format.add_uv_channel(1);
        assert_eq!(format.get_vertex_size(), 44);
        assert_eq!(format.get_uv_count(), 2);
    }

    #[test]
    fn test_legacy_shader_sorting() {
        let mut def1 = LegacyW3DShaderDef::new();
        def1.set_pass_count(1);
        def1.set_texture_name(0, 0, "a.tga".to_string());

        let mut def2 = LegacyW3DShaderDef::new();
        def2.set_pass_count(1);
        def2.set_texture_name(0, 0, "b.tga".to_string());

        let shader1 = LegacyW3DShader::new(&def1).unwrap();
        let shader2 = LegacyW3DShader::new(&def2).unwrap();

        // Sorting should work based on texture names
        let result = shader1.compare_for_sorting(&shader2, 0);
        assert_eq!(result, Ordering::Less); // "a.tga" < "b.tga"
    }

    #[test]
    fn test_legacy_shader_transparency_detection() {
        let mut def = LegacyW3DShaderDef::new();
        def.set_pass_count(1);

        // Opaque shader (dest blend = zero)
        let mut shader_desc = LegacyShaderDesc::default();
        shader_desc.dest_blend = LegacyBlendMode::Zero;
        def.set_shader(0, shader_desc);
        assert!(!def.requires_sorting());

        // Transparent shader (dest blend = one minus src alpha)
        shader_desc.dest_blend = LegacyBlendMode::OneMinusSrcAlpha;
        def.set_shader(0, shader_desc);
        assert!(def.requires_sorting());
    }

    #[test]
    fn test_legacy_shader_serialization() {
        let mut def = LegacyW3DShaderDef::new();
        def.set_pass_count(1);
        def.set_texture_name(0, 0, "serialize_test.tga".to_string());
        def.set_mapper_args(0, 0, "test_args".to_string());
        def.set_map_channel(0, 0, 1);

        let data = def.save().unwrap();

        let mut loaded_def = LegacyW3DShaderDef::default();
        loaded_def.load(&data).unwrap();

        assert_eq!(loaded_def.get_pass_count(), 1);
        assert_eq!(loaded_def.get_texture_name(0, 0), "serialize_test.tga");
        assert_eq!(loaded_def.get_mapper_args(0, 0), "test_args");
        assert_eq!(loaded_def.get_map_channel(0, 0), 1);
    }

    #[test]
    fn test_legacy_enums() {
        assert_eq!(LegacyBlendMode::Zero as u32, 0);
        assert_eq!(LegacyBlendMode::One as u32, 1);
        assert_eq!(LegacyDepthCompare::LessEqual as u32, 3);
        assert_eq!(LegacyColorMask::All as u32, 15);
        assert_eq!(LegacyShaderPreset::AlphaBlend as u32, 2);
    }
}
