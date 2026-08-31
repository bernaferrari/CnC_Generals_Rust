#![allow(dead_code)]
//! W3D data structures based on the C++ implementation

use crate::w3d::chunks::W3D_NAME_LEN;
use glam::{Quat, Vec3};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Fixed-length name used throughout W3D format
pub type W3dName = [u8; W3D_NAME_LEN];

/// Helper function to convert W3dName to String
pub fn w3d_name_to_string(name: &W3dName) -> String {
    let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());
    String::from_utf8_lossy(&name[..end]).to_string()
}

/// Helper function to convert String to W3dName
pub fn string_to_w3d_name(s: &str) -> W3dName {
    let mut name = [0u8; W3D_NAME_LEN];
    let bytes = s.as_bytes();
    let len = bytes.len().min(W3D_NAME_LEN - 1);
    name[..len].copy_from_slice(&bytes[..len]);
    name
}

/// Complete W3D file data
#[derive(Debug, Default)]
pub struct W3dFile {
    pub meshes: Vec<W3dMesh>,
    pub hierarchies: Vec<W3dHierarchy>,
    pub animations: Vec<W3dAnimation>,
    pub hmodels: Vec<W3dHModel>,
    pub lod_models: Vec<W3dLodModel>,
    pub lights: Vec<W3dLight>,
}

/// 3D vector structure
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct W3dVector {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl W3dVector {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub fn zero() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub fn to_vec3(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    pub fn to_point(&self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

impl From<Vec3> for W3dVector {
    fn from(v: Vec3) -> Self {
        Self {
            x: v.x,
            y: v.y,
            z: v.z,
        }
    }
}

/// Quaternion structure
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct W3dQuaternion {
    pub q: [f32; 4], // [x, y, z, w]
}

impl W3dQuaternion {
    pub fn new(x: f32, y: f32, z: f32, w: f32) -> Self {
        Self { q: [x, y, z, w] }
    }

    pub fn identity() -> Self {
        Self {
            q: [0.0, 0.0, 0.0, 1.0],
        }
    }

    pub fn to_quat(&self) -> Quat {
        // Stored as [x, y, z, w]
        Quat::from_xyzw(self.q[0], self.q[1], self.q[2], self.q[3])
    }

    pub fn to_unit_quaternion(&self) -> Quat {
        self.to_quat().normalize()
    }
}

impl From<Quat> for W3dQuaternion {
    fn from(q: Quat) -> Self {
        Self {
            q: [q.x, q.y, q.z, q.w],
        }
    }
}

/// Texture coordinate structure
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct W3dTexCoord {
    pub u: f32,
    pub v: f32,
}

impl W3dTexCoord {
    pub fn new(u: f32, v: f32) -> Self {
        Self { u, v }
    }
}

/// RGB color structure with padding
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[repr(C)]
pub struct W3dRgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub pad: u8,
}

impl W3dRgb {
    pub fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b, pad: 0 }
    }

    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            pad: 0,
        }
    }

    pub fn white() -> Self {
        Self {
            r: 255,
            g: 255,
            b: 255,
            pad: 0,
        }
    }

    pub fn to_f32_array(&self) -> [f32; 3] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
        ]
    }

    pub fn to_f32_array_alpha(&self, alpha: f32) -> [f32; 4] {
        [
            self.r as f32 / 255.0,
            self.g as f32 / 255.0,
            self.b as f32 / 255.0,
            alpha,
        ]
    }
}

/// Triangle structure (Version 3)
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dTriangle {
    pub v_indices: [u32; 3],
    pub attributes: u32,
    pub normal: W3dVector,
    pub dist: f32,
}

impl W3dTriangle {
    pub fn new(v0: u32, v1: u32, v2: u32) -> Self {
        Self {
            v_indices: [v0, v1, v2],
            attributes: 0,
            normal: W3dVector::zero(),
            dist: 0.0,
        }
    }
}

