//! Material system - vertex and pixel materials with WGSL shader support

pub mod material_pass;
pub mod texture_mapper;
pub mod vertex_material;

use crate::config;
use crate::rendering::shader_system::ShaderClass;
use crate::rendering::texture_system::texture_base::{TextureAddressMode, TextureFilterMode};
use crate::texture_system::{SurfaceClass, TextureClass};
use glam::{Vec3, Vec4};
use std::array;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use ww3d_collision::bounding_volumes::OBBoxClass;

/// Vertex material class - defines surface properties
#[derive(Debug, Clone)]
pub struct VertexMaterialClass {
    pub name: String,
    pub ambient: Vec3,
    pub diffuse: Vec3,
    pub specular: Vec3,
    pub emissive: Vec3,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

impl VertexMaterialClass {
    /// Create a new vertex material
    pub fn new(name: &str) -> Self {
        Self {
            name: name.to_string(),
            ambient: Vec3::new(0.2, 0.2, 0.2),
            diffuse: Vec3::new(0.8, 0.8, 0.8),
            specular: Vec3::new(1.0, 1.0, 1.0),
            emissive: Vec3::ZERO,
            shininess: 32.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }

    /// Get sort level for material batching
    pub fn get_sort_level(&self) -> u32 {
        // Simple sort level based on opacity
        if self.opacity < 1.0 {
            1 // Transparent materials sort later
        } else {
            0 // Opaque materials sort first
        }
    }

    pub fn from_w3d_material(name: &str, material: &ww3d_core::W3dVertexMaterialStruct) -> Self {
        let to_vec3 = |color: &ww3d_core::W3dRGBAStruct| -> Vec3 {
            Vec3::new(
                color.r as f32 / 255.0,
                color.g as f32 / 255.0,
                color.b as f32 / 255.0,
            )
        };

        Self {
            name: name.to_string(),
            ambient: to_vec3(&material.ambient),
            diffuse: to_vec3(&material.diffuse),
            specular: to_vec3(&material.specular),
            emissive: to_vec3(&material.emissive),
            shininess: material.shininess,
            opacity: material.opacity,
            translucency: material.translucency,
        }
    }
}

/// Texture stage binding extracted from W3D material data
#[derive(Debug, Clone)]
pub struct MaterialTextureBinding {
    pub stage: usize,
    pub texture: Arc<TextureClass>,
    pub per_polygon_texture_ids: Option<Vec<u32>>,
    pub per_face_texcoord_ids: Option<Vec<[u32; 3]>>,
    pub settings: TextureStageSettings,
}

impl MaterialTextureBinding {
    pub fn new(
        stage: usize,
        descriptor: &ww3d_core::W3dTextureStruct,
        per_polygon_texture_ids: Option<Vec<u32>>,
        per_face_texcoord_ids: Option<Vec<[u32; 3]>>,
    ) -> Self {
        let settings = TextureStageSettings::from_descriptor(descriptor);
        Self {
            stage,
            texture: Arc::new(TextureClass::from_w3d_descriptor(descriptor)),
            per_polygon_texture_ids,
            per_face_texcoord_ids,
            settings,
        }
    }

