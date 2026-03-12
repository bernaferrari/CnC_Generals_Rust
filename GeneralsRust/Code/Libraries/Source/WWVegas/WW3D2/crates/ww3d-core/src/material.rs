/// Material system for WW3D
///
/// This module implements the material system including shaders, vertex materials,
/// and material passes for multi-pass rendering.
use crate::w3d_format::*;
use glam::{Vec3, Vec4};
use std::fmt::Debug;

/// RGBA color representation
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const RED: Color = Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const GREEN: Color = Color {
        r: 0.0,
        g: 1.0,
        b: 0.0,
        a: 1.0,
    };
    pub const BLUE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 1.0,
        a: 1.0,
    };
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self { r, g, b, a: 1.0 }
    }

    pub fn from_rgba_u8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    pub fn to_rgba_u8(&self) -> [u8; 4] {
        [
            (self.r * 255.0) as u8,
            (self.g * 255.0) as u8,
            (self.b * 255.0) as u8,
            (self.a * 255.0) as u8,
        ]
    }

    pub fn to_vec3(&self) -> Vec3 {
        Vec3::new(self.r, self.g, self.b)
    }

    pub fn to_vec4(&self) -> Vec4 {
        Vec4::new(self.r, self.g, self.b, self.a)
    }

    pub fn lerp(&self, other: &Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }
}

impl From<W3dRGBAStruct> for Color {
    fn from(rgba: W3dRGBAStruct) -> Self {
        Color::from_rgba_u8(rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

impl From<Color> for W3dRGBAStruct {
    fn from(color: Color) -> Self {
        let rgba = color.to_rgba_u8();
        W3dRGBAStruct {
            r: rgba[0],
            g: rgba[1],
            b: rgba[2],
            a: rgba[3],
        }
    }
}

/// Blend mode for transparency and compositing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    Opaque,
    Additive,
    Multiply,
    Alpha,
    AlphaTest,
    Screen,
}

/// Depth comparison function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthCompare {
    Never,
    Less,
    Equal,
    LessOrEqual,
    Greater,
    NotEqual,
    GreaterOrEqual,
    Always,
}

/// Cull mode for face culling
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullMode {
    None,
    Front,
    Back,
}

/// Shader types in WW3D
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShaderType {
    /// Standard diffuse lighting
    Diffuse,
    /// Specular lighting
    Specular,
    /// Additive blending
    Additive,
    /// Multiplicative blending
    Multiply,
    /// Alpha blending
    Alpha,
    /// Alpha testing
    AlphaTest,
    /// Screen-space blending
    Screen,
    /// Bump mapping
    BumpMap,
    /// Environment mapping
    EnvironmentMap,
}

/// Shader material properties
#[derive(Debug, Clone)]
pub struct Shader {
    pub shader_type: ShaderType,
    pub depth_compare: DepthCompare,
    pub depth_write: bool,
    pub color_write: bool,
    pub alpha_test_threshold: f32,
    pub cull_mode: CullMode,
    pub blend_mode: BlendMode,
    pub detail_color: Color,
    pub detail_alpha: f32,
    pub secondary_texture_blend_mode: BlendMode,
    pub texture_stage_count: u32,
}

impl Shader {
    pub fn new() -> Self {
        Self {
            shader_type: ShaderType::Diffuse,
            depth_compare: DepthCompare::LessOrEqual,
            depth_write: true,
            color_write: true,
            alpha_test_threshold: 0.0,
            cull_mode: CullMode::Back,
            blend_mode: BlendMode::Opaque,
            detail_color: Color::WHITE,
            detail_alpha: 1.0,
            secondary_texture_blend_mode: BlendMode::Opaque,
            texture_stage_count: 1,
        }
    }

