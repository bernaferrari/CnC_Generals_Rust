//! Mechanical split from `assets/models.rs`. No behavior change.
#![allow(dead_code, unused_imports)]
use super::prelude::*;
use super::w3d_anim::*;
use super::w3d_format::*;
use super::w3d_loader::*;
use super::w3d_loader_parse::*;
use super::w3d_mesh_build::*;
use super::w3d_model::*;
use super::*;

pub(super) const W3D_MESH_FLAG_NONE: u32 = 0;
pub(super) const W3D_MESH_FLAG_HIDDEN: u32 = 0x00000001;
pub(super) const W3D_MESH_FLAG_TWO_SIDED: u32 = 0x00000002;
pub(super) const W3D_MESH_FLAG_CAST_SHADOW: u32 = 0x00000004;
pub(super) const W3D_MESH_FLAG_GEOMETRY_TYPE_MASK: u32 = 0x00FF0000;
pub(super) const W3D_MESH_FLAG_GEOMETRY_TYPE_NORMAL: u32 = 0x00000000;
pub(super) const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED: u32 = 0x00010000;
pub(super) const W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN: u32 = 0x00020000;
pub(super) const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ORIENTED: u32 = 0x00060000;

/// C++ SAGE engine compatible vertex data - internal format for W3D loading
/// This gets converted to VertexXYZNDUV2 for rendering
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct W3DVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl W3DVertex {
    /// Convert to C++ SAGE VertexFormatXYZNDUV2 format for rendering
    pub fn to_sage_vertex(&self, material_color: Vec3) -> crate::cnc_game_engine::VertexXYZNDUV2 {
        // Pack diffuse color as RGBA bytes (D3D8 style)
        let r = ((self.color[0] * material_color.x * 255.0) as u32).min(255);
        let g = ((self.color[1] * material_color.y * 255.0) as u32).min(255);
        let b = ((self.color[2] * material_color.z * 255.0) as u32).min(255);
        let a = ((self.color[3] * 255.0) as u32).min(255);
        let diffuse_packed = (a << 24) | (r << 16) | (g << 8) | b;

        crate::cnc_game_engine::VertexXYZNDUV2 {
            position: self.position,
            normal: self.normal,
            diffuse: diffuse_packed,
            tex_coords0: self.uv,    // Primary texture coordinates
            tex_coords1: [0.0, 0.0], // Secondary UV for multi-texturing
        }
    }
}

/// Map W3D shader blend factors to BlendMode — matches C++ W3DSHADER_SRCBLENDFUNC_*
/// and W3DSHADER_DESTBLENDFUNC_* constants from w3d_file.h.
///
/// C++ W3D src blend constants:
///   0 = ZERO, 1 = ONE (default), 2 = SRC_ALPHA, 3 = ONE_MINUS_SRC_ALPHA
/// C++ W3D dest blend constants:
///   0 = ZERO (default), 1 = ONE, 2 = SRC_COLOR, 3 = ONE_MINUS_SRC_COLOR,
///   4 = SRC_ALPHA, 5 = ONE_MINUS_SRC_ALPHA, 6 = SRC_COLOR_PREFOG
pub(super) fn shader_blend_to_mode(
    src_blend: u8,
    dest_blend: u8,
    alpha_test: u8,
) -> (BlendMode, bool) {
    let alpha_test_enabled = alpha_test != 0;

    match (src_blend, dest_blend) {
        // Opaque (default shader state): src=ONE, dest=ZERO
        (1, 0) | (0, 0) => (BlendMode::Opaque, alpha_test_enabled),

        // Standard alpha blending: src=SRC_ALPHA, dest=ONE_MINUS_SRC_ALPHA
        (2, 5) => (BlendMode::Alpha, alpha_test_enabled),

        // Additive: src=ONE, dest=ONE (full additive)
        (1, 1) => (BlendMode::Additive, alpha_test_enabled),

        // Additive with alpha: src=SRC_ALPHA, dest=ONE
        (2, 1) => (BlendMode::Additive, alpha_test_enabled),

        // Modulate (multiply): src combined with dest=SRC_COLOR or ONE_MINUS_SRC_COLOR
        (_, 2) | (_, 3) => (BlendMode::Modulate, alpha_test_enabled),

        // Alpha-blended with dest=SRC_ALPHA
        (_, 4) => (BlendMode::Alpha, alpha_test_enabled),

        // Any other non-zero dest blend → treat as alpha blend
        (_, d) if d != 0 => (BlendMode::Alpha, alpha_test_enabled),

        // Fallback: opaque
        _ => (BlendMode::Opaque, alpha_test_enabled),
    }
}