    pub fn surface(&self) -> crate::core::Result<SurfaceClass> {
        self.texture.to_surface()
    }
}

/// Material pass class - defines a rendering pass
const MAX_TEXTURE_STAGES: usize = 8;

static ENABLE_PER_POLYGON_CULLING: AtomicBool = AtomicBool::new(true);

#[derive(Debug, Clone)]
pub struct MaterialPassClass {
    pub vertex_material: Option<Arc<VertexMaterialClass>>,
    pub shader: ShaderClass,
    pub textures: [Option<Arc<TextureClass>>; MAX_TEXTURE_STAGES],
    pub texture_bindings: Vec<MaterialTextureBinding>,
    pub stage_uv_channels: [u8; MAX_TEXTURE_STAGES],
    pub diffuse_vertex_colors: Option<Vec<Vec4>>,
    pub illumination_vertex_colors: Option<Vec<Vec4>>,
    pub cull_volume: Option<OBBoxClass>,
    pub enable_on_translucent_meshes: bool,
    /// Pass index for multi-pass rendering
    pub pass_index: usize,
    /// Mapper ID for animated texture transformations (0=UV, 4=LinearOffset, 7=Grid, 8=Rotate, 9=SineLinearOffset)
    pub mapper_id: u32,
    /// Mapper arguments for texture coordinate transformations
    pub mapper_args: [i32; 4],
    /// Floating-point mapper arguments
    pub mapper_float_args: [f32; 4],
}

impl MaterialPassClass {
    /// Create a new material pass
    pub fn new() -> Self {
        let mut shader = ShaderClass::new();
        shader.enable_fog("MaterialPassClass::new");
        Self {
            vertex_material: None,
            shader,
            textures: array::from_fn(|_| None),
            texture_bindings: Vec::new(),
            stage_uv_channels: array::from_fn(|idx| idx as u8),
            diffuse_vertex_colors: None,
            illumination_vertex_colors: None,
            cull_volume: None,
            enable_on_translucent_meshes: true,
            pass_index: 0,
            mapper_id: 0,
            mapper_args: [0, 0, 0, 0],
            mapper_float_args: [0.0; 4],
        }
    }

    pub fn add_texture_binding(&mut self, binding: MaterialTextureBinding) {
        self.texture_bindings
            .retain(|existing| existing.stage != binding.stage);
        self.texture_bindings.push(binding);
    }

    pub fn get_shader(&self) -> &ShaderClass {
        &self.shader
    }

    pub fn set_shader(&mut self, mut shader: ShaderClass) {
        shader.enable_fog("MaterialPassClass::set_shader");
        self.shader = shader;
    }

    pub fn get_vertex_material(&self) -> Option<&VertexMaterialClass> {
        self.vertex_material.as_ref().map(|arc| arc.as_ref())
    }

    pub fn get_textures(&self) -> &[Option<Arc<TextureClass>>] {
        &self.textures
    }

    pub fn set_texture(&mut self, stage: usize, texture: Arc<TextureClass>) {
        if stage < self.textures.len() {
            self.textures[stage] = Some(texture);
        }
    }

    pub fn get_texture(&self, stage: usize) -> Option<&Arc<TextureClass>> {
        self.textures.get(stage).and_then(|t| t.as_ref())
    }

    pub fn set_stage_uv_channel(&mut self, stage: usize, channel: u8) {
        if stage < self.stage_uv_channels.len() {
            self.stage_uv_channels[stage] = channel.min(3);
        }
    }

    pub fn stage_uv_channel(&self, stage: usize) -> u8 {
        self.stage_uv_channels
            .get(stage)
            .copied()
            .unwrap_or(stage as u8)
    }

    pub fn set_cull_volume(&mut self, volume: Option<OBBoxClass>) {
        self.cull_volume = volume;
    }

    pub fn cull_volume(&self) -> Option<&OBBoxClass> {
        self.cull_volume.as_ref()
    }

    pub fn enable_on_translucent_meshes(&mut self, onoff: bool) {
        self.enable_on_translucent_meshes = onoff;
    }

    pub fn is_enabled_on_translucent_meshes(&self) -> bool {
        self.enable_on_translucent_meshes
    }

    pub fn enable_per_polygon_culling(onoff: bool) {
        ENABLE_PER_POLYGON_CULLING.store(onoff, Ordering::Relaxed);
    }

    pub fn is_per_polygon_culling_enabled() -> bool {
        ENABLE_PER_POLYGON_CULLING.load(Ordering::Relaxed)
    }

    /// Get the pass index for multi-pass rendering
    pub fn get_pass_index(&self) -> usize {
        self.pass_index
    }

    /// Set the pass index for multi-pass rendering
    pub fn set_pass_index(&mut self, index: usize) {
        self.pass_index = index;
    }

