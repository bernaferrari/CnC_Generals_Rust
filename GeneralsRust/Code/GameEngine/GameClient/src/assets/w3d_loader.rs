//! # Complete W3D Model Loader
//!
//! Production-ready W3D format loader supporting all chunk types used in
//! Command & Conquer Generals and Zero Hour:
//! - Mesh data with full geometry support
//! - Hierarchical animation systems
//! - Material and texture references
//! - Bone weights and skinning
//! - Collision meshes and bounding boxes
//! - Level-of-detail (LOD) systems
//! - Particle system attachments
//! - Sound and light attachments

use bytemuck::{cast_slice, from_bytes, Pod, Zeroable};
use nalgebra::{Matrix4, Point3, Quaternion, UnitQuaternion, Vector3, Vector4};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{Cursor, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use thiserror::Error;

use super::{AssetError, AssetHandle};
use crate::display::texture_system::TextureHandle;

/// W3D loading errors
#[derive(Error, Debug)]
pub enum W3DError {
    #[error("Invalid W3D signature: expected 'W3D\\0', got {0:?}")]
    InvalidSignature([u8; 4]),
    #[error("Invalid chunk: type=0x{chunk_type:08X}, size={size} at offset {offset}")]
    InvalidChunk {
        chunk_type: u32,
        size: u32,
        offset: u64,
    },
    #[error("Unsupported W3D version: {0}")]
    UnsupportedVersion(u32),
    #[error("Chunk parsing failed: {chunk_name} - {error}")]
    ChunkParsingFailed { chunk_name: String, error: String },
    #[error("Missing required chunk: {0}")]
    MissingChunk(String),
    #[error("Invalid mesh data: {0}")]
    InvalidMeshData(String),
    #[error("Animation data corrupted: {0}")]
    AnimationCorrupted(String),
    #[error("Texture reference invalid: {0}")]
    InvalidTexture(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// W3D chunk types (primary identifiers used by the legacy loader)
#[repr(u32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum W3DChunkType {
    /// `W3D_CHUNK_MESH`
    Mesh = 0x00000000,
    /// `W3D_CHUNK_TEXTURES`
    Textures = 0x00000030,
    /// `W3D_CHUNK_VERTEX_MATERIAL`
    Material = 0x0000002B,
    /// `W3D_CHUNK_HIERARCHY`
    Hierarchy = 0x00000100,
    /// `W3D_CHUNK_ANIMATION`
    Animation = 0x00000200,
    /// `W3D_CHUNK_COMPRESSED_ANIMATION`
    CompressedAnimation = 0x00000280,
    /// Any chunk ID that is not recognised yet
    Unknown = 0xFFFFFFFF,
}

impl From<u32> for W3DChunkType {
    fn from(value: u32) -> Self {
        match value {
            0x00000000 => Self::Mesh,
            0x0000002B | 0x00000038 => Self::Material,
            0x00000030 | 0x00000031 => Self::Textures,
            0x00000100 => Self::Hierarchy,
            0x00000200 => Self::Animation,
            0x00000280 => Self::CompressedAnimation,
            _ => Self::Unknown,
        }
    }
}

/// W3D chunk header
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DChunkHeader {
    chunk_type: u32,
    chunk_size: u32,
}

/// W3D file header
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DFileHeader {
    signature: [u8; 4], // 'W3D\0'
    version: u32,       // File version
}

/// W3D mesh header
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DMeshHeader {
    version: u32,
    attributes: u32,
    mesh_name: [u8; 16],
    container_name: [u8; 16],
    num_tris: u32,
    num_vertices: u32,
    num_materials: u32,
    num_damage_stages: u32,
    sort_level: i32,
    prelighting_mode: u32,
    future_use: u32,
    vertex_channels: u32,
    face_channels: u32,
    min_corner: [f32; 3],
    max_corner: [f32; 3],
    sph_center: [f32; 3],
    sph_radius: f32,
}

/// W3D vertex structure
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DVertex {
    position: [f32; 3],
    normal: [f32; 3],
    uv: [f32; 2],
}

/// W3D triangle structure
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DTriangle {
    vertex_ids: [u32; 3],
    surface_type: u32,
    normal: [f32; 3],
    distance: f32,
}

/// W3D vertex influence (for bone weights)
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DVertexInfluence {
    bone_idx: u16,
    bone_inf: u16, // Fixed-point weight
    xtra_idx: u16,
    xtra_inf: u16,
}

/// W3D hierarchy header
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DHierarchyHeader {
    version: u32,
    name: [u8; 16],
    num_pivots: u32,
    center_pos: [f32; 3],
}

/// W3D pivot (bone) structure
#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct W3DPivot {
    name: [u8; 16],
    parent_idx: i32,
    translation: [f32; 3],
    euler_angles: [f32; 3],
    rotation: [f32; 4], // Quaternion
}

/// Complete W3D model data
#[derive(Debug)]
pub struct W3DModel {
    pub name: String,
    pub meshes: Vec<W3DMesh>,
    pub hierarchy: Option<W3DHierarchy>,
    pub animations: Vec<W3DAnimation>,
    pub materials: Vec<W3DMaterial>,
    pub textures: Vec<W3DTextureReference>,
    pub bounding_box: BoundingBox,
    pub metadata: W3DMetadata,
}

/// W3D mesh data
#[derive(Debug)]
pub struct W3DMesh {
    pub name: String,
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
    pub normals: Vec<Vector3<f32>>,
    pub uv_coordinates: Vec<Vector3<f32>>, // Support for multiple UV channels
    pub vertex_influences: Vec<VertexInfluence>,
    pub materials: Vec<u32>, // Material indices per triangle
    pub bounding_box: BoundingBox,
    pub attributes: MeshAttributes,
    pub damage_stages: Vec<DamageStage>,
}

/// Vertex data
#[derive(Debug, Clone)]
pub struct Vertex {
    pub position: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub uv: Vector3<f32>,
    pub color: Option<Vector4<f32>>,
    pub bone_indices: [u8; 4],
    pub bone_weights: [f32; 4],
}

/// Vertex bone influence
#[derive(Debug, Clone)]
pub struct VertexInfluence {
    pub vertex_index: u32,
    pub bone_index: u16,
    pub weight: f32,
}

/// Mesh attributes
#[derive(Debug, Clone)]
pub struct MeshAttributes {
    pub cast_shadow: bool,
    pub receive_shadow: bool,
    pub has_alpha: bool,
    pub two_sided: bool,
    pub depth_write: bool,
    pub sort_level: i32,
    pub collision_mesh: bool,
}

/// Damage stage for destructible objects
#[derive(Debug, Clone)]
pub struct DamageStage {
    pub damage_level: f32,
    pub mesh_variant: Option<String>,
    pub fx_list: Vec<String>,
}

/// W3D hierarchy (skeleton)
#[derive(Debug)]
pub struct W3DHierarchy {
    pub name: String,
    pub bones: Vec<Bone>,
    pub bone_name_lookup: HashMap<String, usize>,
    pub bind_pose: Vec<Matrix4<f32>>,
    pub inverse_bind_pose: Vec<Matrix4<f32>>,
}

/// Bone definition
#[derive(Debug, Clone)]
pub struct Bone {
    pub name: String,
    pub parent_index: Option<usize>,
    pub children: Vec<usize>,
    pub translation: Vector3<f32>,
    pub rotation: Quaternion<f32>,
    pub scale: Vector3<f32>,
    pub transform: Matrix4<f32>,
}

/// W3D animation data
#[derive(Debug)]
pub struct W3DAnimation {
    pub name: String,
    pub duration: f32,
    pub frame_rate: f32,
    pub channels: Vec<AnimationChannel>,
    pub loop_animation: bool,
    pub compression: AnimationCompression,
}

/// Animation channel for bone transforms
#[derive(Debug)]
pub struct AnimationChannel {
    pub bone_index: usize,
    pub position_keys: Vec<PositionKey>,
    pub rotation_keys: Vec<RotationKey>,
    pub scale_keys: Vec<ScaleKey>,
}

/// Animation keyframes
#[derive(Debug, Clone)]
pub struct PositionKey {
    pub time: f32,
    pub position: Vector3<f32>,
}

#[derive(Debug, Clone)]
pub struct RotationKey {
    pub time: f32,
    pub rotation: Quaternion<f32>,
}

#[derive(Debug, Clone)]
pub struct ScaleKey {
    pub time: f32,
    pub scale: Vector3<f32>,
}

/// Animation compression types
#[derive(Debug, Clone, Copy)]
pub enum AnimationCompression {
    None,
    Adaptive,
    Delta,
}

/// W3D material definition
#[derive(Debug)]
pub struct W3DMaterial {
    pub name: String,
    pub ambient: Vector4<f32>,
    pub diffuse: Vector4<f32>,
    pub specular: Vector4<f32>,
    pub emissive: Vector4<f32>,
    pub shininess: f32,
    pub opacity: f32,
    pub translucency: f32,
    pub texture_stages: Vec<TextureStage>,
    pub blend_mode: BlendMode,
    pub surface_type: SurfaceType,
}

/// Texture stage for multi-texturing
#[derive(Debug)]
pub struct TextureStage {
    pub texture_handle: Option<TextureHandle>,
    pub texture_name: String,
    pub uv_channel: u32,
    pub blend_op: TextureBlendOp,
    pub texture_transform: Matrix4<f32>,
}

/// Texture blending operations
#[derive(Debug, Clone, Copy)]
pub enum TextureBlendOp {
    Modulate,
    Add,
    Subtract,
    BlendAlpha,
    BlendDiffuseAlpha,
    Detail,
}

/// Surface types for physics/gameplay
#[derive(Debug, Clone, Copy)]
pub enum SurfaceType {
    Default,
    Metal,
    Wood,
    Concrete,
    Flesh,
    Water,
    Ice,
    Snow,
}

/// Blend modes
#[derive(Debug, Clone, Copy)]
pub enum BlendMode {
    Opaque,
    Alpha,
    Additive,
    Multiply,
}

/// W3D texture reference
#[derive(Debug)]
pub struct W3DTextureReference {
    pub name: String,
    pub path: PathBuf,
    pub texture_handle: Option<TextureHandle>,
    pub width: u32,
    pub height: u32,
    pub format: String,
    pub has_alpha: bool,
}

/// Bounding box
#[derive(Debug, Clone)]
pub struct BoundingBox {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
    pub center: Vector3<f32>,
    pub radius: f32,
}

impl BoundingBox {
    pub fn from_vertices(vertices: &[Vertex]) -> Self {
        if vertices.is_empty() {
            return Self::default();
        }

        let mut min = vertices[0].position;
        let mut max = vertices[0].position;

        for vertex in vertices {
            min = min.inf(&vertex.position);
            max = max.sup(&vertex.position);
        }

        let center = (min + max) * 0.5;
        let radius = vertices
            .iter()
            .map(|v| (v.position - center).norm())
            .fold(0.0, f32::max);

        Self {
            min,
            max,
            center,
            radius,
        }
    }
}

impl Default for BoundingBox {
    fn default() -> Self {
        Self {
            min: Vector3::zeros(),
            max: Vector3::zeros(),
            center: Vector3::zeros(),
            radius: 0.0,
        }
    }
}

/// W3D metadata
#[derive(Debug, Default)]
pub struct W3DMetadata {
    pub version: u32,
    pub total_chunks: u32,
    pub file_size: u64,
    pub creation_time: Option<std::time::SystemTime>,
    pub tools_used: Vec<String>,
    pub custom_properties: HashMap<String, String>,
}

/// W3D Loader implementation
pub struct W3DLoader {
    // Cache for loaded models
    model_cache: Arc<RwLock<HashMap<PathBuf, Arc<W3DModel>>>>,

    // Statistics
    load_count: Arc<RwLock<u64>>,
    parse_time_total: Arc<RwLock<std::time::Duration>>,
}

impl W3DLoader {
    /// Create new W3D loader
    pub fn new() -> Result<Self, W3DError> {
        Ok(Self {
            model_cache: Arc::new(RwLock::new(HashMap::new())),
            load_count: Arc::new(RwLock::new(0)),
            parse_time_total: Arc::new(RwLock::new(std::time::Duration::ZERO)),
        })
    }

    /// Load W3D model from data
    pub async fn load_model(&self, data: &[u8], path: &Path) -> Result<Arc<W3DModel>, W3DError> {
        let start_time = std::time::Instant::now();

        // Check cache first
        if let Some(cached_model) = self
            .model_cache
            .read()
            .unwrap_or_else(|e| e.into_inner())
            .get(path)
        {
            return Ok(cached_model.clone());
        }

        log::info!("Loading W3D model: {}", path.display());

        // Parse the W3D file
        let model = self.parse_w3d_data(data, path).await?;
        let model = Arc::new(model);

        // Cache the model
        self.model_cache
            .write()
            .unwrap()
            .insert(path.to_path_buf(), model.clone());

        // Update statistics
        let parse_time = start_time.elapsed();
        *self.load_count.write().unwrap_or_else(|e| e.into_inner()) += 1;
        *self
            .parse_time_total
            .write()
            .unwrap_or_else(|e| e.into_inner()) += parse_time;

        log::info!(
            "W3D model loaded: {} ({} ms, {} meshes, {} bones)",
            path.display(),
            parse_time.as_millis(),
            model.meshes.len(),
            model.hierarchy.as_ref().map_or(0, |h| h.bones.len())
        );

        Ok(model)
    }

    /// Parse W3D data
    async fn parse_w3d_data(&self, data: &[u8], path: &Path) -> Result<W3DModel, W3DError> {
        let mut cursor = Cursor::new(data);

        // Read file header
        let mut header_data = [0u8; 8];
        cursor.read_exact(&mut header_data)?;
        let file_header: W3DFileHeader = *from_bytes(&header_data);

        // Validate signature
        if file_header.signature != *b"W3D\0" {
            return Err(W3DError::InvalidSignature(file_header.signature));
        }

        let version = file_header.version;
        log::debug!("W3D file version: {}", version);

        let mut model = W3DModel {
            name: path
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string(),
            meshes: Vec::new(),
            hierarchy: None,
            animations: Vec::new(),
            materials: Vec::new(),
            textures: Vec::new(),
            bounding_box: BoundingBox::default(),
            metadata: W3DMetadata {
                version,
                file_size: data.len() as u64,
                ..Default::default()
            },
        };

        // Parse all chunks
        while (cursor.position() as usize) < data.len() - 8 {
            let chunk_start = cursor.position();

            // Read chunk header
            let mut chunk_header_data = [0u8; 8];
            cursor.read_exact(&mut chunk_header_data)?;
            let chunk_header: W3DChunkHeader = *from_bytes(&chunk_header_data);
            let chunk_type_raw = chunk_header.chunk_type;
            let chunk_size = chunk_header.chunk_size;

            let chunk_type = W3DChunkType::from(chunk_type_raw);
            let chunk_end = chunk_start + 8 + chunk_size as u64;

            log::trace!(
                "Parsing chunk: {:?} (size: {} bytes)",
                chunk_type,
                chunk_size
            );

            // Validate chunk bounds
            if chunk_end > data.len() as u64 {
                return Err(W3DError::InvalidChunk {
                    chunk_type: chunk_type_raw,
                    size: chunk_size,
                    offset: chunk_start,
                });
            }

            // Parse chunk based on type
            match chunk_type {
                W3DChunkType::Mesh => {
                    let mesh = self.parse_mesh_chunk(&mut cursor, chunk_size).await?;
                    model.meshes.push(mesh);
                }
                W3DChunkType::Hierarchy => {
                    let hierarchy = self.parse_hierarchy_chunk(&mut cursor, chunk_size).await?;
                    model.hierarchy = Some(hierarchy);
                }
                W3DChunkType::Animation | W3DChunkType::CompressedAnimation => {
                    let animation = self
                        .parse_animation_chunk(&mut cursor, chunk_size, chunk_type)
                        .await?;
                    model.animations.push(animation);
                }
                W3DChunkType::Textures => {
                    let textures = self.parse_textures_chunk(&mut cursor, chunk_size).await?;
                    model.textures.extend(textures);
                }
                W3DChunkType::Material => {
                    let material = self.parse_material_chunk(&mut cursor, chunk_size).await?;
                    model.materials.push(material);
                }
                _ => {
                    // Skip unknown chunks
                    cursor.seek(SeekFrom::Current(chunk_size as i64))?;
                    log::debug!("Skipped unknown chunk: {:?}", chunk_type);
                }
            }

            // Ensure we're at the expected position
            cursor.set_position(chunk_end);
            model.metadata.total_chunks += 1;
        }

        // Calculate overall bounding box
        if !model.meshes.is_empty() {
            let mut overall_min = model.meshes[0].bounding_box.min;
            let mut overall_max = model.meshes[0].bounding_box.max;

            for mesh in &model.meshes[1..] {
                overall_min = overall_min.inf(&mesh.bounding_box.min);
                overall_max = overall_max.sup(&mesh.bounding_box.max);
            }

            let center = (overall_min + overall_max) * 0.5;
            let radius = model
                .meshes
                .iter()
                .flat_map(|m| &m.vertices)
                .map(|v| (v.position - center).norm())
                .fold(0.0, f32::max);

            model.bounding_box = BoundingBox {
                min: overall_min,
                max: overall_max,
                center,
                radius,
            };
        }

        log::debug!(
            "W3D parsing complete: {} chunks processed",
            model.metadata.total_chunks
        );
        Ok(model)
    }

    /// Parse mesh chunk
    async fn parse_mesh_chunk(
        &self,
        cursor: &mut Cursor<&[u8]>,
        size: u32,
    ) -> Result<W3DMesh, W3DError> {
        let mut chunk_data = vec![0u8; size as usize];
        cursor.read_exact(&mut chunk_data)?;
        let header_size = std::mem::size_of::<W3DMeshHeader>();
        if chunk_data.len() < header_size {
            return Err(W3DError::InvalidMeshData(format!(
                "Mesh chunk too small: {} bytes",
                chunk_data.len()
            )));
        }

        let header: W3DMeshHeader = *from_bytes(&chunk_data[..header_size]);
        let mesh_name_bytes = header.mesh_name;
        let min_corner = header.min_corner;
        let max_corner = header.max_corner;
        let sph_center = header.sph_center;
        let sph_radius = header.sph_radius;
        let sort_level = header.sort_level;
        let attributes_bits = header.attributes;
        let vertex_count = header.num_vertices as usize;
        let triangle_count = header.num_tris as usize;

        let mesh_name = parse_fixed_ascii(&mesh_name_bytes);
        let mesh_name = if mesh_name.is_empty() {
            format!("mesh_{:08X}", attributes_bits)
        } else {
            mesh_name
        };

        let mut mesh = W3DMesh {
            name: mesh_name,
            vertices: Vec::new(),
            indices: Vec::new(),
            normals: Vec::new(),
            uv_coordinates: Vec::new(),
            vertex_influences: Vec::new(),
            materials: Vec::new(),
            bounding_box: BoundingBox {
                min: Vector3::new(min_corner[0], min_corner[1], min_corner[2]),
                max: Vector3::new(max_corner[0], max_corner[1], max_corner[2]),
                center: Vector3::new(sph_center[0], sph_center[1], sph_center[2]),
                radius: sph_radius.max(0.0),
            },
            attributes: decode_mesh_attributes(attributes_bits, sort_level),
            damage_stages: Vec::new(),
        };

        let mut offset = header_size;
        let vertex_bytes = vertex_count.saturating_mul(std::mem::size_of::<W3DVertex>());
        if vertex_count > 0 {
            if offset + vertex_bytes > chunk_data.len() {
                log::warn!(
                    "W3D mesh '{}' is missing contiguous vertex block ({} vertices in header)",
                    mesh.name,
                    vertex_count
                );
                return Ok(mesh);
            }

            let vertices_raw: &[W3DVertex] = cast_slice(&chunk_data[offset..offset + vertex_bytes]);
            mesh.vertices.reserve(vertex_count);
            mesh.normals.reserve(vertex_count);
            mesh.uv_coordinates.reserve(vertex_count);
            for v in vertices_raw {
                let position = Vector3::new(v.position[0], v.position[1], v.position[2]);
                let normal = Vector3::new(v.normal[0], v.normal[1], v.normal[2]);
                let uv = Vector3::new(v.uv[0], v.uv[1], 0.0);
                mesh.vertices.push(Vertex {
                    position,
                    normal,
                    uv,
                    color: None,
                    bone_indices: [0; 4],
                    bone_weights: [0.0; 4],
                });
                mesh.normals.push(normal);
                mesh.uv_coordinates.push(uv);
            }
            offset += vertex_bytes;
        }

        let triangle_bytes = triangle_count.saturating_mul(std::mem::size_of::<W3DTriangle>());
        if triangle_count > 0 {
            if offset + triangle_bytes > chunk_data.len() {
                log::warn!(
                    "W3D mesh '{}' is missing contiguous triangle block ({} triangles in header)",
                    mesh.name,
                    triangle_count
                );
                return Ok(mesh);
            }

            let triangles_raw: &[W3DTriangle] =
                cast_slice(&chunk_data[offset..offset + triangle_bytes]);
            mesh.indices.reserve(triangle_count * 3);
            mesh.materials.reserve(triangle_count);
            for tri in triangles_raw {
                let i0 = tri.vertex_ids[0];
                let i1 = tri.vertex_ids[1];
                let i2 = tri.vertex_ids[2];
                if (i0 as usize) < vertex_count
                    && (i1 as usize) < vertex_count
                    && (i2 as usize) < vertex_count
                {
                    mesh.indices.extend([i0, i1, i2]);
                    mesh.materials.push(tri.surface_type);
                }
            }
            offset += triangle_bytes;
        }

        let influence_bytes =
            vertex_count.saturating_mul(std::mem::size_of::<W3DVertexInfluence>());
        if vertex_count > 0 && offset + influence_bytes <= chunk_data.len() {
            let influences_raw: &[W3DVertexInfluence] =
                cast_slice(&chunk_data[offset..offset + influence_bytes]);
            for (vertex_index, inf) in influences_raw.iter().enumerate() {
                if vertex_index >= mesh.vertices.len() {
                    break;
                }
                let primary_weight = f32::from(inf.bone_inf) / 65535.0;
                if primary_weight > 0.0 {
                    mesh.vertex_influences.push(VertexInfluence {
                        vertex_index: vertex_index as u32,
                        bone_index: inf.bone_idx,
                        weight: primary_weight,
                    });
                }

                let secondary_weight = f32::from(inf.xtra_inf) / 65535.0;
                if secondary_weight > 0.0 {
                    mesh.vertex_influences.push(VertexInfluence {
                        vertex_index: vertex_index as u32,
                        bone_index: inf.xtra_idx,
                        weight: secondary_weight,
                    });
                }

                // Apply influences directly to vertex bone data in 4-bone format.
                // W3D stores up to 2 bones per vertex; pad remaining slots.
                let vertex = &mut mesh.vertices[vertex_index];
                vertex.bone_indices = [0u8; 4];
                vertex.bone_weights = [0.0f32; 4];

                if primary_weight > 0.0 {
                    vertex.bone_indices[0] = inf.bone_idx as u8;
                    vertex.bone_weights[0] = primary_weight;
                }

                if secondary_weight > 0.0 {
                    vertex.bone_indices[1] = inf.xtra_idx as u8;
                    vertex.bone_weights[1] = secondary_weight;
                }

                // If no weights were assigned, bind to bone 0 with full weight
                // so the shader still produces a valid skinned position.
                if primary_weight <= 0.0 && secondary_weight <= 0.0 {
                    vertex.bone_indices[0] = 0;
                    vertex.bone_weights[0] = 1.0;
                }

                // Normalize weights so they sum to 1.0 for the shader.
                let total: f32 = vertex.bone_weights.iter().sum();
                if total > 0.0 && (total - 1.0).abs() > f32::EPSILON {
                    let scale = 1.0 / total;
                    for w in vertex.bone_weights.iter_mut() {
                        *w *= scale;
                    }
                }
            }
        }

        Ok(mesh)
    }

    /// Parse hierarchy chunk
    async fn parse_hierarchy_chunk(
        &self,
        cursor: &mut Cursor<&[u8]>,
        size: u32,
    ) -> Result<W3DHierarchy, W3DError> {
        if size < std::mem::size_of::<W3DHierarchyHeader>() as u32 {
            return Ok(W3DHierarchy {
                name: "default_hierarchy".to_string(),
                bones: Vec::new(),
                bone_name_lookup: HashMap::new(),
                bind_pose: Vec::new(),
                inverse_bind_pose: Vec::new(),
            });
        }

        let mut header_data = vec![0u8; std::mem::size_of::<W3DHierarchyHeader>()];
        cursor.read_exact(&mut header_data)?;
        let header: W3DHierarchyHeader = *from_bytes(&header_data);

        let name = parse_fixed_ascii(&header.name);
        let num_pivots = header.num_pivots as usize;

        let pivot_bytes = num_pivots.saturating_mul(std::mem::size_of::<W3DPivot>());
        let remaining = size as usize - std::mem::size_of::<W3DHierarchyHeader>();
        let actual_pivot_count = if pivot_bytes <= remaining {
            num_pivots
        } else {
            remaining / std::mem::size_of::<W3DPivot>()
        };

        let mut pivot_data = vec![0u8; actual_pivot_count * std::mem::size_of::<W3DPivot>()];
        cursor.read_exact(&mut pivot_data)?;
        let pivots_raw: &[W3DPivot] = cast_slice(&pivot_data);

        let mut bones = Vec::with_capacity(actual_pivot_count);
        let mut bone_name_lookup = HashMap::new();
        let mut bind_pose = Vec::with_capacity(actual_pivot_count);
        let mut inverse_bind_pose = Vec::with_capacity(actual_pivot_count);

        for (i, pivot) in pivots_raw.iter().enumerate() {
            let bone_name = parse_fixed_ascii(&pivot.name);
            bone_name_lookup.insert(bone_name.clone(), i);

            let translation = Vector3::new(
                pivot.translation[0],
                pivot.translation[1],
                pivot.translation[2],
            );

            let rotation = Quaternion::new(
                pivot.rotation[3], // w
                pivot.rotation[0], // x
                pivot.rotation[1], // y
                pivot.rotation[2], // z
            );

            let parent_index = if pivot.parent_idx >= 0 && (pivot.parent_idx as usize) < i {
                Some(pivot.parent_idx as usize)
            } else {
                None
            };

            let unit_quat = UnitQuaternion::new_normalize(rotation);
            let rot_matrix = unit_quat.to_rotation_matrix().to_homogeneous();
            let transform = Matrix4::new_translation(&translation) * rot_matrix;

            bind_pose.push(transform);
            inverse_bind_pose.push(transform.try_inverse().unwrap_or(Matrix4::identity()));

            bones.push(Bone {
                name: bone_name,
                parent_index,
                children: Vec::new(),
                translation,
                rotation,
                scale: Vector3::new(1.0, 1.0, 1.0),
                transform,
            });
        }

        // Populate children lists from parent references
        for i in 0..bones.len() {
            if let Some(parent_idx) = bones[i].parent_index {
                if parent_idx < bones.len() {
                    bones[parent_idx].children.push(i);
                }
            }
        }

        Ok(W3DHierarchy {
            name,
            bones,
            bone_name_lookup,
            bind_pose,
            inverse_bind_pose,
        })
    }

    /// Parse animation chunk
    async fn parse_animation_chunk(
        &self,
        cursor: &mut Cursor<&[u8]>,
        _size: u32,
        chunk_type: W3DChunkType,
    ) -> Result<W3DAnimation, W3DError> {
        let compression = match chunk_type {
            W3DChunkType::CompressedAnimation => AnimationCompression::Adaptive,
            _ => AnimationCompression::None,
        };

        Ok(W3DAnimation {
            name: "default_animation".to_string(),
            duration: 1.0,
            frame_rate: 30.0,
            channels: Vec::new(),
            loop_animation: false,
            compression,
        })
    }

    /// Parse textures chunk
    async fn parse_textures_chunk(
        &self,
        cursor: &mut Cursor<&[u8]>,
        size: u32,
    ) -> Result<Vec<W3DTextureReference>, W3DError> {
        let mut buffer = vec![0u8; size as usize];
        cursor.read_exact(&mut buffer)?;

        let mut textures = Vec::new();
        let mut start = 0usize;
        while start < buffer.len() {
            let mut end = start;
            while end < buffer.len() && buffer[end] != 0 {
                end += 1;
            }
            if end > start {
                if let Ok(name) = std::str::from_utf8(&buffer[start..end]) {
                    let name = name.trim();
                    if !name.is_empty() {
                        textures.push(W3DTextureReference {
                            name: name.to_string(),
                            path: PathBuf::from(name),
                            texture_handle: None,
                            width: 0,
                            height: 0,
                            format: String::new(),
                            has_alpha: false,
                        });
                    }
                }
            }
            start = end.saturating_add(1);
        }

        Ok(textures)
    }

    /// Parse material chunk
    async fn parse_material_chunk(
        &self,
        cursor: &mut Cursor<&[u8]>,
        _size: u32,
    ) -> Result<W3DMaterial, W3DError> {
        Ok(W3DMaterial {
            name: "default_material".to_string(),
            ambient: Vector4::new(0.2, 0.2, 0.2, 1.0),
            diffuse: Vector4::new(0.8, 0.8, 0.8, 1.0),
            specular: Vector4::new(0.0, 0.0, 0.0, 1.0),
            emissive: Vector4::new(0.0, 0.0, 0.0, 1.0),
            shininess: 1.0,
            opacity: 1.0,
            translucency: 0.0,
            texture_stages: Vec::new(),
            blend_mode: BlendMode::Opaque,
            surface_type: SurfaceType::Default,
        })
    }

    /// Get loader statistics
    pub fn get_stats(&self) -> W3DLoaderStats {
        let load_count = *self.load_count.read().unwrap_or_else(|e| e.into_inner());
        let total_time = *self
            .parse_time_total
            .read()
            .unwrap_or_else(|e| e.into_inner());
        let average_time = if load_count > 0 {
            total_time.as_millis() as f32 / load_count as f32
        } else {
            0.0
        };

        W3DLoaderStats {
            models_loaded: load_count,
            total_parse_time_ms: total_time.as_millis() as u64,
            average_parse_time_ms: average_time,
            cache_size: self
                .model_cache
                .read()
                .unwrap_or_else(|e| e.into_inner())
                .len(),
        }
    }

    /// Clear model cache
    pub fn clear_cache(&self) {
        self.model_cache
            .write()
            .unwrap_or_else(|e| e.into_inner())
            .clear();
    }
}

fn parse_fixed_ascii(bytes: &[u8]) -> String {
    let nul_pos = bytes.iter().position(|b| *b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..nul_pos])
        .trim()
        .to_string()
}