    pub fn from_w3d(w3d_shader: &W3dShaderStruct) -> Self {
        Self {
            shader_type: Self::parse_shader_type(w3d_shader),
            depth_compare: Self::parse_depth_compare(w3d_shader.depth_compare),
            depth_write: (w3d_shader.depth_mask & 0x01) != 0,
            color_write: (w3d_shader.depth_mask & 0x02) != 0,
            alpha_test_threshold: w3d_shader.alpha_test as f32 / 255.0,
            cull_mode: Self::parse_cull_mode(w3d_shader.depth_compare),
            blend_mode: Self::parse_blend_mode(w3d_shader.src_blend, w3d_shader.dest_blend),
            detail_color: Color::WHITE, // detail_color_func is an enum, use default
            detail_alpha: w3d_shader.detail_alpha_func as f32 / 255.0,
            secondary_texture_blend_mode: Self::parse_blend_mode(
                w3d_shader.src_blend,
                w3d_shader.dest_blend,
            ),
            texture_stage_count: 1,
        }
    }

    /// Parse shader type from W3D shader preset and flags
    /// C++ Reference: shader.cpp shader type parsing
    /// Presets 0-15 from W3D format specification
    fn parse_shader_type(w3d_shader: &W3dShaderStruct) -> ShaderType {
        // Shader preset indicates the base shader type (diffuse, specular, etc.)
        match w3d_shader.shader_preset {
            0 => ShaderType::Diffuse,   // W3D_SHADER_PRESET_DIFFUSE
            1 => ShaderType::Specular,  // W3D_SHADER_PRESET_SPECULAR
            2 => ShaderType::Diffuse,   // W3D_SHADER_PRESET_EMISSIVE (fallback to diffuse)
            3 => ShaderType::Alpha,     // W3D_SHADER_PRESET_GLASS
            4 => ShaderType::AlphaTest, // W3D_SHADER_PRESET_ALPHA_TEST
            5 => ShaderType::Additive,  // W3D_SHADER_PRESET_ADDITIVE
            6 => ShaderType::Multiply,  // W3D_SHADER_PRESET_MULTIPLY
            7 => ShaderType::Diffuse,   // W3D_SHADER_PRESET_ENVMAP (fallback to diffuse)
            8 => ShaderType::Diffuse,   // W3D_SHADER_PRESET_BUMPMAP
            9 => ShaderType::Diffuse,   // W3D_SHADER_PRESET_BUMPENVMAP
            10 => ShaderType::Specular, // W3D_SHADER_PRESET_SHINY_MASK
            _ => {
                // Fall back based on blend mode for unknown presets (11-15)
                // Use D3D8 blend constants to detect type
                const D3DBLEND_ONE: u8 = 2;
                const D3DBLEND_SRCALPHA: u8 = 5;
                const D3DBLEND_INVSRCALPHA: u8 = 6;

                if w3d_shader.src_blend == D3DBLEND_SRCALPHA
                    && w3d_shader.dest_blend == D3DBLEND_INVSRCALPHA
                {
                    ShaderType::Alpha
                } else if w3d_shader.src_blend == D3DBLEND_ONE
                    && w3d_shader.dest_blend == D3DBLEND_ONE
                {
                    ShaderType::Additive
                } else {
                    ShaderType::Diffuse
                }
            }
        }
    }

    fn parse_depth_compare(depth_compare: u8) -> DepthCompare {
        match depth_compare & 0x0F {
            1 => DepthCompare::Never,
            2 => DepthCompare::Less,
            3 => DepthCompare::Equal,
            4 => DepthCompare::LessOrEqual,
            5 => DepthCompare::Greater,
            6 => DepthCompare::NotEqual,
            7 => DepthCompare::GreaterOrEqual,
            8 => DepthCompare::Always,
            _ => DepthCompare::LessOrEqual,
        }
    }

    fn parse_cull_mode(depth_compare: u8) -> CullMode {
        match (depth_compare >> 4) & 0x03 {
            1 => CullMode::None,
            2 => CullMode::Front,
            3 => CullMode::Back,
            _ => CullMode::Back,
        }
    }