pub(super) fn w3d_position_to_world(position: [f32; 3]) -> [f32; 3] {
    // Legacy W3D content is authored in X/Y ground with Z-up. The active Rust world
    // uses X/Z ground with Y-up, so swap the vertical and depth axes on import.
    [position[0], position[2], position[1]]
}

pub(super) fn w3d_normal_to_world(normal: [f32; 3]) -> [f32; 3] {
    [normal[0], normal[2], normal[1]]
}

pub(super) fn push_world_space_triangle(indices: &mut Vec<u32>, a: u32, b: u32, c: u32) {
    // Swapping Y/Z to move legacy W3D content into Rust's Y-up world flips handedness.
    // Mirror the C++ visible winding by reversing triangle order at import time so
    // backface culling in the WW3D renderer keeps the same observable result.
    indices.push(a);
    indices.push(c);
    indices.push(b);
}

/// W3D material information - matches C++ VertexMaterialClass exactly
#[derive(Debug, Clone)]
pub struct W3DMaterial {
    pub name: String,
    pub diffuse_color: Vec3,  // Color reflected when illuminated by lighting
    pub specular_color: Vec3, // Sharp, concentrated reflective highlights
    pub emissive_color: Vec3, // Self-illumination color (glow)
    pub shininess: f32,       // Specular power (higher = sharper highlights)
    pub opacity: f32,         // Transparency: 1.0 = opaque, 0.0 = transparent
    pub texture_name: Option<String>,

    // C++ VertexMaterialClass multi-stage texture mapping properties
    pub stage0_mapping: TextureStageMapping,
    pub stage1_mapping: Option<TextureStageMapping>,
    pub stage2_mapping: Option<TextureStageMapping>,
    pub stage3_mapping: Option<TextureStageMapping>,

    // BumpEnv vertex material mapping (for normal/bump mapping)
    pub bump_rotation: f32, // Bump texture rotation
    pub bump_scale: f32,    // Bump effect intensity
    pub u_per_sec: f32,     // U coordinate animation speed
    pub v_per_sec: f32,     // V coordinate animation speed
    pub u_scale: f32,       // U coordinate scaling
    pub v_scale: f32,       // V coordinate scaling

    // Shader blending modes for transparency and alpha testing
    pub blend_mode: BlendMode,
    pub alpha_test_enabled: bool,
    pub alpha_reference: f32,
}

/// Texture stage mapping - matches C++ texture stage system
#[derive(Debug, Clone)]
pub struct TextureStageMapping {
    pub texture_name: Option<String>,
    pub uv_source: UVSource, // Which UV set to use
    pub blend_mode: TextureBlendMode,
    pub address_u: TextureAddressMode,
    pub address_v: TextureAddressMode,
    pub min_filter: TextureFilter,
    pub mag_filter: TextureFilter,
    pub mip_filter: TextureFilter,
}

/// UV coordinate source for multi-UV models
#[derive(Debug, Clone, Copy)]
pub enum UVSource {
    UV0, // Primary texture coordinates
    UV1, // Secondary texture coordinates
    UV2, // Tertiary texture coordinates
    UV3, // Quaternary texture coordinates
}

/// Texture blending modes - matches C++ shader blending
#[derive(Debug, Clone, Copy)]
pub enum TextureBlendMode {
    Replace,  // Replace previous stage
    Modulate, // Multiply with previous stage
    Add,      // Add to previous stage
    Subtract, // Subtract from previous stage
    Blend,    // Alpha blend with previous stage
}

/// Material blending modes for transparency
#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Opaque,   // No blending (solid)
    Alpha,    // Standard alpha blending
    Additive, // Additive blending (for effects)
    Modulate, // Multiplicative blending
}

/// Texture addressing modes
#[derive(Debug, Clone, Copy)]
pub enum TextureAddressMode {
    Wrap,   // Repeat texture
    Clamp,  // Clamp to edge
    Mirror, // Mirror texture
}

/// Texture filtering modes
#[derive(Debug, Clone, Copy)]
pub enum TextureFilter {
    Point,       // Nearest neighbor
    Linear,      // Linear interpolation
    Anisotropic, // Anisotropic filtering
}