/// Mesh header structure (Version 3) — matches C++ `W3dMeshHeader3Struct`
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct W3dMeshHeader3 {
    pub version: u32,
    pub attributes: u32,
    pub mesh_name: W3dName,
    pub container_name: W3dName,
    pub num_tris: u32,
    pub num_vertices: u32,
    pub num_materials: u32,
    pub num_damage_stages: u32,
    pub sort_level: i32,
    pub prelit_version: u32,
    pub future_counts: [u32; 1],
    pub vertex_channels: u32,
    pub face_channels: u32,
    pub min: W3dVector,
    pub max: W3dVector,
    pub sph_center: W3dVector,
    pub sph_radius: f32,
}

/// Complete mesh structure
#[derive(Debug, Default)]
pub struct W3dMesh {
    pub header: W3dMeshHeader3,
    pub vertices: Vec<W3dVector>,
    pub normals: Vec<W3dVector>,
    pub tex_coords: Vec<W3dTexCoord>,
    pub vertex_colors: Vec<W3dRgb>,
    pub triangles: Vec<W3dTriangle>,
    pub materials: Vec<W3dMaterial3>,
    pub shaders: Vec<W3dShader>,
    pub vertex_materials: Vec<W3dVertexMaterial>,
    pub textures: Vec<W3dTexture>,
    pub material_passes: Vec<W3dMaterialPass>,
    pub vertex_influences: Vec<W3dVertexInfluence>,
    pub user_text: String,
    // Optional morph targets derived from Deform sets
    pub morph_targets: Vec<W3dMorphTarget>,
    pub aabtree: Option<W3dAABTree>,
    // Optional per-triangle material indices (maps each triangle to material index)
    pub per_tri_materials: Option<Vec<u32>>,
}

impl W3dMesh {
    pub fn mesh_name(&self) -> String {
        w3d_name_to_string(&self.header.mesh_name)
    }

    pub fn container_name(&self) -> String {
        w3d_name_to_string(&self.header.container_name)
    }
}

/// Material structure (Version 3)
#[derive(Debug, Clone, Default)]
pub struct W3dMaterial3 {
    pub name: String,
    pub info: W3dMaterial3Info,
    pub maps: HashMap<String, W3dMap3>, // Map type -> Map data
}

/// Material properties structure
#[derive(Debug, Clone, Default)]
#[repr(C)]
pub struct W3dMaterial3Info {
    pub attributes: u32,
    pub diffuse_color: W3dRgb,
    pub specular_color: W3dRgb,
    pub emissive_coefficients: W3dRgb,
    pub ambient_coefficients: W3dRgb,
    pub diffuse_coefficients: W3dRgb,
    pub specular_coefficients: W3dRgb,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
    pub fog_coeff: f32,
}

/// Texture map structure
#[derive(Debug, Clone, Default)]
pub struct W3dMap3 {
    pub filename: String,
    pub info: W3dMap3Info,
}

/// Texture map information
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dMap3Info {
    pub mapping_type: u16,
    pub frame_count: u16,
    pub frame_rate: f32,
}

/// Shader structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dShader {
    pub depth_compare: u8,
    pub depth_mask: u8,
    pub color_mask: u8,
    pub dest_blend: u8,
    pub fog_func: u8,
    pub pri_gradient: u8,
    pub sec_gradient: u8,
    pub src_blend: u8,
    pub texturing: u8,
    pub detail_color_func: u8,
    pub detail_alpha_func: u8,
    pub shader_preset: u8,
    pub alpha_test: u8,
    pub post_detail_color_func: u8,
    pub post_detail_alpha_func: u8,
    pub pad: u8,
}

/// Vertex material structure
#[derive(Debug, Clone, Default)]
pub struct W3dVertexMaterial {
    pub name: String,
    pub info: W3dVertexMaterialInfo,
}

/// Vertex material information
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dVertexMaterialInfo {
    pub attributes: u32,
    pub ambient: W3dRgb,
    pub diffuse: W3dRgb,
    pub specular: W3dRgb,
    pub emissive: W3dRgb,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
}

/// Texture structure
#[derive(Debug, Clone, Default)]
pub struct W3dTexture {
    pub name: String,
    pub info: W3dTextureInfo,
}

/// Texture information
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dTextureInfo {
    pub attributes: u16,
    pub animation_type: u16,
    pub frame_count: u32,
    pub frame_rate: f32,
}

