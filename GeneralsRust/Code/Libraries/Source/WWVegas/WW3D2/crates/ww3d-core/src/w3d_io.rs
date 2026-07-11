/// W3D file I/O implementation
///
/// This module provides reading and writing capabilities for W3D files,
/// supporting all chunk types defined in the W3D format specification.
use crate::chunks::W3DChunkType;
use crate::errors::{W3DError, W3DResult};
use crate::w3d_format::*;
use binrw::{BinRead, BinWrite};
use std::io::{Cursor, Read, Seek, SeekFrom, Write};
use std::path::Path;

/// W3D file reader
///
/// Provides methods to read W3D files and parse all chunk types.
pub struct W3DReader<R: Read + Seek> {
    reader: R,
}

impl<R: Read + Seek> W3DReader<R> {
    /// Create a new W3D reader from any Read + Seek source
    pub fn new(reader: R) -> Self {
        Self { reader }
    }

    /// Read a chunk header at the current position
    pub fn read_chunk_header(&mut self) -> W3DResult<W3dChunkHeader> {
        W3dChunkHeader::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read chunk header: {}", e)))
    }

    /// Read a chunk and its data
    pub fn read_chunk(&mut self) -> W3DResult<W3DChunk> {
        let header = self.read_chunk_header()?;
        let chunk_type = W3DChunkType::from_u32(header.chunk_type)
            .ok_or(W3DError::InvalidChunkType(header.chunk_type))?;
        let chunk_size = header.actual_size();

        let start_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?;

        let chunk = match chunk_type {
            W3DChunkType::Mesh => W3DChunk::Mesh(self.read_mesh(chunk_size)?),
            W3DChunkType::MeshHeader3 => W3DChunk::MeshHeader(self.read_mesh_header()?),
            W3DChunkType::Vertices => W3DChunk::Vertices(self.read_vertices(chunk_size)?),
            W3DChunkType::VertexNormals => W3DChunk::VertexNormals(self.read_normals(chunk_size)?),
            W3DChunkType::Triangles => W3DChunk::Triangles(self.read_triangles(chunk_size)?),
            W3DChunkType::StageTexcoords => W3DChunk::TexCoords(self.read_texcoords(chunk_size)?),
            W3DChunkType::VertexInfluences => {
                W3DChunk::VertexInfluences(self.read_vertex_influences(chunk_size)?)
            }
            W3DChunkType::MaterialInfo => W3DChunk::MaterialInfo(self.read_material_info()?),
            W3DChunkType::Shaders => W3DChunk::Shaders(self.read_shaders(chunk_size)?),
            W3DChunkType::VertexMaterials => {
                W3DChunk::VertexMaterials(self.read_vertex_materials(chunk_size)?)
            }
            W3DChunkType::Textures => W3DChunk::Textures(self.read_textures(chunk_size)?),
            W3DChunkType::MaterialPass => {
                W3DChunk::MaterialPass(self.read_material_pass(chunk_size)?)
            }
            W3DChunkType::Hierarchy => W3DChunk::Hierarchy(self.read_hierarchy(chunk_size)?),
            W3DChunkType::HierarchyHeader => {
                W3DChunk::HierarchyHeader(self.read_hierarchy_header()?)
            }
            W3DChunkType::Pivots => W3DChunk::Pivots(self.read_pivots(chunk_size)?),
            W3DChunkType::Animation => W3DChunk::Animation(self.read_animation(chunk_size)?),
            W3DChunkType::AnimationHeader => {
                W3DChunk::AnimationHeader(self.read_animation_header()?)
            }
            W3DChunkType::AnimationChannel => {
                W3DChunk::AnimationChannel(self.read_animation_channel()?)
            }
            W3DChunkType::CompressedAnimation => {
                W3DChunk::CompressedAnimation(self.read_compressed_animation(chunk_size)?)
            }
            W3DChunkType::CompressedAnimationHeader => {
                W3DChunk::CompressedAnimationHeader(self.read_compressed_animation_header()?)
            }
            W3DChunkType::TimeCodedAnimChannel => {
                W3DChunk::TimeCodedAnimChannel(self.read_timecoded_anim_channel(chunk_size)?)
            }
            W3DChunkType::AdaptiveDeltaAnimChannel => W3DChunk::AdaptiveDeltaAnimChannel(
                self.read_adaptive_delta_anim_channel(chunk_size)?,
            ),
            W3DChunkType::TimeCodedBitChannel => {
                W3DChunk::TimeCodedBitChannel(self.read_timecoded_bit_channel(chunk_size)?)
            }
            W3DChunkType::BitChannel => W3DChunk::BitChannel(self.read_bit_channel(chunk_size)?),
            W3DChunkType::MorphAnimation => {
                W3DChunk::MorphAnimation(self.read_morph_animation(chunk_size)?)
            }
            W3DChunkType::MorphanimHeader => {
                W3DChunk::MorphAnimHeader(self.read_morph_anim_header()?)
            }
            W3DChunkType::Hmodel => W3DChunk::HModel(self.read_hmodel(chunk_size)?),
            W3DChunkType::HmodelHeader => W3DChunk::HModelHeader(self.read_hmodel_header()?),
            W3DChunkType::Node => W3DChunk::HModelNode(self.read_hmodel_node()?),
            W3DChunkType::CollisionNode => W3DChunk::HModelNode(self.read_hmodel_node()?),
            W3DChunkType::SkinNode => W3DChunk::HModelNode(self.read_hmodel_node()?),
            W3DChunkType::Aabtree => W3DChunk::AABTree(self.read_aabtree(chunk_size)?),
            W3DChunkType::AabtreeHeader => W3DChunk::AABTreeHeader(self.read_aabtree_header()?),
            W3DChunkType::AabtreePolyindices => {
                W3DChunk::AABTreePolyindices(self.read_aabtree_polyindices(chunk_size)?)
            }
            W3DChunkType::AabtreeNodes => {
                W3DChunk::AABTreeNodes(self.read_aabtree_nodes(chunk_size)?)
            }
            W3DChunkType::Light => W3DChunk::Light(self.read_light(chunk_size)?),
            W3DChunkType::Emitter => W3DChunk::Emitter(self.read_emitter(chunk_size)?),
            W3DChunkType::EmitterHeader => W3DChunk::EmitterHeader(self.read_emitter_header()?),
            W3DChunkType::EmitterUserData => {
                W3DChunk::EmitterUserData(self.read_emitter_user_data(chunk_size)?)
            }
            W3DChunkType::EmitterInfo => W3DChunk::EmitterInfo(self.read_emitter_info()?),
            W3DChunkType::EmitterInfov2 => W3DChunk::EmitterInfov2(self.read_emitter_infov2()?),
            W3DChunkType::EmitterProps => W3DChunk::EmitterProps(self.read_emitter_props()?),
            W3DChunkType::EmitterLineProperties => {
                W3DChunk::EmitterLineProperties(self.read_emitter_line_properties()?)
            }
            W3DChunkType::EmitterRotationKeyframes => W3DChunk::EmitterRotationKeyframes(
                self.read_emitter_rotation_keyframes(chunk_size)?,
            ),
            W3DChunkType::EmitterFrameKeyframes => {
                W3DChunk::EmitterFrameKeyframes(self.read_emitter_frame_keyframes(chunk_size)?)
            }
            W3DChunkType::EmitterBlurTimeKeyframes => W3DChunk::EmitterBlurTimeKeyframes(
                self.read_emitter_blur_time_keyframes(chunk_size)?,
            ),
            W3DChunkType::EmitterExtraInfo => {
                W3DChunk::EmitterExtraInfo(self.read_emitter_extra_info()?)
            }
            W3DChunkType::Aggregate => W3DChunk::Aggregate(self.read_aggregate(chunk_size)?),
            W3DChunkType::Box => W3DChunk::Box(self.read_box()?),
            W3DChunkType::Sphere => W3DChunk::Sphere(self.read_sphere()?),
            W3DChunkType::Ring => W3DChunk::Ring(self.read_ring()?),
            W3DChunkType::NullObject => W3DChunk::NullObject(self.read_null_object()?),
            _ => {
                // Unknown or unimplemented chunk type - read as raw data
                let mut data = vec![0u8; chunk_size as usize];
                self.reader
                    .read_exact(&mut data)
                    .map_err(|e| W3DError::IoError(e.to_string()))?;
                W3DChunk::Unknown {
                    chunk_type: header.chunk_type,
                    data,
                }
            }
        };

        // Ensure we're at the right position after reading
        let end_pos = start_pos + chunk_size as u64;
        self.reader
            .seek(SeekFrom::Start(end_pos))
            .map_err(|e| W3DError::IoError(e.to_string()))?;

        Ok(chunk)
    }