impl Default for W3DMaterial {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            diffuse_color: Vec3::new(1.0, 1.0, 1.0), // Pure white like C++ original
            specular_color: Vec3::new(0.0, 0.0, 0.0), // Black specular like C++ original
            emissive_color: Vec3::ZERO,
            shininess: 0.0, // C++ default shininess
            opacity: 1.0,
            texture_name: None,

            // Default texture stage 0 mapping
            stage0_mapping: TextureStageMapping::default(),
            stage1_mapping: None,
            stage2_mapping: None,
            stage3_mapping: None,

            // Default BumpEnv properties
            bump_rotation: 0.0,
            bump_scale: 1.0,
            u_per_sec: 0.0,
            v_per_sec: 0.0,
            u_scale: 1.0,
            v_scale: 1.0,

            // Default blending
            blend_mode: BlendMode::Opaque,
            alpha_test_enabled: false,
            alpha_reference: 0.5,
        }
    }
}

impl Default for TextureStageMapping {
    fn default() -> Self {
        Self {
            texture_name: None,
            uv_source: UVSource::UV0,
            blend_mode: TextureBlendMode::Replace,
            address_u: TextureAddressMode::Wrap,
            address_v: TextureAddressMode::Wrap,
            min_filter: TextureFilter::Linear,
            mag_filter: TextureFilter::Linear,
            mip_filter: TextureFilter::Linear,
        }
    }
}

/// W3D mesh data
#[derive(Debug, Clone)]
pub struct W3DMesh {
    pub name: String,
    /// Exact source `W3dMeshHeader3Struct::ContainerName`.  HLOD child binding
    /// requires this authored identity; `name` alone is not authority.
    pub container_name: String,
    pub vertices: Vec<W3DVertex>,
    pub indices: Vec<u32>,
    pub material: W3DMaterial,
    pub transform: Mat4,
    pub header: Option<W3dMeshHeader3Struct>,
    pub stage_texcoords: Vec<Vec<[f32; 2]>>,
    pub passes: Vec<MaterialPassInfo>,
    pub per_pass_stage_texture_ids: Vec<Vec<Vec<u32>>>,
    pub per_pass_stage_texture_names: Vec<Vec<Vec<String>>>,
    pub per_pass_vertex_material_ids: Vec<Vec<u32>>,
    pub per_pass_shader_ids: Vec<Vec<u32>>,
    pub per_pass_dcg_colors: Vec<Vec<W3dRGBAStruct>>,
    pub per_pass_dig_colors: Vec<Vec<W3dRGBAStruct>>,
    pub vertex_materials: Vec<W3dVertexMaterialStruct>,
    pub shaders: Vec<W3dShaderStruct>,
    pub vertex_influences: Option<Vec<W3dVertInfStruct>>,
    pub vertex_shade_indices: Option<Vec<u32>>,
    pub per_stage_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    pub stage_uv_channels: Vec<u8>,
    pub texture_library: Vec<String>,
    pub vertex_mappers: Vec<VertexMapperConfig>,
    pub vertices_in_render_space: bool,
    pub has_explicit_vertex_colors: bool,
}

impl W3DMesh {
    pub fn new(name: String) -> Self {
        Self {
            name,
            container_name: String::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            material: W3DMaterial::default(),
            transform: Mat4::IDENTITY,
            header: None,
            stage_texcoords: Vec::new(),
            passes: Vec::new(),
            per_pass_stage_texture_ids: Vec::new(),
            per_pass_stage_texture_names: Vec::new(),
            per_pass_vertex_material_ids: Vec::new(),
            per_pass_shader_ids: Vec::new(),
            per_pass_dcg_colors: Vec::new(),
            per_pass_dig_colors: Vec::new(),
            vertex_materials: Vec::new(),
            shaders: Vec::new(),
            vertex_influences: None,
            vertex_shade_indices: None,
            per_stage_face_texcoord_ids: Vec::new(),
            stage_uv_channels: Vec::new(),
            texture_library: Vec::new(),
            vertex_mappers: Vec::new(),
            vertices_in_render_space: false,
            has_explicit_vertex_colors: false,
        }
    }

    pub fn texture_name_from_library(&self, texture_id: u32) -> Option<&str> {
        if texture_id == u32::MAX {
            return None;
        }
        self.texture_library
            .get(texture_id as usize)
            .map(|name| name.as_str())
            .filter(|name| !name.is_empty())
    }

