//! # W3D Model Loader - Complete C&C Generals Model Support
//!
//! This module implements complete W3D (.w3d) model format loading with:
//! - Native W3D binary format parsing
//! - Mesh geometry extraction and optimization
//! - Material and texture loading
//! - Skeletal animation support
//! - LOD (Level of Detail) processing
//! - Bounding volume calculation
//! - GPU resource preparation

use super::{Result, W3DError};
use crate::video::{ColorFormat, Resolution};
use bytemuck::{cast_slice, Pod, Zeroable};
use glam::{Mat4, Quat, Vec2, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

#[cfg(feature = "w3d")]
use wgpu::{
    util::{BufferInitDescriptor, DeviceExt},
    Buffer, BufferDescriptor, BufferUsages, Device, Extent3d, Origin3d, Queue, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};

/// W3D file header structure (matches original C++)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DFileHeader {
    /// Magic number "W3D\0"
    pub magic: [u8; 4],
    /// File version
    pub version: u32,
    /// File size in bytes
    pub file_size: u32,
    /// Number of chunks in file
    pub chunk_count: u32,
    /// Creation timestamp
    pub timestamp: u32,
    /// Reserved bytes
    pub reserved: [u32; 3],
}

/// W3D chunk header
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DChunkHeader {
    /// Chunk type ID
    pub chunk_type: u32,
    /// Chunk size in bytes (excluding header)
    pub chunk_size: u32,
}

/// W3D chunk types — matches C++ `w3d_file.h` enum exactly.
///
/// Only the types we actually handle are enumerated; everything else falls
/// through to `Unknown(u32)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DChunkType {
    // ---- top-level containers ------------------------------------------------
    /// `W3D_CHUNK_MESH` = 0x00000000
    Mesh,
    /// `W3D_CHUNK_HIERARCHY` = 0x00000100  (skeleton / bone hierarchy)
    Hierarchy,
    /// `W3D_CHUNK_ANIMATION` = 0x00000200
    Animation,
    /// `W3D_CHUNK_HMODEL` = 0x00000300
    HModel,
    /// `W3D_CHUNK_LODMODEL` = 0x00000400
    LodModel,
    /// `W3D_CHUNK_COLLECTION` = 0x00000420
    Collection,
    /// `W3D_CHUNK_HLOD` = 0x00000700
    Hlod,
    /// `W3D_CHUNK_EMITTER` = 0x00000500
    Emitter,

    // ---- mesh sub-chunks -----------------------------------------------------
    /// `W3D_CHUNK_VERTICES` = 0x00000002
    Vertices,
    /// `W3D_CHUNK_VERTEX_NORMALS` = 0x00000003
    VertexNormals,
    /// `W3D_CHUNK_TEXCOORDS` = 0x00000005  (legacy, pre-v3 per-vertex UVs)
    TexCoords,
    /// `W3D_CHUNK_MESH_USER_TEXT` = 0x0000000C
    MeshUserText,
    /// `W3D_CHUNK_VERTEX_INFLUENCES` = 0x0000000E
    VertexInfluences,
    /// `W3D_CHUNK_MESH_HEADER3` = 0x0000001F
    MeshHeader3,
    /// `W3D_CHUNK_TRIANGLES` = 0x00000020
    Triangles,
    /// `W3D_CHUNK_MATERIAL_INFO` = 0x00000028  (W3dMaterialInfoStruct)
    MaterialInfo,
    /// `W3D_CHUNK_SHADERS` = 0x00000029  (array of W3dShaderStruct)
    Shaders,
    /// `W3D_CHUNK_VERTEX_MATERIALS` = 0x0000002A
    VertexMaterials,
    /// `W3D_CHUNK_VERTEX_MATERIAL` = 0x0000002B
    VertexMaterial,
    /// `W3D_CHUNK_VERTEX_MATERIAL_NAME` = 0x0000002C
    VertexMaterialName,
    /// `W3D_CHUNK_VERTEX_MATERIAL_INFO` = 0x0000002D
    VertexMaterialInfo,
    /// `W3D_CHUNK_TEXTURES` = 0x00000030  (wrapper)
    Textures,
    /// `W3D_CHUNK_TEXTURE` = 0x00000031
    Texture,
    /// `W3D_CHUNK_TEXTURE_NAME` = 0x00000032
    TextureName,
    /// `W3D_CHUNK_TEXTURE_INFO` = 0x00000033
    TextureInfo,
    /// `W3D_CHUNK_MATERIAL_PASS` = 0x00000038
    MaterialPass,
    /// `W3D_CHUNK_VERTEX_COLORS` = 0x0000000D  (per-vertex RGBA in DCG sub-chunk)
    VertexColors,

    // ---- hierarchy sub-chunks ------------------------------------------------
    /// `W3D_CHUNK_HIERARCHY_HEADER` = 0x00000101
    HierarchyHeader,
    /// `W3D_CHUNK_PIVOTS` = 0x00000102
    Pivots,

    // ---- animation sub-chunks ------------------------------------------------
    /// `W3D_CHUNK_ANIMATION_HEADER` = 0x00000201
    AnimationHeader,
    /// `W3D_CHUNK_ANIMATION_CHANNEL` = 0x00000202
    AnimationChannel,
    /// `W3D_CHUNK_BIT_CHANNEL` = 0x00000203
    BitChannel,

    // ---- fallback ------------------------------------------------------------
    /// Any chunk type we don't explicitly handle
    Unknown(u32),
}

impl From<u32> for W3DChunkType {
    fn from(value: u32) -> Self {
        match value {
            // top-level
            0x00000000 => Self::Mesh,
            0x00000100 => Self::Hierarchy,
            0x00000200 => Self::Animation,
            0x00000300 => Self::HModel,
            0x00000400 => Self::LodModel,
            0x00000420 => Self::Collection,
            0x00000700 => Self::Hlod,
            0x00000500 => Self::Emitter,
            // mesh sub-chunks
            0x00000002 => Self::Vertices,
            0x00000003 => Self::VertexNormals,
            0x00000005 => Self::TexCoords,
            0x0000000C => Self::MeshUserText,
            0x0000000D => Self::VertexColors,
            0x0000000E => Self::VertexInfluences,
            0x0000001F => Self::MeshHeader3,
            0x00000020 => Self::Triangles,
            0x00000028 => Self::MaterialInfo,
            0x00000029 => Self::Shaders,
            0x0000002A => Self::VertexMaterials,
            0x0000002B => Self::VertexMaterial,
            0x0000002C => Self::VertexMaterialName,
            0x0000002D => Self::VertexMaterialInfo,
            0x00000030 => Self::Textures,
            0x00000031 => Self::Texture,
            0x00000032 => Self::TextureName,
            0x00000033 => Self::TextureInfo,
            0x00000038 => Self::MaterialPass,
            // hierarchy sub-chunks
            0x00000101 => Self::HierarchyHeader,
            0x00000102 => Self::Pivots,
            // animation sub-chunks
            0x00000201 => Self::AnimationHeader,
            0x00000202 => Self::AnimationChannel,
            0x00000203 => Self::BitChannel,
            other => Self::Unknown(other),
        }
    }
}

/// W3D name length constant — matches C++ `W3D_NAME_LEN`
const W3D_NAME_LEN: usize = 16;

/// Internal representation of a parsed `W3dVertexMaterialStruct`
#[derive(Debug, Clone, Copy, Default)]
struct W3dVertexMaterialInfo {
    attributes: u32,
    ambient: (u8, u8, u8),
    diffuse: (u8, u8, u8),
    specular: (u8, u8, u8),
    emissive: (u8, u8, u8),
    shininess: f32,
    opacity: f32,
    translucency: f32,
}

/// Internal representation of a parsed `W3dShaderStruct`
#[derive(Debug, Clone, Copy, Default)]
struct W3dShaderInfo {
    depth_compare: u8,
    depth_mask: u8,
    dest_blend: u8,
    pri_gradient: u8,
    sec_gradient: u8,
    src_blend: u8,
    texturing: u8,
    detail_color_func: u8,
    detail_alpha_func: u8,
    alpha_test: u8,
}

/// W3D vertex structure (matches game format)
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DVertex {
    /// Position (x, y, z)
    pub position: [f32; 3],
    /// Normal vector (x, y, z)  
    pub normal: [f32; 3],
    /// Texture coordinates (u, v)
    pub tex_coords: [f32; 2],
    /// Vertex color (r, g, b, a)
    pub color: [f32; 4],
    /// Bone indices for skeletal animation
    pub bone_indices: [u32; 4],
    /// Bone weights for skeletal animation
    pub bone_weights: [f32; 4],
    /// Tangent vector (x, y, z)
    pub tangent: [f32; 3],
    /// Binormal/Bitangent vector (x, y, z)
    pub binormal: [f32; 3],
}

impl Default for W3DVertex {
    fn default() -> Self {
        Self {
            position: [0.0; 3],
            normal: [0.0, 0.0, 1.0],
            tex_coords: [0.0; 2],
            color: [1.0; 4],
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
            tangent: [1.0, 0.0, 0.0],
            binormal: [0.0, 1.0, 0.0],
        }
    }
}