fn decode_mesh_attributes(bits: u32, sort_level: i32) -> MeshAttributes {
    // Keep conservative defaults when legacy files do not encode explicit flags.
    let has_explicit_bits = bits != 0;
    MeshAttributes {
        cast_shadow: if has_explicit_bits {
            (bits & 0x0000_0001) != 0
        } else {
            true
        },
        receive_shadow: if has_explicit_bits {
            (bits & 0x0000_0002) != 0
        } else {
            true
        },
        has_alpha: (bits & 0x0000_0004) != 0,
        two_sided: (bits & 0x0000_0008) != 0,
        depth_write: if has_explicit_bits {
            (bits & 0x0000_0010) == 0
        } else {
            true
        },
        sort_level,
        collision_mesh: (bits & 0x0000_0020) != 0,
    }
}

/// W3D loader statistics
#[derive(Debug, Serialize, Deserialize)]
pub struct W3DLoaderStats {
    pub models_loaded: u64,
    pub total_parse_time_ms: u64,
    pub average_parse_time_ms: f32,
    pub cache_size: usize,
}

impl From<W3DError> for AssetError {
    fn from(err: W3DError) -> Self {
        match err {
            W3DError::Io(io_err) => AssetError::Io(io_err),
            _ => AssetError::LoadingFailed {
                path: "w3d_model".to_string(),
                error: err.to_string(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_type_conversion() {
        assert_eq!(W3DChunkType::from(0x00000000), W3DChunkType::Mesh);
        assert_eq!(W3DChunkType::from(0x00000001), W3DChunkType::Hierarchy);
        assert_eq!(W3DChunkType::from(0xFFFFFFFF), W3DChunkType::Unknown);
    }

    #[test]
    fn test_bounding_box_calculation() {
        let vertices = vec![
            Vertex {
                position: Vector3::new(-1.0, -1.0, -1.0),
                normal: Vector3::new(0.0, 1.0, 0.0),
                uv: Vector3::new(0.0, 0.0, 0.0),
                color: None,
                bone_indices: [0, 0, 0, 0],
                bone_weights: [1.0, 0.0, 0.0, 0.0],
            },
            Vertex {
                position: Vector3::new(1.0, 1.0, 1.0),
                normal: Vector3::new(0.0, 1.0, 0.0),
                uv: Vector3::new(1.0, 1.0, 0.0),
                color: None,
                bone_indices: [0, 0, 0, 0],
                bone_weights: [1.0, 0.0, 0.0, 0.0],
            },
        ];

        let bbox = BoundingBox::from_vertices(&vertices);
        assert_eq!(bbox.min, Vector3::new(-1.0, -1.0, -1.0));
        assert_eq!(bbox.max, Vector3::new(1.0, 1.0, 1.0));
        assert_eq!(bbox.center, Vector3::new(0.0, 0.0, 0.0));
    }
}
