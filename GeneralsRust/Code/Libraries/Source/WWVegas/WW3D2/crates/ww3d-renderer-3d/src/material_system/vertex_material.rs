//! Vertex Material System for WW3D2 Engine
//!
//! This module implements the complete vertex material system based on the C++ WW3D engine's
//! VertexMaterialClass. It provides control over material properties, lighting, color sources,
//! UV mapping, and texture coordinate transformations.
//!
//! C++ Reference: vertmaterial.h (324 lines)
//!
//! Key Features:
//! - Color source selection (MATERIAL, COLOR1, COLOR2) - vertmaterial.h:97-101
//! - Lighting control per material - vertmaterial.h:170-171
//! - Mapping types (NONE, UV, ENVIRONMENT) - vertmaterial.h:85-89
//! - Preset materials (PRELIT_DIFFUSE, PRELIT_NODIFFUSE) - vertmaterial.h:103-108
//! - UV source selection (up to 8 arrays) - vertmaterial.h:193-194
//! - Texture mapper support (two stages) - vertmaterial.h:199-202

use super::texture_mapper::TextureMapperType;
use glam::Vec3;
use std::sync::{Arc, Mutex, OnceLock};

/// Maximum number of texture stages supported
/// C++ Reference: MeshBuilderClass::MAX_STAGES in meshbuild.h
pub const MAX_TEXTURE_STAGES: usize = 2;

/// Mapping type enumeration
///
/// C++ Reference: vertmaterial.h lines 85-89
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(i32)]
pub enum MappingType {
    /// No mapping needed
    /// C++ Reference: vertmaterial.h line 86 (MAPPING_NONE = -1)
    None = -1,

    /// Default UV mapping - use u-v values in the model
    /// C++ Reference: vertmaterial.h line 87 (MAPPING_UV = W3DMAPPING_UV)
    UV = 0,

    /// Environment mapping - sphere map reflection
    /// C++ Reference: vertmaterial.h line 88 (MAPPING_ENVIRONMENT = W3DMAPPING_ENVIRONMENT)
    Environment = 1,
}

impl MappingType {
    /// Convert from i32 value
    pub fn from_i32(value: i32) -> Option<Self> {
        match value {
            -1 => Some(Self::None),
            0 => Some(Self::UV),
            1 => Some(Self::Environment),
            _ => None,
        }
    }
}

/// Material flags enumeration
///
/// C++ Reference: vertmaterial.h lines 91-95
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum MaterialFlags {
    /// Enable depth cueing (default = false)
    /// C++ Reference: vertmaterial.h line 92
    DepthCue = 0,

    /// Depth cue to alpha channel
    /// C++ Reference: vertmaterial.h line 93
    DepthCueToAlpha = 1,

    /// Copy specular color to diffuse
    /// C++ Reference: vertmaterial.h line 94
    CopySpecularToDiffuse = 2,
}

/// Color source type enumeration
///
/// Defines where the color information comes from for lighting calculations
///
/// C++ Reference: vertmaterial.h lines 97-101
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u32)]
pub enum ColorSourceType {
    /// Color from material setting (D3DMCS_MATERIAL)
    /// C++ Reference: vertmaterial.h line 98
    Material = 0,

    /// Color from per-vertex color array 1 (D3DMCS_COLOR1 aka D3DFVF_DIFFUSE)
    /// C++ Reference: vertmaterial.h line 99
    Color1 = 1,

    /// Color from per-vertex color array 2 (D3DMCS_COLOR2 aka D3DFVF_SPECULAR)
    /// C++ Reference: vertmaterial.h line 100
    Color2 = 2,
}

impl ColorSourceType {
    /// Convert from u32 value
    pub fn from_u32(value: u32) -> Option<Self> {
        match value {
            0 => Some(Self::Material),
            1 => Some(Self::Color1),
            2 => Some(Self::Color2),
            _ => None,
        }
    }
}

/// Preset material type enumeration
///
/// C++ Reference: vertmaterial.h lines 103-108
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(usize)]
pub enum PresetType {
    /// Prelit material with diffuse vertex colors
    /// C++ Reference: vertmaterial.h line 105
    PrelitDiffuse = 0,