    /// Parse blend mode from D3D8 blend constants
    /// CRITICAL: These are D3DBLEND enum values from d3d8types.h, NOT 0-based!
    fn parse_blend_mode(src: u8, dest: u8) -> BlendMode {
        // D3DBLEND enumeration constants (1-10)
        const D3DBLEND_ZERO: u8 = 1;
        const D3DBLEND_ONE: u8 = 2;
        const D3DBLEND_SRCCOLOR: u8 = 3;
        const D3DBLEND_INVSRCCOLOR: u8 = 4;
        const D3DBLEND_SRCALPHA: u8 = 5;
        const D3DBLEND_INVSRCALPHA: u8 = 6;
        const D3DBLEND_DESTCOLOR: u8 = 9;

        match (src, dest) {
            (D3DBLEND_ONE, D3DBLEND_ZERO) => BlendMode::Opaque,
            (D3DBLEND_ONE, D3DBLEND_ONE) => BlendMode::Additive,
            (D3DBLEND_SRCALPHA, D3DBLEND_INVSRCALPHA) => BlendMode::Alpha,
            (D3DBLEND_DESTCOLOR, D3DBLEND_ZERO) => BlendMode::Multiply,
            (D3DBLEND_ZERO, D3DBLEND_SRCCOLOR) => BlendMode::Multiply,
            (D3DBLEND_ONE, D3DBLEND_INVSRCCOLOR) => BlendMode::Screen,
            _ => BlendMode::Opaque,
        }
    }
}

impl Default for Shader {
    fn default() -> Self {
        Self::new()
    }
}

/// Vertex material properties (lighting parameters)
#[derive(Debug, Clone)]
pub struct VertexMaterial {
    pub name: String,
    pub ambient: Color,
    pub diffuse: Color,
    pub specular: Color,
    pub emissive: Color,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

impl VertexMaterial {
    pub fn new(name: String) -> Self {
        Self {
            name,
            ambient: Color::WHITE,
            diffuse: Color::WHITE,
            specular: Color::BLACK,
            emissive: Color::BLACK,
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }

    pub fn from_w3d(w3d_vmat: &W3dVertexMaterialStruct) -> Self {
        Self {
            name: "VertexMaterial".to_string(), // W3dVertexMaterialStruct doesn't have a name field
            ambient: w3d_vmat.ambient.into(),
            diffuse: w3d_vmat.diffuse.into(),
            specular: w3d_vmat.specular.into(),
            emissive: w3d_vmat.emissive.into(),
            shininess: w3d_vmat.shininess,
            opacity: w3d_vmat.opacity,
            translucency: w3d_vmat.translucency,
        }
    }

    pub fn is_transparent(&self) -> bool {
        self.opacity < 1.0 || self.translucency > 0.0
    }
}

/// Texture mapping modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureAddressMode {
    Wrap,
    Clamp,
    Mirror,
    Border,
}

/// Texture filtering modes
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextureFilter {
    Point,
    Linear,
    Anisotropic,
}

/// Texture stage for multi-texturing
#[derive(Debug, Clone)]
pub struct TextureStage {
    pub texture_id: u32,
    pub texture_name: String,
    pub address_u: TextureAddressMode,
    pub address_v: TextureAddressMode,
    pub filter_min: TextureFilter,
    pub filter_mag: TextureFilter,
    pub filter_mip: TextureFilter,
    pub uv_channel: u32,
}

impl TextureStage {
    pub fn new(texture_id: u32) -> Self {
        Self {
            texture_id,
            texture_name: String::new(),
            address_u: TextureAddressMode::Wrap,
            address_v: TextureAddressMode::Wrap,
            filter_min: TextureFilter::Linear,
            filter_mag: TextureFilter::Linear,
            filter_mip: TextureFilter::Linear,
            uv_channel: 0,
        }
    }

    pub fn from_w3d(w3d_stage: &W3dTextureStageStruct) -> Self {
        Self {
            texture_id: w3d_stage.tx_id,
            texture_name: String::new(),
            address_u: TextureAddressMode::Wrap,
            address_v: TextureAddressMode::Wrap,
            filter_min: TextureFilter::Linear,
            filter_mag: TextureFilter::Linear,
            filter_mip: TextureFilter::Linear,
            uv_channel: 0,
        }
    }
}

/// Material pass for multi-pass rendering
#[derive(Debug, Clone)]
pub struct MaterialPass {
    pub vertex_material: Option<VertexMaterial>,
    pub shader: Shader,
    pub texture_stages: Vec<TextureStage>,
    pub diffuse_uv_channel: u32,
    pub normal_uv_channel: u32,
}

impl MaterialPass {
    pub fn new() -> Self {
        Self {
            vertex_material: None,
            shader: Shader::new(),
            texture_stages: Vec::new(),
            diffuse_uv_channel: 0,
            normal_uv_channel: 0,
        }
    }

