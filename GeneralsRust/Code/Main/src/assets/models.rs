////////////////////////////////////////////////////////////////////////////////
//                                                                            //
//  (c) 2001-2003 Electronic Arts Inc.                                       //
//                                                                            //
////////////////////////////////////////////////////////////////////////////////

// W3D model loading system for real C&C 3D assets

use crate::assets::archive::ArchiveFileSystem;
use anyhow::{anyhow, Result};
use crc32fast::Hasher;
use glam::{Mat4, Vec3};
use log::{debug, info, warn};
use std::collections::HashMap;
use std::sync::Arc;
use ww3d_assets::prototypes::{MaterialPassInfo, VertexMapperConfig};
use ww3d_core::w3d_format::{
    w3d_string_from_bytes, W3dMeshHeader3Struct, W3dRGBAStruct, W3dShaderStruct, W3dVertInfStruct,
    W3dVertexMaterialStruct,
};
use ww3d_renderer_3d::rendering::mesh_system::MeshModelClass;

/// W3D file format constants based on C++ w3d_file.h
const W3D_CHUNK_MESH: u32 = 0x00000000;
const W3D_CHUNK_MESH_HEADER: u32 = 0x0000001F; // W3dMeshHeader3Struct
const W3D_CHUNK_VERTICES: u32 = 0x00000002;
const W3D_CHUNK_VERTEX_NORMALS: u32 = 0x00000003;
const W3D_CHUNK_MESH_USER_TEXT: u32 = 0x0000000C;
const W3D_CHUNK_VERTEX_INFLUENCES: u32 = 0x0000000E;
const W3D_CHUNK_TRIANGLES: u32 = 0x00000020;
const W3D_CHUNK_VERTEX_SHADE_INDICES: u32 = 0x00000022;
const W3D_CHUNK_MATERIAL_INFO: u32 = 0x00000028;
const W3D_CHUNK_SHADERS: u32 = 0x00000029;
const W3D_CHUNK_VERTEX_MATERIALS: u32 = 0x0000002A;
const W3D_CHUNK_VERTEX_MATERIAL: u32 = 0x0000002B;
const W3D_CHUNK_VERTEX_MATERIAL_NAME: u32 = 0x0000002C;
const W3D_CHUNK_VERTEX_MATERIAL_INFO: u32 = 0x0000002D;
const W3D_CHUNK_VERTEX_MAPPER_ARGS0: u32 = 0x0000002E;
const W3D_CHUNK_VERTEX_MAPPER_ARGS1: u32 = 0x0000002F;
// Obsolete v3 material chunks from w3d_obsolete.h (still used by shipped content).
const W3D_CHUNK_MATERIALS3: u32 = 0x00000015;
const W3D_CHUNK_MATERIAL3: u32 = 0x00000016;
const W3D_CHUNK_MATERIAL3_NAME: u32 = 0x00000017;
const W3D_CHUNK_MATERIAL3_INFO: u32 = 0x00000018;
const W3D_CHUNK_MATERIAL3_DC_MAP: u32 = 0x00000019;
const W3D_CHUNK_MAP3_FILENAME: u32 = 0x0000001A;
const W3D_CHUNK_MAP3_INFO: u32 = 0x0000001B;
const W3D_CHUNK_TEXTURES: u32 = 0x00000030; // FIXED: Was 0x32
const W3D_CHUNK_TEXTURE: u32 = 0x00000031; // FIXED: Was 0x33
const W3D_CHUNK_TEXTURE_NAME: u32 = 0x00000032; // FIXED: Was 0x34
const W3D_CHUNK_TEXTURE_INFO: u32 = 0x00000033; // FIXED: Was 0x35
const W3D_CHUNK_MATERIAL_PASS: u32 = 0x00000038;
const W3D_CHUNK_VERTEX_MATERIAL_IDS: u32 = 0x00000039;
const W3D_CHUNK_SHADER_IDS: u32 = 0x0000003A;
const W3D_CHUNK_DCG: u32 = 0x0000003B;
const W3D_CHUNK_DIG: u32 = 0x0000003C;
const W3D_CHUNK_TEXTURE_STAGE: u32 = 0x00000048;
const W3D_CHUNK_TEXTURE_IDS: u32 = 0x00000049; // NEW: Texture index array
const W3D_CHUNK_STAGE_TEXCOORDS: u32 = 0x0000004A;
const W3D_CHUNK_PER_FACE_TEXCOORD_IDS: u32 = 0x0000004B;

// Additional W3D chunks
const W3D_CHUNK_VERTEX_COLORS: u32 = 0x00000008;
const W3D_CHUNK_TEXCOORDS: u32 = 0x00000005;
const W3D_CHUNK_MATERIALS: u32 = 0x00000028;
const W3D_CHUNK_HIERARCHY: u32 = 0x00000100;
const W3D_CHUNK_ANIMATION: u32 = 0x00000200;
const W3D_CHUNK_HMODEL: u32 = 0x00000300;
const W3D_CHUNK_LODMODEL: u32 = 0x00000400;
const W3D_CHUNK_HLOD: u32 = 0x00000700; // NEW: Hierarchical LOD model

// Hierarchy sub-chunk types
const W3D_CHUNK_HIERARCHY_HEADER: u32 = 0x00000101;
const W3D_CHUNK_PIVOTS: u32 = 0x00000102;
const W3D_CHUNK_PIVOT_FIXUPS: u32 = 0x00000103;

// Animation sub-chunk types
const W3D_CHUNK_ANIMATION_HEADER: u32 = 0x00000201;
const W3D_CHUNK_ANIMATION_CHANNEL: u32 = 0x00000202;
const W3D_CHUNK_BIT_CHANNEL: u32 = 0x00000203;

// Compressed animation chunk types (timecoded and adaptive delta)
const W3D_CHUNK_COMPRESSED_ANIMATION: u32 = 0x00000280;
const W3D_CHUNK_COMPRESSED_ANIMATION_HEADER: u32 = 0x00000281;
const W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL: u32 = 0x00000282;
const W3D_CHUNK_COMPRESSED_BIT_CHANNEL: u32 = 0x00000283;

// Compressed animation flavor constants — C++ ANIM_FLAVOR_*
const ANIM_FLAVOR_TIMECODED: u16 = 0;
const ANIM_FLAVOR_ADAPTIVE_DELTA: u16 = 1;

/// W3D fixed-length name size (matches C++ W3D_NAME_LEN = 16)
const W3D_NAME_LEN: usize = 16;

/// Bone pivot from W3D hierarchy chunk. C++ parity: W3dPivotStruct (w3d_file.h:1322)
#[derive(Debug, Clone)]
pub struct W3dPivot {
    pub name: String,
    pub parent_idx: u32, // 0xFFFFFFFF = root
    pub translation: [f32; 3],
    pub euler_angles: [f32; 3],
    pub rotation: [f32; 4], // Quaternion [x,y,z,w]
}

/// W3D hierarchy data. C++ parity: W3dHierarchyStruct + W3dPivotStruct array
#[derive(Debug, Clone)]
pub struct W3dHierarchy {
    pub name: String,
    pub pivots: Vec<W3dPivot>,
    pub pivot_fixups: Vec<[[f32; 3]; 4]>,
}

/// Animation channel targeting a specific bone. C++ parity: animation channel data
#[derive(Debug, Clone)]
pub struct W3dAnimChannel {
    pub first_frame: u16,
    pub last_frame: u16,
    pub vector_len: u16, // 1 for scalar (X/Y/Z), 4 for quaternion
    pub flags: u16,      // 0=X, 1=Y, 2=Z, 6=Q
    pub pivot: u16,      // Bone index
    pub data: Vec<f32>,
}

/// W3D animation data. C++ parity: W3dAnimHeaderStruct + channels
#[derive(Debug, Clone)]
pub struct W3dAnimation {
    pub name: String,
    pub hierarchy_name: String,
    pub num_frames: u32,
    pub frame_rate: u32,
    pub channels: Vec<W3dAnimChannel>,
}

#[derive(Debug, Default)]
struct ParsedTextureStage {
    texture_ids: Vec<u32>,
    texcoords: Vec<[f32; 2]>,
    per_face_texcoord_ids: Vec<[u32; 3]>,
}

#[derive(Debug, Default)]
struct ParsedMaterialPass {
    stage_texture_ids: Vec<Vec<u32>>,
    stage_texcoords: Vec<Vec<[f32; 2]>>,
    stage_per_face_texcoord_ids: Vec<Vec<[u32; 3]>>,
    vertex_material_ids: Vec<u32>,
    shader_ids: Vec<u32>,
    dcg_colors: Vec<W3dRGBAStruct>,
    dig_colors: Vec<W3dRGBAStruct>,
}