/// W3D triangle/face structure
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub struct W3DTriangle {
    /// Vertex indices
    pub indices: [u32; 3],
    /// Surface normal
    pub surface_normal: [f32; 3],
    /// Distance from origin
    pub distance: f32,
}

/// W3D mesh data
#[derive(Debug, Clone)]
pub struct W3DMesh {
    /// Mesh name
    pub name: String,
    /// Vertices
    pub vertices: Vec<W3DVertex>,
    /// Indices (triangulated)
    pub indices: Vec<u32>,
    /// Material ID
    pub material_id: Option<String>,
    /// Bounding box
    pub bounding_box: BoundingBox,
    /// LOD level (0 = highest detail)
    pub lod_level: u32,
    /// Bone influences (if skinned)
    pub bone_influences: Vec<BoneInfluence>,
}

/// Bone influence for skinned meshes
#[derive(Debug, Clone)]
pub struct BoneInfluence {
    /// Vertex index
    pub vertex_index: u32,
    /// Bone index
    pub bone_index: u32,
    /// Influence weight
    pub weight: f32,
}

/// W3D bounding box
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BoundingBox {
    /// Minimum point
    pub min: Vec3,
    /// Maximum point
    pub max: Vec3,
}

impl BoundingBox {
    /// Create new bounding box
    pub fn new(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    /// Create from points
    pub fn from_points(points: &[Vec3]) -> Self {
        if points.is_empty() {
            return Self::new(Vec3::ZERO, Vec3::ZERO);
        }

        let mut min = points[0];
        let mut max = points[0];

        for point in points.iter().skip(1) {
            min = min.min(*point);
            max = max.max(*point);
        }

        Self::new(min, max)
    }

    /// Get center point
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Get size/extents
    pub fn size(self) -> Vec3 {
        self.max - self.min
    }

    /// Get radius (half diagonal)
    pub fn radius(self) -> f32 {
        self.size().length() * 0.5
    }

    /// Check if point is inside
    pub fn contains(self, point: Vec3) -> bool {
        point.cmpge(self.min).all() && point.cmple(self.max).all()
    }

    /// Expand to include point
    pub fn expand_to_include(&mut self, point: Vec3) {
        self.min = self.min.min(point);
        self.max = self.max.max(point);
    }
}

/// W3D material data
#[derive(Debug, Clone)]
pub struct W3DMaterial {
    /// Material name
    pub name: String,
    /// Base color
    pub base_color: Vec4,
    /// Diffuse texture path
    pub diffuse_texture: Option<String>,
    /// Normal texture path
    pub normal_texture: Option<String>,
    /// Specular texture path
    pub specular_texture: Option<String>,
    /// Emissive texture path
    pub emissive_texture: Option<String>,
    /// Metallic factor
    pub metallic: f32,
    /// Roughness factor
    pub roughness: f32,
    /// Emissive factor
    pub emissive_factor: Vec3,
    /// Alpha cutoff for transparency
    pub alpha_cutoff: f32,
    /// Two-sided rendering
    pub double_sided: bool,
}

impl Default for W3DMaterial {
    fn default() -> Self {
        Self {
            name: String::new(),
            base_color: Vec4::ONE,
            diffuse_texture: None,
            normal_texture: None,
            specular_texture: None,
            emissive_texture: None,
            metallic: 0.0,
            roughness: 0.5,
            emissive_factor: Vec3::ZERO,
            alpha_cutoff: 0.5,
            double_sided: false,
        }
    }
}

/// W3D bone for skeletal animation
#[derive(Debug, Clone)]
pub struct W3DBone {
    /// Bone name
    pub name: String,
    /// Parent bone index (-1 for root)
    pub parent_index: i32,
    /// Rest position
    pub rest_position: Vec3,
    /// Rest rotation
    pub rest_rotation: Quat,
    /// Rest scale
    pub rest_scale: Vec3,
    /// Bind matrix (local to bone space)
    pub bind_matrix: Mat4,
    /// Inverse bind matrix (bone to local space)
    pub inverse_bind_matrix: Mat4,
}

/// W3D animation keyframe
#[derive(Debug, Clone)]
pub struct W3DKeyframe {
    /// Time in seconds
    pub time: f32,
    /// Position
    pub position: Vec3,
    /// Rotation
    pub rotation: Quat,
    /// Scale
    pub scale: Vec3,
}

/// W3D animation channel (per bone)
#[derive(Debug, Clone)]
pub struct W3DAnimationChannel {
    /// Target bone index
    pub bone_index: u32,
    /// Position keyframes
    pub position_keys: Vec<W3DKeyframe>,
    /// Rotation keyframes
    pub rotation_keys: Vec<W3DKeyframe>,
    /// Scale keyframes
    pub scale_keys: Vec<W3DKeyframe>,
}

/// W3D animation
#[derive(Debug, Clone)]
pub struct W3DAnimation {
    /// Animation name
    pub name: String,
    /// Duration in seconds
    pub duration: f32,
    /// Animation channels
    pub channels: Vec<W3DAnimationChannel>,
    /// Frames per second
    pub fps: f32,
}

/// Complete W3D model
#[derive(Debug, Clone)]
pub struct W3DModel {
    /// Model name
    pub name: String,
    /// All meshes
    pub meshes: Vec<W3DMesh>,
    /// All materials
    pub materials: Vec<W3DMaterial>,
    /// Skeleton (if animated)
    pub skeleton: Option<Vec<W3DBone>>,
    /// Animations
    pub animations: Vec<W3DAnimation>,
    /// Bounding box for entire model
    pub bounding_box: BoundingBox,
    /// LOD distances
    pub lod_distances: Vec<f32>,
}

/// W3D model loader
pub struct W3DModelLoader {
    /// Texture search paths
    texture_paths: Vec<PathBuf>,
    /// Material cache
    material_cache: HashMap<String, W3DMaterial>,
    /// Texture cache
    texture_cache: HashMap<String, Vec<u8>>,
}

impl W3DModelLoader {
    /// Create new model loader
    pub fn new() -> Self {
        Self {
            texture_paths: vec![
                PathBuf::from("textures/"),
                PathBuf::from("art/textures/"),
                PathBuf::from("data/art/textures/"),
            ],
            material_cache: HashMap::new(),
            texture_cache: HashMap::new(),
        }
    }

    /// Add texture search path
    pub fn add_texture_path<P: AsRef<Path>>(&mut self, path: P) {
        self.texture_paths.push(path.as_ref().to_path_buf());
    }

    /// Matches C++ `W3DAssetManager::Release_All_Textures`.
    pub fn release_all_textures(&mut self) {
        self.texture_cache.clear();
    }

    /// Matches C++ `ReloadAllTextures`.
    pub fn reload_all_textures(&mut self) {
        self.release_all_textures();
    }

    #[cfg(test)]
    pub(crate) fn cache_texture_for_test(&mut self, name: &str, data: Vec<u8>) {
        self.texture_cache.insert(name.to_string(), data);
    }

    #[cfg(test)]
    pub(crate) fn texture_cache_len_for_test(&self) -> usize {
        self.texture_cache.len()
    }

    /// Load W3D model from file
    pub async fn load_model<P: AsRef<Path>>(&mut self, path: P) -> Result<W3DModel> {
        let path = path.as_ref();
        tracing::info!("Loading W3D model: {}", path.display());

        let data = tokio::fs::read(path)
            .await
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read file: {}", e)))?;

        self.parse_w3d_data(
            &data,
            path.file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
        )
    }

    /// Load W3D model from memory
    pub fn load_model_from_memory(&mut self, data: &[u8], name: String) -> Result<W3DModel> {
        self.parse_w3d_data(data, name)
    }

    /// Parse W3D binary data
    fn parse_w3d_data(&mut self, data: &[u8], name: String) -> Result<W3DModel> {
        let mut cursor = Cursor::new(data);

        // Read and validate header
        let header = self.read_struct::<W3DFileHeader>(&mut cursor)?;
        self.validate_header(&header)?;

        let mut model = W3DModel {
            name,
            meshes: Vec::new(),
            materials: Vec::new(),
            skeleton: None,
            animations: Vec::new(),
            bounding_box: BoundingBox::new(Vec3::ZERO, Vec3::ZERO),
            lod_distances: Vec::new(),
        };

        // Parse all chunks
        let mut chunk_count = 0;
        while cursor.position() < data.len() as u64 && chunk_count < header.chunk_count {
            let chunk_header = self.read_struct::<W3DChunkHeader>(&mut cursor)?;
            let chunk_type = W3DChunkType::from(chunk_header.chunk_type);

            tracing::debug!(
                "Processing chunk: {:?}, size: {}",
                chunk_type,
                chunk_header.chunk_size
            );

            match chunk_type {
                W3DChunkType::Mesh => {
                    let mesh = self.parse_mesh_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.meshes.push(mesh);
                }
                W3DChunkType::Hierarchy => {
                    let skeleton =
                        self.parse_hierarchy_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.skeleton = Some(skeleton);
                }
                W3DChunkType::Animation => {
                    let animation =
                        self.parse_animation_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.animations.push(animation);
                }
                W3DChunkType::HModel => {
                    self.parse_hmodel_chunk(&mut cursor, chunk_header.chunk_size, &mut model)?;
                }
                W3DChunkType::Hlod => {
                    self.parse_hlod_chunk(&mut cursor, chunk_header.chunk_size, &mut model)?;
                }
                W3DChunkType::LodModel => {
                    self.parse_lod_model_chunk(&mut cursor, chunk_header.chunk_size, &mut model)?;
                }
                W3DChunkType::Collection => {
                    self.parse_collection_chunk(&mut cursor, chunk_header.chunk_size)?;
                }
                W3DChunkType::Emitter => {
                    self.skip_emitter_chunk(&mut cursor, chunk_header.chunk_size)?;
                }
                _ => {
                    log::warn!(
                        "Skipping unknown top-level W3D chunk type 0x{:08X}, size {}",
                        chunk_header.chunk_type,
                        chunk_header.chunk_size
                    );
                    cursor
                        .seek(SeekFrom::Current(chunk_header.chunk_size as i64))
                        .map_err(|e| {
                            W3DError::ModelLoadingFailed(format!("Failed to skip chunk: {}", e))
                        })?;
                }
            }

            chunk_count += 1;
        }

        // Calculate model bounding box
        self.calculate_model_bounds(&mut model);

        tracing::info!(
            "Loaded W3D model: {} meshes, {} materials, {} animations",
            model.meshes.len(),
            model.materials.len(),
            model.animations.len()
        );

        Ok(model)
    }