    /// Parse material pass from W3D material pass structure
    /// C++ Reference: matpass.cpp MaterialPass loading
    pub fn from_w3d(w3d_pass: &W3dMaterialPassStruct) -> Self {
        Self {
            vertex_material: None, // Will be set separately using vm_id
            shader: Shader::new(), // Will be set separately using shader_id
            texture_stages: Vec::with_capacity(w3d_pass.texture_count as usize),
            diffuse_uv_channel: 0, // Default to first UV channel
            normal_uv_channel: 0,  // Default to first UV channel
        }
    }

    pub fn with_vertex_material(mut self, vmat: VertexMaterial) -> Self {
        self.vertex_material = Some(vmat);
        self
    }

    pub fn with_shader(mut self, shader: Shader) -> Self {
        self.shader = shader;
        self
    }

    pub fn add_texture_stage(&mut self, stage: TextureStage) {
        self.texture_stages.push(stage);
    }

    pub fn is_transparent(&self) -> bool {
        if let Some(ref vmat) = self.vertex_material {
            if vmat.is_transparent() {
                return true;
            }
        }
        matches!(
            self.shader.blend_mode,
            BlendMode::Alpha | BlendMode::AlphaTest
        )
    }
}

impl Default for MaterialPass {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete material information for a mesh
#[derive(Debug, Clone)]
pub struct MaterialInfo {
    pub name: String,
    pub passes: Vec<MaterialPass>,
    pub attributes: u32,
    pub sort_level: i32,
}

impl MaterialInfo {
    pub fn new(name: String) -> Self {
        Self {
            name,
            passes: Vec::new(),
            attributes: 0,
            sort_level: 0,
        }
    }

    /// Parse material info from W3D material info structure
    /// C++ Reference: matinfo.cpp MaterialInfo loading
    pub fn from_w3d(w3d_mat: &W3dMaterialInfoStruct) -> Self {
        Self {
            name: "Material".to_string(), // W3dMaterialInfoStruct doesn't have a name field, use default
            passes: Vec::with_capacity(w3d_mat.pass_count as usize), // Pre-allocate for pass_count passes
            attributes: 0, // W3dMaterialInfoStruct doesn't have attributes, use default
            sort_level: 0, // Default sort level
        }
    }

    pub fn add_pass(&mut self, pass: MaterialPass) {
        self.passes.push(pass);
    }

    pub fn get_pass(&self, index: usize) -> Option<&MaterialPass> {
        self.passes.get(index)
    }

    pub fn get_pass_mut(&mut self, index: usize) -> Option<&mut MaterialPass> {
        self.passes.get_mut(index)
    }

    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    pub fn is_transparent(&self) -> bool {
        self.passes.iter().any(|pass| pass.is_transparent())
    }

    pub fn is_two_sided(&self) -> bool {
        self.passes
            .iter()
            .any(|pass| pass.shader.cull_mode == CullMode::None)
    }
}

/// Material library for managing materials
#[derive(Debug)]
pub struct MaterialLibrary {
    materials: Vec<MaterialInfo>,
}

impl MaterialLibrary {
    pub fn new() -> Self {
        Self {
            materials: Vec::new(),
        }
    }

    pub fn add_material(&mut self, material: MaterialInfo) -> usize {
        let index = self.materials.len();
        self.materials.push(material);
        index
    }

    pub fn get_material(&self, index: usize) -> Option<&MaterialInfo> {
        self.materials.get(index)
    }

    pub fn get_material_mut(&mut self, index: usize) -> Option<&mut MaterialInfo> {
        self.materials.get_mut(index)
    }