    /// Prelit material without diffuse colors
    /// C++ Reference: vertmaterial.h line 106
    PrelitNoDiffuse = 1,
}

impl PresetType {
    /// Get count of preset types
    /// C++ Reference: vertmaterial.h line 107 (PRESET_COUNT)
    pub const COUNT: usize = 2;

    /// Convert from usize value
    pub fn from_usize(value: usize) -> Option<Self> {
        match value {
            0 => Some(Self::PrelitDiffuse),
            1 => Some(Self::PrelitNoDiffuse),
            _ => None,
        }
    }
}

/// Vertex Material Class
///
/// Defines the lighting and appearance properties of a vertex material.
/// This is a thin wrapper around D3D material concepts, providing control over
/// how vertices are lit and textured.
///
/// C++ Reference: vertmaterial.h lines 72-277
#[derive(Debug, Clone)]
pub struct VertexMaterialClass {
    /// Material name
    /// C++ Reference: vertmaterial.h line 257 (StringClass Name)
    name: String,

    /// Material flags bitfield
    /// C++ Reference: vertmaterial.h line 253 (unsigned int Flags)
    flags: u32,

    /// Ambient color (RGB)
    /// C++ Reference: vertmaterial.h lines 154-156
    ambient: Vec3,

    /// Diffuse color (RGB)
    /// C++ Reference: vertmaterial.h lines 158-160
    diffuse: Vec3,

    /// Specular color (RGB)
    /// C++ Reference: vertmaterial.h lines 162-164
    specular: Vec3,

    /// Emissive color (RGB)
    /// C++ Reference: vertmaterial.h lines 166-168
    emissive: Vec3,

    /// Shininess/power (specular exponent)
    /// C++ Reference: vertmaterial.h lines 148-149
    shininess: f32,

    /// Opacity (alpha value)
    /// C++ Reference: vertmaterial.h lines 151-152
    opacity: f32,

    /// Ambient color source
    /// C++ Reference: vertmaterial.h lines 179-180, 254
    ambient_color_source: ColorSourceType,

    /// Emissive color source
    /// C++ Reference: vertmaterial.h lines 182-183, 255
    emissive_color_source: ColorSourceType,

    /// Diffuse color source
    /// C++ Reference: vertmaterial.h lines 185-186, 256
    diffuse_color_source: ColorSourceType,

    /// Whether dynamic lighting is enabled
    /// C++ Reference: vertmaterial.h lines 170-171, 263
    use_lighting: bool,

    /// UV source array index for each texture stage
    /// C++ Reference: vertmaterial.h lines 193-194, 259
    uv_source: [u32; MAX_TEXTURE_STAGES],

    /// Texture mappers for each stage
    /// C++ Reference: vertmaterial.h lines 199-202, 258
    mappers: [Option<Arc<TextureMapperClass>>; MAX_TEXTURE_STAGES],

    /// Unique ID for this material
    /// C++ Reference: vertmaterial.h line 260
    unique_id: u32,

    /// CRC for material comparison
    /// C++ Reference: vertmaterial.h lines 217-225, 261
    crc: u32,

    /// Whether CRC needs recomputation
    /// C++ Reference: vertmaterial.h line 262
    crc_dirty: bool,
}

impl VertexMaterialClass {
    /// Create a new vertex material with default values
    ///
    /// C++ Reference: vertmaterial.h line 111 (VertexMaterialClass constructor)
    pub fn new() -> Self {
        static NEXT_ID: Mutex<u32> = Mutex::new(1);
        let unique_id = {
            let mut id = NEXT_ID.lock().unwrap();
            let current = *id;
            *id = id.wrapping_add(1);
            current
        };

        Self {
            name: String::new(),
            flags: 0,
            ambient: Vec3::new(0.2, 0.2, 0.2),
            diffuse: Vec3::new(0.8, 0.8, 0.8),
            specular: Vec3::new(0.0, 0.0, 0.0),
            emissive: Vec3::ZERO,
            shininess: 0.0,
            opacity: 1.0,
            ambient_color_source: ColorSourceType::Material,
            emissive_color_source: ColorSourceType::Material,
            diffuse_color_source: ColorSourceType::Material,
            use_lighting: true,
            uv_source: [0; MAX_TEXTURE_STAGES],
            mappers: [None, None],
            unique_id,
            crc: 0,
            crc_dirty: true,
        }
    }