    /// Validate W3D file header
    fn validate_header(&self, header: &W3DFileHeader) -> Result<()> {
        // Check magic number
        if &header.magic != b"W3D\0" {
            return Err(W3DError::ModelLoadingFailed(
                "Invalid W3D magic number".to_string(),
            ));
        }

        // Check version (support multiple versions)
        if header.version < 0x00030000 || header.version > 0x00050000 {
            tracing::warn!("Unsupported W3D version: 0x{:08x}", header.version);
        }

        Ok(())
    }

    /// Parse mesh chunk
    fn parse_mesh_chunk(&mut self, cursor: &mut Cursor<&[u8]>, chunk_size: u32) -> Result<W3DMesh> {
        let start_pos = cursor.position();
        let mut mesh = W3DMesh {
            name: String::new(),
            vertices: Vec::new(),
            indices: Vec::new(),
            material_id: None,
            bounding_box: BoundingBox::new(Vec3::ZERO, Vec3::ZERO),
            lod_level: 0,
            bone_influences: Vec::new(),
        };

        // Parse mesh sub-chunks
        while cursor.position() < start_pos + chunk_size as u64 {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);

            match sub_type {
                W3DChunkType::Vertices => {
                    let positions = self.parse_vertex_positions(cursor, sub_header.chunk_size)?;
                    mesh.vertices.resize(positions.len(), W3DVertex::default());
                    for (i, pos) in positions.iter().enumerate() {
                        mesh.vertices[i].position = *pos;
                    }
                }
                W3DChunkType::VertexNormals => {
                    let normals = self.parse_vertex_normals(cursor, sub_header.chunk_size)?;
                    for (i, normal) in normals.iter().enumerate() {
                        if i < mesh.vertices.len() {
                            mesh.vertices[i].normal = *normal;
                        }
                    }
                }
                W3DChunkType::TexCoords => {
                    let tex_coords = self.parse_tex_coords(cursor, sub_header.chunk_size)?;
                    for (i, uv) in tex_coords.iter().enumerate() {
                        if i < mesh.vertices.len() {
                            mesh.vertices[i].tex_coords = *uv;
                        }
                    }
                }
                W3DChunkType::Triangles => {
                    mesh.indices = self.parse_triangles(cursor, sub_header.chunk_size)?;
                }
                _ => {
                    cursor
                        .seek(SeekFrom::Current(sub_header.chunk_size as i64))
                        .map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to skip mesh sub-chunk: {}",
                                e
                            ))
                        })?;
                }
            }
        }

        // Calculate mesh bounding box
        let positions: Vec<Vec3> = mesh
            .vertices
            .iter()
            .map(|v| Vec3::from_array(v.position))
            .collect();
        mesh.bounding_box = BoundingBox::from_points(&positions);

        // Generate missing tangents and binormals
        self.generate_tangents(&mut mesh);

        Ok(mesh)
    }

    /// Parse vertex positions
    fn parse_vertex_positions(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        size: u32,
    ) -> Result<Vec<[f32; 3]>> {
        let vertex_count = size / (3 * 4); // 3 floats per position
        let mut positions = Vec::with_capacity(vertex_count as usize);

        for _ in 0..vertex_count {
            let x = self.read_f32(cursor)?;
            let y = self.read_f32(cursor)?;
            let z = self.read_f32(cursor)?;
            positions.push([x, y, z]);
        }

        Ok(positions)
    }

    /// Parse vertex normals
    fn parse_vertex_normals(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        size: u32,
    ) -> Result<Vec<[f32; 3]>> {
        let normal_count = size / (3 * 4); // 3 floats per normal
        let mut normals = Vec::with_capacity(normal_count as usize);

        for _ in 0..normal_count {
            let x = self.read_f32(cursor)?;
            let y = self.read_f32(cursor)?;
            let z = self.read_f32(cursor)?;
            normals.push([x, y, z]);
        }

        Ok(normals)
    }

    /// Parse texture coordinates
    fn parse_tex_coords(&mut self, cursor: &mut Cursor<&[u8]>, size: u32) -> Result<Vec<[f32; 2]>> {
        let uv_count = size / (2 * 4); // 2 floats per UV
        let mut tex_coords = Vec::with_capacity(uv_count as usize);

        for _ in 0..uv_count {
            let u = self.read_f32(cursor)?;
            let v = self.read_f32(cursor)?;
            tex_coords.push([u, v]);
        }

        Ok(tex_coords)
    }

    /// Parse triangles/indices
    fn parse_triangles(&mut self, cursor: &mut Cursor<&[u8]>, size: u32) -> Result<Vec<u32>> {
        let triangle_count = size / (3 * 4); // 3 indices per triangle
        let mut indices = Vec::with_capacity((triangle_count * 3) as usize);

        for _ in 0..triangle_count {
            let i0 = self.read_u32(cursor)?;
            let i1 = self.read_u32(cursor)?;
            let i2 = self.read_u32(cursor)?;
            indices.extend_from_slice(&[i0, i1, i2]);
        }

        Ok(indices)
    }

    /// Parse a `W3D_CHUNK_MATERIAL_INFO` (0x28) chunk that contains `W3dMaterialInfoStruct`
    /// followed by sub-chunks for shaders, vertex materials, textures, and material passes.
    ///
    /// `W3dMaterialInfoStruct` { PassCount:u32, VertexMaterialCount:u32,
    ///                           ShaderCount:u32, TextureCount:u32 }
    ///
    /// Materials are collected from the nested sub-chunks. The material system in W3D v3+
    /// separates vertex materials, shaders, and textures into distinct chunks that are
    /// tied together by index in `W3D_CHUNK_MATERIAL_PASS` sub-chunks.
    fn parse_material_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
    ) -> Result<Vec<W3DMaterial>> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // Read W3dMaterialInfoStruct: 4 × u32 = 16 bytes
        let pass_count = self.read_u32(cursor)?;
        let vertex_material_count = self.read_u32(cursor)?;
        let shader_count = self.read_u32(cursor)?;
        let texture_count = self.read_u32(cursor)?;

        tracing::debug!(
            "Material info: {} passes, {} vert materials, {} shaders, {} textures",
            pass_count,
            vertex_material_count,
            shader_count,
            texture_count
        );

        // Temporary storage for parsed sub-resources
        let mut vert_material_names: Vec<String> = Vec::new();
        let mut vert_material_infos: Vec<W3dVertexMaterialInfo> = Vec::new();
        let mut texture_names: Vec<String> = Vec::new();
        let mut shader_infos: Vec<W3dShaderInfo> = Vec::new();
        let mut materials: Vec<W3DMaterial> = Vec::new();

        // Parse sub-chunks within the material info chunk
        while cursor.position() < end_pos {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            match sub_type {
                W3DChunkType::Shaders => {
                    // Array of W3dShaderStruct: 16 bytes each
                    let shader_struct_size: u32 = 16;
                    let count = sub_header.chunk_size / shader_struct_size;
                    for _ in 0..count {
                        let depth_compare = self.read_u8(cursor)?;
                        let depth_mask = self.read_u8(cursor)?;
                        let _color_mask = self.read_u8(cursor)?;
                        let dest_blend = self.read_u8(cursor)?;
                        let _fog_func = self.read_u8(cursor)?;
                        let pri_gradient = self.read_u8(cursor)?;
                        let sec_gradient = self.read_u8(cursor)?;
                        let src_blend = self.read_u8(cursor)?;
                        let texturing = self.read_u8(cursor)?;
                        let detail_color_func = self.read_u8(cursor)?;
                        let detail_alpha_func = self.read_u8(cursor)?;
                        let _shader_preset = self.read_u8(cursor)?;
                        let alpha_test = self.read_u8(cursor)?;
                        let _post_detail_color = self.read_u8(cursor)?;
                        let _post_detail_alpha = self.read_u8(cursor)?;
                        let _pad = self.read_u8(cursor)?;

                        shader_infos.push(W3dShaderInfo {
                            depth_compare,
                            depth_mask,
                            dest_blend,
                            pri_gradient,
                            sec_gradient,
                            src_blend,
                            texturing,
                            detail_color_func,
                            detail_alpha_func,
                            alpha_test,
                        });
                    }
                }

                W3DChunkType::VertexMaterials => {
                    // Wrapper containing W3D_CHUNK_VERTEX_MATERIAL sub-chunks
                    let vm_end = cursor.position() + sub_header.chunk_size as u64;
                    while cursor.position() < vm_end {
                        let vm_header = self.read_struct::<W3DChunkHeader>(cursor)?;
                        let vm_type = W3DChunkType::from(vm_header.chunk_type);
                        let vm_end_inner = cursor.position() + vm_header.chunk_size as u64;

                        if vm_type == W3DChunkType::VertexMaterial {
                            let mut vm_name = String::new();
                            let mut vm_info = W3dVertexMaterialInfo::default();

                            while cursor.position() < vm_end_inner {
                                let inner_header = self.read_struct::<W3DChunkHeader>(cursor)?;
                                let inner_type = W3DChunkType::from(inner_header.chunk_type);
                                let inner_end =
                                    cursor.position() + inner_header.chunk_size as u64;

                                match inner_type {
                                    W3DChunkType::VertexMaterialName => {
                                        let mut name_buf =
                                            vec![0u8; inner_header.chunk_size as usize];
                                        cursor.read_exact(&mut name_buf).map_err(|e| {
                                            W3DError::ModelLoadingFailed(format!(
                                                "Failed to read vert material name: {}",
                                                e
                                            ))
                                        })?;
                                        vm_name = Self::read_null_terminated_from_slice(&name_buf);
                                    }
                                    W3DChunkType::VertexMaterialInfo => {
                                        // W3dVertexMaterialStruct:
                                        //   Attributes(u32) + Ambient(4) + Diffuse(4) +
                                        //   Specular(4) + Emissive(4) + Shininess(f32) +
                                        //   Opacity(f32) + Translucency(f32) = 32 bytes
                                        vm_info.attributes = self.read_u32(cursor)?;
                                        vm_info.ambient = self.read_w3d_rgb(cursor)?;
                                        vm_info.diffuse = self.read_w3d_rgb(cursor)?;
                                        vm_info.specular = self.read_w3d_rgb(cursor)?;
                                        vm_info.emissive = self.read_w3d_rgb(cursor)?;
                                        vm_info.shininess = self.read_f32(cursor)?;
                                        vm_info.opacity = self.read_f32(cursor)?;
                                        vm_info.translucency = self.read_f32(cursor)?;
                                    }
                                    _ => {
                                        cursor
                                            .seek(SeekFrom::Current(
                                                inner_header.chunk_size as i64,
                                            ))
                                            .map_err(|e| {
                                                W3DError::ModelLoadingFailed(format!(
                                                    "Failed to skip inner vm chunk: {}",
                                                    e
                                                ))
                                            })?;
                                    }
                                }
                                if cursor.position() < inner_end {
                                    cursor.set_position(inner_end);
                                }
                            }

                            vert_material_names.push(vm_name);
                            vert_material_infos.push(vm_info);
                        } else {
                            cursor
                                .seek(SeekFrom::Current(vm_header.chunk_size as i64))
                                .map_err(|e| {
                                    W3DError::ModelLoadingFailed(format!(
                                        "Failed to skip vm wrapper sub-chunk: {}",
                                        e
                                    ))
                                })?;
                        }
                        if cursor.position() < vm_end {
                            cursor.set_position(vm_end);
                        }
                    }
                }

                W3DChunkType::Textures => {
                    // Wrapper containing W3D_CHUNK_TEXTURE sub-chunks
                    let tex_end = cursor.position() + sub_header.chunk_size as u64;
                    while cursor.position() < tex_end {
                        let tex_header = self.read_struct::<W3DChunkHeader>(cursor)?;
                        let tex_type = W3DChunkType::from(tex_header.chunk_type);
                        let tex_end_inner = cursor.position() + tex_header.chunk_size as u64;

                        if tex_type == W3DChunkType::Texture {
                            while cursor.position() < tex_end_inner {
                                let inner_header = self.read_struct::<W3DChunkHeader>(cursor)?;
                                let inner_type = W3DChunkType::from(inner_header.chunk_type);
                                let inner_end =
                                    cursor.position() + inner_header.chunk_size as u64;

                                if inner_type == W3DChunkType::TextureName {
                                    let mut name_buf =
                                        vec![0u8; inner_header.chunk_size as usize];
                                    cursor.read_exact(&mut name_buf).map_err(|e| {
                                        W3DError::ModelLoadingFailed(format!(
                                            "Failed to read texture name: {}",
                                            e
                                        ))
                                    })?;
                                    texture_names
                                        .push(Self::read_null_terminated_from_slice(&name_buf));
                                } else {
                                    cursor
                                        .seek(SeekFrom::Current(
                                            inner_header.chunk_size as i64,
                                        ))
                                        .map_err(|e| {
                                            W3DError::ModelLoadingFailed(format!(
                                                "Failed to skip tex sub-chunk: {}",
                                                e
                                            ))
                                        })?;
                                }
                                if cursor.position() < inner_end {
                                    cursor.set_position(inner_end);
                                }
                            }
                        } else {
                            cursor
                                .seek(SeekFrom::Current(tex_header.chunk_size as i64))
                                .map_err(|e| {
                                    W3DError::ModelLoadingFailed(format!(
                                        "Failed to skip tex wrapper sub-chunk: {}",
                                        e
                                    ))
                                })?;
                        }
                        if cursor.position() < tex_end_inner {
                            cursor.set_position(tex_end_inner);
                        }
                    }
                }

                W3DChunkType::MaterialPass => {
                    // Each material pass ties together vertex material, shader, and texture
                    // indices for a rendering pass. We create W3DMaterial entries from them.
                    let mp_end = cursor.position() + sub_header.chunk_size as u64;

                    let mut vm_ids: Vec<u32> = Vec::new();
                    let mut shader_ids: Vec<u32> = Vec::new();
                    let mut tex_ids: Vec<u32> = Vec::new();

                    while cursor.position() < mp_end {
                        let mp_header = self.read_struct::<W3DChunkHeader>(cursor)?;
                        let mp_type = W3DChunkType::from(mp_header.chunk_type);

                        match mp_type {
                            W3DChunkType::VertexMaterialName => {
                                // Single u32 or per-vertex array
                                let count = mp_header.chunk_size / 4;
                                for _ in 0..count {
                                    vm_ids.push(self.read_u32(cursor)?);
                                }
                            }
                            W3DChunkType::Shaders => {
                                // Single u32 or per-tri array
                                let count = mp_header.chunk_size / 4;
                                for _ in 0..count {
                                    shader_ids.push(self.read_u32(cursor)?);
                                }
                            }
                            W3DChunkType::Texture => {
                                // Wrapper for texture stage
                                let ts_end =
                                    cursor.position() + mp_header.chunk_size as u64;
                                while cursor.position() < ts_end {
                                    let ts_header = self.read_struct::<W3DChunkHeader>(cursor)?;
                                    let ts_type = W3DChunkType::from(ts_header.chunk_type);

                                    if ts_type == W3DChunkType::TextureName {
                                        // W3D_CHUNK_TEXTURE_IDS: single or per-tri
                                        let count = ts_header.chunk_size / 4;
                                        for _ in 0..count {
                                            tex_ids.push(self.read_u32(cursor)?);
                                        }
                                    } else {
                                        cursor
                                            .seek(SeekFrom::Current(
                                                ts_header.chunk_size as i64,
                                            ))
                                            .map_err(|e| {
                                                W3DError::ModelLoadingFailed(format!(
                                                    "Failed to skip tex stage: {}",
                                                    e
                                                ))
                                            })?;
                                    }
                                }
                            }
                            _ => {
                                cursor
                                    .seek(SeekFrom::Current(mp_header.chunk_size as i64))
                                    .map_err(|e| {
                                        W3DError::ModelLoadingFailed(format!(
                                            "Failed to skip mat pass sub-chunk: {}",
                                            e
                                        ))
                                    })?;
                            }
                        }
                    }

                    // Build W3DMaterial from the first pass's indices
                    let vm_idx = vm_ids.first().copied().unwrap_or(0) as usize;
                    let tex_idx = tex_ids.first().copied().unwrap_or(0) as usize;

                    let vm_name = vert_material_names
                        .get(vm_idx)
                        .cloned()
                        .unwrap_or_else(|| format!("Material{}", materials.len()));
                    let vm_info = vert_material_infos
                        .get(vm_idx)
                        .copied()
                        .unwrap_or_default();

                    let diff_tex = if tex_idx < texture_names.len() {
                        Some(texture_names[tex_idx].clone())
                    } else {
                        None
                    };

                    let base_color = Vec4::new(
                        vm_info.diffuse.0 as f32 / 255.0,
                        vm_info.diffuse.1 as f32 / 255.0,
                        vm_info.diffuse.2 as f32 / 255.0,
                        vm_info.opacity,
                    );

                    let double_sided = shader_infos
                        .first()
                        .map(|s| s.alpha_test != 0)
                        .unwrap_or(false);

                    materials.push(W3DMaterial {
                        name: vm_name,
                        base_color,
                        diffuse_texture: diff_tex,
                        normal_texture: None,
                        specular_texture: None,
                        emissive_texture: None,
                        metallic: 0.0,
                        roughness: 1.0 / vm_info.shininess.max(1.0),
                        emissive_factor: Vec3::new(
                            vm_info.emissive.0 as f32 / 255.0,
                            vm_info.emissive.1 as f32 / 255.0,
                            vm_info.emissive.2 as f32 / 255.0,
                        ),
                        alpha_cutoff: 0.5,
                        double_sided,
                    });
                }

                _ => {
                    log::warn!(
                        "Skipping unhandled material sub-chunk 0x{:08X}, size {}",
                        sub_header.chunk_type,
                        sub_header.chunk_size
                    );
                    cursor
                        .seek(SeekFrom::Current(sub_header.chunk_size as i64))
                        .map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to skip material sub-chunk: {}",
                                e
                            ))
                        })?;
                }
            }

            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }

        // If no materials were built from passes, create defaults from the vertex materials
        if materials.is_empty() && !vert_material_names.is_empty() {
            for (i, name) in vert_material_names.iter().enumerate() {
                let info = vert_material_infos.get(i).copied().unwrap_or_default();
                materials.push(W3DMaterial {
                    name: name.clone(),
                    base_color: Vec4::new(
                        info.diffuse.0 as f32 / 255.0,
                        info.diffuse.1 as f32 / 255.0,
                        info.diffuse.2 as f32 / 255.0,
                        info.opacity,
                    ),
                    diffuse_texture: texture_names.first().cloned(),
                    normal_texture: None,
                    specular_texture: None,
                    emissive_texture: None,
                    metallic: 0.0,
                    roughness: 1.0 / info.shininess.max(1.0),
                    emissive_factor: Vec3::new(
                        info.emissive.0 as f32 / 255.0,
                        info.emissive.1 as f32 / 255.0,
                        info.emissive.2 as f32 / 255.0,
                    ),
                    alpha_cutoff: 0.5,
                    double_sided: false,
                });
            }
        }

        tracing::debug!("Parsed {} materials", materials.len());
        Ok(materials)
    }

    /// Parse a `W3D_CHUNK_HIERARCHY` top-level chunk (0x00000100).
    ///
    /// Binary layout mirrors C++ `HTreeClass::Load_W3D` in `htree.cpp`:
    ///   - Sub-chunk `W3D_CHUNK_HIERARCHY_HEADER` (0x101): `W3dHierarchyStruct`
    ///   - Sub-chunk `W3D_CHUNK_PIVOTS` (0x102):            array of `W3dPivotStruct`
    ///
    /// `W3dHierarchyStruct` { Version: u32, Name: [u8;16], NumPivots: u32, Center: [f32;3] }
    /// `W3dPivotStruct`     { Name: [u8;16], ParentIdx: u32, Translation: [f32;3],
    ///                         EulerAngles: [f32;3], Rotation: [f32;4] }
    fn parse_hierarchy_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
    ) -> Result<Vec<W3DBone>> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        let mut hierarchy_name = String::new();
        let mut num_pivots: u32 = 0;
        let mut version: u32 = 0;
        let mut bones: Vec<W3DBone> = Vec::new();

        while cursor.position() < end_pos {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            match sub_type {
                W3DChunkType::HierarchyHeader => {
                    // W3dHierarchyStruct: Version(4) + Name(16) + NumPivots(4) + Center(12) = 36 bytes
                    version = self.read_u32(cursor)?;
                    let mut name_buf = [0u8; W3D_NAME_LEN];
                    cursor.read_exact(&mut name_buf).map_err(|e| {
                        W3DError::ModelLoadingFailed(format!(
                            "Failed to read hierarchy name: {}",
                            e
                        ))
                    })?;
                    hierarchy_name = Self::read_null_terminated(&name_buf);
                    num_pivots = self.read_u32(cursor)?;
                    // Center: 3 x f32
                    let _center_x = self.read_f32(cursor)?;
                    let _center_y = self.read_f32(cursor)?;
                    let _center_z = self.read_f32(cursor)?;
                }

                W3DChunkType::Pivots => {
                    // W3dPivotStruct per bone:
                    //   Name[16] + ParentIdx(u32) + Translation[3](f32) +
                    //   EulerAngles[3](f32) + Rotation[4](f32) = 16+4+12+12+16 = 60 bytes
                    let pivot_size: u32 = W3D_NAME_LEN as u32 + 4 + 12 + 12 + 16;
                    let pivot_count = sub_header.chunk_size / pivot_size;

                    if pivot_count != num_pivots {
                        log::warn!(
                            "Hierarchy pivot count {} differs from header NumPivots {}",
                            pivot_count,
                            num_pivots
                        );
                    }

                    let is_pre30 = version < ((3u32) << 16);
                    let extra_root = if is_pre30 { 1 } else { 0 };

                    // Pre-3.0 files don't have a root node, so we insert one
                    if is_pre30 {
                        bones.push(W3DBone {
                            name: "RootTransform".to_string(),
                            parent_index: -1,
                            rest_position: Vec3::ZERO,
                            rest_rotation: Quat::IDENTITY,
                            rest_scale: Vec3::ONE,
                            bind_matrix: Mat4::IDENTITY,
                            inverse_bind_matrix: Mat4::IDENTITY,
                        });
                    }

                    for _ in 0..pivot_count {
                        let mut name_buf = [0u8; W3D_NAME_LEN];
                        cursor.read_exact(&mut name_buf).map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to read pivot name: {}",
                                e
                            ))
                        })?;
                        let bone_name = Self::read_null_terminated(&name_buf);

                        let mut parent_idx = self.read_u32(cursor)? as i32;

                        let tx = self.read_f32(cursor)?;
                        let ty = self.read_f32(cursor)?;
                        let tz = self.read_f32(cursor)?;

                        // EulerAngles: read and discard (C++ code only uses Translation + Rotation)
                        let _euler_x = self.read_f32(cursor)?;
                        let _euler_y = self.read_f32(cursor)?;
                        let _euler_z = self.read_f32(cursor)?;

                        let qx = self.read_f32(cursor)?;
                        let qy = self.read_f32(cursor)?;
                        let qz = self.read_f32(cursor)?;
                        let qw = self.read_f32(cursor)?;

                        // Pre-3.0: shift parent indices up by 1 (C++ does piv.ParentIdx += 1)
                        if is_pre30 {
                            parent_idx += 1;
                        }

                        let rest_position = Vec3::new(tx, ty, tz);
                        let rest_rotation = Quat::from_xyzw(qx, qy, qz, qw).normalize();

                        // Build the bind matrix: translation * rotation (matches C++ BaseTransform)
                        let bind_matrix =
                            Mat4::from_translation(rest_position) * Mat4::from_quat(rest_rotation);
                        let inverse_bind_matrix = bind_matrix.inverse();

                        bones.push(W3DBone {
                            name: bone_name,
                            parent_index: parent_idx,
                            rest_position,
                            rest_rotation,
                            rest_scale: Vec3::ONE,
                            bind_matrix,
                            inverse_bind_matrix,
                        });
                    }
                }

                _ => {
                    log::warn!(
                        "Skipping unhandled hierarchy sub-chunk 0x{:08X}, size {}",
                        sub_header.chunk_type,
                        sub_header.chunk_size
                    );
                    cursor
                        .seek(SeekFrom::Current(sub_header.chunk_size as i64))
                        .map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to skip hierarchy sub-chunk: {}",
                                e
                            ))
                        })?;
                }
            }

            // Ensure we're at the end of this sub-chunk (handles partial reads / padding)
            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        // Ensure cursor is at end of our chunk
        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }

        tracing::debug!(
            "Parsed hierarchy '{}' (v{}): {} bones",
            hierarchy_name,
            version,
            bones.len()
        );

        Ok(bones)
    }

    /// Parse a `W3D_CHUNK_ANIMATION` top-level chunk (0x00000200).
    ///
    /// Binary layout mirrors C++ `HRawAnimClass::Load_W3D` in `hrawanim.cpp`:
    ///   - Sub-chunk `W3D_CHUNK_ANIMATION_HEADER` (0x201): `W3dAnimHeaderStruct`
    ///   - Sub-chunk `W3D_CHUNK_ANIMATION_CHANNEL` (0x202): `W3dAnimChannelStruct` × N
    ///   - Sub-chunk `W3D_CHUNK_BIT_CHANNEL` (0x203): `W3dBitChannelStruct` × N
    ///
    /// `W3dAnimHeaderStruct` { Version:u32, Name:[u8;16], HierarchyName:[u8;16],
    ///                         NumFrames:u32, FrameRate:u32 }
    ///
    /// `W3dAnimChannelStruct` { FirstFrame:u16, LastFrame:u16, VectorLen:u16,
    ///                          Flags:u16, Pivot:u16, pad:u16, Data:[f32...] }
    ///   Flags indicate channel type: 0=X,1=Y,2=Z,3=XR,4=YR,5=ZR,6=Q (quaternion)
    fn parse_animation_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
    ) -> Result<W3DAnimation> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        let mut anim_name = String::new();
        let mut _hierarchy_name = String::new();
        let mut num_frames: u32 = 0;
        let mut frame_rate: u32 = 0;
        let mut version: u32 = 0;

        // Accumulate per-bone channels: bone_index -> (pos keys, rot keys, scale keys)
        let mut bone_channels: HashMap<u32, W3DAnimationChannel> = HashMap::new();

        while cursor.position() < end_pos {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            match sub_type {
                W3DChunkType::AnimationHeader => {
                    // W3dAnimHeaderStruct: 4 + 16 + 16 + 4 + 4 = 44 bytes
                    version = self.read_u32(cursor)?;

                    let mut name_buf = [0u8; W3D_NAME_LEN];
                    cursor.read_exact(&mut name_buf).map_err(|e| {
                        W3DError::ModelLoadingFailed(format!(
                            "Failed to read anim name: {}",
                            e
                        ))
                    })?;
                    let raw_name = Self::read_null_terminated(&name_buf);

                    let mut hier_buf = [0u8; W3D_NAME_LEN];
                    cursor.read_exact(&mut hier_buf).map_err(|e| {
                        W3DError::ModelLoadingFailed(format!(
                            "Failed to read anim hierarchy name: {}",
                            e
                        ))
                    })?;
                    _hierarchy_name = Self::read_null_terminated(&hier_buf);

                    num_frames = self.read_u32(cursor)?;
                    frame_rate = self.read_u32(cursor)?;

                    // C++ builds name as "HierarchyName.Name"
                    anim_name = format!("{}.{}", _hierarchy_name, raw_name);
                }

                W3DChunkType::AnimationChannel => {
                    // W3dAnimChannelStruct header: FirstFrame(u16) + LastFrame(u16) +
                    //   VectorLen(u16) + Flags(u16) + Pivot(u16) + pad(u16) + first f32 = 14 bytes
                    let first_frame = self.read_u16(cursor)?;
                    let last_frame = self.read_u16(cursor)?;
                    let vector_len = self.read_u16(cursor)? as usize;
                    let flags = self.read_u16(cursor)? as usize;
                    let pivot = self.read_u16(cursor)? as u32;
                    let _pad = self.read_u16(cursor)?;

                    let frame_count = (last_frame - first_frame + 1) as usize;
                    // Total data floats: frame_count * vector_len, first already read as chan.Data[0]
                    let total_floats = frame_count * vector_len;

                    // Read all float data for this channel
                    let mut channel_data = Vec::with_capacity(total_floats);
                    let first_val = self.read_f32(cursor)?;
                    channel_data.push(first_val);

                    let remaining = total_floats - 1;
                    for _ in 0..remaining {
                        channel_data.push(self.read_f32(cursor)?);
                    }

                    // Skip any extra data (C++ exporter bug: may write too much)
                    let bytes_read = remaining * 4;
                    let expected_sub_data = sub_header.chunk_size as usize - 14;
                    if bytes_read < expected_sub_data {
                        let skip = expected_sub_data - bytes_read;
                        cursor.seek(SeekFrom::Current(skip as i64)).map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to skip channel padding: {}",
                                e
                            ))
                        })?;
                    }

                    // Determine channel type from flags (low byte)
                    let channel_type = flags & 0x0F;
                    let is_pre30 = version < ((3u32) << 16);
                    let bone_idx = if is_pre30 { pivot + 1 } else { pivot };

                    let ch = bone_channels.entry(bone_idx).or_insert_with(|| {
                        W3DAnimationChannel {
                            bone_index: bone_idx,
                            position_keys: Vec::new(),
                            rotation_keys: Vec::new(),
                            scale_keys: Vec::new(),
                        }
                    });

                    // flags 0=X, 1=Y, 2=Z, 6=Q (quaternion)
                    match channel_type {
                        0 | 1 | 2 => {
                            for (fi, &val) in channel_data.iter().enumerate() {
                                let time = (first_frame as usize + fi) as f32
                                    / if frame_rate > 0 { frame_rate as f32 } else { 30.0 };

                                let existing = ch
                                    .position_keys
                                    .iter()
                                    .position(|k| (k.time - time).abs() < f32::EPSILON);
                                match existing {
                                    Some(idx) => {
                                        let key = &mut ch.position_keys[idx];
                                        match channel_type {
                                            0 => key.position.x = val,
                                            1 => key.position.y = val,
                                            _ => key.position.z = val,
                                        }
                                    }
                                    None => {
                                        let mut pos = Vec3::ZERO;
                                        match channel_type {
                                            0 => pos.x = val,
                                            1 => pos.y = val,
                                            _ => pos.z = val,
                                        }
                                        ch.position_keys.push(W3DKeyframe {
                                            time,
                                            position: pos,
                                            rotation: Quat::IDENTITY,
                                            scale: Vec3::ONE,
                                        });
                                    }
                                }
                            }
                        }
                        3 | 4 | 5 => {
                            let axis = channel_type - 3;
                            for (fi, &val) in channel_data.iter().enumerate() {
                                let time = (first_frame as usize + fi) as f32
                                    / if frame_rate > 0 { frame_rate as f32 } else { 30.0 };

                                let euler = match axis {
                                    0 => Vec3::new(val, 0.0, 0.0),
                                    1 => Vec3::new(0.0, val, 0.0),
                                    _ => Vec3::new(0.0, 0.0, val),
                                };
                                let q = Quat::from_euler(
                                    glam::EulerRot::XYZ,
                                    euler.x,
                                    euler.y,
                                    euler.z,
                                );

                                let existing = ch
                                    .rotation_keys
                                    .iter()
                                    .position(|k| (k.time - time).abs() < f32::EPSILON);
                                match existing {
                                    Some(idx) => {
                                        let key = &mut ch.rotation_keys[idx];
                                        key.rotation = q;
                                    }
                                    None => {
                                        ch.rotation_keys.push(W3DKeyframe {
                                            time,
                                            position: Vec3::ZERO,
                                            rotation: q,
                                            scale: Vec3::ONE,
                                        });
                                    }
                                }
                            }
                        }
                        6 => {
                            // Quaternion rotation: each frame is 4 floats
                            for (fi, chunk) in channel_data.chunks_exact(4).enumerate() {
                                let time = (first_frame as usize + fi) as f32
                                    / if frame_rate > 0 { frame_rate as f32 } else { 30.0 };
                                let q = Quat::from_xyzw(chunk[0], chunk[1], chunk[2], chunk[3])
                                    .normalize();
                                ch.rotation_keys.push(W3DKeyframe {
                                    time,
                                    position: Vec3::ZERO,
                                    rotation: q,
                                    scale: Vec3::ONE,
                                });
                            }
                        }
                        _ => {
                            log::warn!(
                                "Unhandled animation channel type {} for bone {}",
                                channel_type,
                                bone_idx
                            );
                        }
                    }
                }

                W3DChunkType::BitChannel => {
                    // W3dBitChannelStruct: FirstFrame(u16) + LastFrame(u16) + Flags(u16) +
                    //   Pivot(u16) + DefaultVal(u8) + Data[...] = 9 bytes header + ceil(n/8) data
                    let _first_frame = self.read_u16(cursor)?;
                    let _last_frame = self.read_u16(cursor)?;
                    let _flags = self.read_u16(cursor)?;
                    let _pivot = self.read_u16(cursor)?;
                    let _default_val = {
                        let mut buf = [0u8; 1];
                        cursor.read_exact(&mut buf).map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to read bit channel default: {}",
                                e
                            ))
                        })?;
                        buf[0]
                    };
                    // Skip the bit data
                    let remaining = sub_header.chunk_size as i64 - 9;
                    if remaining > 0 {
                        cursor.seek(SeekFrom::Current(remaining)).map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to skip bit channel data: {}",
                                e
                            ))
                        })?;
                    }
                }

                _ => {
                    log::warn!(
                        "Skipping unhandled animation sub-chunk 0x{:08X}, size {}",
                        sub_header.chunk_type,
                        sub_header.chunk_size
                    );
                    cursor
                        .seek(SeekFrom::Current(sub_header.chunk_size as i64))
                        .map_err(|e| {
                            W3DError::ModelLoadingFailed(format!(
                                "Failed to skip animation sub-chunk: {}",
                                e
                            ))
                        })?;
                }
            }

            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }

        let fps = if frame_rate > 0 { frame_rate as f32 } else { 30.0 };
        let duration = if num_frames > 0 && fps > 0.0 {
            (num_frames - 1) as f32 / fps
        } else {
            0.0
        };

        let mut channels: Vec<W3DAnimationChannel> =
            bone_channels.into_values().collect();
        channels.sort_by_key(|c| c.bone_index);

        tracing::debug!(
            "Parsed animation '{}' (v{}): {} frames @ {} fps, {} channels",
            anim_name,
            version,
            num_frames,
            fps,
            channels.len()
        );

        Ok(W3DAnimation {
            name: anim_name,
            duration,
            channels,
            fps,
        })
    }

    /// Generate tangent and binormal vectors for normal mapping
    fn generate_tangents(&self, mesh: &mut W3DMesh) {
        if mesh.indices.len() % 3 != 0 {
            return;
        }

        // Initialize tangents and binormals
        for vertex in &mut mesh.vertices {
            vertex.tangent = [0.0; 3];
            vertex.binormal = [0.0; 3];
        }

        // Calculate tangents for each triangle
        for triangle_idx in (0..mesh.indices.len()).step_by(3) {
            let i0 = mesh.indices[triangle_idx] as usize;
            let i1 = mesh.indices[triangle_idx + 1] as usize;
            let i2 = mesh.indices[triangle_idx + 2] as usize;

            if i0 >= mesh.vertices.len() || i1 >= mesh.vertices.len() || i2 >= mesh.vertices.len() {
                continue;
            }

            let v0 = &mesh.vertices[i0];
            let v1 = &mesh.vertices[i1];
            let v2 = &mesh.vertices[i2];

            let pos0 = Vec3::from_array(v0.position);
            let pos1 = Vec3::from_array(v1.position);
            let pos2 = Vec3::from_array(v2.position);

            let uv0 = Vec2::from_array(v0.tex_coords);
            let uv1 = Vec2::from_array(v1.tex_coords);
            let uv2 = Vec2::from_array(v2.tex_coords);

            let delta_pos1 = pos1 - pos0;
            let delta_pos2 = pos2 - pos0;
            let delta_uv1 = uv1 - uv0;
            let delta_uv2 = uv2 - uv0;

            let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
            let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
            let binormal = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * r;

            // Accumulate tangents
            mesh.vertices[i0].tangent =
                (Vec3::from_array(mesh.vertices[i0].tangent) + tangent).to_array();
            mesh.vertices[i1].tangent =
                (Vec3::from_array(mesh.vertices[i1].tangent) + tangent).to_array();
            mesh.vertices[i2].tangent =
                (Vec3::from_array(mesh.vertices[i2].tangent) + tangent).to_array();

            mesh.vertices[i0].binormal =
                (Vec3::from_array(mesh.vertices[i0].binormal) + binormal).to_array();
            mesh.vertices[i1].binormal =
                (Vec3::from_array(mesh.vertices[i1].binormal) + binormal).to_array();
            mesh.vertices[i2].binormal =
                (Vec3::from_array(mesh.vertices[i2].binormal) + binormal).to_array();
        }

        // Normalize tangents and binormals
        for vertex in &mut mesh.vertices {
            let tangent = Vec3::from_array(vertex.tangent).normalize();
            let binormal = Vec3::from_array(vertex.binormal).normalize();

            vertex.tangent = tangent.to_array();
            vertex.binormal = binormal.to_array();
        }
    }

    /// Calculate bounding box for entire model
    fn calculate_model_bounds(&self, model: &mut W3DModel) {
        if model.meshes.is_empty() {
            return;
        }

        let mut min = model.meshes[0].bounding_box.min;
        let mut max = model.meshes[0].bounding_box.max;

        for mesh in &model.meshes[1..] {
            min = min.min(mesh.bounding_box.min);
            max = max.max(mesh.bounding_box.max);
        }

        model.bounding_box = BoundingBox::new(min, max);
    }

    /// Parse a `W3D_CHUNK_HMODEL` top-level chunk (0x00000300).
    ///
    /// An HModel is a hierarchical model containing a hierarchy sub-chunk and
    /// one or more mesh sub-chunks. C++ loads these in `W3DAssetManager`.
    fn parse_hmodel_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
        model: &mut W3DModel,
    ) -> Result<()> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // HModel header: Version(u32) + Name[16] + unknown(u32) = 24 bytes
        let _version = self.read_u32(cursor)?;
        let mut name_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read HModel name: {}", e))
        })?;
        let _hmodel_name = Self::read_null_terminated(&name_buf);
        let _unknown = self.read_u32(cursor)?;

        while cursor.position() < end_pos {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            match sub_type {
                W3DChunkType::Hierarchy => {
                    let skeleton =
                        self.parse_hierarchy_chunk(cursor, sub_header.chunk_size)?;
                    model.skeleton = Some(skeleton);
                }
                W3DChunkType::Mesh => {
                    let mesh = self.parse_mesh_chunk(cursor, sub_header.chunk_size)?;
                    model.meshes.push(mesh);
                }
                _ => {
                    log::warn!(
                        "Skipping unhandled HModel sub-chunk 0x{:08X}, size {}",
                        sub_header.chunk_type,
                        sub_header.chunk_size
                    );
                    cursor.seek(SeekFrom::Current(sub_header.chunk_size as i64)).map_err(|e| {
                        W3DError::ModelLoadingFailed(format!("Failed to skip HModel sub-chunk: {}", e))
                    })?;
                }
            }

            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }
        Ok(())
    }

    /// Parse a `W3D_CHUNK_HLOD` top-level chunk (0x00000700).
    ///
    /// HLod is the most common chunk type in Generals models. It contains a
    /// hierarchy and multiple meshes with LOD information.
    /// C++ reference: `W3DAssetManager::Load_HLOD`.
    fn parse_hlod_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
        model: &mut W3DModel,
    ) -> Result<()> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // HLod header: Version(u32) + Name[16] + HierarchyName[16] + NumLods(u32) = 40 bytes
        let _version = self.read_u32(cursor)?;
        let mut name_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read HLod name: {}", e))
        })?;
        let _hlod_name = Self::read_null_terminated(&name_buf);

        let mut hier_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut hier_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read HLod hierarchy name: {}", e))
        })?;
        let _hier_name = Self::read_null_terminated(&hier_buf);

        let num_lods = self.read_u32(cursor)?;

        model.lod_distances = vec![0.0; num_lods as usize];

        while cursor.position() < end_pos {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            match sub_type {
                W3DChunkType::Hierarchy => {
                    let skeleton =
                        self.parse_hierarchy_chunk(cursor, sub_header.chunk_size)?;
                    model.skeleton = Some(skeleton);
                }
                W3DChunkType::Mesh => {
                    let mesh = self.parse_mesh_chunk(cursor, sub_header.chunk_size)?;
                    model.meshes.push(mesh);
                }
                // W3D_CHUNK_HLOD_LOD_ARRAY wrapper (0x00000701)
                _ if sub_header.chunk_type == 0x00000701 => {
                    self.parse_lod_array(cursor, sub_header.chunk_size, model)?;
                }
                _ => {
                    log::warn!(
                        "Skipping unhandled HLod sub-chunk 0x{:08X}, size {}",
                        sub_header.chunk_type,
                        sub_header.chunk_size
                    );
                    cursor.seek(SeekFrom::Current(sub_header.chunk_size as i64)).map_err(|e| {
                        W3DError::ModelLoadingFailed(format!("Failed to skip HLod sub-chunk: {}", e))
                    })?;
                }
            }

            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }
        tracing::debug!(
            "Parsed HLod: {} meshes, {} LODs",
            model.meshes.len(),
            num_lods
        );
        Ok(())
    }

    /// Parse a LOD array sub-chunk within an HLod.
    ///
    /// Contains model mesh references with LOD distances.
    fn parse_lod_array(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
        model: &mut W3DModel,
    ) -> Result<()> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // LOD array header: Version(u32) + ModelName[16] + NumLods(u32) = 24 bytes
        let _version = self.read_u32(cursor)?;
        let mut name_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read LOD array name: {}", e))
        })?;
        let _array_name = Self::read_null_terminated(&name_buf);
        let num_lods = self.read_u32(cursor)?;

        for i in 0..num_lods as usize {
            if cursor.position() >= end_pos {
                break;
            }
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            // W3D_CHUNK_HLOD_SUB_OBJECT (0x00000702): BoneIndex(u32) + Distance(f32) + Name[16]
            if sub_header.chunk_type == 0x00000702 && sub_header.chunk_size >= 24 {
                let _bone_index = self.read_u32(cursor)?;
                let distance = self.read_f32(cursor)?;
                let mut obj_name_buf = [0u8; W3D_NAME_LEN];
                cursor.read_exact(&mut obj_name_buf).map_err(|e| {
                    W3DError::ModelLoadingFailed(format!("Failed to read LOD sub-object name: {}", e))
                })?;
                let _obj_name = Self::read_null_terminated(&obj_name_buf);

                if i < model.lod_distances.len() {
                    model.lod_distances[i] = distance;
                }
            } else {
                cursor.seek(SeekFrom::Current(sub_header.chunk_size as i64)).map_err(|e| {
                    W3DError::ModelLoadingFailed(format!("Failed to skip LOD entry: {}", e))
                })?;
            }

            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }
        Ok(())
    }

    /// Parse a `W3D_CHUNK_LODMODEL` top-level chunk (0x00000400).
    ///
    /// LOD model contains multiple meshes at different detail levels.
    fn parse_lod_model_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
        model: &mut W3DModel,
    ) -> Result<()> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // LOD model header: Version(u32) + Name[16] + HierarchyName[16] + NumLods(u32)
        let _version = self.read_u32(cursor)?;
        let mut name_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read LOD model name: {}", e))
        })?;
        let _lod_name = Self::read_null_terminated(&name_buf);

        let mut hier_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut hier_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read LOD hierarchy name: {}", e))
        })?;
        let _hier_name = Self::read_null_terminated(&hier_buf);

        let _num_lods = self.read_u32(cursor)?;

        while cursor.position() < end_pos {
            let sub_header = self.read_struct::<W3DChunkHeader>(cursor)?;
            let sub_type = W3DChunkType::from(sub_header.chunk_type);
            let sub_end = cursor.position() + sub_header.chunk_size as u64;

            match sub_type {
                W3DChunkType::Mesh => {
                    let mesh = self.parse_mesh_chunk(cursor, sub_header.chunk_size)?;
                    model.meshes.push(mesh);
                }
                W3DChunkType::Hierarchy => {
                    let skeleton =
                        self.parse_hierarchy_chunk(cursor, sub_header.chunk_size)?;
                    model.skeleton = Some(skeleton);
                }
                _ => {
                    log::warn!(
                        "Skipping unhandled LOD model sub-chunk 0x{:08X}, size {}",
                        sub_header.chunk_type,
                        sub_header.chunk_size
                    );
                    cursor.seek(SeekFrom::Current(sub_header.chunk_size as i64)).map_err(|e| {
                        W3DError::ModelLoadingFailed(format!("Failed to skip LOD model sub-chunk: {}", e))
                    })?;
                }
            }

            if cursor.position() < sub_end {
                cursor.set_position(sub_end);
            }
        }

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }
        Ok(())
    }

    /// Parse a `W3D_CHUNK_COLLECTION` top-level chunk (0x00000420).
    ///
    /// Collections group multiple transform references to other models.
    fn parse_collection_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
    ) -> Result<()> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // Collection header: Version(u32) + Name[16] + NumTransforms(u32)
        let _version = self.read_u32(cursor)?;
        let mut name_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read collection name: {}", e))
        })?;
        let coll_name = Self::read_null_terminated(&name_buf);
        let num_transforms = self.read_u32(cursor)?;

        // Skip the transform data (each transform is ~68 bytes: Mat4 + name)
        let remaining = end_pos.saturating_sub(cursor.position());
        if remaining > 0 {
            cursor.seek(SeekFrom::Current(remaining as i64)).map_err(|e| {
                W3DError::ModelLoadingFailed(format!("Failed to skip collection data: {}", e))
            })?;
        }

        log::warn!(
            "Collection '{}' has {} transforms — parsed header, data skipped (not yet populated into model)",
            coll_name,
            num_transforms
        );

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }
        Ok(())
    }

    /// Skip an emitter chunk with a descriptive log.
    fn skip_emitter_chunk(
        &mut self,
        cursor: &mut Cursor<&[u8]>,
        chunk_size: u32,
    ) -> Result<()> {
        let start_pos = cursor.position();
        let end_pos = start_pos + chunk_size as u64;

        // Read emitter header for logging: Version(u32) + Name[16]
        let _version = self.read_u32(cursor)?;
        let mut name_buf = [0u8; W3D_NAME_LEN];
        cursor.read_exact(&mut name_buf).map_err(|e| {
            W3DError::ModelLoadingFailed(format!("Failed to read emitter name: {}", e))
        })?;
        let emitter_name = Self::read_null_terminated(&name_buf);

        let remaining = end_pos.saturating_sub(cursor.position());
        if remaining > 0 {
            cursor.seek(SeekFrom::Current(remaining as i64)).map_err(|e| {
                W3DError::ModelLoadingFailed(format!("Failed to skip emitter data: {}", e))
            })?;
        }

        log::warn!(
            "Emitter '{}' parsed header, particle system data skipped",
            emitter_name
        );

        if cursor.position() < end_pos {
            cursor.set_position(end_pos);
        }
        Ok(())
    }

    /// Helper to read struct from cursor
    fn read_struct<T: Pod>(&self, cursor: &mut Cursor<&[u8]>) -> Result<T> {
        let size = std::mem::size_of::<T>();
        let mut buffer = vec![0u8; size];

        cursor
            .read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read struct: {}", e)))?;

        Ok(*bytemuck::from_bytes(&buffer))
    }

    /// Helper to read f32
    fn read_f32(&self, cursor: &mut Cursor<&[u8]>) -> Result<f32> {
        let mut buffer = [0u8; 4];
        cursor
            .read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read f32: {}", e)))?;
        Ok(f32::from_le_bytes(buffer))
    }

    /// Helper to read u32
    fn read_u32(&self, cursor: &mut Cursor<&[u8]>) -> Result<u32> {
        let mut buffer = [0u8; 4];
        cursor
            .read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read u32: {}", e)))?;
        Ok(u32::from_le_bytes(buffer))
    }

    /// Helper to read u16 (little-endian)
    fn read_u16(&self, cursor: &mut Cursor<&[u8]>) -> Result<u16> {
        let mut buffer = [0u8; 2];
        cursor
            .read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read u16: {}", e)))?;
        Ok(u16::from_le_bytes(buffer))
    }

    /// Helper to read u8
    fn read_u8(&self, cursor: &mut Cursor<&[u8]>) -> Result<u8> {
        let mut buffer = [0u8; 1];
        cursor
            .read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read u8: {}", e)))?;
        Ok(buffer[0])
    }

    /// Read a W3dRGBStruct: R, G, B as u8 + 1 byte pad
    fn read_w3d_rgb(&self, cursor: &mut Cursor<&[u8]>) -> Result<(u8, u8, u8)> {
        let r = self.read_u8(cursor)?;
        let g = self.read_u8(cursor)?;
        let b = self.read_u8(cursor)?;
        let _pad = self.read_u8(cursor)?;
        Ok((r, g, b))
    }

    /// Read a null-terminated string from a fixed-size buffer (W3D_NAME_LEN bytes)
    fn read_null_terminated(buf: &[u8; W3D_NAME_LEN]) -> String {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(W3D_NAME_LEN);
        String::from_utf8_lossy(&buf[..end]).to_string()
    }

    /// Read a null-terminated string from a variable-length slice
    fn read_null_terminated_from_slice(buf: &[u8]) -> String {
        let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
        String::from_utf8_lossy(&buf[..end]).to_string()
    }
}