    pub fn find_material_by_name(&self, name: &str) -> Option<usize> {
        self.materials.iter().position(|mat| mat.name == name)
    }

    pub fn material_count(&self) -> usize {
        self.materials.len()
    }

    pub fn clear(&mut self) {
        self.materials.clear();
    }
}

impl Default for MaterialLibrary {
    fn default() -> Self {
        Self::new()
    }
}

/// Create a default material for testing
pub fn create_default_material(name: String) -> MaterialInfo {
    let mut material = MaterialInfo::new(name);

    let vmat = VertexMaterial {
        name: "Default".to_string(),
        ambient: Color::WHITE,
        diffuse: Color::WHITE,
        specular: Color::new(0.5, 0.5, 0.5, 1.0),
        emissive: Color::BLACK,
        shininess: 32.0,
        opacity: 1.0,
        translucency: 0.0,
    };

    let pass = MaterialPass::new()
        .with_vertex_material(vmat)
        .with_shader(Shader::new());

    material.add_pass(pass);
    material
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_color_creation() {
        let color = Color::new(0.5, 0.6, 0.7, 0.8);
        assert_eq!(color.r, 0.5);
        assert_eq!(color.g, 0.6);
        assert_eq!(color.b, 0.7);
        assert_eq!(color.a, 0.8);
    }

    #[test]
    fn test_color_conversion() {
        let color = Color::from_rgba_u8(127, 127, 127, 255);
        let rgba = color.to_rgba_u8();

        assert_eq!(rgba[0], 127);
        assert_eq!(rgba[1], 127);
        assert_eq!(rgba[2], 127);
        assert_eq!(rgba[3], 255);
    }

    #[test]
    fn test_color_lerp() {
        let c1 = Color::BLACK;
        let c2 = Color::WHITE;
        let mid = c1.lerp(&c2, 0.5);

        assert!((mid.r - 0.5).abs() < 0.001);
        assert!((mid.g - 0.5).abs() < 0.001);
        assert!((mid.b - 0.5).abs() < 0.001);
    }

    #[test]
    fn test_shader_creation() {
        let shader = Shader::new();
        assert_eq!(shader.shader_type, ShaderType::Diffuse);
        assert_eq!(shader.depth_compare, DepthCompare::LessOrEqual);
        assert!(shader.depth_write);
    }

    #[test]
    fn test_vertex_material() {
        let vmat = VertexMaterial::new("test".to_string());
        assert_eq!(vmat.name, "test");
        assert!(!vmat.is_transparent());

        let mut vmat_transparent = vmat.clone();
        vmat_transparent.opacity = 0.5;
        assert!(vmat_transparent.is_transparent());
    }

    #[test]
    fn test_material_pass() {
        let mut pass = MaterialPass::new();
        pass.add_texture_stage(TextureStage::new(1));

        assert_eq!(pass.texture_stages.len(), 1);
        assert!(!pass.is_transparent());
    }

    #[test]
    fn test_material_info() {
        let mut material = MaterialInfo::new("test_mat".to_string());
        material.add_pass(MaterialPass::new());

        assert_eq!(material.pass_count(), 1);
        assert!(material.get_pass(0).is_some());
        assert!(material.get_pass(1).is_none());
    }

    #[test]
    fn test_material_library() {
        let mut lib = MaterialLibrary::new();

        let mat1 = create_default_material("mat1".to_string());
        let mat2 = create_default_material("mat2".to_string());

        lib.add_material(mat1);
        lib.add_material(mat2);

        assert_eq!(lib.material_count(), 2);
        assert_eq!(lib.find_material_by_name("mat1"), Some(0));
        assert_eq!(lib.find_material_by_name("mat2"), Some(1));
        assert_eq!(lib.find_material_by_name("mat3"), None);
    }

    #[test]
    fn test_default_material() {
        let material = create_default_material("default".to_string());

        assert_eq!(material.name, "default");
        assert_eq!(material.pass_count(), 1);

        let pass = material.get_pass(0).unwrap();
        assert!(pass.vertex_material.is_some());
    }
}