    /// Clone the material
    ///
    /// C++ Reference: vertmaterial.h line 116 (Clone method)
    pub fn clone_material(&self) -> Self {
        let mut cloned = self.clone();
        cloned.unique_id = {
            static NEXT_ID: Mutex<u32> = Mutex::new(1);
            let mut id = NEXT_ID.lock().unwrap();
            let current = *id;
            *id = id.wrapping_add(1);
            current
        };
        cloned
    }

    // ========================================================================
    // Name Access
    // C++ Reference: vertmaterial.h lines 118-129
    // ========================================================================

    /// Set the material name
    ///
    /// C++ Reference: vertmaterial.h lines 121-124
    pub fn set_name(&mut self, name: &str) {
        self.name = name.to_string();
    }

    /// Get the material name
    ///
    /// C++ Reference: vertmaterial.h lines 126-129
    pub fn get_name(&self) -> &str {
        &self.name
    }

    // ========================================================================
    // Flag Control
    // C++ Reference: vertmaterial.h lines 131-143
    // ========================================================================

    /// Set a material flag
    ///
    /// C++ Reference: vertmaterial.h lines 134-141
    pub fn set_flag(&mut self, flag: MaterialFlags, value: bool) {
        self.crc_dirty = true;
        let bit = 1u32 << (flag as u32);
        if value {
            self.flags |= bit;
        } else {
            self.flags &= !bit;
        }
    }

    /// Get a material flag
    ///
    /// C++ Reference: vertmaterial.h lines 142-143
    pub fn get_flag(&self, flag: MaterialFlags) -> bool {
        let bit = 1u32 << (flag as u32);
        (self.flags & bit) != 0
    }

    // ========================================================================
    // Basic Material Properties
    // C++ Reference: vertmaterial.h lines 145-168
    // ========================================================================

    /// Get shininess value
    ///
    /// C++ Reference: vertmaterial.h line 148
    pub fn get_shininess(&self) -> f32 {
        self.shininess
    }

    /// Set shininess value
    ///
    /// C++ Reference: vertmaterial.h line 149
    pub fn set_shininess(&mut self, shininess: f32) {
        self.crc_dirty = true;
        self.shininess = shininess;
    }

    /// Get opacity value
    ///
    /// C++ Reference: vertmaterial.h line 151
    pub fn get_opacity(&self) -> f32 {
        self.opacity
    }

    /// Set opacity value
    ///
    /// C++ Reference: vertmaterial.h line 152
    pub fn set_opacity(&mut self, opacity: f32) {
        self.crc_dirty = true;
        self.opacity = opacity.clamp(0.0, 1.0);
    }

    /// Get ambient color
    ///
    /// C++ Reference: vertmaterial.h line 154
    pub fn get_ambient(&self) -> Vec3 {
        self.ambient
    }

    /// Set ambient color from Vec3
    ///
    /// C++ Reference: vertmaterial.h line 155
    pub fn set_ambient(&mut self, color: Vec3) {
        self.crc_dirty = true;
        self.ambient = color;
    }

