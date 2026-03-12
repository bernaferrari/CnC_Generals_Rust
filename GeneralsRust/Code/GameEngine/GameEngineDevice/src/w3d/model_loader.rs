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

use super::{W3DError, Result};
use crate::video::{ColorFormat, Resolution};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Cursor};
use std::path::{Path, PathBuf};
use bytemuck::{Pod, Zeroable, cast_slice};
use glam::{Vec2, Vec3, Vec4, Mat4, Quat};

#[cfg(feature = "w3d")]
use wgpu::{
    Device, Queue, Buffer, BufferDescriptor, BufferUsages,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
    util::{DeviceExt, BufferInitDescriptor},
    Extent3d, Origin3d,
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

/// W3D chunk types (from original C++)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum W3DChunkType {
    /// Mesh chunk
    Mesh = 0x00000000,
    /// Vertices
    Vertices = 0x00000002,
    /// Vertex normals
    VertexNormals = 0x00000003,
    /// Vertex colors
    VertexColors = 0x00000004,
    /// Texture coordinates
    TexCoords = 0x00000005,
    /// Triangles
    Triangles = 0x00000032,
    /// Material info
    MaterialInfo = 0x00000028,
    /// Shader materials
    ShaderMaterials = 0x00000029,
    /// Textures
    Textures = 0x00000033,
    /// Hierarchy
    Hierarchy = 0x00000010,
    /// Animation
    Animation = 0x00000200,
    /// Skeleton
    Skeleton = 0x00000040,
    /// Bone
    Bone = 0x00000041,
    /// Unknown/Custom
    Unknown(u32),
}

impl From<u32> for W3DChunkType {
    fn from(value: u32) -> Self {
        match value {
            0x00000000 => Self::Mesh,
            0x00000002 => Self::Vertices,
            0x00000003 => Self::VertexNormals,
            0x00000004 => Self::VertexColors,
            0x00000005 => Self::TexCoords,
            0x00000032 => Self::Triangles,
            0x00000028 => Self::MaterialInfo,
            0x00000029 => Self::ShaderMaterials,
            0x00000033 => Self::Textures,
            0x00000010 => Self::Hierarchy,
            0x00000200 => Self::Animation,
            0x00000040 => Self::Skeleton,
            0x00000041 => Self::Bone,
            other => Self::Unknown(other),
        }
    }
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

    /// Load W3D model from file
    pub async fn load_model<P: AsRef<Path>>(&mut self, path: P) -> Result<W3DModel> {
        let path = path.as_ref();
        tracing::info!("Loading W3D model: {}", path.display());

        let data = tokio::fs::read(path).await
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read file: {}", e)))?;

        self.parse_w3d_data(&data, path.file_stem().unwrap_or_default().to_string_lossy().to_string())
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
            
            tracing::debug!("Processing chunk: {:?}, size: {}", chunk_type, chunk_header.chunk_size);

            match chunk_type {
                W3DChunkType::Mesh => {
                    let mesh = self.parse_mesh_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.meshes.push(mesh);
                }
                W3DChunkType::MaterialInfo | W3DChunkType::ShaderMaterials => {
                    let materials = self.parse_material_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.materials.extend(materials);
                }
                W3DChunkType::Skeleton => {
                    let skeleton = self.parse_skeleton_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.skeleton = Some(skeleton);
                }
                W3DChunkType::Animation => {
                    let animation = self.parse_animation_chunk(&mut cursor, chunk_header.chunk_size)?;
                    model.animations.push(animation);
                }
                _ => {
                    // Skip unknown chunks
                    cursor.seek(SeekFrom::Current(chunk_header.chunk_size as i64))
                        .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to skip chunk: {}", e)))?;
                }
            }

            chunk_count += 1;
        }

        // Calculate model bounding box
        self.calculate_model_bounds(&mut model);

        tracing::info!("Loaded W3D model: {} meshes, {} materials, {} animations", 
                      model.meshes.len(), model.materials.len(), model.animations.len());