// Mesh types
const W3D_MESH_FLAG_NONE: u32 = 0;
const W3D_MESH_FLAG_HIDDEN: u32 = 0x00000001;
const W3D_MESH_FLAG_TWO_SIDED: u32 = 0x00000002;
const W3D_MESH_FLAG_CAST_SHADOW: u32 = 0x00000004;
const W3D_MESH_FLAG_GEOMETRY_TYPE_MASK: u32 = 0x00FF0000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_NORMAL: u32 = 0x00000000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_CAMERA_ALIGNED: u32 = 0x00010000;
const W3D_MESH_FLAG_GEOMETRY_TYPE_SKIN: u32 = 0x00020000;

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
fn shader_blend_to_mode(src_blend: u8, dest_blend: u8, alpha_test: u8) -> (BlendMode, bool) {
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

fn w3d_position_to_world(position: [f32; 3]) -> [f32; 3] {
    // Legacy W3D content is authored in X/Y ground with Z-up. The active Rust world
    // uses X/Z ground with Y-up, so swap the vertical and depth axes on import.
    [position[0], position[2], position[1]]
}

fn w3d_normal_to_world(normal: [f32; 3]) -> [f32; 3] {
    [normal[0], normal[2], normal[1]]
}

fn push_world_space_triangle(indices: &mut Vec<u32>, a: u32, b: u32, c: u32) {
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
    pub animations: Vec<W3dAnimation>,
}

impl W3DModel {
    pub fn new(name: String) -> Self {
        Self {
            name,
            meshes: Vec::new(),
            materials: HashMap::new(),
            texture_names: Vec::new(),
            ww3d_mesh_models: HashMap::new(),
            bounding_box_min: Vec3::splat(f32::MAX),
            bounding_box_max: Vec3::splat(f32::MIN),
            hierarchy: None,
            animations: Vec::new(),
        }
    }

    pub fn calculate_bounding_box(&mut self) {
        self.bounding_box_min = Vec3::splat(f32::MAX);
        self.bounding_box_max = Vec3::splat(f32::MIN);

        for mesh in &self.meshes {
            for vertex in &mesh.vertices {
                let pos = Vec3::from_array(vertex.position);
                self.bounding_box_min = self.bounding_box_min.min(pos);
                self.bounding_box_max = self.bounding_box_max.max(pos);
            }
        }
    }

    /// Get the list of animation names available on this model.
    pub fn animation_names(&self) -> Vec<&str> {
        self.animations.iter().map(|a| a.name.as_str()).collect()
    }

    /// Find an animation index by name (case-insensitive).
    pub fn find_animation_index(&self, name: &str) -> Option<usize> {
        let lower = name.to_ascii_lowercase();
        self.animations
            .iter()
            .position(|a| a.name.to_ascii_lowercase() == lower)
    }

    /// Get animation metadata: (num_frames, frame_rate) for the given animation.
    pub fn animation_metadata(&self, anim_index: usize) -> Option<(u32, u32)> {
        let anim = self.animations.get(anim_index)?;
        Some((anim.num_frames, anim.frame_rate))
    }

    /// Sample an animation at the given frame, producing per-bone global transforms.
    ///
    /// Returns a Vec of column-major 4x4 matrices indexed by pivot (bone) index,
    /// or `None` if the animation or hierarchy is missing.
    ///
    /// The frame parameter is a continuous value; fractional parts interpolate
    /// between adjacent keyframes.
    pub fn sample_animation(&self, anim_index: usize, frame: f32) -> Option<Vec<[f32; 16]>> {
        let anim = self.animations.get(anim_index)?;
        let hierarchy = self.hierarchy.as_ref()?;

        let mut local_transforms: Vec<[f32; 16]> = hierarchy
            .pivots
            .iter()
            .map(|p| mat4_from_pivot(p))
            .collect();

        for channel in &anim.channels {
            let pivot_idx = channel.pivot as usize;
            if pivot_idx >= local_transforms.len() {
                continue;
            }

            let values = sample_channel(channel, frame);

            match channel.flags {
                0 => {
                    if let Some(v) = values.first() {
                        local_transforms[pivot_idx][12] = *v;
                    }
                }
                1 => {
                    if let Some(v) = values.first() {
                        local_transforms[pivot_idx][13] = *v;
                    }
                }
                2 => {
                    if let Some(v) = values.first() {
                        local_transforms[pivot_idx][14] = *v;
                    }
                }
                6 => {
                    if values.len() >= 4 {
                        let qx = values[0];
                        let qy = values[1];
                        let qz = values[2];
                        let qw = values[3];
                        apply_quat_to_transform(&mut local_transforms[pivot_idx], qx, qy, qz, qw);
                    }
                }
                _ => {}
            }
        }

        Some(compute_global_transforms_from_locals(
            hierarchy,
            &local_transforms,
        ))
    }
}

/// Build a column-major 4x4 matrix from a pivot's translation + quaternion rotation.
/// Same logic as W3DLoader::mat4_from_tr_quat but operates on W3dPivot directly.
fn mat4_from_pivot(pivot: &W3dPivot) -> [f32; 16] {
    let x = pivot.rotation[0];
    let y = pivot.rotation[1];
    let z = pivot.rotation[2];
    let w = pivot.rotation[3];
    let xx = x * x;
    let yy = y * y;
    let zz = z * z;
    let xy = x * y;
    let xz = x * z;
    let yz = y * z;
    let wx = w * x;
    let wy = w * y;
    let wz = w * z;
    let m00 = 1.0 - 2.0 * (yy + zz);
    let m01 = 2.0 * (xy - wz);
    let m02 = 2.0 * (xz + wy);
    let m10 = 2.0 * (xy + wz);
    let m11 = 1.0 - 2.0 * (xx + zz);
    let m12 = 2.0 * (yz - wx);
    let m20 = 2.0 * (xz - wy);
    let m21 = 2.0 * (yz + wx);
    let m22 = 1.0 - 2.0 * (xx + yy);
    let tx = pivot.translation[0];
    let ty = pivot.translation[1];
    let tz = pivot.translation[2];
    [
        m00, m10, m20, 0.0, m01, m11, m21, 0.0, m02, m12, m22, 0.0, tx, ty, tz, 1.0,
    ]
}

/// Replace the 3×3 rotation part of a column-major 4x4 matrix with the given quaternion.
/// Translation is preserved.
fn apply_quat_to_transform(m: &mut [f32; 16], qx: f32, qy: f32, qz: f32, qw: f32) {
    let xx = qx * qx;
    let yy = qy * qy;
    let zz = qz * qz;
    let xy = qx * qy;
    let xz = qx * qz;
    let yz = qy * qz;
    let wx = qw * qx;
    let wy = qw * qy;
    let wz = qw * qz;
    m[0] = 1.0 - 2.0 * (yy + zz); // col0 row0
    m[1] = 2.0 * (xy + wz); // col0 row1
    m[2] = 2.0 * (xz - wy); // col0 row2
    m[4] = 2.0 * (xy - wz); // col1 row0
    m[5] = 1.0 - 2.0 * (xx + zz); // col1 row1
    m[6] = 2.0 * (yz + wx); // col1 row2
    m[8] = 2.0 * (xz + wy); // col2 row0
    m[9] = 2.0 * (yz - wx); // col2 row1
    m[10] = 1.0 - 2.0 * (xx + yy); // col2 row2
}

/// Interpolate an animation channel at the given continuous frame value.
/// Returns the interpolated values (1 for scalar channels, 4 for quaternion).
fn sample_channel(channel: &W3dAnimChannel, frame: f32) -> Vec<f32> {
    let first = channel.first_frame as f32;
    let last = channel.last_frame as f32;

    // Clamp frame to channel range
    let t = (frame - first).max(0.0).min((last - first).max(0.0));

    let vl = channel.vector_len as usize;
    if vl == 0 || channel.data.is_empty() {
        return Vec::new();
    }

    // Number of keyframes in this channel
    let num_keys = channel.data.len() / vl;
    if num_keys == 0 {
        return vec![0.0; vl];
    }

    let frame_idx = (t as usize).min(num_keys - 1);
    let frac = t - frame_idx as f32;

    let idx0 = frame_idx * vl;
    let idx1 = if frame_idx + 1 < num_keys {
        (frame_idx + 1) * vl
    } else {
        idx0
    };

    if idx0 + vl > channel.data.len() {
        return vec![0.0; vl];
    }

    // Linear interpolation between adjacent keyframes
    let mut result = Vec::with_capacity(vl);
    for i in 0..vl {
        let a = channel.data[idx0 + i];
        let b = if idx1 + i < channel.data.len() {
            channel.data[idx1 + i]
        } else {
            a
        };
        result.push(a + (b - a) * frac);
    }

    // For quaternion channels (flags=6), normalize to unit quaternion
    if channel.flags == 6 && result.len() == 4 {
        let len = (result[0] * result[0]
            + result[1] * result[1]
            + result[2] * result[2]
            + result[3] * result[3])
            .sqrt();
        if len > 1e-10 {
            for v in result.iter_mut() {
                *v /= len;
            }
        }
    }

    result
}

/// Column-major 4x4 matrix multiply.
fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
    let mut r = [0.0; 16];
    for i in 0..4 {
        for j in 0..4 {
            r[i * 4 + j] = a[i * 4 + 0] * b[0 * 4 + j]
                + a[i * 4 + 1] * b[1 * 4 + j]
                + a[i * 4 + 2] * b[2 * 4 + j]
                + a[i * 4 + 3] * b[3 * 4 + j];
        }
    }
    r
}

/// Compute global bone transforms from local transforms by walking the parent chain.
fn compute_global_transforms_from_locals(
    hierarchy: &W3dHierarchy,
    locals: &[[f32; 16]],
) -> Vec<[f32; 16]> {
    let n = hierarchy.pivots.len();
    let mut globals: Vec<[f32; 16]> = vec![[0.0; 16]; n];
    for i in 0..n {
        let local = locals[i];
        let g = if hierarchy.pivots[i].parent_idx != 0xFFFF_FFFF {
            let p = hierarchy.pivots[i].parent_idx as usize;
            mat4_mul(&globals[p], &local)
        } else {
            local
        };
        globals[i] = g;
    }
    globals
}

/// W3D model loader
pub struct W3DLoader;

impl Default for W3DLoader {
    fn default() -> Self {
        Self::new()
    }
}

impl W3DLoader {
    /// Create new W3D loader
    pub fn new() -> Self {
        Self
    }