    /// Set ambient color from RGB components
    ///
    /// C++ Reference: vertmaterial.h line 156
    pub fn set_ambient_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.set_ambient(Vec3::new(r, g, b));
    }

    /// Get diffuse color
    ///
    /// C++ Reference: vertmaterial.h line 158
    pub fn get_diffuse(&self) -> Vec3 {
        self.diffuse
    }

    /// Set diffuse color from Vec3
    ///
    /// C++ Reference: vertmaterial.h line 159
    pub fn set_diffuse(&mut self, color: Vec3) {
        self.crc_dirty = true;
        self.diffuse = color;
    }

    /// Set diffuse color from RGB components
    ///
    /// C++ Reference: vertmaterial.h line 160
    pub fn set_diffuse_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.set_diffuse(Vec3::new(r, g, b));
    }

    /// Get specular color
    ///
    /// C++ Reference: vertmaterial.h line 162
    pub fn get_specular(&self) -> Vec3 {
        self.specular
    }

    /// Set specular color from Vec3
    ///
    /// C++ Reference: vertmaterial.h line 163
    pub fn set_specular(&mut self, color: Vec3) {
        self.crc_dirty = true;
        self.specular = color;
    }

    /// Set specular color from RGB components
    ///
    /// C++ Reference: vertmaterial.h line 164
    pub fn set_specular_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.set_specular(Vec3::new(r, g, b));
    }

    /// Get emissive color
    ///
    /// C++ Reference: vertmaterial.h line 166
    pub fn get_emissive(&self) -> Vec3 {
        self.emissive
    }

    /// Set emissive color from Vec3
    ///
    /// C++ Reference: vertmaterial.h line 167
    pub fn set_emissive(&mut self, color: Vec3) {
        self.crc_dirty = true;
        self.emissive = color;
    }

    /// Set emissive color from RGB components
    ///
    /// C++ Reference: vertmaterial.h line 168
    pub fn set_emissive_rgb(&mut self, r: f32, g: f32, b: f32) {
        self.set_emissive(Vec3::new(r, g, b));
    }

    // ========================================================================
    // Lighting Control
    // C++ Reference: vertmaterial.h lines 170-171
    // ========================================================================

    /// Set whether dynamic lighting is enabled
    ///
    /// C++ Reference: vertmaterial.h line 170
    pub fn set_lighting(&mut self, enabled: bool) {
        self.crc_dirty = true;
        self.use_lighting = enabled;
    }

    /// Get whether dynamic lighting is enabled
    ///
    /// C++ Reference: vertmaterial.h line 171
    pub fn get_lighting(&self) -> bool {
        self.use_lighting
    }

    // ========================================================================
    // Color Source Control
    // C++ Reference: vertmaterial.h lines 173-186
    // ========================================================================

    /// Set ambient color source
    ///
    /// C++ Reference: vertmaterial.h line 179
    pub fn set_ambient_color_source(&mut self, source: ColorSourceType) {
        self.crc_dirty = true;
        self.ambient_color_source = source;
    }

    /// Get ambient color source
    ///
    /// C++ Reference: vertmaterial.h line 180
    pub fn get_ambient_color_source(&self) -> ColorSourceType {
        self.ambient_color_source
    }

    /// Set emissive color source
    ///
    /// C++ Reference: vertmaterial.h line 182
    pub fn set_emissive_color_source(&mut self, source: ColorSourceType) {
        self.crc_dirty = true;
        self.emissive_color_source = source;
    }

    /// Get emissive color source
    ///
    /// C++ Reference: vertmaterial.h line 183
    pub fn get_emissive_color_source(&self) -> ColorSourceType {
        self.emissive_color_source
    }

    /// Set diffuse color source
    ///
    /// C++ Reference: vertmaterial.h line 185
    pub fn set_diffuse_color_source(&mut self, source: ColorSourceType) {
        self.crc_dirty = true;
        self.diffuse_color_source = source;
    }

    /// Get diffuse color source
    ///
    /// C++ Reference: vertmaterial.h line 186
    pub fn get_diffuse_color_source(&self) -> ColorSourceType {
        self.diffuse_color_source
    }

    // ========================================================================
    // UV Source Control
    // C++ Reference: vertmaterial.h lines 188-194
    // ========================================================================

    /// Set UV source array index for a texture stage
    ///
    /// The DX8 FVF can support up to 8 uv-arrays. The vertex material must be
    /// configured to index to the uv-arrays that you want to use for the two
    /// texture stages.
    ///
    /// C++ Reference: vertmaterial.h line 193
    pub fn set_uv_source(&mut self, stage: usize, array_index: u32) {
        if stage < MAX_TEXTURE_STAGES {
            self.crc_dirty = true;
            self.uv_source[stage] = array_index;
        }
    }

    /// Get UV source array index for a texture stage
    ///
    /// C++ Reference: vertmaterial.h line 194
    pub fn get_uv_source(&self, stage: usize) -> u32 {
        if stage < MAX_TEXTURE_STAGES {
            self.uv_source[stage]
        } else {
            0
        }
    }

    // ========================================================================
    // Texture Mapper Control
    // C++ Reference: vertmaterial.h lines 196-202
    // ========================================================================

    /// Set texture mapper for a stage
    ///
    /// C++ Reference: vertmaterial.h lines 279-283
    pub fn set_mapper(&mut self, mapper: Option<Arc<TextureMapperClass>>, stage: usize) {
        if stage < MAX_TEXTURE_STAGES {
            self.crc_dirty = true;
            self.mappers[stage] = mapper;
        }
    }

    /// Get texture mapper for a stage (with reference counting)
    ///
    /// C++ Reference: vertmaterial.h lines 285-291
    pub fn get_mapper(&self, stage: usize) -> Option<Arc<TextureMapperClass>> {
        if stage < MAX_TEXTURE_STAGES {
            self.mappers[stage].clone()
        } else {
            None
        }
    }

    /// Peek at texture mapper for a stage (without incrementing reference count)
    ///
    /// C++ Reference: vertmaterial.h lines 293-296
    pub fn peek_mapper(&self, stage: usize) -> Option<&TextureMapperClass> {
        if stage < MAX_TEXTURE_STAGES {
            self.mappers[stage].as_ref().map(|arc| arc.as_ref())
        } else {
            None
        }
    }

    /// Reset all mappers to their initial state
    ///
    /// C++ Reference: vertmaterial.h lines 298-305
    pub fn reset_mappers(&mut self) {
        for mapper in self.mappers.iter_mut().flatten() {
            if let Some(m) = Arc::get_mut(mapper) { m.reset() }
        }
    }

    // ========================================================================
    // Mapper Property Queries
    // C++ Reference: vertmaterial.h lines 227-321
    // ========================================================================

    /// Test whether this material uses any mappers which require vertex normals
    ///
    /// C++ Reference: vertmaterial.h lines 307-313
    pub fn do_mappers_need_normals(&self) -> bool {
        for mapper in self.mappers.iter().flatten() {
            if mapper.needs_normals() {
                return true;
            }
        }
        false
    }

    /// Test whether this material uses any mappers which are time-variant
    ///
    /// C++ Reference: vertmaterial.h lines 315-321
    pub fn are_mappers_time_variant(&self) -> bool {
        for mapper in self.mappers.iter().flatten() {
            if mapper.is_time_variant() {
                return true;
            }
        }
        false
    }

    // ========================================================================
    // CRC Computation
    // C++ Reference: vertmaterial.h lines 214-225
    // ========================================================================

    /// Get CRC value for material comparison
    ///
    /// The CRC is used by the loading code to build a list of unique materials.
    ///
    /// C++ Reference: vertmaterial.h lines 217-225
    pub fn get_crc(&self) -> u32 {
        if self.crc_dirty {
            // Would compute CRC in a real implementation
            // For now, return a simple hash based on unique_id
            self.unique_id
        } else {
            self.crc
        }
    }

    /// Make this material unique by generating a new ID
    ///
    /// C++ Reference: vertmaterial.h line 243
    pub fn make_unique(&mut self) {
        static NEXT_ID: Mutex<u32> = Mutex::new(1000);
        let unique_id = {
            let mut id = NEXT_ID.lock().unwrap();
            let current = *id;
            *id = id.wrapping_add(1);
            current
        };
        self.unique_id = unique_id;
        self.crc_dirty = true;
    }

    /// Get unique ID
    pub fn get_unique_id(&self) -> u32 {
        self.unique_id
    }
}