    /// Read all chunks from the file
    pub fn read_all_chunks(&mut self) -> W3DResult<Vec<W3DChunk>> {
        let mut chunks = Vec::new();
        loop {
            match self.read_chunk() {
                Ok(chunk) => chunks.push(chunk),
                Err(W3DError::IoError(_)) => break, // End of file
                Err(e) => return Err(e),
            }
        }
        Ok(chunks)
    }

    // Mesh reading functions
    pub fn read_mesh(&mut self, size: u32) -> W3DResult<W3dMesh> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut mesh = W3dMesh::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            let chunk = self.read_chunk()?;
            match chunk {
                W3DChunk::MeshHeader(header) => mesh.header = header,
                W3DChunk::Vertices(verts) => mesh.vertices = verts,
                W3DChunk::VertexNormals(normals) => mesh.normals = normals,
                W3DChunk::Triangles(tris) => mesh.triangles = tris,
                W3DChunk::TexCoords(coords) => mesh.texture_coords = coords,
                W3DChunk::MaterialInfo(mat) => mesh.material_info = Some(mat),
                W3DChunk::Shaders(shaders) => mesh.shaders = shaders,
                W3DChunk::VertexMaterials(vmats) => mesh.materials = vmats,
                // CRITICAL: Parse vertex influences for skinned meshes (C++ parity)
                W3DChunk::VertexInfluences(infs) => mesh.vertex_influences = infs,
                // CRITICAL: Parse texture data for multi-texture materials (C++ parity)
                W3DChunk::Textures(texs) => mesh.textures = texs,
                // CRITICAL: Parse material pass for multi-pass rendering (C++ parity)
                W3DChunk::MaterialPass(pass) => mesh.material_pass = Some(pass),
                _ => {} // Ignore other unknown sub-chunks
            }
        }

        Ok(mesh)
    }

    fn read_mesh_header(&mut self) -> W3DResult<W3dMeshHeader3Struct> {
        W3dMeshHeader3Struct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read mesh header: {}", e)))
    }

    fn read_vertices(&mut self, size: u32) -> W3DResult<Vec<W3dVectorStruct>> {
        let count = size / std::mem::size_of::<W3dVectorStruct>() as u32;
        let mut vertices = Vec::with_capacity(count as usize);
        for _ in 0..count {
            vertices.push(
                W3dVectorStruct::read(&mut self.reader)
                    .map_err(|e| W3DError::IoError(e.to_string()))?,
            );
        }
        Ok(vertices)
    }

    fn read_normals(&mut self, size: u32) -> W3DResult<Vec<W3dVectorStruct>> {
        self.read_vertices(size)
    }

    fn read_triangles(&mut self, size: u32) -> W3DResult<Vec<W3dTriangleStruct>> {
        let count = size / std::mem::size_of::<W3dTriangleStruct>() as u32;
        let mut triangles = Vec::with_capacity(count as usize);
        for _ in 0..count {
            triangles.push(
                W3dTriangleStruct::read(&mut self.reader)
                    .map_err(|e| W3DError::IoError(e.to_string()))?,
            );
        }
        Ok(triangles)
    }

    fn read_texcoords(&mut self, size: u32) -> W3DResult<Vec<W3dTexCoordStruct>> {
        let count = size / std::mem::size_of::<W3dTexCoordStruct>() as u32;
        let mut coords = Vec::with_capacity(count as usize);
        for _ in 0..count {
            coords.push(
                W3dTexCoordStruct::read(&mut self.reader)
                    .map_err(|e| W3DError::IoError(e.to_string()))?,
            );
        }
        Ok(coords)
    }

    fn read_vertex_influences(&mut self, size: u32) -> W3DResult<Vec<W3dVertInfStruct>> {
        let count = size / std::mem::size_of::<W3dVertInfStruct>() as u32;
        let mut influences = Vec::with_capacity(count as usize);
        for _ in 0..count {
            influences.push(
                W3dVertInfStruct::read(&mut self.reader)
                    .map_err(|e| W3DError::IoError(e.to_string()))?,
            );
        }
        Ok(influences)
    }

    fn read_material_info(&mut self) -> W3DResult<W3dMaterialInfoStruct> {
        W3dMaterialInfoStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read material info: {}", e)))
    }

    fn read_shaders(&mut self, size: u32) -> W3DResult<Vec<W3dShaderStruct>> {
        let count = size / std::mem::size_of::<W3dShaderStruct>() as u32;
        let mut shaders = Vec::with_capacity(count as usize);
        for _ in 0..count {
            shaders.push(
                W3dShaderStruct::read(&mut self.reader)
                    .map_err(|e| W3DError::IoError(e.to_string()))?,
            );
        }
        Ok(shaders)
    }

    fn read_vertex_materials(&mut self, size: u32) -> W3DResult<Vec<W3dVertexMaterialStruct>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut materials = Vec::new();
        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            let header = self.read_chunk_header()?;
            let subchunk_size = header.actual_size();
            let subchunk_end = self
                .reader
                .stream_position()
                .map_err(|e| W3DError::IoError(e.to_string()))?
                + subchunk_size as u64;

            if header.chunk_type == W3DChunkType::VertexMaterial.as_u32() {
                let mut material = None;
                while self
                    .reader
                    .stream_position()
                    .map_err(|e| W3DError::IoError(e.to_string()))?
                    < subchunk_end
                {
                    let nested = self.read_chunk_header()?;
                    let nested_size = nested.actual_size();
                    match W3DChunkType::from_u32(nested.chunk_type) {
                        Some(W3DChunkType::VertexMaterialInfo) => {
                            material = Some(
                                W3dVertexMaterialStruct::read(&mut self.reader)
                                    .map_err(|e| W3DError::IoError(e.to_string()))?,
                            );
                        }
                        _ => {
                            self.reader
                                .seek(SeekFrom::Current(nested_size as i64))
                                .map_err(|e| W3DError::IoError(e.to_string()))?;
                        }
                    }
                }
                if let Some(material) = material {
                    materials.push(material);
                }
            } else {
                self.reader
                    .seek(SeekFrom::Current(subchunk_size as i64))
                    .map_err(|e| W3DError::IoError(e.to_string()))?;
            }

            self.reader
                .seek(SeekFrom::Start(subchunk_end))
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }

        Ok(materials)
    }

    fn read_textures(&mut self, size: u32) -> W3DResult<Vec<W3dTextureStruct>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut textures = Vec::new();
        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            let header = self.read_chunk_header()?;
            let subchunk_size = header.actual_size();
            let subchunk_end = self
                .reader
                .stream_position()
                .map_err(|e| W3DError::IoError(e.to_string()))?
                + subchunk_size as u64;

            if header.chunk_type == W3DChunkType::Texture.as_u32() {
                let mut texture = W3dTextureStruct {
                    name: [0u8; 256],
                    texture_info: W3dTextureInfoStruct {
                        attributes: 0,
                        animation_type: 0,
                        frame_count: 0,
                        frame_rate: 0.0,
                    },
                };
                while self
                    .reader
                    .stream_position()
                    .map_err(|e| W3DError::IoError(e.to_string()))?
                    < subchunk_end
                {
                    let nested = self.read_chunk_header()?;
                    let nested_size = nested.actual_size();
                    match W3DChunkType::from_u32(nested.chunk_type) {
                        Some(W3DChunkType::TextureName) => {
                            let mut name = vec![0u8; nested_size as usize];
                            self.reader
                                .read_exact(&mut name)
                                .map_err(|e| W3DError::IoError(e.to_string()))?;
                            let len = name.iter().position(|&b| b == 0).unwrap_or(name.len());
                            let copy_len = len.min(texture.name.len());
                            texture.name[..copy_len].copy_from_slice(&name[..copy_len]);
                        }
                        Some(W3DChunkType::TextureInfo) => {
                            texture.texture_info = W3dTextureInfoStruct::read(&mut self.reader)
                                .map_err(|e| W3DError::IoError(e.to_string()))?;
                        }
                        _ => {
                            self.reader
                                .seek(SeekFrom::Current(nested_size as i64))
                                .map_err(|e| W3DError::IoError(e.to_string()))?;
                        }
                    }
                }
                textures.push(texture);
            } else {
                self.reader
                    .seek(SeekFrom::Current(subchunk_size as i64))
                    .map_err(|e| W3DError::IoError(e.to_string()))?;
            }

            self.reader
                .seek(SeekFrom::Start(subchunk_end))
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }

        Ok(textures)
    }

    fn read_material_pass(&mut self, size: u32) -> W3DResult<W3dMaterialPassStruct> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut pass = W3dMaterialPassStruct {
            vm_id: 0,
            shader_id: 0,
            dcg: [0; 3],
            dig: [0; 3],
            scg: [0; 3],
            texture_count: 0,
        };

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            let header = self.read_chunk_header()?;
            let subchunk_size = header.actual_size();
            match W3DChunkType::from_u32(header.chunk_type) {
                Some(W3DChunkType::VertexMaterialIds) => {
                    let count = self.read_u32_le()?;
                    if count > 0 {
                        pass.vm_id = self.read_u32_le()?;
                    }
                }
                Some(W3DChunkType::ShaderIds) => {
                    let count = self.read_u32_le()?;
                    if count > 0 {
                        pass.shader_id = self.read_u32_le()?;
                    }
                }
                Some(W3DChunkType::Dcg) => {
                    for value in &mut pass.dcg {
                        *value = self.read_u32_le()?;
                    }
                }
                Some(W3DChunkType::Dig) => {
                    for value in &mut pass.dig {
                        *value = self.read_u32_le()?;
                    }
                }
                Some(W3DChunkType::Scg) => {
                    for value in &mut pass.scg {
                        *value = self.read_u32_le()?;
                    }
                }
                Some(W3DChunkType::TextureStage) => {
                    pass.texture_count += 1;
                    self.reader
                        .seek(SeekFrom::Current(subchunk_size as i64))
                        .map_err(|e| W3DError::IoError(e.to_string()))?;
                }
                _ => {
                    self.reader
                        .seek(SeekFrom::Current(subchunk_size as i64))
                        .map_err(|e| W3DError::IoError(e.to_string()))?;
                }
            }
        }

        Ok(pass)
    }

    fn read_u32_le(&mut self) -> W3DResult<u32> {
        let mut bytes = [0u8; 4];
        self.reader
            .read_exact(&mut bytes)
            .map_err(|e| W3DError::IoError(e.to_string()))?;
        Ok(u32::from_le_bytes(bytes))
    }

    // Hierarchy reading functions
    pub fn read_hierarchy(&mut self, size: u32) -> W3DResult<W3dHierarchy> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut hierarchy = W3dHierarchy::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            let chunk = self.read_chunk()?;
            match chunk {
                W3DChunk::HierarchyHeader(header) => hierarchy.header = header,
                W3DChunk::Pivots(pivots) => hierarchy.pivots = pivots,
                _ => {}
            }
        }

        Ok(hierarchy)
    }

    fn read_hierarchy_header(&mut self) -> W3DResult<W3dHierarchyStruct> {
        W3dHierarchyStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read hierarchy header: {}", e)))
    }

    fn read_pivots(&mut self, size: u32) -> W3DResult<Vec<W3dPivotStruct>> {
        let count = size / std::mem::size_of::<W3dPivotStruct>() as u32;
        let mut pivots = Vec::with_capacity(count as usize);
        for _ in 0..count {
            pivots.push(
                W3dPivotStruct::read(&mut self.reader)
                    .map_err(|e| W3DError::IoError(e.to_string()))?,
            );
        }
        Ok(pivots)
    }

    // Animation reading functions
    pub fn read_animation(&mut self, size: u32) -> W3DResult<W3dAnimation> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut animation = W3dAnimation::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            let chunk = self.read_chunk()?;
            match chunk {
                W3DChunk::AnimationHeader(header) => animation.header = header,
                W3DChunk::AnimationChannel(channel) => animation.channels.push(channel),
                _ => {}
            }
        }

        Ok(animation)
    }

    fn read_animation_header(&mut self) -> W3DResult<W3dAnimationStruct> {
        W3dAnimationStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read animation header: {}", e)))
    }

    fn read_animation_channel(&mut self) -> W3DResult<W3dAnimChannelStruct> {
        W3dAnimChannelStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read animation channel: {}", e)))
    }

    pub fn read_compressed_animation(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    fn read_compressed_animation_header(&mut self) -> W3DResult<W3dCompressedAnimHeaderStruct> {
        W3dCompressedAnimHeaderStruct::read(&mut self.reader).map_err(|e| {
            W3DError::IoError(format!("Failed to read compressed animation header: {}", e))
        })
    }

    fn read_timecoded_anim_channel(
        &mut self,
        size: u32,
    ) -> W3DResult<(W3dTimeCodedAnimChannelStruct, Vec<u32>)> {
        // Read the header part
        let num_time_codes: u32 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read num_time_codes: {}", e)))?;
        let pivot: u16 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read pivot: {}", e)))?;
        let vector_len: u8 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read vector_len: {}", e)))?;
        let flags: u8 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read flags: {}", e)))?;

        // Read the packed data (time codes + values)
        let header_size = 8; // num_time_codes(4) + pivot(2) + vector_len(1) + flags(1)
        let data_size = (size - header_size) / 4; // Convert bytes to u32 count
        let mut data = Vec::with_capacity(data_size as usize);
        for _ in 0..data_size {
            let val: u32 = binrw::BinRead::read_le(&mut self.reader)
                .map_err(|e| W3DError::IoError(format!("Failed to read channel data: {}", e)))?;
            data.push(val);
        }

        let header = W3dTimeCodedAnimChannelStruct {
            num_time_codes,
            pivot,
            vector_len,
            flags,
            data: [0], // Placeholder - actual data is in the Vec
        };

        Ok((header, data))
    }

    fn read_adaptive_delta_anim_channel(
        &mut self,
        size: u32,
    ) -> W3DResult<(W3dAdaptiveDeltaAnimChannelStruct, Vec<u32>)> {
        // Read the header
        let header = W3dAdaptiveDeltaAnimChannelStruct::read(&mut self.reader).map_err(|e| {
            W3DError::IoError(format!("Failed to read adaptive delta header: {}", e))
        })?;

        // Read the compressed data
        let header_size = std::mem::size_of::<W3dAdaptiveDeltaAnimChannelStruct>() as u32;
        let data_size = (size - header_size) / 4; // Convert bytes to u32 count
        let mut data = Vec::with_capacity(data_size as usize);
        for _ in 0..data_size {
            let val: u32 = binrw::BinRead::read_le(&mut self.reader)
                .map_err(|e| W3DError::IoError(format!("Failed to read channel data: {}", e)))?;
            data.push(val);
        }

        Ok((header, data))
    }

    fn read_timecoded_bit_channel(
        &mut self,
        size: u32,
    ) -> W3DResult<(W3dTimeCodedBitChannelStruct, Vec<u32>)> {
        // Read the header
        let num_time_codes: u32 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read num_time_codes: {}", e)))?;
        let pivot: u16 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read pivot: {}", e)))?;
        let flags: u8 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read flags: {}", e)))?;
        let default_val: u8 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read default_val: {}", e)))?;

        // Read packed bit data
        let header_size = 8; // num_time_codes(4) + pivot(2) + flags(1) + default_val(1)
        let data_size = (size - header_size) / 4; // Convert bytes to u32 count
        let mut data = Vec::with_capacity(data_size as usize);
        for _ in 0..data_size {
            let val: u32 = binrw::BinRead::read_le(&mut self.reader)
                .map_err(|e| W3DError::IoError(format!("Failed to read bit data: {}", e)))?;
            data.push(val);
        }

        let header = W3dTimeCodedBitChannelStruct {
            num_time_codes,
            pivot,
            flags,
            default_val,
            data: [0], // Placeholder
        };

        Ok((header, data))
    }

    fn read_bit_channel(&mut self, size: u32) -> W3DResult<(W3dBitChannelStruct, Vec<u8>)> {
        // Read header
        let header = W3dBitChannelStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read bit channel header: {}", e)))?;

        // Read bit data
        let header_size = std::mem::size_of::<W3dBitChannelStruct>() as u32;
        let data_size = size - header_size;
        let mut data = vec![0u8; data_size as usize];
        self.reader
            .read_exact(&mut data)
            .map_err(|e| W3DError::IoError(format!("Failed to read bit channel data: {}", e)))?;

        Ok((header, data))
    }

    fn read_morph_animation(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    fn read_morph_anim_header(&mut self) -> W3DResult<W3dMorphAnimHeaderStruct> {
        W3dMorphAnimHeaderStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read morph animation header: {}", e)))
    }

    // HModel reading functions
    pub fn read_hmodel(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    fn read_hmodel_header(&mut self) -> W3DResult<W3dHModelHeaderStruct> {
        W3dHModelHeaderStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read hmodel header: {}", e)))
    }

    fn read_hmodel_node(&mut self) -> W3DResult<W3dHModelNodeStruct> {
        W3dHModelNodeStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read hmodel node: {}", e)))
    }

    // AABTree reading functions
    fn read_aabtree(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    fn read_aabtree_header(&mut self) -> W3DResult<W3dAABTreeHeader> {
        let header = W3dAABTreeHeader::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read aabtree header: {}", e)))?;

        // Validate header consistency with C++ format
        header
            .validate()
            .map_err(|e| W3DError::IoError(format!("Invalid AABTree header: {}", e)))?;

        Ok(header)
    }

    fn read_aabtree_polyindices(&mut self, size: u32) -> W3DResult<Vec<u32>> {
        let count = (size / 4) as usize;
        let mut indices = Vec::with_capacity(count);
        for _ in 0..count {
            let index = binrw::BinRead::read_le(&mut self.reader)
                .map_err(|e| W3DError::IoError(format!("Failed to read poly index: {}", e)))?;
            indices.push(index);
        }
        Ok(indices)
    }

    fn read_aabtree_nodes(&mut self, size: u32) -> W3DResult<Vec<W3dAABTreeNode>> {
        let node_size = std::mem::size_of::<W3dAABTreeNode>() as u32;
        let count = (size / node_size) as usize;
        let mut nodes = Vec::with_capacity(count);
        for _ in 0..count {
            let node = W3dAABTreeNode::read(&mut self.reader)
                .map_err(|e| W3DError::IoError(format!("Failed to read aabtree node: {}", e)))?;
            nodes.push(node);
        }
        Ok(nodes)
    }

    // Light reading functions
    fn read_light(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    // Emitter reading functions
    fn read_emitter(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    fn read_emitter_header(&mut self) -> W3DResult<W3dEmitterHeaderStruct> {
        W3dEmitterHeaderStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read emitter header: {}", e)))
    }

    fn read_emitter_user_data(&mut self, _size: u32) -> W3DResult<W3dEmitterUserDataStruct> {
        let type_id: u32 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read user data type: {}", e)))?;
        let data_size: u32 = binrw::BinRead::read_le(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read user data size: {}", e)))?;
        let mut data = vec![0u8; data_size as usize];
        self.reader
            .read_exact(&mut data)
            .map_err(|e| W3DError::IoError(format!("Failed to read user data: {}", e)))?;
        Ok(W3dEmitterUserDataStruct {
            type_id,
            size: data_size,
            data,
        })
    }

    fn read_emitter_info(&mut self) -> W3DResult<W3dEmitterInfoStruct> {
        W3dEmitterInfoStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read emitter info: {}", e)))
    }

    fn read_emitter_infov2(&mut self) -> W3DResult<W3dEmitterInfoStructV2> {
        W3dEmitterInfoStructV2::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read emitter info v2: {}", e)))
    }

    fn read_emitter_props(&mut self) -> W3DResult<W3dEmitterPropertyStruct> {
        W3dEmitterPropertyStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read emitter props: {}", e)))
    }

    fn read_emitter_line_properties(&mut self) -> W3DResult<W3dEmitterLinePropertiesStruct> {
        W3dEmitterLinePropertiesStruct::read(&mut self.reader).map_err(|e| {
            W3DError::IoError(format!("Failed to read emitter line properties: {}", e))
        })
    }

    fn read_emitter_rotation_keyframes(
        &mut self,
        size: u32,
    ) -> W3DResult<Vec<W3dEmitterRotationKeyframeStruct>> {
        let keyframe_size = std::mem::size_of::<W3dEmitterRotationKeyframeStruct>() as u32;
        let count = (size / keyframe_size) as usize;
        let mut keyframes = Vec::with_capacity(count);
        for _ in 0..count {
            let kf = W3dEmitterRotationKeyframeStruct::read(&mut self.reader).map_err(|e| {
                W3DError::IoError(format!("Failed to read rotation keyframe: {}", e))
            })?;
            keyframes.push(kf);
        }
        Ok(keyframes)
    }

    fn read_emitter_frame_keyframes(
        &mut self,
        size: u32,
    ) -> W3DResult<Vec<W3dEmitterFrameKeyframeStruct>> {
        let keyframe_size = std::mem::size_of::<W3dEmitterFrameKeyframeStruct>() as u32;
        let count = (size / keyframe_size) as usize;
        let mut keyframes = Vec::with_capacity(count);
        for _ in 0..count {
            let kf = W3dEmitterFrameKeyframeStruct::read(&mut self.reader)
                .map_err(|e| W3DError::IoError(format!("Failed to read frame keyframe: {}", e)))?;
            keyframes.push(kf);
        }
        Ok(keyframes)
    }

    fn read_emitter_blur_time_keyframes(
        &mut self,
        size: u32,
    ) -> W3DResult<Vec<W3dEmitterBlurTimeKeyframeStruct>> {
        let keyframe_size = std::mem::size_of::<W3dEmitterBlurTimeKeyframeStruct>() as u32;
        let count = (size / keyframe_size) as usize;
        let mut keyframes = Vec::with_capacity(count);
        for _ in 0..count {
            let kf = W3dEmitterBlurTimeKeyframeStruct::read(&mut self.reader).map_err(|e| {
                W3DError::IoError(format!("Failed to read blur time keyframe: {}", e))
            })?;
            keyframes.push(kf);
        }
        Ok(keyframes)
    }

    fn read_emitter_extra_info(&mut self) -> W3DResult<W3dEmitterExtraInfoStruct> {
        W3dEmitterExtraInfoStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read emitter extra info: {}", e)))
    }

    // Aggregate reading functions
    fn read_aggregate(&mut self, size: u32) -> W3DResult<Vec<W3DChunk>> {
        let end_pos = self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            + size as u64;

        let mut chunks = Vec::new();

        while self
            .reader
            .stream_position()
            .map_err(|e| W3DError::IoError(e.to_string()))?
            < end_pos
        {
            chunks.push(self.read_chunk()?);
        }

        Ok(chunks)
    }

    // Primitive reading functions
    fn read_box(&mut self) -> W3DResult<W3dBoxStruct> {
        W3dBoxStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read box: {}", e)))
    }

    fn read_sphere(&mut self) -> W3DResult<W3dSphereStruct> {
        W3dSphereStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read sphere: {}", e)))
    }

    fn read_ring(&mut self) -> W3DResult<W3dRingStruct> {
        W3dRingStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read ring: {}", e)))
    }

    fn read_null_object(&mut self) -> W3DResult<W3dNullObjectStruct> {
        W3dNullObjectStruct::read(&mut self.reader)
            .map_err(|e| W3DError::IoError(format!("Failed to read null object: {}", e)))
    }
}

/// W3D file writer
///
/// Provides methods to write W3D files with proper chunk structure.
pub struct W3DWriter<W: Write + Seek> {
    writer: W,
}

impl<W: Write + Seek> W3DWriter<W> {
    /// Create a new W3D writer
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Write a chunk header
    pub fn write_chunk_header(&mut self, chunk_type: W3DChunkType, size: u32) -> W3DResult<()> {
        let header = W3dChunkHeader {
            chunk_type: chunk_type as u32,
            chunk_size: size,
        };
        header
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write chunk header: {}", e)))
    }

    /// Write a complete chunk
    pub fn write_chunk(&mut self, chunk: &W3DChunk) -> W3DResult<()> {
        match chunk {
            W3DChunk::Mesh(mesh) => self.write_mesh(mesh),
            W3DChunk::MeshHeader(header) => self.write_mesh_header(header),
            W3DChunk::Vertices(verts) => self.write_vertices(verts),
            W3DChunk::VertexNormals(normals) => self.write_normals(normals),
            W3DChunk::Triangles(tris) => self.write_triangles(tris),
            W3DChunk::TexCoords(coords) => self.write_texcoords(coords),
            W3DChunk::VertexInfluences(infs) => self.write_vertex_influences(infs),
            W3DChunk::MaterialInfo(mat) => self.write_material_info(mat),
            W3DChunk::Shaders(shaders) => self.write_shaders(shaders),
            W3DChunk::VertexMaterials(vmats) => self.write_vertex_materials(vmats),
            W3DChunk::Textures(texs) => self.write_textures(texs),
            W3DChunk::MaterialPass(pass) => self.write_material_pass(pass),
            W3DChunk::Hierarchy(hier) => self.write_hierarchy(hier),
            W3DChunk::HierarchyHeader(header) => self.write_hierarchy_header(header),
            W3DChunk::Pivots(pivots) => self.write_pivots(pivots),
            W3DChunk::Animation(anim) => self.write_animation(anim),
            W3DChunk::AnimationHeader(header) => self.write_animation_header(header),
            W3DChunk::AnimationChannel(channel) => self.write_animation_channel(channel),
            W3DChunk::CompressedAnimation(chunks) => self.write_compressed_animation(chunks),
            W3DChunk::CompressedAnimationHeader(header) => {
                self.write_compressed_animation_header(header)
            }
            W3DChunk::TimeCodedAnimChannel(channel) => self.write_timecoded_anim_channel(channel),
            W3DChunk::AdaptiveDeltaAnimChannel(channel) => {
                self.write_adaptive_delta_anim_channel(channel)
            }
            W3DChunk::TimeCodedBitChannel(channel) => self.write_timecoded_bit_channel(channel),
            W3DChunk::BitChannel(channel) => self.write_bit_channel(channel),
            W3DChunk::MorphAnimation(chunks) => self.write_morph_animation(chunks),
            W3DChunk::MorphAnimHeader(header) => self.write_morph_anim_header(header),
            W3DChunk::Box(b) => self.write_box(b),
            W3DChunk::Sphere(s) => self.write_sphere(s),
            W3DChunk::Ring(r) => self.write_ring(r),
            W3DChunk::NullObject(n) => self.write_null_object(n),
            W3DChunk::Unknown { chunk_type, data } => self.write_unknown(*chunk_type, data),
            _ => Err(W3DError::UnsupportedType(
                "Chunk type not yet implemented for writing".to_string(),
            )),
        }
    }

    fn write_mesh(&mut self, mesh: &W3dMesh) -> W3DResult<()> {
        // Calculate total size of mesh chunk
        let size = self.calculate_mesh_size(mesh);
        self.write_chunk_header(W3DChunkType::Mesh, size)?;

        self.write_mesh_header(&mesh.header)?;

        if !mesh.vertices.is_empty() {
            self.write_vertices(&mesh.vertices)?;
        }
        if !mesh.normals.is_empty() {
            self.write_normals(&mesh.normals)?;
        }
        if !mesh.triangles.is_empty() {
            self.write_triangles(&mesh.triangles)?;
        }
        if !mesh.texture_coords.is_empty() {
            self.write_texcoords(&mesh.texture_coords)?;
        }

        Ok(())
    }

    fn write_mesh_header(&mut self, header: &W3dMeshHeader3Struct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dMeshHeader3Struct>() as u32;
        self.write_chunk_header(W3DChunkType::MeshHeader3, size)?;
        header
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write mesh header: {}", e)))
    }

    fn write_vertices(&mut self, vertices: &[W3dVectorStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(vertices) as u32;
        self.write_chunk_header(W3DChunkType::Vertices, size)?;
        for vertex in vertices {
            vertex
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_normals(&mut self, normals: &[W3dVectorStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(normals) as u32;
        self.write_chunk_header(W3DChunkType::VertexNormals, size)?;
        for normal in normals {
            normal
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_triangles(&mut self, triangles: &[W3dTriangleStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(triangles) as u32;
        self.write_chunk_header(W3DChunkType::Triangles, size)?;
        for triangle in triangles {
            triangle
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_texcoords(&mut self, coords: &[W3dTexCoordStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(coords) as u32;
        self.write_chunk_header(W3DChunkType::StageTexcoords, size)?;
        for coord in coords {
            coord
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_vertex_influences(&mut self, influences: &[W3dVertInfStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(influences) as u32;
        self.write_chunk_header(W3DChunkType::VertexInfluences, size)?;
        for influence in influences {
            influence
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_material_info(&mut self, material: &W3dMaterialInfoStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dMaterialInfoStruct>() as u32;
        self.write_chunk_header(W3DChunkType::MaterialInfo, size)?;
        material
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write material info: {}", e)))
    }

    fn write_shaders(&mut self, shaders: &[W3dShaderStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(shaders) as u32;
        self.write_chunk_header(W3DChunkType::Shaders, size)?;
        for shader in shaders {
            shader
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_vertex_materials(&mut self, materials: &[W3dVertexMaterialStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(materials) as u32;
        self.write_chunk_header(W3DChunkType::VertexMaterials, size)?;
        for material in materials {
            material
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_textures(&mut self, textures: &[W3dTextureStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(textures) as u32;
        self.write_chunk_header(W3DChunkType::Textures, size)?;
        for texture in textures {
            texture
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_material_pass(&mut self, pass: &W3dMaterialPassStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dMaterialPassStruct>() as u32;
        self.write_chunk_header(W3DChunkType::MaterialPass, size)?;
        pass.write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write material pass: {}", e)))
    }

    fn write_hierarchy(&mut self, hierarchy: &W3dHierarchy) -> W3DResult<()> {
        let size = self.calculate_hierarchy_size(hierarchy);
        self.write_chunk_header(W3DChunkType::Hierarchy, size)?;

        self.write_hierarchy_header(&hierarchy.header)?;

        if !hierarchy.pivots.is_empty() {
            self.write_pivots(&hierarchy.pivots)?;
        }

        Ok(())
    }

    fn write_hierarchy_header(&mut self, header: &W3dHierarchyStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dHierarchyStruct>() as u32;
        self.write_chunk_header(W3DChunkType::HierarchyHeader, size)?;
        header
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write hierarchy header: {}", e)))
    }

    fn write_pivots(&mut self, pivots: &[W3dPivotStruct]) -> W3DResult<()> {
        let size = std::mem::size_of_val(pivots) as u32;
        self.write_chunk_header(W3DChunkType::Pivots, size)?;
        for pivot in pivots {
            pivot
                .write(&mut self.writer)
                .map_err(|e| W3DError::IoError(e.to_string()))?;
        }
        Ok(())
    }

    fn write_animation(&mut self, animation: &W3dAnimation) -> W3DResult<()> {
        let size = self.calculate_animation_size(animation);
        self.write_chunk_header(W3DChunkType::Animation, size)?;

        self.write_animation_header(&animation.header)?;

        for channel in &animation.channels {
            self.write_animation_channel(channel)?;
        }

        Ok(())
    }

    fn write_animation_header(&mut self, header: &W3dAnimationStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dAnimationStruct>() as u32;
        self.write_chunk_header(W3DChunkType::AnimationHeader, size)?;
        header
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write animation header: {}", e)))
    }

    fn write_animation_channel(&mut self, channel: &W3dAnimChannelStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dAnimChannelStruct>() as u32;
        self.write_chunk_header(W3DChunkType::AnimationChannel, size)?;
        channel
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write animation channel: {}", e)))
    }

    fn write_compressed_animation_header(
        &mut self,
        header: &W3dCompressedAnimHeaderStruct,
    ) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dCompressedAnimHeaderStruct>() as u32;
        self.write_chunk_header(W3DChunkType::CompressedAnimationHeader, size)?;
        header.write(&mut self.writer).map_err(|e| {
            W3DError::IoError(format!(
                "Failed to write compressed animation header: {}",
                e
            ))
        })
    }

    fn write_compressed_animation(&mut self, chunks: &[W3DChunk]) -> W3DResult<()> {
        self.write_chunk_list(W3DChunkType::CompressedAnimation, chunks)
    }

    fn write_timecoded_anim_channel(
        &mut self,
        channel: &(W3dTimeCodedAnimChannelStruct, Vec<u32>),
    ) -> W3DResult<()> {
        let (header, data) = channel;
        let size = 8 + (data.len() * 4) as u32;
        self.write_chunk_header(W3DChunkType::TimeCodedAnimChannel, size)?;

        binrw::BinWrite::write_le(&header.num_time_codes, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write num_time_codes: {}", e)))?;
        binrw::BinWrite::write_le(&header.pivot, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write pivot: {}", e)))?;
        binrw::BinWrite::write_le(&header.vector_len, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write vector_len: {}", e)))?;
        binrw::BinWrite::write_le(&header.flags, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write flags: {}", e)))?;

        for value in data {
            binrw::BinWrite::write_le(value, &mut self.writer)
                .map_err(|e| W3DError::IoError(format!("Failed to write channel data: {}", e)))?;
        }

        Ok(())
    }

    fn write_adaptive_delta_anim_channel(
        &mut self,
        channel: &(W3dAdaptiveDeltaAnimChannelStruct, Vec<u32>),
    ) -> W3DResult<()> {
        let (header, data) = channel;
        let size = std::mem::size_of::<W3dAdaptiveDeltaAnimChannelStruct>() as u32
            + (data.len() * 4) as u32;
        self.write_chunk_header(W3DChunkType::AdaptiveDeltaAnimChannel, size)?;
        header.write(&mut self.writer).map_err(|e| {
            W3DError::IoError(format!("Failed to write adaptive delta header: {}", e))
        })?;

        for value in data {
            binrw::BinWrite::write_le(value, &mut self.writer)
                .map_err(|e| W3DError::IoError(format!("Failed to write channel data: {}", e)))?;
        }

        Ok(())
    }

    fn write_timecoded_bit_channel(
        &mut self,
        channel: &(W3dTimeCodedBitChannelStruct, Vec<u32>),
    ) -> W3DResult<()> {
        let (header, data) = channel;
        let size = 8 + (data.len() * 4) as u32;
        self.write_chunk_header(W3DChunkType::TimeCodedBitChannel, size)?;

        binrw::BinWrite::write_le(&header.num_time_codes, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write num_time_codes: {}", e)))?;
        binrw::BinWrite::write_le(&header.pivot, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write pivot: {}", e)))?;
        binrw::BinWrite::write_le(&header.flags, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write flags: {}", e)))?;
        binrw::BinWrite::write_le(&header.default_val, &mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write default_val: {}", e)))?;

        for value in data {
            binrw::BinWrite::write_le(value, &mut self.writer)
                .map_err(|e| W3DError::IoError(format!("Failed to write bit data: {}", e)))?;
        }

        Ok(())
    }

    fn write_bit_channel(&mut self, channel: &(W3dBitChannelStruct, Vec<u8>)) -> W3DResult<()> {
        let (header, data) = channel;
        let size = std::mem::size_of::<W3dBitChannelStruct>() as u32 + data.len() as u32;
        self.write_chunk_header(W3DChunkType::BitChannel, size)?;
        header
            .write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write bit channel header: {}", e)))?;

        self.writer
            .write_all(data)
            .map_err(|e| W3DError::IoError(format!("Failed to write bit channel data: {}", e)))
    }

    fn write_morph_anim_header(&mut self, header: &W3dMorphAnimHeaderStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dMorphAnimHeaderStruct>() as u32;
        self.write_chunk_header(W3DChunkType::MorphanimHeader, size)?;
        header.write(&mut self.writer).map_err(|e| {
            W3DError::IoError(format!("Failed to write morph animation header: {}", e))
        })
    }

    fn write_morph_animation(&mut self, chunks: &[W3DChunk]) -> W3DResult<()> {
        self.write_chunk_list(W3DChunkType::MorphAnimation, chunks)
    }

    fn write_chunk_list(&mut self, chunk_type: W3DChunkType, chunks: &[W3DChunk]) -> W3DResult<()> {
        let mut buffer = Vec::new();
        {
            let cursor = Cursor::new(&mut buffer);
            let mut sub_writer = W3DWriter::new(cursor);
            for chunk in chunks {
                sub_writer.write_chunk(chunk)?;
            }
        }

        self.write_chunk_header(chunk_type, buffer.len() as u32)?;
        self.writer
            .write_all(&buffer)
            .map_err(|e| W3DError::IoError(format!("Failed to write chunk payload: {}", e)))
    }

    fn write_box(&mut self, b: &W3dBoxStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dBoxStruct>() as u32;
        self.write_chunk_header(W3DChunkType::Box, size)?;
        b.write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write box: {}", e)))
    }

    fn write_sphere(&mut self, s: &W3dSphereStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dSphereStruct>() as u32;
        self.write_chunk_header(W3DChunkType::Sphere, size)?;
        s.write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write sphere: {}", e)))
    }

    fn write_ring(&mut self, r: &W3dRingStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dRingStruct>() as u32;
        self.write_chunk_header(W3DChunkType::Ring, size)?;
        r.write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write ring: {}", e)))
    }

    fn write_null_object(&mut self, n: &W3dNullObjectStruct) -> W3DResult<()> {
        let size = std::mem::size_of::<W3dNullObjectStruct>() as u32;
        self.write_chunk_header(W3DChunkType::NullObject, size)?;
        n.write(&mut self.writer)
            .map_err(|e| W3DError::IoError(format!("Failed to write null object: {}", e)))
    }

    fn write_unknown(&mut self, chunk_type: u32, data: &[u8]) -> W3DResult<()> {
        let header = W3dChunkHeader {
            chunk_type,
            chunk_size: data.len() as u32,
        };
        header.write(&mut self.writer).map_err(|e| {
            W3DError::IoError(format!("Failed to write unknown chunk header: {}", e))
        })?;
        self.writer
            .write_all(data)
            .map_err(|e| W3DError::IoError(format!("Failed to write unknown chunk data: {}", e)))
    }

    // Helper functions to calculate chunk sizes
    fn calculate_mesh_size(&self, mesh: &W3dMesh) -> u32 {
        let mut size = 0u32;

        // Header is always present
        size += 8 + std::mem::size_of::<W3dMeshHeader3Struct>() as u32;

        if !mesh.vertices.is_empty() {
            size += 8 + (mesh.vertices.len() * std::mem::size_of::<W3dVectorStruct>()) as u32;
        }
        if !mesh.normals.is_empty() {
            size += 8 + (mesh.normals.len() * std::mem::size_of::<W3dVectorStruct>()) as u32;
        }
        if !mesh.triangles.is_empty() {
            size += 8 + (mesh.triangles.len() * std::mem::size_of::<W3dTriangleStruct>()) as u32;
        }
        if !mesh.texture_coords.is_empty() {
            size +=
                8 + (mesh.texture_coords.len() * std::mem::size_of::<W3dTexCoordStruct>()) as u32;
        }

        size
    }

    fn calculate_hierarchy_size(&self, hierarchy: &W3dHierarchy) -> u32 {
        let mut size = 0u32;

        // Header is always present
        size += 8 + std::mem::size_of::<W3dHierarchyStruct>() as u32;

        if !hierarchy.pivots.is_empty() {
            size += 8 + (hierarchy.pivots.len() * std::mem::size_of::<W3dPivotStruct>()) as u32;
        }

        size
    }

    fn calculate_animation_size(&self, animation: &W3dAnimation) -> u32 {
        let mut size = 0u32;

        // Header is always present
        size += 8 + std::mem::size_of::<W3dAnimationStruct>() as u32;

        size +=
            (animation.channels.len() * (8 + std::mem::size_of::<W3dAnimChannelStruct>())) as u32;

        size
    }
}

/// Represents a parsed W3D chunk
#[derive(Debug, Clone)]
pub enum W3DChunk {
    Mesh(W3dMesh),
    MeshHeader(W3dMeshHeader3Struct),
    Vertices(Vec<W3dVectorStruct>),
    VertexNormals(Vec<W3dVectorStruct>),
    Triangles(Vec<W3dTriangleStruct>),
    TexCoords(Vec<W3dTexCoordStruct>),
    VertexInfluences(Vec<W3dVertInfStruct>),
    MaterialInfo(W3dMaterialInfoStruct),
    Shaders(Vec<W3dShaderStruct>),
    VertexMaterials(Vec<W3dVertexMaterialStruct>),
    Textures(Vec<W3dTextureStruct>),
    MaterialPass(W3dMaterialPassStruct),
    Hierarchy(W3dHierarchy),
    HierarchyHeader(W3dHierarchyStruct),
    Pivots(Vec<W3dPivotStruct>),
    Animation(W3dAnimation),
    AnimationHeader(W3dAnimationStruct),
    AnimationChannel(W3dAnimChannelStruct),
    CompressedAnimation(Vec<W3DChunk>),
    CompressedAnimationHeader(W3dCompressedAnimHeaderStruct),
    TimeCodedAnimChannel((W3dTimeCodedAnimChannelStruct, Vec<u32>)),
    AdaptiveDeltaAnimChannel((W3dAdaptiveDeltaAnimChannelStruct, Vec<u32>)),
    TimeCodedBitChannel((W3dTimeCodedBitChannelStruct, Vec<u32>)),
    BitChannel((W3dBitChannelStruct, Vec<u8>)),
    MorphAnimation(Vec<W3DChunk>),
    MorphAnimHeader(W3dMorphAnimHeaderStruct),
    HModel(Vec<W3DChunk>),
    HModelHeader(W3dHModelHeaderStruct),
    HModelNode(W3dHModelNodeStruct),
    AABTree(Vec<W3DChunk>),
    AABTreeHeader(W3dAABTreeHeader),
    AABTreePolyindices(Vec<u32>),
    AABTreeNodes(Vec<W3dAABTreeNode>),
    Light(Vec<W3DChunk>),
    Emitter(Vec<W3DChunk>),
    EmitterHeader(W3dEmitterHeaderStruct),
    EmitterUserData(W3dEmitterUserDataStruct),
    EmitterInfo(W3dEmitterInfoStruct),
    EmitterInfov2(W3dEmitterInfoStructV2),
    EmitterProps(W3dEmitterPropertyStruct),
    EmitterLineProperties(W3dEmitterLinePropertiesStruct),
    EmitterRotationKeyframes(Vec<W3dEmitterRotationKeyframeStruct>),
    EmitterFrameKeyframes(Vec<W3dEmitterFrameKeyframeStruct>),
    EmitterBlurTimeKeyframes(Vec<W3dEmitterBlurTimeKeyframeStruct>),
    EmitterExtraInfo(W3dEmitterExtraInfoStruct),
    Aggregate(Vec<W3DChunk>),
    Box(W3dBoxStruct),
    Sphere(W3dSphereStruct),
    Ring(W3dRingStruct),
    NullObject(W3dNullObjectStruct),
    Unknown { chunk_type: u32, data: Vec<u8> },
}

/// Helper functions for loading W3D files
pub fn load_w3d_file<P: AsRef<Path>>(path: P) -> W3DResult<Vec<W3DChunk>> {
    let file = std::fs::File::open(path)
        .map_err(|e| W3DError::IoError(format!("Failed to open file: {}", e)))?;
    let reader = std::io::BufReader::new(file);
    let mut w3d_reader = W3DReader::new(reader);
    w3d_reader.read_all_chunks()
}

/// Helper function for saving W3D files
pub fn save_w3d_file<P: AsRef<Path>>(path: P, chunks: &[W3DChunk]) -> W3DResult<()> {
    let file = std::fs::File::create(path)
        .map_err(|e| W3DError::IoError(format!("Failed to create file: {}", e)))?;
    let writer = std::io::BufWriter::new(file);
    let mut w3d_writer = W3DWriter::new(writer);

    for chunk in chunks {
        w3d_writer.write_chunk(chunk)?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_chunk_roundtrip() {
        let vertices = vec![
            W3dVectorStruct {
                x: 1.0,
                y: 2.0,
                z: 3.0,
            },
            W3dVectorStruct {
                x: 4.0,
                y: 5.0,
                z: 6.0,
            },
        ];

        let chunk = W3DChunk::Vertices(vertices.clone());

        // Write to buffer
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = W3DWriter::new(&mut buffer);
        writer.write_chunk(&chunk).unwrap();

        // Read back
        buffer.set_position(0);
        let mut reader = W3DReader::new(buffer);
        let read_chunk = reader.read_chunk().unwrap();

        match read_chunk {
            W3DChunk::Vertices(verts) => {
                assert_eq!(verts.len(), vertices.len());
                for (v1, v2) in verts.iter().zip(vertices.iter()) {
                    assert!((v1.x - v2.x).abs() < 0.001);
                    assert!((v1.y - v2.y).abs() < 0.001);
                    assert!((v1.z - v2.z).abs() < 0.001);
                }
            }
            _ => panic!("Expected Vertices chunk"),
        }
    }

    #[test]
    fn test_mesh_roundtrip() {
        let mut mesh = W3dMesh::new();
        mesh.vertices = vec![
            W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 1.0,
                y: 0.0,
                z: 0.0,
            },
            W3dVectorStruct {
                x: 0.0,
                y: 1.0,
                z: 0.0,
            },
        ];
        mesh.triangles = vec![W3dTriangleStruct {
            vindex: [0, 1, 2],
            attributes: 0,
            normal: W3dVectorStruct {
                x: 0.0,
                y: 0.0,
                z: 1.0,
            },
            distance: 0.0,
        }];

        let chunk = W3DChunk::Mesh(mesh);

        // Write to buffer
        let mut buffer = Cursor::new(Vec::new());
        let mut writer = W3DWriter::new(&mut buffer);
        writer.write_chunk(&chunk).unwrap();

        // Read back
        buffer.set_position(0);
        let mut reader = W3DReader::new(buffer);
        let read_chunk = reader.read_chunk().unwrap();

        match read_chunk {
            W3DChunk::Mesh(m) => {
                assert_eq!(m.vertices.len(), 3);
                assert_eq!(m.triangles.len(), 1);
            }
            _ => panic!("Expected Mesh chunk"),
        }
    }
}