    /// Load W3D model from BIG archive
    pub async fn load_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        model_name: &str,
    ) -> Result<W3DModel> {
        debug!("Loading W3D model: {}", model_name);

        // C++ parity: deterministic model lookup (requested file and canonical Art/W3D location).
        let base_name = model_name
            .rsplit(['/', '\\'])
            .next()
            .unwrap_or(model_name)
            .trim()
            .trim_end_matches(".w3d")
            .trim_end_matches(".W3D");
        let w3d_filename = format!("{base_name}.w3d");
        let path_variations = [format!("art/w3d/{w3d_filename}"), w3d_filename.clone()];

        let mut last_error = None;
        for path_variant in path_variations {
            debug!("Trying W3D path: {}", path_variant);
            match archive_system.open_file(&path_variant).await {
                Ok(model_data) => {
                    debug!("Found W3D file at path: {}", path_variant);
                    debug!("Loaded W3D file data: {} bytes", model_data.len());
                    return self.parse_w3d_data(&model_data, base_name.to_string());
                }
                Err(e) => {
                    debug!("Failed to find W3D at {}: {}", path_variant, e);
                    last_error = Some(e);
                }
            }
        }

        Err(anyhow!(
            "Failed to load W3D file {}: {}",
            w3d_filename,
            last_error.unwrap_or_else(|| anyhow!("file not found"))
        ))
    }

    /// Parse W3D binary data using the legacy chunk parser path for strict C++ parity.
    fn parse_w3d_data(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        self.parse_w3d_data_legacy(data, model_name)
    }

    // Non-parity companion/heuristic model-family merge path removed.
    // The active parser path is strict legacy chunk parsing (`parse_w3d_data_legacy`).

    // Non-parity companion source loading and alternate ww3d-assets parsing entrypoints removed.

    fn stage_channel_to_uv_source(channel: u8) -> UVSource {
        match channel {
            0 => UVSource::UV0,
            1 => UVSource::UV1,
            2 => UVSource::UV2,
            _ => UVSource::UV3,
        }
    }

    fn stage_mapping_mut<'a>(
        material: &'a mut W3DMaterial,
        stage: usize,
        create: bool,
    ) -> Option<&'a mut TextureStageMapping> {
        match stage {
            0 => Some(&mut material.stage0_mapping),
            1 => {
                if material.stage1_mapping.is_none() && create {
                    material.stage1_mapping = Some(TextureStageMapping::default());
                }
                material.stage1_mapping.as_mut()
            }
            2 => {
                if material.stage2_mapping.is_none() && create {
                    material.stage2_mapping = Some(TextureStageMapping::default());
                }
                material.stage2_mapping.as_mut()
            }
            3 => {
                if material.stage3_mapping.is_none() && create {
                    material.stage3_mapping = Some(TextureStageMapping::default());
                }
                material.stage3_mapping.as_mut()
            }
            _ => None,
        }
    }

    fn apply_material_stage_mappings(material: &mut W3DMaterial, mesh: &W3DMesh) {
        for stage_idx in 0..4 {
            let create = stage_idx == 0
                || mesh.stage_uv_channels.get(stage_idx).is_some()
                || Self::stage_texture_from_mesh(mesh, 0, stage_idx).is_some();

            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, create) {
                if mapping.texture_name.is_none() {
                    if let Some(name) = Self::stage_texture_from_mesh(mesh, 0, stage_idx) {
                        mapping.texture_name = Some(name);
                    }
                }
            }
        }

        for (stage_idx, &channel) in mesh.stage_uv_channels.iter().enumerate().take(4) {
            if let Some(mapping) = Self::stage_mapping_mut(material, stage_idx, true) {
                mapping.uv_source = Self::stage_channel_to_uv_source(channel);
            }
        }

        // Match the common C++ material path more closely: once stage 0 resolves to a texture,
        // expose that as the primary material texture too so caches/debugging/legacy consumers
        // don't diverge from the active pass state.
        if material.texture_name.is_none() {
            material.texture_name = material.stage0_mapping.texture_name.clone();
        }
    }

    fn stage_texture_from_mesh(
        mesh: &W3DMesh,
        pass_index: usize,
        stage_index: usize,
    ) -> Option<String> {
        if let Some(stage_sets) = mesh.per_pass_stage_texture_names.get(pass_index) {
            if let Some(names) = stage_sets.get(stage_index) {
                if let Some(name) = names.iter().find(|n| !n.is_empty()) {
                    return Some(name.clone());
                }
            }
        }

        mesh.stage_texture_names_from_ids(pass_index, stage_index)
            .into_iter()
            .find(|name| !name.is_empty())
    }

    /// Parse W3D binary data using the legacy chunk parser (fallback path)
    fn parse_w3d_data_legacy(&self, data: &[u8], model_name: String) -> Result<W3DModel> {
        if data.len() < 8 {
            return Err(anyhow!("W3D file too small: {} bytes", data.len()));
        }

        let mut model = W3DModel::new(model_name);
        let mut offset = 0usize;

        // Parse W3D chunks with safety counter to prevent infinite loops
        let mut chunk_counter = 0;
        const MAX_CHUNKS: usize = 10000; // Safety limit to prevent infinite loops

        while offset + 8 <= data.len() && chunk_counter < MAX_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Handle W3D chunk size format: MSB indicates container chunk
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize; // Clear MSB to get actual size

            debug!(
                "W3D chunk: type=0x{:08X}, raw_size=0x{:08X}, size={}, container={}",
                chunk_type, raw_chunk_size, chunk_size, is_container_chunk
            );

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "W3D chunk extends beyond file: type 0x{:08X}, size {} (raw: 0x{:08X})",
                    chunk_type, chunk_size, raw_chunk_size
                );
                break;
            }

            // Additional safety checks to prevent infinite loops
            if chunk_size == 0 {
                warn!(
                    "Zero-sized chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8; // Skip just the header
                continue;
            }

            if chunk_size > data.len() {
                warn!(
                    "Chunk size {} exceeds total file size {} - aborting parsing",
                    chunk_size,
                    data.len()
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Parsing W3D mesh chunk, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse W3D mesh chunk");
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!("Parsing W3D hierarchy chunk, size: {}", chunk_size);
                    match self.parse_hierarchy_chunk(chunk_data) {
                        Ok(hierarchy) => {
                            debug!(
                                "Parsed hierarchy '{}' with {} pivots",
                                hierarchy.name,
                                hierarchy.pivots.len()
                            );
                            model.hierarchy = Some(hierarchy);
                        }
                        Err(e) => warn!("Failed to parse hierarchy chunk: {}", e),
                    }
                    if is_container_chunk {
                        self.parse_container_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_MATERIALS3 => {
                    debug!("Parsing W3D materials3 container, size: {}", chunk_size);
                    // Parse materials3 container - contains material definitions with texture names
                    if is_container_chunk {
                        self.parse_materials3_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Parsing W3D textures container, size: {}", chunk_size);
                    // Parse textures container - contains individual texture definitions
                    if is_container_chunk {
                        self.parse_textures_chunk(chunk_data, &mut model)?;
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Parsing W3D animation chunk, size: {}", chunk_size);
                    match self.parse_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            debug!(
                                "Parsed animation '{}' ({} frames @ {}fps)",
                                animation.name, animation.num_frames, animation.frame_rate
                            );
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse animation chunk: {}", e),
                    }
                }
                W3D_CHUNK_COMPRESSED_ANIMATION => {
                    debug!(
                        "Parsing W3D compressed animation chunk (timecoded/adaptive delta), size: {}",
                        chunk_size
                    );
                    match self.parse_compressed_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            debug!(
                                "Parsed compressed animation '{}' ({} frames @ {}fps)",
                                animation.name, animation.num_frames, animation.frame_rate
                            );
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse compressed animation chunk: {}", e),
                    }
                }
                W3D_CHUNK_HMODEL => {
                    debug!("Found W3D hierarchical model chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse hierarchical model container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_LODMODEL => {
                    debug!("Found W3D LOD model chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse LOD model container: {}", e);
                        }
                    }
                }
                W3D_CHUNK_HLOD => {
                    debug!(
                        "Found W3D HLOD (Hierarchical LOD) chunk, size: {}",
                        chunk_size
                    );
                    // HLOD is a container chunk with hierarchical models and LOD info
                    // Recursively parse to find mesh data
                    if is_container_chunk {
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!("Failed to parse HLOD container: {}", e);
                        }
                    }
                }
                _ => {
                    debug!("Unknown W3D chunk type: 0x{:08X}", chunk_type);
                    // If it's a container chunk, try to parse it recursively
                    if is_container_chunk && chunk_size > 0 {
                        debug!("  -> Container chunk, parsing recursively");
                        if let Err(e) = self.parse_container_chunk(chunk_data, &mut model) {
                            warn!(
                                "Failed to parse container chunk 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CHUNKS {
            warn!(
                "⚠️  W3D chunk parsing hit safety limit ({} chunks) - possible malformed file",
                MAX_CHUNKS
            );
        }

        if model.meshes.is_empty() {
            return Err(anyhow!(
                "legacy parser: no valid meshes found in '{}'",
                model.name
            ));
        }

        // Post-process: Resolve texture indices to actual texture names from W3D_CHUNK_TEXTURES
        // This matches C++ behavior where W3D_CHUNK_MAP3_FILENAME contains texture indices
        // that need to be resolved against the texture_names array
        self.resolve_texture_indices(&mut model);

        // Apply hierarchy transforms to mesh vertices.
        // Vertices were already converted to Y-up in build_mesh_from_data, so we undo that,
        // apply hierarchy transform in W3D Z-up space, then re-convert to Y-up.
        if let Some(ref hierarchy) = model.hierarchy {
            if !hierarchy.pivots.is_empty() {
                let globals = Self::compute_global_transforms(hierarchy);
                for mesh in &mut model.meshes {
                    if mesh.transform != Mat4::IDENTITY {
                        continue;
                    }
                    if let Some(pivot_idx) =
                        hierarchy.pivots.iter().position(|p| p.name == mesh.name)
                    {
                        let global = &globals[pivot_idx];
                        let mat = Mat4::from_cols_array(global);
                        mesh.transform = mat;
                        for vertex in &mut mesh.vertices {
                            // Undo Y-up → Z-up, apply hierarchy, convert Z-up → Y-up
                            let zup_pos =
                                [vertex.position[0], vertex.position[2], vertex.position[1]];
                            let pos = mat.transform_point3(Vec3::from_array(zup_pos));
                            vertex.position = [pos.x, pos.z, pos.y]; // Z-up → Y-up
                            let zup_norm = [vertex.normal[0], vertex.normal[2], vertex.normal[1]];
                            let nrm = mat.transform_point3(Vec3::from_array(zup_norm));
                            let len = nrm.length();
                            if len > 1e-10 {
                                vertex.normal = [nrm.x / len, nrm.z / len, nrm.y / len];
                            }
                        }
                    }
                }
            }
        }

        model.calculate_bounding_box();
        Ok(model)
    }

    /// Parse a W3D container chunk recursively
    fn parse_container_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut chunk_counter = 0;
        const MAX_CONTAINER_CHUNKS: usize = 5000; // Safety limit for container chunks

        while offset + 8 <= data.len() && chunk_counter < MAX_CONTAINER_CHUNKS {
            chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Container sub-chunk extends beyond container: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for container chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized container chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH => {
                    debug!("Found mesh chunk in container, size: {}", chunk_size);
                    if let Ok(mut mesh) = self.parse_mesh_chunk(chunk_data) {
                        if mesh.texture_library.is_empty() && !model.texture_names.is_empty() {
                            mesh.texture_library = model.texture_names.clone();
                        }
                        model.meshes.push(mesh);
                    } else {
                        warn!("Failed to parse mesh chunk in container");
                    }
                }
                W3D_CHUNK_TEXTURES => {
                    debug!("Found textures chunk in container, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Err(e) = self.parse_textures_chunk(chunk_data, model) {
                            warn!("Failed to parse textures chunk: {}", e);
                        }
                    }
                }
                W3D_CHUNK_ANIMATION => {
                    debug!("Found animation chunk in container, size: {}", chunk_size);
                    match self.parse_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            model.animations.push(animation);
                        }
                        Err(e) => warn!("Failed to parse animation chunk in container: {}", e),
                    }
                }
                W3D_CHUNK_COMPRESSED_ANIMATION => {
                    debug!(
                        "Found compressed animation chunk in container, size: {}",
                        chunk_size
                    );
                    match self.parse_compressed_animation_chunk(chunk_data) {
                        Ok(animation) => {
                            model.animations.push(animation);
                        }
                        Err(e) => {
                            warn!(
                                "Failed to parse compressed animation chunk in container: {}",
                                e
                            )
                        }
                    }
                }
                W3D_CHUNK_HIERARCHY => {
                    debug!("Found hierarchy chunk in container, size: {}", chunk_size);
                    match self.parse_hierarchy_chunk(chunk_data) {
                        Ok(hierarchy) => {
                            model.hierarchy = Some(hierarchy);
                        }
                        Err(e) => {
                            warn!("Failed to parse hierarchy chunk in container: {}", e)
                        }
                    }
                }
                _ => {
                    debug!(
                        "Container sub-chunk: type 0x{:08X}, size {}, container: {}",
                        chunk_type, chunk_size, is_container_chunk
                    );
                    // Recursively parse nested containers
                    if is_container_chunk && chunk_size > 0 {
                        if let Err(e) = self.parse_container_chunk(chunk_data, model) {
                            warn!(
                                "Failed to parse nested container 0x{:08X}: {}",
                                chunk_type, e
                            );
                        }
                    }
                }
            }

            offset += 8 + chunk_size;
        }

        if chunk_counter >= MAX_CONTAINER_CHUNKS {
            warn!(
                "⚠️  Container chunk parsing hit safety limit ({} chunks)",
                MAX_CONTAINER_CHUNKS
            );
        }

        Ok(())
    }

    /// Parse W3D hierarchy chunk (0x100) — ported from standalone parser's parse_hierarchy_chunk
    fn parse_hierarchy_chunk(&self, data: &[u8]) -> Result<W3dHierarchy> {
        let mut name = String::new();
        let mut num_pivots: u32 = 0;
        let mut pivots: Vec<W3dPivot> = Vec::new();
        let mut pivot_fixups: Vec<[[f32; 3]; 4]> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_HIERARCHY_HEADER => {
                    // W3dHierarchyHeader: version(u32) + name[16] + num_pivots(u32) + center[3]
                    if chunk_data.len() < 32 {
                        return Err(anyhow!("hierarchy header too small: {}", chunk_data.len()));
                    }
                    let _version = u32::from_le_bytes([
                        chunk_data[0],
                        chunk_data[1],
                        chunk_data[2],
                        chunk_data[3],
                    ]);
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    num_pivots = u32::from_le_bytes([
                        chunk_data[20],
                        chunk_data[21],
                        chunk_data[22],
                        chunk_data[23],
                    ]);
                    debug!(
                        "Hierarchy header: name='{}', num_pivots={}",
                        name, num_pivots
                    );
                }
                W3D_CHUNK_PIVOTS => {
                    // Each pivot: name[16] + parent_idx(u32) + translation[3] + euler[3] + quat[4]
                    // = 16 + 4 + 12 + 12 + 16 = 60 bytes
                    const PIVOT_SIZE: usize = 60;
                    let count = if num_pivots > 0 {
                        num_pivots as usize
                    } else {
                        chunk_size / PIVOT_SIZE
                    };
                    pivots.reserve(count);
                    for i in 0..count {
                        let base = i * PIVOT_SIZE;
                        if base + PIVOT_SIZE > chunk_data.len() {
                            break;
                        }
                        let d = &chunk_data[base..base + PIVOT_SIZE];
                        let mut pname = String::new();
                        for j in 0..W3D_NAME_LEN {
                            if d[j] == 0 {
                                break;
                            }
                            pname.push(d[j] as char);
                        }
                        let parent_idx = u32::from_le_bytes([d[16], d[17], d[18], d[19]]);
                        let tx = f32::from_le_bytes([d[20], d[21], d[22], d[23]]);
                        let ty = f32::from_le_bytes([d[24], d[25], d[26], d[27]]);
                        let tz = f32::from_le_bytes([d[28], d[29], d[30], d[31]]);
                        let ex = f32::from_le_bytes([d[32], d[33], d[34], d[35]]);
                        let ey = f32::from_le_bytes([d[36], d[37], d[38], d[39]]);
                        let ez = f32::from_le_bytes([d[40], d[41], d[42], d[43]]);
                        let qx = f32::from_le_bytes([d[44], d[45], d[46], d[47]]);
                        let qy = f32::from_le_bytes([d[48], d[49], d[50], d[51]]);
                        let qz = f32::from_le_bytes([d[52], d[53], d[54], d[55]]);
                        let qw = f32::from_le_bytes([d[56], d[57], d[58], d[59]]);
                        pivots.push(W3dPivot {
                            name: pname,
                            parent_idx,
                            translation: [tx, ty, tz],
                            euler_angles: [ex, ey, ez],
                            rotation: [qx, qy, qz, qw],
                        });
                    }
                }
                W3D_CHUNK_PIVOT_FIXUPS => {
                    // Each fixup: [[f32;3];4] = 48 bytes
                    const FIXUP_SIZE: usize = 48;
                    let count = if num_pivots > 0 {
                        num_pivots as usize
                    } else {
                        chunk_size / FIXUP_SIZE
                    };
                    pivot_fixups.reserve(count);
                    for i in 0..count {
                        let base = i * FIXUP_SIZE;
                        if base + FIXUP_SIZE > chunk_data.len() {
                            break;
                        }
                        let d = &chunk_data[base..base + FIXUP_SIZE];
                        let mut tm = [[0.0f32; 3]; 4];
                        for row in 0..4 {
                            for col in 0..3 {
                                let off = (row * 3 + col) * 4;
                                tm[row][col] = f32::from_le_bytes([
                                    d[off],
                                    d[off + 1],
                                    d[off + 2],
                                    d[off + 3],
                                ]);
                            }
                        }
                        pivot_fixups.push(tm);
                    }
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(W3dHierarchy {
            name,
            pivots,
            pivot_fixups,
        })
    }

    /// Parse W3D animation chunk (0x200) — ported from standalone parser's parse_animation_chunk
    fn parse_animation_chunk(&self, data: &[u8]) -> Result<W3dAnimation> {
        let mut name = String::new();
        let mut hierarchy_name = String::new();
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut channels: Vec<W3dAnimChannel> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_ANIMATION_HEADER => {
                    // version(u32) + name[16] + hierarchy_name[16] + num_frames(u32) + frame_rate(u32)
                    // = 4 + 16 + 16 + 4 + 4 = 44 bytes
                    if chunk_data.len() < 44 {
                        return Err(anyhow!("animation header too small: {}", chunk_data.len()));
                    }
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    let mut hn = String::new();
                    for i in 20..20 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        hn.push(chunk_data[i] as char);
                    }
                    hierarchy_name = hn;
                    num_frames = u32::from_le_bytes([
                        chunk_data[36],
                        chunk_data[37],
                        chunk_data[38],
                        chunk_data[39],
                    ]);
                    frame_rate = u32::from_le_bytes([
                        chunk_data[40],
                        chunk_data[41],
                        chunk_data[42],
                        chunk_data[43],
                    ]);
                    debug!(
                        "Animation header: name='{}', hierarchy='{}', frames={}, fps={}",
                        name, hierarchy_name, num_frames, frame_rate
                    );
                }
                W3D_CHUNK_ANIMATION_CHANNEL => {
                    // first_frame(u16) + last_frame(u16) + vector_len(u16) + flags(u16) + pivot(u16) + pad(u16)
                    // = 12 bytes header, rest is f32 data
                    if chunk_data.len() < 12 {
                        offset += 8 + chunk_size;
                        continue;
                    }
                    let first_frame = u16::from_le_bytes([chunk_data[0], chunk_data[1]]);
                    let last_frame = u16::from_le_bytes([chunk_data[2], chunk_data[3]]);
                    let vector_len = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
                    let flags = u16::from_le_bytes([chunk_data[6], chunk_data[7]]);
                    let pivot = u16::from_le_bytes([chunk_data[8], chunk_data[9]]);
                    let remaining = chunk_size.saturating_sub(12);
                    let count_f32 = remaining / 4;
                    let mut chan_data = Vec::with_capacity(count_f32);
                    for i in 0..count_f32 {
                        let off = 12 + i * 4;
                        if off + 4 > chunk_data.len() {
                            break;
                        }
                        chan_data.push(f32::from_le_bytes([
                            chunk_data[off],
                            chunk_data[off + 1],
                            chunk_data[off + 2],
                            chunk_data[off + 3],
                        ]));
                    }
                    channels.push(W3dAnimChannel {
                        first_frame,
                        last_frame,
                        vector_len,
                        flags,
                        pivot,
                        data: chan_data,
                    });
                }
                W3D_CHUNK_BIT_CHANNEL => {
                    // Skip bit channels for now — just consume the bytes
                    debug!("Skipping bit channel chunk, size: {}", chunk_size);
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(W3dAnimation {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            channels,
        })
    }

    /// Parse W3D compressed animation chunk (0x280) — handles both timecoded (flavor 0)
    /// and adaptive delta (flavor 1) sub-formats.
    /// C++ parity: W3D_CHUNK_COMPRESSED_ANIMATION container with CompressedAnimationHeader,
    /// CompressedAnimationChannel, and CompressedBitChannel sub-chunks.
    fn parse_compressed_animation_chunk(&self, data: &[u8]) -> Result<W3dAnimation> {
        let mut name = String::new();
        let mut hierarchy_name = String::new();
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut flavor: u16 = ANIM_FLAVOR_TIMECODED;
        let mut channels: Vec<W3dAnimChannel> = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_COMPRESSED_ANIMATION_HEADER => {
                    if chunk_data.len() < 44 {
                        return Err(anyhow!(
                            "compressed animation header too small: {}",
                            chunk_data.len()
                        ));
                    }
                    let mut n = String::new();
                    for i in 4..4 + W3D_NAME_LEN {
                        if chunk_data[i] == 0 {
                            break;
                        }
                        n.push(chunk_data[i] as char);
                    }
                    name = n;
                    let mut hn = String::new();
                    for i in 20..20 + W3D_NAME_LEN {
                        if i >= chunk_data.len() || chunk_data[i] == 0 {
                            break;
                        }
                        hn.push(chunk_data[i] as char);
                    }
                    hierarchy_name = hn;
                    num_frames = u32::from_le_bytes([
                        chunk_data[36],
                        chunk_data[37],
                        chunk_data[38],
                        chunk_data[39],
                    ]);
                    frame_rate = u16::from_le_bytes([chunk_data[40], chunk_data[41]]) as u32;
                    flavor = u16::from_le_bytes([chunk_data[42], chunk_data[43]]);
                    debug!(
                        "Compressed animation header: name='{}', hierarchy='{}', frames={}, fps={}, flavor={}",
                        name, hierarchy_name, num_frames, frame_rate, flavor
                    );
                }
                W3D_CHUNK_COMPRESSED_ANIMATION_CHANNEL => {
                    if chunk_data.len() < 8 {
                        offset += 8 + chunk_size;
                        continue;
                    }
                    if flavor == ANIM_FLAVOR_TIMECODED {
                        if let Some(ch) = Self::parse_timecoded_channel(chunk_data, num_frames) {
                            channels.push(ch);
                        }
                    } else {
                        let chs = Self::parse_adaptive_delta_channel(chunk_data, num_frames);
                        channels.extend(chs);
                    }
                }
                W3D_CHUNK_COMPRESSED_BIT_CHANNEL => {
                    debug!(
                        "Skipping compressed bit channel in compressed animation, size: {}",
                        chunk_size
                    );
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(W3dAnimation {
            name,
            hierarchy_name,
            num_frames,
            frame_rate,
            channels,
        })
    }

    /// Parse a single timecoded animation channel (ANIM_FLAVOR_TIMECODED, flavor=0).
    /// Format: num_timecodes(u32) + pivot(u16) + vector_len(u8) + flags(u8)
    ///         then [timecode(u32) + vector_len * f32] per timecode.
    /// Timecode MSB = binary (step) flag; lower 31 bits = frame index.
    /// Produces a densified per-frame W3dAnimChannel matching the uncompressed layout.
    fn parse_timecoded_channel(chunk_data: &[u8], num_frames: u32) -> Option<W3dAnimChannel> {
        if chunk_data.len() < 8 {
            return None;
        }
        let num_timecodes =
            u32::from_le_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]])
                as usize;
        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
        let vector_len = chunk_data[6] as u16;
        let flags_tc = chunk_data[7] as u16;

        if vector_len == 0 {
            return None;
        }

        let vl = vector_len as usize;
        let entry_size = 4 + vl * 4;
        if chunk_data.len() < 8 + num_timecodes * entry_size {
            return None;
        }

        let mut timecodes: Vec<(u32, bool, Vec<f32>)> = Vec::with_capacity(num_timecodes);
        for i in 0..num_timecodes {
            let base = 8 + i * entry_size;
            let tc = u32::from_le_bytes([
                chunk_data[base],
                chunk_data[base + 1],
                chunk_data[base + 2],
                chunk_data[base + 3],
            ]);
            let binary = (tc & 0x8000_0000) != 0;
            let frame = tc & 0x7FFF_FFFF;
            let mut vals = Vec::with_capacity(vl);
            for c in 0..vl {
                let off = base + 4 + c * 4;
                vals.push(f32::from_le_bytes([
                    chunk_data[off],
                    chunk_data[off + 1],
                    chunk_data[off + 2],
                    chunk_data[off + 3],
                ]));
            }
            timecodes.push((frame, binary, vals));
        }

        let total_frames = num_frames as usize;
        if total_frames == 0 {
            return Some(W3dAnimChannel {
                first_frame: 0,
                last_frame: 0,
                vector_len,
                flags: Self::map_timecoded_flag(flags_tc),
                pivot,
                data: Vec::new(),
            });
        }

        let mut data = vec![0.0f32; total_frames * vl];

        if timecodes.is_empty() {
            return Some(W3dAnimChannel {
                first_frame: 0,
                last_frame: (num_frames.max(1) - 1) as u16,
                vector_len,
                flags: Self::map_timecoded_flag(flags_tc),
                pivot,
                data,
            });
        }

        timecodes.sort_by_key(|t| t.0);

        let (first_tc, _, ref v0) = timecodes[0];
        for f in 0..(first_tc.min(num_frames)) as usize {
            let base = f * vl;
            for c in 0..vl {
                data[base + c] = v0[c];
            }
        }

        for seg in 0..timecodes.len() {
            let (f0, bin0, ref v0_vals) = timecodes[seg];
            let f1 = if seg + 1 < timecodes.len() {
                timecodes[seg + 1].0
            } else {
                num_frames - 1
            };
            let v1_vals = if seg + 1 < timecodes.len() {
                &timecodes[seg + 1].2
            } else {
                v0_vals
            };
            let start_f = f0 as usize;
            let end_f = f1 as usize;
            if start_f > end_f || start_f as u32 >= num_frames {
                continue;
            }
            let clamped_end = end_f.min(total_frames - 1);
            for f in start_f..=clamped_end {
                let t = if end_f == start_f {
                    0.0
                } else {
                    (f as f32 - start_f as f32) / (end_f as f32 - start_f as f32)
                };
                let base = f * vl;
                for c in 0..vl {
                    data[base + c] = if bin0 {
                        v0_vals[c]
                    } else {
                        v0_vals[c] * (1.0 - t) + v1_vals[c] * t
                    };
                }
            }
        }

        Some(W3dAnimChannel {
            first_frame: 0,
            last_frame: (num_frames.max(1) - 1) as u16,
            vector_len,
            flags: Self::map_timecoded_flag(flags_tc),
            pivot,
            data,
        })
    }

    /// Parse a single adaptive delta animation channel (ANIM_FLAVOR_ADAPTIVE_DELTA, flavor=1).
    /// Format: num_frames(u32) + pivot(u16) + vector_len(u8) + flags(u8) + scale(f32)
    ///         then blocks of 16 frames each, where each block contains vector_len packets.
    ///         Each packet: 1 byte (filter_idx in lower 7 bits) + 8 bytes (16 4-bit nibbles).
    ///         Delta decoding with adaptive filter table.
    fn parse_adaptive_delta_channel(chunk_data: &[u8], hdr_num_frames: u32) -> Vec<W3dAnimChannel> {
        if chunk_data.len() < 12 {
            return Vec::new();
        }
        let num_frames =
            u32::from_le_bytes([chunk_data[0], chunk_data[1], chunk_data[2], chunk_data[3]])
                as usize;
        let pivot = u16::from_le_bytes([chunk_data[4], chunk_data[5]]);
        let vector_len = chunk_data[6] as usize;
        let _flags_raw = chunk_data[7] as u16;
        let scale =
            f32::from_le_bytes([chunk_data[8], chunk_data[9], chunk_data[10], chunk_data[11]]);

        if vector_len == 0 || num_frames == 0 {
            return Vec::new();
        }

        let blocks = (num_frames + 15) / 16;
        let packet_count = blocks * vector_len;
        let bytes_needed = 8 + packet_count * 9;
        if chunk_data.len() < bytes_needed {
            return Vec::new();
        }

        let filter_table = Self::build_adaptive_delta_filter_table();
        let mut data = vec![0.0f32; num_frames * vector_len];
        let mut last_vals = vec![0.0f32; vector_len];
        if vector_len == 4 {
            last_vals[3] = 1.0;
        }

        let mut read_pos = 12usize;
        for b in 0..blocks {
            for vi in 0..vector_len {
                let b0 = chunk_data[read_pos];
                read_pos += 1;
                let filter_idx = (b0 & 0x7F) as usize;
                let mut nibbles = [0u8; 16];
                for byte_i in 0..8 {
                    let byte = chunk_data[read_pos];
                    read_pos += 1;
                    nibbles[byte_i * 2] = byte & 0x0F;
                    nibbles[byte_i * 2 + 1] = (byte >> 4) & 0x0F;
                }
                let filter = filter_table.get(filter_idx).copied().unwrap_or(1.0) * scale;
                for fi in 0..16 {
                    let frame = b * 16 + fi;
                    if frame >= num_frames {
                        break;
                    }
                    let raw = nibbles[fi] as i32;
                    let factor = (raw - 8) as f32;
                    let value = last_vals[vi] + factor * filter;
                    data[frame * vector_len + vi] = value;
                    last_vals[vi] = value;
                }
            }
        }

        let mut out = Vec::new();
        if vector_len == 3 {
            for axis in 0..3 {
                let mut axis_data = Vec::with_capacity(num_frames);
                for f in 0..num_frames {
                    axis_data.push(data[f * 3 + axis]);
                }
                out.push(W3dAnimChannel {
                    first_frame: 0,
                    last_frame: (hdr_num_frames.max(1) - 1) as u16,
                    vector_len: 1,
                    flags: axis as u16,
                    pivot,
                    data: axis_data,
                });
            }
        } else if vector_len == 4 {
            for f in 0..num_frames {
                let i = f * 4;
                let x = data[i];
                let y = data[i + 1];
                let z = data[i + 2];
                let w = data[i + 3];
                let len = (x * x + y * y + z * z + w * w).sqrt();
                if len > 1e-5 {
                    data[i] = x / len;
                    data[i + 1] = y / len;
                    data[i + 2] = z / len;
                    data[i + 3] = w / len;
                }
            }
            out.push(W3dAnimChannel {
                first_frame: 0,
                last_frame: (hdr_num_frames.max(1) - 1) as u16,
                vector_len: 4,
                flags: 6,
                pivot,
                data,
            });
        } else {
            let mut axis_data = Vec::with_capacity(num_frames);
            for f in 0..num_frames {
                axis_data.push(data[f * vector_len]);
            }
            out.push(W3dAnimChannel {
                first_frame: 0,
                last_frame: (hdr_num_frames.max(1) - 1) as u16,
                vector_len: 1,
                flags: 0,
                pivot,
                data: axis_data,
            });
        }
        out
    }

    fn map_timecoded_flag(flag: u16) -> u16 {
        match flag {
            8 => 0,
            9 => 1,
            10 => 2,
            11 => 6,
            _ => flag,
        }
    }

    fn build_adaptive_delta_filter_table() -> [f32; 256] {
        let mut table = [0.0f32; 256];
        let base: [f32; 16] = [
            1e-8, 1e-7, 1e-6, 1e-5, 1e-4, 1e-3, 1e-2, 1e-1, 1.0, 10.0, 100.0, 1000.0, 10000.0,
            100000.0, 1000000.0, 10000000.0,
        ];
        table[..16].copy_from_slice(&base);
        let gen_start = 16usize;
        let gen_size = 256 - gen_start;
        for i in 0..gen_size {
            let ratio = i as f32 / gen_size as f32;
            table[gen_start + i] = 1.0 - (std::f32::consts::FRAC_PI_2 * ratio).sin();
        }
        table
    }

    /// Compute global bone transforms from hierarchy pivots.
    /// Ported from standalone writer.rs compute_global_transforms + mat4_from_tr_quat.
    fn compute_global_transforms(hierarchy: &W3dHierarchy) -> Vec<[f32; 16]> {
        let n = hierarchy.pivots.len();
        let mut globals: Vec<[f32; 16]> = vec![[0.0; 16]; n];
        for i in 0..n {
            let local = Self::mat4_from_tr_quat(&hierarchy.pivots[i]);
            let g = if hierarchy.pivots[i].parent_idx != 0xFFFF_FFFF {
                let p = hierarchy.pivots[i].parent_idx as usize;
                Self::mat4_mul(&globals[p], &local)
            } else {
                local
            };
            globals[i] = g;
        }
        globals
    }

    /// Build a column-major 4x4 matrix from pivot translation + quaternion.
    /// Ported from standalone writer.rs mat4_from_tr_quat.
    fn mat4_from_tr_quat(pivot: &W3dPivot) -> [f32; 16] {
        let x = pivot.rotation[0];
        let y = pivot.rotation[1];
        let z = pivot.rotation[2];
        let w = pivot.rotation[3];
        let xx = x * x;
        let yy = y * y;
        let zz = z * z;
        let xy = x * y;
        let xz = x * z;
        let yz = y * z;
        let wx = w * x;
        let wy = w * y;
        let wz = w * z;
        let m00 = 1.0 - 2.0 * (yy + zz);
        let m01 = 2.0 * (xy - wz);
        let m02 = 2.0 * (xz + wy);
        let m10 = 2.0 * (xy + wz);
        let m11 = 1.0 - 2.0 * (xx + zz);
        let m12 = 2.0 * (yz - wx);
        let m20 = 2.0 * (xz - wy);
        let m21 = 2.0 * (yz + wx);
        let m22 = 1.0 - 2.0 * (xx + yy);
        let tx = pivot.translation[0];
        let ty = pivot.translation[1];
        let tz = pivot.translation[2];
        [
            m00, m01, m02, 0.0, m10, m11, m12, 0.0, m20, m21, m22, 0.0, tx, ty, tz, 1.0,
        ]
    }

    /// Column-major 4x4 matrix multiply. Ported from standalone writer.rs mat4_mul.
    fn mat4_mul(a: &[f32; 16], b: &[f32; 16]) -> [f32; 16] {
        let mut r = [0.0; 16];
        for i in 0..4 {
            for j in 0..4 {
                r[i * 4 + j] = a[i * 4 + 0] * b[0 * 4 + j]
                    + a[i * 4 + 1] * b[1 * 4 + j]
                    + a[i * 4 + 2] * b[2 * 4 + j]
                    + a[i * 4 + 3] * b[3 * 4 + j];
            }
        }
        r
    }

    /// Resolve texture indices to actual texture names - matches C++ behavior
    /// W3D_CHUNK_MAP3_FILENAME may contain texture indices (e.g., "1", "2", "3")
    /// which need to be resolved against the model.texture_names array
    ///
    /// Special case: If texture_names is empty but materials have numeric texture references,
    /// we need to build a texture array from materials in order (C++ behavior when W3D_CHUNK_TEXTURES is missing)
    fn resolve_texture_indices(&self, model: &mut W3DModel) {
        // Check if any texture references are numeric indices
        let has_numeric_indices = model.materials.values().any(|mat| {
            if let Some(ref tex_ref) = mat.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        }) || model.meshes.iter().any(|mesh| {
            if let Some(ref tex_ref) = mesh.material.texture_name {
                tex_ref.parse::<usize>().is_ok()
            } else {
                false
            }
        });

        // If we have numeric indices but no texture_names array, build one from materials
        if has_numeric_indices && model.texture_names.is_empty() {
            debug!("Building texture array from materials (W3D_CHUNK_TEXTURES missing)");

            // Collect all actual texture filenames from materials in order they appear
            let mut collected_textures: Vec<String> = Vec::new();

            for material in model.materials.values() {
                // Some materials might point to actual filenames (from DC_MAP chunks)
                if let Some(ref tex_name) = material.texture_name {
                    // Only add non-numeric filenames - these are actual texture names
                    if tex_name.parse::<usize>().is_err() {
                        if !collected_textures.contains(tex_name) {
                            debug!("  Added texture from material: {}", tex_name);
                            collected_textures.push(tex_name.clone());
                        }
                    }
                }
            }

            // If we collected any textures, use them as the texture_names array
            if !collected_textures.is_empty() {
                debug!(
                    "Collected {} textures from materials",
                    collected_textures.len()
                );
                model.texture_names = collected_textures;
            } else {
                // No actual filenames found - this might be a pure index-based model
                debug!("No texture filenames in materials, cannot resolve indices");
                return;
            }
        }

        if model.texture_names.is_empty() {
            debug!("No texture names loaded from W3D_CHUNK_TEXTURES, skipping texture index resolution");
            return;
        }

        debug!("Resolving texture indices for model: {}", model.name);
        debug!("  Available textures: {:?}", model.texture_names);

        // Go through each mesh and resolve texture indices
        for mesh in &mut model.meshes {
            if mesh.texture_library.is_empty() {
                mesh.texture_library = model.texture_names.clone();
            }

            if let Some(ref texture_ref) = mesh.material.texture_name {
                // Try to parse texture_ref as an index
                if let Ok(index) = texture_ref.parse::<usize>() {
                    // It's an index - resolve it
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved texture index {} to texture name: {}",
                            index, resolved_name
                        );
                        mesh.material.texture_name = Some(resolved_name);
                    } else {
                        warn!(
                            "Texture index {} out of bounds (only {} textures available)",
                            index,
                            model.texture_names.len()
                        );
                    }
                } else {
                    // It's a filename, keep as-is
                    debug!(
                        "Texture reference '{}' is not an index, keeping as filename",
                        texture_ref
                    );
                }
            }

            if !mesh.per_pass_stage_texture_ids.is_empty() {
                let mut per_pass_names = Vec::with_capacity(mesh.per_pass_stage_texture_ids.len());
                for stages in &mesh.per_pass_stage_texture_ids {
                    let mut stage_names = Vec::with_capacity(stages.len());
                    for ids in stages {
                        let names = ids
                            .iter()
                            .filter_map(|texture_id| {
                                if *texture_id == u32::MAX {
                                    None
                                } else {
                                    mesh.texture_name_from_library(*texture_id)
                                        .map(|name| name.to_string())
                                }
                            })
                            .collect::<Vec<_>>();
                        stage_names.push(names);
                    }
                    per_pass_names.push(stage_names);
                }
                mesh.per_pass_stage_texture_names = per_pass_names;

                if mesh.material.texture_name.is_none() {
                    mesh.material.texture_name = Self::stage_texture_from_mesh(mesh, 0, 0);
                }
            }
        }

        // Also update materials map if they have texture references
        for (name, material) in &mut model.materials {
            if let Some(ref texture_ref) = material.texture_name {
                if let Ok(index) = texture_ref.parse::<usize>() {
                    if index < model.texture_names.len() {
                        let resolved_name = model.texture_names[index].clone();
                        debug!(
                            "Resolved material '{}' texture index {} to: {}",
                            name, index, resolved_name
                        );
                        let mut updated_material = material.clone();
                        updated_material.texture_name = Some(resolved_name);
                        *material = updated_material;
                    }
                }
            }
        }
    }

    fn parse_u32_array(&self, data: &[u8]) -> Result<Vec<u32>> {
        if data.len() % 4 != 0 {
            return Err(anyhow!("invalid u32 array length {}", data.len()));
        }
        let mut values = Vec::with_capacity(data.len() / 4);
        let mut offset = 0usize;
        while offset + 4 <= data.len() {
            values.push(u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]));
            offset += 4;
        }
        Ok(values)
    }

    fn parse_rgba_colors(&self, data: &[u8]) -> Result<Vec<W3dRGBAStruct>> {
        if data.len() % 4 != 0 {
            return Err(anyhow!("invalid RGBA array length {}", data.len()));
        }
        let mut colors = Vec::with_capacity(data.len() / 4);
        let mut offset = 0usize;
        while offset + 4 <= data.len() {
            colors.push(W3dRGBAStruct {
                r: data[offset],
                g: data[offset + 1],
                b: data[offset + 2],
                a: data[offset + 3],
            });
            offset += 4;
        }
        Ok(colors)
    }

    fn parse_per_face_texcoord_ids(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        if data.len() % 12 != 0 {
            return Err(anyhow!(
                "invalid per-face texcoord id array length {}",
                data.len()
            ));
        }
        let mut values = Vec::with_capacity(data.len() / 12);
        let mut offset = 0usize;
        while offset + 12 <= data.len() {
            values.push([
                u32::from_le_bytes([
                    data[offset],
                    data[offset + 1],
                    data[offset + 2],
                    data[offset + 3],
                ]),
                u32::from_le_bytes([
                    data[offset + 4],
                    data[offset + 5],
                    data[offset + 6],
                    data[offset + 7],
                ]),
                u32::from_le_bytes([
                    data[offset + 8],
                    data[offset + 9],
                    data[offset + 10],
                    data[offset + 11],
                ]),
            ]);
            offset += 12;
        }
        Ok(values)
    }

    fn parse_texture_stage_chunk(&self, data: &[u8]) -> Result<ParsedTextureStage> {
        let mut stage = ParsedTextureStage::default();
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_TEXTURE_IDS => {
                    stage.texture_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_STAGE_TEXCOORDS | W3D_CHUNK_TEXCOORDS => {
                    stage.texcoords = self.parse_texcoords(chunk_data)?;
                }
                W3D_CHUNK_PER_FACE_TEXCOORD_IDS => {
                    stage.per_face_texcoord_ids = self.parse_per_face_texcoord_ids(chunk_data)?;
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }
        Ok(stage)
    }

    fn parse_material_pass_chunk(&self, data: &[u8]) -> Result<ParsedMaterialPass> {
        let mut pass = ParsedMaterialPass::default();
        let mut offset = 0usize;
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_VERTEX_MATERIAL_IDS => {
                    pass.vertex_material_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_SHADER_IDS => {
                    pass.shader_ids = self.parse_u32_array(chunk_data)?;
                }
                W3D_CHUNK_DCG => {
                    pass.dcg_colors = self.parse_rgba_colors(chunk_data)?;
                }
                W3D_CHUNK_DIG => {
                    // C++ reads DIG as W3dRGBAStruct and uses RGB channels.
                    pass.dig_colors = self.parse_rgba_colors(chunk_data)?;
                }
                W3D_CHUNK_TEXTURE_STAGE => {
                    let stage = self.parse_texture_stage_chunk(chunk_data)?;
                    pass.stage_texture_ids.push(stage.texture_ids);
                    pass.stage_texcoords.push(stage.texcoords);
                    pass.stage_per_face_texcoord_ids
                        .push(stage.per_face_texcoord_ids);
                }
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok(pass)
    }

    fn parse_shaders_chunk(&self, data: &[u8]) -> Result<Vec<W3dShaderStruct>> {
        // C++ W3dShaderStruct is 16 bytes (15 data bytes + 1 pad byte).
        if data.len() % 16 != 0 {
            return Err(anyhow!("invalid shader chunk length {}", data.len()));
        }

        let mut shaders = Vec::with_capacity(data.len() / 16);
        let mut offset = 0usize;
        while offset + 16 <= data.len() {
            shaders.push(W3dShaderStruct {
                depth_compare: data[offset],
                depth_mask: data[offset + 1],
                color_mask: data[offset + 2],
                dest_blend: data[offset + 3],
                fog_func: data[offset + 4],
                pri_gradient: data[offset + 5],
                sec_gradient: data[offset + 6],
                src_blend: data[offset + 7],
                texturing: data[offset + 8],
                detail_color_func: data[offset + 9],
                detail_alpha_func: data[offset + 10],
                shader_preset: data[offset + 11],
                alpha_test: data[offset + 12],
                post_detail_color_func: data[offset + 13],
                post_detail_alpha_func: data[offset + 14],
            });
            offset += 16;
        }
        Ok(shaders)
    }

    fn default_vertex_material() -> W3dVertexMaterialStruct {
        W3dVertexMaterialStruct {
            attributes: 0,
            ambient: W3dRGBAStruct {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            diffuse: W3dRGBAStruct {
                r: 255,
                g: 255,
                b: 255,
                a: 255,
            },
            specular: W3dRGBAStruct {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            emissive: W3dRGBAStruct {
                r: 0,
                g: 0,
                b: 0,
                a: 255,
            },
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
        }
    }

    fn parse_vertex_material_info_chunk(&self, data: &[u8]) -> Result<W3dVertexMaterialStruct> {
        // C++ W3dVertexMaterialStruct uses 3-byte RGB triplets with 4-byte alignment.
        // Accept both canonical 28-byte layout and 32-byte RGBA-expanded variant.
        if data.len() < 28 {
            return Err(anyhow!(
                "vertex material info chunk too small: {} bytes",
                data.len()
            ));
        }

        let mut material = Self::default_vertex_material();
        material.attributes = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        if data.len() >= 32 {
            material.ambient = W3dRGBAStruct {
                r: data[4],
                g: data[5],
                b: data[6],
                a: data[7],
            };
            material.diffuse = W3dRGBAStruct {
                r: data[8],
                g: data[9],
                b: data[10],
                a: data[11],
            };
            material.specular = W3dRGBAStruct {
                r: data[12],
                g: data[13],
                b: data[14],
                a: data[15],
            };
            material.emissive = W3dRGBAStruct {
                r: data[16],
                g: data[17],
                b: data[18],
                a: data[19],
            };
            material.shininess = f32::from_le_bytes([data[20], data[21], data[22], data[23]]);
            material.opacity = f32::from_le_bytes([data[24], data[25], data[26], data[27]]);
            material.translucency = f32::from_le_bytes([data[28], data[29], data[30], data[31]]);
        } else {
            material.ambient = W3dRGBAStruct {
                r: data[4],
                g: data[5],
                b: data[6],
                a: 255,
            };
            material.diffuse = W3dRGBAStruct {
                r: data[7],
                g: data[8],
                b: data[9],
                a: 255,
            };
            material.specular = W3dRGBAStruct {
                r: data[10],
                g: data[11],
                b: data[12],
                a: 255,
            };
            material.emissive = W3dRGBAStruct {
                r: data[13],
                g: data[14],
                b: data[15],
                a: 255,
            };
            material.shininess = f32::from_le_bytes([data[16], data[17], data[18], data[19]]);
            material.opacity = f32::from_le_bytes([data[20], data[21], data[22], data[23]]);
            material.translucency = f32::from_le_bytes([data[24], data[25], data[26], data[27]]);
        }

        Ok(material)
    }

    fn parse_single_vertex_material_chunk(
        &self,
        data: &[u8],
    ) -> Result<(W3dVertexMaterialStruct, VertexMapperConfig)> {
        let mut material = Self::default_vertex_material();
        let mapper = VertexMapperConfig::default();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
            match chunk_type {
                W3D_CHUNK_VERTEX_MATERIAL_INFO => {
                    material = self.parse_vertex_material_info_chunk(chunk_data)?;
                }
                W3D_CHUNK_VERTEX_MATERIAL_NAME
                | W3D_CHUNK_VERTEX_MAPPER_ARGS0
                | W3D_CHUNK_VERTEX_MAPPER_ARGS1 => {}
                _ => {}
            }

            offset += 8 + chunk_size;
        }

        Ok((material, mapper))
    }

    fn parse_vertex_materials_chunk(
        &self,
        data: &[u8],
    ) -> Result<(Vec<W3dVertexMaterialStruct>, Vec<VertexMapperConfig>)> {
        let mut materials = Vec::new();
        let mut mappers = Vec::new();
        let mut offset = 0usize;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            if chunk_type == W3D_CHUNK_VERTEX_MATERIAL {
                let chunk_data = &data[offset + 8..offset + 8 + chunk_size];
                let (material, mapper) = self.parse_single_vertex_material_chunk(chunk_data)?;
                materials.push(material);
                mappers.push(mapper);
            }

            offset += 8 + chunk_size;
        }

        Ok((materials, mappers))
    }

    /// Parse a W3D mesh chunk
    fn parse_mesh_chunk(&self, data: &[u8]) -> Result<W3DMesh> {
        debug!("parse_mesh_chunk called, data size: {} bytes", data.len());
        let mut mesh = W3DMesh::new("unknown_mesh".to_string());
        let mut offset = 0;
        let mut has_valid_mesh_header = false;

        let mut vertices: Vec<[f32; 3]> = Vec::new();
        let mut normals: Vec<[f32; 3]> = Vec::new();
        let mut texcoords: Vec<[f32; 2]> = Vec::new();
        let mut vertex_colors: Vec<[f32; 4]> = Vec::new();
        let mut triangles: Vec<[u32; 3]> = Vec::new();
        let mut expected_vertex_count: Option<u32> = None;
        let mut texture_names: Vec<String> = Vec::new(); // C++ MeshLoadContextClass texture array

        // Parse mesh sub-chunks with safety counter
        let mut mesh_chunk_counter = 0;
        const MAX_MESH_CHUNKS: usize = 1000; // Safety limit for mesh chunks

        while offset + 8 <= data.len() && mesh_chunk_counter < MAX_MESH_CHUNKS {
            mesh_chunk_counter += 1;
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            let _is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Mesh sub-chunk extends beyond mesh: type 0x{:08X}, size {}",
                    chunk_type, chunk_size
                );
                break;
            }

            // Safety checks for mesh chunks
            if chunk_size == 0 {
                warn!(
                    "Zero-sized mesh chunk detected (type 0x{:08X}) - skipping",
                    chunk_type
                );
                offset += 8;
                continue;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MESH_HEADER => {
                    debug!(
                        "Parsing mesh header (W3dMeshHeader3Struct), size: {}",
                        chunk_size
                    );
                    let header = self
                        .parse_mesh_header(chunk_data)
                        .map_err(|e| anyhow!("invalid mesh header in '{}': {}", mesh.name, e))?;
                    has_valid_mesh_header = true;
                    mesh.name = header.mesh_name;
                    expected_vertex_count = Some(header.num_vertices);
                    debug!(
                        "Mesh name: '{}', expecting {} vertices, {} triangles",
                        mesh.name, header.num_vertices, header.num_triangles
                    );
                }
                W3D_CHUNK_VERTICES => {
                    vertices = self.parse_vertices_with_count(chunk_data, expected_vertex_count)?;
                    debug!("Parsed {} vertices", vertices.len());
                }
                W3D_CHUNK_VERTEX_NORMALS => {
                    normals = self.parse_normals(chunk_data)?;
                    debug!("Parsed {} normals", normals.len());
                }
                W3D_CHUNK_TEXCOORDS => {
                    texcoords = self.parse_texcoords(chunk_data)?;
                    debug!("Parsed {} texture coordinates", texcoords.len());
                }
                W3D_CHUNK_VERTEX_COLORS => {
                    vertex_colors = self.parse_vertex_colors(chunk_data)?;
                    debug!("Parsed {} vertex colors", vertex_colors.len());
                }
                W3D_CHUNK_TRIANGLES => {
                    triangles = self.parse_triangles(chunk_data)?;
                    debug!("Parsed {} triangles", triangles.len());
                }
                W3D_CHUNK_MATERIAL_INFO => {
                    debug!("Parsing material info chunk, size: {}", chunk_size);
                    if let Ok(material) = self.parse_material_info(chunk_data) {
                        mesh.material = material;
                        debug!(
                            "Parsed material: {} (texture: {:?})",
                            mesh.material.name, mesh.material.texture_name
                        );
                    } else {
                        warn!("Failed to parse material info chunk");
                    }
                }
                W3D_CHUNK_MAP3_FILENAME => {
                    // Extract texture filename from MAP3_FILENAME chunk
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }
                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        mesh.material.texture_name = Some(filename);
                    }
                }
                W3D_CHUNK_VERTEX_SHADE_INDICES => {
                    // Shade indices for vertex coloring - skip for now
                    debug!(
                        "Skipping W3D_CHUNK_VERTEX_SHADE_INDICES ({} bytes)",
                        chunk_size
                    );
                }
                W3D_CHUNK_SHADERS => match self.parse_shaders_chunk(chunk_data) {
                    Ok(shaders) => {
                        debug!("Parsed {} shaders", shaders.len());
                        mesh.shaders = shaders;
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_SHADERS: {}", err);
                    }
                },
                W3D_CHUNK_VERTEX_MATERIALS => match self.parse_vertex_materials_chunk(chunk_data) {
                    Ok((materials, mappers)) => {
                        debug!(
                            "Parsed {} vertex materials and {} mapper configs",
                            materials.len(),
                            mappers.len()
                        );
                        mesh.vertex_materials = materials;
                        mesh.vertex_mappers = mappers;
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_VERTEX_MATERIALS: {}", err);
                    }
                },
                W3D_CHUNK_MATERIAL_PASS => match self.parse_material_pass_chunk(chunk_data) {
                    Ok(pass_data) => {
                        let mut stage_texture_names = Vec::new();
                        for texture_ids in &pass_data.stage_texture_ids {
                            let names = texture_ids
                                .iter()
                                .filter_map(|texture_id| {
                                    if *texture_id == u32::MAX {
                                        return None;
                                    }
                                    texture_names.get(*texture_id as usize).cloned()
                                })
                                .collect::<Vec<_>>();
                            stage_texture_names.push(names);
                        }

                        mesh.passes.push(MaterialPassInfo {
                            vm_id: pass_data.vertex_material_ids.first().copied().unwrap_or(0),
                            shader_id: pass_data.shader_ids.first().copied().unwrap_or(0),
                            texture_count: pass_data.stage_texture_ids.len() as u32,
                        });
                        mesh.per_pass_vertex_material_ids
                            .push(pass_data.vertex_material_ids.clone());
                        mesh.per_pass_shader_ids.push(pass_data.shader_ids.clone());
                        mesh.per_pass_dcg_colors.push(pass_data.dcg_colors.clone());
                        mesh.per_pass_dig_colors.push(pass_data.dig_colors.clone());
                        mesh.per_pass_stage_texture_ids
                            .push(pass_data.stage_texture_ids.clone());
                        mesh.per_pass_stage_texture_names.push(stage_texture_names);

                        for (stage_index, stage_uvs) in pass_data.stage_texcoords.iter().enumerate()
                        {
                            mesh.stage_texcoords.push(stage_uvs.clone());
                            mesh.per_stage_face_texcoord_ids.push(
                                pass_data
                                    .stage_per_face_texcoord_ids
                                    .get(stage_index)
                                    .cloned()
                                    .unwrap_or_default(),
                            );
                        }
                    }
                    Err(err) => {
                        warn!("Failed to parse W3D_CHUNK_MATERIAL_PASS: {}", err);
                    }
                },
                W3D_CHUNK_TEXTURES => {
                    // Parse textures container - C++ read_textures() equivalent
                    debug!(
                        "Found W3D_CHUNK_TEXTURES inside mesh, size: {} bytes",
                        chunk_size
                    );
                    // Parse texture names from W3D_CHUNK_TEXTURE/W3D_CHUNK_TEXTURE_NAME
                    if let Ok(names) = self.parse_textures_chunk_into_array(chunk_data) {
                        debug!("Loaded {} texture(s) for mesh: {:?}", names.len(), names);
                        texture_names.extend(names);
                    }
                }
                _ => {
                    debug!("Unknown mesh sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        if mesh_chunk_counter >= MAX_MESH_CHUNKS {
            warn!(
                "⚠️  Mesh chunk parsing hit safety limit ({} chunks)",
                MAX_MESH_CHUNKS
            );
        }

        if !has_valid_mesh_header {
            return Err(anyhow!("mesh chunk missing required W3D mesh header"));
        }

        let stage0_fallback_texcoords = texcoords.clone();

        // Build final mesh (logging disabled)
        self.build_mesh_from_data(
            &mut mesh,
            vertices,
            normals,
            texcoords,
            vertex_colors,
            triangles,
        )?;

        if !texture_names.is_empty() {
            mesh.texture_library = texture_names.clone();
        }

        if mesh.stage_texcoords.is_empty() && !stage0_fallback_texcoords.is_empty() {
            mesh.stage_texcoords.push(stage0_fallback_texcoords);
            mesh.stage_uv_channels = vec![0];
            if mesh.per_stage_face_texcoord_ids.is_empty() {
                mesh.per_stage_face_texcoord_ids.push(Vec::new());
            }
        } else if !mesh.stage_texcoords.is_empty() {
            let (unique_layers, stage_channels) =
                deduplicate_stage_uv_layers(mesh.stage_texcoords.clone());
            mesh.stage_texcoords = unique_layers;
            mesh.stage_uv_channels = stage_channels;
            if mesh.per_stage_face_texcoord_ids.is_empty() {
                mesh.per_stage_face_texcoord_ids = vec![Vec::new(); mesh.stage_texcoords.len()];
            }
        }

        if !mesh.per_pass_stage_texture_ids.is_empty() {
            let mut per_pass_names = Vec::with_capacity(mesh.per_pass_stage_texture_ids.len());
            for stage_set in &mesh.per_pass_stage_texture_ids {
                let mut stage_names = Vec::with_capacity(stage_set.len());
                for ids in stage_set {
                    let names = ids
                        .iter()
                        .filter_map(|texture_id| {
                            if *texture_id == u32::MAX {
                                None
                            } else {
                                mesh.texture_name_from_library(*texture_id)
                                    .map(|name| name.to_string())
                            }
                        })
                        .collect::<Vec<_>>();
                    stage_names.push(names);
                }
                per_pass_names.push(stage_names);
            }
            mesh.per_pass_stage_texture_names = per_pass_names;
        }

        // C++ behavior: single-material fallback uses first texture if pass data does not bind one.
        if mesh.material.texture_name.is_none() && !texture_names.is_empty() {
            mesh.material.texture_name = Some(texture_names[0].clone());
        }
        if mesh.material.texture_name.is_none() {
            mesh.material.texture_name = Self::stage_texture_from_mesh(&mesh, 0, 0);
        }

        if let Some(texture_name) = &mesh.material.texture_name {
            debug!("Mesh '{}' will use texture: '{}'", mesh.name, texture_name);
        }

        // Map W3D shader blend factors to material blend_mode for C++ parity.
        // Uses the first shader, or the shader referenced by the first material pass.
        let shader_idx = mesh
            .passes
            .first()
            .map(|p| p.shader_id as usize)
            .unwrap_or(0);
        if let Some(shader) = mesh.shaders.get(shader_idx) {
            let (mode, alpha_test) =
                shader_blend_to_mode(shader.src_blend, shader.dest_blend, shader.alpha_test);
            mesh.material.blend_mode = mode;
            mesh.material.alpha_test_enabled = alpha_test;
            debug!(
                "Mesh '{}' blend_mode={:?}, alpha_test={} (src={}, dest={})",
                mesh.name,
                mesh.material.blend_mode,
                mesh.material.alpha_test_enabled,
                shader.src_blend,
                shader.dest_blend
            );
        }

        Ok(mesh)
    }

    /// Parse mesh header - C++ compatible W3dMeshHeader3Struct format
    fn parse_mesh_header(&self, data: &[u8]) -> Result<MeshHeader> {
        // W3dMeshHeader3Struct layout:
        // 0: uint32 Version
        // 4: uint32 Attributes
        // 8: char MeshName[16]
        // 24: char ContainerName[16]
        // 40: uint32 NumTris
        // 44: uint32 NumVertices
        // 48: uint32 NumMaterials
        // 52: uint32 NumDamageStages
        // 56: sint32 SortLevel
        // 60: uint32 PrelitVersion
        // 64: uint32 FutureCounts[1]
        // 68: uint32 VertexChannels
        // 72: uint32 FaceChannels
        // Plus bounding box, sphere data...

        if data.len() < 76 {
            // Minimum size for core header fields
            return Err(anyhow!("Mesh header too small: {} bytes", data.len()));
        }

        let version = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
        let attributes = u32::from_le_bytes([data[4], data[5], data[6], data[7]]);
        let num_triangles = u32::from_le_bytes([data[40], data[41], data[42], data[43]]);
        let num_vertices = u32::from_le_bytes([data[44], data[45], data[46], data[47]]);

        // Extract mesh name (null-terminated string at offset 8, max 16 chars)
        let mut mesh_name = String::new();
        for i in 8..24 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            mesh_name.push(data[i] as char);
        }

        // Extract container name (null-terminated string at offset 24, max 16 chars)
        let mut container_name = String::new();
        for i in 24..40 {
            if i >= data.len() || data[i] == 0 {
                break;
            }
            container_name.push(data[i] as char);
        }

        debug!("Mesh header - version: 0x{:08X}, attributes: 0x{:08X}, triangles: {}, vertices: {}, mesh_name: '{}', container: '{}'", 
               version, attributes, num_triangles, num_vertices, mesh_name, container_name);

        Ok(MeshHeader {
            version,
            flags: attributes, // attributes field is what was called flags in the old structure
            num_triangles,
            num_vertices,
            mesh_name: if mesh_name.is_empty() {
                "unnamed_mesh".to_string()
            } else {
                mesh_name
            },
        })
    }

    /// Parse vertices array with expected count validation - C++ compatible version
    fn parse_vertices_with_count(
        &self,
        data: &[u8],
        expected_count: Option<u32>,
    ) -> Result<Vec<[f32; 3]>> {
        // In C++: reads vertex count from mesh header, then reads that many W3dVectorStruct (12 bytes each)
        // No headers or padding in vertex chunk data itself - just raw vertex data

        let vertex_count = if let Some(expected) = expected_count {
            expected as usize
        } else {
            // Fallback: assume data contains only vertices (12 bytes each)
            data.len() / 12
        };

        debug!(
            "parse_vertices_with_count: data.len()={}, expected_count={:?}, using vertex_count={}",
            data.len(),
            expected_count,
            vertex_count
        );

        // Verify we have enough data for the expected vertices
        let required_size = vertex_count * 12; // 12 bytes per W3dVectorStruct
        if data.len() < required_size {
            return Err(anyhow!(
                "Insufficient vertex data: need {} bytes, have {} (for {} vertices)",
                required_size,
                data.len(),
                vertex_count
            ));
        }

        let mut vertices = Vec::with_capacity(vertex_count);

        // Read vertices directly as W3dVectorStruct (float32 X, Y, Z)
        for i in 0..vertex_count {
            let offset = i * 12;
            if offset + 12 > data.len() {
                warn!(
                    "Vertex {} would exceed data bounds, stopping at {} vertices",
                    i,
                    vertices.len()
                );
                break;
            }

            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Validate vertices are reasonable (not NaN, not infinite)
            if !x.is_finite() || !y.is_finite() || !z.is_finite() {
                warn!(
                    "Vertex {} has non-finite coordinates: ({}, {}, {})",
                    i, x, y, z
                );
                continue;
            }

            vertices.push([x, y, z]);

            // Log first few vertices for debugging
            if i < 3 {
                debug!("Vertex {}: ({:.3}, {:.3}, {:.3})", i, x, y, z);
            }
        }

        if vertices.is_empty() {
            return Err(anyhow!("No valid vertices parsed from data"));
        }

        debug!("Successfully parsed {} vertices", vertices.len());
        Ok(vertices)
    }

    /// Legacy parse vertices for backward compatibility
    fn parse_vertices(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        self.parse_vertices_with_count(data, None)
    }

    /// Parse normals array
    fn parse_normals(&self, data: &[u8]) -> Result<Vec<[f32; 3]>> {
        if data.len() % 12 != 0 {
            return Err(anyhow!("Invalid normals data size: {}", data.len()));
        }

        let normal_count = data.len() / 12;
        let mut normals = Vec::with_capacity(normal_count);

        for i in 0..normal_count {
            let offset = i * 12;
            let x = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let y = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let z = f32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);
            normals.push([x, y, z]);
        }

        Ok(normals)
    }

    /// Parse texture coordinates array
    fn parse_texcoords(&self, data: &[u8]) -> Result<Vec<[f32; 2]>> {
        if data.len() % 8 != 0 {
            return Err(anyhow!("Invalid texcoords data size: {}", data.len()));
        }

        let texcoord_count = data.len() / 8;
        let mut texcoords = Vec::with_capacity(texcoord_count);

        for i in 0..texcoord_count {
            let offset = i * 8;
            let u = f32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v = f32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            // C++ parity: WW3D stores V upside-down in chunk payload and flips on load.
            texcoords.push([u, 1.0 - v]);
        }

        Ok(texcoords)
    }

    /// Parse vertex colors array
    fn parse_vertex_colors(&self, data: &[u8]) -> Result<Vec<[f32; 4]>> {
        let mut colors = Vec::new();

        if data.len() % 3 == 0 {
            let color_count = data.len() / 3;
            colors.reserve(color_count);
            for i in 0..color_count {
                let offset = i * 3;
                colors.push([
                    data[offset] as f32 / 255.0,
                    data[offset + 1] as f32 / 255.0,
                    data[offset + 2] as f32 / 255.0,
                    1.0,
                ]);
            }
            return Ok(colors);
        }

        if data.len() % 4 == 0 {
            let color_count = data.len() / 4;
            colors.reserve(color_count);
            for i in 0..color_count {
                let offset = i * 4;
                colors.push([
                    data[offset] as f32 / 255.0,
                    data[offset + 1] as f32 / 255.0,
                    data[offset + 2] as f32 / 255.0,
                    data[offset + 3] as f32 / 255.0,
                ]);
            }
            return Ok(colors);
        }

        Err(anyhow!("Invalid vertex colors data size: {}", data.len()))
    }

    /// Parse material info
    fn parse_material_info(&self, data: &[u8]) -> Result<W3DMaterial> {
        if data.len() < 4 {
            // Need at least 4 bytes for basic parsing
            return Err(anyhow!("Material info chunk too small: {}", data.len()));
        }

        // Material info structure is complex, let's extract basic information
        let mut material = W3DMaterial::default();

        // For small material info chunks (16 bytes), extract basic properties
        // For larger chunks, try to extract more detailed information

        if data.len() >= 48 {
            // Extract C++ VertexMaterialClass compatible color values for larger chunks
            let diffuse_r = f32::from_le_bytes(data[32..36].try_into().unwrap_or([0; 4]));
            let diffuse_g = f32::from_le_bytes(data[36..40].try_into().unwrap_or([0; 4]));
            let diffuse_b = f32::from_le_bytes(data[40..44].try_into().unwrap_or([0; 4]));

            if diffuse_r.is_finite() && diffuse_g.is_finite() && diffuse_b.is_finite() {
                material.diffuse_color = Vec3::new(diffuse_r, diffuse_g, diffuse_b);
            }
        }

        if data.len() >= 32 {
            // Try to extract material name for larger chunks
            let mut name = String::new();
            for i in 0..std::cmp::min(32, data.len()) {
                if data[i] == 0 {
                    break;
                }
                if data[i].is_ascii() && data[i] >= 32 {
                    name.push(data[i] as char);
                }
            }
            if !name.is_empty() {
                material.name = name;
            }
        } else if data.len() >= 16 {
            // For small material info chunks (16 bytes), extract basic properties
            debug!("Parsing 16-byte material info chunk - basic material properties");

            // Try to extract some basic properties from the first few bytes
            // Material index or type might be at the beginning
            let material_type = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
            debug!("Material type/index: 0x{:08X}", material_type);

            // Set basic properties for small chunks
            material.name = format!("material_{:08X}", material_type);
            material.diffuse_color = Vec3::new(0.8, 0.8, 0.8); // Default gray
        }

        // Note: Texture names are now loaded separately from W3D_CHUNK_TEXTURES
        // They will be associated with materials through material passes

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        // Set texture name in stage 0 if found
        if let Some(ref texture_name) = material.texture_name {
            material.stage0_mapping.texture_name = Some(texture_name.clone());
        }

        debug!(
            "Parsed material: name='{}', diffuse={:?}, texture={:?}",
            material.name, material.diffuse_color, material.texture_name
        );

        Ok(material)
    }

    /// Parse W3D textures container chunk - contains individual texture definitions
    fn parse_textures_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut texture_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid texture chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE => {
                    debug!("Parsing individual texture chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                            debug!("Found texture: {}", texture_name);
                            model.texture_names.push(texture_name);
                            texture_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Loaded {} textures from W3D_CHUNK_TEXTURES", texture_count);
        Ok(())
    }

    /// Parse W3D_CHUNK_TEXTURES and return array of texture names - C++ read_textures() equivalent
    fn parse_textures_chunk_into_array(&self, data: &[u8]) -> Result<Vec<String>> {
        debug!("parse_textures_chunk_into_array: data.len()={}", data.len());
        let mut textures = Vec::new();
        let mut offset = 0;

        // C++ code: for (TextureClass *newtex = ::Load_Texture(cload); newtex != NULL; newtex = ::Load_Texture(cload))
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);

            // Check for container chunk flag (bit 31 set on chunk size - C++ behavior)
            let is_container = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFFFFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            // C++ Load_Texture checks for W3D_CHUNK_TEXTURE
            if chunk_type == W3D_CHUNK_TEXTURE && is_container {
                if let Ok(texture_name) = self.parse_single_texture_chunk(chunk_data) {
                    textures.push(texture_name);
                }
            }

            offset += 8 + chunk_size;
        }

        debug!("Returning {} textures", textures.len());
        Ok(textures)
    }

    /// Parse a single W3D_CHUNK_TEXTURE and extract the texture name
    fn parse_single_texture_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;
        let mut texture_name: Option<String> = None;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
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

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_TEXTURE_NAME => {
                    // Read null-terminated string directly from chunk data
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        debug!("Found texture name in W3D_CHUNK_TEXTURE_NAME: {}", name);
                        texture_name = Some(name);
                    }
                }
                W3D_CHUNK_TEXTURE_INFO => {
                    debug!("Found W3D_CHUNK_TEXTURE_INFO (not parsing texture properties yet)");
                    // W3dTextureInfoStruct parsing can be added here later if needed
                }
                _ => {
                    debug!(
                        "Unknown texture sub-chunk in W3D_CHUNK_TEXTURE: 0x{:08X}",
                        chunk_type
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        texture_name.ok_or_else(|| anyhow!("No texture name found in W3D_CHUNK_TEXTURE"))
    }

    /// Parse W3D MATERIALS3 container chunk - contains material definitions with texture filenames
    /// This matches the C++ approach: create materials and directly assign texture names
    fn parse_materials3_chunk(&self, data: &[u8], model: &mut W3DModel) -> Result<()> {
        let mut offset = 0;
        let mut material_count = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let is_container_chunk = (raw_chunk_size & 0x80000000) != 0;
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                warn!(
                    "Invalid materials3 chunk size: {} at offset {}",
                    chunk_size, offset
                );
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3 => {
                    debug!("Parsing individual material3 chunk, size: {}", chunk_size);
                    if is_container_chunk {
                        // Parse the complete material (name + properties + texture) like C++ does
                        if let Ok(material) = self.parse_complete_material3_chunk(chunk_data) {
                            debug!(
                                "Found material3: '{}' with texture: {:?}",
                                material.name, material.texture_name
                            );

                            // Store the material in the model's materials HashMap
                            model
                                .materials
                                .insert(material.name.clone(), material.clone());

                            // Also add texture name to the model's texture list for loading
                            if let Some(ref texture_name) = material.texture_name {
                                model.texture_names.push(texture_name.clone());
                            }
                            material_count += 1;
                        }
                    }
                }
                _ => {
                    debug!(
                        "Unknown materials3 sub-chunk: 0x{:08X}, size: {}",
                        chunk_type, chunk_size
                    );
                }
            }

            offset += 8 + chunk_size;
        }

        debug!(
            "Loaded {} complete materials from W3D_CHUNK_MATERIALS3",
            material_count
        );
        Ok(())
    }

    /// Parse a complete W3D_CHUNK_MATERIAL3 exactly like C++ does:
    /// 1. Read W3D_CHUNK_MATERIAL3_NAME
    /// 2. Read W3D_CHUNK_MATERIAL3_INFO (material properties)
    /// 3. Read W3D_CHUNK_MATERIAL3_DC_MAP -> W3D_CHUNK_MAP3_FILENAME (texture)
    fn parse_complete_material3_chunk(&self, data: &[u8]) -> Result<W3DMaterial> {
        let mut offset = 0;
        let mut material = W3DMaterial::default();
        let mut material_name: Option<String> = None;

        // Parse chunks inside W3D_CHUNK_MATERIAL3 container like C++ does
        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_NAME => {
                    // 0x0000002D
                    // Read material name exactly like C++: cload.Read(name,cload.Cur_Chunk_Length());
                    let mut name = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            name.push(byte as char);
                        }
                    }

                    if !name.is_empty() {
                        material_name = Some(name);
                        debug!("Found material3 name: {}", material_name.as_ref().unwrap());
                    }
                }
                W3D_CHUNK_MATERIAL3_INFO => {
                    // 0x0000002E
                    // Read W3dMaterial3Struct like C++: cload.Read(&mat,sizeof(W3dMaterial3Struct))
                    debug!("Parsing W3D_CHUNK_MATERIAL3_INFO, size: {}", chunk_size);
                    // For now, set basic material properties - we can expand this later
                    material.diffuse_color = Vec3::new(0.8, 0.8, 0.8);
                    material.specular_color = Vec3::new(0.2, 0.2, 0.2);
                    material.shininess = 16.0;
                    material.opacity = 1.0;
                }
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    // 0x0000002F - Diffuse Color Map
                    debug!(
                        "Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename like C++"
                    );
                    let _is_container_chunk = (chunk_type & 0x80000000) != 0 || chunk_size > 256; // DC_MAP is a container

                    if let Ok(texture_filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                        debug!(
                            "C++ style: Found texture filename from DC_MAP: {}",
                            texture_filename
                        );
                        material.texture_name = Some(texture_filename);
                        material.stage0_mapping.texture_name = material.texture_name.clone();
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        // Set material name like C++: vmat->Set_Name(name);
        if let Some(name) = material_name {
            material.name = name;
        } else {
            material.name = "unnamed_material3".to_string();
        }

        // Set C++ compatible material properties
        material.stage0_mapping.uv_source = UVSource::UV0;
        material.stage0_mapping.blend_mode = TextureBlendMode::Modulate;
        material.blend_mode = BlendMode::Opaque;

        debug!(
            "Completed material3 parsing: '{}' with texture: {:?}",
            material.name, material.texture_name
        );

        Ok(material)
    }

    /// Parse a single W3D_CHUNK_MATERIAL3 and extract texture filenames from DC_MAP chunks
    fn parse_single_material3_chunk(&self, data: &[u8]) -> Result<Vec<String>> {
        let mut offset = 0;
        let mut texture_names: Vec<String> = Vec::new();

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let raw_chunk_size = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let chunk_size = (raw_chunk_size & 0x7FFF_FFFF) as usize;

            let is_container_chunk = (chunk_type & 0x80000000) != 0;
            let chunk_type = chunk_type & 0x7FFFFFFF;

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MATERIAL3_DC_MAP => {
                    debug!("Found W3D_CHUNK_MATERIAL3_DC_MAP, extracting texture filename");
                    if is_container_chunk {
                        // Parse the DC_MAP container to find the filename
                        if let Ok(filename) = self.parse_material3_dc_map_chunk(chunk_data) {
                            debug!("Found texture filename from material3 DC_MAP: {}", filename);
                            texture_names.push(filename);
                        }
                    }
                }
                _ => {
                    debug!("Unknown material3 sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Ok(texture_names)
    }

    /// Parse W3D_CHUNK_MATERIAL3_DC_MAP to extract texture filename
    fn parse_material3_dc_map_chunk(&self, data: &[u8]) -> Result<String> {
        let mut offset = 0;

        while offset + 8 <= data.len() {
            let chunk_type = u32::from_le_bytes([
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

            if offset + 8 + chunk_size > data.len() {
                break;
            }

            let chunk_data = &data[offset + 8..offset + 8 + chunk_size];

            match chunk_type {
                W3D_CHUNK_MAP3_FILENAME => {
                    // 0x00000030
                    // Read null-terminated string directly from chunk data
                    let mut filename = String::new();
                    for &byte in chunk_data {
                        if byte == 0 {
                            break;
                        }
                        if byte.is_ascii() && byte >= 32 {
                            filename.push(byte as char);
                        }
                    }

                    if !filename.is_empty() {
                        debug!(
                            "Found texture filename in W3D_CHUNK_MAP3_FILENAME: {}",
                            filename
                        );
                        return Ok(filename);
                    }
                }
                _ => {
                    debug!("Unknown DC_MAP sub-chunk: 0x{:08X}", chunk_type);
                }
            }

            offset += 8 + chunk_size;
        }

        Err(anyhow!(
            "No texture filename found in W3D_CHUNK_MATERIAL3_DC_MAP"
        ))
    }

    /// Parse triangles array - C++ compatible W3dTriStruct format
    fn parse_triangles(&self, data: &[u8]) -> Result<Vec<[u32; 3]>> {
        // W3dTriStruct format: 3 x uint32 vertex indices, uint32 attributes, W3dVectorStruct normal, float32 distance
        // Total size: 3*4 + 4 + 3*4 + 4 = 32 bytes per triangle
        const TRI_STRUCT_SIZE: usize = 32;

        if data.len() % TRI_STRUCT_SIZE != 0 {
            return Err(anyhow!(
                "Invalid triangles data size: {} (expected multiple of {})",
                data.len(),
                TRI_STRUCT_SIZE
            ));
        }

        let triangle_count = data.len() / TRI_STRUCT_SIZE;
        let mut triangles = Vec::with_capacity(triangle_count);

        for i in 0..triangle_count {
            let offset = i * TRI_STRUCT_SIZE;

            // Read the 3 vertex indices (first 12 bytes of W3dTriStruct)
            let v0 = u32::from_le_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]);
            let v1 = u32::from_le_bytes([
                data[offset + 4],
                data[offset + 5],
                data[offset + 6],
                data[offset + 7],
            ]);
            let v2 = u32::from_le_bytes([
                data[offset + 8],
                data[offset + 9],
                data[offset + 10],
                data[offset + 11],
            ]);

            // Skip attributes (4 bytes), normal (12 bytes), and distance (4 bytes) for now
            // We only need the vertex indices for basic rendering

            triangles.push([v0, v1, v2]);

            // Log first few triangles for debugging
            if i < 3 {
                debug!("Triangle {}: [{}, {}, {}]", i, v0, v1, v2);
            }
        }

        debug!("Successfully parsed {} triangles", triangles.len());
        Ok(triangles)
    }

    /// Build final mesh from parsed data
    fn build_mesh_from_data(
        &self,
        mesh: &mut W3DMesh,
        vertices: Vec<[f32; 3]>,
        normals: Vec<[f32; 3]>,
        texcoords: Vec<[f32; 2]>,
        vertex_colors: Vec<[f32; 4]>,
        triangles: Vec<[u32; 3]>,
    ) -> Result<()> {
        if vertices.is_empty() {
            return Err(anyhow!("No vertices in mesh"));
        }

        let vertex_count = vertices.len();
        mesh.vertices.clear();
        mesh.vertices.reserve(vertex_count);
        mesh.indices.clear();

        // Build vertices with available data
        for i in 0..vertex_count {
            let position = w3d_position_to_world(vertices[i]);
            let normal = if i < normals.len() {
                w3d_normal_to_world(normals[i])
            } else {
                [0.0, 1.0, 0.0]
            };
            let uv = if i < texcoords.len() {
                texcoords[i]
            } else {
                [0.0, 0.0]
            };
            let color = if i < vertex_colors.len() {
                vertex_colors[i]
            } else {
                [1.0, 1.0, 1.0, 1.0]
            };

            mesh.vertices.push(W3DVertex {
                position,
                normal,
                uv,
                color,
            });
        }
        mesh.vertices_in_render_space = true;
        mesh.has_explicit_vertex_colors = !vertex_colors.is_empty();

        // Build indices from triangles
        for triangle in triangles {
            if triangle[0] < vertex_count as u32
                && triangle[1] < vertex_count as u32
                && triangle[2] < vertex_count as u32
            {
                push_world_space_triangle(&mut mesh.indices, triangle[0], triangle[1], triangle[2]);
            }
        }

        // C++ parity: never synthesize triangle lists when triangle chunks are missing/invalid.
        if mesh.indices.is_empty() {
            return Err(anyhow!("mesh '{}' has no valid triangles", mesh.name));
        }

        debug!(
            "Built mesh with {} vertices and {} indices",
            mesh.vertices.len(),
            mesh.indices.len()
        );
        Ok(())
    }

    /// Load C&C model by exact asset name.
    pub async fn load_cnc_model(
        &self,
        archive_system: &mut ArchiveFileSystem,
        unit_name: &str,
    ) -> Result<W3DModel> {
        self.load_model(archive_system, unit_name).await
    }

    /// List available W3D models in archives
    pub fn list_available_models(&self, archive_system: &ArchiveFileSystem) -> Vec<String> {
        let mut models = Vec::new();
        let all_files = archive_system.list_all_files();

        for file in all_files {
            if file.to_lowercase().ends_with(".w3d") {
                models.push(file);
            }
        }

        models.sort();
        models
    }
}

/// Mesh header structure
#[derive(Debug)]
struct MeshHeader {
    pub version: u32,
    pub flags: u32,
    pub num_triangles: u32,
    pub num_vertices: u32,
    pub mesh_name: String,
}

/// Get common C&C unit models - updated with actual units found in archives
pub fn get_common_cnc_units() -> Vec<&'static str> {
    vec![
        // USA Units
        "humvee",   // avhummer - Confirmed exists
        "crusader", // avcrusader - Confirmed exists
        "chinook",  // avchinook - Confirmed exists
        "comanche", // avcomanche - Attack helicopter
        "abrams",   // Maps to crusader (main US tank)
        // China Units
        "mig",          // nvmign - Confirmed exists
        "helix",        // nvhelix - Confirmed exists
        "gattling",     // nvgatttank - Confirmed exists
        "battlemaster", // Chinese main battle tank
        "dragon",       // Dragon tank
        // GLA Units
        "scorpion",  // uvscorpion - Confirmed exists
        "toxin",     // uvtoxintrk - Confirmed exists
        "scud",      // SCUD launcher
        "technical", // Technical truck
        "marauder",  // GLA tank
        // Test units with confirmed models
        "test_tank",    // Uses uvscorpion
        "test_vehicle", // Uses avhummer
        "test_air",     // Uses nvhelix
    ]
}