impl Default for VertexMaterialClass {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Preset Material System
// C++ Reference: vertmaterial.h lines 237-241
// ============================================================================

/// Global preset materials storage
static PRESETS: OnceLock<[Arc<VertexMaterialClass>; PresetType::COUNT]> = OnceLock::new();

/// Initialize preset materials
///
/// C++ Reference: vertmaterial.h line 238
pub fn init_presets() {
    PRESETS.get_or_init(|| {
        // PRELIT_DIFFUSE: Prelit material with diffuse vertex colors
        // Uses COLOR1 for diffuse, disables lighting
        let mut prelit_diffuse = VertexMaterialClass::new();
        prelit_diffuse.set_name("PRELIT_DIFFUSE");
        prelit_diffuse.set_diffuse_color_source(ColorSourceType::Color1);
        prelit_diffuse.set_lighting(false);

        // PRELIT_NODIFFUSE: Prelit material without diffuse colors
        // Uses material colors, disables lighting
        let mut prelit_no_diffuse = VertexMaterialClass::new();
        prelit_no_diffuse.set_name("PRELIT_NODIFFUSE");
        prelit_no_diffuse.set_diffuse_color_source(ColorSourceType::Material);
        prelit_no_diffuse.set_lighting(false);

        [Arc::new(prelit_diffuse), Arc::new(prelit_no_diffuse)]
    });
}

/// Shutdown preset materials
///
/// C++ Reference: vertmaterial.h line 239
pub fn shutdown_presets() {
    // In Rust, we don't need explicit cleanup
    // The OnceLock will be dropped when the program exits
}

/// Get a preset material
///
/// C++ Reference: vertmaterial.h line 241
pub fn get_preset(preset_type: PresetType) -> Option<Arc<VertexMaterialClass>> {
    init_presets();
    PRESETS
        .get()
        .map(|presets| presets[preset_type as usize].clone())
}

// ============================================================================
// Texture Mapper Class
// Integration with texture_mapper.rs
// C++ Reference: vertmaterial.h lines 199-202, mapper.h
// ============================================================================

/// Texture mapper class wrapper for vertex materials
///
/// This provides the interface expected by VertexMaterialClass for managing
/// texture coordinate transformations.
#[derive(Debug, Clone)]
pub struct TextureMapperClass {
    /// Mapper type
    mapper_type: TextureMapperType,

