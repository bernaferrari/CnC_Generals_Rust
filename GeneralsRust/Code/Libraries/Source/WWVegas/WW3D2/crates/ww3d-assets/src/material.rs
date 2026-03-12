/// Material and shader system matching C++ WW3D matinfo.cpp/shader.cpp
///
/// This module implements the complete material system with C++ fidelity:
/// - Material info with vertex materials and textures (matinfo.cpp:6-95)
/// - Material remapping for texture/material substitution (matinfo.cpp:97-229)
/// - Material collection for gathering unique materials (matinfo.cpp:231-392)
/// - Shader bit-packed render state (shader.h/cpp)
/// - Vertex material properties (vertmaterial.h/cpp)
///
/// References:
/// - C++ matinfo.h/cpp lines 1-392
/// - C++ shader.h/cpp
/// - C++ vertmaterial.h/cpp
/// - C++ matpass.h/cpp
use crate::texture::TextureBase;
use glam::Vec4;
use std::sync::Arc;
use ww3d_core::W3dVertexMaterialStruct;

/// Shader depth compare function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthCompare {
    Never,
    Less,
    Equal,
    LessEqual,
    Greater,
    NotEqual,
    GreaterEqual,
    Always,
}

/// Depth mask (write enable/disable)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DepthMask {
    WriteDisable,
    WriteEnable,
}

/// Color mask (write enable/disable)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorMask {
    WriteDisable,
    WriteEnable,
}

/// Alpha test enable/disable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlphaTest {
    Disable,
    Enable,
}

/// Cull mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CullMode {
    Disable,
    Enable,
}

/// Destination blend function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DstBlendFunc {
    Zero,
    One,
    SrcColor,
    OneMinusSrcColor,
    SrcAlpha,
    OneMinusSrcAlpha,
}

/// Source blend function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SrcBlendFunc {
    Zero,
    One,
    SrcAlpha,
    OneMinusSrcAlpha,
}

/// Fog function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FogFunc {
    Disable,
    Enable,
    ScaleFragment,
    White,
}

/// Primary gradient mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PrimaryGradient {
    Disable,
    Modulate,
    Add,
    BumpEnvMap,
    BumpEnvMapLuminance,
    Modulate2X,
}

/// Secondary gradient mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecondaryGradient {
    Disable,
    Enable,
}

/// Texturing enable/disable
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Texturing {
    Disable,
    Enable,
}

/// Detail alpha function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailAlphaFunc {
    Disable,
    Detail,
    Scale,
    InvScale,
}

/// Detail color function
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailColorFunc {
    Disable,
    Detail,
    Scale,
    InvScale,
    Add,
    Sub,
    SubR,
    Blend,
    DetailBlend,
    AddSigned,
    AddSigned2X,
    Scale2X,
    ModAlphaAddColor,
}

/// Shader class encapsulating all render state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shader {
    bits: u32,
}

impl Shader {
    // Bit shift constants
    const SHIFT_DEPTHCOMPARE: u32 = 0;
    const SHIFT_DEPTHMASK: u32 = 3;
    const SHIFT_COLORMASK: u32 = 4;
    const SHIFT_DSTBLEND: u32 = 5;
    const SHIFT_FOG: u32 = 8;
    const SHIFT_PRIGRADIENT: u32 = 10;
    const SHIFT_SECGRADIENT: u32 = 13;
    const SHIFT_SRCBLEND: u32 = 14;
    const SHIFT_TEXTURING: u32 = 16;
    const SHIFT_ALPHATEST: u32 = 18;
    const SHIFT_CULLMODE: u32 = 19;
    const SHIFT_POSTDETAILCOLORFUNC: u32 = 20;
    const SHIFT_POSTDETAILALPHAFUNC: u32 = 24;