fn deduplicate_stage_uv_layers(layers: Vec<Vec<[f32; 2]>>) -> (Vec<Vec<[f32; 2]>>, Vec<u8>) {
    const MAX_CHANNELS: usize = 4;
    let mut unique_layers: Vec<Vec<[f32; 2]>> = Vec::new();
    let mut stage_channels: Vec<u8> = Vec::new();
    let mut crc_map = HashMap::new();

    for coords in layers {
        if coords.is_empty() {
            if unique_layers.is_empty() {
                unique_layers.push(Vec::new());
            }
            stage_channels.push(0);
            continue;
        }

        let mut hasher = Hasher::new();
        hasher.update(bytemuck::cast_slice(&coords));
        let crc = hasher.finalize();

        let channel = if let Some(&existing) = crc_map.get(&crc) {
            existing
        } else {
            let assigned = if unique_layers.len() < MAX_CHANNELS {
                let ch = unique_layers.len() as u8;
                unique_layers.push(coords.clone());
                ch
            } else {
                (MAX_CHANNELS.saturating_sub(1)) as u8
            };
            crc_map.insert(crc, assigned);
            assigned
        };

        stage_channels.push(channel);
    }

    if unique_layers.is_empty() {
        unique_layers.push(Vec::new());
    }

    (unique_layers, stage_channels)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deduplicate_stage_uv_layers_merges_duplicate_channels() {
        let stage0 = vec![[0.0, 0.0], [1.0, 0.0]];
        let stage1 = stage0.clone();
        let stage2 = vec![[0.5, 0.5], [0.75, 0.75]];
        let layers = vec![stage0.clone(), stage1, stage2.clone()];
        let (unique_layers, stage_channels) = deduplicate_stage_uv_layers(layers);

        assert_eq!(unique_layers.len(), 2);
        assert_eq!(unique_layers[0], stage0);
        assert_eq!(unique_layers[1], stage2);
        assert_eq!(stage_channels, vec![0, 0, 1]);
    }

    #[test]
    fn apply_material_stage_mappings_sets_texture_and_uv_source() {
        let mut material = W3DMaterial::default();
        let mut mesh = W3DMesh::new("TestMesh".to_string());
        mesh.stage_uv_channels = vec![0, 2];
        mesh.per_pass_stage_texture_names = vec![vec![
            vec!["base.dds".to_string()],
            vec!["detail.dds".to_string()],
        ]];

        W3DLoader::apply_material_stage_mappings(&mut material, &mesh);

        assert_eq!(
            material.stage0_mapping.texture_name.as_deref(),
            Some("base.dds")
        );
        assert!(matches!(material.stage0_mapping.uv_source, UVSource::UV0));
        let stage1 = material
            .stage1_mapping
            .as_ref()
            .expect("stage 1 mapping missing");
        assert_eq!(stage1.texture_name.as_deref(), Some("detail.dds"));
        assert!(matches!(stage1.uv_source, UVSource::UV2));
    }
}