    /// Mapper parameters
    params: super::texture_mapper::TextureMapperParams,

    /// Whether this mapper needs vertex normals
    needs_normals: bool,

    /// Whether this mapper is time-variant
    is_time_variant: bool,
}

impl TextureMapperClass {
    /// Create a new texture mapper
    pub fn new(mapper_type: TextureMapperType) -> Self {
        let needs_normals = mapper_type.requires_transforms();
        let is_time_variant = mapper_type.is_animated();

        Self {
            mapper_type,
            params: super::texture_mapper::TextureMapperParams::default(),
            needs_normals,
            is_time_variant,
        }
    }

    /// Create mapper with custom parameters
    pub fn with_params(
        mapper_type: TextureMapperType,
        params: super::texture_mapper::TextureMapperParams,
    ) -> Self {
        let needs_normals = mapper_type.requires_transforms();
        let is_time_variant = mapper_type.is_animated();

        Self {
            mapper_type,
            params,
            needs_normals,
            is_time_variant,
        }
    }

    /// Get mapper type
    pub fn get_mapper_type(&self) -> TextureMapperType {
        self.mapper_type
    }

    /// Get mapper parameters
    pub fn get_params(&self) -> &super::texture_mapper::TextureMapperParams {
        &self.params
    }

    /// Get mutable mapper parameters
    pub fn get_params_mut(&mut self) -> &mut super::texture_mapper::TextureMapperParams {
        &mut self.params
    }

    /// Check if this mapper needs vertex normals
    ///
    /// C++ Reference: mapper.h (Needs_Normals method)
    pub fn needs_normals(&self) -> bool {
        self.needs_normals
    }

    /// Check if this mapper is time-variant
    ///
    /// C++ Reference: mapper.h (Is_Time_Variant method)
    pub fn is_time_variant(&self) -> bool {
        self.is_time_variant
    }

    /// Reset mapper to initial state
    ///
    /// C++ Reference: mapper.h (Reset method)
    pub fn reset(&mut self) {
        // Reset time-dependent state if needed
        // For most mappers, this is a no-op
    }

