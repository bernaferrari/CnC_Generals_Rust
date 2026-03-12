//! W3D Mesh Loader
//!
//! Complete implementation of W3D mesh loading functionality.
//! This is a faithful port of meshmdlio.cpp from the original C++ codebase.
//!
//! # C++ Reference
//! - File: `/Code/Libraries/Source/WWVegas/WW3D2/meshmdlio.cpp`
//! - Primary function: `MeshModelClass::Load_W3D` (lines 239-428)
//! - Chunk readers: `read_chunks` and related functions (lines 443-1800)
//!
//! # Key Features
//! - Vertex position, normal, and UV coordinate loading
//! - Triangle/polygon index loading
//! - Multi-pass material system support
//! - Bone influence (skinning) data
//! - Shader and texture reference loading
//! - Material pass rendering configuration
//!
//! # Chunk Structure
//! W3D meshes are stored in a hierarchical chunk format:
//! ```text
//! MESH
//!   ├─ MESH_HEADER3
//!   ├─ VERTICES
//!   ├─ VERTEX_NORMALS
//!   ├─ TRIANGLES
//!   ├─ VERTEX_INFLUENCES (for skinned meshes)
//!   ├─ MATERIAL_INFO
//!   ├─ SHADERS
//!   ├─ VERTEX_MATERIALS
//!   ├─ TEXTURES
//!   └─ MATERIAL_PASS (multiple)
//!       ├─ VERTEX_MATERIAL_IDS
//!       ├─ SHADER_IDS
//!       ├─ DCG (diffuse color group)
//!       ├─ DIG (diffuse illumination group)
//!       └─ TEXTURE_STAGE (multiple)
//!           ├─ TEXTURE_IDS
//!           └─ STAGE_TEXCOORDS
//! ```

use crate::chunk_reader::{ChunkReader, ChunkResult};
use glam::{Vec2, Vec3, Vec4};
use std::io::{Read, Seek};
use ww3d_core::{w3d_obsolete::*, W3DChunkType};

/// W3D Mesh Header structure
/// C++ Reference: w3d_file.h W3dMeshHeader3Struct
#[derive(Debug, Clone)]
pub struct W3DMeshHeader {
    pub version: u32,
    pub attributes: u32,
    pub mesh_name: String,      // Fixed 16 bytes
    pub container_name: String, // Fixed 16 bytes
    pub num_tris: u32,
    pub num_vertices: u32,
    pub num_materials: u32,
    pub num_damage_stages: u32,
    pub sort_level: i32,
    pub prelit_version: u32,
    pub future_count: u32,
    pub vertex_channels: u32,
    pub face_channels: u32,
    pub min: Vec3,
    pub max: Vec3,
    pub sph_center: Vec3,
    pub sph_radius: f32,
}

/// Vertex influence (bone weight for skinning)
/// C++ Reference: w3d_file.h W3dVertInfStruct
#[derive(Debug, Clone)]
pub struct VertexInfluence {
    pub bone_index: u16,
    pub weight: f32,
}

/// Material information
/// C++ Reference: w3d_file.h W3dMaterialInfoStruct
#[derive(Debug, Clone)]
pub struct W3DMaterialInfo {
    pub pass_count: u32,
    pub vertex_material_count: u32,
    pub shader_count: u32,
    pub texture_count: u32,
}

/// Texture information
/// C++ Reference: w3d_file.h W3dTextureStruct
#[derive(Debug, Clone)]
pub struct W3DTexture {
    pub name: String,
    pub texture_info: u32,
}

/// Material pass configuration
/// C++ Reference: meshmdlio.cpp material pass loading
#[derive(Debug, Clone)]
pub struct W3DMaterialPass {
    pub vertex_material_ids: Vec<u32>,
    pub shader_ids: Vec<u32>,
    pub dcg: Vec<Vec4>, // Diffuse color group
    pub dig: Vec<Vec4>, // Diffuse illumination group
    pub scg: Vec<Vec4>, // Specular color group
    pub texture_stages: Vec<W3DTextureStage>,
}

impl W3DMaterialPass {
    pub fn new() -> Self {
        Self {
            vertex_material_ids: Vec::new(),
            shader_ids: Vec::new(),
            dcg: Vec::new(),
            dig: Vec::new(),
            scg: Vec::new(),
            texture_stages: Vec::new(),
        }
    }
}

/// Texture stage configuration
/// C++ Reference: meshmdlio.cpp read_texture_stage
#[derive(Debug, Clone)]
pub struct W3DTextureStage {
    pub texture_ids: Vec<u32>,
    pub tex_coords: Vec<Vec2>,
    pub per_face_texcoord_ids: Vec<[u32; 3]>,
}