/// Material pass structure
#[derive(Debug, Clone, Default)]
pub struct W3dMaterialPass {
    pub vertex_material_ids: Vec<u32>,
    pub shader_ids: Vec<u32>,
    pub texture_stages: Vec<W3dTextureStage>,
}

/// Texture stage structure
#[derive(Debug, Clone, Default)]
pub struct W3dTextureStage {
    pub texture_ids: Vec<u32>,
    pub per_face_texcoord_ids: Vec<u32>,
    pub tex_coords: Vec<W3dTexCoord>,
}

/// Vertex influence structure for skinning
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dVertexInfluence {
    pub bone_idx: u16,
    pub pad: [u8; 6],
}

/// Hierarchy structure
#[derive(Debug, Default)]
pub struct W3dHierarchy {
    pub header: W3dHierarchyHeader,
    pub pivots: Vec<W3dPivot>,
    pub pivot_fixups: Vec<W3dPivotFixup>,
}

impl W3dHierarchy {
    pub fn name(&self) -> String {
        w3d_name_to_string(&self.header.name)
    }
}

/// Hierarchy header structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dHierarchyHeader {
    pub version: u32,
    pub name: W3dName,
    pub num_pivots: u32,
    pub center: W3dVector,
}

/// Pivot (bone) structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dPivot {
    pub name: W3dName,
    pub parent_idx: u32,
    pub translation: W3dVector,
    pub euler_angles: W3dVector,
    pub rotation: W3dQuaternion,
}

impl W3dPivot {
    pub fn name(&self) -> String {
        w3d_name_to_string(&self.name)
    }

    pub fn is_root(&self) -> bool {
        self.parent_idx == 0xFFFFFFFF
    }
}

/// Pivot fixup matrix structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dPivotFixup {
    pub tm: [[f32; 3]; 4], // 3x4 matrix
}

/// Animation structure
#[derive(Debug, Default)]
pub struct W3dAnimation {
    pub header: W3dAnimationHeader,
    pub channels: Vec<W3dAnimationChannel>,
    pub bit_channels: Vec<W3dBitChannel>,
    // Densified channels originating from timecoded or adaptive delta formats
    pub extra_channels: Vec<W3dAnimationChannel>,
    // Morph animation data: pivot -> (target index, (frame, weight) keys)
    pub morph_tracks: Vec<W3dMorphTrack>,
}

impl W3dAnimation {
    pub fn name(&self) -> String {
        w3d_name_to_string(&self.header.name)
    }

    pub fn hierarchy_name(&self) -> String {
        w3d_name_to_string(&self.header.hierarchy_name)
    }
}

/// Animation header structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dAnimationHeader {
    pub version: u32,
    pub name: W3dName,
    pub hierarchy_name: W3dName,
    pub num_frames: u32,
    pub frame_rate: u32,
}

/// Animation channel structure
#[derive(Debug, Clone, Default)]
pub struct W3dAnimationChannel {
    pub first_frame: u16,
    pub last_frame: u16,
    pub vector_len: u16,
    pub flags: u16,
    pub pivot: u16,
    pub data: Vec<f32>,
}

impl W3dAnimationChannel {
    pub fn frame_count(&self) -> usize {
        (self.last_frame - self.first_frame + 1) as usize
    }

    pub fn total_elements(&self) -> usize {
        self.frame_count() * self.vector_len as usize
    }
}

/// Bit channel structure for boolean animation
#[derive(Debug, Clone, Default)]
pub struct W3dBitChannel {
    pub first_frame: u16,
    pub last_frame: u16,
    pub flags: u16,
    pub pivot: u16,
    pub default_val: u8,
    pub data: Vec<u8>,
}

impl W3dBitChannel {
    pub fn frame_count(&self) -> usize {
        (self.last_frame - self.first_frame + 1) as usize
    }

    pub fn data_bytes_needed(&self) -> usize {
        (self.frame_count() + 7) / 8
    }
}