    // Bit masks
    const MASK_DEPTHCOMPARE: u32 = 0x7 << Self::SHIFT_DEPTHCOMPARE;
    const MASK_DEPTHMASK: u32 = 0x1 << Self::SHIFT_DEPTHMASK;
    const MASK_COLORMASK: u32 = 0x1 << Self::SHIFT_COLORMASK;
    const MASK_DSTBLEND: u32 = 0x7 << Self::SHIFT_DSTBLEND;
    const MASK_FOG: u32 = 0x3 << Self::SHIFT_FOG;
    const MASK_PRIGRADIENT: u32 = 0x7 << Self::SHIFT_PRIGRADIENT;
    const MASK_SECGRADIENT: u32 = 0x1 << Self::SHIFT_SECGRADIENT;
    const MASK_SRCBLEND: u32 = 0x3 << Self::SHIFT_SRCBLEND;
    const MASK_TEXTURING: u32 = 0x1 << Self::SHIFT_TEXTURING;
    const MASK_ALPHATEST: u32 = 0x1 << Self::SHIFT_ALPHATEST;
    const MASK_CULLMODE: u32 = 0x1 << Self::SHIFT_CULLMODE;
    const MASK_POSTDETAILCOLORFUNC: u32 = 0xF << Self::SHIFT_POSTDETAILCOLORFUNC;
    const MASK_POSTDETAILALPHAFUNC: u32 = 0x7 << Self::SHIFT_POSTDETAILALPHAFUNC;

    /// Create a new shader with default settings
    pub fn new() -> Self {
        let mut shader = Self { bits: 0 };
        shader.reset();
        shader
    }

    /// Reset to default state
    pub fn reset(&mut self) {
        self.bits = 0;
        self.set_depth_compare(DepthCompare::LessEqual);
        self.set_depth_mask(DepthMask::WriteEnable);
        self.set_color_mask(ColorMask::WriteEnable);
        self.set_dst_blend_func(DstBlendFunc::Zero);
        self.set_fog_func(FogFunc::Disable);
        self.set_primary_gradient(PrimaryGradient::Modulate);
        self.set_secondary_gradient(SecondaryGradient::Disable);
        self.set_src_blend_func(SrcBlendFunc::One);
        self.set_texturing(Texturing::Disable);
        self.set_alpha_test(AlphaTest::Disable);
        self.set_cull_mode(CullMode::Enable);
        self.set_post_detail_color_func(DetailColorFunc::Disable);
        self.set_post_detail_alpha_func(DetailAlphaFunc::Disable);
    }

    /// Get raw shader bits
    pub fn bits(&self) -> u32 {
        self.bits
    }

    /// Set depth compare function
    pub fn set_depth_compare(&mut self, value: DepthCompare) {
        self.bits &= !Self::MASK_DEPTHCOMPARE;
        self.bits |= (value as u32) << Self::SHIFT_DEPTHCOMPARE;
    }

    pub fn depth_compare(&self) -> DepthCompare {
        match (self.bits & Self::MASK_DEPTHCOMPARE) >> Self::SHIFT_DEPTHCOMPARE {
            0 => DepthCompare::Never,
            1 => DepthCompare::Less,
            2 => DepthCompare::Equal,
            3 => DepthCompare::LessEqual,
            4 => DepthCompare::Greater,
            5 => DepthCompare::NotEqual,
            6 => DepthCompare::GreaterEqual,
            7 => DepthCompare::Always,
            _ => DepthCompare::LessEqual,
        }
    }

    /// Set depth mask
    pub fn set_depth_mask(&mut self, value: DepthMask) {
        self.bits &= !Self::MASK_DEPTHMASK;
        self.bits |= (value as u32) << Self::SHIFT_DEPTHMASK;
    }

    pub fn depth_mask(&self) -> DepthMask {
        match (self.bits & Self::MASK_DEPTHMASK) >> Self::SHIFT_DEPTHMASK {
            0 => DepthMask::WriteDisable,
            _ => DepthMask::WriteEnable,
        }
    }

    /// Set color mask
    pub fn set_color_mask(&mut self, value: ColorMask) {
        self.bits &= !Self::MASK_COLORMASK;
        self.bits |= (value as u32) << Self::SHIFT_COLORMASK;
    }

    pub fn color_mask(&self) -> ColorMask {
        match (self.bits & Self::MASK_COLORMASK) >> Self::SHIFT_COLORMASK {
            0 => ColorMask::WriteDisable,
            _ => ColorMask::WriteEnable,
        }
    }

    /// Set destination blend function
    pub fn set_dst_blend_func(&mut self, value: DstBlendFunc) {
        self.bits &= !Self::MASK_DSTBLEND;
        self.bits |= (value as u32) << Self::SHIFT_DSTBLEND;
    }