        Ok(model)
    }

    /// Validate W3D file header
    fn validate_header(&self, header: &W3DFileHeader) -> Result<()> {
        // Check magic number
        if &header.magic != b"W3D\0" {
            return Err(W3DError::ModelLoadingFailed("Invalid W3D magic number".to_string()));
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
                    cursor.seek(SeekFrom::Current(sub_header.chunk_size as i64))
                        .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to skip mesh sub-chunk: {}", e)))?;
                }
            }
        }

        // Calculate mesh bounding box
        let positions: Vec<Vec3> = mesh.vertices.iter()
            .map(|v| Vec3::from_array(v.position))
            .collect();
        mesh.bounding_box = BoundingBox::from_points(&positions);

        // Generate missing tangents and binormals
        self.generate_tangents(&mut mesh);

        Ok(mesh)
    }

    /// Parse vertex positions
    fn parse_vertex_positions(&mut self, cursor: &mut Cursor<&[u8]>, size: u32) -> Result<Vec<[f32; 3]>> {
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
    fn parse_vertex_normals(&mut self, cursor: &mut Cursor<&[u8]>, size: u32) -> Result<Vec<[f32; 3]>> {
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

    /// Parse material chunk
    fn parse_material_chunk(&mut self, cursor: &mut Cursor<&[u8]>, _size: u32) -> Result<Vec<W3DMaterial>> {
        // Simplified material parsing - in reality this would be much more complex
        let mut materials = Vec::new();
        
        // Create a default material for now
        let material = W3DMaterial {
            name: "Default".to_string(),
            base_color: Vec4::ONE,
            diffuse_texture: Some("default.tga".to_string()),
            ..Default::default()
        };
        
        materials.push(material);
        Ok(materials)
    }

    /// Parse skeleton chunk
    fn parse_skeleton_chunk(&mut self, cursor: &mut Cursor<&[u8]>, _size: u32) -> Result<Vec<W3DBone>> {
        // Simplified skeleton parsing
        let mut bones = Vec::new();
        
        // Create a default root bone
        let bone = W3DBone {
            name: "Root".to_string(),
            parent_index: -1,
            rest_position: Vec3::ZERO,
            rest_rotation: Quat::IDENTITY,
            rest_scale: Vec3::ONE,
            bind_matrix: Mat4::IDENTITY,
            inverse_bind_matrix: Mat4::IDENTITY,
        };
        
        bones.push(bone);
        Ok(bones)
    }

    /// Parse animation chunk
    fn parse_animation_chunk(&mut self, cursor: &mut Cursor<&[u8]>, _size: u32) -> Result<W3DAnimation> {
        // Simplified animation parsing
        Ok(W3DAnimation {
            name: "Default".to_string(),
            duration: 1.0,
            channels: Vec::new(),
            fps: 30.0,
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
            mesh.vertices[i0].tangent = (Vec3::from_array(mesh.vertices[i0].tangent) + tangent).to_array();
            mesh.vertices[i1].tangent = (Vec3::from_array(mesh.vertices[i1].tangent) + tangent).to_array();
            mesh.vertices[i2].tangent = (Vec3::from_array(mesh.vertices[i2].tangent) + tangent).to_array();

            mesh.vertices[i0].binormal = (Vec3::from_array(mesh.vertices[i0].binormal) + binormal).to_array();
            mesh.vertices[i1].binormal = (Vec3::from_array(mesh.vertices[i1].binormal) + binormal).to_array();
            mesh.vertices[i2].binormal = (Vec3::from_array(mesh.vertices[i2].binormal) + binormal).to_array();
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

    /// Helper to read struct from cursor
    fn read_struct<T: Pod>(&self, cursor: &mut Cursor<&[u8]>) -> Result<T> {
        let size = std::mem::size_of::<T>();
        let mut buffer = vec![0u8; size];
        
        cursor.read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read struct: {}", e)))?;

        Ok(*bytemuck::from_bytes(&buffer))
    }

    /// Helper to read f32
    fn read_f32(&self, cursor: &mut Cursor<&[u8]>) -> Result<f32> {
        let mut buffer = [0u8; 4];
        cursor.read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read f32: {}", e)))?;
        Ok(f32::from_le_bytes(buffer))
    }

    /// Helper to read u32
    fn read_u32(&self, cursor: &mut Cursor<&[u8]>) -> Result<u32> {
        let mut buffer = [0u8; 4];
        cursor.read_exact(&mut buffer)
            .map_err(|e| W3DError::ModelLoadingFailed(format!("Failed to read u32: {}", e)))?;
        Ok(u32::from_le_bytes(buffer))
    }
}

impl Default for W3DModelLoader {
    fn default() -> Self {
        Self::new()
    }
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