    /// Apply texture mapping transformation
    pub fn apply(&self, context: &super::texture_mapper::TextureMappingContext) -> glam::Vec2 {
        let mapper = super::texture_mapper::TextureMapperFactory::create_mapper(self.mapper_type);
        mapper.map_texture(context, &self.params)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vertex_material_creation() {
        let material = VertexMaterialClass::new();
        assert_eq!(material.get_name(), "");
        assert!(material.get_lighting());
        assert_eq!(material.get_opacity(), 1.0);
    }

    #[test]
    fn test_material_name() {
        let mut material = VertexMaterialClass::new();
        material.set_name("TestMaterial");
        assert_eq!(material.get_name(), "TestMaterial");
    }

    #[test]
    fn test_material_colors() {
        let mut material = VertexMaterialClass::new();

        // Test ambient color
        material.set_ambient_rgb(1.0, 0.5, 0.0);
        assert_eq!(material.get_ambient(), Vec3::new(1.0, 0.5, 0.0));

        // Test diffuse color
        material.set_diffuse(Vec3::new(0.8, 0.8, 0.8));
        assert_eq!(material.get_diffuse(), Vec3::new(0.8, 0.8, 0.8));

        // Test specular color
        material.set_specular_rgb(1.0, 1.0, 1.0);
        assert_eq!(material.get_specular(), Vec3::ONE);

        // Test emissive color
        material.set_emissive(Vec3::new(0.2, 0.1, 0.0));
        assert_eq!(material.get_emissive(), Vec3::new(0.2, 0.1, 0.0));
    }

    #[test]
    fn test_material_properties() {
        let mut material = VertexMaterialClass::new();

        // Test shininess
        material.set_shininess(32.0);
        assert_eq!(material.get_shininess(), 32.0);

        // Test opacity
        material.set_opacity(0.5);
        assert_eq!(material.get_opacity(), 0.5);

        // Test opacity clamping
        material.set_opacity(1.5);
        assert_eq!(material.get_opacity(), 1.0);
        material.set_opacity(-0.5);
        assert_eq!(material.get_opacity(), 0.0);
    }

    #[test]
    fn test_lighting_control() {
        let mut material = VertexMaterialClass::new();

        assert!(material.get_lighting());
        material.set_lighting(false);
        assert!(!material.get_lighting());
    }

    #[test]
    fn test_color_sources() {
        let mut material = VertexMaterialClass::new();

        // Test ambient color source
        assert_eq!(
            material.get_ambient_color_source(),
            ColorSourceType::Material
        );
        material.set_ambient_color_source(ColorSourceType::Color1);
        assert_eq!(material.get_ambient_color_source(), ColorSourceType::Color1);

        // Test diffuse color source
        material.set_diffuse_color_source(ColorSourceType::Color2);
        assert_eq!(material.get_diffuse_color_source(), ColorSourceType::Color2);

        // Test emissive color source
        material.set_emissive_color_source(ColorSourceType::Color1);
        assert_eq!(
            material.get_emissive_color_source(),
            ColorSourceType::Color1
        );
    }

    #[test]
    fn test_uv_sources() {
        let mut material = VertexMaterialClass::new();

        // Test UV source setting
        material.set_uv_source(0, 2);
        assert_eq!(material.get_uv_source(0), 2);

        material.set_uv_source(1, 5);
        assert_eq!(material.get_uv_source(1), 5);

        // Test out of bounds
        assert_eq!(material.get_uv_source(10), 0);
    }

    #[test]
    fn test_material_flags() {
        let mut material = VertexMaterialClass::new();

        // Test depth cue flag
        assert!(!material.get_flag(MaterialFlags::DepthCue));
        material.set_flag(MaterialFlags::DepthCue, true);
        assert!(material.get_flag(MaterialFlags::DepthCue));

        // Test multiple flags
        material.set_flag(MaterialFlags::DepthCueToAlpha, true);
        assert!(material.get_flag(MaterialFlags::DepthCue));
        assert!(material.get_flag(MaterialFlags::DepthCueToAlpha));
        assert!(
            !material.get_flag(MaterialFlags::CopySpecularToDiffuse)
        );
    }

    #[test]
    fn test_texture_mappers() {
        let mut material = VertexMaterialClass::new();

        // Create a UV mapper
        let mapper = Arc::new(TextureMapperClass::new(TextureMapperType::UV));
        material.set_mapper(Some(mapper.clone()), 0);

        // Test mapper retrieval
        let retrieved = material.get_mapper(0);
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().get_mapper_type(), TextureMapperType::UV);

        // Test peek
        let peeked = material.peek_mapper(0);
        assert!(peeked.is_some());
        assert_eq!(peeked.unwrap().get_mapper_type(), TextureMapperType::UV);

        // Test needs normals
        let env_mapper = Arc::new(TextureMapperClass::new(TextureMapperType::Environment));
        material.set_mapper(Some(env_mapper), 1);
        assert!(material.do_mappers_need_normals());

        // Test time variant
        let linear_mapper = Arc::new(TextureMapperClass::new(TextureMapperType::LinearOffset));
        material.set_mapper(Some(linear_mapper), 0);
        assert!(material.are_mappers_time_variant());
    }