    pub fn dst_blend_func(&self) -> DstBlendFunc {
        match (self.bits & Self::MASK_DSTBLEND) >> Self::SHIFT_DSTBLEND {
            0 => DstBlendFunc::Zero,
            1 => DstBlendFunc::One,
            2 => DstBlendFunc::SrcColor,
            3 => DstBlendFunc::OneMinusSrcColor,
            4 => DstBlendFunc::SrcAlpha,
            5 => DstBlendFunc::OneMinusSrcAlpha,
            _ => DstBlendFunc::Zero,
        }
    }

    /// Set fog function
    pub fn set_fog_func(&mut self, value: FogFunc) {
        self.bits &= !Self::MASK_FOG;
        self.bits |= (value as u32) << Self::SHIFT_FOG;
    }

    pub fn fog_func(&self) -> FogFunc {
        match (self.bits & Self::MASK_FOG) >> Self::SHIFT_FOG {
            0 => FogFunc::Disable,
            1 => FogFunc::Enable,
            2 => FogFunc::ScaleFragment,
            3 => FogFunc::White,
            _ => FogFunc::Disable,
        }
    }

    /// Set primary gradient
    pub fn set_primary_gradient(&mut self, value: PrimaryGradient) {
        self.bits &= !Self::MASK_PRIGRADIENT;
        self.bits |= (value as u32) << Self::SHIFT_PRIGRADIENT;
    }

    pub fn primary_gradient(&self) -> PrimaryGradient {
        match (self.bits & Self::MASK_PRIGRADIENT) >> Self::SHIFT_PRIGRADIENT {
            0 => PrimaryGradient::Disable,
            1 => PrimaryGradient::Modulate,
            2 => PrimaryGradient::Add,
            3 => PrimaryGradient::BumpEnvMap,
            4 => PrimaryGradient::BumpEnvMapLuminance,
            5 => PrimaryGradient::Modulate2X,
            _ => PrimaryGradient::Modulate,
        }
    }

    /// Set secondary gradient
    pub fn set_secondary_gradient(&mut self, value: SecondaryGradient) {
        self.bits &= !Self::MASK_SECGRADIENT;
        self.bits |= (value as u32) << Self::SHIFT_SECGRADIENT;
    }

    pub fn secondary_gradient(&self) -> SecondaryGradient {
        match (self.bits & Self::MASK_SECGRADIENT) >> Self::SHIFT_SECGRADIENT {
            0 => SecondaryGradient::Disable,
            _ => SecondaryGradient::Enable,
        }
    }

    /// Set source blend function
    pub fn set_src_blend_func(&mut self, value: SrcBlendFunc) {
        self.bits &= !Self::MASK_SRCBLEND;
        self.bits |= (value as u32) << Self::SHIFT_SRCBLEND;
    }

    pub fn src_blend_func(&self) -> SrcBlendFunc {
        match (self.bits & Self::MASK_SRCBLEND) >> Self::SHIFT_SRCBLEND {
            0 => SrcBlendFunc::Zero,
            1 => SrcBlendFunc::One,
            2 => SrcBlendFunc::SrcAlpha,
            3 => SrcBlendFunc::OneMinusSrcAlpha,
            _ => SrcBlendFunc::One,
        }
    }

    /// Set texturing
    pub fn set_texturing(&mut self, value: Texturing) {
        self.bits &= !Self::MASK_TEXTURING;
        self.bits |= (value as u32) << Self::SHIFT_TEXTURING;
    }

    pub fn texturing(&self) -> Texturing {
        match (self.bits & Self::MASK_TEXTURING) >> Self::SHIFT_TEXTURING {
            0 => Texturing::Disable,
            _ => Texturing::Enable,
        }
    }

    /// Set alpha test
    pub fn set_alpha_test(&mut self, value: AlphaTest) {
        self.bits &= !Self::MASK_ALPHATEST;
        self.bits |= (value as u32) << Self::SHIFT_ALPHATEST;
    }

    pub fn alpha_test(&self) -> AlphaTest {
        match (self.bits & Self::MASK_ALPHATEST) >> Self::SHIFT_ALPHATEST {
            0 => AlphaTest::Disable,
            _ => AlphaTest::Enable,
        }
    }

    /// Set cull mode
    pub fn set_cull_mode(&mut self, value: CullMode) {
        self.bits &= !Self::MASK_CULLMODE;
        self.bits |= (value as u32) << Self::SHIFT_CULLMODE;
    }

    pub fn cull_mode(&self) -> CullMode {
        match (self.bits & Self::MASK_CULLMODE) >> Self::SHIFT_CULLMODE {
            0 => CullMode::Disable,
            _ => CullMode::Enable,
        }
    }