    /// Whether this source mesh has a complete, safe skin declaration for an
    /// HMODEL palette of `palette_len` pivots.
    ///
    /// C++ `MeshGeometryClass::read_vertex_influences` reads exactly one
    /// `W3dVertInfStruct` per vertex and sets `SKIN` only after that succeeds.
    /// Until Main's importer retains that exact chunk, an HMODEL `SKIN_NODE`
    /// must stay absent rather than drawing the mesh with a guessed rigid
    /// transform or palette. Every influence must also address the HMODEL's
    /// own palette; a foreign/out-of-range bone is not recoverable safely.
    pub fn has_complete_skin_influences_for_palette(&self, palette_len: usize) -> bool {
        if palette_len == 0 || self.vertices.is_empty() {
            return false;
        }
        let Some(influences) = self.vertex_influences.as_ref() else {
            return false;
        };
        influences.len() == self.vertices.len()
            && influences
                .iter()
                .all(|influence| usize::from(influence.bone_idx) < palette_len)
    }

    pub fn stage_texture_names_from_ids(
        &self,
        pass_index: usize,
        stage_index: usize,
    ) -> Vec<String> {
        self.per_pass_stage_texture_ids
            .get(pass_index)
            .and_then(|stages| stages.get(stage_index))
            .map(|ids| {
                ids.iter()
                    .filter_map(|tex_id| self.texture_name_from_library(*tex_id))
                    .map(|name| name.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// Complete W3D model
#[derive(Debug, Clone)]
pub struct W3DModel {
    pub name: String,
    pub meshes: Vec<W3DMesh>,
    pub materials: HashMap<String, W3DMaterial>,
    pub texture_names: Vec<String>,
    pub ww3d_mesh_models: HashMap<String, Arc<MeshModelClass>>,
    pub bounding_box_min: Vec3,
    pub bounding_box_max: Vec3,
    pub hierarchy: Option<W3dHierarchy>,
    /// Every source HTree retained from this exact W3D file in C++ load
    /// order. `hierarchy` remains the legacy whole-model selection used by
    /// existing draw paths; HMODEL definitions resolve their explicitly named
    /// HTree from this source set instead of borrowing that convenience field.
    pub hierarchies: Vec<W3dHierarchy>,
    /// Source-authored HLOD records. Main supports the C++ constructor-time
    /// static level for one rigid HLOD. Multiple independent HLODs remain
    /// non-rendering rather than flattening every group into one visible
    /// model; external aggregates resolve independently and proxies remain
    /// retained non-rendering application metadata.
    pub hlods: Vec<W3dHlod>,
    /// Source HMODEL definitions registered as their own C++ render-object
    /// prototypes. They must not be flattened into `meshes`: each instance
    /// owns the HTree named by its definition and attaches its node records at
    /// their authored pivots.
    pub hmodels: Vec<W3dHmodel>,
    /// `W3D_CHUNK_EMITTER` prototypes (`ParticleEmitterLoaderClass`).
    pub emitters: Vec<super::w3d_emitter_loader::W3dEmitterProto>,
    /// `W3D_CHUNK_DAZZLE` prototypes (`DazzleLoaderClass`).
    pub dazzles: Vec<super::w3d_dazzle_loader::W3dDazzleProto>,
    /// `W3D_CHUNK_BOX` prototypes, including hidden BOUNDINGBOX OBBOX.
    pub boxes: Vec<super::w3d_primitive_protos::W3dBoxProto>,
    /// `W3D_CHUNK_RING` prototypes.
    pub rings: Vec<super::w3d_primitive_protos::W3dRingProto>,
    /// `W3D_CHUNK_SPHERE` prototypes.
    pub spheres: Vec<super::w3d_primitive_protos::W3dSphereProto>,
    /// `W3D_CHUNK_NULL_OBJECT` prototypes.
    pub nulls: Vec<super::w3d_primitive_protos::W3dNullProto>,
    /// `W3D_CHUNK_COLLECTION` prototypes.
    pub collections: Vec<super::w3d_collection_aggregate::W3dCollectionProto>,
    /// `W3D_CHUNK_AGGREGATE` prototypes.
    pub aggregates: Vec<super::w3d_collection_aggregate::W3dAggregateProto>,
    /// `W3D_CHUNK_LODMODEL` DistLOD prototypes.
    pub dist_lods: Vec<super::w3d_collection_aggregate::W3dDistLodProto>,
    /// A malformed HLOD must not silently fall back to generic mesh rendering:
    /// that would falsely claim a usable hierarchy/binding relationship.
    pub hlod_parse_failed: bool,
    pub animations: Vec<W3dAnimation>,
}