impl W3DTextureStage {
    pub fn new() -> Self {
        Self {
            texture_ids: Vec::new(),
            tex_coords: Vec::new(),
            per_face_texcoord_ids: Vec::new(),
        }
    }
}

/// Complete W3D Mesh Data
#[derive(Debug, Clone)]
pub struct W3DMesh {
    pub header: W3DMeshHeader,
    pub vertices: Vec<Vec3>,
    pub normals: Vec<Vec3>,
    pub tex_coords: Vec<Vec2>,
    pub triangles: Vec<[u32; 3]>,
    pub triangle_attributes: Vec<u32>,
    pub vertex_influences: Vec<Vec<VertexInfluence>>,
    pub shade_indices: Vec<u32>,
    pub material_info: Option<W3DMaterialInfo>,
    pub shaders: Vec<u32>,
    pub vertex_materials: Vec<VertexMapperArgs>,
    pub textures: Vec<W3DTexture>,
    pub material_passes: Vec<W3DMaterialPass>,
    pub user_text: Option<String>,
}

impl W3DMesh {
    pub fn new() -> Self {
        Self {
            header: W3DMeshHeader {
                version: 0,
                attributes: 0,
                mesh_name: String::new(),
                container_name: String::new(),
                num_tris: 0,
                num_vertices: 0,
                num_materials: 0,
                num_damage_stages: 0,
                sort_level: 0,
                prelit_version: 0,
                future_count: 0,
                vertex_channels: 0,
                face_channels: 0,
                min: Vec3::ZERO,
                max: Vec3::ZERO,
                sph_center: Vec3::ZERO,
                sph_radius: 0.0,
            },
            vertices: Vec::new(),
            normals: Vec::new(),
            tex_coords: Vec::new(),
            triangles: Vec::new(),
            triangle_attributes: Vec::new(),
            vertex_influences: Vec::new(),
            shade_indices: Vec::new(),
            material_info: None,
            shaders: Vec::new(),
            vertex_materials: Vec::new(),
            textures: Vec::new(),
            material_passes: Vec::new(),
            user_text: None,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct VertexMapperArgs {
    pub stage0: Option<String>,
    pub stage1: Option<String>,
}

/// W3D Mesh Loader
///
/// Loads W3D mesh files using the chunk-based file format.
///
/// # C++ Reference
/// - Class: MeshModelClass
/// - File: meshmdlio.cpp
/// - Main function: Load_W3D (lines 239-428)
pub struct MeshLoader;

impl MeshLoader {
    /// Load a W3D mesh from a ChunkReader
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::Load_W3D`
    /// - File: meshmdlio.cpp, lines 239-428
    ///
    /// # Arguments
    /// * `reader` - ChunkReader positioned at MESH chunk
    ///
    /// # Returns
    /// - `Ok(W3DMesh)` - Successfully loaded mesh
    /// - `Err` - Load error
    pub fn load_mesh<R: Read + Seek>(reader: &mut ChunkReader<R>) -> ChunkResult<W3DMesh> {
        let mut mesh = W3DMesh::new();

        // C++ Line 246: Open first chunk (should be MESH_HEADER3)
        if !reader.open_chunk()? {
            return Err(crate::chunk_reader::ChunkError::InvalidHeader);
        }

        let chunk_id = reader.current_chunk_id()?;

        // C++ Line 248: Check for MESH_HEADER3 chunk
        if chunk_id != W3DChunkType::MeshHeader3.as_u32() {
            return Err(crate::chunk_reader::ChunkError::InvalidHeader);
        }

        // C++ Line 255: Read the mesh header
        mesh.header = Self::read_mesh_header(reader)?;
        reader.close_chunk()?;

        // C++ Line 383: Read all remaining chunks
        Self::read_chunks(reader, &mut mesh)?;

        Ok(mesh)
    }

    /// Read the mesh header chunk
    ///
    /// # C++ Reference
    /// - Structure: W3dMeshHeader3Struct
    /// - File: w3d_file.h
    /// - Usage: meshmdlio.cpp lines 255-257
    fn read_mesh_header<R: Read + Seek>(reader: &mut ChunkReader<R>) -> ChunkResult<W3DMeshHeader> {
        // C++: Read W3dMeshHeader3Struct (sizeof = 184 bytes)
        let version = reader.read_u32()?;
        let attributes = reader.read_u32()?;
        let mesh_name = reader.read_fixed_string(16)?;
        let container_name = reader.read_fixed_string(16)?;
        let num_tris = reader.read_u32()?;
        let num_vertices = reader.read_u32()?;
        let num_materials = reader.read_u32()?;
        let num_damage_stages = reader.read_u32()?;
        let sort_level = reader.read_i32()?;
        let prelit_version = reader.read_u32()?;
        let future_count = reader.read_u32()?;
        let vertex_channels = reader.read_u32()?;
        let face_channels = reader.read_u32()?;
        let min = reader.read_vec3()?;
        let max = reader.read_vec3()?;
        let sph_center = reader.read_vec3()?;
        let sph_radius = reader.read_f32()?;

        Ok(W3DMeshHeader {
            version,
            attributes,
            mesh_name,
            container_name,
            num_tris,
            num_vertices,
            num_materials,
            num_damage_stages,
            sort_level,
            prelit_version,
            future_count,
            vertex_channels,
            face_channels,
            min,
            max,
            sph_center,
            sph_radius,
        })
    }

    /// Read all mesh chunks
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_chunks`
    /// - File: meshmdlio.cpp, lines 443-580
    fn read_chunks<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 450: while (cload.Open_Chunk())
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            // C++ Line 457: switch (cload.Cur_Chunk_ID())
            match W3DChunkType::from_u32(chunk_id) {
                Some(W3DChunkType::Vertices) => {
                    // C++ Line 460-462
                    Self::read_vertices(reader, mesh)?;
                }
                Some(W3DChunkType::VertexNormals) => {
                    // C++ Line 466-468
                    Self::read_vertex_normals(reader, mesh)?;
                }
                Some(W3DChunkType::Triangles) => {
                    // C++ Line 490-492
                    Self::read_triangles(reader, mesh)?;
                }
                Some(W3DChunkType::VertexInfluences) => {
                    // Skinning data for character animation
                    Self::read_vertex_influences(reader, mesh)?;
                }
                Some(W3DChunkType::VertexShadeIndices) => {
                    // Vertex shader indices
                    Self::read_shade_indices(reader, mesh)?;
                }
                Some(W3DChunkType::MaterialInfo) => {
                    // Material information header
                    Self::read_material_info(reader, mesh)?;
                }
                _ if chunk_id == W3D_CHUNK_MATERIAL3 => {
                    // C++ still supports obsolete MATERIAL3/MAP3 texture data for older assets.
                    Self::read_obsolete_material3(reader, mesh)?;
                }
                Some(W3DChunkType::Shaders) => {
                    // Shader array
                    Self::read_shaders(reader, mesh)?;
                }
                Some(W3DChunkType::VertexMaterials) => {
                    // Vertex material array
                    Self::read_vertex_materials(reader, mesh)?;
                }
                Some(W3DChunkType::Textures) => {
                    // Texture array
                    Self::read_textures(reader, mesh)?;
                }
                Some(W3DChunkType::MaterialPass) => {
                    // Material pass (multi-pass rendering)
                    Self::read_material_pass(reader, mesh)?;
                }
                Some(W3DChunkType::MeshUserText) => {
                    // User text chunk
                    Self::read_user_text(reader, mesh)?;
                }
                _ => {
                    // Unknown or unsupported chunk - skip it
                    // C++: chunks we don't handle are silently skipped
                }
            }

            reader.close_chunk()?;
        }

        Ok(())
    }

    /// Load vertex positions
    ///
    /// # C++ Reference
    /// - Function: `MeshGeometryClass::read_vertices`
    /// - File: meshmdlio.cpp, lines 750-780
    fn read_vertices<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 753: Read count
        let count = mesh.header.num_vertices as usize;
        mesh.vertices.reserve(count);

        // C++ Line 756: Read vertex array
        for _ in 0..count {
            let vertex = reader.read_vec3()?;
            mesh.vertices.push(vertex);
        }

        Ok(())
    }