    /// Get the mapper ID for animated texture transformations
    pub fn get_mapper_id(&self) -> u32 {
        self.mapper_id
    }

    /// Set the mapper ID for animated texture transformations
    pub fn set_mapper_id(&mut self, id: u32) {
        self.mapper_id = id;
    }

    /// Get a mapper argument by index (0-3)
    pub fn get_mapper_arg(&self, index: usize) -> i32 {
        if index < 4 {
            self.mapper_args[index]
        } else {
            0
        }
    }

    /// Set a mapper argument by index (0-3)
    pub fn set_mapper_arg(&mut self, index: usize, value: i32) {
        if index < 4 {
            self.mapper_args[index] = value;
        }
    }

    /// Set mapper float argument
    pub fn set_mapper_float_arg(&mut self, index: usize, value: f32) {
        if index < 4 {
            self.mapper_float_args[index] = value;
        }
    }

    /// Get mapper float argument
    pub fn get_mapper_float_arg(&self, index: usize) -> f32 {
        if index < 4 {
            self.mapper_float_args[index]
        } else {
            0.0
        }
    }

    pub fn set_mapper_float_args(&mut self, args: [f32; 4]) {
        self.mapper_float_args = args;
    }

    pub fn mapper_float_args(&self) -> [f32; 4] {
        self.mapper_float_args
    }

    /// Apply shadow-specific overrides for this pass (C++ parity)
    pub fn apply_shadow_settings(&self) -> crate::core::error::Result<()> {
        // In DX8 this would tweak render states; with WGPU those are in pipeline defs.
        // Keep as no-op for now to satisfy call sites while we complete pipeline mapping.
        Ok(())
    }
}

/// Material pass structure (equivalent to MaterialPass in C++)
#[derive(Debug)]
pub struct MaterialPass {
    pub shader: ShaderClass,
    pub vertex_material: Option<Arc<VertexMaterialClass>>,
    pub textures: [Option<Arc<TextureClass>>; MAX_TEXTURE_STAGES],
    pub texture_bindings: Vec<MaterialTextureBinding>,
    pub diffuse_vertex_colors: Option<Vec<Vec4>>,
    pub illumination_vertex_colors: Option<Vec<Vec4>>,
}

impl Clone for MaterialPass {
    fn clone(&self) -> Self {
        Self {
            shader: self.shader,
            vertex_material: self.vertex_material.clone(),
            textures: self.textures.clone(),
            texture_bindings: self.texture_bindings.clone(),
            diffuse_vertex_colors: self.diffuse_vertex_colors.clone(),
            illumination_vertex_colors: self.illumination_vertex_colors.clone(),
        }
    }
}

impl MaterialPass {
    pub fn new() -> Self {
        let mut shader = ShaderClass::new();
        shader.enable_fog("MaterialPass::new");
        Self {
            shader,
            vertex_material: None,
            textures: array::from_fn(|_| None),
            texture_bindings: Vec::new(),
            diffuse_vertex_colors: None,
            illumination_vertex_colors: None,
        }
    }

    pub fn get_shader(&self) -> &ShaderClass {
        &self.shader
    }

    pub fn get_vertex_material(&self) -> Option<&VertexMaterialClass> {
        self.vertex_material.as_ref().map(|arc| arc.as_ref())
    }

    pub fn get_textures(&self) -> &[Option<Arc<TextureClass>>] {
        &self.textures
    }

    pub fn set_texture(&mut self, stage: usize, texture: Arc<TextureClass>) {
        if stage < self.textures.len() {
            self.textures[stage] = Some(texture);
        }
    }

    pub fn get_texture(&self, stage: usize) -> Option<&Arc<TextureClass>> {
        self.textures.get(stage).and_then(|t| t.as_ref())
    }
}

/// Material manager for handling material state and batching
#[derive(Debug)]
pub struct MaterialManager {
    pub current_pass: Option<MaterialPass>,
    pub material_stack: Vec<MaterialPass>,
}

impl MaterialManager {
    /// Create a new material manager
    pub fn new() -> Self {
        Self {
            current_pass: None,
            material_stack: Vec::new(),
        }
    }