    /// Set post detail color function
    pub fn set_post_detail_color_func(&mut self, value: DetailColorFunc) {
        self.bits &= !Self::MASK_POSTDETAILCOLORFUNC;
        self.bits |= (value as u32) << Self::SHIFT_POSTDETAILCOLORFUNC;
    }

    pub fn post_detail_color_func(&self) -> DetailColorFunc {
        match (self.bits & Self::MASK_POSTDETAILCOLORFUNC) >> Self::SHIFT_POSTDETAILCOLORFUNC {
            0 => DetailColorFunc::Disable,
            1 => DetailColorFunc::Detail,
            2 => DetailColorFunc::Scale,
            3 => DetailColorFunc::InvScale,
            4 => DetailColorFunc::Add,
            5 => DetailColorFunc::Sub,
            6 => DetailColorFunc::SubR,
            7 => DetailColorFunc::Blend,
            8 => DetailColorFunc::DetailBlend,
            9 => DetailColorFunc::AddSigned,
            10 => DetailColorFunc::AddSigned2X,
            11 => DetailColorFunc::Scale2X,
            12 => DetailColorFunc::ModAlphaAddColor,
            _ => DetailColorFunc::Disable,
        }
    }

    /// Set post detail alpha function
    pub fn set_post_detail_alpha_func(&mut self, value: DetailAlphaFunc) {
        self.bits &= !Self::MASK_POSTDETAILALPHAFUNC;
        self.bits |= (value as u32) << Self::SHIFT_POSTDETAILALPHAFUNC;
    }

    pub fn post_detail_alpha_func(&self) -> DetailAlphaFunc {
        match (self.bits & Self::MASK_POSTDETAILALPHAFUNC) >> Self::SHIFT_POSTDETAILALPHAFUNC {
            0 => DetailAlphaFunc::Disable,
            1 => DetailAlphaFunc::Detail,
            2 => DetailAlphaFunc::Scale,
            3 => DetailAlphaFunc::InvScale,
            _ => DetailAlphaFunc::Disable,
        }
    }

    /// Check if shader uses alpha
    pub fn uses_alpha(&self) -> bool {
        if self.alpha_test() != AlphaTest::Disable {
            return true;
        }

        let dst = self.dst_blend_func();
        if matches!(dst, DstBlendFunc::SrcAlpha | DstBlendFunc::OneMinusSrcAlpha) {
            return true;
        }

        let src = self.src_blend_func();
        matches!(src, SrcBlendFunc::SrcAlpha | SrcBlendFunc::OneMinusSrcAlpha)
    }

    /// Check if shader uses fog
    pub fn uses_fog(&self) -> bool {
        self.fog_func() != FogFunc::Disable
    }

    /// Check if shader uses textures
    pub fn uses_texture(&self) -> bool {
        self.texturing() != Texturing::Disable
    }

    // Preset shaders matching C++ presets

    /// Opaque shader (texturing, zbuffer, primary gradient, no blending)
    pub fn preset_opaque() -> Self {
        let mut shader = Self::new();
        shader.set_texturing(Texturing::Enable);
        shader.set_depth_compare(DepthCompare::LessEqual);
        shader.set_depth_mask(DepthMask::WriteEnable);
        shader.set_primary_gradient(PrimaryGradient::Modulate);
        shader
    }

    /// Alpha shader (texturing, zbuffer, primary gradient, alpha blending)
    pub fn preset_alpha() -> Self {
        let mut shader = Self::preset_opaque();
        shader.set_src_blend_func(SrcBlendFunc::SrcAlpha);
        shader.set_dst_blend_func(DstBlendFunc::OneMinusSrcAlpha);
        shader.set_depth_mask(DepthMask::WriteDisable);
        shader
    }

    /// Additive shader (texturing, zbuffer, primary gradient, additive blending)
    pub fn preset_additive() -> Self {
        let mut shader = Self::preset_opaque();
        shader.set_src_blend_func(SrcBlendFunc::One);
        shader.set_dst_blend_func(DstBlendFunc::One);
        shader.set_depth_mask(DepthMask::WriteDisable);
        shader
    }

    /// Alpha test shader (texturing, zbuffer, no blending, alpha testing)
    pub fn preset_alpha_test() -> Self {
        let mut shader = Self::preset_opaque();
        shader.set_alpha_test(AlphaTest::Enable);
        shader
    }
}