impl Default for W3DModelLoader {
    fn default() -> Self {
        Self::new()
    }
}

/// Matches C++ global `ReloadAllTextures`.
pub fn reload_all_textures(loader: &mut W3DModelLoader) {
    loader.reload_all_textures();
}

/// GPU-optimized mesh representation
#[cfg(feature = "w3d")]
pub struct W3DGpuMesh {
    /// Vertex buffer
    pub vertex_buffer: Buffer,
    /// Index buffer
    pub index_buffer: Buffer,
    /// Vertex count
    pub vertex_count: u32,
    /// Index count
    pub index_count: u32,
    /// Material index
    pub material_index: u32,
    /// Bounding box
    pub bounding_box: BoundingBox,
}

#[cfg(feature = "w3d")]
impl W3DGpuMesh {
    /// Create GPU mesh from CPU mesh
    pub fn from_mesh(
        device: &Device,
        mesh: &W3DMesh,
        material_indices: &std::collections::HashMap<String, u32>,
    ) -> Self {
        let vertex_data = cast_slice(&mesh.vertices);
        let vertex_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Vertex Buffer"),
            contents: vertex_data,
            usage: BufferUsages::VERTEX,
        });

        let index_data = cast_slice(&mesh.indices);
        let index_buffer = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("W3D Index Buffer"),
            contents: index_data,
            usage: BufferUsages::INDEX,
        });

        let material_index = mesh
            .material_id
            .as_ref()
            .and_then(|id| material_indices.get(id).copied())
            .unwrap_or(0);

        Self {
            vertex_buffer,
            index_buffer,
            vertex_count: mesh.vertices.len() as u32,
            index_count: mesh.indices.len() as u32,
            material_index,
            bounding_box: mesh.bounding_box,
        }
    }
}

/// GPU-optimized model representation
#[cfg(feature = "w3d")]
pub struct W3DGpuModel {
    /// GPU meshes
    pub meshes: Vec<W3DGpuMesh>,
    /// Model bounding box
    pub bounding_box: BoundingBox,
    /// LOD distances
    pub lod_distances: Vec<f32>,
}

#[cfg(feature = "w3d")]
impl W3DGpuModel {
    /// Create GPU model from CPU model
    pub fn from_model(device: &Device, model: &W3DModel) -> Self {
        let material_indices = model
            .materials
            .iter()
            .enumerate()
            .map(|(index, material)| (material.name.clone(), index as u32))
            .collect::<std::collections::HashMap<_, _>>();

        let meshes = model
            .meshes
            .iter()
            .map(|mesh| W3DGpuMesh::from_mesh(device, mesh, &material_indices))
            .collect();

        Self {
            meshes,
            bounding_box: model.bounding_box,
            lod_distances: model.lod_distances.clone(),
        }
    }
}