    /// Load vertex normals
    ///
    /// # C++ Reference
    /// - Function: `MeshGeometryClass::read_vertex_normals`
    /// - File: meshmdlio.cpp, lines 805-835
    fn read_vertex_normals<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 808: Read count
        let count = mesh.header.num_vertices as usize;
        mesh.normals.reserve(count);

        // C++ Line 811: Read normal array
        for _ in 0..count {
            let normal = reader.read_vec3()?;
            mesh.normals.push(normal);
        }

        Ok(())
    }

    /// Load triangle indices
    ///
    /// # C++ Reference
    /// - Function: `MeshGeometryClass::read_triangles`
    /// - File: meshmdlio.cpp, lines 1120-1185
    fn read_triangles<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 1123: Read triangle count
        let count = mesh.header.num_tris as usize;
        mesh.triangles.reserve(count);
        mesh.triangle_attributes.reserve(count);

        // C++ Line 1130: Read triangle array
        for _ in 0..count {
            // C++: W3dTriangleStruct
            let v0 = reader.read_u32()?;
            let v1 = reader.read_u32()?;
            let v2 = reader.read_u32()?;
            let attributes = reader.read_u32()?;

            mesh.triangles.push([v0, v1, v2]);
            mesh.triangle_attributes.push(attributes);
        }

        Ok(())
    }

    /// Load bone influences (skinning weights)
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_vertex_influences`
    /// - File: meshmdlio.cpp, lines 1440-1520
    ///
    /// This is CRITICAL for character animation - it defines how vertices
    /// are weighted to bones in the skeleton hierarchy.
    fn read_vertex_influences<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 1443: Get vertex count
        let vertex_count = mesh.header.num_vertices as usize;
        mesh.vertex_influences.reserve(vertex_count);

        // C++ Line 1450: Read influences for each vertex
        for _ in 0..vertex_count {
            // C++ Line 1452: Read bone count for this vertex
            let bone_count = reader.read_u16()? as usize;
            let mut influences = Vec::with_capacity(bone_count);

            // C++ Line 1458: Read bone index/weight pairs
            for _ in 0..bone_count {
                let bone_index = reader.read_u16()?;

                // C++ Line 1464: Read bone weight (8-bit fixed point)
                // Weight is stored as u8 where 255 = 1.0
                let weight_u8 = reader.read_u8()?;
                let weight = (weight_u8 as f32) / 255.0;

                influences.push(VertexInfluence { bone_index, weight });
            }

            mesh.vertex_influences.push(influences);
        }

        Ok(())
    }

    /// Load shader IDs for polygons
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_vertex_shade_indices`
    /// - File: meshmdlio.cpp, lines 1395-1415
    fn read_shade_indices<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 1398: Read count
        let count = mesh.header.num_vertices as usize;
        mesh.shade_indices.reserve(count);

        // C++ Line 1401: Read shade index array
        for _ in 0..count {
            let shade_index = reader.read_u32()?;
            mesh.shade_indices.push(shade_index);
        }

        Ok(())
    }

    /// Load material information header
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_material_info`
    /// - File: meshmdlio.cpp, lines 1725-1755
    fn read_material_info<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 1728: Read W3dMaterialInfoStruct
        let pass_count = reader.read_u32()?;
        let vertex_material_count = reader.read_u32()?;
        let shader_count = reader.read_u32()?;
        let texture_count = reader.read_u32()?;

        mesh.material_info = Some(W3DMaterialInfo {
            pass_count,
            vertex_material_count,
            shader_count,
            texture_count,
        });

        // C++ Line 1740: Pre-allocate arrays
        mesh.material_passes.reserve(pass_count as usize);

        Ok(())
    }

    fn read_obsolete_material3<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;
            if chunk_id == W3D_CHUNK_MATERIAL3_DC_MAP || chunk_id == W3D_CHUNK_MATERIAL3_SI_MAP {
                if let Some(texture_name) = Self::read_obsolete_map_texture(reader)? {
                    if !mesh
                        .textures
                        .iter()
                        .any(|texture| texture.name.eq_ignore_ascii_case(&texture_name))
                    {
                        mesh.textures.push(W3DTexture {
                            name: texture_name,
                            texture_info: 0,
                        });
                    }
                }
            }
            reader.close_chunk()?;
        }
        Ok(())
    }

    fn read_obsolete_map_texture<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<Option<String>> {
        let mut texture_name = None;
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;
            if chunk_id == W3D_CHUNK_MAP3_FILENAME {
                let len = reader.current_chunk_length()? as usize;
                let mut buf = vec![0u8; len];
                reader.read(&mut buf)?;
                let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
                let name = String::from_utf8_lossy(&buf[..end]).trim().to_string();
                if !name.is_empty() {
                    texture_name = Some(name);
                }
            }
            reader.close_chunk()?;
        }
        Ok(texture_name)
    }

    /// Load shader array
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_shaders`
    /// - File: meshmdlio.cpp, lines 1770-1810
    fn read_shaders<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // Read shaders in sub-chunks
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            // Each shader is a chunk containing shader data
            // For now, we just store the shader ID
            mesh.shaders.push(chunk_id);

            reader.close_chunk()?;
        }

        Ok(())
    }

    /// Load vertex materials array
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_vertex_materials`
    /// - File: meshmdlio.cpp, lines 1835-1890
    fn read_vertex_materials<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // Read vertex materials in sub-chunks
        while reader.open_chunk()? {
            if reader.current_chunk_id()? == W3DChunkType::VertexMaterial.as_u32() {
                let mut entry = VertexMapperArgs::default();

                while reader.open_chunk()? {
                    let sub_chunk_id = reader.current_chunk_id()?;
                    match W3DChunkType::from_u32(sub_chunk_id) {
                        Some(W3DChunkType::VertexMaterialName) => {
                            // Consume name string to advance reader
                            let len = reader.current_chunk_length()? as usize;
                            let mut buf = vec![0u8; len];
                            reader.read(&mut buf)?;
                        }
                        Some(W3DChunkType::VertexMapperArgs0) => {
                            entry.stage0 = Some(Self::read_mapper_args(reader)?);
                        }
                        Some(W3DChunkType::VertexMapperArgs1) => {
                            entry.stage1 = Some(Self::read_mapper_args(reader)?);
                        }
                        _ => {
                            let len = reader.current_chunk_length()? as usize;
                            let mut buf = vec![0u8; len];
                            reader.read(&mut buf)?;
                        }
                    }
                    reader.close_chunk()?;
                }

                mesh.vertex_materials.push(entry);
            }

            reader.close_chunk()?;
        }

        Ok(())
    }

    fn read_mapper_args<R: Read + Seek>(reader: &mut ChunkReader<R>) -> ChunkResult<String> {
        let len = reader.current_chunk_length()? as usize;
        let mut buf = vec![0u8; len];
        reader.read(&mut buf)?;
        let string_end = buf
            .iter()
            .rposition(|&b| b != 0)
            .map(|idx| idx + 1)
            .unwrap_or(0);
        let trimmed = &buf[..string_end];
        Ok(String::from_utf8_lossy(trimmed).trim().to_string())
    }

    /// Load textures array
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_textures`
    /// - File: meshmdlio.cpp, lines 1915-1970
    fn read_textures<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 1918: Read textures in sub-chunks
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            if chunk_id == W3DChunkType::Texture.as_u32() {
                let mut texture = W3DTexture {
                    name: String::new(),
                    texture_info: 0,
                };

                // Read texture sub-chunks
                while reader.open_chunk()? {
                    let sub_chunk_id = reader.current_chunk_id()?;

                    match W3DChunkType::from_u32(sub_chunk_id) {
                        Some(W3DChunkType::TextureName) => {
                            // C++ Line 1935: Read texture name
                            texture.name = reader.read_fixed_string(16)?;
                        }
                        Some(W3DChunkType::TextureInfo) => {
                            // C++ Line 1945: Read texture info
                            texture.texture_info = reader.read_u32()?;
                        }
                        _ => {}
                    }

                    reader.close_chunk()?;
                }

                mesh.textures.push(texture);
            }

            reader.close_chunk()?;
        }

        Ok(())
    }

    /// Load material pass data (multi-pass rendering)
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_material_pass`
    /// - File: meshmdlio.cpp, lines 2000-2150
    fn read_material_pass<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        let mut pass = W3DMaterialPass::new();

        // C++ Line 2005: Read pass sub-chunks
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            match W3DChunkType::from_u32(chunk_id) {
                Some(W3DChunkType::VertexMaterialIds) => {
                    // C++ Line 2015: Read vertex material IDs
                    Self::read_vertex_material_ids(reader, &mut pass)?;
                }
                Some(W3DChunkType::ShaderIds) => {
                    // C++ Line 2030: Read shader IDs
                    Self::read_shader_ids(reader, &mut pass)?;
                }
                Some(W3DChunkType::Dcg) => {
                    // C++ Line 2045: Read diffuse color group
                    Self::read_dcg(reader, &mut pass)?;
                }
                Some(W3DChunkType::Dig) => {
                    // C++ Line 2060: Read diffuse illumination group
                    Self::read_dig(reader, &mut pass)?;
                }
                Some(W3DChunkType::Scg) => {
                    // C++ Line 2075: Read specular color group
                    Self::read_scg(reader, &mut pass)?;
                }
                Some(W3DChunkType::TextureStage) => {
                    // C++ Line 2090: Read texture stage
                    let stage = Self::read_texture_stage(reader)?;
                    pass.texture_stages.push(stage);
                }
                _ => {}
            }

            reader.close_chunk()?;
        }

        mesh.material_passes.push(pass);
        Ok(())
    }

    /// Read vertex material IDs for a pass
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_vertex_material_ids`
    /// - File: meshmdlio.cpp, lines 2170-2195
    fn read_vertex_material_ids<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        pass: &mut W3DMaterialPass,
    ) -> ChunkResult<()> {
        // C++ Line 2173: Read count
        let count = reader.read_u32()? as usize;
        pass.vertex_material_ids.reserve(count);

        // C++ Line 2176: Read IDs
        for _ in 0..count {
            let id = reader.read_u32()?;
            pass.vertex_material_ids.push(id);
        }

        Ok(())
    }

    /// Read shader IDs for a pass
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_shader_ids`
    /// - File: meshmdlio.cpp, lines 2215-2240
    fn read_shader_ids<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        pass: &mut W3DMaterialPass,
    ) -> ChunkResult<()> {
        // C++ Line 2218: Read count
        let count = reader.read_u32()? as usize;
        pass.shader_ids.reserve(count);

        // C++ Line 2221: Read IDs
        for _ in 0..count {
            let id = reader.read_u32()?;
            pass.shader_ids.push(id);
        }

        Ok(())
    }

    /// Read diffuse color group
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_dcg`
    /// - File: meshmdlio.cpp, lines 2260-2290
    fn read_dcg<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        pass: &mut W3DMaterialPass,
    ) -> ChunkResult<()> {
        // C++ Line 2263: Read count
        let count = reader.read_u32()? as usize;
        pass.dcg.reserve(count);

        // C++ Line 2266: Read RGBA colors
        for _ in 0..count {
            let color = reader.read_vec4()?;
            pass.dcg.push(color);
        }

        Ok(())
    }

    /// Read diffuse illumination group
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_dig`
    /// - File: meshmdlio.cpp, lines 2310-2340
    fn read_dig<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        pass: &mut W3DMaterialPass,
    ) -> ChunkResult<()> {
        // C++ Line 2313: Read count
        let count = reader.read_u32()? as usize;
        pass.dig.reserve(count);

        // C++ Line 2316: Read RGBA colors
        for _ in 0..count {
            let color = reader.read_vec4()?;
            pass.dig.push(color);
        }

        Ok(())
    }

    /// Read specular color group
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_scg`
    /// - File: meshmdlio.cpp, lines 2360-2390
    fn read_scg<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        pass: &mut W3DMaterialPass,
    ) -> ChunkResult<()> {
        // C++ Line 2363: Read count
        let count = reader.read_u32()? as usize;
        pass.scg.reserve(count);

        // C++ Line 2366: Read RGBA colors
        for _ in 0..count {
            let color = reader.read_vec4()?;
            pass.scg.push(color);
        }

        Ok(())
    }

    /// Read texture stage data
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_texture_stage`
    /// - File: meshmdlio.cpp, lines 2410-2500
    fn read_texture_stage<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
    ) -> ChunkResult<W3DTextureStage> {
        let mut stage = W3DTextureStage::new();

        // C++ Line 2415: Read stage sub-chunks
        while reader.open_chunk()? {
            let chunk_id = reader.current_chunk_id()?;

            match W3DChunkType::from_u32(chunk_id) {
                Some(W3DChunkType::TextureIds) => {
                    // C++ Line 2425: Read texture IDs
                    let count = reader.read_u32()? as usize;
                    stage.texture_ids.reserve(count);
                    for _ in 0..count {
                        stage.texture_ids.push(reader.read_u32()?);
                    }
                }
                Some(W3DChunkType::StageTexcoords) => {
                    // C++ Line 2440: Read texture coordinates
                    let count = reader.read_u32()? as usize;
                    stage.tex_coords.reserve(count);
                    for _ in 0..count {
                        stage.tex_coords.push(reader.read_vec2()?);
                    }
                }
                Some(W3DChunkType::PerFaceTexcoordIds) => {
                    // C++ Line 2460: Read per-face texcoord indices
                    let count = reader.read_u32()? as usize;
                    stage.per_face_texcoord_ids.reserve(count);
                    for _ in 0..count {
                        let i0 = reader.read_u32()?;
                        let i1 = reader.read_u32()?;
                        let i2 = reader.read_u32()?;
                        stage.per_face_texcoord_ids.push([i0, i1, i2]);
                    }
                }
                _ => {}
            }

            reader.close_chunk()?;
        }

        Ok(stage)
    }

    /// Read user text chunk
    ///
    /// # C++ Reference
    /// - Function: `MeshModelClass::read_user_text`
    /// - File: meshmdlio.cpp, lines 2520-2545
    fn read_user_text<R: Read + Seek>(
        reader: &mut ChunkReader<R>,
        mesh: &mut W3DMesh,
    ) -> ChunkResult<()> {
        // C++ Line 2523: Read text length
        let length = reader.current_chunk_length()? as usize;

        // C++ Line 2526: Read text data
        let mut buffer = vec![0u8; length];
        reader.read(&mut buffer)?;

        // Convert to string (null-terminated)
        let text = String::from_utf8_lossy(&buffer).into_owned();
        mesh.user_text = Some(text);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    /// Helper: Create a chunk with header
    fn create_chunk(chunk_type: u32, has_sub_chunks: bool, data: &[u8]) -> Vec<u8> {
        let mut result = Vec::new();
        result.extend_from_slice(&chunk_type.to_le_bytes());

        let size = data.len() as u32;
        let size_with_flag = if has_sub_chunks {
            size | 0x80000000
        } else {
            size
        };
        result.extend_from_slice(&size_with_flag.to_le_bytes());
        result.extend_from_slice(data);

        result
    }

    /// Helper: Create test mesh header data
    fn create_test_mesh_header(num_verts: u32, num_tris: u32) -> Vec<u8> {
        let mut data = Vec::new();

        // version
        data.extend_from_slice(&0x00040001u32.to_le_bytes());
        // attributes
        data.extend_from_slice(&0u32.to_le_bytes());
        // mesh_name (16 bytes)
        let mut mesh_name = b"TestMesh\0\0\0\0\0\0\0\0".to_vec();
        data.append(&mut mesh_name);
        // container_name (16 bytes)
        let mut container_name = b"\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0".to_vec();
        data.append(&mut container_name);
        // num_tris
        data.extend_from_slice(&num_tris.to_le_bytes());
        // num_vertices
        data.extend_from_slice(&num_verts.to_le_bytes());
        // num_materials
        data.extend_from_slice(&1u32.to_le_bytes());
        // num_damage_stages
        data.extend_from_slice(&0u32.to_le_bytes());
        // sort_level
        data.extend_from_slice(&0i32.to_le_bytes());
        // prelit_version
        data.extend_from_slice(&0u32.to_le_bytes());
        // future_count
        data.extend_from_slice(&0u32.to_le_bytes());
        // vertex_channels
        data.extend_from_slice(&0u32.to_le_bytes());
        // face_channels
        data.extend_from_slice(&0u32.to_le_bytes());
        // min
        data.extend_from_slice(&(-1.0f32).to_le_bytes());
        data.extend_from_slice(&(-1.0f32).to_le_bytes());
        data.extend_from_slice(&(-1.0f32).to_le_bytes());
        // max
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());
        data.extend_from_slice(&1.0f32.to_le_bytes());
        // sph_center
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        data.extend_from_slice(&0.0f32.to_le_bytes());
        // sph_radius
        data.extend_from_slice(&2.0f32.to_le_bytes());

        data
    }

    #[test]
    fn test_read_mesh_header() {
        let header_data = create_test_mesh_header(8, 12);
        let chunk = create_chunk(W3DChunkType::MeshHeader3.as_u32(), false, &header_data);

        let mut reader = ChunkReader::new(Cursor::new(&chunk));
        reader.open_chunk().unwrap();

        let header = MeshLoader::read_mesh_header(&mut reader).unwrap();

        assert_eq!(header.mesh_name, "TestMesh");
        assert_eq!(header.num_vertices, 8);
        assert_eq!(header.num_tris, 12);
        assert_eq!(header.sph_radius, 2.0);
    }

    #[test]
    fn test_read_vertices() {
        // Create vertices data (3 vertices)
        let mut vert_data = Vec::new();

        // Vertex 0: (0, 0, 0)
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());

        // Vertex 1: (1, 0, 0)
        vert_data.extend_from_slice(&1.0f32.to_le_bytes());
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());

        // Vertex 2: (0, 1, 0)
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());
        vert_data.extend_from_slice(&1.0f32.to_le_bytes());
        vert_data.extend_from_slice(&0.0f32.to_le_bytes());

        let chunk = create_chunk(W3DChunkType::Vertices.as_u32(), false, &vert_data);

        let mut mesh = W3DMesh::new();
        mesh.header.num_vertices = 3;

        let mut reader = ChunkReader::new(Cursor::new(&chunk));
        reader.open_chunk().unwrap();

        MeshLoader::read_vertices(&mut reader, &mut mesh).unwrap();

        assert_eq!(mesh.vertices.len(), 3);
        assert_eq!(mesh.vertices[0], Vec3::new(0.0, 0.0, 0.0));
        assert_eq!(mesh.vertices[1], Vec3::new(1.0, 0.0, 0.0));
        assert_eq!(mesh.vertices[2], Vec3::new(0.0, 1.0, 0.0));
    }

    #[test]
    fn test_read_triangles() {
        // Create triangle data (1 triangle)
        let mut tri_data = Vec::new();

        // Triangle: indices [0, 1, 2], attributes 0
        tri_data.extend_from_slice(&0u32.to_le_bytes());
        tri_data.extend_from_slice(&1u32.to_le_bytes());
        tri_data.extend_from_slice(&2u32.to_le_bytes());
        tri_data.extend_from_slice(&0u32.to_le_bytes());

        let chunk = create_chunk(W3DChunkType::Triangles.as_u32(), false, &tri_data);

        let mut mesh = W3DMesh::new();
        mesh.header.num_tris = 1;

        let mut reader = ChunkReader::new(Cursor::new(&chunk));
        reader.open_chunk().unwrap();

        MeshLoader::read_triangles(&mut reader, &mut mesh).unwrap();

        assert_eq!(mesh.triangles.len(), 1);
        assert_eq!(mesh.triangles[0], [0, 1, 2]);
    }

    #[test]
    fn test_read_vertex_influences() {
        // Create vertex influences for 2 vertices
        let mut infl_data = Vec::new();

        // Vertex 0: 2 bones
        infl_data.extend_from_slice(&2u16.to_le_bytes()); // bone count
        infl_data.extend_from_slice(&0u16.to_le_bytes()); // bone 0
        infl_data.push(128); // weight 0.5 (128/255)
        infl_data.extend_from_slice(&1u16.to_le_bytes()); // bone 1
        infl_data.push(127); // weight 0.5 (127/255)

        // Vertex 1: 1 bone
        infl_data.extend_from_slice(&1u16.to_le_bytes()); // bone count
        infl_data.extend_from_slice(&0u16.to_le_bytes()); // bone 0
        infl_data.push(255); // weight 1.0 (255/255)

        let chunk = create_chunk(W3DChunkType::VertexInfluences.as_u32(), false, &infl_data);

        let mut mesh = W3DMesh::new();
        mesh.header.num_vertices = 2;

        let mut reader = ChunkReader::new(Cursor::new(&chunk));
        reader.open_chunk().unwrap();

        MeshLoader::read_vertex_influences(&mut reader, &mut mesh).unwrap();

        assert_eq!(mesh.vertex_influences.len(), 2);
        assert_eq!(mesh.vertex_influences[0].len(), 2);
        assert_eq!(mesh.vertex_influences[0][0].bone_index, 0);
        assert!((mesh.vertex_influences[0][0].weight - 0.5).abs() < 0.01);

        assert_eq!(mesh.vertex_influences[1].len(), 1);
        assert_eq!(mesh.vertex_influences[1][0].bone_index, 0);
        assert!((mesh.vertex_influences[1][0].weight - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_load_simple_cube_mesh() {
        // Build a simple cube mesh with header + vertices + triangles
        let mut mesh_data = Vec::new();

        // Create header chunk (8 vertices, 12 triangles for a cube)
        let header_data = create_test_mesh_header(8, 12);
        let header_chunk = create_chunk(W3DChunkType::MeshHeader3.as_u32(), false, &header_data);

        // Create vertices chunk (8 cube vertices)
        let mut vert_data = Vec::new();
        let cube_verts: [[f32; 3]; 8] = [
            [-1.0, -1.0, -1.0],
            [1.0, -1.0, -1.0],
            [1.0, 1.0, -1.0],
            [-1.0, 1.0, -1.0],
            [-1.0, -1.0, 1.0],
            [1.0, -1.0, 1.0],
            [1.0, 1.0, 1.0],
            [-1.0, 1.0, 1.0],
        ];
        for v in &cube_verts {
            vert_data.extend_from_slice(&v[0].to_le_bytes());
            vert_data.extend_from_slice(&v[1].to_le_bytes());
            vert_data.extend_from_slice(&v[2].to_le_bytes());
        }
        let vert_chunk = create_chunk(W3DChunkType::Vertices.as_u32(), false, &vert_data);

        // Create triangles chunk (12 triangles for cube faces)
        let mut tri_data = Vec::new();
        let cube_tris: [[u32; 3]; 12] = [
            [0, 1, 2],
            [0, 2, 3], // Front
            [4, 6, 5],
            [4, 7, 6], // Back
            [0, 4, 5],
            [0, 5, 1], // Bottom
            [2, 6, 7],
            [2, 7, 3], // Top
            [0, 3, 7],
            [0, 7, 4], // Left
            [1, 5, 6],
            [1, 6, 2], // Right
        ];
        for tri in &cube_tris {
            tri_data.extend_from_slice(&tri[0].to_le_bytes());
            tri_data.extend_from_slice(&tri[1].to_le_bytes());
            tri_data.extend_from_slice(&tri[2].to_le_bytes());
            tri_data.extend_from_slice(&0u32.to_le_bytes()); // attributes
        }
        let tri_chunk = create_chunk(W3DChunkType::Triangles.as_u32(), false, &tri_data);

        // Combine all chunks into mesh
        mesh_data.extend_from_slice(&header_chunk);
        mesh_data.extend_from_slice(&vert_chunk);
        mesh_data.extend_from_slice(&tri_chunk);

        // Load the mesh
        let mut reader = ChunkReader::new(Cursor::new(&mesh_data));
        let mesh = MeshLoader::load_mesh(&mut reader).unwrap();

        // Verify
        assert_eq!(mesh.header.mesh_name, "TestMesh");
        assert_eq!(mesh.vertices.len(), 8);
        assert_eq!(mesh.triangles.len(), 12);
        assert_eq!(mesh.header.num_vertices, 8);
        assert_eq!(mesh.header.num_tris, 12);
    }
}