    /// Set the current material pass
    pub fn set_material_pass(&mut self, pass: MaterialPass) {
        self.current_pass = Some(pass);
    }

    /// Get the current material pass
    pub fn get_current_pass(&self) -> Option<&MaterialPass> {
        self.current_pass.as_ref()
    }

    /// Push current material onto stack
    pub fn push_material(&mut self) {
        if let Some(ref pass) = self.current_pass {
            self.material_stack.push(pass.clone());
        }
    }

    /// Pop material from stack
    pub fn pop_material(&mut self) {
        if let Some(pass) = self.material_stack.pop() {
            self.current_pass = Some(pass);
        }
    }

    /// Reset material state
    pub fn reset(&mut self) {
        self.current_pass = None;
        self.material_stack.clear();
    }

    /// Check if materials are compatible for batching
    pub fn are_materials_compatible(&self, other: &MaterialPass) -> bool {
        if let Some(ref current) = self.current_pass {
            // Check shader compatibility
            if current.shader.get_bits() != other.shader.get_bits() {
                return false;
            }

            // Check vertex material compatibility
            match (&current.vertex_material, &other.vertex_material) {
                (Some(curr_mat), Some(other_mat)) => {
                    if curr_mat.get_sort_level() != other_mat.get_sort_level() {
                        return false;
                    }
                }
                (None, None) => {}
                _ => return false,
            }

            // Check texture compatibility
            for (_i, (curr_tex, other_tex)) in current
                .textures
                .iter()
                .zip(other.textures.iter())
                .enumerate()
            {
                match (curr_tex, other_tex) {
                    (Some(curr_tex), Some(other_tex)) => {
                        if curr_tex.get_sort_level() != other_tex.get_sort_level() {
                            return false;
                        }
                    }
                    (None, None) => {}
                    _ => return false,
                }
            }

            true
        } else {
            true // No current material, so compatible
        }
    }
}

/// Material factory for creating materials from W3D data
pub struct MaterialFactory;

impl MaterialFactory {
    /// Create vertex material from W3D data
    pub fn create_vertex_material_from_w3d(
        w3d_material: &ww3d_core::W3dVertexMaterialStruct,
    ) -> VertexMaterialClass {
        VertexMaterialClass {
            name: "UnnamedVertexMaterial".to_string(),
            ambient: Vec3::new(
                w3d_material.ambient.r as f32 / 255.0,
                w3d_material.ambient.g as f32 / 255.0,
                w3d_material.ambient.b as f32 / 255.0,
            ),
            diffuse: Vec3::new(
                w3d_material.diffuse.r as f32 / 255.0,
                w3d_material.diffuse.g as f32 / 255.0,
                w3d_material.diffuse.b as f32 / 255.0,
            ),
            specular: Vec3::new(
                w3d_material.specular.r as f32 / 255.0,
                w3d_material.specular.g as f32 / 255.0,
                w3d_material.specular.b as f32 / 255.0,
            ),
            emissive: Vec3::new(
                w3d_material.emissive.r as f32 / 255.0,
                w3d_material.emissive.g as f32 / 255.0,
                w3d_material.emissive.b as f32 / 255.0,
            ),
            shininess: w3d_material.shininess,
            opacity: w3d_material.opacity,
            translucency: w3d_material.translucency,
        }
    }

    /// Create shader from W3D data
    pub fn create_shader_from_w3d(w3d_shader: &ww3d_core::W3dShaderStruct) -> ShaderClass {
        ShaderClass::from_w3d_shader(w3d_shader)
    }