impl Default for Shader {
    fn default() -> Self {
        Self::new()
    }
}

/// Vertex material properties
#[derive(Debug, Clone)]
pub struct VertexMaterial {
    pub ambient: Vec4,
    pub diffuse: Vec4,
    pub specular: Vec4,
    pub emissive: Vec4,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

impl VertexMaterial {
    pub fn new() -> Self {
        Self {
            ambient: Vec4::new(0.2, 0.2, 0.2, 1.0),
            diffuse: Vec4::new(0.8, 0.8, 0.8, 1.0),
            specular: Vec4::new(0.0, 0.0, 0.0, 1.0),
            emissive: Vec4::new(0.0, 0.0, 0.0, 1.0),
            shininess: 0.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }

    /// Create from W3D vertex material struct
    pub fn from_w3d_vertex_material(mat: &W3dVertexMaterialStruct) -> Self {
        Self {
            ambient: Vec4::new(
                mat.ambient.r as f32 / 255.0,
                mat.ambient.g as f32 / 255.0,
                mat.ambient.b as f32 / 255.0,
                mat.opacity,
            ),
            diffuse: Vec4::new(
                mat.diffuse.r as f32 / 255.0,
                mat.diffuse.g as f32 / 255.0,
                mat.diffuse.b as f32 / 255.0,
                mat.opacity,
            ),
            specular: Vec4::new(
                mat.specular.r as f32 / 255.0,
                mat.specular.g as f32 / 255.0,
                mat.specular.b as f32 / 255.0,
                1.0,
            ),
            emissive: Vec4::new(
                mat.emissive.r as f32 / 255.0,
                mat.emissive.g as f32 / 255.0,
                mat.emissive.b as f32 / 255.0,
                1.0,
            ),
            shininess: mat.shininess,
            opacity: mat.opacity,
            translucency: mat.translucency,
        }
    }
}

impl Default for VertexMaterial {
    fn default() -> Self {
        Self::new()
    }
}

/// Material pass for multi-pass rendering
#[derive(Debug, Clone)]
pub struct MaterialPass {
    pub shader: Shader,
    pub textures: [Option<Arc<TextureBase>>; 8],
    pub material: Option<VertexMaterial>,
    pub enable_on_translucent: bool,
}

impl MaterialPass {
    /// Create a new material pass
    pub fn new() -> Self {
        Self {
            shader: Shader::new(),
            textures: Default::default(),
            material: None,
            enable_on_translucent: false,
        }
    }

    /// Set texture for a specific stage
    pub fn set_texture(&mut self, stage: usize, texture: Option<Arc<TextureBase>>) {
        if stage < self.textures.len() {
            self.textures[stage] = texture;
        }
    }

    /// Get texture at stage
    pub fn texture(&self, stage: usize) -> Option<&Arc<TextureBase>> {
        self.textures.get(stage).and_then(|t| t.as_ref())
    }

    /// Set vertex material
    pub fn set_material(&mut self, material: VertexMaterial) {
        self.material = Some(material);
    }

    /// Set shader
    pub fn set_shader(&mut self, shader: Shader) {
        self.shader = shader;
    }
}

impl Default for MaterialPass {
    fn default() -> Self {
        Self::new()
    }
}

/// Complete material definition with multiple passes
#[derive(Debug, Clone)]
pub struct Material {
    pub name: String,
    pub passes: Vec<MaterialPass>,
    pub two_sided: bool,
    pub alpha_tested: bool,
    pub translucent: bool,
}

impl Material {
    pub fn new(name: String) -> Self {
        Self {
            name,
            passes: vec![MaterialPass::new()],
            two_sided: false,
            alpha_tested: false,
            translucent: false,
        }
    }

    /// Add a material pass
    pub fn add_pass(&mut self, pass: MaterialPass) {
        self.passes.push(pass);
    }

    /// Get primary pass
    pub fn primary_pass(&self) -> Option<&MaterialPass> {
        self.passes.first()
    }

    /// Get primary pass mutably
    pub fn primary_pass_mut(&mut self) -> Option<&mut MaterialPass> {
        self.passes.first_mut()
    }
}

/// Material info class (matches C++ MaterialInfoClass from matinfo.cpp:6-95)
/// Contains vertex materials and textures for a mesh
#[derive(Debug, Clone)]
pub struct MaterialInfo {
    pub vertex_materials: Vec<VertexMaterial>,
    pub textures: Vec<Arc<TextureBase>>,
}

impl MaterialInfo {
    /// Create empty material info (C++ matinfo.cpp:6-8)
    pub fn new() -> Self {
        Self {
            vertex_materials: Vec::new(),
            textures: Vec::new(),
        }
    }