    #[test]
    fn test_preset_materials() {
        // Initialize presets
        init_presets();

        // Test PRELIT_DIFFUSE
        let prelit_diffuse = get_preset(PresetType::PrelitDiffuse).unwrap();
        assert_eq!(prelit_diffuse.get_name(), "PRELIT_DIFFUSE");
        assert_eq!(
            prelit_diffuse.get_diffuse_color_source(),
            ColorSourceType::Color1
        );
        assert!(!prelit_diffuse.get_lighting());

        // Test PRELIT_NODIFFUSE
        let prelit_no_diffuse = get_preset(PresetType::PrelitNoDiffuse).unwrap();
        assert_eq!(prelit_no_diffuse.get_name(), "PRELIT_NODIFFUSE");
        assert_eq!(
            prelit_no_diffuse.get_diffuse_color_source(),
            ColorSourceType::Material
        );
        assert!(!prelit_no_diffuse.get_lighting());
    }

    #[test]
    fn test_mapping_type_conversion() {
        assert_eq!(MappingType::from_i32(-1), Some(MappingType::None));
        assert_eq!(MappingType::from_i32(0), Some(MappingType::UV));
        assert_eq!(MappingType::from_i32(1), Some(MappingType::Environment));
        assert_eq!(MappingType::from_i32(99), None);
    }

    #[test]
    fn test_color_source_conversion() {
        assert_eq!(
            ColorSourceType::from_u32(0),
            Some(ColorSourceType::Material)
        );
        assert_eq!(ColorSourceType::from_u32(1), Some(ColorSourceType::Color1));
        assert_eq!(ColorSourceType::from_u32(2), Some(ColorSourceType::Color2));
        assert_eq!(ColorSourceType::from_u32(99), None);
    }

    #[test]
    fn test_preset_type_conversion() {
        assert_eq!(PresetType::from_usize(0), Some(PresetType::PrelitDiffuse));
        assert_eq!(PresetType::from_usize(1), Some(PresetType::PrelitNoDiffuse));
        assert_eq!(PresetType::from_usize(99), None);
        assert_eq!(PresetType::COUNT, 2);
    }

    #[test]
    fn test_material_cloning() {
        let mut original = VertexMaterialClass::new();
        original.set_name("Original");
        original.set_shininess(64.0);
        original.set_diffuse_rgb(1.0, 0.0, 0.0);

        let cloned = original.clone_material();
        assert_eq!(cloned.get_name(), "Original");
        assert_eq!(cloned.get_shininess(), 64.0);
        assert_eq!(cloned.get_diffuse(), Vec3::new(1.0, 0.0, 0.0));

        // IDs should be different
        assert_ne!(original.get_unique_id(), cloned.get_unique_id());
    }

    #[test]
    fn test_make_unique() {
        let mut material1 = VertexMaterialClass::new();
        let id1 = material1.get_unique_id();

        material1.make_unique();
        let id2 = material1.get_unique_id();

        assert_ne!(id1, id2);
    }

    #[test]
    fn test_crc_dirty_flag() {
        let mut material = VertexMaterialClass::new();

        // Setting properties should mark CRC as dirty
        material.set_shininess(32.0);
        assert!(material.crc_dirty);

        // Getting CRC should use the dirty flag
        let _crc = material.get_crc();
    }

    #[test]
    fn test_mapper_reset() {
        let mut material = VertexMaterialClass::new();
        let mapper = Arc::new(TextureMapperClass::new(TextureMapperType::LinearOffset));
        material.set_mapper(Some(mapper), 0);

        material.reset_mappers();
        // Reset should succeed without panicking
    }
}