/// HModel structure
#[derive(Debug, Default)]
pub struct W3dHModel {
    pub header: W3dHModelHeader,
    pub nodes: Vec<W3dHModelNode>,
    pub collision_nodes: Vec<W3dHModelNode>,
    pub skin_nodes: Vec<W3dHModelNode>,
    pub shadow_nodes: Vec<W3dHModelNode>,
    pub aux_data: Option<W3dHModelAuxData>,
}

impl W3dHModel {
    pub fn name(&self) -> String {
        w3d_name_to_string(&self.header.name)
    }

    pub fn hierarchy_name(&self) -> String {
        w3d_name_to_string(&self.header.hierarchy_name)
    }
}

/// HModel header structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dHModelHeader {
    pub version: u32,
    pub name: W3dName,
    pub hierarchy_name: W3dName,
    pub num_connections: u16,
}

/// HModel node structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dHModelNode {
    pub render_obj_name: W3dName,
    pub pivot_idx: u16,
}

impl W3dHModelNode {
    pub fn render_obj_name(&self) -> String {
        w3d_name_to_string(&self.render_obj_name)
    }
}

/// HModel auxiliary data structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dHModelAuxData {
    pub attributes: u32,
    pub mesh_count: u32,
    pub collision_count: u32,
    pub skin_count: u32,
    pub shadow_count: u32,
    pub future_counts: [u32; 7],
    pub lod_min: f32,
    pub lod_max: f32,
    pub future_use: [u32; 32],
}

/// LOD Model structure
#[derive(Debug, Default)]
pub struct W3dLodModel {
    pub header: W3dLodModelHeader,
    pub lods: Vec<W3dLod>,
}

impl W3dLodModel {
    pub fn name(&self) -> String {
        w3d_name_to_string(&self.header.name)
    }
}

/// LOD Model header structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dLodModelHeader {
    pub version: u32,
    pub name: W3dName,
    pub num_lods: u16,
}

/// LOD structure
#[derive(Debug, Clone, Default)]
pub struct W3dLod {
    pub render_obj_name: String, // 2 * W3D_NAME_LEN in C++
    pub lod_min: f32,
    pub lod_max: f32,
}

/// Damage information structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dMeshDamage {
    pub num_damage_materials: u32,
    pub num_damage_verts: u32,
    pub num_damage_colors: u32,
    pub damage_index: u32,
    pub future_use: [u32; 4],
}

/// Damage vertex structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dMeshDamageVertex {
    pub vertex_index: u32,
    pub new_vertex: W3dVector,
}

/// Single morph target derived from a Deform keyframe
#[derive(Debug, Clone, Default)]
pub struct W3dMorphTarget {
    pub name: String,
    pub deltas: Vec<[f32; 3]>,
    pub weight: f32,
}

#[derive(Debug, Clone, Default)]
pub struct W3dMorphTrackKey {
    pub frame: u32,
    pub weight: f32,
}

#[derive(Debug, Clone, Default)]
pub struct W3dMorphTrack {
    pub pivot: u16,
    pub target_name: String,
    pub keys: Vec<W3dMorphTrackKey>,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dAABTreeHeader {
    pub node_count: u32,
    pub poly_count: u32,
    pub padding: [u32; 6],
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dAABTreeNode {
    pub min: W3dVector,
    pub max: W3dVector,
    pub front_or_poly0: u32,
    pub back_or_poly_count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct W3dAABTree {
    pub header: W3dAABTreeHeader,
    pub poly_indices: Vec<u32>,
    pub nodes: Vec<W3dAABTreeNode>,
}

#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dRgbF {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

#[derive(Debug, Clone, Default)]
pub struct W3dLight {
    pub kind: W3dLightKind,
    pub color: W3dRgb,
    pub intensity: f32,
    pub spot_angle: Option<f32>,
    pub spot_exponent: Option<f32>,
    pub position: Option<W3dVector>,
    pub direction: Option<W3dVector>,
    pub range: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3dLightKind {
    Point,
    Directional,
    Spot,
}

impl Default for W3dLightKind {
    fn default() -> Self {
        W3dLightKind::Point
    }
}

/// Damage color structure
#[derive(Debug, Clone, Copy, Default)]
#[repr(C)]
pub struct W3dMeshDamageColor {
    pub vertex_index: u32,
    pub new_color: W3dRgb,
}