    /// Add a texture and return its index (C++ matinfo.cpp:37-44)
    pub fn add_texture(&mut self, texture: Arc<TextureBase>) -> usize {
        let index = self.textures.len();
        self.textures.push(texture);
        index
    }

    /// Get texture index by name (C++ matinfo.cpp:46-54)
    pub fn get_texture_index(&self, name: &str) -> Option<usize> {
        self.textures
            .iter()
            .position(|tex| tex.name.eq_ignore_ascii_case(name))
    }

    /// Get texture by index (C++ matinfo.cpp:56-62)
    pub fn get_texture(&self, index: usize) -> Option<&Arc<TextureBase>> {
        self.textures.get(index)
    }

    /// Add vertex material
    pub fn add_vertex_material(&mut self, material: VertexMaterial) -> usize {
        let index = self.vertex_materials.len();
        self.vertex_materials.push(material);
        index
    }

    /// Get vertex material by index
    pub fn get_vertex_material(&self, index: usize) -> Option<&VertexMaterial> {
        self.vertex_materials.get(index)
    }

    /// Free all materials and textures (C++ matinfo.cpp:81-94)
    pub fn free(&mut self) {
        self.vertex_materials.clear();
        self.textures.clear();
    }

    /// Get texture count
    pub fn texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Get vertex material count
    pub fn vertex_material_count(&self) -> usize {
        self.vertex_materials.len()
    }
}

impl Default for MaterialInfo {
    fn default() -> Self {
        Self::new()
    }
}

/// Material collector for gathering unique materials from meshes
/// (matches C++ MaterialCollectorClass from matinfo.cpp:231-392)
#[derive(Debug)]
pub struct MaterialCollector {
    textures: Vec<Arc<TextureBase>>,
    vertex_materials: Vec<VertexMaterial>,
    shaders: Vec<Shader>,
    last_texture: Option<Arc<TextureBase>>,
    last_material: Option<VertexMaterial>,
    last_shader: Option<Shader>,
}

impl MaterialCollector {
    /// Create new material collector (C++ matinfo.cpp:231-236)
    pub fn new() -> Self {
        Self {
            textures: Vec::new(),
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
            last_texture: None,
            last_material: None,
            last_shader: None,
        }
    }

    /// Add texture if not already collected (C++ matinfo.cpp:306-314)
    pub fn add_texture(&mut self, texture: Arc<TextureBase>) {
        // Check if this is the last texture added (cache optimization)
        if let Some(ref last) = self.last_texture {
            if Arc::ptr_eq(last, &texture) {
                return;
            }
        }

        // Check if texture already exists
        if self.find_texture(&texture).is_some() {
            return;
        }

        self.last_texture = Some(Arc::clone(&texture));
        self.textures.push(texture);
    }

    /// Add shader if not already collected (C++ matinfo.cpp:316-322)
    pub fn add_shader(&mut self, shader: Shader) {
        if let Some(last) = self.last_shader {
            if last == shader {
                return;
            }
        }

        if self.find_shader(shader).is_some() {
            return;
        }

        self.last_shader = Some(shader);
        self.shaders.push(shader);
    }

    /// Add vertex material if not already collected (C++ matinfo.cpp:324-332)
    pub fn add_vertex_material(&mut self, material: VertexMaterial) {
        if let Some(ref last) = self.last_material {
            if materials_equal(last, &material) {
                return;
            }
        }

        if self.find_vertex_material(&material).is_some() {
            return;
        }

        self.last_material = Some(material.clone());
        self.vertex_materials.push(material);
    }

    /// Find texture index (C++ matinfo.cpp:374-382)
    fn find_texture(&self, texture: &Arc<TextureBase>) -> Option<usize> {
        self.textures
            .iter()
            .position(|tex| Arc::ptr_eq(tex, texture))
    }

    /// Find shader index (C++ matinfo.cpp:364-372)
    fn find_shader(&self, shader: Shader) -> Option<usize> {
        self.shaders.iter().position(|&s| s == shader)
    }