    /// Create material pass from W3D material info
    pub fn create_material_pass_from_w3d(
        vertex_material: Option<Arc<VertexMaterialClass>>,
        shader: ShaderClass,
        textures: Vec<Arc<TextureClass>>,
    ) -> MaterialPass {
        let mut material_pass = MaterialPass::new();
        material_pass.vertex_material = vertex_material;
        material_pass.shader = shader;

        // Assign textures to stages
        for (i, texture) in textures.into_iter().enumerate() {
            if i < material_pass.textures.len() {
                material_pass.textures[i] = Some(texture);
            }
        }

        material_pass
    }
}

/// Texture stage classification used to translate legacy pipeline state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureStageHint {
    Base,
    Emissive,
    Environment,
    ShinyMask,
    Custom(u8),
}

impl TextureStageHint {
    pub fn to_bits(self) -> u32 {
        match self {
            TextureStageHint::Base => 0,
            TextureStageHint::Emissive => 1,
            TextureStageHint::Environment => 2,
            TextureStageHint::ShinyMask => 3,
            TextureStageHint::Custom(value) => (value & 0x0F) as u32,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureStageType {
    ColorMap,
    BumpMap,
    Unknown(u16),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureMipStrategy {
    Full,
    MaxLevels(u8),
    NoMips,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureStageSettings {
    pub address_u: TextureAddressMode,
    pub address_v: TextureAddressMode,
    pub mip_strategy: TextureMipStrategy,
    pub hint: TextureStageHint,
    pub texture_type: TextureStageType,
    pub alpha_is_bitmap: bool,
    pub filter: TextureFilterMode,
    pub anisotropy: u16,
    pub lod_bias: f32,
}

impl Default for TextureStageSettings {
    fn default() -> Self {
        Self {
            address_u: TextureAddressMode::Wrap,
            address_v: TextureAddressMode::Wrap,
            mip_strategy: TextureMipStrategy::Full,
            hint: TextureStageHint::Base,
            texture_type: TextureStageType::ColorMap,
            alpha_is_bitmap: false,
            filter: TextureFilterMode::Linear,
            anisotropy: 1,
            lod_bias: 0.0,
        }
    }
}

impl TextureStageSettings {
    pub fn from_descriptor(descriptor: &ww3d_core::W3dTextureStruct) -> Self {
        let info = descriptor.texture_info;
        let attrs = info.attributes;
        let renderer_cfg = config::get();

        let address_u = if attrs & ww3d_core::W3D_TEXTURE_CLAMP_U != 0 {
            TextureAddressMode::Clamp
        } else {
            TextureAddressMode::Wrap
        };
        let address_v = if attrs & ww3d_core::W3D_TEXTURE_CLAMP_V != 0 {
            TextureAddressMode::Clamp
        } else {
            TextureAddressMode::Wrap
        };

        let mip_strategy = if attrs & ww3d_core::W3D_TEXTURE_NO_LOD != 0 {
            TextureMipStrategy::NoMips
        } else {
            match attrs & ww3d_core::W3D_TEXTURE_MIP_LEVELS_MASK {
                ww3d_core::W3D_TEXTURE_MIP_LEVELS_ALL => TextureMipStrategy::Full,
                ww3d_core::W3D_TEXTURE_MIP_LEVELS_2 => TextureMipStrategy::MaxLevels(2),
                ww3d_core::W3D_TEXTURE_MIP_LEVELS_3 => TextureMipStrategy::MaxLevels(3),
                ww3d_core::W3D_TEXTURE_MIP_LEVELS_4 => TextureMipStrategy::MaxLevels(4),
                _ => TextureMipStrategy::Full,
            }
        };

        let hint_bits =
            ((attrs & ww3d_core::W3D_TEXTURE_HINT_MASK) >> ww3d_core::W3D_TEXTURE_HINT_SHIFT) as u8;
        let hint = match hint_bits {
            x if x
                == (ww3d_core::W3D_TEXTURE_HINT_BASE >> ww3d_core::W3D_TEXTURE_HINT_SHIFT)
                    as u8 =>
            {
                TextureStageHint::Base
            }
            x if x
                == (ww3d_core::W3D_TEXTURE_HINT_EMISSIVE >> ww3d_core::W3D_TEXTURE_HINT_SHIFT)
                    as u8 =>
            {
                TextureStageHint::Emissive
            }
            x if x
                == (ww3d_core::W3D_TEXTURE_HINT_ENVIRONMENT >> ww3d_core::W3D_TEXTURE_HINT_SHIFT)
                    as u8 =>
            {
                TextureStageHint::Environment
            }
            x if x
                == (ww3d_core::W3D_TEXTURE_HINT_SHINY_MASK >> ww3d_core::W3D_TEXTURE_HINT_SHIFT)
                    as u8 =>
            {
                TextureStageHint::ShinyMask
            }
            other => TextureStageHint::Custom(other & 0x0F),
        };

        let texture_type = match attrs & ww3d_core::W3D_TEXTURE_TYPE_MASK {
            ww3d_core::W3D_TEXTURE_TYPE_COLORMAP => TextureStageType::ColorMap,
            ww3d_core::W3D_TEXTURE_TYPE_BUMPMAP => TextureStageType::BumpMap,
            other => TextureStageType::Unknown(other),
        };

        let default_filter = match renderer_cfg.filter_quality {
            config::TextureFilterQuality::Bilinear => TextureFilterMode::Linear,
            config::TextureFilterQuality::Trilinear => TextureFilterMode::Linear,
            config::TextureFilterQuality::Anisotropic => TextureFilterMode::Anisotropic,
        };
        let default_aniso = match renderer_cfg.filter_quality {
            config::TextureFilterQuality::Anisotropic => renderer_cfg.max_anisotropy,
            _ => 1,
        };

        let (filter, anisotropy) = match hint {
            TextureStageHint::Emissive | TextureStageHint::ShinyMask => {
                (TextureFilterMode::Linear, 1)
            }
            TextureStageHint::Environment => (TextureFilterMode::Linear, 1),
            _ => (default_filter, default_aniso),
        };

        let (filter, anisotropy) = if matches!(texture_type, TextureStageType::BumpMap) {
            (TextureFilterMode::Linear, 1)
        } else {
            (filter, anisotropy)
        };

        Self {
            address_u,
            address_v,
            mip_strategy,
            hint,
            texture_type,
            alpha_is_bitmap: attrs & ww3d_core::W3D_TEXTURE_ALPHA_BITMAP != 0,
            filter,
            anisotropy,
            lod_bias: 0.0,
        }
    }
}

/// Material state tracker for render state management
#[derive(Debug, Clone)]
pub struct MaterialState {
    pub depth_test: bool,
    pub depth_write: bool,
    pub alpha_blend: bool,
    pub src_blend: u32,
    pub dst_blend: u32,
    pub cull_mode: u32,
    pub alpha_test: bool,
    pub alpha_test_value: f32,
}

impl MaterialState {
    /// Create default material state
    pub fn new() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            alpha_blend: false,
            src_blend: 2, // ONE
            dst_blend: 1, // ZERO
            cull_mode: 1, // CCW
            alpha_test: false,
            alpha_test_value: 0.0,
        }
    }

    /// Apply material state to rendering
    pub fn apply(&self) {
        // This would set the actual render state
        // In WGPU, this is handled by the pipeline and bind groups
        println!(
            "Applying material state: depth_test={}, alpha_blend={}",
            self.depth_test, self.alpha_blend
        );
    }

    /// Check if states are compatible for batching
    pub fn is_compatible(&self, other: &MaterialState) -> bool {
        self.depth_test == other.depth_test
            && self.depth_write == other.depth_write
            && self.alpha_blend == other.alpha_blend
            && self.src_blend == other.src_blend
            && self.dst_blend == other.dst_blend
            && self.cull_mode == other.cull_mode
    }
}

/// Default implementation
impl Default for MaterialState {
    fn default() -> Self {
        Self::new()
    }
}