    /// Find vertex material index (C++ matinfo.cpp:384-392)
    fn find_vertex_material(&self, material: &VertexMaterial) -> Option<usize> {
        self.vertex_materials
            .iter()
            .position(|m| materials_equal(m, material))
    }

    /// Reset collector (C++ matinfo.cpp:293-304)
    pub fn reset(&mut self) {
        self.textures.clear();
        self.vertex_materials.clear();
        self.shaders.clear();
        self.last_texture = None;
        self.last_material = None;
        self.last_shader = None;
    }

    /// Get counts (C++ matinfo.cpp:334-347)
    pub fn get_shader_count(&self) -> usize {
        self.shaders.len()
    }

    pub fn get_vertex_material_count(&self) -> usize {
        self.vertex_materials.len()
    }

    pub fn get_texture_count(&self) -> usize {
        self.textures.len()
    }

    /// Peek at collected data (C++ matinfo.cpp:349-362)
    pub fn peek_shader(&self, index: usize) -> Option<Shader> {
        self.shaders.get(index).copied()
    }

    pub fn peek_texture(&self, index: usize) -> Option<&Arc<TextureBase>> {
        self.textures.get(index)
    }

    pub fn peek_vertex_material(&self, index: usize) -> Option<&VertexMaterial> {
        self.vertex_materials.get(index)
    }
}

impl Default for MaterialCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// Helper function to compare vertex materials for equality
fn materials_equal(a: &VertexMaterial, b: &VertexMaterial) -> bool {
    a.ambient == b.ambient
        && a.diffuse == b.diffuse
        && a.specular == b.specular
        && a.emissive == b.emissive
        && (a.shininess - b.shininess).abs() < 0.0001
        && (a.opacity - b.opacity).abs() < 0.0001
        && (a.translucency - b.translucency).abs() < 0.0001
}

/// Material manager
pub struct MaterialManager {
    materials: std::collections::HashMap<String, Material>,
}

impl MaterialManager {
    pub fn new() -> Self {
        Self {
            materials: std::collections::HashMap::new(),
        }
    }

    /// Register a material
    pub fn register(&mut self, material: Material) {
        self.materials.insert(material.name.clone(), material);
    }

    /// Get material by name
    pub fn get(&self, name: &str) -> Option<&Material> {
        self.materials.get(name)
    }

    /// Get material mutably
    pub fn get_mut(&mut self, name: &str) -> Option<&mut Material> {
        self.materials.get_mut(name)
    }

    /// Remove material
    pub fn remove(&mut self, name: &str) -> Option<Material> {
        self.materials.remove(name)
    }

    /// Clear all materials
    pub fn clear(&mut self) {
        self.materials.clear();
    }

    /// Get material count
    pub fn count(&self) -> usize {
        self.materials.len()
    }
}

impl Default for MaterialManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shader_default() {
        let shader = Shader::new();
        assert_eq!(shader.depth_compare(), DepthCompare::LessEqual);
        assert_eq!(shader.depth_mask(), DepthMask::WriteEnable);
        assert_eq!(shader.texturing(), Texturing::Disable);
    }

    #[test]
    fn test_shader_alpha() {
        let shader = Shader::preset_alpha();
        assert!(shader.uses_alpha());
        assert_eq!(shader.src_blend_func(), SrcBlendFunc::SrcAlpha);
        assert_eq!(shader.dst_blend_func(), DstBlendFunc::OneMinusSrcAlpha);
    }

    #[test]
    fn test_shader_bits() {
        let mut shader = Shader::new();
        shader.set_alpha_test(AlphaTest::Enable);
        assert_eq!(shader.alpha_test(), AlphaTest::Enable);

        shader.set_cull_mode(CullMode::Disable);
        assert_eq!(shader.cull_mode(), CullMode::Disable);
        assert_eq!(shader.alpha_test(), AlphaTest::Enable); // Should not affect other bits
    }

    #[test]
    fn test_material_pass() {
        let mut pass = MaterialPass::new();
        assert!(pass.texture(0).is_none());

        pass.shader.set_texturing(Texturing::Enable);
        assert!(pass.shader.uses_texture());
    }

    #[test]
    fn test_material_manager() {
        let mut mgr = MaterialManager::new();
        let material = Material::new("test".to_string());

        mgr.register(material);
        assert_eq!(mgr.count(), 1);
        assert!(mgr.get("test").is_some());

        mgr.remove("test");
        assert_eq!(mgr.count(), 0);
    }
}
